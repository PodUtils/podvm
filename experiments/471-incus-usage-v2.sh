#!/bin/sh
# Question: on a host with cgroup v2 and the system incusd of apt (a hosted
# GitHub runner, ubuntu-24.04, AppArmor on), does `inspect` of an instance
# of `run -d --podvm-tier=incus` whose command ended by itself give the
# record of the stop (the CPU time, the peak memory and the OOM kills),
# also for an OOM kill of a child of the command?
#
# TODO/machine.md T-2086 (PodUtils/podrunner#350). The cloud host of podvm
# has cgroup v1 and an incusd with no AppArmor (experiment 434, route A),
# where clause 26 of experiment 434 gives each value.
#
# Pinned inputs:
#   podvm   PODVM_471_BIN (default the release build of the workspace)
#   incusd  the system daemon at PODVM_INCUS_SOCKET (default
#           /var/lib/incus/unix.socket), which the caller can open; the
#           job `incus` of .github/workflows/kvm.yml installs incus of apt
#   OCI     alpine 3.20 by digest (experiment 434); PODVM_471_MIRROR=1
#           takes the same digest from mirror.gcr.io
#
# Clauses (each value from the one `inspect` that also gives Stopped):
#   1. a command that holds 64 MiB and burns about 3 s of CPU in each of
#      four processes, then ends with 0: `CpuSeconds` no less than the CPU
#      time that the shell of the command counted (its own and its reaped
#      children's, /proc/$$/stat) and at most 1.5 s more,
#      `PeakMemoryBytes` from 64 MiB to 256 MiB, `OomKills` 0, exit 0.
#   2. under `--memory 256m`, a command whose child `dd` asks for 1 GiB,
#      which the OOM killer takes, then ends with 0 (podrunner's payload):
#      `OomKills` 1 or more, `PeakMemoryBytes` 128 MiB to 256 MiB, exit 0.
#   3. after each run, the file /.podvm/usage of the instance, read with
#      the client of incus (an observer that is not podvm), gives the same
#      values as `inspect`.
# Recorded, not a clause: the values of a running instance (incusd's
# state and metrics), the reasons of `inspect --verbose`, and the
# AppArmor denials of the kernel log where `sudo -n dmesg` reads it.
#
# Exit: 0 each clause matched, 1 a clause disagreed or an instance stays,
# 2 the experiment could not run.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_471_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
OUT="$ROOT/experiments/results/incus-usage-v2.txt"
OCI='public.ecr.aws/docker/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
[ "${PODVM_471_MIRROR:-0}" = 1 ] && OCI='mirror.gcr.io/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
export PODVM_INCUS_SOCKET="${PODVM_INCUS_SOCKET:-/var/lib/incus/unix.socket}"
[ -x "$BIN" ] || { echo "SKIP: no podvm at $BIN" >&2; exit 2; }
[ -S "$PODVM_INCUS_SOCKET" ] && [ -w "$PODVM_INCUS_SOCKET" ] ||
	{ echo "SKIP: no incusd socket that this user can open at $PODVM_INCUS_SOCKET" >&2; exit 2; }
command -v incus >/dev/null 2>&1 || { echo "SKIP: no incus client (the observer)" >&2; exit 2; }
WORK=$(mktemp -d "${TMPDIR:-/var/tmp}/podvm-471.XXXXXX") || exit 2
export PODVM_STORE="$WORK/store"
P() { timeout 300 "$BIN" "$@"; }
RI() { INCUS_SOCKET="$PODVM_INCUS_SOCKET" timeout 120 incus "$@"; }
NAMES="use471 oom471 run471"
end() { for n in $NAMES; do P rm -f --podvm-tier=incus "$n" >/dev/null 2>&1; done; rm -rf "$WORK"; }
trap end EXIT
fail=0
mkdir -p "$(dirname "$OUT")"
: >"$OUT"
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }

say "== conditions"
say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "kernel    $(uname -sr)"
say "cgroup    $(stat -fc %T /sys/fs/cgroup)"
say "apparmor  $(cat /sys/module/apparmor/parameters/enabled 2>/dev/null || echo -)"
say "incus     $(RI version 2>/dev/null | tr '\n' ' ')"
say "podvm     $(P version 2>/dev/null | head -1)"
say "cpus      $(nproc)"
say "OCI       $OCI"
P pull "$OCI" >"$WORK/pull.log" 2>&1 || { say "pull FAILED: COULD NOT RUN"; cat "$WORK/pull.log" >>"$OUT"; exit 2; }

F='{{.Status}} {{.CpuSeconds}} {{.PeakMemoryBytes}} {{.OomKills}} {{.ExitCode}}'
# The values of the first `inspect` that gives Stopped, and its reasons.
stopped() {
	i=0
	while [ $i -lt 120 ]; do
		s=$(P inspect --podvm-tier=incus --verbose --format "$F" "$1" 2>"$WORK/why.$1")
		case "$s" in Stopped\ *) break ;; esac
		i=$((i + 1)); sleep 1
	done
	printf '%s' "$s" | sed 's/^Stopped //'
}
# The file of the record, read by the client of incus.
record() { RI file pull "$1/.podvm/usage" - 2>&1 | tr '\n' ' '; }

