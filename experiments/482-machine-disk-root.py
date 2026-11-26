#!/usr/bin/env python3
"""Is the root of a machine guest the disk of its image, made once?

TODO/machine.md T-1994 (PodUtils/podrunner#313 ask 2, #316 ask 2) and
T-1999. Run with Python 3.11 or later on an x86_64 host, as a user that is
not root. PODVM_482_BIN selects the binary. podvm fetches its own QEMU
where none is on PATH, and the runs use TCG.

The script saves the pinned alpine image, adds a layer with
/opt/cap/cat (busybox with `cap_net_raw+ep` in a PAX record), /opt/plain/cat
(the same bytes with no capability) and /opt/big.bin (220 MiB of random
bytes), and loads it. Each run of the image is
`run --rm --verbose --podvm-tier=machine --podvm-net none` with no
`--podvm-mem`. The payload prints the mount of the root, the CapEff of
each `cat` as the user nobody, MemTotal and the SHA-256 of /opt/big.bin,
writes /etc/podvm-482, and exits 7.

Clauses:
  1. The first run exits 7, the code of the payload.
  2. The first run builds the root disk: its notice says "root disk built
     now", and one root-*.img is in the store.
  3. As nobody, /opt/cap/cat has cap_net_raw in CapEff (0000000000002000),
     and /opt/plain/cat has none: the disk keeps security.capability.
  4. The root is an overlay with the disk as its lower layer and a tmpfs
     as its upper layer, and the write to /etc succeeds.
  5. The second run exits 7, says "root disk from the cache", and the disk
     file has the same inode, size, modification time and SHA-256 as after
     the first run: no build, and no write of a guest reached it.
  6. The guest memory does not grow with the image: with a 220 MiB file in
     the root, MemTotal is below 600 MiB, and the guest reads the file
     with the SHA-256 of the host. The control with
     PODVM_MACHINE_ROOT=initramfs has MemTotal above 1000 MiB (podvm sizes
     the guest for its initrd, T-1922) and the same SHA-256.
  7. A changed byte of the disk is found: the next run names the damaged
     disk, builds it again and exits 7.
  8. PODVM_MACHINE_ROOT=ram exits 125 and names the values, with no boot.
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
    "ghcr.io/linuxcontainers/alpine:latest"
    "@sha256:28a33e0390da2b341d05e53803c6766977039baedfc8a82a4910d5acc4755b66"
)
REF = "localhost/cap482:1"
BIG = 220 << 20
PAYLOAD = (
    "grep ' / ' /proc/mounts; "
    "echo CAP=$(su -s /bin/sh nobody -c '/opt/cap/cat /proc/self/status' | grep CapEff); "
    "echo PLAIN=$(su -s /bin/sh nobody -c '/opt/plain/cat /proc/self/status' | grep CapEff); "
    "echo MEM=$(grep MemTotal /proc/meminfo); "
    "echo BIG=$(sha256sum /opt/big.bin); "
    "echo written > /etc/podvm-482 && echo WRITE-OK; exit 7"
)
# The bound of one guest run under TCG.
RUN_BOUND = 900
CLAUSES = 9
# vfs_cap_data v2: the effective flag, then cap_net_raw (13) permitted.
CAP = (0x02000001).to_bytes(4, "little") + (1 << 13).to_bytes(4, "little") + bytes(12)


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


def add_layer(src, dst, big):
    """The saved image `src` with one more layer, written to `dst`."""
    blobs = {}
    with tarfile.open(src) as t:
        for m in t.getmembers():
            if m.isfile():
                blobs[m.name] = t.extractfile(m).read()
    idx = json.loads(blobs["index.json"])
    mdesc = idx["manifests"][0]
    man = json.loads(blobs["blobs/" + mdesc["digest"].replace(":", "/")])
    cfg = json.loads(blobs["blobs/" + man["config"]["digest"].replace(":", "/")])
    busybox = None
    for layer in man["layers"]:
        with tarfile.open(fileobj=io.BytesIO(blobs["blobs/" + layer["digest"].replace(":", "/")])) as t:
            for m in t.getmembers():
                if m.name.lstrip("./") == "bin/busybox" and m.isfile():
                    busybox = t.extractfile(m).read()
    if busybox is None:
        raise OSError("the image has no /bin/busybox")
    lay = io.BytesIO()
    with tarfile.open(fileobj=lay, mode="w", format=tarfile.PAX_FORMAT) as t:
        for d in ("opt", "opt/cap", "opt/plain"):
            i = tarfile.TarInfo(d)
            i.type = tarfile.DIRTYPE
            i.mode = 0o755
            t.addfile(i)
        for name, xattrs in (("opt/cap/cat", True), ("opt/plain/cat", False)):
            i = tarfile.TarInfo(name)
            i.size = len(busybox)
            i.mode = 0o755
            if xattrs:
                i.pax_headers = {
                    "SCHILY.xattr.security.capability": CAP.decode("utf-8", "surrogateescape")
                }
            t.addfile(i, io.BytesIO(busybox))
        i = tarfile.TarInfo("opt/big.bin")
        i.size = len(big)
        i.mode = 0o644
        t.addfile(i, io.BytesIO(big))
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
    mdesc.setdefault("annotations", {})["org.opencontainers.image.ref.name"] = REF
    blobs["index.json"] = json.dumps(idx).encode()
    with tarfile.open(dst, "w") as t:
        for name, b in blobs.items():
            i = tarfile.TarInfo(name)
            i.size = len(b)
            t.addfile(i, io.BytesIO(b))


def sha256(path):
    with open(path, "rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


def prove(binary, work):
    store = work / "store"
    env = dict(os.environ, PODVM_STORE=str(store), PODVM_MACHINE_ACCEL="tcg",
               PODVM_RUNSC="/nonexistent/runsc", HOME=str(work / "home"))
    for k in ("PODVM_MACHINE_KERNEL", "PODVM_MACHINE_ROOT"):
        env.pop(k, None)
    (work / "home").mkdir()

    def podvm(*args, bound=RUN_BOUND, extra=None):
        """Run podvm; the seconds to its first line of stdout, its exit
        code and its output."""
        start = time.monotonic()
        p = subprocess.Popen([binary, *args], env=dict(env, **(extra or {})),
                             stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE, text=True, errors="replace")
        first = None
        out = []
        try:
            for line in p.stdout:
                if first is None:
                    first = time.monotonic() - start
                out.append(line)
                if time.monotonic() - start > bound:
                    p.kill()
                    break
            err = p.stderr.read()
            code = p.wait(timeout=max(1, bound - (time.monotonic() - start)))
        except subprocess.TimeoutExpired:
            p.kill()
            p.wait()
            code, err = -9, ""
        text = ("".join(out) + err).replace(str(work), "WORK")
        took = time.monotonic() - start
        shown = " ".join(args).replace(PAYLOAD, "PAYLOAD").replace(str(work), "WORK")
        prefix = "".join(f"{k}={v} " for k, v in (extra or {}).items())
        print(f"$ {prefix}podvm {shown}\n"
              f"  | exit {code} in {took:.1f} s, first output after "
              f"{'-' if first is None else f'{first:.1f} s'}")
        for line in text.splitlines():
            print(f"  | {line}")
        return code, text, first

    code, _, _ = podvm("pull", "-q", IMAGE, bound=480)
    if code != 0:
        print(f"COULD NOT RUN: the pinned image pull exited {code}")
        return 2
    big = os.urandom(BIG)
    big_sha = hashlib.sha256(big).hexdigest()
    code, _, _ = podvm("save", "-o", str(work / "alp.tar"), IMAGE, bound=300)
    if code != 0:
        print(f"COULD NOT RUN: the save of the image exited {code}")
        return 2
    add_layer(work / "alp.tar", work / "cap.tar", big)
    del big
    (work / "alp.tar").unlink()
    code, _, _ = podvm("load", "-i", str(work / "cap.tar"), bound=600)
    (work / "cap.tar").unlink()
    if code != 0:
        print(f"COULD NOT RUN: the load of the image exited {code}")
        return 2
    print(f"big.bin SHA-256 {big_sha}")

    rows = {}

    def verdict(n, ok, what):
        rows[n] = ok
        print(f"{'PASS' if ok else 'FAIL'} {n}: {what}", flush=True)

    def disks():
        return sorted((store / "machine").glob("root-*.img"))

    def ident(p):
        s = p.stat()
        return (s.st_ino, s.st_size, s.st_mtime_ns, sha256(p))

    machine = ["run", "--rm", "--verbose", "--podvm-tier=machine", "--podvm-net", "none"]
    code, out1, first1 = podvm(*machine, REF, "sh", "-c", PAYLOAD)
    verdict(1, code == 7, f"the first run exited {code}")
    found = disks()
    verdict(2, "root disk built now" in out1 and len(found) == 1,
            f"the notice of the first run and {len(found)} root disk(s) in the store")
    cap = re.search(r"^CAP=CapEff:\s*(\S+)", out1, re.M)
    plain = re.search(r"^PLAIN=CapEff:\s*(\S+)", out1, re.M)
    verdict(3, bool(cap and plain and cap[1] == "0000000000002000"
                    and plain[1] == "0000000000000000"),
            f"CapEff of /opt/cap/cat {cap[1] if cap else '-'}, "
            f"of /opt/plain/cat {plain[1] if plain else '-'}")
    verdict(4, "lowerdir=/:/podvm-lower" in out1 and "upperdir=/podvm-upper/upper" in out1
            and "WRITE-OK" in out1, "the root is an overlay of the disk with a tmpfs upper layer")
    before = ident(found[0]) if found else None
    if before:
        print(f"root disk {found[0].name}: {before[1]} bytes, SHA-256 {before[3]}")

    code, out2, first2 = podvm(*machine, REF, "sh", "-c", PAYLOAD)
    after = ident(found[0]) if found else None
    verdict(5, code == 7 and "root disk from the cache" in out2 and before and before == after,
            f"the second run exited {code}; the disk is the same file with the same bytes: "
            f"{before == after}; first output after {first1 and round(first1, 1)} s, "
            f"then {first2 and round(first2, 1)} s")
    mem = re.search(r"^MEM=MemTotal:\s*(\d+) kB", out2, re.M)
    sums = re.findall(r"^BIG=([0-9a-f]{64})", out2, re.M)
    code, out3, _ = podvm(*machine, REF, "sh", "-c", PAYLOAD,
                          extra={"PODVM_MACHINE_ROOT": "initramfs"})
    cmem = re.search(r"^MEM=MemTotal:\s*(\d+) kB", out3, re.M)
    csums = re.findall(r"^BIG=([0-9a-f]{64})", out3, re.M)
    mib = int(mem[1]) // 1024 if mem else None
    cmib = int(cmem[1]) // 1024 if cmem else None
    verdict(6, bool(mib and mib < 600 and sums == [big_sha] and cmib and cmib > 1000
                    and csums == [big_sha] and code == 7),
            f"MemTotal {mib} MiB on the root disk, {cmib} MiB with the initramfs "
            f"(control, exit {code}); the guest SHA-256 of big.bin is the host's: "
            f"{sums == [big_sha]} and {csums == [big_sha]}")

    if found:
        os.chmod(found[0], 0o644)
        with open(found[0], "r+b") as f:
            f.seek(4096 * 5)
            byte = f.read(1)
            f.seek(4096 * 5)
            f.write(bytes([byte[0] ^ 0xff]))
        os.chmod(found[0], 0o444)
    code, out4, _ = podvm(*machine, REF, "sh", "-c", PAYLOAD)
    rebuilt = disks()
    verdict(7, code == 7 and "was damaged" in out4 and "root disk built now" in out4
            and len(rebuilt) == 1 and before and sha256(rebuilt[0]) == before[3],
            f"the run after a changed byte exited {code} and built the disk again "
            f"with the bytes of the first build")

    code, out5, _ = podvm(*machine, REF, "true", bound=120, extra={"PODVM_MACHINE_ROOT": "ram"})
    verdict(8, code == 125 and "the values are disk and initramfs" in out5,
            f"PODVM_MACHINE_ROOT=ram exited {code}")

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
        "PODVM_482_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    if not os.access(binary, os.X_OK):
        print("COULD NOT RUN: give the podvm binary")
        return 2
    print("Is the root of a machine guest the disk of its image, made once?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}, TCG, no /dev/kvm: {not os.path.exists('/dev/kvm')}")
    print(f"podvm SHA-256 {sha256(binary)}\nimage {IMAGE} with a layer of {BIG >> 20} MiB",
          flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-482-", dir="/var/tmp") as name:
        try:
            result = prove(binary, Path(name))
        except (subprocess.TimeoutExpired, OSError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
        finally:
            for pid in emulators(Path(name)):
                os.kill(pid, 9)
            for p in Path(name).rglob("*"):
                if p.is_dir() and not p.is_symlink():
                    try:
                        os.chmod(p, 0o700)
                    except OSError:
                        pass
    if result == 0:
        print("THE ROOT OF A MACHINE GUEST IS THE DISK OF ITS IMAGE, MADE ONCE")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
