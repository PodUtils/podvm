#!/usr/bin/env python3
"""Can the host user remove what a guest user wrote in a writable volume?

TODO/machine.md T-2103 (PodUtils/podrunner#371, COORD-143 of podrunner).
Run as root with Python 3.11 or later on an x86_64 Linux host with the
`incus` package (6.0.0 here) and a user of uid 1000. Each podvm runs as uid
1000 with a private store. The machine tier runs under TCG
(`PODVM_MACHINE_ACCEL=tcg`), each guest under `flock` of
`/workspace/podvm/.work/guest.lock` (or PODVM_486_GUEST_LOCK) and a
timeout. The incus part starts a PRIVATE incusd as root in a new directory
(its socket of the group of uid 1000), and at the end deletes each instance
and image, stops it, ends its dnsmasq and removes the directory.

PODVM_486_BIN selects the binary.

Clauses:
  1. Machine tier: a guest user of uid 1001 writes a file of mode 0640 and
     a directory of mode 0700 with a file in `-v H:/data:mapped`; the guest
     sees its own owner and modes.
  2. On the host each file belongs to uid 1000 with mode 0644 (a directory
     0755), and `user.virtfs.uid` holds 1001.
  3. A second guest with `-v H:/data:mapped,ro` sees the owner 1001 and the
     modes; uid 1002 cannot list the 0700 directory, uid 1001 reads it, and
     a write fails with EROFS.
  4. uid 1000 removes the directory and the file with `rm -rf`, no root.
  5. `:mapped` is refused by name with 125 on the incus tier, with
     `overlay`, and for a host file.
  6. No emulator of the runs stays.
  7. Incus tier: two `run -d` with `-v H2:/cache`, where the image user
     (uid 1001) writes a directory of mode 0700 and one of mode 0755, end
     0, and the store records the id map of each instance for H2: the
     default profile has `security.idmap.isolated=true`, so the two maps
     differ.
  8. uid 1000 cannot remove them (`rm -rf` fails with EACCES).
  9. `podvm volume rm --podvm-tier=incus H2` removes H2 (exit 0).
 10. No instance of the removal stays, and the record of H2 is gone.
 11. `volume rm` refuses by name with 125: `/`, the home directory, a link,
     a file, a directory of another user, and no `--podvm-tier=incus`.
 12. The cleanup: no instance, no image, incusd stopped, no dnsmasq of it,
     no incusbr0.

Exit 0 is a match, 1 is a failed clause or a missing row, and 2 means a
required input was absent.
"""

import datetime
import grp
import hashlib
import os
import pwd
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
IMAGE = "mirror.gcr.io/library/alpine:3.20"
BOUND = 900
CLAUSES = 12
LOCK = os.environ.get("PODVM_486_GUEST_LOCK", "/workspace/podvm/.work/guest.lock")
RUNNER = "adduser -D -u 1001 runner && adduser -D -u 1002 other && "
WRITE = ("su -s /bin/sh runner -c 'umask 022; echo f > /data/f && chmod 0640 /data/f && "
         "mkdir -m 0700 /data/d && echo x > /data/d/x && stat -c \"%u %a %n\" /data/f /data/d'")
READ = ("su -s /bin/sh runner -c 'stat -c \"%u %a %n\" /data/f /data/d; cat /data/d/x'; "
        "su -s /bin/sh other -c 'ls /data/d' 2>&1 | sed 's/^/other: /'; "
        "touch /data/new 2>&1 | sed 's/^/write: /'; true")
# The image user writes a directory of mode 0700 and one of mode 0755.
INCUS_WRITE = ("adduser -D -u 1001 runner && su -s /bin/sh runner -c 'mkdir -m 0700 /cache/d{n} && "
               "echo x > /cache/d{n}/x && mkdir -m 0755 /cache/e{n} && echo y > /cache/e{n}/y'")


def qemus(mark):
    """The pids of each emulator whose argv names `mark`."""
    out = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            cmd = (entry / "cmdline").read_bytes()
        except OSError:
            continue
        if b"qemu" in cmd.split(b"\0")[0] and mark.encode() in cmd:
            out.append(int(entry.name))
    return out


def xattr(p, name):
    try:
        return os.getxattr(p, name, follow_symlinks=False)
    except OSError:
        return None


