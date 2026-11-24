#!/usr/bin/env python3
"""Watch the Actions runs of a repository through the forge service of
pkgforge (https://forge.web.pkgforge.dev, GitHub's REST API with no token).

Each poll prints one line for each run that is new or that changed state,
and for a run that ended red, one line for each failed job with its failed
steps. Run it with `python3 -I`.

  python3 -I scripts/agent/runs-watch.py [--repo OWNER/NAME]
      [--branch main] [--every 60] [--until SHA] [--timeout 3600] [--once]

The owner's ruling "Watchers" (2026-11-24, docs/decisions.md): each session
starts it at its start as a background process with a log, beside the
watcher of the organization (AGENTS.md, sections 8 and 9), and reads the
log itself; a notice of the harness is not relied on. podcrates'
scripts/agent/watch-runs.sh follows one commit of podcrates only; COORD-245
asks podcrates to take this script as the shared one.

--until SHA  poll until each run of the commit SHA (a prefix is enough)
             ended; exit 0 when each one is green (success or skipped), 1
             when one is red, 2 at the timeout or when the API does not
             answer. A run that a newer push cancelled counts as neither:
             the line says so, and the exit is 3 when only cancelled runs
             are left.
--once       one poll, print the state of each run of the newest commit,
             then exit.

The answers of the service: 429 and 503 wait for Retry-After; 502 waits
with backoff; another error (a 403: keyless callers share GitHub's 60
reads an hour of each egress address) is one line on stderr, and the next
poll waits twice as long, up to 600 s, until a poll passes again. Keep
--every at 120 or more where other readers share the address.
"""

import json
import sys
import time
import urllib.error
import urllib.request

BASE = "https://forge.web.pkgforge.dev/github"
GREEN = {"success", "skipped", "neutral"}


def get(path, tries=5):
    """The JSON of GET BASE/path, or None after `tries` failures."""
    delay = 5
    for _ in range(tries):
        sep = "&" if "?" in path else "?"
        url = f"{BASE}/{path}{sep}n={time.time_ns()}"
        req = urllib.request.Request(url, headers={"User-Agent": "runs-watch"})
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            wait = delay
            if e.code in (429, 503):
                try:
                    wait = max(1, min(600, int(e.headers.get("Retry-After", delay))))
                except ValueError:
                    wait = delay
            elif e.code != 502:
                print(f"runs-watch: GET {path}: HTTP {e.code}", file=sys.stderr, flush=True)
                return None
            time.sleep(wait)
            delay = min(delay * 2, 300)
        except (urllib.error.URLError, TimeoutError, ValueError, OSError) as e:
            print(f"runs-watch: GET {path}: {e}", file=sys.stderr, flush=True)
            time.sleep(delay)
            delay = min(delay * 2, 300)
    return None


def say(line):
    print(f"{time.strftime('%H:%M:%SZ', time.gmtime())} {line}", flush=True)


def failed_jobs(repo, run_id):
    """One line for each job of the run that did not end green."""
    doc = get(f"repos/{repo}/actions/runs/{run_id}/jobs?per_page=100")
    if not doc:
        return ["  (the jobs of the run could not be read)"]
    out = []
    for j in doc.get("jobs", []):
        if j.get("conclusion") in GREEN or j.get("status") != "completed":
            continue
        steps = [s["name"] for s in j.get("steps", []) if s.get("conclusion") not in GREEN | {None}]
        out.append(f"  job {j['id']} {j['name']}: {j.get('conclusion')}"
                   + (f" (steps: {'; '.join(steps)})" if steps else ""))
    return out


def runs(repo, branch):
    doc = get(f"repos/{repo}/actions/runs?branch={branch}&per_page=30")
    return None if doc is None else doc.get("workflow_runs", [])


def main(argv):
    opt = {"--repo": "PodUtils/podvm", "--branch": "main", "--every": "120",
           "--until": None, "--timeout": "3600"}
    once = False
    it = iter(argv)
    for a in it:
        if a == "--once":
            once = True
        elif a in opt:
            opt[a] = next(it, None)
            if opt[a] is None:
                print(f"runs-watch: {a} needs a value", file=sys.stderr)
                return 2
        else:
            print(f"runs-watch: unknown argument {a}", file=sys.stderr)
            return 2
    repo, branch, until = opt["--repo"], opt["--branch"], opt["--until"]
    try:
        every, timeout = max(20, int(opt["--every"])), int(opt["--timeout"])
    except ValueError:
        print("runs-watch: --every and --timeout take seconds", file=sys.stderr)
        return 2
    seen = {}
    start = time.monotonic()
    wait = every
    while True:
        rs = runs(repo, branch)
        if rs is None:
            wait = min(wait * 2, 600)
            say(f"the API gave no list of runs; the next poll in {wait} s")
        else:
            wait = every
            if once:
                head = rs[0]["head_sha"] if rs else ""
                for r in rs:
                    if r["head_sha"] == head:
                        say(f"{r['name']} {r['id']} {head[:7]} {r['status']} {r.get('conclusion')}")
                return 0
            for r in reversed(rs):
                key = (r["status"], r.get("conclusion"), r.get("run_attempt"))
                if seen.get(r["id"]) == key:
                    continue
                first = r["id"] not in seen
                seen[r["id"]] = key
                # An old run at the first poll is no event, except a run of
                # the commit of --until.
                if first and r["status"] == "completed" and not (
                        until and r["head_sha"].startswith(until)):
                    continue
                say(f"{r['name']} {r['id']} ({r['event']}, {r['head_sha'][:7]}): "
                    f"{r['status']} {r.get('conclusion') or ''}".rstrip())
                if r["status"] == "completed" and r.get("conclusion") not in GREEN | {"cancelled"}:
                    for line in failed_jobs(repo, r["id"]):
                        say(line)
            if until:
                mine = [r for r in rs if r["head_sha"].startswith(until)]
                if mine and all(r["status"] == "completed" for r in mine):
                    ends = {r.get("conclusion") for r in mine}
                    if ends - GREEN - {"cancelled"}:
                        say(f"{until}: RED")
                        return 1
                    if ends <= {"cancelled"}:
                        say(f"{until}: each run was cancelled")
                        return 3
                    say(f"{until}: GREEN")
                    return 0
        if until and time.monotonic() - start > timeout:
            say(f"{until}: no end within {timeout} s")
            return 2
        time.sleep(wait)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
