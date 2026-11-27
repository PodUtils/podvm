#!/usr/bin/env python3
"""Can the payload of the namespace rung change a mount of its run?

TODO/enter.md T-2105 (PodUtils/podrunner#378, COORD-147 of podrunner).
Run with Python 3.11 or later on an x86_64 Linux host that grants user
namespaces and has `cc` with a static libc, `setpriv` and `unshare`, as
root (each caller then runs as root, as uid 1000, and as uid 0 of a user
namespace of one id that uid 1000 opens) or as a user with no root (the
rows of root are skipped). No guest starts.

PODVM_489_BIN selects the binary. PODVM_489_BEFORE names a binary before
the fix (the release binary of `main`): it must show the fault.

A static probe goes into `/tmp` of the run through stdin, and tries:
remount a `:ro` volume writable (`MS_REMOUNT | MS_BIND`) and write to it;
clear `nosuid`, `nodev` and `noexec` of `/proc`; give `--tmpfs /w:size=8m`
the size 64m; in a mount namespace of its own, remount the volume writable
and unmount it; unmount the volume and the tmpfs. A row is locked where
each of these calls fails and each file of the volume source is the same
byte for byte. The probe also tries the way out of a `chroot(2)` and a
write to a host file that is in no volume: an information line.

Clauses (a row marked open names a path that waits for pod-enter, see
T-2105; it prints OPEN while the fault shows, PASS when the path locks,
and fails the run only where its row is missing):
  1. uid 1000, a foreground run (podvm's own user namespace): locked.
  2. uid 1000, `exec` into that foreground run: locked.
  3. uid 1000, `run -d` with `-v H2:/c:overlay`, and `exec`: locked.
  4. uid 0 of a namespace of one id, a foreground run: locked.
  5. uid 0 of a namespace of one id, `run -d` and `exec`: locked.
  6. uid 0 of a namespace of one id, `create`, `start` and `exec`: locked.
  7. uid 1000, `run -d` and `exec` (pod-enter's user namespace): open.
  8. uid 1000, `create`, `start` and `exec`: open.
  9. root, a foreground run (`--tmpfs` only: a volume of root takes the
     loader family, a path rewrite with no mount): open.
 10. root, `run -d` and `exec`: open.
 11. root, `create`, `start` and `exec`: open.
 12. uid 1000, `--read-only`: the root is a path rewrite of the loader
     family, not a mount, and a remount of `/` fails.
 13. The way out of the chroot reaches no host file: open on each entry.
 14. PODVM_489_BEFORE, row 1: the fault shows (the remount works).
 15. No process of podvm stays, and no tree of a run stays in a store.

Exit 0 is a match, 1 is a failed clause or a missing row, and 2 means a
required input was absent.
"""

import datetime
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
IMAGE = "mirror.gcr.io/library/alpine:3.20"
BOUND = 120
CLAUSES = 15
# The mount calls of a row: each one must fail.
CALLS = ("remount-ro-volume", "remount-proc-flags", "remount-tmpfs-size",
         "ns-remount-ro-volume", "ns-unmount-volume", "unmount-volume", "unmount-tmpfs")
