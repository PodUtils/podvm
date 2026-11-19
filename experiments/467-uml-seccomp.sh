#!/bin/sh
# Question: on a host that denies ptrace, unshare, setns, chroot and mount
# (the userland rung), does a User-Mode Linux kernel in its seccomp mode
# boot the init of an image (systemd) as PID 1 for a user with no root,
# with units that run as their own users, and how long does a run take?
#
# TODO/enter.md T-2005. This is a measure, not a tier of podvm: the kernel
# is a stock one that the script builds, and the harness is in this file.
# A kernel that podvm carries is shared code (AGENTS.md, section 2a).
#
# Run it as a user that is not root, or as root with PODVM_467_USER
# (default `ubuntu`). It needs gcc, make, flex, bison, bc, python3 and
# curl, about 3 GB of disk, and about 6 minutes (the kernel build is 3).
#
# Pinned inputs:
#   kernel  linux-7.3.5.tar.xz of cdn.kernel.org, by its SHA-256 (seccomp
#           mode is in 6.16, SMP in 6.19, the SIGCHLD fix in 7.0)
#   podvm   PODVM_467_BIN (default the release build of the workspace):
#           it pulls and extracts the image, and gives the sidecar
#   image   ghcr.io/pkgforge-dev/archlinux:latest at the digest of
#           scripts/common/distro-matrix.sh (systemd 261)
#
# Clauses:
#   1. the kernel builds for ARCH=um, and `linux --version` gives 7.3.5;
#   2. the launcher works: under it `unshare(CLONE_NEWUSER)` gives EPERM,
#      and the same kernel with `seccomp=off` ends at "ptrace: Operation
#      not permitted";
#   3. the builder: under the launcher, a guest with the tree of the image
#      as its hostfs root writes an ext4 base with the owners of the
#      sidecar: `/usr/bin/mount` is 0:0 with its set-user-ID bit;
#   4. the run: under the launcher, `seccomp=on ncpus=4`, the base behind a
#      copy-on-write file: the check of experiment 465 exits 7 (systemd is
#      PID 1, the state is running or degraded, the units of mysql,
#      postgres, www-data and nobody run as those uids);
#   5. the base keeps its SHA-256 over the run, and no process stays.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_467_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
OUT="$ROOT/experiments/results/uml-seccomp.txt"
IMG='ghcr.io/pkgforge-dev/archlinux:latest@sha256:b2507f1964270cab3cc190aa8df858521f6a7e45e969146a012877891d5dfc9b'
KVER=7.3.5
KSUM=8f293f83d54f0b7b9be56863ebc5e4ea79d73e530b4ae55c572be97ef39f0990
[ "$(uname -m)" = x86_64 ] || { echo "SKIP: the measure is x86_64" >&2; exit 2; }
for t in gcc make flex bison bc python3 curl; do
	command -v "$t" >/dev/null 2>&1 || { echo "SKIP: no $t" >&2; exit 2; }
done
WORK=$(mktemp -d /var/tmp/podvm-467.XXXXXX)
cleanup() {
	i=0
	while [ $i -lt 30 ] && ! rm -rf "$WORK" 2>/dev/null; do
		sleep 1
		i=$((i + 1))
	done
}
trap cleanup EXIT
chmod 0755 "$WORK"
mkdir -p "$WORK/home" "$WORK/fx" "$WORK/tmp" "$(dirname "$OUT")"
cp "$BIN" "$WORK/podvm"
: >"$OUT"
fail=0
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }
if [ "$(id -u)" -eq 0 ]; then
	WHO="${PODVM_467_USER:-ubuntu}"
	id "$WHO" >/dev/null 2>&1 || { echo "SKIP: root needs PODVM_467_USER" >&2; exit 2; }
	chown -R "$WHO" "$WORK"
	as() { runuser -u "$WHO" -- env HOME="$WORK/home" PODVM_STORE="$WORK/store" TMPDIR="$WORK/tmp" "$@"; }
else
	WHO=$(id -un)
	as() { env HOME="$WORK/home" PODVM_STORE="$WORK/store" TMPDIR="$WORK/tmp" "$@"; }
