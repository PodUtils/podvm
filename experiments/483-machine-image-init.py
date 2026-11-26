#!/usr/bin/env python3
"""Is systemd of the image PID 1 of a machine guest, and does the exit
code of the command come back?

TODO/machine.md T-1994, steps 2 and 3 (PodUtils/podrunner#313 ask 2, #316
ask 2). Run with Python 3.11 or later on an x86_64 host, as a user that is
not root. PODVM_483_BIN selects the binary. podvm fetches its own QEMU
where none is on PATH, and the runs use TCG.

The script pulls the pinned Arch Linux image of experiment 473 and makes
two more images from it with `save` and `load`: one with a unit that
fails at the boot, and one with a unit that never ends its start. Each run
is `run --rm --verbose --podvm-tier=machine --podvm-net none
--podvm-init=image`. The saved state of T-2008 takes each run that it
serves first (call C27), so clauses 1 to 4 set PODVM_MACHINE_STATE=0, and
clause 5 is a run that a state does not serve.

Clauses:
  1. A run exits 7: PID 1 of the guest is systemd and PID 2 is kthreadd
     (the PID namespace of the guest itself), `systemctl
     is-system-running` gives running or degraded, the command is in the
     cgroup of podvm-run.service, and the notice names systemd as PID 1.
  2. A run of `exit 0` exits 0.
  3. With the unit that fails, the state is degraded, the unit is failed,
     and the command runs and exits 7.
  4. With the unit that never ends its start and PODVM_INIT_BOOT_TIMEOUT=60,
     the run exits 125 with one line that names the bound, the command
     does not run, and the run ends before 60 s and the boot of the kernel
     plus 180 s.
  5. With the default state and `--storage-opt size=256m`, which a state
     does not serve, the run takes the route of clause 1 and exits 7.
  6. With the default state, the first run saves a state and the second
     starts from it: each exits 7 with systemd as PID 1 of the guest
     (PID 2 is kthreadd): the state is on the root disk (T-1994,
     experiment 486).
  7. The control with PODVM_MACHINE_ROOT=initramfs and
     PODVM_MACHINE_STATE=0 (the route of T-2002) exits 7 with systemd as
     PID 1 of a PID namespace: PID 2 is not kthreadd.
  8. PODVM_INIT_BOOT_TIMEOUT=abc exits 125 and names the variable, with no
     boot.
  9. No emulator, no run directory and no partial file stay.

Exit 0 is a match, 1 is a failed clause or a missing row, and 2 means a
required input was absent.
"""

import datetime
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
IMAGE = (
    "ghcr.io/pkgforge-dev/archlinux:latest"
    "@sha256:b2507f1964270cab3cc190aa8df858521f6a7e45e969146a012877891d5dfc9b"
)
PAYLOAD = (
    "echo PID1=$(cat /proc/1/comm) PID2=$(cat /proc/2/comm); "
    "echo STATE=$(systemctl is-system-running); "
    "echo FAILED=$(systemctl is-failed podvm483-fail.service); "
    "echo CGROUP=$(cat /proc/self/cgroup); echo COMMAND-RAN; exit 7"
)
RUN_BOUND = 900
CLAUSES = 9
OFF = {"PODVM_MACHINE_STATE": "0"}
UNITS = {
    "fail": "[Unit]\nDescription=A unit that fails\n\n[Service]\nType=oneshot\n"
            "ExecStart=/usr/bin/false\n",
    "hang": "[Unit]\nDescription=A unit that never ends its start\n\n[Service]\n"
            "Type=oneshot\nExecStart=/usr/bin/sleep infinity\nTimeoutStartSec=infinity\n",
}


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


