#!/bin/sh
# Question: does the Incus tier stream a fresh OCI image and reuse it?
# TODO/machine.md T-2085. Run on the Incus job of kvm.yml before the other
# experiments use this Alpine digest. The daemon must lack both image aliases
# for this podvm binary. The second fixture is from experiment 433. Set
# PODVM_475_BIN to use a different build.
# Exit 0: each clause passed. 1: a clause failed. 2: a prerequisite failed.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=${PODVM_475_BIN:-$root/target/x86_64-unknown-linux-musl/release/podvm}
out=$root/experiments/results/incus-layer-stream.txt
image=mirror.gcr.io/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
retained_image=mirror.gcr.io/library/alpine@sha256:bc21487101ae47172509130ee1092cc273a831968d64b617776d1f69c81e42ab
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
    shift
    token=incus-stream-475-$phase
    rc=0
    podvm_call run --rm --pull never --verbose "$@" "$image" /bin/echo "$token" \
        </dev/null >"$work/$phase.out" 2>"$work/$phase.err" || rc=$?
    show "$phase exit $rc"
    sed 's/^/  stdout  /' "$work/$phase.out" | tee -a "$out"
    sed 's/^/  stderr  /' "$work/$phase.err" | tee -a "$out"
    [ "$rc" -eq 0 ] || fail "$phase payload exit was $rc"
    printf '%s\n' "$token" >"$work/want"
    cmp -s "$work/want" "$work/$phase.out" || fail "$phase payload output differed"
}

run_incus() {
    run_payload "$1" --name "$name" --podvm-tier=incus --podvm-net none
    remaining=$(instances) || fail "could not list Incus instances after $1"
    if exists "$remaining"; then fail "$name stayed after $1"; fi
    show "$1 instance removed"
}

fresh_import() {
    run_incus "$1"
    grep -Eq '^[[:xdigit:]]{12}: Flattened \(' "$work/$1.err" || fail "$1 import did not report a flattened layer"
    if grep -Eq '^[[:xdigit:]]{12}: Extracted \(' "$work/$1.err"; then
        fail "$1 import extracted a tree"
    fi
    imported_alias=$(sed -nE 's/^podvm run: incus tier: .* image (podvm-[0-9a-f]+-[0-9a-f]+-v[0-9]+) imported now .*/\1/p' "$work/$1.err")
    [ -n "$imported_alias" ] || fail "$1 run did not report a fresh Incus import"
    incus_call image info "local:$imported_alias" >>"$out" 2>&1 || fail "Incus could not read the imported image"
}

check_tree before
fresh_import fresh
check_tree fresh
first_alias=$imported_alias

run_incus cached
check_tree cached
grep -Fq "image $first_alias from incus (" "$work/cached.err" || fail "the second run did not take the same image from Incus"
if grep -Eq '^[[:xdigit:]]{12}: (Flattened|Extracted) \(' "$work/cached.err"; then
    fail "the cached run read OCI layers"
fi
# A second digest makes its Incus import fresh while the first alias stays
# available for experiment 471.
image=$retained_image
show "native image $image"
podvm_call pull "$image" </dev/null >>"$out" 2>&1 || skip "could not pull the native fixture"
check_tree before_native
run_payload native
native_tree=$(podvm_call inspect --format '{{.RootfsPath}}' "$image" 2>>"$out") || fail "cannot look up the native tree"
case "$native_tree" in
    "$PODVM_STORE"/rootfs/sha256_*/rootfs) ;;
    *) fail "the native tree path is outside the private store" ;;
esac

tree_id() {
    [ -d "$native_tree" ] && [ -f "${native_tree%/rootfs}/.extracted" ] || return 1
    stat -c '%d:%i' "$native_tree"
}

before=$(tree_id) || fail "the native run did not create a complete tree"
show "native store tree device:inode $before"
fresh_import retained
[ "$imported_alias" != "$first_alias" ] || fail "the two fixtures have the same Incus alias"
after=$(tree_id) || fail "the fresh import removed the native tree"
[ "$before" = "$after" ] || fail "the fresh import replaced the native tree"
show "retained store tree device:inode $after"
show "THE INCUS LAYER STREAM, CACHED RUN AND NATIVE TREE HOLD"
