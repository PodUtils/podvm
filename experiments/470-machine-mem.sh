#!/bin/sh
# Question: what is the smallest memory with which a guest of the machine
# tier boots an image whose initrd is N bytes, for N of about 256, 512,
# 1024 and 2048 MiB? Is podvm's estimate (13/4 of the initrd and 512 MiB,
# `crates/podvm/src/machine/guest/mem.rs`) at or above each point?
#
# TODO/machine.md T-2047 (PodUtils/podrunner#334, COORD-219 #629). The rule
# of T-1922 comes from one image: an initrd of 1416 MiB resets the guest at
# 4G and boots at 5G (experiment 463, run 43876504275).
#
# Pinned inputs:
#   podvm   PODVM_470_BIN (default the release build of the workspace)
#   image   public.ecr.aws/docker/library/alpine:3.20 at its digest (412),
#           with files of random bytes added, which no compression shrinks;
#           PODVM_470_MIRROR=1 takes the same digest from mirror.gcr.io
#   sizes   PODVM_470_SIZES, in MiB (default "256 512 1024 2048")
#
# Method: for each size, one image; a search in steps of 256 MiB between a
# low bound that does not boot and a high bound that boots (each a run of
# `run --rm --podvm-tier=machine --podvm-mem MEM IMAGE true` under a
# timeout of 600 s). The high bound is checked first: a high bound that
# does not boot fails the experiment. Run it as the user of a GitHub
# runner with /dev/kvm 0666 (the job `mem` of .github/workflows/kvm.yml),
# or with PODVM_MACHINE_ACCEL=tcg and small sizes on a host with no KVM.
#
# Each row: the size asked, the size of the initrd, the largest memory
# that did not boot, the smallest that booted, podvm's estimate, and the
# count of runs.
#
# Exit: 0 each size has its point and the estimate is at or above it; 1 a
# high bound did not boot, the estimate is below a point, or an emulator
# stays; 2 the experiment could not run.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_470_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
OUT="$ROOT/experiments/results/machine-mem.txt"
IMG='public.ecr.aws/docker/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
[ "${PODVM_470_MIRROR:-0}" = 1 ] && IMG='mirror.gcr.io/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
SIZES="${PODVM_470_SIZES:-256 512 1024 2048}"
STEP=256
[ -x "$BIN" ] || { echo "SKIP: no podvm at $BIN" >&2; exit 2; }
if [ "${PODVM_MACHINE_ACCEL:-}" != tcg ] && { [ ! -r /dev/kvm ] || [ ! -w /dev/kvm ]; }; then
	echo "SKIP: no usable /dev/kvm (PODVM_MACHINE_ACCEL=tcg runs under TCG)" >&2
	exit 2
fi
WORK=$(mktemp -d "${TMPDIR:-/var/tmp}/podvm-470.XXXXXX") || exit 2
trap 'rm -rf "$WORK"' EXIT
export PODVM_STORE="$WORK/store"
# The memory of the initramfs root, which T-1994 keeps behind a variable.
export PODVM_MACHINE_ROOT=initramfs
P=$BIN
fail=0
mkdir -p "$(dirname "$OUT")"
: >"$OUT"
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }
up() { echo $((($1 + STEP - 1) / STEP * STEP)); }

say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "podvm     $($P version 2>/dev/null)"
say "qemu      $(qemu-system-x86_64 --version 2>/dev/null | head -1)"
say "host      $(uname -r), $(nproc) CPUs, $(awk '/MemTotal/{print int($2/1024)}' /proc/meminfo) MiB, accel ${PODVM_MACHINE_ACCEL:-kvm}"

# The rootfs of the image: its one layer, from the OCI layout of `save`.
timeout 600 "$P" pull "$IMG" >/dev/null 2>"$WORK/err" ||
	{ echo "SKIP: podvm cannot pull $IMG: $(tail -1 "$WORK/err")" >&2; exit 2; }
timeout 600 "$P" save -o "$WORK/oci.tar" "$IMG" 2>"$WORK/err" ||
	{ echo "SKIP: podvm cannot save $IMG: $(tail -1 "$WORK/err")" >&2; exit 2; }