def prove(binary, work):
    home = work / "home"
    home.mkdir()
    store = home / "store"
    shutil.copy(binary, work / "podvm")
    podvm = str(work / "podvm")
    h, h2 = work / "H", work / "H2"
    h.mkdir(mode=0o777)
    h2.mkdir(mode=0o777)
    os.chmod(h, 0o777)
    os.chmod(h2, 0o777)
    subprocess.run(["chown", "-R", "1000:1000", str(work)], check=True)
    wrap = ["setpriv", "--reuid", "1000", "--regid", "1000", "--clear-groups"]
    env = dict(os.environ, HOME=str(home), TMPDIR=str(home), PODVM_STORE=str(store),
               PODVM_MACHINE_ACCEL="tcg", PATH="/usr/local/bin:/usr/bin:/bin")
    rows = set()
    fail = 0

    def run(argv, guest=False, extra=None, as_user=True):
        e = dict(env, **(extra or {}))
        full = (wrap if as_user else []) + argv
        full = ["timeout", str(BOUND)] + full
        if guest:
            full = ["flock", LOCK] + full
        r = subprocess.run(full, env=e, stdin=subprocess.DEVNULL, capture_output=True,
                           text=True, errors="replace")
        return r.returncode, r.stdout, r.stderr.replace(str(work), "WORK")

    def verdict(n, ok, what):
        nonlocal fail
        rows.add(n)
        print(f"{'PASS' if ok else 'FAIL'} {n}: {what}", flush=True)
        fail |= not ok

    code, _, err = run([podvm, "pull", "-q", IMAGE])
    if code != 0:
        print(f"COULD NOT RUN: pull exited {code}\n{err[-1500:]}")
        return 2

    # 1 to 6: the machine tier.
    mapped = ["-v", f"{h}:/data:mapped"]
    code, out, err = run([podvm, "run", "--rm", "--podvm-tier=machine"] + mapped
                         + [IMAGE, "sh", "-c", RUNNER + WRITE], guest=True)
    print(f"  | write: exit {code}: {out.strip()!r} {err.strip()[-400:]}")
    verdict(1, code == 0 and out == "1001 640 /data/f\n1001 700 /data/d\n",
            f"the guest user wrote and sees {out.strip()!r}")
    st = {p: os.lstat(h / p) for p in ("f", "d", "d/x")}
    host = {p: (s.st_uid, oct(s.st_mode & 0o777)) for p, s in st.items()}
    uid = xattr(h / "d", "user.virtfs.uid")
    verdict(2, host == {"f": (1000, "0o644"), "d": (1000, "0o755"), "d/x": (1000, "0o644")}
            and uid == (1001).to_bytes(4, "little"),
            f"host owners and modes {host}, user.virtfs.uid of d {uid!r}")
    code, out, err = run([podvm, "run", "--rm", "--podvm-tier=machine", "-v",
                          f"{h}:/data:mapped,ro", IMAGE, "sh", "-c", RUNNER + READ], guest=True)
    print(f"  | read: exit {code}: {out.strip()!r} {err.strip()[-300:]}")
    lines = out.splitlines()
    verdict(3, code == 0 and lines[:3] == ["1001 640 /data/f", "1001 700 /data/d", "x"]
            and any(l.startswith("other:") and "Permission denied" in l for l in lines)
            and any(l.startswith("write:") and "Read-only file system" in l for l in lines),
            f"a second guest reads {lines!r}")
    code, _, err = run(["rm", "-rf", str(h / "d"), str(h / "f")])
    verdict(4, code == 0 and not any(h.iterdir()),
            f"uid 1000 rm -rf exit {code}, {len(list(h.iterdir()))} entries left {err.strip()}")
    (work / "file").write_text("x")
    cases = [
        ("incus tier", ["run", "--rm", "--podvm-tier=incus"] + mapped + [IMAGE, "true"]),
        ("overlay", ["run", "--rm", "--podvm-tier=machine", "-v", f"{h}:/data:mapped,overlay",
                     IMAGE, "true"]),
        ("a file", ["run", "--rm", "--podvm-tier=machine", "-v", f"{work}/file:/data:mapped",
                    IMAGE, "true"]),
    ]
    named = []
    for label, argv in cases:
        code, _, err = run([podvm] + argv)
        line = next((l for l in err.splitlines() if "mapped" in l), "")
        named.append(code == 125 and ("T-2103" in line or "conflict" in line))
        print(f"  | {label}: exit {code}: {(line or err.strip())[:240]}")
    verdict(5, all(named), f"{sum(named)} of {len(cases)} refusals by name at 125")
    time.sleep(0.5)
    left = qemus(str(store))
    verdict(6, not left, f"{len(left)} emulator(s) of the runs stayed")

    # 7 to 12: the incus tier, against a private incusd.
    d = work / "incus"
    d.mkdir()
    ienv = dict(os.environ, INCUS_DIR=str(d))
    log = open(d / "log", "wb")
    # The socket belongs to the group of uid 1000, so podvm reaches it.
    daemon = subprocess.Popen(["/usr/libexec/incus/incusd", "--group", group_of(1000)],
                              env=ienv, stdout=log, stderr=subprocess.STDOUT)
    sock = d / "unix.socket"
    try:
        for _ in range(120):
            if sock.exists():
                break
            time.sleep(0.5)
        r = subprocess.run(["timeout", "120", "incus", "admin", "init", "--minimal"], env=ienv,
                           capture_output=True, text=True)
        if r.returncode != 0:
            print(f"COULD NOT RUN: incus admin init: {r.stderr.strip()}")
            return 2
        prove_incus(run, verdict, podvm, h2, home, store, sock, work, ienv)
    finally:
        cleanup(d, ienv, daemon, verdict)
        log.close()
    missing = set(range(1, CLAUSES + 1)) - rows
    if missing:
        print(f"FAIL: no row for clause(s) {sorted(missing)}")
        return 1
    return fail


