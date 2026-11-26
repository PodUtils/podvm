#!/bin/sh
# Question: how long does a run of `--podvm-init=image` take on each route,
# with real services, and what does each change of a route gain?
#
# TODO/enter.md T-2010 (the owner, 2026-11-20: "make each route as fast as
# possible", with a measured baseline first). The image is Debian 12 with
# `mariadb-server` and `postgresql` of apt under systemd 252; the script
# makes it once with `podvm run` and `podvm import`.
#
# Run it as root with PODVM_468_USER (default `ubuntu`): root makes the
# stand-in of a delegated cgroup2 directory for the namespace rung, and
# reads the memory of the processes of the user. It needs the network (the
# packages of Debian; HTTPS_PROXY is used where it is set, with the CA file
# of PODVM_468_CA), qemu-system-x86_64, and gVisor's runsc
# (PODVM_468_RUNSC, else the pinned release of experiment 436).
#
# Pinned inputs:
#   podvm   PODVM_468_BIN (default the release build of the workspace)
#   base    mirror.gcr.io/library/debian:12 (its packages are of the day)
#   runsc   release 20261102, by its SHA-512
#
# One row for each route, each from one run (ROUTES selects: `ns`, `runsc`,
# `machine`, `machine-save`, `machine-load`; `machine` boots its guest with
# no saved state, `machine-save` removes each state of the store and makes
# the run that boots and saves one, and `machine-load` is a run from that
# state (TODO/machine.md T-2008); PODVM_468_EXTRA gives more flags of
# `podvm run`,
# PODVM_468_ENV more variables, PODVM_468_VOLMODE=:ro makes the one volume
# read-only, and PODVM_468_LABEL names the row):
#   run       the wall time of `podvm run`, from its start to its exit
#   first     the time until the first byte of the command reaches stdout
#   boot      `systemd-analyze` in the run (userspace)
#   unit      `systemctl start` of a unit of `/bin/true`, the median of 3
#   mariadb   `systemctl restart mariadb.service`
#   postgres  `systemctl restart postgresql@15-main.service`
#   mem       the peak of the sum of the resident memory of the processes
#             of the user and of its subordinate ids, read each 0.5 s
#
# The transcript is appended to, so that a baseline and a tuned run stand
# in one file. Exit 0: each row has its line of the bench and no process
# stays; 1: a row has no line (its run failed or hung), or a process stays;
# 2: the experiment could not run.

set -u
norow=""
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_468_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
OUT="$ROOT/experiments/results/init-bench.txt"
ROUTES="${ROUTES:-ns runsc machine}"
LABEL="${PODVM_468_LABEL:-baseline}"
EXTRA="${PODVM_468_EXTRA:-}"
KEEP="${PODVM_468_WORK:-}"
[ "$(id -u)" -eq 0 ] || { echo "SKIP: run it as root with PODVM_468_USER" >&2; exit 2; }
WHO="${PODVM_468_USER:-ubuntu}"
id "$WHO" >/dev/null 2>&1 || { echo "SKIP: no user $WHO" >&2; exit 2; }
if [ -n "$KEEP" ]; then
	WORK="$KEEP"
	mkdir -p "$WORK"
else
	WORK=$(mktemp -d /var/tmp/podvm-468.XXXXXX)
	cleanup() {
		i=0
		while [ $i -lt 30 ] && ! rm -rf "$WORK" 2>/dev/null; do
			sleep 1
			i=$((i + 1))
		done
	}
	trap cleanup EXIT
fi
chmod 0755 "$WORK"
mkdir -p "$WORK/home" "$WORK/fx" "$WORK/vol" "$(dirname "$OUT")"
cp "$BIN" "$WORK/podvm"
P="$WORK/podvm"
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
as() { runuser -u "$WHO" -- env HOME="$WORK/home" PODVM_STORE="$WORK/store" "$@"; }
IMG=localhost/svc-debian:12

