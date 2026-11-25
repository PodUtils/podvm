#!/usr/bin/env python3
"""Does a rootless EROFS build keep the metadata of the fixture in 477?

TODO/machine.md T-1994. This proof uses the Debian amd64 erofs-utils
1.8.6-1 package (SHA-256 81042b8c0c3eb63699aa7d0763dceedc675a747d9f5cfebf914b4160233f411e).
Set PODVM_478_TOOLS to the directory with mkfs.erofs, dump.erofs, and
the dynamically linked fsck.erofs, or put the tools on PATH. It needs a
C compiler (CC or cc) to observe the xattr writes of the offline reader.
No guest, mount or privilege is needed. Exit 0 is a match, 1 a failed
assertion, and 2 a missing prerequisite.
"""

from datetime import datetime, timezone
import hashlib
import os
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import tempfile


# fsck.erofs decodes attributes from the disk image before calling
# lsetxattr. Capture its bytes in place of making that host system call.
OBSERVER = r"""
#include <stdio.h>
#include <string.h>
#include <sys/xattr.h>

int lsetxattr(const char *path, const char *name, const void *value,
              size_t size, int flags)
{
    const unsigned char *bytes = value;
    const char *leaf = strrchr(path, '/');
    (void)flags;
    fprintf(stderr, "XATTR %s %s %zu ", leaf ? leaf + 1 : path, name, size);
    for (size_t i = 0; i < size; i++)
        fprintf(stderr, "%02x", bytes[i]);
    fputc('\n', stderr);
    return 0;
}
"""


def check(condition, message):
    if not condition:
        raise ValueError(message)


def call(*args, data=None, env=None):
    return subprocess.run(
        args, input=data, capture_output=True, timeout=40, env=env,
        stdin=subprocess.DEVNULL if data is None else None,
    )


def output(*args, data=None, env=None):
    result = call(*args, data=data, env=env)
    check(result.returncode == 0, result.stderr.decode("utf-8", errors="replace"))
    return result.stdout


def number(text, pattern):
    match = re.search(pattern, text)
    check(match is not None, f"missing {pattern}")
    return int(match[1])


def prove(mkfs, dump, fsck, cc, work):
    fixture = runpy.run_path(str(Path(__file__).with_name("477-machine-root-image.py")))
    archive = fixture["archive"](acl=True)
    image = work / "root.erofs"
    alternate = work / "default.erofs"
    stamp = ["-T0", "--mkfs-time", "-U", "00000000-0000-0000-0000-000000000000"]
    output(mkfs, "--tar=f", *stamp, image, data=archive)
    output(mkfs, "--tar", *stamp, alternate, data=archive)
    image_bytes = image.read_bytes()
    check(image_bytes == alternate.read_bytes(), "the default tar mode differs from full mode")
    print(f"stdin pipe: --tar and --tar=f produced the same {len(image_bytes)}-byte image")
    print(f"image SHA-256 {hashlib.sha256(image_bytes).hexdigest()}")

    parent = output(dump, "--path=/usr/bin", image).decode()
    original = output(dump, "--path=/usr/bin/tool", image).decode()
    link = output(dump, "--path=/usr/bin/tool-link", image).decode()
    shortcut = output(dump, "--path=/usr/bin/shortcut", image).decode()
    check("Uid: 44   Gid: 55  Access: 0555" in parent, "late parent metadata")
    check("Uid: 123456   Gid: 234567" in original, "file owners")
    check("Uid: 123456   Gid: 234567" in shortcut, "symlink owners")
    nid = number(original, r"NID: (\d+)")
    check(nid == number(link, r"NID: (\d+)") and "Links: 2" in original, "hard link identity")
    check(output(dump, "--cat", "--path=/usr/bin/tool-link", image) == fixture["DATA"], "hard link data")
    check(output(dump, "--cat", "--path=/usr/bin/shortcut", image) == b"tool", "symlink target")
    # dump.erofs masks the mode to 0777. The on-disk inode mode is a
    # little-endian word at offset 4 in both format 1.8.6 inode layouts.
    superblock = output(dump, image).decode()
    block_size = number(superblock, r"Filesystem blocksize:\s+(\d+)")
    start = number(superblock, r"Filesystem inode metadata start block:\s+(\d+)")
    mode_offset = start * block_size + nid * 32 + 4
    mode = int.from_bytes(image_bytes[mode_offset:mode_offset + 2], "little")
    check(mode == 0o104755, "raw setuid file mode")
    print("/usr/bin mode 0555, uid 44, gid 55")
    print(f"tool mode {mode & 0o7777:04o}, uid 123456, gid 234567, hard link inode {nid}")
    print("shortcut symlink target and hard link bytes match")

    observer = work / "xattr-observer.so"
    output(cc, "-std=c17", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
           "-x", "c", "-o", observer, "-", data=OBSERVER.encode())
    env = dict(os.environ, LD_PRELOAD=str(observer))
    loaded = call(fsck, "--xattrs", f"--extract={work / 'unpacked'}", image, env=env)
    check(loaded.returncode == 0, loaded.stderr.decode("utf-8", errors="replace"))
    records = set(loaded.stderr.decode().splitlines())
    for name, value in (
        ("user.marker", b"fixture"),
        ("security.capability", fixture["CAP"]),
        ("system.posix_acl_access", fixture["ACL"]),
    ):
        for leaf in ("tool", "tool-link"):
            row = f"XATTR {leaf} {name} {len(value)} {value.hex()}"
            check(row in records, f"offline reader did not return {name} for {leaf}")
        print(f"offline reader decoded {name} ({len(value)} bytes), exact on both names")

    source_tar = work / "source.tar"
    source_tar.write_bytes(archive)
    index = work / "index.erofs"
    output(mkfs, "--tar=i", *stamp, index, source_tar)
    alone = call(dump, "--cat", "--path=/usr/bin/tool", index)
    check(not alone.stdout and b"invalid device id" in alone.stderr, "index unexpectedly holds file bytes")
    check(output(dump, f"--device={source_tar}", "--cat", "--path=/usr/bin/tool", index)
          == fixture["DATA"], "index did not read the bytes from the original tar")
    print("--tar=i needed the original tar as an extra device")


def main():
    directory = os.environ.get("PODVM_478_TOOLS")
    tools = tuple(shutil.which(str(Path(directory, name)) if directory else name)
                  for name in ("mkfs.erofs", "dump.erofs", "fsck.erofs"))
    cc = shutil.which(os.environ.get("CC", "cc"))
    if os.geteuid() == 0 or cc is None or None in tools:
        print("COULD NOT RUN: use an unprivileged uid and give the EROFS tools and C compiler")
        return 2
    try:
        version = output(tools[0], "-V").decode().splitlines()[0]
    except (OSError, ValueError, IndexError, subprocess.TimeoutExpired):
        print("COULD NOT RUN: mkfs.erofs did not report its version")
        return 2
    host = os.uname()
    print(f"{datetime.now(timezone.utc):%FT%TZ} {host.sysname} {host.release}, {host.machine}, uid {os.geteuid()}")
    print(version)
    with tempfile.TemporaryDirectory(prefix="podvm-478-") as path:
        try:
            prove(*tools, cc, Path(path))
        except (OSError, ValueError, subprocess.TimeoutExpired) as fault:
            print(f"FAIL: {str(fault).replace(path, 'WORK')}")
            return 1
    print("PASS: full EROFS tar input kept the fixture's metadata")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
