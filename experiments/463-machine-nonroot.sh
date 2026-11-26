#!/bin/sh
# Question: does `run --podvm-tier=machine` under KVM pass for a user with
# no root that `sudo -u` makes, as it passes for the user of the runner?
#
# PodUtils/podrunner#195 (COORD-069 of podrunner), TODO/machine.md T-1922.
# On a hosted runner, the lab user of podrunner (made with `sudo -u USER
# --preserve-env=PATH,TMPDIR`, a fresh HOME, no root, /dev/kvm 0666) got
# exit 125 and "the guest ended before it gave the exit code of the
# payload", with the chardev notice of QEMU as the only line. The user of
# the runner passes the same command. Run it as the user of a GitHub
# runner, with passwordless sudo and /dev/kvm 0666 (the job `nonroot` of
# .github/workflows/kvm.yml).
#
# Pinned inputs:
#   podvm   PODVM_463_BIN (default the release build of the workspace)
#   image   public.ecr.aws/docker/library/alpine:3.20 at its digest (412)
#
# Clauses:
#   1. as the user of the runner, `run --rm --podvm-tier=machine
#      --podvm-mem 4G IMAGE true` exits 0;
#   2. as a new user through `sudo -u`, with a fresh HOME and its own store,
#      the same command exits 0;
#   3. as that user, with --podvm-mem 1G, it exits 0;
#   4. no emulator stays after the runs;
#   5. an image with a root of about 300 MiB (PodUtils/podrunner#198: the
#      runner image of podrunner, 293 MiB, ends before the command for the
#      lab user) boots with --podvm-mem 4G, as the user of the runner and
#      as the new user. Skipped with no docker, which builds its tar;
#   6. an image with a root of about 1400 MiB (PodUtils/podrunner#199: the
#      runner image's initrd of 1348 MiB closes the console after 2.6 s), as
#      the user of the runner: --podvm-mem 4G fails and names the estimate
#      of the memory; 5G and no --podvm-mem (podvm sizes it) boot.
# Each failed clause keeps its stderr, the limits of the user, and the end
# of the kernel log in the transcript.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_463_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
OUT="$ROOT/experiments/results/machine-nonroot.txt"
IMG='public.ecr.aws/docker/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
LAB=podvmlab
if [ ! -r /dev/kvm ] || [ ! -w /dev/kvm ] || ! sudo -n true 2>/dev/null; then
	echo "SKIP: no usable /dev/kvm or no passwordless sudo" >&2
	exit 2
fi
fail=0
mkdir -p "$(dirname "$OUT")"
: >"$OUT"
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }
keep() { sed 's/^/    /' "$1" | tee -a "$OUT"; }
kernel() { sudo dmesg 2>/dev/null | tail -15 | sed 's/^/    dmesg: /' | tee -a "$OUT"; }

# The binary goes where each user can run it, as an installer puts it.
sudo install -m 0755 "$BIN" /usr/local/bin/podvm-463
P=/usr/local/bin/podvm-463
say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "podvm     $($P version 2>/dev/null)"
say "runner    uid $(id -u), groups $(id -Gn)"
say "kvm       $(ls -l /dev/kvm)"

# Clause 1: the user of the runner.
timeout 600 "$P" run --verbose --rm --podvm-tier=machine --podvm-mem 4G "$IMG" true \
	>/dev/null 2>"$OUT.1.err"
rc=$?
say "clause 1  rc $rc (the user of the runner)"
[ "$rc" -eq 0 ] || { bad "clause 1"; keep "$OUT.1.err"; kernel; }

# Clauses 2 and 3: a new user through sudo, as podrunner's nonroot.sh.
id "$LAB" >/dev/null 2>&1 || sudo useradd -m -s /bin/sh "$LAB"
LH=$(getent passwd "$LAB" | cut -d: -f6)
sudo -u "$LAB" mkdir -p "$LH/gate/home" "$LH/store"
# The sudo of a GitHub runner keeps XDG_CONFIG_HOME of the runner's user
# (run 43816612095: "/home/runner/.config/podvm/registries.conf: Permission
# denied"); podrunner's lab user gets no XDG_* of another user.
lab() {
	sudo -u "$LAB" --preserve-env=PATH,TMPDIR \
		env -u XDG_CONFIG_HOME -u XDG_CACHE_HOME -u XDG_DATA_HOME \
		-u XDG_STATE_HOME -u XDG_RUNTIME_DIR \
		HOME="$LH/gate/home" PODVM_STORE="$LH/store" "$@"
}
say "lab       $(lab id)"
say "lab env   $(lab env | cut -d= -f1 | sort | tr '\n' ' ')"
lab sh -c 'ulimit -a' 2>&1 | sed 's/^/    limit: /' >>"$OUT"
lab timeout 300 "$P" pull -q "$IMG" >/dev/null 2>&1

n=2
for mem in 4G 1G; do
	lab timeout 600 "$P" run --verbose --rm --podvm-tier=machine --podvm-mem "$mem" "$IMG" true \
		>/dev/null 2>"$OUT.$n.err"
	rc=$?
	say "clause $n  rc $rc (user $LAB through sudo, --podvm-mem $mem)"
	[ "$rc" -eq 0 ] || { bad "clause $n"; keep "$OUT.$n.err"; kernel; }
	n=$((n + 1))
