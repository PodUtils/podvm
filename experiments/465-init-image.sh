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
#      run names the missing part and the machine tier, the check exits 7,
#      and the run saves the state of its guest (TODO/machine.md T-2048).
#      Skipped where the cgroup of the script is delegated;
#   3. the userland rung (`PODVM_USERNS=0` and a volume): the run names the
#      machine tier, and the check exits 7. Where clause 2 ran, this is the
#      second run of the digest on the machine tier: its guest comes from
#      the saved state (TODO/machine.md T-2008), and the run takes less
#      than half of the time of clause 2, which booted;
#   4. the userland rung with no init, for the record of why it has none:
#      PID 1 is a process of the host, and `setresuid` to another uid fails;
#   5. `run --podvm-tier=syscall --podvm-init=image` and `create
#      --podvm-tier=machine --podvm-init=image` refuse by name with 125,
#      and so does `run -d --podvm-init=image` from a cgroup that is not
#      delegated: it names the missing part, and no other route keeps an
#      init for a container that stays;
#   6. after the runs: no emulator and no systemd of the user stays, and no
#      cgroup `podvm-init-*` stays in the delegated directory;
#   7. a SIGKILL of podvm during a run (TODO/supervise.md T-2006): no
#      process of the run stays, for a run with podvm's init and, from the
#      delegated directory, for a run with the init of the image;
#   8. with runsc and no delegated cgroup (TODO/enter.md T-2007): the run
#      names the runsc tier, and the check exits 7, from the namespace rung
#      and with `PODVM_USERNS=0`; a SIGKILL of podvm leaves no process.
#      Clauses 2 and 3 run with `PODVM_RUNSC` on a path with no file, so
#      that they measure the guest. Skipped with no runsc;
#   9. the saved state of a guest (TODO/machine.md T-2008): each run of the
#      clause names the machine tier (no user bus, `PODVM_USERNS=0`, no
#      runsc), and its first run saves the state that the others load
#      (T-2048); after a SIGKILL
#      of podvm with a guest from a state, no emulator stays; a state file
#      that is cut is not loaded, and the run saves a new one; a state that
#      the emulator refuses gives a boot with the same result, one line,
#      and a marker with the count 1 in place of the state;
#      `PODVM_MACHINE_STATE=0` writes
#      no state, and another value is refused with 125;
#  10. a container that stays, on the namespace rung from the delegated
#      directory (TODO/enter.md T-2002): `run -d --podvm-init=image`
#      returns after the boot; `exec` of `systemctl is-system-running`
#      gives `running` or `degraded`, the check exits 7 through `exec`,
#      `exec -u` gives the uid, and `exec -t` gives `tty` a name;
#      `stop` gives the command SIGTERM (its trap writes a file, and the
#      record has its exit code 5) and the init its power off (the
#      ExecStop of a unit writes a file); nothing stays; `start`, `exec`
#      and `rm -f` hold, and nothing stays;
#  11. the pty of `-t` on the namespace rung (TODO/enter.md T-2002): `tty`
#      names a pty of the init, and a shell that reads two lines from the
#      pipe of `-it` runs them and gives its exit code;
#  12. the flags that limit a run (TODO/enter.md T-2017): `--podvm-confine`
#      and a `--device` with w and no r are refused by name with 125; with
#      `--ulimit nofile=512:1024`, `--security-opt no-new-privileges` and
#      `--device /dev/null:/dev/xnull`, the command has the soft value 512
#      and the hard value 1024, PID 1 and a unit of `systemd-run` have the
#      hard value 1024, each of the three has NoNewPrivs 1, and
#      `/dev/xnull` is a character device that takes a write; PID 1 and
#      the device hold the same through `exec` of `run -d`.

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
mkdir -p "$WORK/home" "$WORK/fx" "$WORK/vol" "$(dirname "$OUT")"
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
# A user manager of systemd gives a run a delegated scope (TODO/enter.md
# T-2004), as on a GitHub-hosted runner. A clause that needs a host with no
# delegation hides the user bus: podvm reads it only in XDG_RUNTIME_DIR.
NOBUS="$WORK/no-user-bus"
mkdir -p "$NOBUS" && chmod 777 "$NOBUS"
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
	took=$(($(date +%s) - start))
	say "clause $n  rc $rc in $took s"
	keep "$T.$n.out"
	grep -E 'podvm run: (--podvm-init=image|machine tier: the (guest that podvm|state of the guest))' "$T.$n.err" | cut -c1-260 | sed 's/^/    stderr: /' | tee -a "$OUT"
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

