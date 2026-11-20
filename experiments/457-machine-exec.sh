#!/bin/sh
# Question: does `run -d --podvm-tier=machine` keep a guest for `exec`,
# and does `exec` into it give the output and the exit code of each
# command, fast, with no emulator after `stop` and `rm`?
#
# TODO/machine.md T-1750. The guest is mirror.gcr.io/library/alpine:3.20
# under TCG (KVM where /dev/kvm opens), with the pinned kernel of the
# machine tier. One guest, bounded by --timeout 600.
#
# Pinned inputs:
#   podvm   PODVM_457_BIN (default the release build of the workspace)
#   store   a fresh HOME and PODVM_STORE under .work/exp457
#
# Clauses:
#   1. `run -d --name w1 --podvm-tier=machine` exits 0 and prints its id
#      of 64 hex digits (TODO/machine.md T-2042);
#      `ps` lists w1 as Up;
#   2. `exec w1 uname -r` prints the release of the guest kernel, exit 0;
#   3. `exec -e K=v -w /tmp w1 sh -c 'echo $K $(pwd); echo err >&2; exit 7'`
#      gives `v /tmp` on stdout, `err` on stderr and exit 7;
#   4. `exec w1 /no/such` exits 127;
#   5. ten execs take less than 10 s together (a warm guest: no boot);
#   6. `stop w1` and `rm w1` exit 0, and no emulator stays.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_457_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
WORK="$ROOT/.work/exp457"
OUT="$ROOT/experiments/results/machine-exec.txt"
IMG=mirror.gcr.io/library/alpine:3.20
rm -rf "$WORK"
mkdir -p "$WORK/home"
export HOME="$WORK/home" PODVM_STORE="$WORK/store"
fail=0
: >"$OUT"
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }

say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "podvm     $("$BIN" version 2>/dev/null)"
say "accel     $([ -r /dev/kvm ] && [ -w /dev/kvm ] && echo kvm || echo tcg)"
timeout 300 "$BIN" pull -q "$IMG" >/dev/null 2>&1

t0=$(date +%s)
out=$(timeout 300 "$BIN" run -d --name w1 --timeout 600 --podvm-tier=machine "$IMG" sh -c 'echo started; sleep 600' 2>"$WORK/run.err")
rc=$?
say "run -d    rc $rc in $(( $(date +%s) - t0 )) s: $out; $("$BIN" ps 2>&1 | grep w1)"
[ "$rc" -eq 0 ] && printf '%s' "$out" | grep -qxE '[0-9a-f]{64}' && "$BIN" ps 2>&1 | grep -q "w1.*Up" || { bad "clause 1"; cat "$WORK/run.err" | tee -a "$OUT"; }

out=$(timeout 120 "$BIN" exec w1 uname -r 2>&1); rc=$?
say "uname -r  rc $rc: $out"
[ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q -- "-virt" || bad "clause 2"

timeout 120 "$BIN" exec -e K=v -w /tmp w1 sh -c 'echo $K $(pwd); echo err >&2; exit 7' >"$WORK/3.out" 2>"$WORK/3.err"
rc=$?
say "streams   rc $rc: stdout [$(cat "$WORK/3.out")] stderr [$(cat "$WORK/3.err")]"
[ "$rc" -eq 7 ] && [ "$(cat "$WORK/3.out")" = "v /tmp" ] && grep -qx err "$WORK/3.err" || bad "clause 3"

timeout 120 "$BIN" exec w1 /no/such >/dev/null 2>&1; rc=$?
say "missing   rc $rc"
[ "$rc" -eq 127 ] || bad "clause 4"

t0=$(date +%s%N)
i=0
while [ $i -lt 10 ]; do timeout 60 "$BIN" exec w1 true || bad "clause 5: an exec failed"; i=$((i + 1)); done
ms=$(( ($(date +%s%N) - t0) / 1000000 ))
say "ten execs $ms ms"
[ "$ms" -lt 10000 ] || bad "clause 5"

timeout 60 "$BIN" stop w1 >/dev/null 2>&1; s=$?
timeout 60 "$BIN" rm w1 >/dev/null 2>&1; r=$?
say "stop rm   rc $s $r"
[ "$s" -eq 0 ] && [ "$r" -eq 0 ] || bad "clause 6"
if pgrep -f "qemu-system-x86_64.*$WORK" >/dev/null; then
	bad "clause 6: an emulator stayed"
	pkill -9 -f "qemu-system-x86_64.*$WORK"
fi
rm -rf "$WORK"
[ "$fail" -eq 0 ] && say "PASS" || say "FAIL"
exit "$fail"
