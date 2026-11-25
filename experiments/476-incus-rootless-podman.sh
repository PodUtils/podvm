#!/bin/sh
# T-1996: does the hosted snapshot support rootless Podman and a FUSE overlay?
# Run this mounted script as root inside the snapshot booted with
# --podvm-tier=incus --podvm-init=image --podvm-engine --podvm-net none.
# Give an absolute path to a locally mounted, tagged OCI archive with
# /bin/sh (for example Alpine), and its image name as loaded by Podman.
set -u

skip() { printf 'SKIP: %s\n' "$*" >&2; exit 2; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

need() {
    for cmd do
        command -v "$cmd" >/dev/null 2>&1 || skip "need $cmd in the snapshot"
    done
}

runner_probe() {
    archive=$1 image=$2 scratch=$3
    uid=$(id -u) && gid=$(id -g) || skip "cannot identify the probe user"
    [ "$uid" -ne 0 ] && [ "$(id -un)" = runner ] || fail "the probe must run as runner"
    [ -r "$archive" ] || skip "runner cannot read the mounted OCI archive: $archive"
    cd "$scratch" || skip "runner cannot enter the probe scratch directory"
    need podman newuidmap newgidmap fuse-overlayfs fusermount3 findmnt
    timeout -k 5 10 podman --version || skip "cannot read the Podman version"
    timeout -k 5 10 fuse-overlayfs --version || skip "cannot read the fuse-overlayfs version"

    # Use a fresh image store, with no dependency on a user systemd session.
    podman_probe() {
        timeout -k 5 120 podman --root "$scratch/images" --runroot "$scratch/containers" \
            --cgroup-manager=cgroupfs "$@" </dev/null
    }
    podman_probe load --input "$archive" || fail "Podman could not load the local OCI archive"
    image_id=$(podman_probe image inspect --format '{{.Id}}' "$image") ||
        fail "the loaded archive lacks image $image"
    [ -n "$image_id" ] || fail "the loaded image has no ID"
    for kind in uid gid; do
        case $kind in
            uid) owner=$uid ;;
            gid) owner=$gid ;;
        esac
        map=$(podman_probe unshare cat "/proc/self/${kind}_map") ||
            fail "Podman could not enter the rootless $kind map"
        printf 'rootless Podman %s_map:\n%s\n' "$kind" "$map"
        awk -v owner="$owner" '
            $1 == 0 && $2 == owner && $3 == 1 { self = 1 }
            $1 == 1 && $2 == 165536 && $3 == 65536 { subordinate = 1 }
            END { exit !(self && subordinate) }
        ' <<EOF || fail "Podman did not map runner and all 65,536 subordinate $kind values"
$map
EOF
    done
    result=$(podman_probe run --rm --pull=never --network none --cgroups=disabled \
        --entrypoint /bin/sh "$image_id" -ec 'test "$(id -u)" -eq 0; echo podvm-476-workload') ||
        fail "offline rootless Podman run failed"
    [ "$result" = podvm-476-workload ] || fail "rootless workload gave unexpected output: $result"
    printf 'rootless Podman ran the local image %s\n' "$image_id"

    lower=$scratch/lower upper=$scratch/upper work=$scratch/work merged=$scratch/merged
    mkdir "$lower" "$upper" "$work" "$merged" || skip "cannot make overlay directories"
    printf '%s\n' from-lower >"$lower/probe" || skip "cannot write the lower fixture"
    [ ! -e "$merged/probe" ] || fail "overlay fixture is visible before mounting"
    timeout -k 5 90 fuse-overlayfs -f -o "lowerdir=$lower,upperdir=$upper,workdir=$work" \
        "$merged" >"$scratch/fuse.log" 2>&1 </dev/null &
    fuse_pid=$!
    trap 'finish_fuse' 0
    trap 'exit 1' HUP INT TERM
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        fstype=$(findmnt -rnC -M "$merged" -o FSTYPE) && break
        kill -0 "$fuse_pid" 2>/dev/null || break
        sleep 1
    done
    [ "${fstype:-}" = fuse.fuse-overlayfs ] || fail "fuse-overlayfs did not mount the test directory"
    content=$(timeout -k 5 10 cat "$merged/probe") || fail "cannot read through the overlay"
    [ "$content" = from-lower ] || fail "overlay did not expose the lower file"
    timeout -k 5 10 sh -c 'printf "from-upper\n" >"$1"' sh "$merged/probe" ||
        fail "cannot write through the overlay"
    content=$(timeout -k 5 10 cat "$merged/probe") || fail "cannot read back the overlay write"
    [ "$content" = from-upper ] || fail "overlay did not expose the write"
    timeout -k 5 10 fusermount3 -u "$merged" || fail "cannot unmount the overlay"
    wait "$fuse_pid" || fail "fuse-overlayfs did not exit cleanly"
    trap - 0 HUP INT TERM
    [ ! -e "$merged/probe" ] || fail "overlay fixture is still visible after unmounting"
    [ "$(cat "$upper/probe")" = from-upper ] && [ "$(cat "$lower/probe")" = from-lower ] ||
        fail "overlay did not copy the write to upper while preserving lower"
}

