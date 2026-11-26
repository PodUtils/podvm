#!/usr/bin/env python3
"""Does a SIGTERM during the save of a guest state end the run at once?

TODO/machine.md T-2048, item (2). Run with Python 3.11 or later on an
x86_64 host. PODVM_480_BIN selects the binary. podvm fetches its own QEMU
where none is on PATH. The image is the one of experiment 473.

The run saves the state of a guest of `--podvm-init=image` under TCG.
When the partial file of the state appears, the experiment stops the
emulator with SIGSTOP, so the save cannot end, and sends SIGTERM to podvm.

Clauses:
  1. podvm ends on SIGTERM within 10 s of the signal.
  2. stderr names SIGTERM with one line.
  3. No emulator of the run stays.
  4. No partial file of the state stays, and no state is saved.

Exit 0 is a match, 1 is a failed clause, and 2 means a required input was
absent.
"""

import datetime
import hashlib
import os
from pathlib import Path
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
# The bound of the boot until the partial file of the state appears.
BOOT_BOUND = 300
# The bound of the end of podvm after SIGTERM.
END_BOUND = 10


def children(pid):
    """The pids of the processes whose parent is `pid`."""
    found = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            stat = (entry / "stat").read_text()
        except OSError:
            continue
        fields = stat[stat.rfind(")") + 2:].split()
        if len(fields) > 1 and fields[1] == str(pid):
            found.append(int(entry.name))
    return found


def emulators_of(pid):
    """The pids of each QEMU under `pid`, at any depth."""
    found, todo = [], [pid]
    while todo:
        for child in children(todo.pop()):
            todo.append(child)
            try:
                comm = Path(f"/proc/{child}/comm").read_text().strip()
            except OSError:
                continue
            if comm.startswith("qemu-system"):
                found.append(child)
    return found


def alive(pid):
    try:
        stat = Path(f"/proc/{pid}/stat").read_text()
    except OSError:
        return False
    return stat[stat.rfind(")") + 2:].split()[0] != "Z"


def prove(binary, work):
    store = work / "store"
    env = dict(os.environ, PODVM_STORE=str(store), PODVM_MACHINE_ACCEL="tcg",
               PODVM_MACHINE_STATE="1", PODVM_RUNSC="/nonexistent/runsc",
               HOME=str(work / "home"))
    env.pop("PODVM_MACHINE_KERNEL", None)
    (work / "home").mkdir()
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
    err = (work / "stderr").open("w+")
    run = subprocess.Popen(
        [binary, "run", "--rm", "--verbose", "--podvm-tier=machine",
         "--podvm-init=image", "--podvm-net", "none", IMAGE, "true"],
        env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=err,
    )
    emulators = []
    try:
        start = time.monotonic()
        part = []
        while time.monotonic() - start < BOOT_BOUND and run.poll() is None:
            part = list(machine.glob("*.vm.part-*")) if machine.is_dir() else []
            if part:
                break
            time.sleep(0.05)
        if not part:
            err.seek(0)
            print(f"COULD NOT RUN: no partial state within {BOOT_BOUND} s (exit {run.poll()})")
            print(err.read().replace(str(work), "WORK"), end="")
            return 2
        emulators = emulators_of(run.pid)
        for pid in emulators:
            os.kill(pid, signal.SIGSTOP)
        print(f"  | the partial state is there after {time.monotonic() - start:.1f} s; "
              f"{len(emulators)} emulator(s) stopped")
        time.sleep(1)
        sent = time.monotonic()
        run.send_signal(signal.SIGTERM)
        try:
            code = run.wait(timeout=60)
        except subprocess.TimeoutExpired:
            code = None
        took = time.monotonic() - sent
    finally:
        if run.poll() is None:
            run.kill()
            run.wait()
        for pid in emulators:
            if alive(pid):
                os.kill(pid, signal.SIGKILL)

    err.seek(0)
    text = err.read().replace(str(work), "WORK")
    print(text.rstrip())
    fail = 0

    def verdict(n, ok, what):
        nonlocal fail
        print(f"{'PASS' if ok else 'FAIL'} {n}: {what}")
        fail |= not ok

    verdict(1, code == -signal.SIGTERM and took <= END_BOUND,
            f"podvm ended with {code} {took:.1f} s after SIGTERM")
    lines = [l for l in text.splitlines() if "SIGTERM came" in l]
    verdict(2, len(lines) == 1, f"{len(lines)} line(s) of stderr name SIGTERM")
    time.sleep(1)
    left = [pid for pid in emulators if alive(pid)]
    verdict(3, emulators and not left, f"{len(left)} of {len(emulators)} emulator(s) stayed")
    parts = list(machine.glob("*.part-*"))
    states = list(machine.glob("*.vm"))
    verdict(4, not parts and not states,
            f"{len(parts)} partial file(s) and {len(states)} state(s) stayed")
    return fail


def main():
    if sys.version_info < (3, 11) or sys.platform != "linux" or os.uname().machine != "x86_64":
        print("COULD NOT RUN: use Python 3.11 or later on x86_64 Linux")
        return 2
    binary = Path(os.environ.get(
        "PODVM_480_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    if not os.access(binary, os.X_OK):
        print("COULD NOT RUN: give the podvm binary")
        return 2
    with binary.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    print("Does a SIGTERM during the save of a guest state end the run at once?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}, TCG")
    print(f"podvm SHA-256 {digest}\nimage {IMAGE}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-480-", dir="/var/tmp") as name:
        try:
            result = prove(binary, Path(name))
        except (subprocess.TimeoutExpired, OSError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
    if result == 0:
        print("A SIGTERM DURING A SAVE ENDS THE RUN AT ONCE")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