# The files of the saved states of the store, by their extension.
states() { find "$WORK/store/machine" -maxdepth 1 -name "state-*.$1" 2>/dev/null | wc -l; }

# Clause 2: the namespace rung with no delegated cgroup: the machine tier.
# It is the first run of the digest on that tier: it boots, and saves the
# state of its guest (TODO/machine.md T-2008). A save that fails says why
# on stderr, and the run goes on (T-2048: a late answer of the monitor hung
# this run until its timeout).
if delegated "$OWN"; then
	say "clause 2  skipped: the cgroup of this shell is delegated"
else
	run_check 2 as env XDG_RUNTIME_DIR="$NOBUS" PODVM_RUNSC=/nonexistent/runsc
	rc=$?
	first=$took
	[ "$rc" -eq 7 ] || { bad "clause 2: the check gave $rc, not 7"; keep "$T.2.err"; }
	grep -q 'takes the machine tier' "$T.2.err" || bad "clause 2: the run names no machine tier"
	say "clause 2  state files after the run: $(states vm)"
	[ "$(states vm)" -ge 1 ] || { bad "clause 2: the run saved no state of its guest"; keep "$T.2.err"; }
fi

# Clause 3: the userland rung: the machine tier. It is the second run of
# the digest on that tier: the guest comes from the state that the first
# one saved (TODO/machine.md T-2008).
first=${first:-0}
before=$(states vm)
run_check 3 as env XDG_RUNTIME_DIR="$NOBUS" PODVM_USERNS=0 PODVM_RUNSC=/nonexistent/runsc
rc=$?
[ "$rc" -eq 7 ] || { bad "clause 3: the check gave $rc, not 7"; keep "$T.3.err"; }
grep -q 'takes the machine tier' "$T.3.err" || bad "clause 3: the run names no machine tier"
if [ "$before" -ge 1 ]; then
	say "clause 3  the guest came from a saved state: $before state file, $took s"
	grep -q 'podvm boots a guest with no state' "$T.3.err" && bad "clause 3: the saved state did not serve the run"
	# The time of a run depends on the load of the host (13 s and 16 s for
	# the same build): the bound is half of the run that booted.
	[ "$first" -eq 0 ] || [ "$took" -lt $((first / 2)) ] || bad "clause 3: the second run of the digest took $took s, not under half of the $first s of the first"
else
	say "clause 3  no saved state before the run (clause 2 did not run, or its save failed)"
fi

# Clause 4: the userland rung with no init.
as env PODVM_USERNS=0 timeout 300 "$P" run --rm --verbose -v "$WORK/fx:/fx" "$IMG" sh -c \
	'echo "pid1=$(cat /proc/1/comm) self=$$ uid=$(id -u)"; setpriv --reuid 33 --regid 33 --clear-groups id' \
	>"$T.4.out" 2>"$T.4.err"
say "clause 4  rc $? (the userland rung with no init: $(grep -o 'mode=[a-z-]*' "$T.4.err" | head -1))"
keep "$T.4.out"
grep -a 'setresuid' "$T.4.err" | cut -c1-200 | sed 's/^/    stderr: /' | tee -a "$OUT"
grep -q 'mode=userland' "$T.4.err" || bad "clause 4: the run did not enter the userland rung"
# The userland rung shows the PID 1 of the host: a host whose own init is
# systemd (a hosted runner) says nothing of the payload.
[ "$(cat /proc/1/comm 2>/dev/null)" = systemd ] ||
	{ grep -q 'pid1=systemd' "$T.4.out" && bad "clause 4: the userland rung has systemd as PID 1"; }