def add_unit(src, dst, kind, ref):
    """The saved image `src` with one more layer that enables the unit of
    `kind` in multi-user.target, written to `dst` as `ref`."""
    blobs = {}
    with tarfile.open(src) as t:
        for m in t.getmembers():
            if m.isfile():
                blobs[m.name] = t.extractfile(m).read()
    idx = json.loads(blobs["index.json"])
    mdesc = idx["manifests"][0]
    man = json.loads(blobs["blobs/" + mdesc["digest"].replace(":", "/")])
    cfg = json.loads(blobs["blobs/" + man["config"]["digest"].replace(":", "/")])
    name = f"podvm483-{kind}.service"
    lay = io.BytesIO()
    with tarfile.open(fileobj=lay, mode="w", format=tarfile.PAX_FORMAT) as t:
        for d in ("etc", "etc/systemd", "etc/systemd/system",
                  "etc/systemd/system/multi-user.target.wants"):
            i = tarfile.TarInfo(d)
            i.type = tarfile.DIRTYPE
            i.mode = 0o755
            t.addfile(i)
        data = UNITS[kind].encode()
        i = tarfile.TarInfo(f"etc/systemd/system/{name}")
        i.size = len(data)
        i.mode = 0o644
        t.addfile(i, io.BytesIO(data))
        i = tarfile.TarInfo(f"etc/systemd/system/multi-user.target.wants/{name}")
        i.type = tarfile.SYMTYPE
        i.linkname = f"../{name}"
        t.addfile(i)
    layer = lay.getvalue()

    def put(b):
        d = "sha256:" + hashlib.sha256(b).hexdigest()
        blobs["blobs/" + d.replace(":", "/")] = b
        return d

    blobs.pop("blobs/" + mdesc["digest"].replace(":", "/"))
    blobs.pop("blobs/" + man["config"]["digest"].replace(":", "/"))
    ld = put(layer)
    man["layers"].append({"mediaType": "application/vnd.oci.image.layer.v1.tar",
                          "digest": ld, "size": len(layer)})
    cfg["rootfs"]["diff_ids"].append(ld)
    cb = json.dumps(cfg).encode()
    man["config"]["digest"] = put(cb)
    man["config"]["size"] = len(cb)
    mb = json.dumps(man).encode()
    mdesc["digest"] = put(mb)
    mdesc["size"] = len(mb)
    mdesc.setdefault("annotations", {})["org.opencontainers.image.ref.name"] = ref
    blobs["index.json"] = json.dumps(idx).encode()
    with tarfile.open(dst, "w") as t:
        for n, b in blobs.items():
            i = tarfile.TarInfo(n)
            i.size = len(b)
            t.addfile(i, io.BytesIO(b))


