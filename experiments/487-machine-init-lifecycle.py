#!/usr/bin/env python3
"""Does a guest with systemd as PID 1 hold the lifecycle of a container?

TODO/machine.md T-1994. Run with Python 3.11 or later on an x86_64 host,
as a user that is not root. PODVM_487_BIN selects the binary. podvm
fetches its own QEMU where none is on PATH, and the runs use TCG. The
image is the pinned Arch Linux image of experiments 473, 483 and 486.

Each guest runs with `--podvm-tier=machine --podvm-net none
--podvm-init=image`. The command of `run -d` prints `started`, traps
SIGTERM (it prints `got-term` and exits 5), and waits.

Clauses:
  1. `run -d --name lc` exits 0 and prints an id; `ps -a` shows lc Up.
  2. `logs lc` gives `started` once the guest runs the command.
  3. `exec lc`: PID 1 of the guest is systemd and PID 2 kthreadd,
     `systemctl is-system-running` gives running or degraded, and the
     exit code 3 passes.
  4. `exec -e A=b -w /tmp lc` gives A=b and PWD=/tmp, and exits 0.
  5. `exec -t lc` exits 125 and names the refusal (T-2043): the exec
     channel of a guest carries no terminal.
  6. `stop lc` gives the command SIGTERM: `ps -a` shows Exited (5),
     `inspect --format {{.State.ExitCode}}` gives 5, and `logs lc` has
     `got-term`.
  7. `rm lc` removes it: `ps -a` has no lc.
  8. A foreground `run -t` gives the command a terminal (`tty` is
     /dev/hvc0) and exits 6.
  9. A foreground `run` that gets SIGTERM after `started` gives the
     command the signal: its trap runs and the run exits 5.
 10. `create --podvm-tier=machine --podvm-init=image` exits 125 and names
     the refusal.
 11. No emulator, no run directory, no partial file and no machine stays.

Exit 0 is a match, 1 is a failed clause or a missing row, and 2 means a
required input was absent.
"""

import datetime
import hashlib
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
IMAGE = (
    "ghcr.io/pkgforge-dev/archlinux:latest"
    "@sha256:b2507f1964270cab3cc190aa8df858521f6a7e45e969146a012877891d5dfc9b"
)
WAITER = 'echo started; trap "echo got-term; exit 5" TERM; while :; do sleep 1; done'
RUN_BOUND = 900
CLAUSES = 11


