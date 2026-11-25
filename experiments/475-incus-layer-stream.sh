#!/bin/sh
# Question: does the Incus tier stream a fresh OCI image and reuse it?
# TODO/machine.md T-2085. Run on the Incus job of kvm.yml before the other
# experiments use this Alpine digest. The daemon must lack the image alias
# for this podvm binary. Set PODVM_475_BIN to use a different build.
# Exit 0: each clause passed. 1: a clause failed. 2: a prerequisite failed.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=${PODVM_475_BIN:-$root/target/x86_64-unknown-linux-musl/release/podvm}
out=$root/experiments/results/incus-layer-stream.txt
image=mirror.gcr.io/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
export PODVM_INCUS_SOCKET=${PODVM_INCUS_SOCKET:-/var/lib/incus/unix.socket}
export INCUS_SOCKET=$PODVM_INCUS_SOCKET

incus_call() { timeout -k 5 30 incus "$@"; }
podvm_call() { timeout -k 5 180 "$bin" "$@"; }
show() { printf '%s\n' "$*" | tee -a "$out"; }
fail() { show "FAIL: $*"; exit 1; }
skip() { show "SKIP: $*"; exit 2; }

mkdir -p "$(dirname "$out")"
: >"$out"
show "question  does Incus import the OCI layers and reuse the image?"
show "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
show "host      $(uname -srm)"
show "revision  $(git -C "$root" rev-parse HEAD)"
show "image     $image"
[ -x "$bin" ] || skip "no executable podvm at $bin"
command -v incus >/dev/null || skip "no Incus client"
[ -S "$PODVM_INCUS_SOCKET" ] || skip "no Incus socket at $PODVM_INCUS_SOCKET"
version=$(podvm_call version 2>&1) || skip "podvm version failed: $version"
show "$version"
incus_call version >>"$out" 2>&1 || skip "the Incus daemon did not answer"

work=$(mktemp -d "${TMPDIR:-/tmp}/podvm-475.XXXXXXXX") || skip "could not make a private store"
name=stream475-$(basename "$work" | cut -d. -f2 | tr '[:upper:]' '[:lower:]')
export PODVM_STORE=$work/store
owns_name=0

instances() { incus_call list local: --format csv -c n; }
exists() { printf '%s\n' "$1" | grep -Fxq "$name"; }
cleanup() {
    status=$?
    trap - EXIT HUP INT TERM
    if [ "$owns_name" -eq 1 ]; then
        if ! remaining=$(instances); then
            show "FAIL: could not check instance cleanup"
            status=1
        elif exists "$remaining"; then
            if ! incus_call delete "local:$name" --force >>"$out" 2>&1 ||
                ! remaining=$(instances) || exists "$remaining"; then
                show "FAIL: could not remove $name"
                status=1
            fi
        fi
    fi
    rm -rf -- "$work"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

remaining=$(instances) || skip "could not list Incus instances"
if exists "$remaining"; then skip "the instance name $name is in use"; fi
owns_name=1
podvm_call pull "$image" </dev/null >>"$out" 2>&1 || skip "could not pull the pinned image"

check_tree() {
    if [ -d "$PODVM_STORE/rootfs" ]; then
        files=$(find "$PODVM_STORE/rootfs" -mindepth 1 -maxdepth 1 -print) || fail "cannot read the store tree directory"
        [ -z "$files" ] || fail "an entry stayed in the store tree directory: $files"
    fi
    show "$1 store tree entries 0"
}

run_payload() {
    phase=$1
    token=incus-stream-475-$phase
    rc=0
    podvm_call run --rm --name "$name" --podvm-tier=incus --podvm-net none \
        --pull never --verbose "$image" /bin/echo "$token" \
        </dev/null >"$work/$phase.out" 2>"$work/$phase.err" || rc=$?
    show "$phase exit $rc"
    sed 's/^/  stdout  /' "$work/$phase.out" | tee -a "$out"
    sed 's/^/  stderr  /' "$work/$phase.err" | tee -a "$out"
    [ "$rc" -eq 0 ] || fail "$phase payload exit was $rc"
    printf '%s\n' "$token" >"$work/want"
    cmp -s "$work/want" "$work/$phase.out" || fail "$phase payload output differed"
    check_tree "$phase"
    remaining=$(instances) || fail "could not list Incus instances after $phase"
    if exists "$remaining"; then fail "$name stayed after $phase"; fi
    show "$phase instance removed"
}

check_tree before
run_payload fresh
grep -Eq '^[[:xdigit:]]{12}: Flattened \(' "$work/fresh.err" || fail "the fresh import did not flatten a layer"
if grep -Eq '^[[:xdigit:]]{12}: Extracted \(' "$work/fresh.err"; then
    fail "the fresh import used the extracted-tree path"
fi
alias=$(sed -nE 's/^podvm run: incus tier: .* image (podvm-[0-9a-f]+-[0-9a-f]+-v[0-9]+) imported now .*/\1/p' "$work/fresh.err")
[ -n "$alias" ] || fail "the first run did not report a fresh Incus import"
incus_call image info "local:$alias" >>"$out" 2>&1 || fail "Incus could not read the imported image"

run_payload cached
grep -Fq "image $alias from incus (" "$work/cached.err" || fail "the second run did not take the same image from Incus"
if grep -Eq '^[[:xdigit:]]{12}: (Flattened|Extracted) \(' "$work/cached.err"; then
    fail "the cached run read OCI layers"
fi
show "THE INCUS LAYER STREAM AND CACHED RUN HOLD"
