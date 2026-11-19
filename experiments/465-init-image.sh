#!/bin/sh
# Question: does `run --podvm-init=image` give a caller with no root the
# init of the image (systemd) as PID 1, units that run as their own users,
# and the exit code of the command, from each rung of the host?
#
# TODO/enter.md T-2002. The table of the owner (2026-11-19) said "not
# possible" for the userland rung and "limited: one uid" for the namespace
# rung. Three routes answer it, and a run takes the first that the host
# gives: the namespace rung with the subordinate id range and a delegated
# cgroup2 subtree (the init is PID 1 of a PID namespace of the host); the
# runsc tier (T-2007: PID 1 of a gVisor sandbox, which needs a user
# namespace and no more); a guest of the machine tier under KVM or TCG
# (PID 1 of a PID namespace of the guest).
#
# Run it as a user that is not root, or as root with PODVM_465_USER (the
# user that runs podvm, default `ubuntu`; root then makes the store, and a
# stand-in of a delegated cgroup2 directory for clause 1). The user needs
# `newuidmap`, `newgidmap` and a range in /etc/subuid and /etc/subgid for
# clause 1, qemu-system-x86_64 for clauses 2 and 3, and for clause 8
# gVisor's runsc (PODVM_465_RUNSC, else the pinned release of experiment
# 436, which the script fetches and checks by its SHA-512).
#
# Pinned inputs:
#   podvm   PODVM_465_BIN (default the release build of the workspace)
#   image   ghcr.io/pkgforge-dev/archlinux:latest at the digest of
#           scripts/common/distro-matrix.sh (systemd 261)
#
# The check (the command of each run) passes with exit 7:
#   a. /proc/1/comm is `systemd`, and `systemctl is-system-running` gives
#      `running` or `degraded`;
#   b. a unit with `User=` of mysql, postgres, www-data and nobody starts
#      with `systemctl start`; the uid of its main process is the uid of
#      the user, and the file that it writes belongs to that uid;
#   c. `systemctl stop` ends a unit.
#
# Clauses:
#   1. the namespace rung, from a delegated cgroup2 directory: the check
#      exits 7, the `uid_map` holds the range, and the run names no other
#      tier. Skipped where the user has no delegated directory and the
#      script is not root;
#   2. the namespace rung of a host that delegates no cgroup2 subtree: the
#      run names the missing part and the machine tier, and the check exits
#      7. Skipped where the cgroup of the script is delegated;
#   3. the userland rung (`PODVM_USERNS=0` and a volume): the run names the
#      machine tier, and the check exits 7;
#   4. the userland rung with no init, for the record of why it has none:
#      PID 1 is a process of the host, and `setresuid` to another uid fails;
#   5. `run -d --podvm-init=image` and `--podvm-tier=syscall
#      --podvm-init=image` refuse by name with 125;
#   6. after the runs: no emulator and no systemd of the user stays, and no
#      cgroup `podvm-init-*` stays in the delegated directory;
#   7. a SIGKILL of podvm during a run (TODO/supervise.md T-2006): no
#      process of the run stays, for a run with podvm's init and, from the
#      delegated directory, for a run with the init of the image;
#   8. with runsc and no delegated cgroup (TODO/enter.md T-2007): the run
#      names the runsc tier, and the check exits 7, from the namespace rung
#      and with `PODVM_USERNS=0`; a SIGKILL of podvm leaves no process.
#      Clauses 2 and 3 run with `PODVM_RUNSC` on a path with no file, so
#      that they measure the guest. Skipped with no runsc.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="${PODVM_465_BIN:-$ROOT/target/x86_64-unknown-linux-musl/release/podvm}"
OUT="$ROOT/experiments/results/init-image.txt"
IMG='ghcr.io/pkgforge-dev/archlinux:latest@sha256:b2507f1964270cab3cc190aa8df858521f6a7e45e969146a012877891d5dfc9b'
WORK=$(mktemp -d /var/tmp/podvm-465.XXXXXX)
# The helpers of podvm (the removal of a tree, the fill of the spare tree)
# can write in the store for some seconds after a run: try again.
cleanup() {
	i=0
	while [ $i -lt 30 ] && ! rm -rf "$WORK" 2>/dev/null; do
		sleep 1
		i=$((i + 1))
	done
}
trap cleanup EXIT
chmod 0755 "$WORK"
mkdir -p "$WORK/home" "$WORK/fx" "$(dirname "$OUT")"
cp "$BIN" "$WORK/podvm"
: >"$OUT"
fail=0
say() { printf '%s\n' "$*" | tee -a "$OUT"; }
bad() { say "FAIL: $*"; fail=1; }
keep() { sed 's/^/    /' "$1" | tee -a "$OUT"; }

