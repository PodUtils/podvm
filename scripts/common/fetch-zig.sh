#!/bin/sh
# Download a Zig archive with the SHA-256 pin supplied by the caller.
set -eu

arch=$1
version=$2
digest=$3
archive=$4
direct_limit=900

if [ "$arch" = x86_64 ]; then
    # podcrates supplies the amd64 mirror order and verifies each download.
    revision=54427f6677b17cc96638f0fafc44b615e467fb4f
    mirror_tools=$(mktemp -d "${TMPDIR:-/tmp}/podvm-zig.XXXXXXXX")
    trap 'rm -rf -- "$mirror_tools"' EXIT
    base="https://raw.githubusercontent.com/PodUtils/podcrates/$revision/scripts"
    if curl -qfsSL --max-time 25 -o "$mirror_tools/fetch-zig.sh" "$base/fetch-zig.sh" </dev/null &&
       curl -qfsSL --max-time 25 -o "$mirror_tools/zig-mirrors.txt" "$base/zig-mirrors.txt" </dev/null; then
        ZIG_SOURCE=podvm timeout --kill-after=5 300 sh "$mirror_tools/fetch-zig.sh" \
            "$version" "$digest" "$archive" "$mirror_tools/zig-mirrors.txt" </dev/null
        exit $?
    fi
    echo "fetch-zig: cannot read the mirror list. Use ziglang.org." >&2
    direct_limit=120
fi

url="https://ziglang.org/download/$version/zig-$arch-linux-$version.tar.xz"
curl -qfsSL --proto '=https' --proto-redir '=https' --max-time "$direct_limit" \
    -o "$archive" "$url" </dev/null
observed=$(sha256sum <"$archive")
if [ "${observed%% *}" != "$digest" ]; then
    rm -f -- "$archive"
    echo "fetch-zig: the $arch archive failed the SHA-256 check" >&2
    exit 1
fi
