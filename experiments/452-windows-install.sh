#!/bin/sh
# Question: does `podvm pull windows:NAME` install a setup ISO of the
# catalog into a golden image under KVM, how long does it take, is the
# golden image standalone, is the ISO unchanged, and does no emulator
# stay?
#
# TODO/machine.md T-1867. `pull` fetches and checks the ISO, boots it with
# the answer stick on a blank 64 GiB disk, sends Enter for the prompt of
# the firmware, watches the phases, and converts the disk into
# `windows-NAME.golden.qcow2` in the store. One guest runs at a time, with
# the bound of `timeout`.
#
# Pinned inputs:
#   podvm   PODVM_452_BIN (default the release build of the workspace)
#   names   PODVM_452_NAMES (default "2025")
#   store   HOME (the caller gives a disk with room: about 60 GB)
#
# Clauses, for each name:
#   1. `podvm pull windows:NAME` exits 0 and prints the golden image;
#   2. `qemu-img info` names no backing file of the golden image;
#   3. the sha256 of the ISO in the store is the pin of the catalog;
#   4. no emulator stays;
#   5. `podvm run windows:NAME ver` exits 0 and prints the version of
#      Windows; no work directory of the run stays in the store (T-1932),
#      and no machine of the run stays (T-1933: the command goes through
#      the guest agent of a machine). The same run through the mailbox
#      agent (`PODVM_WINDOWS_MAILBOX=1`) is measured, not judged.
#   7. TODO/machine.md T-1868: a detached machine of the golden image
#      (`run -d`) answers `exec ver` (rc 0, the version), `exec exit 7`
#      gives 7, a `cp` in and out keeps the sha256 of 3 MB, and `stop` ends
#      it by `guest-shutdown` of the QEMU Guest Agent.
# With PODVM_452_REFUSAL=1, the clause of a host with TCG only:
#   6. `podvm pull windows:2025` exits 125 at once, names `--accel tcg`,
#      and fetches nothing.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_452_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
NAMES="${PODVM_452_NAMES:-2025}"
OUT="$ROOT/experiments/results/windows-install.txt"
STORE="${PODVM_STORE:-${XDG_DATA_HOME:-$HOME/.local/share}/podvm}/windows/catalog"
fail=0
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }

if [ "${PODVM_452_REFUSAL:-0}" = 1 ]; then
	say "refusal  $(date -u +%Y-%m-%dT%H:%M:%SZ), /dev/kvm: $(ls -l /dev/kvm 2>&1)"
	t0=$(date +%s)
	err=$(env -u PODVM_MACHINE_ACCEL timeout 120 "$BIN" pull windows:2025 2>&1 >/dev/null)
	rc=$?
	say "refusal  rc $rc in $(( $(date +%s) - t0 )) s: $err"
	[ "$rc" -eq 125 ] && printf '%s' "$err" | grep -q -- "--accel tcg" || bad "clause 6: no refusal that names --accel tcg"
	[ -e "$STORE/win2025-eval.iso" ] && bad "clause 6: the refusal fetched the ISO"
	exit "$fail"
fi

: >"$OUT"
say "date     $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "podvm    $("$BIN" version 2>/dev/null)"
say "accel    $([ -r /dev/kvm ] && [ -w /dev/kvm ] && echo kvm || echo tcg)"
say "disk     $(df -h "$HOME" | tail -1)"