fi
say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "user      $WHO (uid $(id -u "$WHO"))"
say "host      Linux $(uname -r), $(nproc) CPUs"

# The check of experiment 465, and the job that runs it in the guest.
sed -n "/^cat >\"\$WORK\/fx\/check.sh\" <<'EOF'\$/,/^EOF\$/p" "$ROOT/experiments/465-init-image.sh" |
	sed '1d;$d' >"$WORK/fx/check.sh"
[ -s "$WORK/fx/check.sh" ] || { echo "the check of experiment 465 was not found" >&2; exit 2; }

# Clause 1: the kernel.
start=$(date +%s)
curl -qfsSL -o "$WORK/linux.tar.xz" "https://cdn.kernel.org/pub/linux/kernel/v7.x/linux-$KVER.tar.xz" &&
	echo "$KSUM  $WORK/linux.tar.xz" | sha256sum -c - >/dev/null 2>&1 || {
	echo "the kernel source: the fetch or its SHA-256 FAILED: COULD NOT RUN" >&2
	exit 2
}
chown "$WHO" "$WORK/linux.tar.xz" 2>/dev/null
as tar -xJf "$WORK/linux.tar.xz" -C "$WORK" && rm -f "$WORK/linux.tar.xz"
K="$WORK/linux-$KVER"
(
	cd "$K" || exit 1
	as make ARCH=um defconfig >/dev/null 2>&1 || exit 1
	as ./scripts/config -e SMP --set-val NR_CPUS 8 -e CGROUPS -e MEMCG -e CGROUP_PIDS \
		-e CGROUP_SCHED -e CGROUP_FREEZER -e CPUSETS -e NAMESPACES -e USER_NS -e PID_NS \
		-e CGROUP_NS -e DEVTMPFS -e DEVTMPFS_MOUNT -e TMPFS -e TMPFS_POSIX_ACL -e TMPFS_XATTR \
		-e FHANDLE -e AUTOFS_FS -e INOTIFY_USER -e SIGNALFD -e TIMERFD -e EPOLL -e EXT4_FS \
		-e EXT4_FS_POSIX_ACL -e HOSTFS -e BLK_DEV_UBD -e UNIX -e INET -e IPV6 -e SECCOMP \
		-e BPF_SYSCALL -e CGROUP_BPF -e OVERLAY_FS
	as make ARCH=um olddefconfig >/dev/null 2>&1 || exit 1
	as make ARCH=um -j"$(nproc)" >"$WORK/build.log" 2>&1
) || { bad "clause 1: the kernel build"; tail -5 "$WORK/build.log" | tee -a "$OUT"; }
UML="$WORK/linux.uml"
cp "$K/linux" "$UML" 2>/dev/null && strip "$UML" && chown "$WHO" "$UML"
rm -rf "$K"
ver=$(as "$UML" --version 2>/dev/null)
say "clause 1  the kernel: $ver, $(stat -c %s "$UML" 2>/dev/null) bytes stripped, built in $(($(date +%s) - start)) s"
[ "$ver" = "$KVER" ] || { bad "clause 1: no kernel"; say "A CLAUSE FAILED"; exit 1; }

