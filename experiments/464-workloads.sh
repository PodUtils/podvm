#!/bin/sh
# Question: do real tools of a CI job run in a detached podvm unit on each
# mechanism that a host gives: a raw `syscall` instruction of a dynamic
# program (Rust's `rustix`), a spawn of a child (`posix_spawn`, Python's
# `subprocess`), a pty, `uv` and `ruff`?
#
# TODO/gate.md T-1936. The gate and the integration tests start payloads;
# they do not run the tools of a job. On 2026-11-17 such tools found four
# faults that each check passed (T-1934, T-1935): no pty on the namespace
# rung, EACCES of a raw `renameat2` on the loader family, and two crashes
# of the syscall tier. This experiment runs them as a user with no root.
#
# Pinned inputs:
#   podvm   PODVM_464_BIN (default the release build of the workspace)
#   image   PODVM_464_IMAGE (default public.ecr.aws/docker/library/python:3.13-slim)
#   modes   PODVM_464_MODES (default "default syscall")
#   extra   PODVM_464_RUN_ARGS: more arguments of `run -d` (a proxy and its
#           CA on a host whose egress needs them; none on a hosted runner)
#
# Clauses, for each mode (no flag, and `--podvm-tier=syscall`):
#   1. `run -d` starts the unit, and `inspect` gives its rung and tier;
#   2. a glibc program with raw `renameat2(RENAME_NOREPLACE)`, `rename` and
#      `link` in `/root` gets 0 for each (judged on the namespace rung and
#      on the syscall tier; measured on the loader family, T-1935);
#   3. Python's `subprocess.run(["uname"])` gives 0;
#   4. `uv venv`, `uv pip install six` and an import of six pass (judged
#      as clause 2: `uv` persists its lock file with a raw `renameat2`);
#   5. `ruff check` writes its cache (judged as clause 2);
#   6. `pty.spawn(["true"])` passes (judged on the userland rung; measured
#      on the namespace rung, T-1934, and on the tier);
#   7. `rm -f` removes the unit, and no process of it stays.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_464_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
IMG="${PODVM_464_IMAGE:-public.ecr.aws/docker/library/python:3.13-slim}"
MODES="${PODVM_464_MODES:-default syscall}"
EXTRA="${PODVM_464_RUN_ARGS:-}"
OUT="$ROOT/experiments/results/workloads.txt"
W=$(mktemp -d)
fail=0
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }
# Judge a clause where `judged` is 1, else only measure it.
check() { # clause judged rc what
	if [ "$3" -eq 0 ]; then say "  $1 pass: $4"
	elif [ "$2" -eq 1 ]; then bad "$1: $4 (rc $3)"
	else say "  $1 measured, not judged: $4 (rc $3)"; fi
}

: >"$OUT"
say "date     $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "podvm    $("$BIN" version 2>/dev/null)"
say "user     $(id -u), unshare -Urm: $(unshare -Urm true 2>/dev/null && echo yes || echo no)"

cat >"$W/raw.c" <<'EOF'
#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>
static long raw(long n, long a, long b, long c, long d, long e) {
	long r; register long r10 __asm__("r10") = d; register long r8 __asm__("r8") = e;
	__asm__ volatile("syscall" : "=a"(r) : "a"(n), "D"(a), "S"(b), "d"(c), "r"(r10), "r"(r8) : "rcx", "r11", "memory");
	return r;
}
int main(void) {
	unlink("/root/raw-a"); unlink("/root/raw-b"); unlink("/root/raw-c"); unlink("/root/raw-l");
	close(open("/root/raw-a", O_CREAT | O_WRONLY, 0644));
	long a = raw(316, AT_FDCWD, (long)"/root/raw-a", AT_FDCWD, (long)"/root/raw-b", 1);
	long b = raw(82, (long)"/root/raw-b", (long)"/root/raw-c", 0, 0, 0);
	long c = raw(86, (long)"/root/raw-c", (long)"/root/raw-l", 0, 0, 0);
	printf("renameat2 %ld rename %ld link %ld\n", a, b, c);
	return a || b || c;
}
EOF
cc -O1 -o "$W/raw" "$W/raw.c" || { say "FAIL: cc"; exit 1; }
chmod -R a+rX "$W"
"$BIN" pull -q "$IMG" >/dev/null 2>&1 || "$BIN" pull "$IMG" 2>&1 | tail -2 | tee -a "$OUT"

for mode in $MODES; do
	u="pv464-$mode"
	flag=""
	[ "$mode" = syscall ] && flag="--podvm-tier=syscall"
	say "== mode $mode"
	"$BIN" rm -f "$u" >/dev/null 2>&1
	# shellcheck disable=SC2086
	if ! timeout 300 "$BIN" run -d $flag $EXTRA --name "$u" -v "$W:/w:ro" "$IMG" sleep 1800 >/dev/null 2>"$W/run.err"; then
		bad "clause 1 ($mode): run -d: $(tail -3 "$W/run.err")"
		continue
	fi
	ri=$("$BIN" inspect "$u" --format '{{.Rung}} {{.Tier}}' 2>&1)
	say "  clause 1 pass: rung and tier: $ri"
	rung=${ri%% *}
	# The judgement of each clause on this mechanism.
	j_raw=1; j_pty=0
	if [ "$mode" = default ] && [ "$rung" = userland ]; then j_raw=0; j_pty=1; fi
	x() { timeout 600 "$BIN" exec "$u" sh -c "$1" >"$W/x.out" 2>&1; }
	x '/w/raw'; check "clause 2 ($mode)" "$j_raw" $? "$(tail -1 "$W/x.out")"
	x 'python3 -c "import subprocess,sys; sys.exit(subprocess.run([\"uname\",\"-r\"]).returncode)"'
	check "clause 3 ($mode)" 1 $? "$(tail -1 "$W/x.out")"
	x 'pip install -q --root-user-action=ignore uv ruff 2>&1 | grep -v notice; cd /tmp && uv venv -q v && uv pip install -q --python v/bin/python six && v/bin/python -c "import six; print(six.__version__)"'
	check "clause 4 ($mode)" "$j_raw" $? "$(tail -1 "$W/x.out")"
	x 'mkdir -p /root/p && cd /root/p && printf "import os\nx = 1\n" > a.py && { ruff check . >/dev/null 2>/root/p/err; ls .ruff_cache | grep -q "^[0-9]" && ! grep -q "os error" /root/p/err; }; s=$?; tail -2 /root/p/err; exit $s'
	check "clause 5 ($mode)" "$j_raw" $? "$(tail -1 "$W/x.out")"
	x 'python3 -c "import pty; pty.spawn([\"true\"]); print(\"pty ok\")"'
	check "clause 6 ($mode)" "$j_pty" $? "$(tail -1 "$W/x.out")"
	timeout 120 "$BIN" rm -f "$u" >/dev/null 2>&1
	sleep 1
	left=$(pgrep -fc "sleep 1800" || true)
	[ "${left:-0}" -eq 0 ] && say "  clause 7 pass: no process stays" || bad "clause 7 ($mode): $left process(es) stay"
done
rm -rf "$W"
[ "$fail" -eq 0 ] && say "PASS" || say "FAIL"
exit "$fail"