finish_fuse() {
    status=$?
    trap - 0
    if findmnt -rnC -M "$merged" >/dev/null 2>&1; then
        timeout -k 5 10 fusermount3 -u "$merged" || status=1
    fi
    kill "$fuse_pid" 2>/dev/null || :
    wait "$fuse_pid" 2>/dev/null || :
    cat "$scratch/fuse.log" >&2
    exit "$status"
}

finish_scratch() {
    status=$?
    trap - 0
    rm -rf --one-file-system -- "$scratch" || fail "cannot remove probe scratch: $scratch"
    if [ "$status" -eq 0 ]; then
        printf 'PASS: runner used rootless Podman offline and read and wrote through fuse-overlayfs\n'
    fi
    exit "$status"
}

if [ "${1:-}" = --runner ]; then
    shift
    [ "$#" -eq 3 ] || skip "runner probe needs its archive, image name and scratch directory"
    runner_probe "$@"
    exit
fi

[ "$#" -eq 2 ] || skip "usage: $0 /absolute/local.oci.tar image-name"
archive=$1 image=$2
case $archive in
    /*) [ -f "$archive" ] && [ -r "$archive" ] || skip "need a readable local OCI archive" ;;
    *) skip "give the absolute path to the mounted OCI archive" ;;
esac
case $image in
    ''|-*) skip "give the image name as stored in the OCI archive" ;;
esac
[ "$(id -u)" -eq 0 ] || skip "start as root inside the hosted snapshot"
need awk chown date env getent grep mktemp readlink runuser timeout uname
script=$(readlink -f -- "$0") || skip "cannot resolve the mounted script"
account=$(getent passwd runner) || skip "the hosted snapshot must have a runner account"
runner_home=$(printf '%s\n' "$account" | awk -F: '{print $6}') || skip "cannot read runner home"
[ -n "$runner_home" ] || skip "runner has no home"
IFS= read -r pid1 < /proc/1/comm || skip "cannot identify PID 1"
[ "$pid1" = systemd ] || skip "boot the hosted snapshot with its systemd init"
printf 'date %s\nkernel %s\narchive %s\nimage %s\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -sr)" "$archive" "$image"
for kind in uid gid; do
    grep -Fxq 'runner:165536:65536' "/etc/sub$kind" ||
        fail "/etc/sub$kind does not give runner 165536:65536"
    printf 'instance %s_map:\n' "$kind"
    cat "/proc/self/${kind}_map" || skip "cannot read the instance $kind map"
    awk '$1 == 0 && $3 >= 231072 { found = 1 } END { exit !found }' "/proc/self/${kind}_map" ||
        fail "the instance does not map the first 231,072 $kind values"
done
[ -c /dev/fuse ] || fail "the snapshot has no /dev/fuse character device"

umask 077
scratch=$(mktemp -d /tmp/podvm-476.XXXXXXXX) || skip "cannot create private scratch"
trap 'finish_scratch' 0
trap 'exit 1' HUP INT TERM
mkdir "$scratch/runtime" && chown -R runner: "$scratch" || skip "cannot prepare runner scratch"
runuser -u runner -- env -i HOME="$runner_home" USER=runner LOGNAME=runner \
    PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    XDG_RUNTIME_DIR="$scratch/runtime" LC_ALL=C \
    sh "$script" --runner "$archive" "$image" "$scratch" </dev/null