cat >"$WORK/fx/bench.sh" <<'EOF'
echo FIRST
printf '[Service]\nType=oneshot\nExecStart=/bin/true\n' >/etc/systemd/system/t1.service
systemctl daemon-reload
grep -q localhost /etc/hosts 2>/dev/null || echo "127.0.0.1 localhost" >>/etc/hosts
t() {
	s=$(date +%s%N)
	"$@" >/dev/null 2>&1
	e=$(date +%s%N)
	echo $(((e - s) / 1000000))
}
u=$(for i in 1 2 3; do t systemctl start t1.service; done | sort -n | sed -n 2p)
m=$(t systemctl restart mariadb.service)
p=$(t systemctl restart postgresql@15-main.service)
q=$(mariadb -N -B -e 'select 40+2' 2>&1 | tail -1)
r=$(runuser -u postgres -- psql -At -c 'select 40+2' 2>&1 | tail -1)
b=$(systemd-analyze 2>/dev/null | sed -n 's/.* \([0-9.]*m\{0,1\}s\) (userspace).*/\1/p' | head -1)
echo "BENCH state=$(systemctl is-system-running) boot=$b unit=${u}ms mariadb=${m}ms postgres=${p}ms sql=$q,$r uid=$(awk '/^Uid:/{print $2}' /proc/$(systemctl show -p MainPID --value mariadb.service)/status)"
EOF

# The image, once for the work directory.
if ! as "$P" images 2>/dev/null | grep -q 'svc-debian'; then
	[ -n "${PODVM_468_CA:-}" ] && cp "$PODVM_468_CA" "$WORK/vol/ca.crt"
	cat >"$WORK/vol/build.sh" <<'EOF'
set -e
export DEBIAN_FRONTEND=noninteractive
if [ -n "${HTTPS_PROXY:-}" ]; then
	{
		echo "Acquire::http::Proxy \"$HTTPS_PROXY\";"
		echo "Acquire::https::Proxy \"$HTTPS_PROXY\";"
		[ -f /vol/ca.crt ] && echo 'Acquire::https::CaInfo "/vol/ca.crt";'
	} >/etc/apt/apt.conf.d/99proxy
fi
apt-get update -qq >/dev/null
apt-get install -y -qq --no-install-recommends systemd systemd-sysv dbus mariadb-server postgresql >/dev/null
rm -f /etc/apt/apt.conf.d/99proxy
tar --numeric-owner --one-file-system --exclude='./proc/*' --exclude='./sys/*' --exclude='./dev/*' \
	--exclude='./tmp/*' --exclude=./vol -cf /vol/rootfs.tar -C / . 2>/dev/null
EOF
	chown -R "$WHO" "$WORK"
	as env PODVM_USERNS_RANGE=1 HTTPS_PROXY="${HTTPS_PROXY:-}" timeout 1200 "$P" run --rm -e HTTPS_PROXY \
		-v "$WORK/vol:/vol" mirror.gcr.io/library/debian:12 sh /vol/build.sh >/dev/null 2>"$WORK/build.err" &&
		as timeout 600 "$P" import "$WORK/vol/rootfs.tar" "$IMG" >/dev/null 2>>"$WORK/build.err" ||
		{ echo "the image: the build FAILED: COULD NOT RUN" >&2; tail -12 "$WORK/build.err" >&2; exit 2; }
	rm -f "$WORK/vol/rootfs.tar"
fi
chown -R "$WHO" "$WORK/fx" "$WORK/home" "$WORK/podvm" 2>/dev/null