PROBE = r"""
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <sched.h>
#include <stdio.h>
#include <string.h>
#include <sys/mount.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <sys/wait.h>
#include <unistd.h>
static unsigned long keep(const char *p) {
  struct statvfs s;
  if (statvfs(p, &s) != 0) return 0;
  return s.f_flag & (MS_RDONLY | MS_NOSUID | MS_NODEV | MS_NOEXEC | MS_NOATIME | MS_NODIRATIME | MS_RELATIME);
}
static void say(const char *what, int r) {
  printf("%s: %s\n", what, r == 0 ? "OK" : strerror(errno));
  fflush(stdout);
}
static int rw(const char *p) { return mount(NULL, p, NULL, MS_REMOUNT | MS_BIND | (keep(p) & ~MS_RDONLY), NULL); }
static void apart(const char *what, int unmount) {
  pid_t c = fork();
  if (c == 0) {
    if (unshare(CLONE_NEWNS) != 0) { say(what, -1); _exit(0); }
    say(what, unmount ? umount2("/x", MNT_DETACH) : rw("/x"));
    _exit(0);
  }
  waitpid(c, NULL, 0);
}
int main(int argc, char **argv) {
  if (argc < 2) return 2;
  FILE *m = fopen("/proc/self/mountinfo", "r");
  char line[4096]; int x = 0;
  while (m && fgets(line, sizeof line, m)) if (strstr(line, " /x ")) x = 1;
  if (m) fclose(m);
  printf("volume-is-a-mount: %s\n", x ? "yes" : "no");
  say("remount-ro-volume", rw("/x"));
  int fd = open("/x/f", O_WRONLY | O_TRUNC);
  say("write-ro-volume", fd < 0 ? -1 : (write(fd, "changed\n", 8) == 8 ? 0 : -1));
  if (fd >= 0) close(fd);
  say("remount-proc-flags", mount(NULL, "/proc", NULL, MS_REMOUNT | MS_BIND | (keep("/proc") & ~(MS_NOSUID | MS_NODEV | MS_NOEXEC)), NULL));
  say("remount-tmpfs-size", mount(NULL, "/w", NULL, MS_REMOUNT | keep("/w"), "size=64m"));
  apart("ns-remount-ro-volume", 0);
  apart("ns-unmount-volume", 1);
  say("remount-root", mount(NULL, "/", NULL, MS_REMOUNT | MS_BIND | (keep("/") & ~MS_RDONLY), NULL));
  say("unmount-volume", umount2("/x", MNT_DETACH));
  say("unmount-tmpfs", umount2("/w", MNT_DETACH));
  pid_t c = fork();
  if (c == 0) {
    mkdir("/tmp/out", 0700);
    if (chroot("/tmp/out") != 0) { say("way-out-write", -1); _exit(0); }
    for (int i = 0; i < 64; i++) if (chdir("..") != 0) break;
    if (chroot(".") != 0) { say("way-out-write", -1); _exit(0); }
    int o = open(argv[1], O_WRONLY | O_TRUNC);
    say("way-out-write", o < 0 ? -1 : (write(o, "out\n", 4) == 4 ? 0 : -1));
    _exit(0);
  }
  waitpid(c, NULL, 0);
  return 0;
}
"""


