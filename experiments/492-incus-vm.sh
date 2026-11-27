#!/bin/sh
# T-1998 (PodUtils/podrunner#316): does `run -d --podvm-tier=incus
# --podvm-vm --podvm-init=image` boot an OCI image with a kernel as a
# virtual machine of incusd?
#
# Run on the incusvm job of kvm.yml: an incusd of apt with its VM driver
# (KVM, QEMU, OVMF and incus-agent), a docker daemon and the podvm of the
# checkout. The fixture: Ubuntu 24.04 with the kernel package
# linux-image-virtual (its postinst writes the initrd), systemd and udev,
# built with docker and served by a registry on loopback.
#
# Clauses:
#   1. The first run boots the image as a VM: incusd lists the instance
#      as a virtual-machine, and podvm imported the image now.
#   2. In the VM, `systemctl is-system-running` gives running or degraded.
#   3. In the VM, `losetup -f` prints a loop device.
#   4. `wait` gives the exit code of the command (7), and the VM stops.
#   5. A second run of the same digest takes the VM image of incusd, and
#      its `wait` gives 0.
#
# PODVM_492_BIN and PODVM_492_OUT override the binary and the transcript.
# Exit 0: each clause held, 1: one did not, 2: the host cannot run it.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=${PODVM_492_BIN:-$root/target/x86_64-unknown-linux-musl/release/podvm}
out=${PODVM_492_OUT:-$root/experiments/results/incus-vm.txt}
base=mirror.gcr.io/library/ubuntu:24.04
registry=mirror.gcr.io/library/registry:2
port=${PODVM_492_PORT:-5492}
image=localhost:$port/podvm492:1
export PODVM_INCUS_SOCKET=${PODVM_INCUS_SOCKET:-/var/lib/incus/unix.socket}
export INCUS_SOCKET=$PODVM_INCUS_SOCKET
export LC_ALL=C

incus_call() { timeout -k 5 60 incus --project default "$@" </dev/null; }
podvm_call() {
    t=$1
    shift
    timeout -k 10 "$t" "$bin" "$@" </dev/null
}
show() { printf '%s\n' "$*" | tee -a "$out"; }
fail() { show "FAIL: $*"; exit 1; }
skip() { show "SKIP: $*"; exit 2; }

umask 022
mkdir -p "$(dirname -- "$out")"
: >"$out"
show "question  does --podvm-vm boot an OCI image with a kernel as an incus VM?"
show "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
show "host      $(uname -srm)"
show "revision  $(git -C "$root" rev-parse HEAD)"
[ -x "$bin" ] || skip "no executable podvm at $bin"
for cmd in incus docker jq timeout; do
    command -v "$cmd" >/dev/null || skip "no $cmd"
done
[ -S "$PODVM_INCUS_SOCKET" ] || skip "no incus socket at $PODVM_INCUS_SOCKET"
driver=$(incus_call query local:/1.0 | jq -r .environment.driver) || skip "incusd did not answer"
show "drivers   $driver"
case "$driver" in
*qemu*) ;;
*) skip "incusd has no VM driver" ;;
esac

# The store is under the checkout: QEMU of incusd reads the kernel there.
work=$(mktemp -d "$root/.work/podvm-492.XXXXXXXX") || skip "no private store"
chmod 0755 "$work"
export PODVM_STORE=$work/store
reg=podvm-492-registry-$$
names=""
cleanup() {
    result=$?
    trap - 0 HUP INT TERM
    for n in $names; do
        podvm_call 120 rm -f --podvm-tier=incus "$n" >>"$out" 2>&1 || true
        if incus_call list local: --format csv -c n | grep -Fxq "$n"; then
            show "FAIL: the instance $n stays"
            result=1
        fi
    done
    for a in $(incus_call image alias list local: --format csv -c n 2>/dev/null | grep '^podvm-vm-' || true); do
        incus_call image delete "local:$a" >>"$out" 2>&1 || true
    done
    docker rm -f "$reg" >/dev/null 2>&1 || true
    rm -rf -- "$work" || result=1
    [ "$result" -ne 0 ] || show "PASS: THE INCUS VM MODE RUNS"
    exit "$result"
}
trap cleanup 0
trap 'exit 1' HUP INT TERM

