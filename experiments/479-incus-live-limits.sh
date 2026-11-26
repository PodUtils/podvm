#!/bin/sh
# T-2093: does podvm update change the limits of a running Incus instance?
# Run on the Incus job of kvm.yml, with the job's initialized cgroup v2
# daemon and the current podvm binary. PODVM_479_BIN and PODVM_479_OUT
# override the binary and transcript. Exit 0: matched, 1: failed, 2: skipped.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=${PODVM_479_BIN:-$root/target/x86_64-unknown-linux-musl/release/podvm}
out=${PODVM_479_OUT:-$root/experiments/results/incus-live-limits.txt}
image=mirror.gcr.io/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
export PODVM_INCUS_SOCKET=${PODVM_INCUS_SOCKET:-/var/lib/incus/unix.socket}
export INCUS_SOCKET=$PODVM_INCUS_SOCKET
export LC_ALL=C

incus_call() { timeout -k 5 15 incus --project default "$@" </dev/null; }
query() { timeout -k 5 15 incus query "local:$1?project=default" </dev/null; }
podvm_call() { timeout -k 5 180 "$bin" "$@" </dev/null; }
show() { printf '%s\n' "$*" | tee -a "$out"; }
fail() { show "FAIL: $*"; exit 1; }
skip() { show "SKIP: $*"; exit 2; }

umask 077
mkdir -p "$(dirname -- "$out")"
: >"$out"
show "question  can podvm lower the limits of a running Incus instance?"
show "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
show "host      $(uname -srm)"
show "revision  $(git -C "$root" rev-parse HEAD)"
show "image     $image"
[ -x "$bin" ] || skip "no executable podvm at $bin"
for cmd in incus jq nproc timeout; do
    command -v "$cmd" >/dev/null || skip "no $cmd"
done
[ -S "$PODVM_INCUS_SOCKET" ] || skip "no Incus socket at $PODVM_INCUS_SOCKET"
[ "$(stat -fc %T /sys/fs/cgroup)" = cgroup2fs ] || skip "the host must use cgroup v2"
podvm_call version >>"$out" 2>&1 || skip "podvm version failed"
incus_call version local: >>"$out" 2>&1 || skip "the Incus daemon did not answer"

work=$(mktemp -d "${TMPDIR:-/tmp}/podvm-479.XXXXXXXX") || skip "could not make a private store"
name=live479-$(printf '%s' "${work##*.}" | tr '[:upper:]' '[:lower:]')
export PODVM_STORE=$work/store
attempted=0

instances() { incus_call list local: --format csv -c n; }
exists() { printf '%s\n' "$1" | grep -Fxq "$name"; }
config() { query "/1.0/instances/$name"; }
owned() {
    jq -e --arg name "$name" --arg image "$image" '
        .name == $name and .config["user.podvm"] == "1" and .config["user.podvm.image"] == $image
    ' >/dev/null
}
cleanup() {
    result=$?
    trap - 0 HUP INT TERM
    if [ "$attempted" -eq 1 ]; then
        if ! remaining=$(instances); then
            show "FAIL: could not check cleanup of $name"
            result=1
        elif exists "$remaining"; then
            if ! instance=$(config) || ! owned <<EOF
$instance
EOF
            then
                show "FAIL: ownership of $name could not be established for cleanup"
                result=1
            elif ! podvm_call rm -f --podvm-tier=incus "$name" >>"$out" 2>&1 ||
                ! remaining=$(instances) || exists "$remaining"; then
                show "FAIL: could not remove $name"
                result=1
            fi
        fi
    fi
    rm -rf -- "$work" || result=1
    [ "$result" -ne 0 ] || show "PASS: the live Incus limits changed without restarting the instance"
    exit "$result"
}
trap cleanup 0
trap 'exit 1' HUP INT TERM

remaining=$(instances) || skip "could not list Incus instances"
if exists "$remaining"; then skip "the generated instance name is in use"; fi
podvm_call pull "$image" >>"$out" 2>&1 || skip "could not pull the pinned image"
set --
cpu=
available=$(nproc) || skip "could not count available CPUs"
if [ "$available" -ge 2 ]; then
    cpu=2
    set -- --cpus "$cpu"
else
    show "CPU count check skipped: fewer than two available CPUs"
fi
attempted=1
podvm_call run -d --name "$name" --podvm-tier=incus --podvm-net none --pull never \
    --memory 512m --pids-limit 128 "$@" "$image" sleep 900 >>"$out" 2>&1 ||
    fail "could not start $name with the initial limits"

init_pid() {
    state=$(query "/1.0/instances/$name/state") || return 1
    printf '%s\n' "$state" | jq -er 'select(.status == "Running") | .pid | numbers | select(. > 0)'
}
pid=$(init_pid) || fail "Incus did not report a running instance and host init PID"
show "instance  $name (host init PID $pid)"

observe() {
    phase=$1 memory=$2 processes=$3 cpus=$4
    instance=$(config) || fail "$phase: Incus did not return the instance config"
    owned <<EOF || fail "$phase: the instance identity does not match"
$instance
EOF
    printf '%s\n' "$instance" | jq -e --arg memory "${memory}B" --arg pids "$processes" --arg cpu "$cpus" '
        .config | .["limits.memory"] == $memory and .["limits.processes"] == $pids and
        ($cpu == "" or .["limits.cpu"] == $cpu)
    ' >/dev/null || fail "$phase: persisted config differs from requested limits: $instance"
    show "$phase config memory processes CPUs: ${memory}B $processes ${cpus:--}"
    running=$(podvm_call inspect --podvm-tier=incus --format '{{.Status}} {{.EnteredRung}}' "$name") ||
        fail "$phase: podvm inspect failed"
    [ "$running" = "Running incus" ] || fail "$phase: podvm inspect gave $running"

    # The cpuset scheduler can settle after the Incus configuration changes.
    want="$memory $processes ${cpus:--}"
    for _ in 1 2 3 4 5; do
        observed=$(incus_call exec "local:$name" -- sh -ec '
            read -r memory < /sys/fs/cgroup/memory.max
            read -r pids < /sys/fs/cgroup/pids.max
            cpus=-
            if [ -n "$1" ]; then
                cpus=$(awk "/^Cpus_allowed_list:/ {
                    n=split(\$2, ranges, \",\")
                    for (i=1; i<=n; i++) {
                        count=split(ranges[i], limits, \"-\")
                        total += count == 2 ? limits[2] - limits[1] + 1 : 1
                    }
                    if (total) print total
                }" /proc/self/status)
            fi
            printf "%s %s %s\n" "$memory" "$pids" "$cpus"
        ' sh "$cpus" 2>>"$out") || observed="<unreadable>"
        [ "$observed" != "$want" ] || break
        sleep 1
    done
    show "$phase cgroup memory.max pids.max allowed-CPU count: $observed"
    [ "$observed" = "$want" ] || fail "$phase: cgroup values differ (expected $want)"
    current=$(init_pid) || fail "$phase: Incus did not report the running instance"
    [ "$current" = "$pid" ] || fail "$phase: init PID changed from $pid to $current"
}

observe initial 536870912 128 "$cpu"
podvm_call update --podvm-tier=incus --memory 256m "$name" >>"$out" 2>&1 ||
    fail "podvm update of memory to 256m failed"
observe memory-update 268435456 128 "$cpu"

set --
if [ -n "$cpu" ]; then
    cpu=1
    set -- --cpus "$cpu"
fi
podvm_call update --podvm-tier=incus --pids-limit 64 "$@" "$name" >>"$out" 2>&1 ||
    fail "podvm update of process and CPU limits failed"
observe process-update 268435456 64 "$cpu"