def emulators(store):
    """The pids of each QEMU whose arguments name `store`."""
    found = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            cmd = (entry / "cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        if cmd and b"qemu-system" in cmd[0] and any(str(store).encode() in a for a in cmd):
            found.append(int(entry.name))
    return found


def prove(binary, work):
    store = work / "store"
    env = dict(os.environ, PODVM_STORE=str(store), PODVM_MACHINE_ACCEL="tcg",
               PODVM_RUNSC="/nonexistent/runsc", HOME=str(work / "home"))
    for k in ("PODVM_MACHINE_KERNEL", "PODVM_MACHINE_ROOT", "PODVM_MACHINE_STATE",
              "PODVM_INIT_BOOT_TIMEOUT"):
        env.pop(k, None)
    (work / "home").mkdir()

    def show(args, code, out, took):
        shown = " ".join(args).replace(WAITER, "WAITER").replace(str(work), "WORK")
        print(f"$ podvm {shown}\n  | exit {code} in {took:.1f} s")
        for line in out.replace(str(work), "WORK").splitlines():
            if "MiB of" not in line:
                print(f"  | {line}")

    def podvm(*args, bound=RUN_BOUND):
        start = time.monotonic()
        r = subprocess.run([binary, *args], env=env, stdin=subprocess.DEVNULL,
                           capture_output=True, text=True, errors="replace", timeout=bound)
        show(args, r.returncode, r.stdout + r.stderr, time.monotonic() - start)
        return r.returncode, r.stdout, r.stderr

    code, _, _ = podvm("pull", "-q", IMAGE)
    if code != 0:
        print(f"COULD NOT RUN: the pinned image pull exited {code}")
        return 2

    rows = {}

    def verdict(n, ok, what):
        rows[n] = ok
        print(f"{'PASS' if ok else 'FAIL'} {n}: {what}", flush=True)

    def status(out, name):
        for line in out.splitlines():
            if line.split() and line.split()[-1] == name:
                m = re.search(r"\s(Up|Exited(?: \(\d+\))?|Created)\s", line)
                return m[1] if m else line
        return None

    guest = ["--podvm-tier=machine", "--podvm-net", "none", "--podvm-init=image"]
    code, out, _ = podvm("run", "-d", "--name", "lc", *guest, IMAGE, "sh", "-c", WAITER)
    _, ps, _ = podvm("ps", "-a")
    verdict(1, code == 0 and re.fullmatch(r"[0-9a-f]{64}\n", out) is not None
            and status(ps, "lc") == "Up", f"run -d exited {code}, lc is {status(ps, 'lc')}")
    t0 = time.monotonic()
    logs = ""
    while time.monotonic() - t0 < 300:
        _, logs, _ = podvm("logs", "lc", bound=60)
        if "started" in logs:
            break
        time.sleep(5)
    verdict(2, "started" in logs,
            f"logs gave started after {time.monotonic() - t0:.0f} s of the wait")
    code, out, _ = podvm("exec", "lc", "sh", "-c",
                         "echo PID1=$(cat /proc/1/comm) PID2=$(cat /proc/2/comm) "
                         "STATE=$(systemctl is-system-running); exit 3")
    m = re.search(r"PID1=(\S+) PID2=(\S+) STATE=(\S+)", out)
    verdict(3, code == 3 and m is not None and m[1] == "systemd" and m[2] == "kthreadd"
            and m[3] in ("running", "degraded"),
            f"exec exited {code}: {m[0] if m else 'no line'}")
    code, out, _ = podvm("exec", "-e", "A=b", "-w", "/tmp", "lc", "sh", "-c",
                         "echo A=$A PWD=$PWD")
    verdict(4, code == 0 and "A=b PWD=/tmp" in out, f"exec -e -w exited {code}")
    code, _, err = podvm("exec", "-t", "lc", "tty")
    verdict(5, code == 125 and "carries no terminal" in err,
            f"exec -t exited {code} and {'names' if 'carries no terminal' in err else 'does not name'} "
            f"the refusal")
    start = time.monotonic()
    code, _, _ = podvm("stop", "lc")
    took = time.monotonic() - start
    _, ps, _ = podvm("ps", "-a")
    _, exit_code, _ = podvm("inspect", "--format", "{{.State.ExitCode}}", "lc")
    _, logs, _ = podvm("logs", "lc")
    verdict(6, code == 0 and status(ps, "lc") == "Exited (5)" and exit_code.strip() == "5"
            and "got-term" in logs,
            f"stop exited {code} in {took:.1f} s; lc is {status(ps, 'lc')}, exit code "
            f"{exit_code.strip()}, got-term in the logs: {'got-term' in logs}")
    code, _, _ = podvm("rm", "lc")
    _, ps, _ = podvm("ps", "-a")
    verdict(7, code == 0 and status(ps, "lc") is None, f"rm exited {code}")
    code, out, _ = podvm("run", "--rm", "-t", *guest, IMAGE, "sh", "-c", "tty; exit 6")
    verdict(8, code == 6 and "/dev/hvc0" in out, f"run -t exited {code}")

    args = ["run", "--rm", *guest, IMAGE, "sh", "-c", WAITER]
    start = time.monotonic()
    p = subprocess.Popen([binary, *args], env=env, stdin=subprocess.DEVNULL,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                         errors="replace")
    lines = []
    for line in p.stdout:
        lines.append(line)
        if line.strip() == "started":
            p.send_signal(signal.SIGTERM)
            break
    rest, err = p.communicate(timeout=RUN_BOUND)
    show(args + ["(SIGTERM after started)"], p.returncode, "".join(lines) + rest + err,
         time.monotonic() - start)
    verdict(9, p.returncode == 5 and "got-term" in rest,
            f"the foreground run that got SIGTERM exited {p.returncode}")

    code, _, err = podvm("create", *guest, IMAGE, "true", bound=120)
    verdict(10, code == 125 and "--podvm-init=image" in err, f"create exited {code}")

    left = emulators(store)
    stale = sorted(p.name for p in (store / "machine").glob("run-*")) + sorted(
        p.name for p in (store / "machine").glob("*partial*"))
    _, ps, _ = podvm("ps", "-a")
    verdict(11, not left and not stale and len(ps.splitlines()) == 1,
            f"{len(left)} emulator(s), run directories or partial files {stale}, "
            f"{len(ps.splitlines()) - 1} container(s)")

    missing = [n for n in range(1, CLAUSES + 1) if n not in rows]
    if missing:
        print(f"FAIL: no row for clause(s) {missing}")
    return 0 if not missing and all(rows.values()) else 1


def main():
    if sys.version_info < (3, 11) or sys.platform != "linux" or os.uname().machine != "x86_64":
        print("COULD NOT RUN: use Python 3.11 or later on x86_64 Linux")
        return 2
    if os.getuid() == 0:
        print("COULD NOT RUN: run as a user that is not root")
        return 2
    binary = Path(os.environ.get(
        "PODVM_487_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    if not os.access(binary, os.X_OK):
        print("COULD NOT RUN: give the podvm binary")
        return 2
    with binary.open("rb") as f:
        digest = hashlib.file_digest(f, "sha256").hexdigest()
    print("Does a guest with systemd as PID 1 hold the lifecycle of a container?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}, TCG, no /dev/kvm: {not os.path.exists('/dev/kvm')}")
    print(f"podvm SHA-256 {digest}\nimage {IMAGE}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-487-", dir="/var/tmp") as name:
        try:
            result = prove(binary, Path(name))
        except (subprocess.TimeoutExpired, OSError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
        finally:
            for pid in emulators(Path(name)):
                os.kill(pid, 9)
            subprocess.run(["pkill", "-9", "-f", name], check=False)
    if result == 0:
        print("A GUEST WITH SYSTEMD AS PID 1 HOLDS THE LIFECYCLE OF A CONTAINER")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
