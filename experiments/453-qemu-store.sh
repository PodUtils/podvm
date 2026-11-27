#!/bin/sh
# Question: on a host with no QEMU, does `podvm run windows:dos ver` fetch
# the pinned static QEMU and its firmware into the store, boot FreeDOS
# with them, and does `podvm windows doctor` name the store copy?
#
# TODO/machine.md T-1875. A mount namespace (`unshare -Urm`) hides the
# QEMU of the host: /dev/null is bound over each program, and an empty
# tmpfs over each firmware directory. Outside the namespace, a host with
# QEMU keeps its own (the Decision of T-1875). One guest at a time, with a
# bound; no emulator may stay.
#
# Pinned inputs:
#   podvm   PODVM_453_BIN (default the release build of the workspace)
#   store   a fresh HOME under .work/exp453
#
# Clauses:
#   1. `podvm run windows:dos ver` exits 0 and prints the FreeCOM version;
#   2. each file of `podvm_windows::tools::TOOLS` is in the store, and the
#      sha256 of each program is printed;
#   3. `podvm windows doctor` names the emulator of the store;
#   4. outside the namespace, on a host with QEMU, the doctor names the
#      emulator of the host;
#   5. no emulator stays;
#   6. `podvm run --podvm-tier=machine` boots the Linux guest with the
#      store copy and prints the line of the payload.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_453_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
WORK="$ROOT/.work/exp453"
OUT="$ROOT/experiments/results/qemu-store.txt"
rm -rf "$WORK"
mkdir -p "$WORK/home"
export HOME="$WORK/home" PODVM_STORE="$WORK/store"
unset PODVM_QEMU PODVM_QEMU_IMG PODVM_QEMU_SHARE PODVM_OVMF_CODE PODVM_OVMF_VARS PODVM_TOOLS_DIR
fail=0
: >"$OUT"
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }
STORE="$PODVM_STORE/tools/qemu-11.1.3"

say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "podvm     $("$BIN" version 2>/dev/null)"

# The body that runs with no QEMU of the host.
cat >"$WORK/hidden.sh" <<'EOF'
for p in qemu-system-x86_64 qemu-img; do
	for d in $(echo "$PATH" | tr ':' ' '); do
		[ -f "$d/$p" ] && mount --bind /dev/null "$d/$p"
	done
done
for d in /usr/share/qemu /usr/share/OVMF /usr/share/edk2 /usr/share/edk2-ovmf /usr/share/seabios; do
	[ -d "$d" ] && mount -t tmpfs none "$d"
done
command -v qemu-system-x86_64 >/dev/null && [ -f "$(command -v qemu-system-x86_64)" ] && echo "HOST QEMU STILL SEEN"
"$@"
EOF

t0=$(date +%s)
out=$(timeout 900 unshare -Urm sh "$WORK/hidden.sh" "$BIN" run windows:dos ver 2>"$WORK/run.err")
rc=$?
say "run ver   rc $rc in $(( $(date +%s) - t0 )) s: $(printf '%s' "$out" | tr -d '\r' | grep -i version | head -1)"
grep "QEMU" "$WORK/run.err" | tee -a "$OUT"
[ "$rc" -eq 0 ] && printf '%s' "$out" | grep -qi "version" || { bad "clause 1"; tail -5 "$WORK/run.err" | tee -a "$OUT"; }
printf '%s' "$out" | grep -q "HOST QEMU STILL SEEN" && bad "the namespace did not hide the QEMU of the host"

n=$(find "$STORE" -type f | wc -l)
say "store     $n files in $STORE"
[ "$n" -eq 19 ] || bad "clause 2: the store has $n files, not 19"
for f in "$STORE/bin/qemu-system-x86_64" "$STORE/bin/qemu-img" "$STORE/share/qemu/edk2-x86_64-code.fd"; do
	say "sha256    $(sha256sum "$f" | cut -d' ' -f1)  ${f#"$STORE"/}"
done

doc=$(unshare -Urm sh "$WORK/hidden.sh" "$BIN" windows doctor 2>&1)
say "doctor    $(printf '%s\n' "$doc" | grep '^emulator:')"
printf '%s\n' "$doc" | grep '^emulator:' | grep -q "$STORE" || bad "clause 3: the doctor does not name the store copy"

lin=$(timeout 600 unshare -Urm sh "$WORK/hidden.sh" "$BIN" run --rm --podvm-tier=machine \
	mirror.gcr.io/library/alpine:3.20 sh -c 'echo guest-on-store; uname -r' 2>"$WORK/lin.err")
lrc=$?
say "linux     rc $lrc: $(printf '%s' "$lin" | tr '\n' ' ')"
[ "$lrc" -eq 0 ] && printf '%s' "$lin" | grep -q "guest-on-store" || { bad "clause 6"; tail -5 "$WORK/lin.err" | tee -a "$OUT"; }

if command -v qemu-system-x86_64 >/dev/null; then
	host=$("$BIN" windows doctor 2>&1 | grep '^emulator:')
	say "host      $host"
	printf '%s' "$host" | grep -q "$STORE" && bad "clause 4: the store copy won over the QEMU of the host"
else
	say "host      no QEMU on this host: clause 4 not run"
fi

if pgrep -a qemu-system >/dev/null; then
	bad "clause 5: an emulator stayed"
	pkill -9 qemu-system
fi
rm -rf "$WORK"
[ "$fail" -eq 0 ] && say "PASS" || say "FAIL"
exit "$fail"
