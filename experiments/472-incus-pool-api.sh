#!/bin/sh
# Question: where podvm cannot read the free space of the storage pool of
# the default profile with statfs (a driver other than `dir`, or a pool
# that podvm does not see), does the incus tier take it from incusd
# (`GET /1.0/storage-pools/NAME/resources`), refuse by name a run that
# cannot fit, and give a true line after a failure?
#
# TODO/machine.md T-2078 remaining (1) (PodUtils/podrunner#346). Run as
# root: the experiment makes two pools, a loop device and a mount, and puts
# the root disk of the default profile back at the end.
#
# Pinned inputs:
#   podvm   PODVM_472_BIN (default the release build of the workspace)
#   incusd  the system daemon of apt at PODVM_INCUS_SOCKET (default
#           /var/lib/incus/unix.socket), 6.0.0 on 2026-11-25
#   btrfs   btrfs-progs of apt and a kernel with btrfs
#   OCI     alpine 3.20 by digest (experiment 434); PODVM_472_MIRROR=1
#           takes the same digest from mirror.gcr.io
#
# Clauses:
#   1. a pool `bt472` of the driver `btrfs` (200 MiB, a loop file of
#      incusd) that a file fills, and an image that incus does not have:
#      exit 125, and the line after the failure names the pool "(driver
#      `btrfs`, as incusd measures it)" and the volume of the image with
#      the bytes of the tree (not 0); no instance stays.
#   2. the same pool with the file removed: the run gives its output and
#      exit 0.
#   3. a pool `small472` of the driver `dir` on an ext4 of 8 MiB, with
#      `INCUS_DIR` of podvm at a directory where podvm sees no pool (as
#      route A of docs/incus.md), and an image that incus has: exit 125
#      before the create, one line that names the pool "(driver `dir`, as
#      incusd measures it)" and the need; no instance stays.
# Recorded, not a clause: the free space that incusd gives for the full
# btrfs pool (btrfs gives more than a write can use, so the check cannot
# refuse there: a lower bound refuses only what surely cannot fit).
#
# Exit: 0 each clause matched, 1 a clause disagreed or an instance stays,
# 2 the experiment could not run.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_472_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
OUT="$ROOT/experiments/results/incus-pool-api.txt"
OCI='public.ecr.aws/docker/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
[ "${PODVM_472_MIRROR:-0}" = 1 ] && OCI='mirror.gcr.io/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
export PODVM_INCUS_SOCKET="${PODVM_INCUS_SOCKET:-/var/lib/incus/unix.socket}"
[ -x "$BIN" ] || { echo "SKIP: no podvm at $BIN" >&2; exit 2; }
[ "$(id -u)" = 0 ] || { echo "SKIP: run as root (pools, a loop device, a mount)" >&2; exit 2; }
[ -S "$PODVM_INCUS_SOCKET" ] || { echo "SKIP: no incusd socket at $PODVM_INCUS_SOCKET" >&2; exit 2; }
command -v incus >/dev/null 2>&1 || { echo "SKIP: no incus client" >&2; exit 2; }
command -v mkfs.btrfs >/dev/null 2>&1 && grep -qw btrfs /proc/filesystems ||
	{ echo "SKIP: no btrfs (btrfs-progs and the kernel)" >&2; exit 2; }
WORK=$(mktemp -d "${TMPDIR:-/var/tmp}/podvm-472.XXXXXX") || exit 2
export PODVM_STORE="$WORK/store"
P() { timeout 300 "$BIN" "$@"; }
RI() { INCUS_SOCKET="$PODVM_INCUS_SOCKET" timeout 120 incus "$@"; }
API() { curl -qfsS --max-time 30 --unix-socket "$PODVM_INCUS_SOCKET" "http://incus$1"; }
OLD=$(RI profile device get default root pool 2>/dev/null)
INCUS_DIR_OF=$(dirname "$PODVM_INCUS_SOCKET")
FILLER="$INCUS_DIR_OF/storage-pools/bt472/filler"
end() {
	if [ -n "$OLD" ]; then RI profile device set default root pool="$OLD" >/dev/null 2>&1
	else RI profile device remove default root >/dev/null 2>&1; fi
	rm -f "$FILLER"
	for i in $(RI image list --format csv -c f 2>/dev/null); do
		RI image show "$i" 2>/dev/null | grep -q 'podvm' && RI image delete "$i" >/dev/null 2>&1
	done
	for v in $(RI storage volume list bt472 --format csv -c n 2>/dev/null); do
		RI storage volume delete bt472 "image/$v" >/dev/null 2>&1
	done
	RI storage delete bt472 >/dev/null 2>&1
	RI storage delete small472 >/dev/null 2>&1
	mountpoint -q "$WORK/small" && umount "$WORK/small"
	rm -rf "$WORK"
}
trap end EXIT
fail=0
mkdir -p "$(dirname "$OUT")"
: >"$OUT"
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }
left() { RI list --format csv -c n 2>/dev/null | grep -c .; }

