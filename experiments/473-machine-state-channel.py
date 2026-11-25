#!/usr/bin/env python3
"""Does one TCG state serve different channels and process limits?

TODO/machine.md T-2094. Run with Python 3.11 or later and the pinned
amd64 kernel and modules of podvm. PODVM_473_BIN selects the binary.
PODVM_QEMU selects QEMU, as with podvm. The OCI digest is from experiment 465.
Each run has a 180 s bound. Exit 0 is a match, 1 is a failed assertion,
and 2 means a required input was absent.
"""

import datetime
import hashlib
import os
from pathlib import Path
import re
import shutil
import socketserver
import subprocess
import sys
import tempfile
import threading
import time


ROOT = Path(__file__).resolve().parents[1]
IMAGE = (
    "ghcr.io/pkgforge-dev/archlinux:latest"
    "@sha256:b2507f1964270cab3cc190aa8df858521f6a7e45e969146a012877891d5dfc9b"
)
PAYLOAD = r"""
set -o pipefail
test "$(cat /proc/1/comm)" = systemd
exec 3<>"/dev/tcp/127.0.0.1/$1"
printf 'job-%s\n' "$3" >&3
IFS= read -r -t 15 reply <&3
test "$reply" = "answer-$3"
echo "channel=$reply"

# A fresh cgroup namespace roots this mount at the command's actual
# cgroup, even if the mounts of the image's init hide that cgroup.
limit=$(unshare --cgroup --mount -- sh -ec '
    mkdir -p /run/podvm-473
    mount -t cgroup2 cgroup2 /run/podvm-473
    cat /run/podvm-473/pids.max
')
echo "limit=$limit"
test "$limit" = "$2"
printf 'clock=%s\n' "$(date +%s)"
printf 'seed='
od -An -v -tx1 -N32 /dev/urandom | tr -d ' \n'
echo
printf 'boot=%s\n' "$(cat /proc/sys/kernel/random/boot_id)"
"""


class Echo(socketserver.StreamRequestHandler):
    def handle(self):
        self.request.settimeout(15)
        self.server.received.append(self.rfile.readline(64))
        self.wfile.write(f"answer-{self.server.job}\n".encode())


def check(test, message):
    if not test:
        raise AssertionError(message)


def launch(binary, env, work, job, port, pids, restore):
    socket = work / f"{job}.sock"
    with socketserver.UnixStreamServer(str(socket), Echo) as server:
        server.job = job
        server.received = []
        relay = threading.Thread(target=server.serve_forever)
        relay.start()
        try:
            before = int(time.time())
            result = subprocess.run(
                [
                    binary, "run", "--rm", "--verbose", "--podvm-tier=machine",
                    "--podvm-init=image", "--podvm-net", "none", "--podvm-cpus", "2",
                    "--podvm-channel", f"{socket}:127.0.0.1:{port}",
                    "--pids-limit", str(pids), IMAGE,
                    "bash", "-ec", PAYLOAD, "--", str(port), str(pids), str(job),
                ],
                env=env, stdin=subprocess.DEVNULL, capture_output=True,
                text=True, errors="replace", timeout=180,
            )
            after = int(time.time())
        finally:
            server.shutdown()
            relay.join()

    mode = "load" if restore else "save"
    diagnostics = result.stderr.replace(str(work), "WORK")
    print(f"job {job}: {mode}, port {port}, pids {pids}, exit {result.returncode}")
    for line in result.stdout.splitlines():
        key, _, _ = line.partition("=")
        print(f"{key}=<measured>" if key in {"seed", "boot"} else line)
    print(diagnostics.rstrip(), flush=True)
    check(result.returncode == 0, f"job {job}: guest command failed")
    expected = (
        "the guest starts from the saved state"
        if restore else "the state of the guest is saved:"
    )
    check(expected in diagnostics, f"job {job}: expected a state {mode}")
    check("did not reach the command" not in diagnostics, f"job {job}: cold fallback")
    check(server.received == [f"job-{job}\n".encode()], f"job {job}: wrong host request")
    values = dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)
    check(values.get("channel") == f"answer-{job}", f"job {job}: wrong reply")
    check(values.get("limit") == str(pids), f"job {job}: wrong guest limit")
    clock = values.get("clock", "")
    check(clock.isdigit() and before - 1 <= int(clock) <= after + 1, f"job {job}: stale clock")
    check(re.fullmatch(r"[0-9a-f]{64}", values.get("seed", "")), f"job {job}: missing random bytes")
    check(re.fullmatch(r"[0-9a-f-]{36}", values.get("boot", "")), f"job {job}: missing boot ID")
    return values


