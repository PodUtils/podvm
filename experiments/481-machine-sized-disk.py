#!/usr/bin/env python3
"""Does `--storage-opt size=N` bound the root of a machine guest?

TODO/machine.md T-1968 (PodUtils/podrunner#296 ask 5). Run with Python
3.11 or later on an x86_64 host, as a user that is not root. PODVM_481_BIN
selects the binary. podvm fetches its own QEMU where none is on PATH, and
the run uses TCG.

The run boots a guest with a disk of 64 MiB and the working directory
/work. The payload writes 16 MiB, then 96 MiB, prints the mount of the
root, and exits 7. A control run with no disk writes the same 96 MiB in
the RAM of the guest.

Clauses:
  1. The run with the disk exits 7, the code of the payload.
  2. The write of 16 MiB in /work succeeds.
  3. The write of 96 MiB fails with "No space left on device" and a
     status that is not 0.
  4. The root of the guest is an overlay whose upper layer is on the disk.
  5. No disk file and no run directory stay in the store.
  6. The control run with no disk writes the 96 MiB and exits 7.
  7. No emulator of the runs stays.
  8. `--storage-opt size=17T` exits 125 and names 16 TiB, with no boot.
  9. The size on a native tier exits 125 and names the machine tier.

Exit 0 is a match, 1 is a failed clause or a missing row, and 2 means a
required input was absent.
"""

import datetime
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
IMAGE = (
    "ghcr.io/linuxcontainers/alpine:latest"
    "@sha256:28a33e0390da2b341d05e53803c6766977039baedfc8a82a4910d5acc4755b66"
)
PAYLOAD = (
    "dd if=/dev/zero of=small bs=1M count=16 && echo SMALL-OK; "
    "dd if=/dev/zero of=big bs=1M count=96; echo BIG-RC=$?; "
    "grep ' / ' /proc/mounts; exit 7"
)
# The bound of one guest run under TCG.
RUN_BOUND = 600
CLAUSES = 9


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
    env.pop("PODVM_MACHINE_KERNEL", None)
    (work / "home").mkdir()

    def podvm(*args, bound=RUN_BOUND):
        r = subprocess.run([binary, *args], env=env, stdin=subprocess.DEVNULL,
                           capture_output=True, text=True, errors="replace",
                           timeout=bound)
        out = (r.stdout + r.stderr).replace(str(work), "WORK")
        lines = [l for l in out.splitlines() if not l.startswith("podvm: ") or "MiB of" not in l]
        print(f"$ podvm {' '.join(args)}\n  | exit {r.returncode}")
        for line in lines:
            print(f"  | {line}")
        return r.returncode, out

    code, out = podvm("pull", "-q", IMAGE, bound=480)
    if code != 0:
        print(f"COULD NOT RUN: the pinned image pull exited {code}")
        return 2

    rows = {}

    def verdict(n, ok, what):
        rows[n] = ok
        print(f"{'PASS' if ok else 'FAIL'} {n}: {what}")

    machine = ["run", "--rm", "--podvm-tier=machine", "--podvm-net", "none", "-w", "/work"]
    code, out = podvm(*machine, "--storage-opt", "size=64m", IMAGE, "sh", "-c", PAYLOAD)
    verdict(1, code == 7, f"the run with the disk exited {code}")
    verdict(2, "SMALL-OK" in out, "the write of 16 MiB in /work")
    big = [l for l in out.splitlines() if l.startswith("BIG-RC=")]
    verdict(3, "No space left on device" in out and big and big[0] != "BIG-RC=0",
            f"the write of 96 MiB: {big[0] if big else 'no status'}, ENOSPC "
            f"{'named' if 'No space left on device' in out else 'not named'}")
    verdict(4, "overlay / overlay" in out and "upperdir=/podvm-disk/upper" in out,
            "the root is an overlay with its upper layer on the disk")
    left = sorted(p.name for p in (store / "machine").glob("run-*")) + sorted(
        str(p.relative_to(store)) for p in store.rglob("disk.raw"))
    verdict(5, not left, f"{len(left)} disk file(s) or run directories stayed {left}")

    code, out = podvm(*machine, "--podvm-mem", "512m", IMAGE, "sh", "-c", PAYLOAD)
    verdict(6, code == 7 and "BIG-RC=0" in out and "No space" not in out,
            f"the control with no disk exited {code} and wrote 96 MiB in RAM")
    left = emulators(store)
    verdict(7, not left, f"{len(left)} emulator(s) of the runs stayed")

    code, out = podvm(*machine, "--storage-opt", "size=17T", IMAGE, "true", bound=120)
    verdict(8, code == 125 and "16 TiB" in out and "machine tier refused" in out,
            f"size=17T exited {code}")
    code, out = podvm("run", "--rm", "--podvm-tier=syscall", "--storage-opt", "size=64m",
                      IMAGE, "true", bound=120)
    verdict(9, code == 125 and "--podvm-tier=machine" in out,
            f"the size on the syscall tier exited {code}")

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
        "PODVM_481_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    if not os.access(binary, os.X_OK):
        print("COULD NOT RUN: give the podvm binary")
        return 2
    with binary.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    print("Does --storage-opt size=N bound the root of a machine guest?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}, TCG, no /dev/kvm: {not os.path.exists('/dev/kvm')}")
    print(f"podvm SHA-256 {digest}\nimage {IMAGE}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-481-", dir="/var/tmp") as name:
        try:
            result = prove(binary, Path(name))
        except (subprocess.TimeoutExpired, OSError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
        finally:
            for pid in emulators(Path(name)):
                os.kill(pid, 9)
    if result == 0:
        print("THE SIZED DISK BOUNDS THE ROOT OF A MACHINE GUEST")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