# gVisor.
RUNSC="${PODVM_468_RUNSC:-$WORK/gvisor/runsc}"
if [ ! -x "$RUNSC" ]; then
	REL=20261102
	SUM=2e6671bb2d033b871c5811ab57e16ac570988b0e0054c525f911cfaf191bab3ca4efb08e0a98daa4b9e7d248f9e5776ef2ef2f8a6b82d829646a42924c98bb05
	mkdir -p "$WORK/gvisor"
	curl -qfsSL -o "$WORK/gvisor.tar.bz2" "https://storage.googleapis.com/gvisor/releases/release/$REL/x86_64/gvisor.tar.bz2" &&
		echo "$SUM  $WORK/gvisor.tar.bz2" | sha512sum -c - >/dev/null 2>&1 &&
		tar -xjf "$WORK/gvisor.tar.bz2" -C "$WORK/gvisor" runsc gvisor-bin && chmod -R a+rX "$WORK/gvisor"
	rm -f "$WORK/gvisor.tar.bz2"
fi

# The stand-in of a delegated cgroup2 directory for the namespace rung.
CGROOT=$(awk '$9 == "cgroup2" || $10 == "cgroup2" {print $5; exit}' /proc/self/mountinfo)
STANDIN="$CGROOT/podvm-468"
low=$(awk -F: -v u="$WHO" '$1==u{print $2; exit}' /etc/subuid)
uid=$(id -u "$WHO")

one() { # the route, then the prefix of the command
	route=$1
	shift
	# The helpers of the run before (the removal of its tree, the fill of
	# the spare tree) end first: each row is a run with a spare tree.
	i=0
	while [ $i -lt 120 ] && pgrep -u "$WHO" -x podvm >/dev/null 2>&1; do
		sleep 0.5
		i=$((i + 1))
	done
	: >"$WORK/mem"
	(
		peak=0
		while [ ! -e "$WORK/stop" ]; do
			now=$(ps -eo uid=,rss= | awk -v u="$uid" -v l="${low:-0}" '$1==u || (l>0 && $1>=l && $1<l+65536) {s+=$2} END{print int(s/1024)}')
			[ "$now" -gt "$peak" ] && peak=$now && echo "$peak" >"$WORK/mem"
			sleep 0.5
		done
	) &
	sampler=$!
	rm -f "$WORK/stop" "$WORK/first"
	: >"$WORK/out"
	start=$(date +%s%N)
	# The first byte of the command: a watcher reads the size of the file.
	# The streams of podvm are files, not pipes: a helper of podvm that
	# outlives the run (the removal of the tree) holds a pipe open.
	(
		while [ ! -s "$WORK/out" ] && [ ! -e "$WORK/stop" ]; do sleep 0.02; done
		date +%s%N >"$WORK/first"
	) &
	watcher=$!
	# shellcheck disable=SC2086
	"$@" timeout 900 "$P" run --rm --podvm-init=image $TIER $EXTRA -v "$WORK/fx:/fx${PODVM_468_VOLMODE:-}" "$IMG" sh /fx/bench.sh \
		>"$WORK/out" 2>"$WORK/err" </dev/null
	end=$(date +%s%N)
	: >"$WORK/stop"
	wait "$watcher" "$sampler" 2>/dev/null
	rm -f "$WORK/stop"
	first=$(cat "$WORK/first" 2>/dev/null || echo "$start")
	b=$(grep '^BENCH' "$WORK/out" | sed 's/^BENCH //')
	# A row with no line of the bench is a run that failed or hung: the
	# experiment fails (TODO/machine.md T-2048: the rows mach-sav and
	# mach-lod of kvm.yml run 44786700578 hung 900 s, and the step passed).
	[ -n "$b" ] || norow="$norow $LABEL/$route"
	say "$(printf '%-22s %-8s run=%5.1fs first=%5.1fs mem=%5sMiB %s' "$LABEL" "$route" \
		"$(echo "$start $end" | awk '{print ($2-$1)/1e9}')" "$(echo "$start $first" | awk '{print ($2-$1)/1e9}')" \
		"$(cat "$WORK/mem")" "${b:-NO BENCH LINE: $(tail -1 "$WORK/err" | cut -c1-120)}")"
	# TODO/machine.md T-2008: each line of podvm on the saved state of the
	# guest (a save that failed, a load that booted) stands in the transcript.
	grep -h 'T-2008' "$WORK/err" 2>/dev/null | head -3 | while IFS= read -r l; do
		say "$(printf '%-22s %-8s %s' "$LABEL" "$route" "$(echo "$l" | cut -c1-300)")"
	done
}