# The helpers of the guest (no set-user-ID program of a hostfs tree keeps
# root: its owner there is the user of the host), and the launcher.
cat >"$WORK/mnt.c" <<'EOF'
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <sys/mount.h>
#include <sys/reboot.h>
int main(int argc, char **argv) {
	if (argc == 2 && !strcmp(argv[1], "--poweroff")) { sync(); reboot(RB_POWER_OFF); return 1; }
	if (argc == 2 && !strcmp(argv[1], "--init")) { execl("/bin/sh", "sh", "/uml-build.sh", (char *)0); return 127; }
	if (argc < 4) return 2;
	if (mount(argv[1], argv[2], argv[3], 0, argc > 4 ? argv[4] : NULL)) { perror(argv[2]); return 1; }
	return 0;
}
EOF
cat >"$WORK/init.c" <<'EOF'
#include <unistd.h>
int main(void) { execl("/uml-mnt", "uml-mnt", "--init", (char *)0); return 127; }
EOF
cat >"$WORK/deny.c" <<'EOF'
#include <errno.h>
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <stddef.h>
#include <stdio.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <unistd.h>
int main(int argc, char **argv) {
	struct sock_filter f[] = {
		BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, nr)),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_ptrace, 5, 0),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_unshare, 4, 0),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_setns, 3, 0),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_chroot, 2, 0),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_mount, 1, 0),
		BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
		BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EPERM),
	};
	struct sock_fprog p = {sizeof f / sizeof f[0], f};
	if (argc < 2 || prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) || prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &p)) return 2;
	execvp(argv[1], argv + 1);
	return 127;
}
EOF
gcc -O2 -static -o "$WORK/uml-mnt" "$WORK/mnt.c" && gcc -O2 -static -o "$WORK/uml-init" "$WORK/init.c" &&
	gcc -O2 -static -o "$WORK/deny" "$WORK/deny.c" || { echo "the helpers: COMPILE FAILED: COULD NOT RUN" >&2; exit 2; }
chmod 755 "$WORK/uml-mnt" "$WORK/uml-init" "$WORK/deny"
D="$WORK/deny"

# Clause 2: the launcher, and the ptrace mode under it.
as "$D" unshare -U true >/dev/null 2>"$WORK/unshare.err"
u=$?
as timeout 60 "$D" "$UML" seccomp=off mem=256M root=/dev/root rootfstype=hostfs hostfs=/ init=/bin/true \
	con=null con0=fd:0,fd:1 </dev/null >"$WORK/ptrace.out" 2>&1
line=$(grep -a -m1 'ptrace' "$WORK/ptrace.out" | cut -c1-80)
say "clause 2  under the launcher: unshare -U gives exit $u; seccomp=off gives \"$line\""
{ [ "$u" -ne 0 ] && grep -aq 'ptrace: Operation not permitted' "$WORK/ptrace.out"; } ||
	bad "clause 2: the launcher denies nothing, or the ptrace mode ran"

