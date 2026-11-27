#!/bin/sh
# cloud-setup.sh - make a cloud session or a Linux host ready for the builds
# and checks of podvm. Each step is skipped when its tool is there, so the
# script can run at the start of each session.
#
#   sh scripts/cloud-setup.sh [--dev-tools]
#
# 1. zig from the download procedure shared with scripts/common/bootstrap-env.sh.
#    Both callers use the same version and SHA-256 pin.
# 2. Rustup from the Rust distribution when it is absent. Rustup reads
#    rust-toolchain.toml. scripts/common/bootstrap-env.sh checks the musl
#    target and sets up cargo-bloat, gcc, the binary tools and OpenSSH.
# 3. With --dev-tools, install missing mandoc and shellcheck packages when
#    root or passwordless sudo apt-get works, and fetch locked crates.
# 4. A report of the host facts that podvm's checks depend on. A cloud
#    container is a restricted host of its own kind: record what it allows.
#
# Exit codes: 0 ready, 1 a step failed, 2 could not run (not Linux), 64 usage.
set -u

dev_tools=0
while [ $# -gt 0 ]; do
    case "$1" in
        --dev-tools)
            dev_tools=1
            shift
            ;;
        -h|--help)
            sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "usage: sh scripts/cloud-setup.sh [--dev-tools]" >&2
            exit 64
            ;;
    esac
done

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
[ "$(uname -s)" = Linux ] || { echo "cloud-setup: Linux only" >&2; exit 2; }
cd "$root" || exit 1