# Clause 5: the refusals by name.
for flags in "run --podvm-tier=syscall" "create --podvm-tier=machine"; do
	# shellcheck disable=SC2086
	as timeout 60 "$P" $flags --podvm-init=image "$IMG" true >/dev/null 2>"$T.5.err"
	rc=$?
	say "clause 5  rc $rc ($flags --podvm-init=image)"
	{ [ "$rc" -eq 125 ] && grep -q 'podvm-init=image holds on' "$T.5.err"; } || {
		bad "clause 5: $flags"
		keep "$T.5.err"
	}
done
if delegated "$OWN"; then
	say "clause 5  run -d from a cgroup with no delegation: skipped, the cgroup of this shell is delegated"
else
	as env XDG_RUNTIME_DIR="$NOBUS" timeout 60 "$P" run -d --podvm-init=image "$IMG" true >/dev/null 2>"$T.5.err"
	rc=$?
	say "clause 5  rc $rc (run -d --podvm-init=image from a cgroup that is not delegated)"
	cut -c1-200 "$T.5.err" | sed 's/^/    stderr: /' | tee -a "$OUT"
	{ [ "$rc" -eq 125 ] && grep -q 'delegates no cgroup2 subtree' "$T.5.err" && grep -q 'a container that stays' "$T.5.err"; } ||
		bad "clause 5: run -d with no delegated cgroup"
	[ "$(as "$P" ps -a 2>/dev/null | grep -c ' true ')" -eq 0 ] || bad "clause 5: the refused run left a record"
fi

# Clause 6: nothing stays.
sleep 2
left=$(ps -u "$WHO" -o pid=,stat=,args= 2>/dev/null | grep -E 'qemu-system|systemd|guest-init' | grep -v -e ' Z' -e 'systemd --user' -e 'dbus-daemon' -e 'grep -E' || true)
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
	left=$(ps -u "$WHO" -o pid=,stat=,args= 2>/dev/null | grep -E 'sleep 465465|systemd|guest-init' | grep -v -e ' Z' -e 'systemd --user' -e 'dbus-daemon' -e 'grep -E' || true)
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
	run_check 8 as env XDG_RUNTIME_DIR="$NOBUS" PODVM_RUNSC="$RUNSC"
	rc=$?
	[ "$rc" -eq 7 ] || { bad "clause 8: the check gave $rc, not 7"; keep "$T.8.err"; }
	grep -q 'takes the runsc tier' "$T.8.err" || bad "clause 8: the run names no runsc tier"
	cp "$T.8.err" "$T.8a.err"
	run_check 8 as env PODVM_USERNS=0 PODVM_RUNSC="$RUNSC"
	rc=$?
	[ "$rc" -eq 7 ] || { bad "clause 8 (PODVM_USERNS=0): the check gave $rc, not 7"; keep "$T.8.err"; }
	KC=8
	killed "a SIGKILL of podvm on the runsc tier" as env PODVM_RUNSC="$RUNSC" timeout 120 "$P" run --rm --podvm-init=image "$IMG" sleep 465465
	left=$(ps -u "$WHO" -o pid=,stat=,args= 2>/dev/null | grep -E 'runsc|gvisor' | grep -v -e ' Z' -e 'systemd --user' -e 'dbus-daemon' -e 'grep -E' || true)
	say "clause 8  processes of gVisor that stay: ${left:-none}"
	[ -z "$left" ] || bad "clause 8: a process of gVisor stays"
else
	say "clause 8  skipped: no runsc, or the cgroup of this shell is delegated"
fi

