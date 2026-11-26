#!/usr/bin/env python3
"""Does `-v HOST:GUEST:overlay` keep HOST as it was and give GUEST a layer?

TODO/enter.md T-2101 (PodUtils/podrunner#368, COORD-141 of podrunner). Run
with Python 3.11 or later on an x86_64 Linux host that grants user
namespaces and has `cc`, as root (each podvm then runs as uid 1000 with a
private store) or as a user with no root. `scripts/dev/nouserns.c` denies
user namespaces, so podvm takes the userland rung, as on a GitHub-hosted
runner. No guest starts: each guest tier refuses before its dispatch.

PODVM_485_BIN selects the binary. PODVM_485_HOME is an optional HOME whose
store already holds the image.

Clauses:
  1. A confined `run -d` with `-v H:/c:overlay` writes, changes, chmods,
     renames and removes files of H and runs a script of H through /c.
  2. `exec` in the unit sees each change: the upper layer over the lower
     one, and no removed or renamed file.
  3. The upper layer is in the tree of the container in the store.
  4. `rm -f` leaves no tree and no layer of the container.
  5. A foreground `run --rm` with the same layer sees its changes, and
     leaves no tree after its end.
  6. `create` and `start` hold the layer, and `exec` sees it.
  7. Each rung and tier that cannot hold it refuses by name with 125: the
     userland rung (`run` and `run -d`), PODVM_USERNS=0, the syscall,
     emulate, machine, incus and runsc tiers; and `overlay,ro` conflicts.
  8. `system info` names the rungs: the namespace rung, held here, and not
     held under the userland rung.
  9. H is the same byte for byte and mode for mode after each run (a
     SHA-256 of each file), and no process of a run stays.

Exit 0 is a match, 1 is a failed clause or a missing row, and 2 means a
required input was absent.
"""

import datetime
import hashlib
import json
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
CLAUSES = 9
CONFINE = "read=/;write=/c,/tmp,/dev/null;exec=/bin,/usr,/lib,/c"
# The payload of a run: each change of the lower layer, then a script of H.
CHANGE = ("echo new > /c/new && echo changed > /c/a && chmod 600 /c/b && "
          "mv /c/b /c/b2 && rm /c/sub/c && /c/bin/hello.sh > /c/ran")
# The view that `exec` must see.
LOOK = ("cat /c/a /c/new /c/b2 /c/ran; stat -c %a /c/b2; "
        "test -e /c/b && echo b-still; test -e /c/sub/c && echo c-still; echo end")
WANT = "changed\nnew\nbeta\nhello from H\n600\nend\n"


def digest(d):
    """Each path under `d`: its mode and the SHA-256 of a file."""
    out = {}
    for p in sorted(d.rglob("*")):
        st = p.lstat()
        sha = hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else "-"
        out[str(p.relative_to(d))] = (oct(st.st_mode), sha)
    return out


def podvm_procs():
    """The pids of each live process (not a zombie that init has yet to
    reap) of uid 1000 whose command line names podvm."""
    found = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit() or int(entry.name) == os.getpid():
            continue
        try:
            if entry.stat().st_uid != 1000:
                continue
            cmd = (entry / "cmdline").read_bytes()
            stat = (entry / "stat").read_text()
        except OSError:
            continue
        if stat[stat.rfind(")") + 2:].startswith("Z"):
            continue
        if b"podvm" in cmd:
            found.append(int(entry.name))
    return found