# The fixture image.
mkdir -p "$work/ctx"
cat >"$work/ctx/Dockerfile" <<EOF
FROM $base
RUN apt-get -qq update \\
 && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \\
      linux-image-virtual initramfs-tools systemd-sysv udev kmod mount >/dev/null \\
 && rm -rf /var/lib/apt/lists/* \\
 && ls -l /boot
EOF
timeout -k 10 1200 docker build -q -t "$image" "$work/ctx" >>"$out" 2>&1 || fail "docker build of the fixture"
docker run -d --rm --name "$reg" -p "127.0.0.1:$port:5000" "$registry" >>"$out" 2>&1 || skip "no registry"
sleep 2
timeout -k 10 600 docker push -q "$image" >>"$out" 2>&1 || fail "docker push of the fixture"

run_vm() {
    name=$1
    shift
    names="$names $name"
    podvm_call 1500 run -d --podvm-tier=incus --podvm-vm --podvm-init=image \
        --insecure-registry "localhost:$port" --name "$name" "$image" "$@"
}

# Clause 1: the first run builds, imports and boots the VM.
start=$(date +%s)
run_vm vm492a sh -c 'while [ ! -e /run/podvm492.go ]; do sleep 1; done; exit 7' \
    >"$work/run1.out" 2>"$work/run1.err" || {
    cat "$work/run1.err" >>"$out"
    fail "clause 1: the first run -d failed"
}
cat "$work/run1.err" >>"$out"
show "clause 1  first run -d took $(($(date +%s) - start)) s"
kind=$(incus_call query local:/1.0/instances/vm492a | jq -r .type)
[ "$kind" = virtual-machine ] || fail "clause 1: the instance is a $kind"
grep -q "imported now" "$work/run1.err" || fail "clause 1: no import of the VM image"
show "clause 1  PASS: a virtual-machine, image imported now"

# Clause 2: systemd of the image is up.
state=$(podvm_call 120 exec --podvm-tier=incus vm492a systemctl is-system-running 2>>"$out" || true)
show "clause 2  systemctl is-system-running: $state"
case "$state" in
running | degraded) show "clause 2  PASS" ;;
*) fail "clause 2: systemd gives $state" ;;
esac

# Clause 3: a loop device.
loop=$(podvm_call 120 exec --podvm-tier=incus vm492a losetup -f 2>>"$out" || true)
show "clause 3  losetup -f: $loop"
case "$loop" in
/dev/loop[0-9]*) show "clause 3  PASS" ;;
*) fail "clause 3: losetup -f gives $loop" ;;
esac
podvm_call 120 exec --podvm-tier=incus vm492a uname -r >>"$out" 2>&1 || true

# Clause 4: the exit code of the command, then the VM stops.
podvm_call 120 exec --podvm-tier=incus vm492a touch /run/podvm492.go >>"$out" 2>&1 ||
    fail "clause 4: exec touch failed"
code=$(podvm_call 300 wait --podvm-tier=incus vm492a 2>>"$out") || fail "clause 4: wait failed"
show "clause 4  wait: $code"
[ "$code" = 7 ] || fail "clause 4: wait gives $code, not 7"
i=0
while [ "$(incus_call query local:/1.0/instances/vm492a | jq -r .status)" != Stopped ]; do
    i=$((i + 1))
    [ "$i" -lt 120 ] || fail "clause 4: the VM is not Stopped 120 s after wait"
    sleep 1
done
show "clause 4  PASS: exit 7, the VM stopped after $i s"

# Clause 5: a second run takes the VM image of incusd.
run_vm vm492b true >"$work/run2.out" 2>"$work/run2.err" || {
    cat "$work/run2.err" >>"$out"
    fail "clause 5: the second run -d failed"
}
cat "$work/run2.err" >>"$out"
grep -q "from incus" "$work/run2.err" || fail "clause 5: the second run imported again"
code=$(podvm_call 300 wait --podvm-tier=incus vm492b 2>>"$out") || fail "clause 5: wait failed"
[ "$code" = 0 ] || fail "clause 5: wait gives $code, not 0"
show "clause 5  PASS: the VM image of incusd, exit 0"