# Clause 9: the saved state of a guest (TODO/machine.md T-2008). A state
# that is cut does not load: the run boots a guest, gives the same result,
# and says so in one line; the state is gone, and a marker holds the
# reason. With PODVM_MACHINE_STATE=0 no state is read or written. After a
# SIGKILL of podvm with a guest from a state, no emulator stays.
#
# Each run of this clause takes the machine tier on each host
# (TODO/machine.md T-2048): no user bus (a user manager gives a delegated
# scope), PODVM_USERNS=0 (a delegated cgroup of this shell), and no runsc.
# The first run saves the state that the later runs load and cut: no state
# of another clause is in the store.
if command -v qemu-system-x86_64 >/dev/null 2>&1; then
	m9() { as env XDG_RUNTIME_DIR="$NOBUS" PODVM_USERNS=0 PODVM_RUNSC=/nonexistent/runsc "$@"; }
	# The run names the machine tier, or this clause tests nothing.
	tier9() { grep -q 'takes the machine tier' "$1" || bad "clause 9: $2: the run names no machine tier"; }
	find "$WORK/store/machine" -maxdepth 1 -name 'state-*' -exec rm -f {} + 2>/dev/null
	run_check 9 m9
	rc=$?
	[ "$rc" -eq 7 ] || { bad "clause 9: the run that saves the state gave $rc, not 7"; keep "$T.9.err"; }
	tier9 "$T.9.err" "the run that saves the state"
	ST=$(find "$WORK/store/machine" -maxdepth 1 -name 'state-*.vm' 2>/dev/null | head -1)
	say "clause 9  the run that saves: state files $(states vm), $(basename "${ST:-none}")"
	[ "$(states vm)" -eq 1 ] || { bad "clause 9: the run that boots saved $(states vm) state files, not 1"; keep "$T.9.err"; }
fi
if [ -n "${ST:-}" ]; then
	KC=9
	killed "a SIGKILL of podvm with a guest from a state" m9 timeout 120 "$P" run --rm --podvm-init=image "$IMG" sleep 465465
	left=$(ps -u "$WHO" -o pid=,stat=,args= 2>/dev/null | grep -E 'qemu-system' | grep -v -e ' Z' -e 'systemd --user' -e 'dbus-daemon' -e 'grep -E' || true)
	say "clause 9  emulators that stay: ${left:-none}"
	[ -z "$left" ] || bad "clause 9: an emulator stays"
	size=$(stat -c %s "$ST")
	truncate -s $((size / 2)) "$ST"
	# The record names the old length: the cut file is no state, and the run saves a new one.
	run_check 9 m9
	rc=$?
	[ "$rc" -eq 7 ] || { bad "clause 9: after a cut state the check gave $rc, not 7"; keep "$T.9.err"; }
	tier9 "$T.9.err" "after a cut state"
	[ "$(stat -c %s "$ST" 2>/dev/null || echo 0)" -ne $((size / 2)) ] || bad "clause 9: the cut state stays"
	# The same cut with a record that names the new length: the emulator refuses the load.
	if [ -s "$ST" ]; then
		size=$(stat -c %s "$ST")
		truncate -s $((size / 2)) "$ST"
		# The record of pod_guest::warm::line (T-2051): a line `bytes N`.
		sed -i "s/^bytes $size\$/bytes $((size / 2))/" "${ST%.vm}.record"
		grep -qx "bytes $((size / 2))" "${ST%.vm}.record" || bad "clause 9: the record names no length of $((size / 2))"
		run_check 9 m9
		rc=$?
		[ "$rc" -eq 7 ] || { bad "clause 9: after a state that does not load the check gave $rc, not 7"; keep "$T.9.err"; }
		tier9 "$T.9.err" "after a state that does not load"
		grep -q 'podvm boots a guest with no state' "$T.9.err" || { bad "clause 9: the run names no boot after the failed load"; keep "$T.9.err"; }
		say "clause 9  markers of a failed state: $(states bad); state files: $(states vm)"
		[ "$(states bad)" -eq 1 ] && [ "$(states vm)" -eq 0 ] || bad "clause 9: the failed state stays, or it has no marker"
		[ "$(head -1 "${ST%.vm}.bad" 2>/dev/null)" = 1 ] || bad "clause 9: the marker does not count one failure"
	else
		bad "clause 9: the run after a cut state saved no new state"
	fi
	find "$WORK/store/machine" -maxdepth 1 -name 'state-*' -exec rm -f {} +
	run_check 9 m9 env PODVM_MACHINE_STATE=0
	rc=$?
	[ "$rc" -eq 7 ] || { bad "clause 9: with PODVM_MACHINE_STATE=0 the check gave $rc, not 7"; keep "$T.9.err"; }
	tier9 "$T.9.err" "PODVM_MACHINE_STATE=0"
	[ "$(states vm)" -eq 0 ] || bad "clause 9: PODVM_MACHINE_STATE=0 saved a state"
	m9 env PODVM_MACHINE_STATE=maybe timeout 300 "$P" run --rm --podvm-init=image "$IMG" true >/dev/null 2>"$T.9v.err"
	rc=$?
	say "clause 9  PODVM_MACHINE_STATE=maybe: rc $rc"
	tier9 "$T.9v.err" "PODVM_MACHINE_STATE=maybe"
	{ [ "$rc" -eq 125 ] && grep -q 'the values are 0, 1 and pipe' "$T.9v.err"; } || { bad "clause 9: a wrong value of the variable"; keep "$T.9v.err"; }
