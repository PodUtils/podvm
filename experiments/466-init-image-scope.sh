#!/bin/sh
# Question: on a host with systemd, where the shell of a user is in a
# cgroup of root, does `run --podvm-init=image` get a delegated cgroup2
# subtree from the user manager by itself, and boot the init of the image
# on the namespace rung?
#
# TODO/enter.md T-2004. The cloud host runs no systemd, so the host of this
# experiment is a guest of the machine tier with `--podvm-init=image`
# (T-2002): systemd of the image is its init. In that guest, a new user
# (uid 1000, a subordinate range, `user@1000.service`) runs podvm from a
# transient service of the system manager: a cgroup of root, as the
# `session-N.scope` of a login.
#
# Run it as a user that is not root, or as root with PODVM_466_USER
# (default `ubuntu`). It needs qemu-system-x86_64; under TCG it takes
# about 3 minutes.
#
# Pinned inputs:
#   podvm   PODVM_466_BIN (default the release build of the workspace)
#   image   ghcr.io/pkgforge-dev/archlinux:latest at the digest of
#           scripts/common/distro-matrix.sh, for the guest and for the
#           run in it
#
# The setup in the guest that a real host has already: the user, its
# range, its user manager, and `newuidmap` with its set-user-ID bit (the
# root of the machine tier keeps no file capability, T-1999).
#
# Clauses:
#   1. the user manager answers, and the cgroup of podvm before the run is
#      not under `user@1000.service`;
#   2. the run in the guest gives exit 7 from the check of experiment 465,
#      with the range in its `uid_map`, and it names no machine tier;
#   3. the scope `podvm-init-*` of the user manager ends with the last
#      process of the run (the helpers of podvm): none stays 90 s after it.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_466_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
OUT="$ROOT/experiments/results/init-image-scope.txt"
IMG='ghcr.io/pkgforge-dev/archlinux:latest@sha256:b2507f1964270cab3cc190aa8df858521f6a7e45e969146a012877891d5dfc9b'
WORK=$(mktemp -d /var/tmp/podvm-466.XXXXXX)
cleanup() {
	i=0
	while [ $i -lt 30 ] && ! rm -rf "$WORK" 2>/dev/null; do
		sleep 1
		i=$((i + 1))
	done
}
trap cleanup EXIT
chmod 0755 "$WORK"
mkdir -p "$WORK/home" "$WORK/host/fx" "$(dirname "$OUT")"
cp "$BIN" "$WORK/podvm"
cp "$BIN" "$WORK/host/podvm"
: >"$OUT"
fail=0
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }

# The check of experiment 465, from its script.
sed -n "/^cat >\"\$WORK\/fx\/check.sh\" <<'EOF'\$/,/^EOF\$/p" "$ROOT/experiments/465-init-image.sh" |
	sed '1d;$d' >"$WORK/host/fx/check.sh"
[ -s "$WORK/host/fx/check.sh" ] || {
	echo "the check of experiments/465-init-image.sh was not found" >&2
	exit 2
}

cat >"$WORK/host/inner.sh" <<'EOF'
#!/bin/sh
echo "NEST cgroup-before $(sed -n 's/^0:://p' /proc/self/cgroup)"
podvm load -i /host/image.tar >/dev/null 2>&1
echo "NEST load $?"
img=$(podvm images 2>/dev/null | awk 'NR==2{print $1":"$2}')
podvm run --rm --podvm-init=image -v /host/fx:/fx "$img" sh /fx/check.sh 2>/tmp/inner.err
echo "NEST inner-rc $?"
grep -c 'takes the machine tier' /tmp/inner.err | sed 's/^/NEST machine-tier-lines /'
head -c 600 /tmp/inner.err | sed 's/^/NEST stderr: /'
# The scope ends with its last process: the helpers of podvm (the removal
# of the tree, the fill of the spare tree) run for some time after the run.
i=0
while [ $i -lt 90 ] && systemctl --user list-units --type=scope --all --no-legend 2>/dev/null | grep -q 'podvm-init-'; do
	[ $i -eq 0 ] && systemctl --user list-units --type=scope --all --no-legend 2>/dev/null | grep 'podvm-init-' | sed 's/^/NEST scope-at-the-end: /'
	sleep 1
	i=$((i + 1))