def group_of(uid):
    """The name of the primary group of `uid`."""
    return grp.getgrgid(pwd.getpwuid(uid).pw_gid).gr_name


def prove_incus(run, verdict, podvm, h2, home, store, sock, work, ienv):
    extra = {"PODVM_INCUS_SOCKET": str(sock)}
    subprocess.run(["incus", "profile", "set", "default", "security.idmap.isolated=true"],
                   env=ienv, check=True)
    ends = []
    for n in ("a", "b"):
        code, out, err = run([podvm, "run", "-d", "--podvm-tier=incus", "--name", f"w486{n}",
                              "-v", f"{h2}:/cache", IMAGE, "sh", "-c",
                              INCUS_WRITE.format(n=n)], extra=extra)
        print(f"  | run -d {n}: exit {code}: {out.strip()} {err.strip()[-300:]}")
        c2, waited, _ = run([podvm, "wait", "--podvm-tier=incus", f"w486{n}"], extra=extra)
        ends.append((code, c2, waited.strip()))
    records = list((store / "incus" / "volumes").glob("*.map"))
    text = records[0].read_text() if records else ""
    print(f"  | the record: {text!r}")
    verdict(7, ends == [(0, 0, "0")] * 2 and len(records) == 1
            and f"host {h2}\n" in text and text.count("\nmap ") == 2,
            f"run -d, wait and its code {ends}, {len(records)} record(s)")
    owners = sorted((p.name, p.lstat().st_uid, oct(p.lstat().st_mode & 0o777))
                    for p in h2.iterdir())
    code, _, err = run(["rm", "-rf"] + [str(h2 / p) for p in ("da", "ea", "db", "eb")])
    print(f"  | the entries: {owners}")
    verdict(8, code != 0 and err.count("Permission denied") == 4 and (h2 / "da").exists(),
            f"uid 1000 rm -rf exit {code}: {err.strip().splitlines()[:4]}")
    for n in ("a", "b"):
        run([podvm, "rm", "-f", "--podvm-tier=incus", f"w486{n}"], extra=extra)
    t0 = time.monotonic()
    code, out, err = run([podvm, "volume", "rm", "--podvm-tier=incus", str(h2)], extra=extra)
    took = time.monotonic() - t0
    print(f"  | volume rm: exit {code} in {took:.1f} s: {out.strip()} {err.strip()[-400:]}")
    verdict(9, code == 0 and not h2.exists(), f"volume rm exit {code}; H2 there: {h2.exists()}")
    r = subprocess.run(["incus", "list", "-c", "n", "-f", "csv"], env=ienv, capture_output=True,
                       text=True)
    names = r.stdout.split()
    records = list((store / "incus" / "volumes").glob("*.map"))
    verdict(10, r.returncode == 0 and not names and not records,
            f"instances {names}, records {len(records)}")
    link = work / "link"
    os.symlink(work / "H", link)
    rootdir = work / "rootdir"
    rootdir.mkdir()
    cases = [
        ("/", ["/"]),
        ("the home directory", [str(home)]),
        ("a link", [str(link)]),
        ("a file", [str(work / "file")]),
        ("a directory of root", [str(rootdir)]),
    ]
    named = []
    for label, a in cases:
        code, _, err = run([podvm, "volume", "rm", "--podvm-tier=incus"] + a, extra=extra)
        line = err.strip().splitlines()[0] if err.strip() else ""
        named.append(code == 125 and "T-2103" in line)
        print(f"  | {label}: exit {code}: {line[:220]}")
    code, _, err = run([podvm, "volume", "rm", str(work / "H")], extra=extra)
    named.append(code == 125 and "requires --podvm-tier=incus" in err)
    print(f"  | no tier: exit {code}: {err.strip().splitlines()[0][:200]}")
    verdict(11, all(named) and rootdir.exists() and (work / "H").exists(),
            f"{sum(named)} of {len(cases) + 1} refusals by name at 125; each path stays")