else
	say "clause 9  skipped: no qemu-system-x86_64, or the run of the machine tier saved no state"
fi

# A command from the delegated directory, as the user.
ind() {
	if [ "$(id -u)" -eq 0 ]; then
		from_deleg runuser -u "$WHO" -- env HOME="$WORK/home" PODVM_STORE="$WORK/store" "$@"
	else
		from_deleg env HOME="$WORK/home" PODVM_STORE="$WORK/store" "$@"
	fi
}
stays() { # the processes and the cgroups of the runs that stay
	sleep 1
	left=$(ps -u "$WHO" -o pid=,stat=,args= 2>/dev/null | grep -E 'systemd|guest-init|/sbin/init' | grep -v -e ' Z' -e 'systemd --user' -e 'dbus-daemon' -e 'grep -E' || true)
	# The base of a run that a SIGKILL of podvm ended (clause 7) is empty
	# and stays for the sweep: only a base with a process counts here.
	cg=$(for d in "$DELEG"/podvm-init-*; do [ -s "$d/cgroup.procs" ] || grep -qs 'populated 1' "$d/cgroup.events" && echo "$d"; done 2>/dev/null)
	say "clause $1  processes that stay: ${left:-none}; cgroups with a process: ${cg:-none}"
	[ -z "$left$cg" ] || bad "clause $1: a process or a cgroup of the container stays"
}

