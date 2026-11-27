#!/usr/bin/env python3
"""Does `--read-only` keep the namespace rung with a root that stays read-only?

TODO/cli.md T-2108 (OR-B42-02 of the eleventh outside pass). Run with
Python 3.11 or later on an x86_64 Linux host that grants user namespaces
and has `zig` (the probe for musl), `setpriv` and `unshare`, as
root (each caller then runs as uid 1000, as root, and as uid 0 of a user
namespace of one id that uid 1000 opens) or as a user with no root (the
rows of root and of the namespace of one id are skipped). No guest
starts.

PODVM_490_BIN selects the binary. PODVM_490_BEFORE names a binary before
the fix (the release binary of `main`): it must show the fault.

A static probe goes into `/tmp` of the run through stdin and tries:
`mount(NULL, "/", NULL, MS_REMOUNT | MS_BIND, NULL)` to clear `ro`, then a
write of `/x`; the same in a mount namespace of its own; and an `openat`
of a file in `/tmp` by the `syscall` instruction, which no interposer sees.
The same probe, linked dynamically, gives the raw `openat` of rows 4 and 16.

Clauses (a row marked open names a path that waits for another entry; it
prints OPEN while the fault shows, PASS when the path holds, and fails the
run only where its row is missing):
  1. uid 1000, foreground: `--verbose` gives `mode=namespace`.
  2. uid 1000, foreground: `touch /x` gives EROFS; `/tmp` and `/dev/null`
     are writable.
  3. uid 1000, foreground: the probe cannot make `/` writable (locked).
  4. uid 1000, foreground: the raw `openat` writes the run's `/tmp`, and
     no file comes in the host's `/tmp`.
  5. uid 1000, `--read-only --tmpfs /w:size=1m` runs, and a write past
     1 MiB gives ENOSPC.
  6. uid 1000, foreground: `-v H:/newvol` (no such path in the image) is
     writable, `-v H2:/rov:ro` gives EROFS, and H2 stays the same.
  7. uid 1000, `exec` into a foreground run: EROFS, locked.
  8. uid 1000, `run -d` and `exec`: `.Rung` namespace, EROFS, locked, the
     raw `openat` in the run's `/tmp`, `--tmpfs` ENOSPC, the volumes.
  9. uid 1000, `create`, `start` and `exec`: as 8.
 10. uid 1000, `run -d` and `create`: `/dev/null` of `exec` is writable
     (`/dev` of the tree stays writable): open, as the host's devices in
     the launcher's own namespace are T-2107.
 11. root, foreground: `mode=namespace`, EROFS, the run's `/tmp`, ENOSPC.
 12. root, `run -d` and `exec`, `create`, `start` and `exec`: `.Rung`
     namespace and EROFS.
 13. root: the probe makes `/` writable: open, it waits for pod-enter
     (T-2105: the payload of root holds each capability of the initial
     user namespace).
 14. uid 0 of a namespace of one id: a foreground run, `run -d` and
     `exec`: `mode=namespace`, EROFS, locked.
 15. uid 1000 with PODVM_USERNS=0 (podvm owns no mount namespace): the
     read-only layer of the interposer as before, `mode=userland`, EROFS.
 16. PODVM_490_BEFORE: uid 1000 gets `mode=userland`, and the raw
     `openat` writes the host's `/tmp` (the fault).
 17. The cost: the median of 20 runs of `run --rm --read-only IMAGE true`
     before and after, and of the same run with no `--read-only`.
 18. No process of podvm stays, and no tree of a run stays in a store.

Exit 0 is a match, 1 is a failed clause or a missing row, and 2 means a
required input was absent.
"""