def cleanup(d, ienv, daemon, verdict):
    """Delete each instance and image, stop incusd and its dnsmasq."""
    def incus(*a):
        return subprocess.run(["timeout", "120", "incus", *a], env=ienv, capture_output=True,
                              text=True)
    for n in incus("list", "-c", "n", "-f", "csv").stdout.split():
        incus("delete", "-f", n)
    for f in incus("image", "list", "-c", "f", "-f", "csv").stdout.split():
        incus("image", "delete", f)
    insts = incus("list", "-c", "n", "-f", "csv").stdout.split()
    images = incus("image", "list", "-c", "f", "-f", "csv").stdout.split()
    stop = subprocess.run(["timeout", "120", "incus", "admin", "shutdown"], env=ienv,
                          capture_output=True, text=True)
    try:
        daemon.wait(timeout=60)
    except subprocess.TimeoutExpired:
        daemon.kill()
        daemon.wait()
    dns = []
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit():
            try:
                cmd = (entry / "cmdline").read_bytes()
            except OSError:
                continue
            if b"dnsmasq" in cmd and str(d).encode() in cmd:
                os.kill(int(entry.name), signal.SIGTERM)
                dns.append(int(entry.name))
    time.sleep(1)
    bridge = subprocess.run(["ip", "link", "show", "incusbr0"], capture_output=True).returncode == 0
    if bridge:
        subprocess.run(["ip", "link", "delete", "incusbr0"], capture_output=True)
    verdict(12, not insts and not images and stop.returncode == 0 and not bridge,
            f"instances {insts}, images {len(images)}, shutdown {stop.returncode}, "
            f"dnsmasq ended {len(dns)}, incusbr0 left {bridge}")


def main():
    if sys.version_info < (3, 11) or sys.platform != "linux" or os.uname().machine != "x86_64":
        print("COULD NOT RUN: use Python 3.11 or later on x86_64 Linux")
        return 2
    if os.getuid() != 0:
        print("COULD NOT RUN: run as root (podvm runs as uid 1000; incusd as root)")
        return 2
    for tool in ("setpriv", "timeout", "flock", "incus", "ip"):
        if shutil.which(tool) is None:
            print(f"COULD NOT RUN: no {tool} on PATH")
            return 2
    binary = Path(os.environ.get(
        "PODVM_486_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    if not os.access(binary, os.X_OK):
        print("COULD NOT RUN: give the podvm binary")
        return 2
    with binary.open("rb") as source:
        sha = hashlib.file_digest(source, "sha256").hexdigest()
    print("Can the host user remove what a guest user wrote in a writable volume?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}, incus {subprocess.run(['incus', '--version'], capture_output=True, text=True).stdout.strip()}")
    print(f"podvm SHA-256 {sha}\nimage {IMAGE}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-486-", dir="/var/tmp") as name:
        os.chmod(name, 0o755)
        try:
            result = prove(binary, Path(name))
        except (subprocess.SubprocessError, OSError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
        finally:
            subprocess.run(["chown", "-R", "0:0", name], check=False)
            subprocess.run(["chmod", "-R", "u+rwX", name], check=False)
    if result == 0:
        print("THE HOST USER REMOVES WHAT A GUEST USER WROTE")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