for name in $NAMES; do
	t0=$(date +%s)
	golden=$(timeout 170m "$BIN" pull "windows:$name" 2>"$ROOT/experiments/results/windows-install-$name.err")
	rc=$?
	secs=$(( $(date +%s) - t0 ))
	say "$name     pull rc $rc in ${secs} s ($(( secs / 60 )) min): $golden"
	tail -25 "$ROOT/experiments/results/windows-install-$name.err" | tee -a "$OUT"
	if [ "$rc" -ne 0 ] || [ ! -f "$golden" ]; then
		bad "clause 1 ($name)"
	else
		info=$(qemu-img info "$golden" 2>&1)
		say "$name     $(printf '%s\n' "$info" | grep -E '^(virtual size|disk size)' | tr '\n' ' ')"
		printf '%s' "$info" | grep -qi "backing file" && bad "clause 2 ($name): the golden image names a backing file"
	fi
	# The ISO of this name by its file in the catalog: the store also holds
	# the virtio-win CD of TODO/machine.md T-1868.
	entry=$(grep -A12 "name: \"$name\"" "$ROOT/crates/podvm-windows/src/catalog.rs")
	file=$(printf '%s\n' "$entry" | sed -n 's/.*file: "\([^"]*\)".*/\1/p' | head -1)
	pin=$(printf '%s\n' "$entry" | sed -n 's/.*sha256: "\([0-9a-f]*\)".*/\1/p' | head -1)
	iso="$STORE/$file"
	if [ -n "$file" ] && [ -f "$iso" ]; then
		got=$(sha256sum "$iso" | cut -d' ' -f1)
		say "$name     iso sha256 $got"
		[ "$got" = "$pin" ] || bad "clause 3 ($name): the ISO changed (pin $pin)"
	fi
	if [ "$rc" -eq 0 ]; then
		t0=$(date +%s)
		ver=$(timeout 15m "$BIN" run "windows:$name" ver 2>"$ROOT/experiments/results/windows-install-$name-run.err")
		vrc=$?
		say "$name     run ver rc $vrc in $(( $(date +%s) - t0 )) s: $(printf '%s' "$ver" | tr -d '\r' | grep -i version | head -1)"
		[ "$vrc" -eq 0 ] && printf '%s' "$ver" | grep -qi "version" || { bad "clause 5 ($name)"; tail -5 "$ROOT/experiments/results/windows-install-$name-run.err" | tee -a "$OUT"; }
		# TODO/machine.md T-1932: the overlay of the run went into the store,
		# and its work directory is gone after the run.
		left=$(ls -d "$STORE"/podvm-windows-*-run 2>/dev/null | wc -l)
		say "$name     work directories of a run left in the store: $left"
		[ "$left" -eq 0 ] || bad "clause 5 ($name): a work directory of the run stayed"
		machines=$("$BIN" ps -a 2>/dev/null | grep -c ' win-')
		say "$name     machines of the run left: $machines"
		[ "$machines" -eq 0 ] || bad "clause 5 ($name): a machine of the run stayed"
		t0=$(date +%s)
		mver=$(PODVM_WINDOWS_MAILBOX=1 timeout 12m "$BIN" run "windows:$name" ver 2>"$ROOT/experiments/results/windows-install-$name-mailbox.err")
		mrc=$?
		say "$name     (measured) run ver through the mailbox rc $mrc in $(( $(date +%s) - t0 )) s: $(printf '%s' "$mver" | tr -d '\r' | grep -i version | head -1)"
	fi
	if [ "$rc" -eq 0 ]; then
		m="pv452-$name"
		E="$ROOT/experiments/results/windows-install-$name-machine.err"
		: >"$E"
		t0=$(date +%s)
		if timeout 15m "$BIN" run -d --name "$m" "windows:$name" >>"$E" 2>&1; then
			v=$(timeout 10m "$BIN" exec "$m" ver 2>>"$E")
			erc=$?
			timeout 5m "$BIN" exec "$m" exit 7 >>"$E" 2>&1
			e7=$?
			in="${TMPDIR:-/tmp}/pv452-$name.in"
			out="${TMPDIR:-/tmp}/pv452-$name.out"
			head -c 3000000 /dev/urandom >"$in"
			timeout 5m "$BIN" cp "$in" "$m:C:\\Users\\Public\\" >>"$E" 2>&1
			crc1=$?
			timeout 5m "$BIN" cp "$m:C:\\Users\\Public\\pv452-$name.in" "$out" >>"$E" 2>&1
			crc2=$?
			same=no
			[ -f "$out" ] && [ "$(sha256sum <"$in")" = "$(sha256sum <"$out")" ] && same=yes
			timeout 5m "$BIN" stop "$m" >>"$E" 2>&1
			src=$?
			ended=$(find "$HOME" "${XDG_RUNTIME_DIR:-/tmp}" -name machine.json -path "*$m*" 2>/dev/null | head -1 | xargs -r grep -o '"ended":"[^"]*"' | head -1)
			say "$name     machine: exec ver rc $erc ($(printf '%s' "$v" | tr -d '\r' | grep -i version | head -1)), exec exit 7 rc $e7, cp rc $crc1/$crc2 same $same, stop rc $src $ended, in $(( $(date +%s) - t0 )) s"
			[ "$erc" -eq 0 ] && [ "$e7" -eq 7 ] && [ "$same" = yes ] && [ "$src" -eq 0 ] || bad "clause 7 ($name)"
		else
			bad "clause 7 ($name): run -d"
		fi
		"$BIN" rm -f "$m" >>"$E" 2>&1 || true
		tail -8 "$E" | tee -a "$OUT"
	fi
	if pgrep -a qemu-system >/dev/null; then
		bad "clause 4 ($name): an emulator stayed"
		pkill -9 qemu-system
	fi
	# The next name needs the room of this one.
	rm -f "$STORE"/*.iso
	"$BIN" rmi "windows:$name" >/dev/null 2>&1 || true
done
[ "$fail" -eq 0 ] && say "PASS" || say "FAIL"
exit "$fail"