import datetime
import hashlib
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
IMAGE = "mirror.gcr.io/library/alpine:3.20"
BOUND = 120
CLAUSES = 18
ROUNDS = 20
PROBE = r"""
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <sched.h>
#include <stdio.h>
#include <string.h>
#include <sys/mount.h>
#include <sys/statvfs.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>
static unsigned long keep(const char *p) {
  struct statvfs s;
  if (statvfs(p, &s) != 0) return 0;
  return s.f_flag & (MS_NOSUID | MS_NODEV | MS_NOEXEC | MS_NOATIME | MS_NODIRATIME | MS_RELATIME);
}
static void say(const char *what, int r) {
  printf("%s: %s\n", what, r == 0 ? "OK" : strerror(errno));
  fflush(stdout);
}
static int rw_root(void) { return mount(NULL, "/", NULL, MS_REMOUNT | MS_BIND | keep("/"), NULL); }
static int write_root(void) {
  int fd = open("/x", O_WRONLY | O_CREAT | O_TRUNC, 0644);
  if (fd < 0) return -1;
  int ok = write(fd, "x\n", 2) == 2;
  close(fd);
  return ok ? 0 : -1;
}
static long raw_openat(const char *p) {
  long r;
  register long mode __asm__("r10") = 0644;
  __asm__ volatile("syscall" : "=a"(r)
                   : "a"(SYS_openat), "D"(AT_FDCWD), "S"(p), "d"(O_WRONLY | O_CREAT | O_TRUNC), "r"(mode)
                   : "rcx", "r11", "memory");
  return r;
}
int main(int argc, char **argv) {
  if (argc < 2) return 2;
  say("remount-root", rw_root());
  say("write-root", write_root());
  pid_t c = fork();
  if (c == 0) {
    if (unshare(CLONE_NEWNS) != 0) { say("ns-remount-root", -1); say("ns-write-root", -1); _exit(0); }
    say("ns-remount-root", rw_root());
    say("ns-write-root", write_root());
    _exit(0);
  }
  waitpid(c, NULL, 0);
  long fd = raw_openat(argv[1]);
  if (fd < 0) { errno = (int)-fd; say("raw-openat-tmp", -1); }
  else { int ok = write((int)fd, "raw\n", 4) == 4; close((int)fd); errno = EIO; say("raw-openat-tmp", ok ? 0 : -1); }
  int t = open(argv[1], O_RDONLY);
  char b[8] = {0};
  printf("run-tmp-has-it: %s\n", t >= 0 && read(t, b, 4) == 4 && memcmp(b, "raw\n", 4) == 0 ? "yes" : "no");
  return 0;
}
"""
# The writes of a payload with a read-only root, one line each.
WRITES = ("if touch /x 2>&1 | grep -q 'Read-only file system'; then echo root=erofs; "
          "else echo root=writable; fi; echo t > /tmp/t && echo tmp=ok; "
          "echo n > /dev/null && echo null=ok; "
          "yes x | dd of=/w/big bs=1k count=2000 iflag=fullblock 2>&1 | grep -q 'No space left on device' "
          "&& echo enospc=yes; test -d /newvol || exit 0; echo v > /newvol/f && echo newvol=ok; "
          "if (echo r > /rov/f) 2>&1 | grep -q 'Read-only file system'; then echo rov=erofs; fi")
LOCK_CALLS = ("remount-root", "write-root", "ns-remount-root", "ns-write-root")