done

# Clause 5: a large root. Alpine and 300 MiB of random bytes, which no
# compression shrinks, imported by each user into its own store.
if command -v docker >/dev/null 2>&1 && docker pull -q alpine:3.20 >/dev/null 2>&1; then
	BIG=$(mktemp -d /tmp/podvm-463-big.XXXXXX)
	cid=$(docker create alpine:3.20 true)
	docker export "$cid" >"$BIG/big.tar"
	docker rm "$cid" >/dev/null
	mkdir -p "$BIG/add" && head -c 314572800 /dev/urandom >"$BIG/add/blob"
	tar -rf "$BIG/big.tar" -C "$BIG/add" blob && rm -rf "$BIG/add"
	chmod 0755 "$BIG" && chmod 0644 "$BIG/big.tar"
	say "big       $(du -m "$BIG/big.tar" | cut -f1) MiB tar"
	timeout 300 "$P" import "$BIG/big.tar" big:463 >/dev/null 2>&1
	lab timeout 300 "$P" import "$BIG/big.tar" big:463 >/dev/null 2>&1
	start=$(date +%s)
	timeout 900 "$P" run --verbose --rm --podvm-tier=machine --podvm-mem 4G big:463 true \
		>/dev/null 2>"$OUT.5.err"
	rc=$?
	say "clause 5  rc $rc in $(($(date +%s) - start)) s (the user of the runner, a root of 300 MiB)"
	[ "$rc" -eq 0 ] || { bad "clause 5, the user of the runner"; keep "$OUT.5.err"; kernel; }
	start=$(date +%s)
	lab timeout 900 "$P" run --verbose --rm --podvm-tier=machine --podvm-mem 4G big:463 true \
		>/dev/null 2>"$OUT.6.err"
	rc=$?
	say "clause 5  rc $rc in $(($(date +%s) - start)) s (user $LAB through sudo, a root of 300 MiB)"
	[ "$rc" -eq 0 ] || { bad "clause 5, user $LAB"; keep "$OUT.6.err"; kernel; }
	rm -rf "$BIG"
else
	say "clause 5  skipped: no docker to build the large image"
fi

# Clause 6: a root of 1400 MiB, as podrunner's runner image.
if command -v docker >/dev/null 2>&1 && docker pull -q alpine:3.20 >/dev/null 2>&1; then
	HUGE=$(mktemp -d /tmp/podvm-463-huge.XXXXXX)
	cid=$(docker create alpine:3.20 true)
	docker export "$cid" >"$HUGE/huge.tar"
	docker rm "$cid" >/dev/null
	mkdir -p "$HUGE/add"
	for i in 1 2 3 4 5 6 7; do head -c 209715200 /dev/urandom >"$HUGE/add/blob$i"; done
	tar -rf "$HUGE/huge.tar" -C "$HUGE/add" . && rm -rf "$HUGE/add"
	say "huge      $(du -m "$HUGE/huge.tar" | cut -f1) MiB tar, $(df -m /tmp | awk 'NR==2{print $4}') MiB free"
	timeout 600 "$P" import "$HUGE/huge.tar" huge:463 >/dev/null 2>&1
	rm -f "$HUGE/huge.tar"
	# Run 43876504275: 4G resets the guest, 5G, 6G and 8G boot. With no
	# --podvm-mem podvm sizes the memory for the initrd (T-1922); with 4G it
	# warns before the boot and names the estimate after the failure.
	for mem in 4G 5G default; do
		flag="--podvm-mem $mem"
		[ "$mem" = default ] && flag=""
		start=$(date +%s)
		# The memory of the initramfs root (T-1922), which T-1994 keeps
		# behind PODVM_MACHINE_ROOT=initramfs.
		# shellcheck disable=SC2086
		PODVM_MACHINE_ROOT=initramfs timeout 900 "$P" run --verbose --rm --podvm-tier=machine $flag huge:463 true \
			>/dev/null 2>"$OUT.7.err"
		rc=$?
		say "clause 6  rc $rc in $(($(date +%s) - start)) s (a root of 1400 MiB, --podvm-mem $mem)"
		keep "$OUT.7.err"
		case $mem in
		4G) [ "$rc" -ne 0 ] && grep -q "likely needs about" "$OUT.7.err" ||
			bad "clause 6, 4G: no failure that names the estimate" ;;
		*) [ "$rc" -eq 0 ] || bad "clause 6, --podvm-mem $mem" ;;
		esac
	done
	timeout 120 "$P" rmi -f huge:463 >/dev/null 2>&1
	rm -rf "$HUGE"
else
	say "clause 6  skipped: no docker to build the large image"
fi

# Clause 4: nothing of a guest stays.
if pgrep -a qemu-system >/dev/null; then
	bad "clause 4: $(pgrep -a qemu-system | tr '\n' ' ')"
	sudo pkill -9 qemu-system || true
else
	say "clause 4  no emulator stays"
fi
rm -f "$OUT".*.err
[ "$fail" -eq 0 ] && say "PASS" || say "FAIL"
exit "$fail"
