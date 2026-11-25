#!/usr/bin/env python3
"""Can a user build a SquashFS root from a flattened OCI tar stream?

TODO/machine.md T-1994, the disk image of an OCI tree. The fixture has
the ordering of pod-extract's flatten stream. It is a synthetic tar,
so it needs no registry, root, mount, emulator or Cargo build.

Set PODVM_477_SQFSTAR and PODVM_477_UNSQUASHFS to the tools to measure.
The first run uses the Debian amd64 squashfs-tools 4.6.1-1 package:
SHA-256 0e185122f5858132bbc709e26228ba28cf7f0c41f6ed26e3f55f518f2a53b59d.
Exit 0 means the supported metadata passed and the ACL result is known.
Exit 1 means an assertion failed. Exit 2 means an input was absent.
"""

from datetime import datetime, timezone
from io import BytesIO
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tarfile
import tempfile


STAMP = 1_700_000_000
DATA = b"tool data\n"
CAP = struct.pack("<IIIII", 0x02000001, 1 << 10, 0, 0, 0)
ACL = struct.pack("<I", 2) + b"".join(
    struct.pack("<HHI", tag, mode, uid)
    for tag, mode, uid in (
        (1, 7, 0xFFFFFFFF), (2, 5, 123456), (4, 5, 0xFFFFFFFF),
        (16, 5, 0xFFFFFFFF), (32, 0, 0xFFFFFFFF),
    )
)


def archive(acl=False):
    stream = BytesIO()
    with tarfile.open(fileobj=stream, mode="w", format=tarfile.PAX_FORMAT) as tar:
        tool = tarfile.TarInfo("usr/bin/tool")
        tool.mtime, tool.uid, tool.gid, tool.mode = STAMP, 123456, 234567, 0o4755
        tool.size = len(DATA)
        tool.pax_headers["SCHILY.xattr.user.marker"] = "fixture"
        tool.pax_headers["SCHILY.xattr.security.capability"] = CAP.decode("ascii")
        if acl:
            tool.pax_headers["SCHILY.xattr.system.posix_acl_access"] = ACL.decode(
                "utf-8", "surrogateescape"
            )
        tar.addfile(tool, BytesIO(DATA))

        for name, kind, target in (
            ("usr/bin/tool-link", tarfile.LNKTYPE, "usr/bin/tool"),
            ("usr/bin/shortcut", tarfile.SYMTYPE, "tool"),
        ):
            link = tarfile.TarInfo(name)
            link.type, link.linkname = kind, target
            link.mtime, link.uid, link.gid = STAMP, tool.uid, tool.gid
            link.mode = 0o777 if kind == tarfile.SYMTYPE else tool.mode
            tar.addfile(link)

        # Also test a directory that the reader must make for a child.
        implicit = tarfile.TarInfo("implicit/file")
        tar.addfile(implicit, BytesIO())
        for name, mode in (("usr", 0o755), ("usr/bin", 0o555)):
            parent = tarfile.TarInfo(name)
            parent.type, parent.mode = tarfile.DIRTYPE, mode
            parent.mtime, parent.uid, parent.gid = STAMP, 44, 55
            tar.addfile(parent)
    return stream.getvalue()


def check(condition, message):
    if not condition:
        raise ValueError(message)


def call(*args, data=None):
    return subprocess.run(
        args, input=data, capture_output=True, timeout=40,
        stdin=subprocess.DEVNULL if data is None else None,
    )


def build(sqfstar, image, data):
    return call(
        sqfstar, "-quiet", "-no-progress", "-exit-on-error", "-processors", "2",
        "-default-uid", "0", "-default-gid", "0", "-default-mode", "755",
        "-mkfs-time", "0", "-root-time", "0", image, data=data,
    )


def records(unsquashfs, image):
    out = call(unsquashfs, "-no-progress", "-pf", "-", image)
    check(out.returncode == 0, "unsquashfs could not read the image")
    header, sep, _ = out.stdout.partition(b"# START OF DATA")
    check(bool(sep), "unsquashfs returned no pseudo-file header")
    return header.decode("ascii").splitlines()