ZIG_VERSION=0.16.0
ZIG_SHA256=70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00
tools=${PODVM_TOOLS:-$HOME/.local/podvm}
cargo_bin=${CARGO_HOME:-$HOME/.cargo}/bin
case "$cargo_bin" in /*) ;; *) cargo_bin=$root/$cargo_bin ;; esac
rc=0

mkdir -p "$tools" "$HOME/.local/bin" || exit 1
# shellcheck source=scripts/cloud-env.sh
. ./scripts/cloud-env.sh || exit 1

if [ "$(zig version 2>/dev/null)" != "$ZIG_VERSION" ]; then
    if ! command -v xz >/dev/null 2>&1; then
        bash "$root/scripts/common/bootstrap-env.sh" tools || rc=1
    fi
    if [ "$rc" -eq 0 ] &&
        sh "$root/scripts/common/fetch-zig.sh" x86_64 "$ZIG_VERSION" "$ZIG_SHA256" "$tools/zig.tar.xz" &&
        tar -xJf "$tools/zig.tar.xz" -C "$tools"; then
        rm -f "$tools/zig.tar.xz"
        ln -sf "$tools/zig-x86_64-linux-$ZIG_VERSION/zig" "$HOME/.local/bin/zig"
    else
        echo "cloud-setup: zig $ZIG_VERSION could not be prepared" >&2
        rc=1
    fi
fi

install_rustup() (
    version=1.29.3
    arch=$(uname -m)
    # SHA-256 values from the .sha256 files next to the official binaries.
    case "$arch" in
        x86_64) sha256=b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55 ;;
        aarch64|arm64) arch=aarch64; sha256=9f608c142e0e38a78957d0661ba1dfaf0b6fb37184cfaa4b9a0aa57af6184282 ;;
        *) echo "cloud-setup: no Rustup pin for $arch" >&2; exit 1 ;;
    esac
    tmp=$(mktemp -d "${TMPDIR:-/tmp}/podvm-rustup.XXXXXXXX") || exit 1
    trap 'rm -f -- "$tmp/rustup-init"; rmdir -- "$tmp"' 0
    url=https://static.rust-lang.org/rustup/archive/$version/$arch-unknown-linux-gnu/rustup-init
    curl -qfsSL --proto '=https' --proto-redir '=https' --max-time 60 \
        --max-filesize 33554432 -o "$tmp/rustup-init" "$url" </dev/null || exit 1
    observed=$(sha256sum < "$tmp/rustup-init") || exit 1
    if [ "${observed%% *}" != "$sha256" ]; then
        echo "cloud-setup: Rustup installer failed the SHA-256 check" >&2
        exit 1
    fi
    chmod u+x "$tmp/rustup-init" &&
        timeout 60 "$tmp/rustup-init" -y --no-modify-path --default-toolchain none </dev/null
)

setup_rust() {
    if [ ! -x "$cargo_bin/rustup" ] && ! command -v rustup >/dev/null 2>&1; then
        install_rustup || return 1
    fi
    if [ -x "$cargo_bin/rustup" ]; then
        # A later shell can find these commands on ~/.local/bin.
        for tool in cargo rustc rustup; do
            target=$HOME/.local/bin/$tool
            if [ -x "$cargo_bin/$tool" ] && [ ! -e "$target" ] && [ ! -L "$target" ]; then
                ln -sT -- "$cargo_bin/$tool" "$target" || return 1
            fi
        done
    fi
    # With no toolchain argument, Rustup uses rust-toolchain.toml.
    if ! RUSTUP_AUTO_INSTALL=0 timeout 15 rustup which cargo >/dev/null 2>&1 </dev/null; then
        timeout 600 rustup toolchain install --no-self-update </dev/null
    fi
}

install_dev_tools() {
    missing=
    for pkg in mandoc shellcheck; do
        if ! command -v "$pkg" >/dev/null 2>&1; then
            missing="$missing $pkg"
        fi
    done
    if [ -n "$missing" ] && command -v apt-get >/dev/null 2>&1; then
        apt_sudo=none
        if [ "$(id -u)" = 0 ]; then
            apt_sudo=
        elif command -v sudo >/dev/null 2>&1 && sudo -n -E true 2>/dev/null; then
            apt_sudo="sudo -n -E"
        elif command -v sudo >/dev/null 2>&1 &&
            (sudo -n true 2>/dev/null || sudo -n apt-get --version >/dev/null 2>&1); then
            apt_sudo="sudo -n"
        fi
        if [ "$apt_sudo" != none ]; then
            # shellcheck disable=SC2086
            DEBIAN_FRONTEND=noninteractive timeout 300 $apt_sudo apt-get update -qq </dev/null &&
                DEBIAN_FRONTEND=noninteractive timeout 300 $apt_sudo apt-get install -y -qq \
                    --no-install-recommends $missing </dev/null || rc=1
        fi
    fi
    if command -v cargo >/dev/null 2>&1; then
        if ! cargo fetch --locked --offline >/dev/null 2>&1 </dev/null; then
            timeout 300 cargo fetch --locked </dev/null || rc=1
        fi
    fi
}

if setup_rust; then
    bash "$root/scripts/common/bootstrap-env.sh" rust bloat cc tools openssh || rc=1
else
    echo "cloud-setup: the Rust toolchain could not be prepared" >&2
    bash "$root/scripts/common/bootstrap-env.sh" cc tools openssh
    rc=1
fi

if [ "$dev_tools" = 1 ]; then
    install_dev_tools
fi

# The host facts. Each line is measured here, not assumed.
fact() { printf '  %-22s %s\n' "$1" "$2"; }
echo "cloud-setup: host facts"
fact "kernel" "$(uname -r)"
fact "cpus" "$(nproc 2>/dev/null || echo unknown)"
if [ -e /dev/kvm ]; then fact "/dev/kvm" present; else fact "/dev/kvm" absent; fi
if [ -e /dev/ptmx ]; then fact "/dev/ptmx" present; else fact "/dev/ptmx" absent; fi
if [ -e /dev/fuse ]; then fact "/dev/fuse" present; else fact "/dev/fuse" absent; fi
if unshare -U true 2>/dev/null; then fact "user namespaces" allowed; else fact "user namespaces" denied; fi
if [ "$(id -u)" = 0 ]; then fact "uid" "0 (root)"; else fact "uid" "$(id -u)"; fi
fact "seccomp" "$(grep -m1 '^Seccomp:' /proc/self/status 2>/dev/null | awk '{print $2}')"
fact "disk free (repo)" "$(df -h "$root" 2>/dev/null | awk 'NR==2 {print $4}')"
if [ -n "${HTTPS_PROXY:-${https_proxy:-}}" ]; then fact "egress" "through HTTPS_PROXY"; else fact "egress" "direct"; fi
fact "zig" "$(zig version 2>/dev/null || echo missing)"
fact "rustc" "$(rustc --version 2>/dev/null || echo missing)"
echo "cloud-setup: probe the host with podvm itself: ./target/x86_64-unknown-linux-musl/release/podvm probe"
exit "$rc"
