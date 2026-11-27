#!/usr/bin/env python3
"""Does a machine guest boot the kernel of its image, with systemd of the
image as PID 1, a 9p volume and the exit code of the command?

TODO/machine.md T-1994, part 2 (PodUtils/podrunner#316 ask 2). Run with
Python 3.11 or later on an x86_64 host, as a user that is not root.
PODVM_491_BIN selects the binary, and PODVM_491_DEBS a directory that
holds the two packages (else the script fetches them). podvm fetches its
own QEMU where none is on PATH, and the runs use TCG.

The fixture: the pinned Arch Linux image of experiments 473, 483 and 487
(systemd as /sbin/init) with one more layer that holds a kernel package of
Debian 13, as `dpkg` puts it: `/boot/vmlinuz-RELEASE`,
`/boot/config-RELEASE` and `/usr/lib/modules/RELEASE` (modules as
`.ko.xz`). The script reads each `.deb` itself (by its pinned SHA-256)
and writes `modules.dep` as `depmod` does, from the `depends=` of the
`.modinfo` of each module. The image `debian` has the kernel of the
Debian flavour `amd64` (9p as modules); the image `cloud` has the flavour
`cloud-amd64`, whose config has `# CONFIG_NET_9P is not set`.

Clauses:
  1. PODVM_MACHINE_IMAGE_KERNEL=1, the default saved state, `run --rm
     --podvm-init=image --podvm-net none -v HOST:/data` of the image
     debian exits 7: `uname -r` is the release of the image, PID 1 is
     systemd and PID 2 kthreadd, `systemctl is-system-running` gives
     running or degraded, the guest reads the file of the host in the
     volume and the host reads the file of the guest, /proc/modules holds
     virtio_blk, ext4, overlay and 9p, the notice names the kernel of the
     image, and no saved state is in the store (a state does not serve the
     kernel of an image).
  2. With podvm's init, the network and `--storage-opt size=256m`, a run
     of the image debian exits 3: `uname -r` is the release of the image,
     eth0 has the address of the guest, and the root of the guest has the
     size of the disk.
  3. `run -d --podvm-init=image` of the image debian holds the lifecycle:
     `exec` gives the release, PID 1 systemd and exit 4; `stop` and `rm`
     exit 0.
  4. PODVM_MACHINE_IMAGE_KERNEL=RELEASE boots the image cloud with podvm's
     init: `uname -r` is the cloud release, and the run exits 0.
  5. The image cloud with `-v` exits 125 before a boot, with one line
     that names 9p and `CONFIG_9P_FS is not set`.
  6. Each other refusal exits 125 before a boot and names its reason: an
     image with no kernel, a release that the image does not have, the
     variable with PODVM_MACHINE_KERNEL, and PODVM_MACHINE_ROOT=initramfs.
  7. The control: with no variable, the image debian boots the pinned
     kernel (`uname -r` is not the release of the image), and exits 0.
  8. No emulator, no run directory and no partial file stay.

Exit 0 is a match, 1 is a failed clause or a missing row, and 2 means a
required input was absent.
"""

import datetime
import hashlib
import io
import json
import lzma
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
IMAGE = (
    "ghcr.io/pkgforge-dev/archlinux:latest"
    "@sha256:b2507f1964270cab3cc190aa8df858521f6a7e45e969146a012877891d5dfc9b"
)
POOL = "pool/main/l/linux-signed-amd64/"
MIRRORS = ("https://deb.debian.org/debian/",
           "https://snapshot.debian.org/archive/debian/20261127T000000Z/")
KERNELS = {
    "debian": ("6.12.117+deb13-amd64",
               "linux-image-6.12.117+deb13-amd64_6.12.117-1_amd64.deb",
               "ca712d36790184f6ac125ce2b20ea4e2202ae6bb7817b7de268f1f0115ccda73"),
    "cloud": ("6.12.117+deb13-cloud-amd64",
              "linux-image-6.12.117+deb13-cloud-amd64_6.12.117-1_amd64.deb",
              "e9893e5295d0549b8852611f12a4b8b5f6cc4b8128dcc2086b9847793b904ab7"),
}
PAYLOAD = (
    "echo KERNEL=$(uname -r); echo PID1=$(cat /proc/1/comm) PID2=$(cat /proc/2/comm); "
    "echo STATE=$(systemctl is-system-running); echo HOSTFILE=$(cat /data/in); "
    "echo from-guest > /data/out; "
    "echo MODULES=$(cut -d' ' -f1 /proc/modules | sort | tr '\\n' ,); exit 7"
)
RUN_BOUND = 900
CLAUSES = 8
VAR = "PODVM_MACHINE_IMAGE_KERNEL"


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


