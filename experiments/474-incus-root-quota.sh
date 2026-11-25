#!/bin/sh
# Question: does --storage-opt size=1GiB bound writes to the Incus root?
# TODO/machine.md T-1964. Run on the isolated incus job of kvm.yml. The
# daemon at PODVM_INCUS_SOCKET must have a btrfs default root pool with
# more than 2 GiB free. Set PODVM_474_BIN to use a different build.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=${PODVM_474_BIN:-$root/target/x86_64-unknown-linux-musl/release/podvm}
out=$root/experiments/results/incus-root-quota.txt
image=mirror.gcr.io/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
export PODVM_INCUS_SOCKET=${PODVM_INCUS_SOCKET:-/var/lib/incus/unix.socket}
export INCUS_SOCKET=$PODVM_INCUS_SOCKET

incus_call() { timeout 30 incus "$@"; }
podvm_call() { timeout 180 "$bin" "$@"; }

if [ ! -x "$bin" ] || [ ! -S "$PODVM_INCUS_SOCKET" ] ||
    ! command -v incus >/dev/null || ! command -v jq >/dev/null; then
    echo "SKIP: need podvm, incus, jq and an incusd socket" >&2
    exit 2
fi
pool=$(incus_call profile device get default root pool) || exit 2
pool_info=$(incus_call query "/1.0/storage-pools/$pool") || exit 2
driver=$(printf '%s\n' "$pool_info" | jq -er '.driver') || exit 2
if [ "$driver" != btrfs ]; then
    echo "SKIP: the default root pool must use btrfs (got $driver)" >&2
    exit 2
fi

room() {
    resources=$(incus_call query "/1.0/storage-pools/$pool/resources") || return 1
    printf '%s\n' "$resources" | jq -er '.space.total - .space.used'
}
free=$(room) || exit 2
if [ "$free" -le 2147483648 ]; then
    echo "SKIP: the pool has less than 2 GiB available for the control write" >&2
    exit 2
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/podvm-474.XXXXXXXX") || exit 2
name=quota474-$(basename "$work" | cut -d. -f2 | tr '[:upper:]' '[:lower:]')
export PODVM_STORE=$work/store
cleanup() {
    result=$?
    podvm_call rm -f --podvm-tier=incus "$name" >/dev/null 2>&1 || :
    if ! remaining=$(incus_call list --format csv -c n) ||
        printf '%s\n' "$remaining" | grep -Fxq "$name"; then
        echo "FAIL: could not verify removal of $name" >&2
        result=1
    fi
    rm -rf -- "$work"
    exit "$result"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

show() { printf '%s\n' "$*" | tee -a "$out"; }
fail() { show "FAIL: $*"; exit 1; }
mkdir -p "$(dirname "$out")"
: >"$out"
show "date     $(date -u +%Y-%m-%dT%H:%M:%SZ)"
show "kernel   $(uname -sr)"
show "revision $(git -C "$root" rev-parse HEAD)"
show "podvm    $(podvm_call version)"
incus_call version >>"$out" 2>&1 || exit 2
show "pool     $pool ($driver)"
show "pool free before the run $free bytes"
show "image    $image"
incus_call storage info "$pool" >>"$out" 2>&1 || exit 2
podvm_call pull "$image" >>"$out" 2>&1 || exit 2

# Hold the command until the caller reads the state of the running disk.
podvm_call run -d --name "$name" --podvm-tier=incus --podvm-net none \
    --storage-opt size=1GiB "$image" sh -c '
    export LC_ALL=C
    until [ -f /quota-start ]; do sleep 1; done
    dd if=/dev/zero of=/quota-small bs=1M count=16 conv=fsync || exit 8
    echo "small write passed"
    dd if=/dev/zero of=/quota-large bs=1M count=2048 conv=fsync && exit 9
    exit 7
' >>"$out" 2>&1 || fail "run -d with a 1 GiB root disk"

running=$(incus_call query "/1.0/instances/$name/state") ||
    fail "incusd did not return the state of the running instance"
disk=$(printf '%s\n' "$running" | jq -er '.disk.root.total') ||
    fail "incusd did not report the size of the running root disk"
show "root disk total $disk bytes"
[ "$disk" = 1073741824 ] || fail "incusd did not report a 1 GiB root disk"
incus_call exec "$name" -- touch /quota-start || fail "could not start the writes"

state=
for i in $(seq 1 60); do
    state=$(timeout 6 "$bin" inspect --podvm-tier=incus --format '{{.Status}} {{.ExitCode}}' "$name") ||
        fail "could not inspect the instance"
    case "$state" in Stopped\ *) break ;; esac
    sleep 1
done
show "state    $state"
[ "$state" = 'Stopped 7' ] || fail "expected the large write to fail with payload exit 7"
podvm_call logs --podvm-tier=incus "$name" >"$work/log" 2>&1 || fail "could not read the payload log"
cat "$work/log" >>"$out"
grep -q '^small write passed$' "$work/log" || fail "the small write did not pass"
grep -Eq 'No space left on device|Disk quota exceeded' "$work/log" || fail "the large write did not report a quota error"
free=$(room) || fail "could not check the space left in the pool"
show "pool free after the writes $free bytes"
[ "$free" -gt 2147483648 ] || fail "the pool ran out of space during the test"
show "THE INCUS ROOT DISK QUOTA HOLDS"
