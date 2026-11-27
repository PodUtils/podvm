#!/bin/sh
# Question: does a detached DOS guest have a docker-shaped lifecycle:
# `run -d` makes it, `ps` shows it, `stop` ends it, `rm` removes it, the
# lifetime of `--timeout` ends it, and does no emulator stay, also when its
# keeper is killed?
#
# TODO/machine.md T-1870. The guest is windows:dos under TCG (or KVM where
# /dev/kvm opens). One guest runs at a time, each with a bound.
#
# Pinned inputs:
#   podvm   PODVM_454_BIN (default the release build of the workspace)
#   store   a fresh HOME and PODVM_STORE under .work/exp454
#
# Clauses:
#   1. `run -d --name m1 windows:dos` exits 0 and prints m1;
#   2. `ps` lists m1 as Up, and an emulator runs;
#   3. `rm m1` of a running machine exits 125 and removes nothing;
#   4. `stop m1` exits 0; `ps -a` lists m1 as Exited; no emulator runs;
#   5. `start m1` boots it again (Up), `logs -f m1` ends when `stop`
#      ends the machine, and then `rm m1` exits 0 and the directory of m1
#      is gone;
#   6. the planted fault: the keeper of m2 is killed with SIGKILL, so the
#      emulator has no owner; `rm -f m2` ends it and removes m2;
#   7. `run -d --timeout 5`: after 15 s no emulator runs, and `ps -a` names
#      the lifetime;
#   8. no emulator and no socket of a machine stays.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_454_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
WORK="$ROOT/.work/exp454"
OUT="$ROOT/experiments/results/windows-lifecycle.txt"
rm -rf "$WORK"
mkdir -p "$WORK/home"
export HOME="$WORK/home" PODVM_STORE="$WORK/store"
MACH="$PODVM_STORE/windows/machines"
fail=0
: >"$OUT"
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }
emu() { pgrep -f "qemu-system-x86_64.*$WORK" >/dev/null; }

say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "podvm     $("$BIN" version 2>/dev/null)"
timeout 600 "$BIN" pull windows:dos >/dev/null 2>"$WORK/pull.err" || { bad "the pull of windows:dos"; tail -3 "$WORK/pull.err" | tee -a "$OUT"; }

out=$(timeout 120 "$BIN" run -d --name m1 windows:dos 2>"$WORK/run.err"); rc=$?
say "run -d    rc $rc: $out"
[ "$rc" -eq 0 ] && [ "$out" = m1 ] || { bad "clause 1"; tail -3 "$WORK/run.err" | tee -a "$OUT"; }

ps=$("$BIN" ps 2>&1)
say "ps        $(printf '%s\n' "$ps" | grep m1)"
printf '%s\n' "$ps" | grep -q "m1.*Up" && emu || bad "clause 2"

"$BIN" rm m1 >/dev/null 2>&1; rc=$?
say "rm (runs) rc $rc"
[ "$rc" -eq 125 ] && [ -d "$MACH/m1" ] || bad "clause 3"

t0=$(date +%s)
timeout 60 "$BIN" stop m1 >/dev/null 2>&1; rc=$?
say "stop      rc $rc in $(( $(date +%s) - t0 )) s; $("$BIN" ps -a 2>&1 | grep m1)"
[ "$rc" -eq 0 ] && "$BIN" ps -a 2>&1 | grep -q "m1.*Exited" && ! emu || bad "clause 4"

"$BIN" start m1 >/dev/null 2>&1; rc=$?
say "start     rc $rc; $("$BIN" ps 2>&1 | grep m1)"
[ "$rc" -eq 0 ] && "$BIN" ps 2>&1 | grep -q "m1.*Up" && emu || bad "clause 5: start"
timeout 60 "$BIN" logs -f m1 >/dev/null 2>&1 &
lf=$!
sleep 1
timeout 60 "$BIN" stop m1 >/dev/null 2>&1
t0=$(date +%s)
wait "$lf"; lrc=$?
say "logs -f   rc $lrc, ended $(( $(date +%s) - t0 )) s after stop"
[ "$lrc" -eq 0 ] || bad "clause 5: logs -f did not end with the machine"
"$BIN" rm m1 >/dev/null 2>&1; rc=$?
say "rm        rc $rc"
[ "$rc" -eq 0 ] && [ ! -e "$MACH/m1" ] || bad "clause 5"

timeout 120 "$BIN" run -d --name m2 windows:dos >/dev/null 2>&1
keeper=$(sed -n 's/.*"keeper_pid":"\([0-9]*\)".*/\1/p' "$MACH/m2/machine.json" 2>/dev/null)
[ -n "$keeper" ] && kill -9 "$keeper"
sleep 1
emu && say "fault     the keeper $keeper is killed; the emulator runs with no owner" || bad "clause 6: no emulator after the kill of the keeper"
timeout 60 "$BIN" rm -f m2 >/dev/null 2>&1; rc=$?
say "rm -f     rc $rc"
[ "$rc" -eq 0 ] && ! emu && [ ! -e "$MACH/m2" ] || bad "clause 6"

name=$(timeout 120 "$BIN" run -d --timeout 5 windows:dos 2>/dev/null)
sleep 15
st=$("$BIN" ps -a 2>&1 | grep "$name")
say "lifetime  $st"
! emu && printf '%s' "$st" | grep -q "lifetime of 5 s" || bad "clause 7"
"$BIN" rm "$name" >/dev/null 2>&1

if emu || pgrep -a qemu-system >/dev/null; then
	bad "clause 8: an emulator stayed"
	pkill -9 -f "qemu-system-x86_64.*$WORK"
fi
ls "${XDG_RUNTIME_DIR:-/tmp}"/podvm-m-* >/dev/null 2>&1 && bad "clause 8: a socket of a machine stayed"
rm -rf "$WORK"
[ "$fail" -eq 0 ] && say "PASS" || say "FAIL"
exit "$fail"