def fetch(name, sha):
    """The bytes of a pinned package: from PODVM_491_DEBS (a directory)
    where it holds the file, else from the first mirror that has it."""
    local = os.environ.get("PODVM_491_DEBS")
    if local and (Path(local) / name).is_file():
        body = (Path(local) / name).read_bytes()
        if hashlib.sha256(body).hexdigest() == sha:
            return body
        print(f"  | PODVM_491_DEBS: {name} is not the pinned SHA-256")
    for base in MIRRORS:
        try:
            with urllib.request.urlopen(base + POOL + name, timeout=300) as r:
                body = r.read(512 << 20)
        except OSError as e:
            print(f"  | {base}: {e}")
            continue
        if hashlib.sha256(body).hexdigest() == sha:
            return body
        print(f"  | {base}: {name} is not the pinned SHA-256")
    return None


def data_tar(deb):
    """The data member of a .deb (an ar archive), as a tar file."""
    if not deb.startswith(b"!<arch>\n"):
        raise ValueError("not an ar archive")
    at = 8
    while at + 60 <= len(deb):
        head = deb[at:at + 60]
        name = head[:16].decode().strip().rstrip("/")
        size = int(head[48:58].decode().strip())
        body = deb[at + 60:at + 60 + size]
        if name.startswith("data.tar"):
            return tarfile.open(fileobj=io.BytesIO(body))
        at += 60 + size + (size & 1)
    raise ValueError("no data.tar")


def modinfo(elf):
    """The fields of the .modinfo section of a module of ELF64."""
    if elf[:4] != b"\x7fELF" or elf[4] != 2:
        raise ValueError("not ELF64")
    (shoff,) = struct.unpack_from("<Q", elf, 0x28)
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", elf, 0x3A)
    secs = [struct.unpack_from("<IIQQQQIIQQ", elf, shoff + i * shentsize)
            for i in range(shnum)]
    names = secs[shstrndx][4]
    for s in secs:
        if elf[names + s[0]:elf.index(b"\0", names + s[0])] == b".modinfo":
            out = {}
            for kv in elf[s[4]:s[4] + s[5]].split(b"\0"):
                if b"=" in kv:
                    k, v = kv.split(b"=", 1)
                    out.setdefault(k.decode(), v.decode(errors="replace"))
            return out
    return {}


def depmod(files):
    """The modules.dep of `files` (path under the release: the bytes), as
    depmod writes it: each module, then each module that it needs."""
    paths, deps = {}, {}
    for rel, raw in sorted(files.items()):
        elf = lzma.decompress(raw) if rel.endswith(".xz") else raw
        name = rel.rsplit("/", 1)[1].split(".ko")[0].replace("-", "_")
        paths[name] = rel
        deps[name] = [d.replace("-", "_")
                      for d in modinfo(elf).get("depends", "").split(",") if d]
    lines = []
    for name in sorted(paths, key=lambda n: paths[n]):
        order = []

        def visit(n):
            for d in deps.get(n, []):
                if d not in order and d in paths:
                    visit(d)
                    order.append(d)
        visit(name)
        lines.append(paths[name] + ":" + "".join(" " + paths[d] for d in reversed(order)))
    return ("\n".join(lines) + "\n").encode()


def kernel_layer(deb, release):
    """A layer with /boot and /usr/lib/modules/RELEASE of the package, and
    the modules.dep of depmod."""
    lay = io.BytesIO()
    mods = {}
    prefix = f"usr/lib/modules/{release}/"
    with data_tar(deb) as src, tarfile.open(fileobj=lay, mode="w",
                                            format=tarfile.PAX_FORMAT) as t:
        for m in src.getmembers():
            name = m.name.lstrip("./")
            if not name or not (name.startswith("boot") or name.startswith("usr")):
                continue
            m.name = name
            data = src.extractfile(m).read() if m.isfile() else None
            if data is not None and name.startswith(prefix) and ".ko" in name:
                mods[name[len(prefix):]] = data
            t.addfile(m, io.BytesIO(data) if data is not None else None)
        dep = depmod(mods)
        i = tarfile.TarInfo(prefix + "modules.dep")
        i.size = len(dep)
        i.mode = 0o644
        t.addfile(i, io.BytesIO(dep))
    return lay.getvalue(), len(mods)