cat >"$WORK/fx/check.sh" <<'EOF'
#!/bin/sh
fail=0
say() { echo "CHECK $1 $2"; }
bad() { echo "FAIL $1"; fail=1; }
comm=$(cat /proc/1/comm 2>/dev/null)
say pid1 "$comm"
[ "$comm" = systemd ] || bad "PID 1 is $comm, not systemd"
state=$(systemctl is-system-running 2>&1)
say state "$state"
case "$state" in running | degraded) ;; *) bad "the state is $state" ;; esac
say uid_map "$(tr -s ' \n' ' ' </proc/self/uid_map)"
add() {
	grep -q "^$1:" /etc/passwd || {
		echo "$1:x:$2:$2:$1:/nonexistent:/usr/sbin/nologin" >>/etc/passwd
		echo "$1:x:$2:" >>/etc/group
	}
}
add mysql 961
add postgres 962
add www-data 33
add nobody 65534
mkdir -p /var/lib/podvm-check && chmod 1777 /var/lib/podvm-check
for u in mysql postgres www-data nobody; do
	cat >/etc/systemd/system/podvm-u-$u.service <<UNIT
[Unit]
Description=podvm check: a process of the user $u
[Service]
User=$u
ExecStart=/bin/sh -c 'grep -E "^(Uid|Gid):" /proc/self/status > /var/lib/podvm-check/$u.txt; exec sleep 600'
UNIT
done
systemctl daemon-reload || bad "daemon-reload"
for u in mysql postgres www-data nobody; do
	want=$(awk -F: -v u=$u '$1==u{print $3}' /etc/passwd)
	systemctl start podvm-u-$u.service || {
		bad "systemctl start podvm-u-$u.service"
		continue
	}
	i=0
	while [ ! -s /var/lib/podvm-check/$u.txt ] && [ $i -lt 50 ]; do
		sleep 0.1
		i=$((i + 1))
	done
	main=$(systemctl show -p MainPID --value podvm-u-$u.service)
	puid=$(awk '/^Uid:/{print $2}' /proc/$main/status 2>/dev/null)
	fuid=$(stat -c %u /var/lib/podvm-check/$u.txt 2>/dev/null)
	active=$(systemctl is-active podvm-u-$u.service)
	say "unit-$u" "active=$active mainpid-uid=$puid file-owner=$fuid want=$want"
	[ "$active" = active ] && [ "$puid" = "$want" ] && [ "$fuid" = "$want" ] ||
		bad "podvm-u-$u.service does not run as uid $want"
done
systemctl stop podvm-u-mysql.service
say stop "$(systemctl is-active podvm-u-mysql.service)"
[ "$(systemctl is-active podvm-u-mysql.service)" = inactive ] || bad "stop left the unit active"
say failed-units "$(systemctl --failed --no-legend --plain 2>/dev/null | awk '{print $1}' | tr '\n' ' ')"
if [ $fail = 0 ]; then
	echo "VERDICT pass"
	exit 7
fi
echo "VERDICT fail"
exit 1
EOF