def metadata(sqfstar, unsquashfs, work):
    image = work / "root.sqfs"
    result = build(sqfstar, image, archive())
    check(result.returncode == 0, f"sqfstar exited {result.returncode}")
    check(not result.stderr, f"sqfstar warned: {result.stderr!r}")
    entries = records(unsquashfs, image)
    capability = "".join(f"\\{byte:03o}" for byte in CAP)
    for line in (
        "/ D 0 755 0 0",
        f"usr D {STAMP} 755 44 55",
        f"usr/bin D {STAMP} 555 44 55",
        f"usr/bin/tool R {STAMP} 4755 123456 234567 {len(DATA)} 0 0",
        "usr/bin/tool-link L usr/bin/tool",
        f"usr/bin/shortcut S {STAMP} 777 123456 234567 tool",
        "usr/bin/tool x user.marker=fixture",
        f"usr/bin/tool x security.capability=0t{capability}",
    ):
        check(line in entries, f"metadata missing: {line}")
        print(line)
    implicit = next((line.split() for line in entries if line.startswith("implicit ")), [])
    check(len(implicit) == 6 and implicit[3:] == ["755", "0", "0"], "wrong implicit directory")
    result = call(unsquashfs, "-cat", image, "usr/bin/tool-link")
    check(result.returncode == 0 and result.stdout == DATA, "the hard link returned different data")
    print(f"implicit directory: mode {implicit[3]}, uid {implicit[4]}, gid {implicit[5]}")
    print(f"PASS: late parents, numeric owners, modes, xattrs and links; image {image.stat().st_size} bytes")


def acl_control(sqfstar, unsquashfs, work):
    image = work / "acl.sqfs"
    result = build(sqfstar, image, archive(acl=True))
    attr = "system.posix_acl_access"
    if result.returncode == 0:
        entries = records(unsquashfs, image)
        encoded = "".join(f"\\{byte:03o}" for byte in ACL)
        exact = f"usr/bin/tool x {attr}=0t{encoded}"
        if exact in entries:
            print("ACL: the image kept all bytes")
            return
        check(not any(f" x {attr}=" in line for line in entries), "the ACL bytes changed")
    warnings = result.stderr.decode("utf-8", errors="replace").splitlines()
    line = next((line for line in warnings if attr in line), "")
    check(bool(line), "the ACL was omitted or refused with no diagnostic")
    print(f"ACL: sqfstar exited {result.returncode}, {line}")


def main():
    sqfstar = shutil.which(os.environ.get("PODVM_477_SQFSTAR", "sqfstar"))
    unsquashfs = shutil.which(os.environ.get("PODVM_477_UNSQUASHFS", "unsquashfs"))
    if os.geteuid() == 0 or sqfstar is None or unsquashfs is None:
        print("COULD NOT RUN: use an unprivileged uid and give sqfstar and unsquashfs")
        return 2
    try:
        versions = []
        for program, name in ((sqfstar, "sqfstar"), (unsquashfs, "unsquashfs")):
            result = call(program, "-version")
            version = result.stdout.decode().splitlines()[0]
            check(result.returncode in (0, 1) and version.startswith(f"{name} version "), "version failed")
            versions.append(version)
    except (OSError, ValueError, IndexError, subprocess.TimeoutExpired):
        print("COULD NOT RUN: a tool did not report its version")
        return 2
    host = os.uname()
    print(f"{datetime.now(timezone.utc):%FT%TZ} {host.sysname} {host.release}, {host.machine}, uid {os.geteuid()}")
    print("\n".join(versions))
    with tempfile.TemporaryDirectory(prefix="podvm-477-") as path:
        try:
            metadata(sqfstar, unsquashfs, Path(path))
            acl_control(sqfstar, unsquashfs, Path(path))
        except (OSError, ValueError, subprocess.TimeoutExpired) as fault:
            print(f"FAIL: {str(fault).replace(path, 'WORK')}")
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