def emulators(store, action, timeout):
    return subprocess.check_output(
        ["sh", "-c", '. "$1"\nKVM=$2\n"$3"', "sh",
         ROOT / "experiments/lib/kvm-owned.sh", store, action],
        stdin=subprocess.DEVNULL, text=True, timeout=timeout,
    ).splitlines()


def prove(binary, work):
    store = work / "store"
    env = dict(os.environ, PODVM_STORE=str(store), PODVM_MACHINE_ACCEL="tcg",
               PODVM_MACHINE_STATE="1", PODVM_RUNSC="/nonexistent/runsc")
    env.pop("PODVM_MACHINE_KERNEL", None)
    try:
        pull = subprocess.run(
            [binary, "pull", "-q", IMAGE], env=env, stdin=subprocess.DEVNULL,
            capture_output=True, text=True, errors="replace", timeout=480,
        )
    except subprocess.TimeoutExpired:
        print("COULD NOT RUN: the pinned image pull reached its bound")
        return 2
    if pull.returncode != 0:
        print(f"COULD NOT RUN: the pinned image pull exited {pull.returncode}")
        print(pull.stderr.replace(str(work), "WORK"), end="")
        return 2

    machine = store / "machine"
    first = launch(binary, env, work, 1, 41001, 64, restore=False)
    saved = set(machine.glob("state-*.vm"))
    check(len(saved) == 1, "the first run must save one state")
    time.sleep(2)
    second = launch(binary, env, work, 2, 41002, 64, restore=True)
    third = launch(binary, env, work, 3, 41003, 96, restore=True)
    check(set(machine.glob("state-*.vm")) == saved, "the change of run settings made another state")
    check(not list(machine.glob("state-*.bad")), "a state failure marker stays")
    check(first["boot"] == second["boot"] == third["boot"], "the guest booted again")
    check(len({first["seed"], second["seed"], third["seed"]}) == 3, "the random stream repeated")
    return 0


def main():
    if sys.version_info < (3, 11) or sys.platform != "linux":
        print("COULD NOT RUN: use Python 3.11 or later on Linux")
        return 2
    binary = Path(os.environ.get(
        "PODVM_473_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    qemu = shutil.which(os.environ.get("PODVM_QEMU", "qemu-system-x86_64"))
    if not os.access(binary, os.X_OK) or qemu is None or os.uname().machine != "x86_64":
        print("COULD NOT RUN: give the podvm binary and QEMU on an x86_64 host")
        return 2
    try:
        version = subprocess.check_output([qemu, "--version"], text=True, timeout=10)
    except (OSError, subprocess.SubprocessError):
        print("COULD NOT RUN: QEMU does not report its version")
        return 2
    with binary.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    print("Does one TCG state serve different channels and process limits?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}, TCG with two vCPUs, {version.splitlines()[0]}")
    print(f"podvm SHA-256 {digest}\nimage {IMAGE}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-473-", dir="/tmp") as name:
        work = Path(name)
        try:
            result = prove(binary, work)
        except (AssertionError, subprocess.TimeoutExpired, OSError) as error:
            print(f"FAIL: {str(error).replace(str(work), 'WORK')}")
            result = 1
        store = work / "store"
        left = emulators(store, "owned_emulators", 10)
        if left:
            print(f"FAIL: {len(left)} emulator processes stayed after the run")
            emulators(store, "stop_owned_emulators", 20)
            return 1
        if result == 0:
            print("PASS: the restored guest used each host socket, guest port, limit, clock and seed")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
