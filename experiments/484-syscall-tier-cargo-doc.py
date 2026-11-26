#!/usr/bin/env python3
"""Does `cargo doc` of ripgrep end on the syscall tier as with no tier?

TODO/enter.md T-1991 (PodUtils/podrunner#308, item 3). Run with Python
3.11 or later on an x86_64 host that has `cc`, `git` and `cargo`, as root
(the payload then runs as uid 1000) or as a user with no root. The host
cargo fetches the crates into a CARGO_HOME of the work directory, because a
cargo in the payload cannot pass the proxy of a cloud host.
`scripts/dev/nouserns.c` denies user namespaces, so podvm takes the
userland rung, as on a GitHub-hosted runner.

PODVM_484_BIN selects the binary. PODVM_484_HOME is an optional HOME whose
store already holds the image; PODVM_484_RG is an optional clone of
ripgrep. The job is the `docs` job of ripgrep's `ci.yml` at `178cc027`
with `PCRE2_SYS_STATIC=1`: the build script of `pcre2-sys` then compiles
PCRE2 with `cc`, as on a runner image with no `libpcre2-dev`.

Clauses:
  1. With no tier, `cargo doc` exits 0.
  2. On the syscall tier, `cargo doc` exits 0 within the bound: four times
     the time with no tier, plus 60 s.
  3. On the tier, the build script of `pcre2-sys` compiled
     `pcre2_match.c` (the file whose `cc1` spun before the fix).
  4. No process of the runs stays: each run has a session of its own; a
     run that ends leaves no process in it, and at a bound a SIGKILL of
     the session ends each process within 10 s.

Exit 0 is a match, 1 is a failed clause, and 2 means a required input was
absent.
"""

import datetime
import hashlib
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
IMAGE = (
    "mirror.gcr.io/library/rust:1"
    "@sha256:e84068000361bf3d4a5ba2b7647f62893528b1eb9f847d7a23b493f5ce8ea254"
)
RG = "https://github.com/BurntSushi/ripgrep"
RG_COMMIT = "178cc027e283fd965cb72d0a7395b219eada170c"
JOB = ["cargo", "doc", "--offline", "--no-deps", "--document-private-items", "--workspace"]
# The bound of a run with no tier, and of each step on the host.
BOUND = 900


def in_session(sid):
    """The pids of each live process of session `sid`."""
    found = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            stat = (entry / "stat").read_text()
        except OSError:
            continue
        fields = stat[stat.rfind(")") + 2:].split()
        if len(fields) > 3 and fields[3] == str(sid) and fields[0] != "Z":
            found.append(int(entry.name))
    return found


def end_session(sid):
    """SIGKILL each process of session `sid`, then wait up to 10 s; the
    pids that stayed."""
    for pid in in_session(sid):
        try:
            os.kill(pid, signal.SIGKILL)
        except OSError:
            pass
    for _ in range(100):
        left = in_session(sid)
        if not left:
            break
        time.sleep(0.1)
    return left


def host(argv, work, env=None, cwd=None):
    """Run one step on the host; True where it exits 0."""
    try:
        r = subprocess.run(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                           capture_output=True, text=True, errors="replace",
                           timeout=BOUND)
    except subprocess.TimeoutExpired:
        print(f"COULD NOT RUN: {argv[0]} reached its bound")
        return False
    if r.returncode != 0:
        print(f"COULD NOT RUN: {' '.join(argv[:3])} exited {r.returncode}")
        print(r.stderr.replace(str(work), "WORK")[-2000:], end="")
    return r.returncode == 0


