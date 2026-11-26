#!/usr/bin/env python3
"""Does the saved state of a machine guest hold systemd as PID 1 on the
root disk of the image?

TODO/machine.md T-1994 and T-2008. Run with Python 3.11 or later on an
x86_64 host, as a user that is not root. PODVM_486_BIN selects the
binary. podvm fetches its own QEMU where none is on PATH, and the runs use
TCG. The image is the pinned Arch Linux image of experiments 473 and 483.

Each run is `run --rm --verbose --podvm-tier=machine --podvm-net none
--podvm-init=image` with the default saved state (PODVM_MACHINE_STATE
unset).

Clauses:
  1. The first run saves a state: its guest has systemd as PID 1 and
     kthreadd as PID 2 (the PID namespace of the guest itself), the
     command is in podvm-run.service, and the run exits 7.
  2. The second run starts from the saved state: PID 1 is systemd and PID
     2 kthreadd, the boot id is the one of the save, the first output
     comes within 15 s of the start, and the run exits 7.
  3. Three runs with `--podvm-channel` and `--pids-limit` (the checks of
     experiment 473): the first saves a state of its own (the channel
     device is in its line), the two others start from it; each run
     reaches its own host socket through its own guest port, and reads
     its own process limit; the boot id stays the one of the save and the
     random bytes differ.
  4. Another image digest (one more layer) saves a state of its own, and
     does not start from the state of clause 1.
  5. After a changed byte of the root disk, the next run builds the disk
     again with the same SHA-256 and starts from the saved state.
  6. PODVM_MACHINE_ROOT=initramfs saves a state of its own with systemd in
     a PID namespace (PID 2 is not kthreadd), as in 0.2.0-beta.41.
  7. No emulator, no run directory and no partial file stay, and no
     marker of a failed state.

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
import socketserver
import subprocess
import sys
import tarfile
import tempfile
import threading
import time


ROOT = Path(__file__).resolve().parents[1]
IMAGE = (
    "ghcr.io/pkgforge-dev/archlinux:latest"
    "@sha256:b2507f1964270cab3cc190aa8df858521f6a7e45e969146a012877891d5dfc9b"
)
PAYLOAD = (
    "echo PID1=$(cat /proc/1/comm) PID2=$(cat /proc/2/comm); "
    "echo CGROUP=$(cat /proc/self/cgroup); "
    "echo BOOT=$(cat /proc/sys/kernel/random/boot_id); exit 7"
)
CHANNEL = r"""
set -o pipefail
exec 3<>"/dev/tcp/127.0.0.1/$1"
printf 'job-%s\n' "$3" >&3
IFS= read -r -t 15 reply <&3
echo "CHANNEL=$reply"
limit=$(unshare --cgroup --mount -- sh -ec '
    mkdir -p /run/podvm-485
    mount -t cgroup2 cgroup2 /run/podvm-485
    cat /run/podvm-485/pids.max
')
echo "LIMIT=$limit"
echo "PID1=$(cat /proc/1/comm) PID2=$(cat /proc/2/comm)"
echo "BOOT=$(cat /proc/sys/kernel/random/boot_id)"
printf 'SEED='; od -An -v -tx1 -N32 /dev/urandom | tr -d ' \n'; echo
"""
RUN_BOUND = 900
CLAUSES = 7
SAVED = "the state of the guest is saved:"
LOADED = "the guest starts from the saved state"


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


def add_layer(src, dst, ref):
    """The saved image `src` with one more small layer, as `ref`."""
    blobs = {}
    with tarfile.open(src) as t:
        for m in t.getmembers():
            if m.isfile():
                blobs[m.name] = t.extractfile(m).read()
    idx = json.loads(blobs["index.json"])
    mdesc = idx["manifests"][0]
    man = json.loads(blobs["blobs/" + mdesc["digest"].replace(":", "/")])
    cfg = json.loads(blobs["blobs/" + man["config"]["digest"].replace(":", "/")])
    lay = io.BytesIO()
    with tarfile.open(fileobj=lay, mode="w", format=tarfile.PAX_FORMAT) as t:
        data = b"another digest\n"
        i = tarfile.TarInfo("etc/podvm-485")
        i.size = len(data)
        i.mode = 0o644
        t.addfile(i, io.BytesIO(data))
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


class Echo(socketserver.StreamRequestHandler):
    def handle(self):
        self.request.settimeout(15)
        self.server.received.append(self.rfile.readline(64))
        self.wfile.write(f"answer-{self.server.job}\n".encode())


def prove(binary, work):
    store = work / "store"
    env = dict(os.environ, PODVM_STORE=str(store), PODVM_MACHINE_ACCEL="tcg",
               PODVM_RUNSC="/nonexistent/runsc", HOME=str(work / "home"))
    for k in ("PODVM_MACHINE_KERNEL", "PODVM_MACHINE_ROOT", "PODVM_MACHINE_STATE",
              "PODVM_INIT_BOOT_TIMEOUT"):
        env.pop(k, None)
    (work / "home").mkdir()

    def podvm(*args, bound=RUN_BOUND, extra=None):
        """Run podvm: its exit code, stdout, stderr, the seconds to the
        first line of stdout and the seconds of the run."""
        start = time.monotonic()
        p = subprocess.Popen([binary, *args], env=dict(env, **(extra or {})),
                             stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE, text=True, errors="replace")
        err = []
        drain = threading.Thread(target=lambda: err.append(p.stderr.read()))
        drain.start()
        first, out = None, []
        for line in p.stdout:
            if first is None:
                first = time.monotonic() - start
            out.append(line)
        try:
            code = p.wait(timeout=bound)
        except subprocess.TimeoutExpired:
            p.kill()
            code = p.wait()
        drain.join()
        took = time.monotonic() - start
        stdout, stderr = "".join(out), "".join(err)
        text = (stdout + stderr).replace(str(work), "WORK")
        shown = " ".join(args).replace(PAYLOAD, "PAYLOAD").replace(CHANNEL, "CHANNEL")
        shown = shown.replace(str(work), "WORK")
        prefix = "".join(f"{k}={v} " for k, v in (extra or {}).items())
        print(f"$ {prefix}podvm {shown}\n  | exit {code} in {took:.1f} s, first output after "
              f"{'-' if first is None else f'{first:.1f} s'}")
        for line in text.splitlines():
            if "MiB of" in line:
                continue
            key = line.partition("=")[0]
            print(f"  | {key}=<measured>" if key == "SEED" else f"  | {line}")
        return code, stdout, stderr, first, took

    code, _, _, _, _ = podvm("pull", "-q", IMAGE, bound=900)
    if code != 0:
        print(f"COULD NOT RUN: the pinned image pull exited {code}")
        return 2
    code, _, _, _, _ = podvm("save", "-o", str(work / "arch.tar"), IMAGE, bound=600)
    if code != 0:
        print(f"COULD NOT RUN: the save of the image exited {code}")
        return 2
    other = "localhost/state485:1"
    add_layer(work / "arch.tar", work / "other.tar", other)
    (work / "arch.tar").unlink()
    code, _, _, _, _ = podvm("load", "-i", str(work / "other.tar"), bound=600)
    (work / "other.tar").unlink()
    if code != 0:
        print(f"COULD NOT RUN: the load of the other image exited {code}")
        return 2

    rows = {}

    def verdict(n, ok, what):
        rows[n] = ok
        print(f"{'PASS' if ok else 'FAIL'} {n}: {what}", flush=True)

    def field(out, key):
        m = re.search(rf"^{key}=(.*)$", out, re.M)
        return m[1].strip() if m else None

    def states():
        return sorted(p.name for p in (store / "machine").glob("state-*.vm"))

    run = ["run", "--rm", "--verbose", "--podvm-tier=machine", "--podvm-net", "none",
           "--podvm-init=image"]
    code, out, err, _, took = podvm(*run, IMAGE, "sh", "-c", PAYLOAD)
    boot = field(out, "BOOT")
    verdict(1, code == 7 and SAVED in err and field(out, "PID1") == "systemd PID2=kthreadd"
            and "podvm-run.service" in (field(out, "CGROUP") or "") and len(states()) == 1,
            f"the first run exited {code} in {took:.0f} s, PID1={field(out, 'PID1')}, "
            f"{len(states())} state file(s)")
    code, out, err, first, took = podvm(*run, IMAGE, "sh", "-c", PAYLOAD)
    verdict(2, code == 7 and LOADED in err and field(out, "PID1") == "systemd PID2=kthreadd"
            and boot is not None and field(out, "BOOT") == boot
            and first is not None and first < 15,
            f"the second run exited {code}, first output after "
            f"{'-' if first is None else f'{first:.1f}'} s, PID1={field(out, 'PID1')}, "
            f"the boot id of the save: {field(out, 'BOOT') == boot}")

    runs = []
    for job, port, pids in ((1, 41001, 64), (2, 41002, 64), (3, 41003, 96)):
        sock = work / f"{job}.sock"
        with socketserver.UnixStreamServer(str(sock), Echo) as server:
            server.job, server.received = job, []
            relay = threading.Thread(target=server.serve_forever)
            relay.start()
            try:
                code, out, err, _, took = podvm(
                    *run, "--podvm-channel", f"{sock}:127.0.0.1:{port}",
                    "--pids-limit", str(pids), IMAGE,
                    "bash", "-ec", CHANNEL, "--", str(port), str(pids), str(job))
            finally:
                server.shutdown()
                relay.join()
        runs.append((job, pids, code, out, err, server.received))
    ok = True
    notes = []
    for i, (job, pids, code, out, err, received) in enumerate(runs):
        want = SAVED if i == 0 else LOADED
        good = (code == 0 and want in err and received == [f"job-{job}\n".encode()]
                and field(out, "CHANNEL") == f"answer-{job}"
                and field(out, "LIMIT") == str(pids)
                and field(out, "PID1") == "systemd PID2=kthreadd")
        notes.append(f"job {job}: exit {code}, limit {field(out, 'LIMIT')}, "
                     f"{'saved' if SAVED in err else 'loaded' if LOADED in err else 'booted'}")
        ok = ok and good
    boots = {field(r[3], "BOOT") for r in runs}
    seeds = {field(r[3], "SEED") for r in runs}
    verdict(3, ok and len(boots) == 1 and len(seeds) == 3 and len(states()) == 2,
            "; ".join(notes) + f"; {len(boots)} boot id(s), {len(seeds)} seeds, "
            f"{len(states())} state files")

    before = set(states())
    code, out, err, _, _ = podvm(*run, other, "sh", "-c", PAYLOAD)
    verdict(4, code == 7 and SAVED in err and LOADED not in err
            and len(set(states()) - before) == 1,
            f"another digest exited {code}, "
            f"{'saved its own state' if SAVED in err else 'did not save'}")

    disks = sorted((store / "machine").glob("root-*.img"))
    # The disk of the pinned image: the one that clause 1 built first.
    first_disk = min(disks, key=lambda d: d.stat().st_mtime_ns) if disks else None
    sha = hashlib.sha256(first_disk.read_bytes()).hexdigest() if first_disk else None
    if first_disk:
        os.chmod(first_disk, 0o644)
        with open(first_disk, "r+b") as f:
            f.seek(4096 * 7)
            b = f.read(1)
            f.seek(4096 * 7)
            f.write(bytes([b[0] ^ 0xff]))
        os.chmod(first_disk, 0o444)
    code, out, err, _, _ = podvm(*run, IMAGE, "sh", "-c", PAYLOAD)
    after = hashlib.sha256(first_disk.read_bytes()).hexdigest() if first_disk else None
    verdict(5, code == 7 and "was damaged" in err and LOADED in err and sha == after,
            f"after a changed byte the run exited {code}, built the disk again with the "
            f"same SHA-256: {sha == after}, and "
            f"{'started from the state' if LOADED in err else 'did not start from the state'}")

    before = set(states())
    code, out, err, _, _ = podvm(*run, IMAGE, "sh", "-c", PAYLOAD,
                                 extra={"PODVM_MACHINE_ROOT": "initramfs"})
    pid = field(out, "PID1") or ""
    verdict(6, code == 7 and SAVED in err and pid.startswith("systemd ")
            and not pid.endswith("kthreadd") and len(set(states()) - before) == 1,
            f"PODVM_MACHINE_ROOT=initramfs exited {code}, PID1={pid}")

    left = emulators(store)
    stale = sorted(p.name for p in (store / "machine").glob("run-*")) + sorted(
        p.name for p in (store / "machine").glob("*partial*")) + sorted(
        p.name for p in (store / "machine").glob("state-*.bad"))
    verdict(7, not left and not stale,
            f"{len(left)} emulator(s); run directories, partial files and failure "
            f"markers {stale}")

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
        "PODVM_486_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    if not os.access(binary, os.X_OK):
        print("COULD NOT RUN: give the podvm binary")
        return 2
    with binary.open("rb") as f:
        digest = hashlib.file_digest(f, "sha256").hexdigest()
    print("Does the saved state of a machine guest hold systemd as PID 1 on the root disk?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}, TCG, no /dev/kvm: {not os.path.exists('/dev/kvm')}")
    print(f"podvm SHA-256 {digest}\nimage {IMAGE}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-485-", dir="/var/tmp") as name:
        try:
            result = prove(binary, Path(name))
        except (subprocess.TimeoutExpired, OSError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
        finally:
            for pid in emulators(Path(name)):
                os.kill(pid, 9)
    if result == 0:
        print("THE SAVED STATE HOLDS SYSTEMD AS PID 1 ON THE ROOT DISK")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