done
echo "NEST scopes-after $(systemctl --user list-units --type=scope --all --no-legend 2>/dev/null | grep -c 'podvm-init-') (waited $i s)"
EOF
cat >"$WORK/host/nest.sh" <<'EOF'
#!/bin/sh
chmod u+s /usr/bin/newuidmap /usr/bin/newgidmap
useradd -m -u 1000 u 2>/dev/null
grep -q '^u:' /etc/subuid || {
	echo u:100000:65536 >>/etc/subuid
	echo u:100000:65536 >>/etc/subgid
}
systemctl start user@1000.service
i=0
while [ ! -S /run/user/1000/bus ] && [ $i -lt 150 ]; do
	sleep 0.2
	i=$((i + 1))
done
echo "NEST systemd $(systemctl --version | head -1)"
echo "NEST user-manager $(systemctl is-active user@1000.service) bus=$([ -S /run/user/1000/bus ] && echo yes || echo no)"
install -m 755 /host/podvm /usr/local/bin/podvm
install -m 755 /host/inner.sh /usr/local/bin/inner.sh
systemd-run --wait --pipe --collect --quiet --uid=1000 --gid=1000 \
	-E HOME=/home/u -E XDG_RUNTIME_DIR=/run/user/1000 -E PODVM_STORE=/home/u/store \
	-E PATH=/usr/local/bin:/usr/bin /usr/local/bin/inner.sh
echo "NEST systemd-run $?"
EOF

if [ "$(id -u)" -eq 0 ]; then
	WHO="${PODVM_466_USER:-ubuntu}"
	id "$WHO" >/dev/null 2>&1 || {
		echo "SKIP: root needs a user with no root in PODVM_466_USER" >&2
		exit 2
	}
	chown -R "$WHO" "$WORK"
	as() { runuser -u "$WHO" -- env HOME="$WORK/home" PODVM_STORE="$WORK/store" "$@"; }
else
	WHO=$(id -un)
	as() { env HOME="$WORK/home" PODVM_STORE="$WORK/store" "$@"; }
fi
command -v qemu-system-x86_64 >/dev/null 2>&1 || {
	echo "SKIP: no qemu-system-x86_64" >&2
	exit 2
}
P="$WORK/podvm"
say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "podvm     $("$P" version 2>/dev/null)"
say "user      $WHO (uid $(id -u "$WHO"))"
say "kvm       $([ -w /dev/kvm ] && echo yes || echo 'no (the guest runs under TCG)')"
as timeout 600 "$P" pull -q "$IMG" >/dev/null 2>"$WORK/pull.err" || bad "the pull of $IMG"
as timeout 300 "$P" save -o "$WORK/host/image.tar" "$IMG" 2>"$WORK/save.err" || bad "the save of $IMG"

start=$(date +%s)
as timeout 1500 "$P" run --rm --podvm-tier=machine --podvm-mem 6G --podvm-init=image \
	-v "$WORK/host:/host" "$IMG" sh /host/nest.sh >"$WORK/guest.out" 2>"$WORK/guest.err"
rc=$?
say "guest     rc $rc in $(($(date +%s) - start)) s"
sed 's/^/    /' "$WORK/guest.out" | tee -a "$OUT"
[ "$rc" -eq 0 ] || {
	bad "the guest gave $rc"
	sed 's/^/    stderr: /' "$WORK/guest.err" | tail -8 | tee -a "$OUT"
}
got() { grep -q "$1" "$WORK/guest.out"; }

# Clause 1.
{ got '^NEST user-manager active bus=yes' && got '^NEST cgroup-before /system.slice/'; } ||
	bad "clause 1: no user manager, or podvm started under it already"
# Clause 2.
{ got '^NEST inner-rc 7$' && got 'CHECK uid_map  0 1000 1 1 100000 65536' && got '^NEST machine-tier-lines 0$'; } ||
	bad "clause 2: the run in the guest did not boot the init on the namespace rung"
# Clause 3.
got '^NEST scopes-after 0 ' || bad "clause 3: a scope of the run stays"

sleep 2
left=$(ps -u "$WHO" -o pid=,stat=,args= 2>/dev/null | grep -E 'qemu-system' | grep -v ' Z' || true)
say "emulators that stay: ${left:-none}"
[ -z "$left" ] || bad "an emulator stays"
if [ "$fail" -eq 0 ]; then
	say "THE USER MANAGER GIVES THE SUBTREE"
	exit 0
fi
say "A CLAUSE FAILED"
exit 1