def prove(binary, work):
    nouserns = work / "nouserns"
    if not host(["cc", "-O1", "-o", str(nouserns), str(ROOT / "scripts/dev/nouserns.c")], work):
        return 2
    rg = Path(os.environ.get("PODVM_484_RG", work / "rg"))
    if not rg.is_dir():
        if not host(["git", "clone", "-q", RG, str(rg)], work):
            return 2
    if not host(["git", "-c", "safe.directory=*", "-C", str(rg), "checkout", "-q", RG_COMMIT], work):
        return 2
    cargo_home = work / "ch"
    if not host(["cargo", "fetch", "--locked"], work,
                env=dict(os.environ, CARGO_HOME=str(cargo_home)), cwd=rg):
        return 2
    home = Path(os.environ.get("PODVM_484_HOME", work / "home"))
    home.mkdir(exist_ok=True)
    wrap = [str(nouserns)]
    if os.getuid() == 0:
        wrap = ["setpriv", "--reuid", "1000", "--regid", "1000", "--clear-groups"] + wrap
        for d in (work, home):
            subprocess.run(["chown", "-R", "1000:1000", str(d)], check=True)
        if rg.parent != work:
            subprocess.run(["chown", "-R", "1000:1000", str(rg)], check=True)
    # No spare tree: its detached copy would outlive the run in its session.
    env = dict(os.environ, HOME=str(home), PODVM_SPARE_TREE="0")
    env.pop("PODVM_STORE", None)
    if not host(wrap + [str(binary), "pull", "-q", IMAGE], work, env=env):
        return 2

    def doc(tier, bound):
        target = work / f"target-{tier or 'none'}"
        shutil.rmtree(target, ignore_errors=True)
        argv = wrap + [str(binary), "run", "--rm"]
        argv += [f"--podvm-tier={tier}"] if tier else []
        argv += ["-v", f"{work}:/w", "-v", f"{rg}:/rg", "-w", "/rg",
                 "-e", "CARGO_HOME=/w/ch", "-e", f"CARGO_TARGET_DIR=/w/{target.name}",
                 "-e", "RUSTDOCFLAGS=-D warnings", "-e", "CARGO_TERM_COLOR=never",
                 "-e", "CARGO_BUILD_JOBS=4", "-e", "PCRE2_SYS_STATIC=1", IMAGE] + JOB
        start = time.monotonic()
        # A session of its own: a timeout ends each process of the payload,
        # not podvm alone.
        run = subprocess.Popen(["nice", "-n", "10"] + argv, env=env, stdin=subprocess.DEVNULL,
                               stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
                               start_new_session=True)
        try:
            err = run.communicate(timeout=bound)[1].decode(errors="replace")
            code = run.returncode
        except subprocess.TimeoutExpired:
            code, err = None, ""
        took = time.monotonic() - start
        if code is not None:
            time.sleep(0.5)
            stayed.update(in_session(run.pid))
        stayed.update(end_session(run.pid))
        if run.poll() is None:
            run.kill()
        run.communicate()
        tail = [l for l in err.splitlines() if l.startswith(("   ", " Doc"))][-3:]
        print(f"  | tier {tier or 'none'}: exit {code} in {took:.1f} s (bound {bound} s)")
        for line in tail:
            print(f"  |   {line.strip()}")
        return code, took, target

    fail = 0
    stayed = set()

    def verdict(n, ok, what):
        nonlocal fail
        print(f"{'PASS' if ok else 'FAIL'} {n}: {what}")
        fail |= not ok

    code, base, _ = doc(None, BOUND)
    verdict(1, code == 0, f"with no tier, exit {code} in {base:.1f} s")
    if code != 0:
        return 1
    bound = int(4 * base + 60)
    code, took, target = doc("syscall", bound)
    verdict(2, code == 0, f"on the tier, exit {code} in {took:.1f} s (bound {bound} s)")
    objs = list(target.glob("debug/build/pcre2-sys/*/out/*pcre2_match.o"))
    verdict(3, bool(objs), f"{len(objs)} object(s) of pcre2_match.c on the tier")
    verdict(4, not stayed, f"{len(stayed)} process(es) of the runs stayed")
    return fail


def main():
    if sys.version_info < (3, 11) or sys.platform != "linux" or os.uname().machine != "x86_64":
        print("COULD NOT RUN: use Python 3.11 or later on x86_64 Linux")
        return 2
    for tool in ("cc", "git", "cargo", "setpriv", "nice"):
        if shutil.which(tool) is None:
            print(f"COULD NOT RUN: no {tool} on PATH")
            return 2
    binary = Path(os.environ.get(
        "PODVM_484_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    if not os.access(binary, os.X_OK):
        print("COULD NOT RUN: give the podvm binary")
        return 2
    with binary.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    print("Does `cargo doc` of ripgrep end on the syscall tier as with no tier?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}, {os.cpu_count()} CPUs")
    print(f"podvm SHA-256 {digest}\nimage {IMAGE}\nripgrep {RG_COMMIT}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-484-", dir="/var/tmp") as name:
        os.chmod(name, 0o755)
        try:
            result = prove(binary, Path(name))
        except (subprocess.SubprocessError, OSError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
        finally:
            if os.getuid() == 0:
                subprocess.run(["chown", "-R", "0:0", name], check=False)
    if result == 0:
        print("CARGO DOC OF RIPGREP ENDS ON THE SYSCALL TIER")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