say "== conditions"
say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "kernel    $(uname -sr)"
say "incus     $(RI version 2>/dev/null | tr '\n' ' ')"
say "podvm     $(P version 2>/dev/null | head -1)"
say "OCI       $OCI"
P pull -q "$OCI" >"$WORK/pull.log" 2>&1 || { say "pull FAILED: COULD NOT RUN"; cat "$WORK/pull.log" >>"$OUT"; exit 2; }
RI storage create bt472 btrfs size=200MiB >/dev/null 2>"$WORK/bt.err" ||
	{ say "the btrfs pool: COULD NOT RUN"; cat "$WORK/bt.err" >>"$OUT"; exit 2; }
if [ -n "$OLD" ]; then RI profile device set default root pool=bt472
else RI profile device add default root disk path=/ pool=bt472 >/dev/null; fi
before=$(left)
say "drivers   (with bt472) $(API /1.0 | sed -n 's/.*"storage":"\([^"]*\)".*/\1/p')"

say ""
say "== 1. a full btrfs pool, an image that incus does not have"
dd if=/dev/zero of="$FILLER" bs=1M count=400 status=none 2>/dev/null
sync
say "incusd    $(API /1.0/storage-pools/bt472/resources | sed -n 's/.*"metadata":\(.*\)}$/\1/p') (recorded)"
P run --rm --podvm-tier=incus --podvm-net none "$OCI" echo hello >"$WORK/1.out" 2>"$WORK/1.err"
e1=$?
sed 's/^/err       /; s/\xe2\x80\xa6/.../g' "$WORK/1.err" | cut -c1-400 | tee -a "$OUT"
say "exit      $e1"
[ "$e1" = 125 ] || bad "1: exit $e1, not 125"
grep -q "the storage pool \`bt472\` (driver \`btrfs\`, as incusd measures it)" "$WORK/1.err" ||
	bad "1: no line names the pool as incusd measures it"
grep -Eq 'the volume of the image \(the root of the instance is a clone of it\) needs at least [1-9][0-9.]* MiB' "$WORK/1.err" ||
	bad "1: the line does not give the bytes of the tree for the volume of the image"
[ "$(left)" = "$before" ] || bad "1: an instance stays"

say ""
say "== 2. the same pool with room"
rm -f "$FILLER"
sync
o2=$(P run --rm --podvm-tier=incus --podvm-net none "$OCI" echo hello 2>"$WORK/2.err")
e2=$?
say "out       $o2"
say "exit      $e2"
[ "$e2" = 0 ] && [ "$o2" = hello ] || { bad "2: exit $e2, output $o2"; cat "$WORK/2.err" >>"$OUT"; }

say ""
say "== 3. a dir pool of 8 MiB that podvm does not see, an image that incus has"
mkdir -p "$WORK/small" "$WORK/elsewhere"
truncate -s 8M "$WORK/small.ext4" && mkfs.ext4 -q "$WORK/small.ext4" && mount -o loop "$WORK/small.ext4" "$WORK/small" &&
	mkdir "$WORK/small/pool" && RI storage create small472 dir source="$WORK/small/pool" >/dev/null 2>"$WORK/small.err" ||
	{ say "the dir pool: COULD NOT RUN"; cat "$WORK/small.err" >>"$OUT"; exit 2; }
RI profile device set default root pool=small472
say "incusd    $(API /1.0/storage-pools/small472/resources | sed -n 's/.*"metadata":\(.*\)}$/\1/p') (recorded)"
INCUS_DIR="$WORK/elsewhere" P run --rm --podvm-tier=incus --podvm-net none "$OCI" echo hello >"$WORK/3.out" 2>"$WORK/3.err"
e3=$?
sed 's/^/err       /; s/\xe2\x80\xa6/.../g' "$WORK/3.err" | tee -a "$OUT"
say "exit      $e3"
[ "$e3" = 125 ] || bad "3: exit $e3, not 125"
grep -q "the storage pool \`small472\` (driver \`dir\`, as incusd measures it) at $WORK/small/pool has [0-9.]* MiB free and the root of the instance needs at least" "$WORK/3.err" ||
	bad "3: no refusal that names the pool and the need"
grep -q 'POST /1.0/instances' "$WORK/3.err" && bad "3: the create ran"
[ "$(left)" = "$before" ] || bad "3: an instance stays"

[ "$fail" = 0 ] && say "THE POOL OF INCUSD IS MEASURED THROUGH ITS API" || say "A CLAUSE DISAGREED"
exit "$fail"