say "# $(date -u +%Y-%m-%dT%H:%M:%SZ) podvm $("$P" version 2>/dev/null | cut -d' ' -f2), host $(uname -r), $(nproc) CPUs, kvm $([ -w /dev/kvm ] && echo yes || echo no), label $LABEL${EXTRA:+, flags $EXTRA}"
for r in $ROUTES; do
	# Each route names its tier: a host whose user manager gives a delegated
	# cgroup (a hosted runner) takes the namespace rung with no tier (call C26).
	TIER=
	case $r in
	ns)
		[ -n "$CGROOT" ] || { say "$LABEL ns: no cgroup2 mount"; continue; }
		mkdir -p "$STANDIN" && chown "$WHO" "$STANDIN" "$STANDIN/cgroup.procs" "$STANDIN/cgroup.subtree_control"
		cat >"$WORK/deleg.sh" <<EOF
#!/bin/sh
echo \$\$ > "$STANDIN/cgroup.procs"
exec "\$@"
EOF
		chmod 755 "$WORK/deleg.sh"
		one ns "$WORK/deleg.sh" runuser -u "$WHO" -- env HOME="$WORK/home" PODVM_STORE="$WORK/store"
		echo $$ >"$CGROOT/cgroup.procs"
		find "$STANDIN" -mindepth 1 -depth -type d -exec rmdir {} \; 2>/dev/null
		rmdir "$STANDIN" 2>/dev/null
		;;
	runsc) TIER=--podvm-tier=runsc; one runsc as env PODVM_RUNSC="$RUNSC" ${PODVM_468_ENV:-} ;;
	machine) TIER=--podvm-tier=machine; one machine as env PODVM_RUNSC=/nonexistent/runsc PODVM_MACHINE_STATE=0 ${PODVM_468_ENV:-} ;;
	machine-save)
		find "$WORK/store/machine" -maxdepth 1 -name 'state-*' -exec rm -f {} + 2>/dev/null
		TIER=--podvm-tier=machine
		# The saved state keeps its root in the initramfs (T-1994).
		one mach-sav as env PODVM_RUNSC=/nonexistent/runsc PODVM_MACHINE_ROOT=initramfs ${PODVM_468_ENV:-}
		;;
	machine-load)
		TIER=--podvm-tier=machine
		one mach-lod as env PODVM_RUNSC=/nonexistent/runsc PODVM_MACHINE_ROOT=initramfs ${PODVM_468_ENV:-}
		st=$(find "$WORK/store/machine" -maxdepth 1 -name 'state-*.vm' -exec stat -c %s {} + 2>/dev/null | awk '{s+=$1} END{print int(s/1048576)}')
		bad=$(find "$WORK/store/machine" -maxdepth 1 -name 'state-*.bad' 2>/dev/null | wc -l)
		say "$(printf '%-22s %-8s the state files: %s MiB; markers of a failed state: %s' "$LABEL" mach-lod "${st:-0}" "$bad")"
		;;
	esac
done
sleep 2
# The user manager of the host and its session bus are not of a run.
left=$(ps -u "$WHO" -o pid=,stat=,args= 2>/dev/null | grep -E 'qemu-system|runsc|systemd|guest-init' |
	grep -v -e ' Z' -e 'systemd --user' -e 'dbus-daemon --session' || true)
[ -z "$left" ] || say "A PROCESS STAYS: $left"
[ -z "$norow" ] || say "ROWS WITH NO LINE OF THE BENCH:$norow"
[ -z "$norow" ] && [ -z "$left" ] || exit 1
exit 0