# The user that runs podvm, and the command that runs as it.
if [ "$(id -u)" -eq 0 ]; then
	WHO="${PODVM_465_USER:-ubuntu}"
	id "$WHO" >/dev/null 2>&1 || {
		echo "SKIP: root needs a user with no root in PODVM_465_USER" >&2
		exit 2
	}
	chown -R "$WHO" "$WORK"
	as() { runuser -u "$WHO" -- env HOME="$WORK/home" PODVM_STORE="$WORK/store" "$@"; }
else
	WHO=$(id -un)
	as() { env HOME="$WORK/home" PODVM_STORE="$WORK/store" "$@"; }
fi
P="$WORK/podvm"
UID_OF=$(id -u "$WHO")

# The cgroup2 directory of this shell, and whether the user can write it.
CGROOT=$(awk '$9 == "cgroup2" || $10 == "cgroup2" {print $5; exit}' /proc/self/mountinfo)
OWN="$CGROOT$(sed -n 's/^0:://p' /proc/self/cgroup)"
delegated() { [ -n "$CGROOT" ] && as sh -c "mkdir '$1/podvm-465-probe' 2>/dev/null && rmdir '$1/podvm-465-probe'"; }
STANDIN=""
if delegated "$OWN"; then
	DELEG="$OWN"
elif [ "$(id -u)" -eq 0 ] && [ -n "$CGROOT" ]; then
	# A stand-in of a host that delegates: root gives the user a directory,
	# as a user manager of systemd or a unit with Delegate= does.
	STANDIN="$CGROOT/podvm-465"
	mkdir -p "$STANDIN" && chown "$WHO" "$STANDIN" "$STANDIN/cgroup.procs" "$STANDIN/cgroup.subtree_control"
	DELEG="$STANDIN"
else
	DELEG=""
fi
# Run a command from the delegated directory.
from_deleg() { sh -c 'echo $$ > "$0/cgroup.procs" && shift 0 && exec "$@"' "$DELEG" "$@"; }

say "date      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "podvm     $("$P" version 2>/dev/null)"
say "user      $WHO (uid $UID_OF)"
say "kernel    $(uname -r)"
say "kvm       $([ -w /dev/kvm ] && echo yes || echo 'no (a guest runs under TCG)')"
say "subuid    $(grep "^$WHO:" /etc/subuid 2>/dev/null | head -1 || true)"
say "cgroup    $OWN; delegated: ${DELEG:-none}${STANDIN:+ (a stand-in that root made)}"
T="$WORK/t"
as timeout 600 "$P" pull -q "$IMG" >/dev/null 2>"$T.pull.err" || {
	bad "the pull of $IMG"
	keep "$T.pull.err"
}

run_check() { # the number of the clause, then the prefix of the command
	n=$1
	shift
	start=$(date +%s)
	"$@" timeout 900 "$P" run --rm --podvm-init=image -v "$WORK/fx:/fx" "$IMG" sh /fx/check.sh \
		>"$T.$n.out" 2>"$T.$n.err"
	rc=$?
	say "clause $n  rc $rc in $(($(date +%s) - start)) s"
	keep "$T.$n.out"
	grep 'podvm run: --podvm-init=image' "$T.$n.err" | cut -c1-260 | sed 's/^/    stderr: /' | tee -a "$OUT"
	return $rc
}

# Clause 1: the namespace rung with a delegated cgroup2 directory.
if [ -n "$DELEG" ]; then
	if [ "$(id -u)" -eq 0 ]; then
		run_check 1 from_deleg runuser -u "$WHO" -- env HOME="$WORK/home" PODVM_STORE="$WORK/store"
	else
		run_check 1 from_deleg env HOME="$WORK/home" PODVM_STORE="$WORK/store"
	fi
	rc=$?
	[ "$rc" -eq 7 ] || { bad "clause 1: the check gave $rc, not 7"; keep "$T.1.err"; }
	grep -q 'uid_map  0 [0-9]* 1 1 [0-9]* [0-9]*' "$T.1.out" || bad "clause 1: the uid_map holds no range"
	grep -q 'takes the machine tier' "$T.1.err" && bad "clause 1: the run left the namespace rung"