def digest(d):
    """Each file under `d`: its SHA-256."""
    return {str(p.relative_to(d)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(d.rglob("*")) if p.is_file()}


def podvm_procs(binaries):
    """The pids of each live process (not a zombie) of one of `binaries`:
    the init, the relay and the reaper of a run hide their command line,
    and other sessions of the host run podvm too."""
    found = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit() or int(entry.name) == os.getpid():
            continue
        try:
            exe = os.readlink(entry / "exe")
            stat = (entry / "stat").read_text()
        except OSError:
            continue
        if not stat[stat.rfind(")") + 2:].startswith("Z") and exe in binaries:
            found.append(int(entry.name))
    return found


def parse(out):
    """The answers of the probe: name to the words after the colon."""
    return dict(l.split(": ", 1) for l in out.splitlines() if ": " in l)


def prove(binary, before, work):
    probe = work / "probe"
    (work / "probe.c").write_text(PROBE)
    r = subprocess.run(["cc", "-static", "-O1", "-o", str(probe), str(work / "probe.c")],
                       capture_output=True, text=True)
    if r.returncode != 0:
        print(f"COULD NOT RUN: cc -static exited {r.returncode}: {r.stderr[-300:]}")
        return 2
    h, h2, out = work / "H", work / "H2", work / "E"
    for d in (h, h2, out):
        d.mkdir()
    (h / "f").write_text("original\n")
    (h2 / "g").write_text("lower\n")
    root = os.getuid() == 0
    callers = {"uid1000": [], "ns": ["unshare", "--user", "--map-root-user"], "root": []}
    if root:
        drop = ["setpriv", "--reuid", "1000", "--regid", "1000", "--clear-groups"]
        callers["uid1000"] = drop
        callers["ns"] = drop + callers["ns"]
    homes = {}
    fail, rows, opens = 0, set(), []
    probe_bytes = probe.read_bytes()

    def env_of(caller):
        home = homes[caller]
        return dict(os.environ, HOME=str(home), TMPDIR=str(home), PODVM_STORE=str(home / "store"),
                    PODVM_SPARE_TREE="0", PATH="/usr/local/bin:/usr/bin:/bin")

    def run(caller, argv, binary=binary, stdin=None):
        full = ["timeout", str(BOUND)] + callers[caller] + [str(binary)] + argv
        try:
            r = subprocess.run(full, env=env_of(caller), input=stdin, capture_output=True)
        except OSError as error:
            return 127, "", str(error)
        text = lambda b: b.decode("utf-8", "replace").replace(str(work), "WORK")
        return r.returncode, text(r.stdout), text(r.stderr)

    def verdict(n, ok, what, open_row=False):
        nonlocal fail
        rows.add(n)
        word = "PASS" if ok else ("OPEN" if open_row else "FAIL")
        if open_row and not ok:
            opens.append(n)
        print(f"{word} {n}: {what}", flush=True)
        fail |= not ok and not open_row

    def target(caller):
        # The way out writes a host file in no volume, of the caller's owner.
        f = out / f"{caller}"
        f.write_text("host\n")
        if caller != "root" and root:
            os.chown(f, 1000, 1000)
        return f

    def judge(n, label, code, text, err, base, caller, open_row=False):
        before = base
        a = parse(text)
        missing = [c for c in CALLS if c not in a]
        worked = [c for c in CALLS if a.get(c) == "OK"]
        same = digest(h) == before
        way = a.get("way-out-write", "-")
        f = out / caller
        reached = f.read_text() != "host\n"
        # An exec that does not join the run enters on its own (T-1910).
        alone = "does not share the mounts" in err
        print(f"  | {label}: exit {code}; the volume is a mount: {a.get('volume-is-a-mount', '-')}; "
              f"the way out of the chroot: {way}{', the host file changed' if reached else ''}"
              + ("; exec did not join the run and entered on its own" if alone else "")
              + (f"; {err.strip()[-300:]}" if code else ""))
        ok = code == 0 and not missing and not worked and same
        what = (f"{label}: " + (f"no answer for {missing}" if missing else
                f"{len(CALLS) - len(worked)} of {len(CALLS)} mount calls fail"
                + (f" (worked: {', '.join(worked)})" if worked else ""))
                + f"; the volume source {'is the same' if same else 'CHANGED'}")
        verdict(n, ok, what, open_row)
        return reached

    def restore():
        (h / "f").write_text("original\n")
        if root:
            os.chown(h / "f", 1000, 1000)

    vol = ["-v", f"{h}:/x:ro", "--tmpfs", "/w:size=8m"]
    script = "cat > /tmp/p && chmod 755 /tmp/p && /tmp/p {}"

    def foreground(caller, extra=vol, binary=binary):
        f = target(caller)
        base = digest(h)
        code, o, e = run(caller, ["run", "--rm", "-i"] + extra + [IMAGE, "sh", "-c", script.format(f)],
                         binary, probe_bytes)
        return code, o, e, base

    def detached(caller, verb, extra=vol):
        f = target(caller)
        base = digest(h)
        name = f"t489{verb.replace(' ', '')}"
        run(caller, ["rm", "-f", name])
        if verb == "create":
            c1, _, e1 = run(caller, ["create", "--name", name] + extra + [IMAGE, "sleep", "300"])
            c2, _, e2 = run(caller, ["start", name])
            c1, e1 = max(c1, c2), e1 + e2
        else:
            c1, _, e1 = run(caller, ["run", "-d", "--name", name] + extra + [IMAGE, "sleep", "300"])
        code, o, e = run(caller, ["exec", "-i", name, "sh", "-c", script.format(f)], stdin=probe_bytes)
        run(caller, ["rm", "-f", name])
        return max(code, c1), o, e1 + e, base

    reached = []
    for caller in ("uid1000", "ns", "root"):
        homes[caller] = work / f"home-{caller}"
        homes[caller].mkdir()
        if root and caller != "root":
            os.chown(homes[caller], 1000, 1000)
    if root:
        subprocess.run(["chown", "-R", "1000:1000", str(h), str(h2), str(out)], check=True)
    for caller in callers:
        if caller == "root" and not root:
            continue
        code, _, err = run(caller, ["pull", "-q", IMAGE])
        if code != 0:
            print(f"COULD NOT RUN: pull as {caller} exited {code}\n{err[-800:]}")
            return 2

    # 1 and 2: uid 1000 in the foreground, and exec into that run.
    reached.append(judge(1, "uid 1000, foreground", *foreground("uid1000"), caller="uid1000"))
    restore()
    f = target("uid1000")
    base = digest(h)
    fg = subprocess.Popen(["timeout", str(BOUND)] + callers["uid1000"]
                          + [str(binary), "run", "--rm", "--name", "t489fg"] + vol + [IMAGE, "sleep", "60"],
                          env=env_of("uid1000"), stdin=subprocess.DEVNULL,
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(100):
        if run("uid1000", ["exec", "t489fg", "true"])[0] == 0:
            break
        time.sleep(0.2)
    code, o, e = run("uid1000", ["exec", "-i", "t489fg", "sh", "-c", script.format(f)], stdin=probe_bytes)
    run("uid1000", ["kill", "t489fg"])
    fg.wait(timeout=BOUND)
    reached.append(judge(2, "uid 1000, exec into a foreground run", code, o, e, base, "uid1000"))
    restore()
    over = ["-v", f"{h2}:/c:overlay"] + vol
    reached.append(judge(3, "uid 1000, run -d with an overlay volume, exec",
                         *detached("uid1000", "run", over), caller="uid1000"))
    restore()
    # 4 to 6: uid 0 of a namespace of one id. A volume of a foreground run
    # of such a caller takes the loader family, so the run has the tmpfs.
    reached.append(judge(4, "uid 0 of one id, foreground (--tmpfs only)",
                         *foreground("ns", ["--tmpfs", "/w:size=8m"]), caller="ns"))
    reached.append(judge(5, "uid 0 of one id, run -d, exec", *detached("ns", "run"), caller="ns"))
    restore()
    reached.append(judge(6, "uid 0 of one id, create, start, exec", *detached("ns", "create"), caller="ns"))
    restore()
    # 7 to 11: the paths that wait for pod-enter.
    reached.append(judge(7, "uid 1000, run -d, exec", *detached("uid1000", "run"), caller="uid1000",
                         open_row=True))
    restore()
    reached.append(judge(8, "uid 1000, create, start, exec", *detached("uid1000", "create"),
                         caller="uid1000", open_row=True))
    restore()
    if root:
        reached.append(judge(9, "root, foreground (--tmpfs only)",
                             *foreground("root", ["--tmpfs", "/w:size=8m"]), caller="root", open_row=True))
        reached.append(judge(10, "root, run -d, exec", *detached("root", "run"), caller="root", open_row=True))
        restore()
        reached.append(judge(11, "root, create, start, exec", *detached("root", "create"), caller="root",
                             open_row=True))
        restore()
    else:
        for n in (9, 10, 11):
            verdict(n, True, "SKIPPED: the rows of root need root")
    # 12: --read-only.
    code, o, e, _ = foreground("uid1000", ["--read-only", "--tmpfs", "/tmp"])
    a = parse(o)
    verdict(12, code == 0 and a.get("remount-root", "OK") != "OK" and a.get("volume-is-a-mount") == "no",
            f"--read-only: exit {code}, remount of / {a.get('remount-root', '-')}")
    # 13: the way out of the chroot.
    verdict(13, not any(reached), f"the way out of the chroot wrote a host file in {sum(reached)} of "
            f"{len(reached)} rows", open_row=True)
    restore()
    # 14: the binary before the fix.
    if before is None:
        print("FAIL 14: give PODVM_489_BEFORE")
        rows.add(14)
        fail = 1
    else:
        code, o, e, _ = foreground("uid1000", binary=before)
        a = parse(o)
        changed = (h / "f").read_text() != "original\n"
        verdict(14, a.get("remount-ro-volume") == "OK" and changed,
                f"before the fix: remount of the :ro volume {a.get('remount-ro-volume', '-')}, "
                f"the host file {'changed' if changed else 'the same'}")
        restore()
    time.sleep(0.5)
    stayed = podvm_procs({str(binary), str(before)})
    trees = [t for c in homes for t in (homes[c] / "store" / "trees").glob("*")
             if not t.name.startswith(".")] if homes else []
    verdict(15, not stayed and not trees, f"{len(stayed)} process(es) of podvm and {len(trees)} tree(s) stayed")
    missing = set(range(1, CLAUSES + 1)) - rows
    if missing:
        print(f"FAIL: no row for clause(s) {sorted(missing)}")
        return 1
    if opens:
        print(f"OPEN: rows {opens} wait for pod-enter (TODO/enter.md T-2105)")
    return fail


def main():
    if sys.version_info < (3, 11) or sys.platform != "linux" or os.uname().machine != "x86_64":
        print("COULD NOT RUN: use Python 3.11 or later on x86_64 Linux")
        return 2
    for tool in ("cc", "setpriv", "timeout", "unshare"):
        if shutil.which(tool) is None:
            print(f"COULD NOT RUN: no {tool} on PATH")
            return 2
    binary = Path(os.environ.get(
        "PODVM_489_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm")).resolve()
    before = os.environ.get("PODVM_489_BEFORE")
    before = Path(before).resolve() if before else None
    for b in [binary] + ([before] if before else []):
        if not os.access(b, os.X_OK):
            print(f"COULD NOT RUN: {b} is not an executable podvm")
            return 2
    print("Can the payload of the namespace rung change a mount of its run?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}")
    for label, b in (("podvm", binary), ("before", before)):
        if b:
            with b.open("rb") as source:
                print(f"{label} SHA-256 {hashlib.file_digest(source, 'sha256').hexdigest()}")
    print(f"image {IMAGE}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-489-", dir="/var/tmp") as name:
        os.chmod(name, 0o755)
        try:
            result = prove(binary, before, Path(name))
        except (subprocess.SubprocessError, OSError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
        finally:
            if os.getuid() == 0:
                subprocess.run(["chown", "-R", "0:0", name], check=False)
                subprocess.run(["chmod", "-R", "u+rwX", name], check=False)
    if result == 0:
        print("EACH LOCKED ROW HOLDS")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