# The tree and the sidecar of the image.
as timeout 600 "$WORK/podvm" pull -q "$IMG" >/dev/null 2>&1
as timeout 600 "$WORK/podvm" extract "$IMG" >/dev/null 2>&1
SIDE=$(ls "$WORK"/store/rootfs/*/.meta.jsonl 2>/dev/null | head -1)
TREE="$(dirname "${SIDE:-/nonexistent/x}")/rootfs"
[ -d "$TREE" ] || { bad "the extraction of $IMG"; say "A CLAUSE FAILED"; exit 1; }
python3 -I - "$SIDE" >"$WORK/uml-chown.sh" <<'EOF'
import json, sys
for l in open(sys.argv[1]):
    try:
        v = json.loads(l)
    except ValueError:
        continue
    if v.get("path") and (v.get("uid"), v.get("gid")) != (0, 0):
        p = v["path"].lstrip("/").replace("'", "'\\''")
        print("chown -h %d:%d '/mnt/%s'" % (v["uid"], v["gid"], p))
EOF
cat >"$WORK/uml-job.sh" <<EOF
#!/bin/sh
i=0
while [ \$i -lt 600 ]; do
	case "\$(systemctl is-system-running 2>/dev/null)" in running | degraded) break ;; esac
	i=\$((i + 1))
	sleep 0.1
done
echo "UMLJOB up at \$(cut -d' ' -f1 /proc/uptime) s, \$(nproc) CPUs, \$(systemd-analyze 2>/dev/null | head -1)"
/uml-mnt none /fx hostfs $WORK/fx
sh /fx/check.sh
echo "UMLJOB exit \$?"
/uml-mnt --poweroff
EOF
cat >"$WORK/uml-build.sh" <<'EOF'
#!/bin/sh
/uml-mnt proc /proc proc
mke2fs -q -t ext4 -F /dev/ubda || echo "BUILD mke2fs failed"
/uml-mnt /dev/ubda /mnt ext4 || echo "BUILD mount failed"
for d in $(ls -A /); do
	case $d in proc | sys | dev | mnt | tmp | run | uml-*) ;; *) cp -a --no-preserve=ownership "/$d" /mnt/ ;; esac
done
mkdir -p /mnt/proc /mnt/sys /mnt/dev /mnt/run /mnt/tmp /mnt/mnt /mnt/fx
chmod 1777 /mnt/tmp
sh /uml-chown.sh 2>&1 | head -3
: >/mnt/etc/machine-id
cp /uml-mnt /uml-job.sh /mnt/
cat >/mnt/etc/systemd/system/uml-job.service <<'U'
[Service]
Type=simple
StandardOutput=tty
StandardError=tty
TTYPath=/dev/console
ExecStart=/bin/sh /uml-job.sh
U
mkdir -p /mnt/etc/systemd/system/multi-user.target.wants
ln -sf ../uml-job.service /mnt/etc/systemd/system/multi-user.target.wants/uml-job.service
echo "BUILD done $(cut -d' ' -f1 /proc/uptime) s; mount $(stat -c '%u:%g %a' /mnt/usr/bin/mount); used $(df -m /mnt | awk 'NR==2{print $3}') MiB"
/uml-mnt --poweroff
EOF
for f in uml-mnt uml-init uml-chown.sh uml-job.sh uml-build.sh; do
	cp "$WORK/$f" "$TREE/$f" && chown "$WHO" "$TREE/$f" 2>/dev/null
done

# Clause 3: the builder.
BASE="$WORK/base.ext4"
as truncate -s 3G "$BASE"
start=$(date +%s)
as timeout 900 "$D" "$UML" seccomp=on ncpus=4 mem=2G root=/dev/root rootfstype=hostfs "hostfs=$TREE" rw \
	"ubd0=$BASE" init=/uml-init con=null con0=fd:0,fd:1 quiet </dev/null >"$WORK/build.out" 2>&1
line=$(grep -a 'BUILD' "$WORK/build.out" | tail -1)
say "clause 3  the builder in $(($(date +%s) - start)) s: $line"
echo "$line" | grep -q 'mount 0:0 4755' || { bad "clause 3: the base has no owners of the image"; tail -5 "$WORK/build.out" | tee -a "$OUT"; }
before=$(sha256sum "$BASE" | cut -d' ' -f1)

# Clause 4: the run.
start=$(date +%s)
as timeout 600 "$D" "$UML" seccomp=on ncpus=4 mem=2G "ubd0=$WORK/run.cow,$BASE" root=/dev/ubda rw \
	init=/sbin/init con=null con0=fd:0,fd:1 quiet systemd.show_status=0 </dev/null >"$WORK/run.out" 2>&1
say "clause 4  the run in $(($(date +%s) - start)) s under the launcher (seccomp=on ncpus=4)"
grep -a 'UMLJOB\|CHECK\|VERDICT' "$WORK/run.out" | sed 's/^.*UMLJOB up/UMLJOB up/; s/^/    /' | cut -c1-200 | tee -a "$OUT"
grep -aq '^UMLJOB exit 7' "$WORK/run.out" || { bad "clause 4: the check did not exit 7"; tail -5 "$WORK/run.out" | cut -c1-200 | tee -a "$OUT"; }

# Clause 5.
after=$(sha256sum "$BASE" | cut -d' ' -f1)
left=$(ps -u "$WHO" -o pid=,args= 2>/dev/null | grep 'linux\.uml' | grep -v grep || true)
say "clause 5  the base: ${before%????????????????????????????????????????????????} before, ${after%????????????????????????????????????????????????} after; the copy-on-write file: $(du -m "$WORK/run.cow" 2>/dev/null | cut -f1) MiB; processes that stay: ${left:-none}"
{ [ "$before" = "$after" ] && [ -z "$left" ]; } || bad "clause 5: the base changed, or a process stays"

if [ "$fail" -eq 0 ]; then
	say "UML IN SECCOMP MODE BOOTS THE INIT WITH NO PTRACE AND NO NAMESPACE"
	exit 0
fi
say "A CLAUSE FAILED"
exit 1