else
	say "clause 1  skipped: no delegated cgroup2 directory for $WHO, and the script is not root"
fi

# Clause 2: the namespace rung with no delegated cgroup: the machine tier.
if delegated "$OWN"; then
	say "clause 2  skipped: the cgroup of this shell is delegated"
else
	run_check 2 as env PODVM_RUNSC=/nonexistent/runsc
	rc=$?
	[ "$rc" -eq 7 ] || { bad "clause 2: the check gave $rc, not 7"; keep "$T.2.err"; }
	grep -q 'takes the machine tier' "$T.2.err" || bad "clause 2: the run names no machine tier"
fi

# Clause 3: the userland rung: the machine tier.
run_check 3 as env PODVM_USERNS=0 PODVM_RUNSC=/nonexistent/runsc
rc=$?
[ "$rc" -eq 7 ] || { bad "clause 3: the check gave $rc, not 7"; keep "$T.3.err"; }
grep -q 'takes the machine tier' "$T.3.err" || bad "clause 3: the run names no machine tier"

# Clause 4: the userland rung with no init.
as env PODVM_USERNS=0 timeout 300 "$P" run --rm --verbose -v "$WORK/fx:/fx" "$IMG" sh -c \
	'echo "pid1=$(cat /proc/1/comm) self=$$ uid=$(id -u)"; setpriv --reuid 33 --regid 33 --clear-groups id' \
	>"$T.4.out" 2>"$T.4.err"
say "clause 4  rc $? (the userland rung with no init: $(grep -o 'mode=[a-z-]*' "$T.4.err" | head -1))"
keep "$T.4.out"
grep -a 'setresuid' "$T.4.err" | cut -c1-200 | sed 's/^/    stderr: /' | tee -a "$OUT"
grep -q 'mode=userland' "$T.4.err" || bad "clause 4: the run did not enter the userland rung"
grep -q 'pid1=systemd' "$T.4.out" && bad "clause 4: the userland rung has systemd as PID 1"

# Clause 5: the refusals by name.
for flags in "-d" "--podvm-tier=syscall"; do
	# shellcheck disable=SC2086
	as timeout 60 "$P" run $flags --podvm-init=image "$IMG" true >/dev/null 2>"$T.5.err"
	rc=$?
	say "clause 5  rc $rc (run $flags --podvm-init=image)"
	{ [ "$rc" -eq 125 ] && grep -q 'podvm-init=image holds on' "$T.5.err"; } || {
		bad "clause 5: run $flags"
		keep "$T.5.err"
	}
done

# Clause 6: nothing stays.
sleep 2
left=$(ps -u "$WHO" -o pid=,stat=,args= 2>/dev/null | grep -E 'qemu-system|systemd|guest-init' | grep -v ' Z' || true)
say "clause 6  processes that stay: ${left:-none}"
[ -z "$left" ] || bad "clause 6: a process of the runs stays"
if [ -n "$DELEG" ]; then
	cg=$(find "$DELEG" -maxdepth 1 -name 'podvm-init-*' 2>/dev/null | head -3)
	say "clause 6  cgroups that stay: ${cg:-none}"
	[ -z "$cg" ] || bad "clause 6: a cgroup of a run stays"
fi