mkdir -p "$WORK/oci" && tar -xf "$WORK/oci.tar" -C "$WORK/oci" && rm -f "$WORK/oci.tar"
layer=$(python3 -I - "$WORK/oci" <<'PYEOF'
import json, os, sys
d = sys.argv[1]
def blob(digest):
    return os.path.join(d, "blobs", *digest.split(":", 1))
index = json.load(open(os.path.join(d, "index.json")))
manifest = json.load(open(blob(index["manifests"][0]["digest"])))
layers = manifest.get("layers", [])
if len(layers) != 1:
    sys.exit("the image has %d layers, not 1" % len(layers))
print(blob(layers[0]["digest"]))
PYEOF
) || { echo "SKIP: no one layer in the OCI layout of $IMG" >&2; exit 2; }
gzip -dc "$layer" >"$WORK/base.tar" || { echo "SKIP: the layer of $IMG is not gzip" >&2; exit 2; }
rm -rf "$WORK/oci"

# boot MEM: 0 where the guest boots and the command exits 0.
runs=0
boot() {
	runs=$((runs + 1))
	timeout 600 "$P" run --rm --podvm-tier=machine --podvm-mem "${1}M" "img:470" true \
		>/dev/null 2>"$WORK/err" </dev/null
	rc=$?
	say "$(printf '  %5s MiB  mem %5s MiB  rc %s' "$n" "$1" "$rc")"
	[ "$rc" -eq 0 ]
}

say "$(printf '%-6s %-9s %-14s %-14s %-10s %s' size initrd 'no boot at' 'boots at' estimate runs)"
for n in $SIZES; do
	cp "$WORK/base.tar" "$WORK/img.tar"
	mkdir -p "$WORK/add"
	head -c $((n * 1048576)) /dev/urandom >"$WORK/add/blob"
	tar -rf "$WORK/img.tar" -C "$WORK/add" . && rm -rf "$WORK/add"
	timeout 600 "$P" rmi -f img:470 >/dev/null 2>&1
	if ! timeout 600 "$P" import "$WORK/img.tar" img:470 >/dev/null 2>"$WORK/err"; then
		bad "size $n: the import failed: $(tail -1 "$WORK/err")"
		continue
	fi
	rm -f "$WORK/img.tar"
	runs=0
	# The estimate of mem.rs, rounded up to a step: the high bound.
	est=$(up $((n / 4 * 13 + 512)))
	hi=$((est + STEP))
	if ! boot "$hi"; then
		bad "size $n: no boot at the high bound $hi MiB: $(tail -1 "$WORK/err" | cut -c1-200)"
		continue
	fi
	lo=$STEP
	if boot "$lo"; then
		bad "size $n: a boot at the low bound $lo MiB: the search has no lower point"
		continue
	fi
	while [ $((hi - lo)) -gt "$STEP" ]; do
		mid=$(((lo + hi) / 2 / STEP * STEP))
		if boot "$mid"; then hi=$mid; else lo=$mid; fi
	done
	bytes=$(find "$PODVM_STORE/machine" -maxdepth 1 -name 'base-*.cpio' -newer "$WORK/base.tar" \
		-exec stat -c '%Y %s' {} + 2>/dev/null | sort -n | tail -1 | awk '{print $2}')
	initrd=-
	if [ -n "$bytes" ]; then
		initrd="$((bytes / 1048576)) MiB"
		# podvm's estimate from the bytes of the initrd, as mem.rs computes it.
		est=$(((bytes / 4 * 13 + 512 * 1048576 + STEP * 1048576 - 1) / (STEP * 1048576) * STEP))
	else
		bad "size $n: no initrd in the store, so the estimate is from the size asked"
	fi
	say "$(printf '%-6s %-9s %-14s %-14s %-10s %s' "$n" "$initrd" "$lo MiB" "$hi MiB" "$est MiB" "$runs")"
	[ "$est" -ge "$hi" ] || bad "size $n: the estimate $est MiB is below the smallest memory that boots, $hi MiB"
	find "$PODVM_STORE/machine" -maxdepth 1 -name 'base-*.cpio' -exec rm -f {} + 2>/dev/null
done
timeout 600 "$P" rmi -f img:470 >/dev/null 2>&1
sleep 2
left=$(pgrep -a -u "$(id -u)" qemu-system || true)
[ -z "$left" ] || bad "an emulator stays: $left"
[ "$fail" -eq 0 ] && say "EACH POINT HOLDS" || say "A POINT FAILED"
exit "$fail"