def digest(d):
    """Each file under `d`: its SHA-256."""
    return {str(p.relative_to(d)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(d.rglob("*")) if p.is_file()}


def podvm_procs(binaries):
    """The pids of each live process (not a zombie) of one of `binaries`."""
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
    """The answers of the probe (`name: words`) and of WRITES (`name=word`)."""
    a = {}
    for line in out.splitlines():
        sep = ": " if ": " in line else "="
        if sep in line:
            k, v = line.split(sep, 1)
            a.setdefault(k.strip(), v.strip())
    return a


def compile_probe(work, link):
    """The probe for musl, `-static` or `-dynamic` (the loader of the image,
    `/lib/ld-musl-x86_64.so.1`: the interposer of the userland rung sees
    its calls, not its raw `syscall` instruction)."""
    src, out = work / "probe.c", work / f"probe{link}"
    src.write_text(PROBE)
    cmd = ["zig", "cc", "-target", "x86_64-linux-musl", link, "-O1", "-o", str(out), str(src)]
    r = subprocess.run(cmd, capture_output=True, text=True)
    return out if r.returncode == 0 else None


def prove(binary, before, work):
    probe, dynamic = compile_probe(work, "-static"), compile_probe(work, "-dynamic")
    if probe is None or dynamic is None:
        print("COULD NOT RUN: the probe did not compile")
        return 2
    probe_bytes, dynamic_bytes = probe.read_bytes(), dynamic.read_bytes()
    h, h2 = work / "H", work / "H2"
    for d in (h, h2):
        d.mkdir()
    (h2 / "f").write_text("original\n")
    root = os.getuid() == 0
    drop = ["setpriv", "--reuid", "1000", "--regid", "1000", "--clear-groups"] if root else []
    callers = {"uid1000": drop, "root": [], "ns": drop + ["unshare", "--user", "--map-root-user"]}
    homes = {}
    fail, rows, opens = 0, set(), []

    def env_of(caller, extra=None):
        home = homes[caller]
        e = dict(os.environ, HOME=str(home), TMPDIR=str(home), PODVM_STORE=str(home / "store"),
                 PODVM_SPARE_TREE="0", PATH="/usr/local/bin:/usr/bin:/bin")
        e.update(extra or {})
        return e

    def run(caller, argv, binary=binary, stdin=None, extra=None):
        full = ["timeout", str(BOUND)] + callers[caller] + [str(binary)] + argv
        try:
            r = subprocess.run(full, env=env_of(caller, extra), input=stdin, capture_output=True)
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

    def mode_of(err):
        for line in err.splitlines():
            if "mode=" in line:
                return line.split("mode=", 1)[1].split(" ", 1)[0]
        return "-"

    def locked(a):
        worked = [c for c in LOCK_CALLS if a.get(c) == "OK"]
        missing = [c for c in LOCK_CALLS if c not in a]
        return not worked and not missing, worked, missing

    def host_tmp(name):
        return Path("/tmp") / name

    # A volume of a foreground run of uid 0 takes the loader family (T-1761),
    # so the rows of uid 0 in the foreground have no volume.
    plain = ["--read-only", "--tmpfs", "/w:size=1m"]
    flags = plain + ["-v", f"{h}:/newvol", "-v", f"{h2}:/rov:ro"]
    probe_cmd = "cat > /tmp/p && chmod 755 /tmp/p && /tmp/p /tmp/{}"

    def writes_ok(a, volumes=True, null=True, tmpfs=True):
        vols = a.get("newvol") == "ok" and a.get("rov") == "erofs"
        return (a.get("root") == "erofs" and a.get("tmp") == "ok" and (a.get("enospc") == "yes" or not tmpfs)
                and (a.get("null") == "ok" or not null) and (vols or not volumes))

    def writes_line(a):
        return (f"/x {a.get('root', '-')}, /tmp {a.get('tmp', '-')}, /dev/null {a.get('null', '-')}, "
                f"/w ENOSPC {a.get('enospc', '-')}, /newvol {a.get('newvol', '-')}, /rov {a.get('rov', '-')}")

    def raw_ok(a, name):
        return a.get("raw-openat-tmp") == "OK" and a.get("run-tmp-has-it") == "yes" and not host_tmp(name).exists()

    def fg(caller, cmd, extra_flags=flags, binary=binary, stdin=None, env=None):
        return run(caller, ["run", "--rm", "-i"] + extra_flags + [IMAGE, "sh", "-c", cmd],
                   binary, stdin, env)

    def detached(caller, verb, name, flags=flags):
        run(caller, ["rm", "-f", name])
        if verb == "create":
            c1, _, e1 = run(caller, ["create", "--name", name] + flags + [IMAGE, "sleep", "300"])
            c2, _, e2 = run(caller, ["start", name])
            c1, e1 = max(c1, c2), e1 + e2
        else:
            c1, _, e1 = run(caller, ["run", "-d", "--name", name] + flags + [IMAGE, "sleep", "300"])
        rung = run(caller, ["inspect", "--format", "{{.Rung}}", name])[1].strip()
        cw, ow, ew = run(caller, ["exec", "-i", name, "sh", "-c", WRITES])
        tag = f"t490{name}"
        cp, op, ep = run(caller, ["exec", "-i", name, "sh", "-c", probe_cmd.format(tag)], stdin=probe_bytes)
        run(caller, ["rm", "-f", name])
        return c1, e1, rung, parse(ow), cw, parse(op), cp, ep, tag

    for caller in callers:
        homes[caller] = work / f"home-{caller}"
        homes[caller].mkdir()
        if root and caller != "root":
            os.chown(homes[caller], 1000, 1000)
    if root:
        subprocess.run(["chown", "-R", "1000:1000", str(h), str(h2)], check=True)
    active = ["uid1000"] + (["root", "ns"] if root else [])
    for caller in active:
        code, _, err = run(caller, ["pull", "-q", IMAGE])
        if code != 0:
            print(f"COULD NOT RUN: pull as {caller} exited {code}\n{err[-800:]}")
            return 2
    base = digest(h2)

    # 1 to 6: uid 1000 in the foreground.
    code, _, err = run("uid1000", ["run", "--rm", "--verbose", "--read-only", IMAGE, "true"])
    verdict(1, code == 0 and mode_of(err) == "namespace", f"uid 1000, foreground: exit {code}, mode={mode_of(err)}")
    code, o, err = fg("uid1000", WRITES)
    a = parse(o)
    verdict(2, code == 0 and writes_ok(a), f"uid 1000, foreground: exit {code}; {writes_line(a)}")
    code, o, err = fg("uid1000", probe_cmd.format("t490fg"), stdin=probe_bytes)
    a = parse(o)
    ok, worked, missing = locked(a)
    verdict(3, code == 0 and ok, f"uid 1000, foreground: exit {code}; the probe: "
            + ", ".join(f"{c} {a.get(c, '-')}" for c in LOCK_CALLS))
    d = parse(fg("uid1000", probe_cmd.format("t490dyn"), stdin=dynamic_bytes)[1])
    verdict(4, raw_ok(a, "t490fg") and raw_ok(d, "t490dyn"),
            f"uid 1000, foreground: raw openat static {a.get('raw-openat-tmp', '-')} and dynamic "
            f"{d.get('raw-openat-tmp', '-')}, in the run's /tmp: {a.get('run-tmp-has-it', '-')} and "
            f"{d.get('run-tmp-has-it', '-')}, in the host's /tmp: "
            f"{'yes' if host_tmp('t490fg').exists() or host_tmp('t490dyn').exists() else 'no'}")
    code, o, err = fg("uid1000", "yes x | dd of=/w/big bs=1k count=2000 iflag=fullblock", extra_flags=plain)
    o += err
    verdict(5, code == 1 and "No space left on device" in o,
            f"uid 1000, --read-only --tmpfs /w:size=1m: exit {code} (of dd), a write of 2000 KiB gives "
            f"{'ENOSPC' if 'No space left on device' in o else 'no ENOSPC'}")
    o = parse(fg("uid1000", WRITES)[1])
    same = digest(h2) == base
    verdict(6, o.get("newvol") == "ok" and o.get("rov") == "erofs" and same and (h / "f").exists(),
            f"uid 1000, foreground: /newvol {o.get('newvol', '-')} (the host file "
            f"{'is there' if (h / 'f').exists() else 'is NOT there'}), /rov {o.get('rov', '-')}, "
            f"H2 {'the same' if same else 'CHANGED'}")
    # 7: exec into a foreground run.
    fgp = subprocess.Popen(["timeout", str(BOUND)] + callers["uid1000"]
                           + [str(binary), "run", "--rm", "--name", "t490fgx"] + flags + [IMAGE, "sleep", "60"],
                           env=env_of("uid1000"), stdin=subprocess.DEVNULL,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(100):
        if run("uid1000", ["exec", "t490fgx", "true"])[0] == 0:
            break
        time.sleep(0.2)
    cw, ow, _ = run("uid1000", ["exec", "-i", "t490fgx", "sh", "-c", WRITES])
    cp, op, ep = run("uid1000", ["exec", "-i", "t490fgx", "sh", "-c", probe_cmd.format("t490fgx")],
                     stdin=probe_bytes)
    run("uid1000", ["kill", "t490fgx"])
    fgp.wait(timeout=BOUND)
    a, p = parse(ow), parse(op)
    ok, worked, _ = locked(p)
    verdict(7, cw == 0 and writes_ok(a) and ok and raw_ok(p, "t490fgx"),
            f"uid 1000, exec into a foreground run: {writes_line(a)}; locked: {'yes' if ok else worked}")
    # 8 to 10: uid 1000, detached.
    null_rows = []
    for n, verb, name in ((8, "run", "rd"), (9, "create", "cs")):
        c1, e1, rung, w, cw, p, cp, ep, tag = detached("uid1000", verb, name)
        ok, worked, _ = locked(p)
        null_rows.append(w.get("null") == "ok")
        verdict(n, c1 == 0 and rung == "namespace" and writes_ok(w, null=False) and ok and raw_ok(p, tag),
                f"uid 1000, {verb}: exit {c1}, .Rung {rung or '-'}; {writes_line(w)}; locked: "
                f"{'yes' if ok else worked}; raw openat in the run's /tmp: {p.get('run-tmp-has-it', '-')}"
                + (f"; {e1.strip()[-300:]}" if c1 else ""))
    verdict(10, all(null_rows), f"uid 1000, detached: /dev/null of exec writable in {sum(null_rows)} of 2 rows "
            "(TODO/enter.md T-2107)", open_row=True)
    # 11 to 13: root.
    if root:
        code, _, err = run("root", ["run", "--rm", "--verbose", "--read-only", IMAGE, "true"])
        o = parse(fg("root", WRITES, plain)[1])
        p = parse(fg("root", probe_cmd.format("t490root"), plain, stdin=probe_bytes)[1])
        verdict(11, code == 0 and mode_of(err) == "namespace" and writes_ok(o, volumes=False)
                and raw_ok(p, "t490root"), f"root, foreground: mode={mode_of(err)}; {writes_line(o)}; "
                f"raw openat in the run's /tmp: {p.get('run-tmp-has-it', '-')}")
        locks, lines = [], []
        for verb, name in (("run", "rrd"), ("create", "rcs")):
            c1, e1, rung, w, cw, pp, cp, ep, tag = detached("root", verb, name)
            locks.append(locked(pp)[0])
            lines.append((c1 == 0 and rung == "namespace" and writes_ok(w) and raw_ok(pp, tag),
                          f"{verb}: exit {c1}, .Rung {rung or '-'}, {writes_line(w)}"))
        verdict(12, all(ok for ok, _ in lines), "root, " + "; ".join(l for _, l in lines))
        locks.insert(0, locked(p)[0])
        verdict(13, all(locks), f"root: / locked in {sum(locks)} of 3 rows (foreground, run -d, create)",
                open_row=True)
        # 14: uid 0 of a namespace of one id.
        code, _, err = run("ns", ["run", "--rm", "--verbose", "--read-only", IMAGE, "true"])
        o = parse(fg("ns", WRITES, plain)[1])
        p = parse(fg("ns", probe_cmd.format("t490ns"), plain, stdin=probe_bytes)[1])
        c1, e1, rung, w, cw, pp, cp, ep, tag = detached("ns", "run", "nsrd")
        alone = "does not share the mounts" in ep
        verdict(14, code == 0 and mode_of(err) == "namespace" and writes_ok(o, volumes=False) and locked(p)[0]
                and c1 == 0 and rung == "namespace" and writes_ok(w, null=False, tmpfs=not alone)
                and locked(pp)[0],
                f"uid 0 of one id: foreground mode={mode_of(err)}, {writes_line(o)}, locked: "
                f"{'yes' if locked(p)[0] else locked(p)[1]}; run -d .Rung {rung or '-'}, exec {writes_line(w)}, "
                f"locked: {'yes' if locked(pp)[0] else locked(pp)[1]}"
                + ("; exec did not join the run and entered on its own, with no --tmpfs of the run "
                   "(T-2105)" if alone else ""))
    else:
        for n in (11, 12, 13, 14):
            verdict(n, True, "SKIPPED: the rows of root need root")
    # 15: no mount namespace of podvm's own.
    code, _, err = run("uid1000", ["run", "--rm", "--verbose", "--read-only", IMAGE, "true"],
                       extra={"PODVM_USERNS": "0"})
    o = parse(run("uid1000", ["run", "--rm", "--read-only", IMAGE, "sh", "-c", WRITES],
                  extra={"PODVM_USERNS": "0"})[1])
    verdict(15, code == 0 and mode_of(err) == "userland" and o.get("root") == "erofs",
            f"uid 1000, PODVM_USERNS=0: mode={mode_of(err)}, /x {o.get('root', '-')}")
    # 16: the binary before the fix.
    if before is None:
        print("FAIL 16: give PODVM_490_BEFORE")
        rows.add(16)
        fail = 1
    else:
        code, _, err = run("uid1000", ["run", "--rm", "--verbose", "--read-only", IMAGE, "true"], before)
        p = parse(run("uid1000", ["run", "--rm", "-i", "--read-only", "--tmpfs", "/tmp", IMAGE, "sh", "-c",
                                  probe_cmd.format("t490before")], before, dynamic_bytes)[1])
        leaked = host_tmp("t490before").exists()
        verdict(16, mode_of(err) == "userland" and leaked,
                f"before the fix: mode={mode_of(err)}; the raw openat wrote the host's /tmp: "
                f"{'yes' if leaked else 'no'}")
        if leaked:
            host_tmp("t490before").unlink()

    # 17: the cost. The three runs of a round go one after the other, so a
    # change of the load of the host reaches each of them.
    costs = []
    for caller in (["uid1000", "root"] if root else ["uid1000"]):
        kinds = [("--read-only", ["--read-only"], binary), ("no --read-only", [], binary)]
        kinds += [("--read-only before", ["--read-only"], before)] if before else []
        times = {k: [] for k, _, _ in kinds}
        for _ in range(ROUNDS):
            for k, args, b in kinds:
                t = time.monotonic()
                run(caller, ["run", "--rm"] + args + [IMAGE, "true"], b)
                times[k].append((time.monotonic() - t) * 1000)
        costs.append(f"{caller}: " + ", ".join(f"{k} {statistics.median(v):.0f} ms" for k, v in times.items()))
    verdict(17, True, f"the median of {ROUNDS} runs of run --rm IMAGE true, an estimate: " + "; ".join(costs))
    # 18: nothing stays.
    time.sleep(0.5)
    stayed = podvm_procs({str(binary)} | ({str(before)} if before else set()))
    trees = [t for c in homes for t in (homes[c] / "store" / "trees").glob("*")
             if not t.name.startswith(".")]
    verdict(18, not stayed and not trees, f"{len(stayed)} process(es) of podvm and {len(trees)} tree(s) stayed")
    missing = set(range(1, CLAUSES + 1)) - rows
    if missing:
        print(f"FAIL: no row for clause(s) {sorted(missing)}")
        return 1
    if opens:
        print(f"OPEN: rows {opens} wait for another entry (TODO/enter.md T-2105, T-2107)")
    return fail


def main():
    if sys.version_info < (3, 11) or sys.platform != "linux" or os.uname().machine != "x86_64":
        print("COULD NOT RUN: use Python 3.11 or later on x86_64 Linux")
        return 2
    for tool in ("setpriv", "timeout", "unshare"):
        if shutil.which(tool) is None:
            print(f"COULD NOT RUN: no {tool} on PATH")
            return 2
    if not shutil.which("zig"):
        print("COULD NOT RUN: no zig on PATH (the probe for musl, static and dynamic)")
        return 2
    binary = Path(os.environ.get(
        "PODVM_490_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm")).resolve()
    before = os.environ.get("PODVM_490_BEFORE")
    before = Path(before).resolve() if before else None
    for b in [binary] + ([before] if before else []):
        if not os.access(b, os.X_OK):
            print(f"COULD NOT RUN: {b} is not an executable podvm")
            return 2
    print("Does --read-only keep the namespace rung with a root that stays read-only?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}")
    for label, b in (("podvm", binary), ("before", before)):
        if b:
            with b.open("rb") as source:
                print(f"{label} SHA-256 {hashlib.file_digest(source, 'sha256').hexdigest()}")
    print(f"image {IMAGE}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-490-", dir="/var/tmp") as name:
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
        print("EACH HELD ROW HOLDS")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