def prove(binary, work):
    nouserns = work / "nouserns"
    r = subprocess.run(["cc", "-O1", "-o", str(nouserns), str(ROOT / "scripts/dev/nouserns.c")],
                       capture_output=True, text=True)
    if r.returncode != 0:
        print(f"COULD NOT RUN: cc exited {r.returncode}")
        return 2
    home = Path(os.environ.get("PODVM_485_HOME", work / "home"))
    home.mkdir(exist_ok=True)
    h = work / "H"
    (h / "sub").mkdir(parents=True)
    (h / "bin").mkdir()
    (h / "a").write_text("alpha\n")
    (h / "b").write_text("beta\n")
    (h / "sub/c").write_text("gamma\n")
    (h / "bin/hello.sh").write_text("#!/bin/sh\necho hello from H\n")
    (h / "bin/hello.sh").chmod(0o755)
    (h / "b").chmod(0o644)
    wrap = []
    if os.getuid() == 0:
        wrap = ["setpriv", "--reuid", "1000", "--regid", "1000", "--clear-groups"]
        subprocess.run(["chown", "-R", "1000:1000", str(work), str(home)], check=True)
    store = home / "store"
    env = dict(os.environ, HOME=str(home), TMPDIR=str(home), PODVM_STORE=str(store),
               PODVM_SPARE_TREE="0", PATH="/usr/local/bin:/usr/bin:/bin")
    before = digest(h)
    fail, rows = 0, set()

    def run(argv, extra=None, deny=False):
        e = dict(env, **(extra or {}))
        full = wrap + ([str(nouserns)] if deny else []) + [str(binary)] + argv
        try:
            r = subprocess.run(["timeout", str(BOUND)] + full, env=e, stdin=subprocess.DEVNULL,
                               capture_output=True, text=True, errors="replace")
        except OSError as error:
            return 127, "", str(error)
        return r.returncode, r.stdout, r.stderr.replace(str(work), "WORK")

    def verdict(n, ok, what):
        nonlocal fail
        rows.add(n)
        print(f"{'PASS' if ok else 'FAIL'} {n}: {what}", flush=True)
        fail |= not ok

    def unchanged(when):
        now = digest(h)
        if now != before:
            print(f"  | H changed {when}: {sorted(set(now.items()) ^ set(before.items()))}")
        return now == before

    def trees():
        t = store / "trees"
        return sorted(p.name for p in t.iterdir() if not p.name.startswith(".")) if t.is_dir() else []

    code, _, err = run(["pull", "-q", IMAGE])
    if code != 0:
        print(f"COULD NOT RUN: pull exited {code}\n{err[-1500:]}")
        return 2
    vol = ["-v", f"{h}:/c:overlay"]
    ok_h = True

    # 1 to 4: a confined unit of run -d.
    code, out, err = run(["run", "-d", "--name", "u485", "--podvm-confine", CONFINE] + vol
                         + [IMAGE, "sh", "-c", CHANGE + " && sleep 300"])
    print(f"  | run -d: exit {code}: {out.strip()[:12]} {err.strip()[-300:]}")
    seen = ""
    for _ in range(50):
        c2, seen, err2 = run(["exec", "u485", "sh", "-c", "test -e /c/ran && " + LOOK])
        if c2 == 0 and seen.endswith("end\n"):
            break
        time.sleep(0.2)
    verdict(1, code == 0 and seen.startswith("changed"), f"run -d exit {code}; the payload made each change")
    verdict(2, seen == WANT, f"exec sees {seen!r}")
    layer = [p for p in (store / "trees").glob("*/overlay/0/upper/a")]
    upper_ok = len(layer) == 1 and layer[0].read_text() == "changed\n"
    whiteout = [p for p in (store / "trees").glob("*/overlay/0/upper/sub/c")]
    verdict(3, upper_ok and len(whiteout) == 1,
            f"{len(layer)} upper layer(s) in the trees of the store, with a whiteout of sub/c")
    ok_h &= unchanged("by run -d")
    held = trees()
    code, _, err = run(["rm", "-f", "u485"])
    left = trees()
    verdict(4, code == 0 and held and not left and not list(store.glob("trees/*/overlay")),
            f"rm -f exit {code}; trees before {len(held)}, after {len(left)}"
            + (f": {err.strip()[-300:]}" if code else ""))

    # 5: a foreground run.
    code, out, err = run(["run", "--rm", "--podvm-confine", CONFINE] + vol
                         + [IMAGE, "sh", "-c", CHANGE + " && " + LOOK])
    verdict(5, code == 0 and out == WANT and not trees(),
            f"foreground exit {code}, output {out!r}, trees after {len(trees())}"
            + (f": {err.strip()[-300:]}" if code else ""))
    ok_h &= unchanged("by a foreground run")

    # 6: create and start.
    code, _, err = run(["create", "--name", "c485"] + vol
                       + [IMAGE, "sh", "-c", "echo created > /c/a && sleep 300"])
    c2, _, err2 = run(["start", "c485"])
    seen = ""
    for _ in range(50):
        c3, seen, _ = run(["exec", "c485", "cat", "/c/a"])
        if seen == "created\n":
            break
        time.sleep(0.2)
    c4, _, _ = run(["rm", "-f", "c485"])
    verdict(6, (code, c2, c4) == (0, 0, 0) and seen == "created\n" and not trees(),
            f"create {code}, start {c2}, exec sees {seen!r}, rm {c4}, trees after {len(trees())}"
            + (f": {(err + err2).strip()[-300:]}" if code or c2 else ""))
    ok_h &= unchanged("by create and start")

    # 7: each refusal by name.
    cases = [
        ("userland run", ["run", "--rm"] + vol + [IMAGE, "true"], None, True),
        ("userland run -d", ["run", "-d"] + vol + [IMAGE, "true"], None, True),
        ("PODVM_USERNS=0", ["run", "--rm"] + vol + [IMAGE, "true"], {"PODVM_USERNS": "0"}, False),
        ("syscall tier", ["run", "--rm", "--podvm-tier=syscall"] + vol + [IMAGE, "true"], None, False),
        ("emulate tier", ["run", "--rm", "--podvm-tier=emulate"] + vol + [IMAGE, "true"], None, False),
        ("machine tier", ["run", "--rm", "--podvm-tier=machine"] + vol + [IMAGE, "true"], None, False),
        ("incus tier", ["run", "--rm", "--podvm-tier=incus"] + vol + [IMAGE, "true"], None, False),
        ("runsc tier", ["run", "--rm", "--podvm-tier=runsc"] + vol + [IMAGE, "true"], None, False),
        ("overlay,ro", ["run", "--rm", "-v", f"{h}:/c:overlay,ro", IMAGE, "true"], None, False),
    ]
    named = []
    for label, argv, extra, deny in cases:
        code, _, err = run(argv, extra, deny)
        line = next((l for l in err.splitlines() if ":overlay" in l and "T-2101" in l), "")
        good = code == 125 and bool(line)
        named.append(good)
        print(f"  | {label}: exit {code}: {(line or err.strip())[:240]}")
    verdict(7, all(named) and not trees(), f"{sum(named)} of {len(cases)} refusals by name at 125")
    ok_h &= unchanged("by the refusals")

    # 8: system info.
    code, out, _ = run(["system", "info", "--format", "{{json .VolumeOverlay}}"])
    c2, out2, _ = run(["system", "info", "--format", "{{json .VolumeOverlay}}"], deny=True)
    try:
        here, denied = json.loads(out), json.loads(out2)
    except ValueError:
        here, denied = {}, {}
    verdict(8, here == {"rungs": ["namespace"], "held": True}
            and denied == {"rungs": ["namespace"], "held": False},
            f"here {out.strip()}, with no user namespace {out2.strip()}")

    time.sleep(0.5)
    stayed = podvm_procs()
    verdict(9, ok_h and not stayed,
            f"H {'unchanged' if ok_h else 'CHANGED'} ({len(before)} paths), "
            f"{len(stayed)} process(es) of podvm stayed")
    missing = set(range(1, CLAUSES + 1)) - rows
    if missing:
        print(f"FAIL: no row for clause(s) {sorted(missing)}")
        return 1
    return fail