say ""
say "== a running instance (recorded)"
P run -d -i --name run471 --podvm-tier=incus --podvm-net none "$OCI" \
	sh -c 'x=$(head -c 33554432 /dev/zero | tr "\0" a); sleep 20; echo ${#x}' </dev/null >/dev/null 2>"$WORK/run.err" ||
	{ say "run -d FAILED: COULD NOT RUN"; cat "$WORK/run.err" >>"$OUT"; exit 2; }
sleep 8
say "running   $(P inspect --podvm-tier=incus --format "$F" run471 2>&1 | tr '\n' ' ')"
P rm -f --podvm-tier=incus run471 >/dev/null 2>&1

say ""
say "== 1. a load that ends by itself"
P run -d -i --name use471 --podvm-tier=incus --podvm-net none "$OCI" sh -c '
x=$(head -c 67108864 /dev/zero | tr "\0" a); echo "held ${#x}"
for i in 1 2 3 4; do timeout 3 sh -c "while :; do :; done" & done; wait
set -- $(cut -d" " -f14-17 /proc/$$/stat); echo "cpu $(( $1 + $2 + $3 + $4 ))"; exit 0' \
	</dev/null >/dev/null 2>"$WORK/use.err" || { bad "run -d of the load"; cat "$WORK/use.err" >>"$OUT"; }
u=$(stopped use471)
say "truth     $(P logs --podvm-tier=incus use471 2>&1 | tr '\n' ' ')"
say "usage     $u"
sed 's/^/why       /' "$WORK/why.use471" | tee -a "$OUT"
say "record    $(record use471)"

say ""
say "== 2. an OOM kill of a child under --memory 256m"
P run -d -i --name oom471 --memory 256m --podvm-tier=incus --podvm-net none "$OCI" sh -c '
dd if=/dev/zero of=/dev/null bs=1M count=1 2>/dev/null || exit 10
if dd if=/dev/zero of=/dev/null bs=1024M count=1 2>/dev/null; then exit 11; fi; exit 0' \
	</dev/null >/dev/null 2>"$WORK/oom.err" || { bad "run -d of the OOM payload"; cat "$WORK/oom.err" >>"$OUT"; }
o=$(stopped oom471)
say "usage     $o"
sed 's/^/why       /' "$WORK/why.oom471" | tee -a "$OUT"
say "record    $(record oom471)"

say ""
say "== the kernel log (recorded)"
if sudo -n true 2>/dev/null; then
	sudo -n dmesg 2>/dev/null | grep -E 'apparmor="DENIED"|oom-kill|Memory cgroup out of memory' | tail -20 | sed 's/^/kernel    /' | tee -a "$OUT"
else
	say "kernel    - (no sudo)"
fi

for n in $NAMES; do P rm -f --podvm-tier=incus "$n" >/dev/null 2>&1; done
left=$(RI list --format csv -c n 2>/dev/null | grep -c 471)
say "stays     $left"

# ---- the clauses -----------------------------------------------------------
echo "$u" | grep -Eq '^[0-9]+\.[0-9]+ [0-9]+ 0 0$' || bad "1: inspect of the stopped instance gave no CPU time, no peak, no 0 OOM kills or no exit 0: $u"
grep -Eq '^truth     .*held 67108864 cpu [0-9]+' "$OUT" || bad "1: the command did not hold 64 MiB or give its CPU time"
awk '/^truth/ {for (i = 1; i <= NF; i++) if ($i == "cpu") t = $(i+1) / 100}
     /^usage/ && !c {c = $2; p = $3}
     END {h = 67108864; ok = (c != "" && t >= 2.0 && c >= t - 0.02 && c <= t + 1.5 && p >= h && p <= 4 * h)
          printf "compare   cpu %s of %s s, peak %s of %s to %s bytes: %s\n", c, t, p, h, 4 * h, (ok ? "close" : "FAR"); exit !ok}' \
	"$OUT" >"$WORK/cmp" || fail=1
cat "$WORK/cmp" | tee -a "$OUT"
echo "$o" | grep -Eq '^[0-9]+\.[0-9]+ [0-9]+ [1-9][0-9]* 0$' || bad "2: no OOM kill counted for the child, or no exit 0: $o"
echo "$o" | awk '{exit !($2 >= 134217728 && $2 <= 268435456)}' || bad "2: the peak is not from 128 MiB to 256 MiB: $o"
for n in use471 oom471; do
	line=$(grep "^record" "$OUT" | sed -n "$([ $n = use471 ] && echo 1 || echo 2)p")
	v=$(grep "^usage" "$OUT" | sed -n "$([ $n = use471 ] && echo 1 || echo 2)p" | awk '{print $3, $4}')
	pk=$(echo "$v" | cut -d' ' -f1); ok=$(echo "$v" | cut -d' ' -f2)
	echo "$line" | grep -q "\"peak_bytes\":$pk" && echo "$line" | grep -q "\"oom_kills\":$ok" ||
		bad "3: the record of $n, read by incus, does not give the values of inspect ($v): $line"
done
[ "$left" = 0 ] || bad "an instance stays"
[ "$fail" = 0 ] && say "THE RECORD OF THE STOP HOLDS ON CGROUP V2" || say "A CLAUSE DISAGREED"
exit "$fail"