def prove(binary, work):
    store = work / "store"
    env = dict(os.environ, PODVM_STORE=str(store), PODVM_MACHINE_ACCEL="tcg",
               PODVM_RUNSC="/nonexistent/runsc", HOME=str(work / "home"))
    for k in ("PODVM_MACHINE_KERNEL", "PODVM_MACHINE_ROOT", "PODVM_MACHINE_STATE",
              "PODVM_INIT_BOOT_TIMEOUT"):
        env.pop(k, None)
    (work / "home").mkdir()

    def podvm(*args, bound=RUN_BOUND, extra=None):
        start = time.monotonic()
        r = subprocess.run([binary, *args], env=dict(env, **(extra or {})),
                           stdin=subprocess.DEVNULL, capture_output=True, text=True,
                           errors="replace", timeout=bound)
        took = time.monotonic() - start
        out = (r.stdout + r.stderr).replace(str(work), "WORK")
        shown = " ".join(args).replace(PAYLOAD, "PAYLOAD").replace(str(work), "WORK")
        prefix = "".join(f"{k}={v} " for k, v in (extra or {}).items())
        print(f"$ {prefix}podvm {shown}\n  | exit {r.returncode} in {took:.1f} s")
        for line in out.splitlines():
            if "MiB of" not in line:
                print(f"  | {line}")
        return r.returncode, r.stdout, r.stderr, took

    code, _, _, _ = podvm("pull", "-q", IMAGE, bound=900)
    if code != 0:
        print(f"COULD NOT RUN: the pinned image pull exited {code}")
        return 2
    code, _, _, _ = podvm("save", "-o", str(work / "arch.tar"), IMAGE, bound=600)
    if code != 0:
        print(f"COULD NOT RUN: the save of the image exited {code}")
        return 2
    refs = {}
    for kind in UNITS:
        refs[kind] = f"localhost/init483-{kind}:1"
        add_unit(work / "arch.tar", work / f"{kind}.tar", kind, refs[kind])
        code, _, _, _ = podvm("load", "-i", str(work / f"{kind}.tar"), bound=600)
        (work / f"{kind}.tar").unlink()
        if code != 0:
            print(f"COULD NOT RUN: the load of the image with the unit {kind} exited {code}")
            return 2
    (work / "arch.tar").unlink()

    rows = {}

    def verdict(n, ok, what):
        rows[n] = ok
        print(f"{'PASS' if ok else 'FAIL'} {n}: {what}", flush=True)

    def field(out, key):
        m = re.search(rf"^{key}=(.*)$", out, re.M)
        return m[1].strip() if m else None

    run = ["run", "--rm", "--verbose", "--podvm-tier=machine", "--podvm-net", "none",
           "--podvm-init=image"]
    code, out, err, _ = podvm(*run, IMAGE, "sh", "-c", PAYLOAD, extra=OFF)
    pid = field(out, "PID1") or ""
    verdict(1, code == 7 and pid == "systemd PID2=kthreadd"
            and field(out, "STATE") in ("running", "degraded")
            and "podvm-run.service" in (field(out, "CGROUP") or "")
            and "systemd of the image as PID 1 of the guest" in err,
            f"exit {code}, PID1={pid}, state {field(out, 'STATE')}, "
            f"cgroup {field(out, 'CGROUP')}")
    code, _, _, _ = podvm(*run, IMAGE, "sh", "-c", "exit 0", extra=OFF)
    verdict(2, code == 0, f"the run of exit 0 exited {code}")
    code, out, _, _ = podvm(*run, refs["fail"], "sh", "-c", PAYLOAD, extra=OFF)
    verdict(3, code == 7 and field(out, "STATE") == "degraded"
            and field(out, "FAILED") == "failed" and "COMMAND-RAN" in out,
            f"with a failed unit: exit {code}, state {field(out, 'STATE')}, "
            f"the unit {field(out, 'FAILED')}")
    code, out, err, took = podvm(*run, refs["hang"], "sh", "-c", PAYLOAD,
                                 extra={"PODVM_INIT_BOOT_TIMEOUT": "60", **OFF})
    lines = [l for l in err.splitlines() if "did not finish its boot in 60 s" in l]
    verdict(4, code == 125 and len(lines) == 1 and "COMMAND-RAN" not in out and took < 240,
            f"with a unit that never ends its start: exit {code} after {took:.0f} s, "
            f"{len(lines)} line(s) that name the bound, the command "
            f"{'ran' if 'COMMAND-RAN' in out else 'did not run'}")
    code, out, err, _ = podvm(*run, "--storage-opt", "size=256m", IMAGE, "sh", "-c", PAYLOAD)
    pid = field(out, "PID1") or ""
    verdict(5, code == 7 and pid == "systemd PID2=kthreadd"
            and "systemd of the image as PID 1 of the guest" in err,
            f"a run that a state does not serve: exit {code}, PID1={pid}")
    states = []
    for _ in range(2):
        code, out, err, took = podvm(*run, IMAGE, "sh", "-c", PAYLOAD)
        states.append((code, field(out, "PID1") or "", err, took))
    (c1, p1, e1, t1), (c2, p2, e2, t2) = states
    verdict(6, c1 == 7 and c2 == 7 and "saves its state" in e1
            and "starts from the saved state" in e2
            and all(p == "systemd PID2=kthreadd" for p in (p1, p2)),
            f"the default state: the save exited {c1} in {t1:.0f} s, the load {c2} in "
            f"{t2:.0f} s; PID1={p1} and {p2}")
    code, out, _, _ = podvm(*run, IMAGE, "sh", "-c", PAYLOAD,
                            extra={"PODVM_MACHINE_ROOT": "initramfs", **OFF})
    pid = field(out, "PID1") or ""
    verdict(7, code == 7 and pid.startswith("systemd ") and not pid.endswith("kthreadd"),
            f"the control of T-2002: exit {code}, PID1={pid}")
    code, _, err, _ = podvm(*run, IMAGE, "true", bound=120,
                            extra={"PODVM_INIT_BOOT_TIMEOUT": "abc"})
    verdict(8, code == 125 and "PODVM_INIT_BOOT_TIMEOUT=abc" in err,
            f"PODVM_INIT_BOOT_TIMEOUT=abc exited {code}")
    left = emulators(store)
    stale = sorted(p.name for p in (store / "machine").glob("run-*")) + sorted(
        p.name for p in (store / "machine").glob("*partial*"))
    verdict(9, not left and not stale,
            f"{len(left)} emulator(s), and run directories or partial files {stale}")

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
        "PODVM_483_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    if not os.access(binary, os.X_OK):
        print("COULD NOT RUN: give the podvm binary")
        return 2
    with binary.open("rb") as f:
        digest = hashlib.file_digest(f, "sha256").hexdigest()
    print("Is systemd of the image PID 1 of a machine guest, and does the exit code "
          "of the command come back?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}, TCG, no /dev/kvm: {not os.path.exists('/dev/kvm')}")
    print(f"podvm SHA-256 {digest}\nimage {IMAGE}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-483-", dir="/var/tmp") as name:
        try:
            result = prove(binary, Path(name))
        except (subprocess.TimeoutExpired, OSError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
        finally:
            for pid in emulators(Path(name)):
                os.kill(pid, 9)
    if result == 0:
        print("SYSTEMD OF THE IMAGE IS PID 1 OF THE GUEST, AND THE EXIT CODE COMES BACK")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