def main():
    if sys.version_info < (3, 11) or sys.platform != "linux" or os.uname().machine != "x86_64":
        print("COULD NOT RUN: use Python 3.11 or later on x86_64 Linux")
        return 2
    for tool in ("cc", "setpriv", "timeout"):
        if shutil.which(tool) is None:
            print(f"COULD NOT RUN: no {tool} on PATH")
            return 2
    binary = Path(os.environ.get(
        "PODVM_485_BIN", ROOT / "target/x86_64-unknown-linux-musl/release/podvm"
    )).resolve()
    if not os.access(binary, os.X_OK):
        print("COULD NOT RUN: give the podvm binary")
        return 2
    with binary.open("rb") as source:
        sha = hashlib.file_digest(source, "sha256").hexdigest()
    print("Does -v HOST:GUEST:overlay keep HOST as it was and give GUEST a layer?")
    print(f"{datetime.datetime.now(datetime.timezone.utc):%FT%TZ} Linux {os.uname().release}, "
          f"uid {os.getuid()}")
    print(f"podvm SHA-256 {sha}\nimage {IMAGE}", flush=True)
    with tempfile.TemporaryDirectory(prefix="podvm-485-", dir="/var/tmp") as name:
        os.chmod(name, 0o755)
        try:
            result = prove(binary, Path(name))
        except (subprocess.SubprocessError, OSError) as error:
            print(f"FAIL: {str(error).replace(name, 'WORK')}")
            result = 1
        finally:
            if os.getuid() == 0:
                subprocess.run(["chown", "-R", "0:0", name], check=False)
                # overlayfs makes `work/work` with mode 0.
                subprocess.run(["chmod", "-R", "u+rwX", name], check=False)
    if result == 0:
        print("AN OVERLAY VOLUME KEEPS HOST AS IT WAS")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