# Clause 10: a container that stays: run -d, exec, stop, start, rm.
if [ -n "$DELEG" ]; then
	C=c465
	before=$(find "$DELEG" -maxdepth 1 -name 'podvm-init-*' 2>/dev/null | wc -l)
	start=$(date +%s)
	ind timeout 300 "$P" run -d --name $C -v "$WORK/fx:/fx" -v "$WORK/vol:/vol" --podvm-init=image "$IMG" \
		sh -c 'trap "echo term > /vol/term; exit 5" TERM; while :; do sleep 1; done' >"$T.10.out" 2>"$T.10.err"
	rc=$?
	say "clause 10  rc $rc in $(($(date +%s) - start)) s (run -d --podvm-init=image)"
	[ "$rc" -eq 0 ] || { bad "clause 10: run -d gave $rc"; keep "$T.10.err"; }
	state=$(as timeout 60 "$P" exec $C systemctl is-system-running 2>&1)
	say "clause 10  exec systemctl is-system-running: $state"
	case "$state" in running | degraded) ;; *) bad "clause 10: the state through exec is $state" ;; esac
	as timeout 300 "$P" exec $C sh /fx/check.sh >"$T.10c.out" 2>"$T.10c.err"
	rc=$?
	say "clause 10  rc $rc (the check through exec)"
	keep "$T.10c.out"
	[ "$rc" -eq 7 ] || { bad "clause 10: the check through exec gave $rc, not 7"; keep "$T.10c.err"; }
	u=$(as timeout 60 "$P" exec -u 65534 -e A=b -w /etc $C sh -c 'echo "$(id -u) $A $PWD"' 2>&1)
	say "clause 10  exec -u 65534 -e A=b -w /etc: $u"
	[ "$u" = "65534 b /etc" ] || bad "clause 10: exec -u, -e and -w"
	t=$(as timeout 60 "$P" exec -t $C tty </dev/null 2>&1 | tr -d '\r\000')
	say "clause 10  exec -t tty: $t"
	case "$t" in /dev/pts/*) ;; *) bad "clause 10: exec -t gives tty no name" ;; esac
	as timeout 60 "$P" exec $C sh -c 'exit 9' 2>/dev/null
	[ $? -eq 9 ] || bad "clause 10: exec does not give the exit code of its command"
	as timeout 60 "$P" exec $C sh -c 'printf "[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart=/bin/true\nExecStop=/bin/sh -c \"echo stopped > /vol/unit\"\n" > /etc/systemd/system/podvm-stop.service; systemctl daemon-reload; systemctl start podvm-stop.service' ||
		bad "clause 10: the unit of the stop check did not start"
	start=$(date +%s)
	as timeout 60 "$P" stop $C >/dev/null 2>"$T.10s.err"
	rc=$?
	code=$(as "$P" inspect --format '{{.State.ExitCode}}' $C 2>/dev/null)
	say "clause 10  rc $rc in $(($(date +%s) - start)) s (stop); the exit code of the record: $code; the command wrote: $(cat "$WORK/vol/term" 2>/dev/null); the unit wrote: $(cat "$WORK/vol/unit" 2>/dev/null)"
	[ "$rc" -eq 0 ] && [ "$code" = 5 ] || bad "clause 10: stop, or the exit code of the command"
	[ "$(cat "$WORK/vol/term" 2>/dev/null)" = term ] || bad "clause 10: the command did not get SIGTERM"
	[ "$(cat "$WORK/vol/unit" 2>/dev/null)" = stopped ] || bad "clause 10: the init did not stop its units"
	stays 10
	ind timeout 300 "$P" start $C >/dev/null 2>"$T.10.err" || { bad "clause 10: start"; keep "$T.10.err"; }
	state=$(as timeout 60 "$P" exec $C sh -c 'cat /proc/1/comm; systemctl is-system-running' 2>&1 | tr '\n' ' ')
	say "clause 10  after start: $state"
	case "$state" in "systemd running "* | "systemd degraded "*) ;; *) bad "clause 10: the init after start" ;; esac
	start=$(date +%s)
	as timeout 60 "$P" rm -f $C >/dev/null 2>"$T.10r.err"
	rc=$?
	say "clause 10  rc $rc in $(($(date +%s) - start)) s (rm -f of the running container)"
	[ "$rc" -eq 0 ] || { bad "clause 10: rm -f"; keep "$T.10r.err"; }
	stays 10
	after=$(find "$DELEG" -maxdepth 1 -name 'podvm-init-*' 2>/dev/null | wc -l)
	[ "$after" -le "$before" ] || bad "clause 10: a cgroup base of the container stays ($before before, $after after)"
	trees=$(find "$WORK/store/trees" -mindepth 1 -maxdepth 1 2>/dev/null | wc -l)
	# The end by SIGKILL, five times: the launcher must wait for no process
	# of the run (the init of a PID namespace ends when each process of
	# its namespace is reaped; before the supervisor was PID 1 of a
	# namespace of its own, rm -f took 5 s and kept the record in some runs).
	slow=0
	for i in 1 2 3 4 5; do
		ind timeout 300 "$P" run -d --name $C --podvm-init=image "$IMG" sh -c 'while :; do sleep 1; done' >/dev/null 2>"$T.10.err" ||
			{ bad "clause 10: run -d number $i"; keep "$T.10.err"; }
		start=$(date +%s)
		as timeout 60 "$P" rm -f $C >/dev/null 2>"$T.10r.err" || { bad "clause 10: rm -f number $i"; keep "$T.10r.err"; }
		[ $(($(date +%s) - start)) -lt 4 ] || slow=$((slow + 1))
	done
	say "clause 10  five times run -d and rm -f: $slow of them took 4 s or more"
	[ "$slow" -eq 0 ] || bad "clause 10: rm -f waited for a process of the run"
	stays 10
	recs=$(as "$P" ps -a 2>/dev/null | grep -c " $C\$")
	say "clause 10  records of $C: $recs; trees in the store: $trees (a run that a SIGKILL ended keeps its tree and its record)"
	[ "$recs" -eq 0 ] || bad "clause 10: the record of $C stays"
else
	say "clause 10  skipped: no delegated cgroup2 directory"
fi

# Clause 11: the pty of -t with the init of the image.
if [ -n "$DELEG" ]; then
	t=$(ind timeout 300 "$P" run --rm -t --podvm-init=image "$IMG" sh -c 'tty; ps -o tty= -p $$' </dev/null 2>"$T.11.err" | tr -d '\r\000' | tr '\n' ' ')
	say "clause 11  run -t: tty and the terminal of the process: $t"
	case "$t" in "/dev/pts/"*" pts/"*) ;; *) bad "clause 11: -t gives the command no pty of the init"; keep "$T.11.err" ;; esac
	printf 'echo typed-$((40+2))\nexit 4\n' | ind timeout 300 "$P" run --rm -it --podvm-init=image "$IMG" sh >"$T.11.out" 2>"$T.11.err"
	rc=$?
	# `run -t` sets TERM=xterm (T-2061), as docker does: the bash of the
	# image then writes the escapes of bracketed paste (ESC [ ? 2004 l)
	# before the output of each command. The check reads the text only.
	plain() { tr -d '\r' <"$1" | sed 's/\x1b\[[0-9;?]*[A-Za-z]//g'; }
	say "clause 11  rc $rc (a shell that reads two lines from the pipe of -it); typed-42: $(plain "$T.11.out" | grep -c '^typed-42$')"
	{ [ "$rc" -eq 4 ] && plain "$T.11.out" | grep -q '^typed-42$'; } || { bad "clause 11: the input of -it"; keep "$T.11.err"; }
	stays 11
else
	say "clause 11  skipped: no delegated cgroup2 directory"
fi

# Clause 12: the flags that limit a run with the init of the image.
if [ -n "$DELEG" ]; then
	for flags in "--podvm-confine read=/usr" "--device /dev/null:/dev/xnull:w"; do
		# shellcheck disable=SC2086
		ind timeout 60 "$P" run --rm $flags --podvm-init=image "$IMG" true >/dev/null 2>"$T.12.err"
		rc=$?
		say "clause 12  rc $rc (run $flags --podvm-init=image)"
		{ [ "$rc" -eq 125 ] && grep -q 'is refused' "$T.12.err"; } || { bad "clause 12: run $flags"; keep "$T.12.err"; }
	done
	cat >"$WORK/fx/limits.sh" <<'LIMITS'
#!/bin/sh
lim() { awk '/Max open files/{print $5}' "/proc/$1/limits"; }
nnp() { awk '/NoNewPrivs/{print $2}' "/proc/$1/status"; }
echo "cmd $(ulimit -Sn) $(ulimit -Hn) $(nnp self)"
echo "pid1 $(lim 1) $(nnp 1)"
systemd-run --wait --pipe -q sh -c 'echo unit $(ulimit -Hn) $(awk "/NoNewPrivs/{print \$2}" /proc/self/status)'
[ -c /dev/xnull ] && echo x >/dev/xnull && echo "dev ok"
LIMITS
	want="cmd 512 1024 1;pid1 1024 1;unit 1024 1;dev ok;"
	hard="--ulimit nofile=512:1024 --security-opt no-new-privileges --device /dev/null:/dev/xnull"
	# shellcheck disable=SC2086
	got=$(ind timeout 300 "$P" run --rm $hard --podvm-init=image -v "$WORK/fx:/fx" "$IMG" sh /fx/limits.sh 2>"$T.12.err" | tr '\n' ';')
	say "clause 12  run $hard: $got"
	[ "$got" = "$want" ] || { bad "clause 12: the limits of the foreground run"; keep "$T.12.err"; }
	# shellcheck disable=SC2086
	ind timeout 300 "$P" run -d --name c465l $hard --podvm-init=image -v "$WORK/fx:/fx" "$IMG" sh -c 'while :; do sleep 1; done' >/dev/null 2>"$T.12.err" ||
		{ bad "clause 12: run -d"; keep "$T.12.err"; }
	got=$(as timeout 60 "$P" exec c465l sh /fx/limits.sh 2>&1 | grep -E '^(pid1|dev)' | tr '\n' ';')
	say "clause 12  exec of run -d: $got"
	[ "$got" = "pid1 1024 1;dev ok;" ] || bad "clause 12: the limits of run -d"
	as timeout 60 "$P" rm -f c465l >/dev/null 2>&1
	stays 12
else
	say "clause 12  skipped: no delegated cgroup2 directory"
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
