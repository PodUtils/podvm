#!/bin/sh
# Question: do `--memory`, `--cpus` and `--pids-limit` hold on the native
# tiers where the host delegates a cgroup2 subtree to the caller, and does
# podvm leave no leaf of its own behind?
#
# TODO/cli.md T-1747 (PodUtils/podrunner#110). Run it as a user that is
# not root, from a shell in a writable cgroup2 leaf whose parent is
# writable and gives memory, cpu and pids to its children (the layout of a
# systemd user session; the job `cgroup` of .github/workflows/kvm.yml
# makes it on a runner).
#
# Pinned inputs:
#   podvm   PODVM_459_BIN (default the release build of the workspace)
#   image   mirror.gcr.io/library/alpine:3.20
#
# Clauses:
#   1. `run -d --memory 64m --cpus 0.5 --pids-limit 32` makes one leaf
#      `podvm-*` beside the cgroup of the shell, with memory.max 67108864,
#      cpu.max "50000 100000" and pids.max 32, and the payload in it;
#   2. `run --rm --pids-limit 16` of a shell that starts 40 processes
#      fails a fork;
#   3. `run --rm --memory 32m` of a shell that holds 200 MB ends with 137,
#      and stderr names the OOM kill;
#   4. after `rm -f` and one more run, no leaf `podvm-*` stays;
#   5. the `--report` of the run of clause 3, and of the same payload as
#      `run -d`, has `oom_kills` greater than 0 (TODO/supervise.md T-1969,
#      PodUtils/podrunner#296 ask 6).

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_459_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
OUT="$ROOT/experiments/results/cgroup-limits.txt"
IMG=mirror.gcr.io/library/alpine:3.20
own=$(sed -n 's/^0:://p' /proc/self/cgroup)
CG=/sys/fs/cgroup$own
PARENT=$(dirname "$CG")
if [ "$(stat -f -c %T /sys/fs/cgroup)" != cgroup2fs ] || [ ! -w "$PARENT/cgroup.procs" ]; then
	echo "SKIP: no delegated cgroup2 subtree for this user ($CG)" >&2
	exit 2
fi
fail=0
: >"$OUT"
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }
leaves() { ls -d "$PARENT"/podvm-* "$CG"/podvm-* 2>/dev/null; }

say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "podvm     $("$BIN" version 2>/dev/null)"
say "cgroup    $own (parent $(cat "$PARENT/cgroup.subtree_control"))"
timeout 300 "$BIN" pull -q "$IMG" >/dev/null 2>&1

id=$(timeout 120 "$BIN" run -d --memory 64m --cpus 0.5 --pids-limit 32 "$IMG" sleep 60 2>"$OUT.1.err")
rc=$?
leaf=$(leaves | head -1)
if [ -n "$leaf" ]; then
	say "leaf      $leaf: memory.max $(cat "$leaf/memory.max"), cpu.max $(cat "$leaf/cpu.max"), pids.max $(cat "$leaf/pids.max"), $(wc -l <"$leaf/cgroup.procs") process(es)"
fi
[ "$rc" -eq 0 ] && [ -n "$leaf" ] && [ "$(cat "$leaf/memory.max")" = 67108864 ] &&
	[ "$(cat "$leaf/cpu.max")" = "50000 100000" ] && [ "$(cat "$leaf/pids.max")" = 32 ] &&
	[ "$(wc -l <"$leaf/cgroup.procs")" -gt 0 ] || { bad "clause 1 (rc $rc)"; cat "$OUT.1.err" | tee -a "$OUT"; }
timeout 60 "$BIN" rm -f "$id" >/dev/null 2>&1

timeout 120 "$BIN" run --rm --pids-limit 16 "$IMG" sh -c 'i=0; while [ $i -lt 40 ]; do sleep 3 & i=$((i+1)); done; wait' >/dev/null 2>"$OUT.2.err"
rc=$?
say "pids      rc $rc: $(grep -c -i 'fork' "$OUT.2.err") fork line(s)"
grep -qi 'fork' "$OUT.2.err" || { bad "clause 2"; tail -3 "$OUT.2.err" | tee -a "$OUT"; }

# The payload names its cgroup on stderr first: run 43740331439 ended with 0
# and an empty stderr, with no word of where the payload ran.
timeout 120 "$BIN" run --rm --memory 32m --report "$OUT.3.json" "$IMG" sh -c 'echo "payload cgroup: $(cat /proc/self/cgroup)" >&2; x=$(head -c 200000000 /dev/zero | tr "\0" a); echo "payload held ${#x} bytes" >&2' >/dev/null 2>"$OUT.3.err"
rc=$?
say "memory    rc $rc: $(grep -o 'oom_kill [0-9]*' "$OUT.3.err" | head -1)"
[ "$rc" -eq 137 ] && grep -q 'OOM killer' "$OUT.3.err" || { bad "clause 3"; tail -5 "$OUT.3.err" | tee -a "$OUT"; cat /proc/swaps | tee -a "$OUT"; }

HOLD='x=$(head -c 200000000 /dev/zero | tr "\0" a); echo "${#x}"'
timeout 120 "$BIN" run -d --name oomd --memory 32m --report "$OUT.5.json" "$IMG" sh -c "$HOLD" >/dev/null 2>"$OUT.5.err"
timeout 120 "$BIN" wait oomd >/dev/null 2>&1
sleep 1
fg=$(sed -n 's/.*"oom_kills": *\([0-9]*\).*/\1/p' "$OUT.3.json" 2>/dev/null)
bg=$(sed -n 's/.*"oom_kills": *\([0-9]*\).*/\1/p' "$OUT.5.json" 2>/dev/null)
say "oom       report run: ${fg:--}, report run -d: ${bg:--}"
[ "${fg:-0}" -gt 0 ] && [ "${bg:-0}" -gt 0 ] || { bad "clause 5"; cat "$OUT.3.json" "$OUT.5.json" "$OUT.5.err" 2>/dev/null | tail -40 | tee -a "$OUT"; }
timeout 60 "$BIN" rm -f oomd >/dev/null 2>&1
timeout 120 "$BIN" run --rm --pids-limit 64 "$IMG" true >/dev/null 2>&1
sleep 2
left=$(leaves | wc -l)
say "leaves    $left left"
[ "$left" -eq 0 ] || { bad "clause 4: $(leaves)"; for l in $(leaves); do say "  $l: $(cat "$l/cgroup.procs" | tr '\n' ' ')"; done; }
rm -f "$OUT".*.err "$OUT".*.json
[ "$fail" -eq 0 ] && say "PASS" || say "FAIL"
exit "$fail"