def add_layer(src, dst, layer, ref):
    """The saved image `src` with one more layer, written to `dst` as `ref`."""
    blobs = {}
    with tarfile.open(src) as t:
        for m in t.getmembers():
            if m.isfile():
                blobs[m.name] = t.extractfile(m).read()
    idx = json.loads(blobs["index.json"])
    mdesc = idx["manifests"][0]
    man = json.loads(blobs["blobs/" + mdesc["digest"].replace(":", "/")])
    cfg = json.loads(blobs["blobs/" + man["config"]["digest"].replace(":", "/")])

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
              "PODVM_INIT_BOOT_TIMEOUT", VAR):
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
    for kind, (release, deb_name, sha) in KERNELS.items():
        deb = fetch(deb_name, sha)
        if deb is None:
            print(f"COULD NOT RUN: no mirror gave {deb_name} with SHA-256 {sha}")
            return 2
        layer, count = kernel_layer(deb, release)
        print(f"  | {deb_name}: {count} modules, a layer of {len(layer) >> 20} MiB")
        refs[kind] = f"localhost/kern491-{kind}:1"
        add_layer(work / "arch.tar", work / f"{kind}.tar", layer, refs[kind])
        code, _, _, _ = podvm("load", "-i", str(work / f"{kind}.tar"), bound=600)
        (work / f"{kind}.tar").unlink()
        if code != 0:
            print(f"COULD NOT RUN: the load of the image {kind} exited {code}")
            return 2
    (work / "arch.tar").unlink()
    release, cloud = KERNELS["debian"][0], KERNELS["cloud"][0]

    rows = {}

    def verdict(n, ok, what):
        rows[n] = ok
        print(f"{'PASS' if ok else 'FAIL'} {n}: {what}", flush=True)

    def field(out, key):
        m = re.search(rf"^{key}=(.*)$", out, re.M)
        return m[1].strip() if m else None

    on = {VAR: "1"}
    vol = work / "vol"
    vol.mkdir()
    (vol / "in").write_text("from-host\n")
    os.chmod(vol, 0o777)
    init = ["run", "--rm", "--verbose", "--podvm-tier=machine", "--podvm-net", "none",
            "--podvm-init=image"]
    code, out, err, _ = podvm(*init, "-v", f"{vol}:/data", refs["debian"], "sh", "-c",
                              PAYLOAD, extra=on)
    mods = (field(out, "MODULES") or "").split(",")
    pid = field(out, "PID1") or ""
    states = sorted(p.name for p in (store / "machine").glob("state-*"))
    back = (vol / "out").read_text().strip() if (vol / "out").exists() else None
    verdict(1, code == 7 and field(out, "KERNEL") == release
            and pid == "systemd PID2=kthreadd"
            and field(out, "STATE") in ("running", "degraded")
            and field(out, "HOSTFILE") == "from-host" and back == "from-guest"
            and all(m in mods for m in ("virtio_blk", "ext4", "overlay", "9p"))
            and f"the kernel {release} of the image" in err and not states,
            f"exit {code}, uname -r {field(out, 'KERNEL')}, PID1={pid}, state "
            f"{field(out, 'STATE')}, the volume {field(out, 'HOSTFILE')}/{back}, "
            f"saved states {states}")

    plain = ["run", "--rm", "--podvm-tier=machine"]
    code, out, _, _ = podvm(*plain, "--storage-opt", "size=256m", refs["debian"], "sh", "-c",
                            "echo KERNEL=$(uname -r); echo NET=$(cat /proc/net/fib_trie "
                            "| grep -c 10.0.2.15); echo ROOT=$(df -k / | tail -1); exit 3",
                            extra=on)
    size = (field(out, "ROOT") or "").split()
    verdict(2, code == 3 and field(out, "KERNEL") == release
            and (field(out, "NET") or "0") != "0"
            and len(size) > 1 and 200_000 < int(size[1]) < 262_144,
            f"exit {code}, uname -r {field(out, 'KERNEL')}, 10.0.2.15 in "
            f"{field(out, 'NET')} lines of fib_trie, the root {field(out, 'ROOT')}")

    detach = ["--podvm-tier=machine", "--podvm-net", "none", "--podvm-init=image"]
    code, out, _, _ = podvm("run", "-d", "--name", "k491", *detach, refs["debian"],
                            "sleep", "600", extra=on)
    ran = code == 0
    got = None
    t0 = time.monotonic()
    while ran and time.monotonic() - t0 < 300:
        code, out, _, _ = podvm("exec", "k491", "sh", "-c",
                                "echo KERNEL=$(uname -r) PID1=$(cat /proc/1/comm); exit 4",
                                bound=120)
        if code == 4:
            got = out
            break
        time.sleep(10)
    stop, _, _, _ = podvm("stop", "k491", bound=120)
    rm, _, _, _ = podvm("rm", "k491", bound=120)
    m = re.search(r"KERNEL=(\S+) PID1=(\S+)", got or "")
    verdict(3, ran and m is not None and m[1] == release and m[2] == "systemd"
            and stop == 0 and rm == 0,
            f"run -d {'held' if ran else 'failed'}, exec gave {m[0] if m else 'no line'}, "
            f"stop exited {stop}, rm {rm}")

    code, out, _, _ = podvm(*plain, refs["cloud"], "sh", "-c", "echo KERNEL=$(uname -r)",
                            extra={VAR: cloud})
    verdict(4, code == 0 and field(out, "KERNEL") == cloud,
            f"exit {code}, uname -r {field(out, 'KERNEL')}")

    code, _, err, took = podvm(*plain, "-v", f"{vol}:/data", refs["cloud"], "true",
                               bound=120, extra=on)
    lines = [l for l in err.splitlines() if "9p (CONFIG_9P_FS is not set)" in l]
    verdict(5, code == 125 and len(lines) == 1 and took < 60 and not emulators(store),
            f"exit {code} in {took:.1f} s, {len(lines)} line(s) that name 9p")

    cases = [
        ("no kernel", IMAGE, on, "the image has no kernel"),
        ("no such release", refs["debian"], {VAR: "9.9.9"},
         f"no kernel of the release 9.9.9 (its kernels: {release})"),
        ("two variables", refs["debian"], {**on, "PODVM_MACHINE_KERNEL": "/nonexistent"},
         "set one"),
        ("the initramfs", refs["debian"], {**on, "PODVM_MACHINE_ROOT": "initramfs"},
         "PODVM_MACHINE_ROOT=initramfs"),
    ]
    bad = []
    for what, ref, extra, words in cases:
        code, _, err, took = podvm(*plain, ref, "true", bound=120, extra=extra)
        if code != 125 or words not in err or took > 60:
            bad.append(what)
    verdict(6, not bad, f"each refusal exited 125 and named its reason; failed: {bad}")

    code, out, _, _ = podvm(*plain, refs["debian"], "sh", "-c", "echo KERNEL=$(uname -r)")
    verdict(7, code == 0 and field(out, "KERNEL") not in (None, release),
            f"exit {code}, uname -r {field(out, 'KERNEL')} with no variable")

    left = emulators(store)
    stale = sorted(p.name for p in (store / "machine").glob("run-*")) + sorted(
        p.name for p in (store / "machine").glob("*partial*"))
    verdict(8, not left and not stale,
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
        "PODVM_491_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    if not os.access(binary, os.X_OK):
        print("COULD NOT RUN: give the podvm binary")
        return 2
    with binary.open("rb") as f:
        digest = hashlib.file_digest(f, "sha256").hexdigest()
    print("Does a machine guest boot the kernel of its image, with systemd of the image "
          "as PID 1, a 9p volume and the exit code of the command?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}, TCG, no /dev/kvm: {not os.path.exists('/dev/kvm')}")
    print(f"podvm SHA-256 {digest}\nimage {IMAGE}")
    for kind, (release, name, sha) in KERNELS.items():
        print(f"kernel {kind}: {release}, {name} SHA-256 {sha}")
    sys.stdout.flush()
    with tempfile.TemporaryDirectory(prefix="podvm-491-", dir="/var/tmp") as name:
        try:
            result = prove(binary, Path(name))
        except (subprocess.TimeoutExpired, OSError, ValueError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
        finally:
            for pid in emulators(Path(name)):
                os.kill(pid, 9)
    if result == 0:
        print("THE GUEST BOOTS THE KERNEL OF ITS IMAGE")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