# Clause 7: a SIGKILL of podvm while the command runs.
killed() { # the name of the case, then the command
	what=$1
	shift
	"$@" >/dev/null 2>&1 &
	sleep 8
	# The oldest process whose command line starts with the binary: the
	# outer podvm, not the `timeout` before it.
	outer=$(pgrep -u "$WHO" -o -f "^$P run --rm.*sleep 465465" || true)
	[ -n "$outer" ] && kill -KILL "$outer"
	wait
	sleep 2
	left=$(ps -u "$WHO" -o pid=,stat=,args= 2>/dev/null | grep -E 'sleep 465465|systemd|guest-init' | grep -v ' Z' || true)
	say "clause ${KC:-7}  $what: podvm was pid ${outer:-none}; processes that stay: ${left:-none}"
	{ [ -n "$outer" ] && [ -z "$left" ]; } || bad "clause ${KC:-7}: $what"
}
killed "a run with podvm's init" as timeout 120 "$P" run --rm "$IMG" sleep 465465
if [ -n "$DELEG" ]; then
	if [ "$(id -u)" -eq 0 ]; then
		killed "a run with the init of the image" from_deleg runuser -u "$WHO" -- \
			env HOME="$WORK/home" PODVM_STORE="$WORK/store" timeout 120 "$P" run --rm --podvm-init=image "$IMG" sleep 465465
	else
		killed "a run with the init of the image" from_deleg \
			env HOME="$WORK/home" PODVM_STORE="$WORK/store" timeout 120 "$P" run --rm --podvm-init=image "$IMG" sleep 465465
	fi
fi

# Clause 8: the runsc tier, on the host as it is.
REL=20261102
SUM=2e6671bb2d033b871c5811ab57e16ac570988b0e0054c525f911cfaf191bab3ca4efb08e0a98daa4b9e7d248f9e5776ef2ef2f8a6b82d829646a42924c98bb05
RUNSC="${PODVM_465_RUNSC:-}"
if [ -z "$RUNSC" ] && [ "$(uname -m)" = x86_64 ]; then
	mkdir -p "$WORK/gvisor"
	if curl -qfsSL -o "$WORK/gvisor.tar.bz2" "https://storage.googleapis.com/gvisor/releases/release/$REL/x86_64/gvisor.tar.bz2" &&
		echo "$SUM  $WORK/gvisor.tar.bz2" | sha512sum -c - >/dev/null 2>&1 &&
		tar -xjf "$WORK/gvisor.tar.bz2" -C "$WORK/gvisor" runsc gvisor-bin; then
		RUNSC="$WORK/gvisor/runsc"
		chmod -R a+rX "$WORK/gvisor"
	fi
	rm -f "$WORK/gvisor.tar.bz2"
fi
if [ -n "$RUNSC" ] && [ -x "$RUNSC" ] && ! delegated "$OWN"; then
	say "runsc     $("$RUNSC" --version 2>/dev/null | head -1)"
	run_check 8 as env PODVM_RUNSC="$RUNSC"
	rc=$?
	[ "$rc" -eq 7 ] || { bad "clause 8: the check gave $rc, not 7"; keep "$T.8.err"; }
	grep -q 'takes the runsc tier' "$T.8.err" || bad "clause 8: the run names no runsc tier"
	cp "$T.8.err" "$T.8a.err"
	run_check 8 as env PODVM_USERNS=0 PODVM_RUNSC="$RUNSC"
	rc=$?
	[ "$rc" -eq 7 ] || { bad "clause 8 (PODVM_USERNS=0): the check gave $rc, not 7"; keep "$T.8.err"; }
	KC=8
	killed "a SIGKILL of podvm on the runsc tier" as env PODVM_RUNSC="$RUNSC" timeout 120 "$P" run --rm --podvm-init=image "$IMG" sleep 465465
	left=$(ps -u "$WHO" -o pid=,stat=,args= 2>/dev/null | grep -E 'runsc|gvisor' | grep -v ' Z' || true)
	say "clause 8  processes of gVisor that stay: ${left:-none}"
	[ -z "$left" ] || bad "clause 8: a process of gVisor stays"
else
	say "clause 8  skipped: no runsc, or the cgroup of this shell is delegated"
fi

# The base of the killed run is empty; the next run removes it when it is
# older than each boot (`initns::cgroup::sweep`). Here the script does.
if [ -n "$STANDIN" ]; then
	find "$STANDIN" -mindepth 1 -depth -type d -exec rmdir {} \; 2>/dev/null
	rmdir "$STANDIN" 2>/dev/null
fi

if [ "$fail" -eq 0 ]; then
	say "THE INIT OF THE IMAGE RUNS"
	exit 0
fi
say "A CLAUSE FAILED"
exit 1
