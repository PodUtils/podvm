#!/bin/sh
# Question: does `podvm run --podvm-tier=incus` run an OCI image on an
# incusd that a user with no root runs (route A), with docker's stdout,
# stderr, exit code, stdin, environment and working directory, and does
# nothing of the run stay in incus?
#
# TODO/machine.md T-1718. The incusd is the one of experiment 433 (route A
# of docs/incus.md): root prepares the host once, then the user starts
# incusd as root of the user's own user namespace. podvm talks to it over
# its unix socket (INCUS_DIR), as the same user. Run as root, which is the
# one preparation.
#
# Pinned inputs:
#   incus     the Ubuntu package 6.0.0-1ubuntu0.3 (apt)
#   OCI       ALPINE of experiments/356-no-chroot-rung.sh (alpine 3.20 by
#             digest), pulled by podvm as the same user
#
# Clauses:
#   1. a run gives the stdout of the command, exit 0, and the stderr line
#      that names the tier, the incusd version and the import of the image.
#   2. the exit code of the command is the exit code of podvm (7).
#   3. stdout and stderr stay apart.
#   4. piped stdin reaches the command.
#   5. -e and -w reach the command.
#   6. PID 1 of the instance is podvm's hold, and the files of the instance
#      have the owners that the image intended (/etc/passwd 0:0,
#      /etc/shadow 0:42), from podvm's .meta.jsonl.
#   7. a second run takes the image from incus (no import); the median
#      milliseconds of RUNS runs of `true`.
#   8. exec on a name that is no instance exits 125 and names it
#      (T-1722; before it, create and exec refused the tier).
#   9. no instance stays in incus after the runs; after the cleanup, no
#      process of the user or its range stays.
#  10. where no incusd answers, a run refuses by name (exit 125).
#  11. -v HOSTDIR:/data (T-1722) is a disk device: the command reads a host
#      file there, its write is on the host after the run, and a `:ro`
#      volume refuses a write. The instance's root is an id of the user's
#      range, so the directory is mode 0777 (a directory of mode 0755 of
#      the user is read-only to the instance: recorded, not a clause).
#  12. an instance that stays (T-1722): `run -d --name` prints the name and
#      `ps` lists it Running; `exec` runs in it with its exit code; `logs`
#      gives the command's output; `stop` makes it Stopped in `ps -a`;
#      `start` runs it again; `rm` of a running one refuses and `rm -f`
#      removes it; `create` makes a Stopped one; a command that ends stops
#      its instance.
#  13. `tier = incus` in the store's configuration (T-1722): `system info`
#      names incus as .EnteredRung; `run` and `run -d` with no flag enter
#      incus; `exec`, `inspect` and `rm` of the name route to the
#      instance; `logs -f` follows the log until the instance stops;
#      `inspect --format` gives the name, the status, the tier and the exit
#      code; with the key removed the ladder no longer names incus.
#  14. `run -d -i` (T-1777, PodUtils/podrunner#85): the piped stdin goes in
#      as a file of root, mode 0600, before the start; the command reads
#      it, the file is gone once the command holds it, and the recorded
#      command does not carry it.
#  15. `--podvm-net none` and `--podvm-channel` (T-1779,
#      PodUtils/podrunner#88): the default profile gets a NIC on a bridge
#      of the incusd where it can make one (recorded); a run with no flag
#      shows it, and a run with `--podvm-net none` shows `lo` only. A run
#      and a `run -d -i` with `--podvm-channel SOCK:127.0.0.1:3128` reach
#      the unix socket SOCK of the host from the instance (wget of
#      127.0.0.1:3128), and the socket logs each request. The socket is a
#      `disk` device of the instance, and podvm's init relays the port to
#      it (PodUtils/podrunner#90: no `proxy` device).
#  16. `--podvm-engine` (T-1966, PodUtils/podrunner#296 ask 3): `run -d`
#      of `docker:dind` with no NIC starts `dockerd`, which answers
#      `docker info`; the instance has `security.nesting`.
#  17. `--storage-opt size=` (T-1964): on this pool of the driver `dir`,
#      which bounds nothing, `run -d` refuses by name and leaves no
#      instance; a native run refuses the flag by name.
#  18. `inspect` of an instance (T-1965) gives `CpuSeconds` and
#      `PeakMemoryBytes` as numbers.
#  19. `--podvm-init=image` (T-1993, PodUtils/podrunner#313 ask 1): `run -d`
#      of an image with systemd boots it as PID 1; the command runs after
#      `systemctl is-system-running` gives running or degraded, with its log;
#      `systemctl start` of a unit of the image works; `exec` reaches the
#      same systemd; the instance stops when its command ends, and
#      `inspect` gives its exit code; the flag off the incus tier takes
#      the next route and names it on stderr (call C26, T-2002), where it
#      refused before.
#  20. The hosted snapshot (T-1996, PodUtils/podrunner#316 ask 1): with
#      `--podvm-init=image` an instance maps at least 231,072 ids, has
#      `/dev/fuse`, and writes a machine-id of its own: two instances of
#      one image have two.
#  21. A large import (T-1997, PodUtils/podrunner#316 ask 1): the first run
#      of `golang:1.24-bookworm` (about 850 MB of tree) streams the image
#      into incusd; the disk use of the store and of incusd grows by at most
#      2.6 times the tree: incusd's image and its volume, and the overhead of
#      incusd's files (measured 2.38 on 2026-11-19), not by a third copy (a
#      tarball file in the store would give about 3.4).
#  22. The capabilities of a file (T-1999, PodUtils/podrunner#316 ask 4):
#      alpine with a layer whose /usr/bin/capfile carries
#      security.capability (cap_net_raw+ep) as a PAX record; a user that is
#      not root cannot set it on the tree, the sidecar names it, and the
#      root of an instance of the incus tier has it.
#  23. The OOM kills in the run report (T-1969, PodUtils/podrunner#296
#      ask 6): `run --rm --memory 32m --report R` of a command that holds
#      200 MB ends with 137, and R has `oom_kills` 1 or more; the report
#      of `true` has `oom_kills` 0 or no such field (incusd's metrics name
#      no sample yet for an instance that lived for less than their cache:
#      the field is optional, never a false count).
#  24. An image whose Entrypoint is its init (PodUtils/podrunner#325):
#      the image of clause 19, saved again with `Entrypoint
#      ["/sbin/init"]` as podrunner's hosted snapshot declares; `run -d
#      --podvm-init=image` of it runs the command beside the init with no
#      such prefix: its line is in `logs` and its exit code in `inspect`.
#
# Exit: 0 every clause matched, 1 a clause disagreed, 2 the lane could not
# run (not root, no user namespaces, no package).
set -u

REPO="$(pwd)"
WORK="$REPO/experiments/.sweep434-work"
rm -rf "$WORK"; mkdir -p "$WORK" || exit 2
REPORT="$WORK/out-434.txt"
RUNS=${PODVM_434_RUNS:-5}
U=podvm434
B=/srv/$U
INCUS_PKG=6.0.0-1ubuntu0.3
OCI='public.ecr.aws/docker/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
# ⚠ PODVM_434_MIRROR=1 takes the same digest from mirror.gcr.io.
[ "${PODVM_434_MIRROR:-0}" = 1 ] && OCI='mirror.gcr.io/library/alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
{
echo "== conditions"
echo "date              $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "host kernel       $(uname -sr)"
echo "cgroup            $(stat -fc %T /sys/fs/cgroup)"
echo "incus package     $INCUS_PKG"
echo "OCI               $OCI"
echo "runs              $RUNS"
} >"$REPORT"
note() { echo "$*" >>"$REPORT"; }
fail=0

[ "$(id -u)" -eq 0 ] || { echo "not root: the one preparation needs root: COULD NOT RUN" | tee -a "$REPORT"; exit 2; }
unshare -Ur true 2>/dev/null || { echo "user namespaces refused: COULD NOT RUN" | tee -a "$REPORT"; exit 2; }
DEBIAN_FRONTEND=noninteractive timeout 600 apt-get install -y -qq "incus=$INCUS_PKG" uidmap >"$WORK/apt.log" 2>&1 ||
	{ echo "apt-get install FAILED: COULD NOT RUN" | tee -a "$REPORT"; exit 2; }
INCUSD=/usr/libexec/incus/incusd
[ -x "$INCUSD" ] || { echo "no $INCUSD: COULD NOT RUN" | tee -a "$REPORT"; exit 2; }
timeout 1800 cargo build --release -p podvm >"$WORK/build.log" 2>&1 || { echo "build FAILED: COULD NOT RUN" | tee -a "$REPORT"; exit 2; }

# ---- the one preparation, by root (as in experiment 433) ---------------
ranged() { ps -eo pid=,uid= | awk -v u="$(id -u "$U" 2>/dev/null || echo -1)" '$2 == u || ($2 >= 2000000000 && $2 < 2100000000) {print $1}'; }
cleanup() {
	i=0
	while [ "$i" -lt 10 ] && [ -n "$(ranged)" ]; do
		ranged | xargs -r kill -9 2>/dev/null; sleep 1; i=$((i + 1))
	done
	rm -rf "$B"
	for c in /sys/fs/cgroup/*/"$U"; do rmdir "$c" 2>/dev/null; done
	case "${FUSE_MODE:--}" in -) ;; *) chmod "$FUSE_MODE" /dev/fuse ;; esac
}
cleanup
id "$U" >/dev/null 2>&1 || useradd -m -s /bin/sh "$U" || exit 2
# T-1996: a host gives /dev/fuse mode 0666; this cloud host gives 0600, which
# the instances of an incusd that is not root cannot open. Restored at the end.
FUSE_MODE=$(stat -c %a /dev/fuse 2>/dev/null || echo -)
[ "$FUSE_MODE" = - ] || chmod 0666 /dev/fuse
for f in /etc/subuid /etc/subgid; do sed -i "/^$U:/d" "$f"; echo "$U:2000000000:100000000" >>"$f"; done
for c in /sys/fs/cgroup/*/; do
	n=$(basename "$c"); [ "$n" = unified ] && continue
	mkdir -p "$c$U" 2>/dev/null || continue
	[ -f "$c/cpuset.cpus" ] && cat "$c/cpuset.cpus" >"$c$U/cpuset.cpus" && cat "$c/cpuset.mems" >"$c$U/cpuset.mems"
	chown -R "$U:$U" "$c$U"
done
mkdir -p "$B/state" /var/log/incus /var/cache/incus
cp "$REPO/target/x86_64-unknown-linux-musl/release/podvm" "$B/podvm"
cat >"$B/ns.sh" <<EOF
#!/bin/sh
set -u
base=$B/state
mkdir -p \$base/lib \$base/log \$base/cache \$base/run/cg
mount --rbind /sys/fs/cgroup \$base/run/cg && mount --rbind \$base/run/cg /sys/fs/cgroup
mount -t proc proc /proc
mount -t tmpfs tmpfs /run && mkdir -p /run/incus
mount --bind \$base/lib /var/lib/incus
mount --bind \$base/log /var/log/incus
mount --bind \$base/cache /var/cache/incus
# T-1996: room for the isolated maps of 231,072 ids of --podvm-init=image.
printf 'root:1000000:10000000\n' >\$base/subid
mount --bind \$base/subid /etc/subuid
mount --bind \$base/subid /etc/subgid
export INCUS_DIR=/var/lib/incus
exec $INCUSD --group root --logfile /var/log/incus/incusd.log
EOF
cat >"$B/start.sh" <<EOF
#!/bin/sh
for h in /sys/fs/cgroup/*/$U; do echo \$\$ >"\$h/cgroup.procs" 2>/dev/null; done
exec unshare --user --map-root-user --map-auto --mount --pid --fork --propagation private sh $B/ns.sh
EOF
chown -R "$U:$U" "$B"

RI() { runuser -u "$U" -- env INCUS_DIR="$B/state/lib" incus "$@" </dev/null; }
# podvm as the user, with the incusd of the user.
P() { runuser -u "$U" -- env PODVM_STORE="$B/store" HOME="$B" INCUS_DIR="$B/state/lib" "$B/podvm" "$@"; }
T() { P run --podvm-tier=incus "$@"; }
ms() { echo $(( $(date +%s%N) / 1000000 )); }
median() { tr ' ' '\n' | grep . | sort -n | awk '{a[NR]=$1} END {print (NR ? a[int((NR+1)/2)] : "-")}'; }

(runuser -u "$U" -- sh "$B/start.sh" >"$WORK/start.log" 2>&1 &)
i=0; while [ "$i" -lt 30 ] && [ ! -S "$B/state/lib/unix.socket" ]; do sleep 1; i=$((i + 1)); done
RI storage create default dir >/dev/null 2>&1
RI profile device add default root disk path=/ pool=default >/dev/null 2>&1
P pull "$OCI" >"$WORK/pull.log" 2>&1 || { echo "podvm pull FAILED: COULD NOT RUN" | tee -a "$REPORT"; cleanup; exit 2; }

# ---- clause 1 -----------------------------------------------------------
t0=$(ms)
out=$(T "$OCI" echo hello </dev/null 2>"$WORK/c1.err"); rc=$?
t1=$(ms)
{ echo ""; echo "== 1. a run"; echo "  stdout  $out"; echo "  exit    $rc"; echo "  ms      $((t1 - t0)) (with the import)"; sed 's/^/  stderr  /' "$WORK/c1.err"; } >>"$REPORT"
[ "$out" = hello ] && [ "$rc" -eq 0 ] || { note "1: the run did not give hello and 0"; fail=1; }
grep -q "incus tier: incusd 6.0.0 at .*imported now" "$WORK/c1.err" || { note "1: no stderr line that names the tier and the import"; fail=1; }

# ---- clause 2 -----------------------------------------------------------
T "$OCI" sh -c 'exit 7' </dev/null >/dev/null 2>&1; rc=$?
{ echo ""; echo "== 2. the exit code"; echo "  exit    $rc"; } >>"$REPORT"
[ "$rc" -eq 7 ] || { note "2: the exit code is not 7"; fail=1; }

# ---- clause 3 -----------------------------------------------------------
out=$(T "$OCI" sh -c 'echo out; echo err >&2' </dev/null 2>"$WORK/c3.err")
{ echo ""; echo "== 3. stdout and stderr"; echo "  stdout  $out"; sed 's/^/  stderr  /' "$WORK/c3.err"; } >>"$REPORT"
[ "$out" = out ] && grep -qx err "$WORK/c3.err" || { note "3: stdout and stderr do not stay apart"; fail=1; }

# ---- clause 4 -----------------------------------------------------------
out=$(echo piped | T "$OCI" cat 2>"$WORK/c4.err")
{ echo ""; echo "== 4. stdin"; echo "  stdout  $out"; } >>"$REPORT"
[ "$out" = piped ] || { note "4: stdin did not reach the command"; fail=1; }

# ---- clause 5 -----------------------------------------------------------
out=$(T -e A=one -w /tmp "$OCI" sh -c 'echo "$A $(pwd)"' </dev/null 2>"$WORK/c5.err")
{ echo ""; echo "== 5. -e and -w"; echo "  stdout  $out"; } >>"$REPORT"
[ "$out" = "one /tmp" ] || { note "5: -e or -w did not reach the command"; fail=1; }

# ---- clause 6 -----------------------------------------------------------
out=$(T "$OCI" sh -c 'tr "\0" " " </proc/1/cmdline; echo; stat -c %u:%g /etc/passwd /etc/shadow' </dev/null 2>"$WORK/c6.err")
pid1=$(echo "$out" | sed -n 1p); owner=$(echo "$out" | sed -n 2p); shadow=$(echo "$out" | sed -n 3p)
{ echo ""; echo "== 6. the instance"; echo "  PID 1                 $pid1"; echo "  owner of /etc/passwd  $owner"; echo "  owner of /etc/shadow  $shadow"; sed 's/^/  stderr  /' "$WORK/c6.err"; } >>"$REPORT"
case "$pid1" in "/.podvm/podvm guest-hold"*) ;; *) note "6: PID 1 is not podvm's hold"; fail=1 ;; esac
[ "$owner" = 0:0 ] && [ "$shadow" = 0:42 ] || { note "6: the files do not have the intended owners"; fail=1; }

# ---- clause 7 -----------------------------------------------------------
R=""
n=0; while [ "$n" -lt "$RUNS" ]; do
	t0=$(ms); T "$OCI" true </dev/null >/dev/null 2>"$WORK/c7.err"; t1=$(ms); R="$R $((t1 - t0))"; n=$((n + 1))
done
{ echo ""; echo "== 7. the cached image"; echo "  run true ms  $(echo "$R" | median) (median of $RUNS)"; sed 's/^/  stderr  /' "$WORK/c7.err"; } >>"$REPORT"
grep -q "from incus" "$WORK/c7.err" || { note "7: the second run imported the image again"; fail=1; }

# ---- clause 8 -----------------------------------------------------------
P exec --podvm-tier=incus none434 true >/dev/null 2>"$WORK/c8.err"; rc_e=$?
{ echo ""; echo "== 8. a name that is no instance"; echo "  exec exit    $rc_e"; sed 's/^/  stderr  /' "$WORK/c8.err"; } >>"$REPORT"
[ "$rc_e" -eq 125 ] && grep -q "none434" "$WORK/c8.err" ||
	{ note "8: exec on no instance did not exit 125 naming it"; fail=1; }

# ---- clause 11 ----------------------------------------------------------
V="$B/vol"; VR="$B/volro"
runuser -u "$U" -- sh -c "mkdir -p '$V' '$VR' && chmod 0777 '$V' '$VR' && echo host-434 >'$V/in.txt'"
out=$(T -v "$V:/data" -v "$VR:/ro:ro" "$OCI" sh -c 'cat /data/in.txt; echo guest-434 > /data/out.txt && echo wrote; touch /ro/x 2>&1; true' </dev/null 2>"$WORK/c11.err"); rc=$?
{ echo ""; echo "== 11. volumes"; echo "  exit    $rc"; echo "$out" | sed 's/^/  stdout  /'; echo "  host    $(cat "$V/out.txt" 2>/dev/null || echo -)"; grep -v "incus tier" "$WORK/c11.err" | sed 's/^/  stderr  /'; } >>"$REPORT"
[ "$rc" -eq 0 ] || { note "11: wanted exit 0, got $rc"; fail=1; }
echo "$out" | grep -qx host-434 || { note "11: the command did not read the host file"; fail=1; }
[ "$(cat "$V/out.txt" 2>/dev/null)" = guest-434 ] || { note "11: the write of the command is not on the host"; fail=1; }
echo "$out" | grep -q "Read-only file system" || { note "11: a write in the :ro volume was not refused"; fail=1; }
[ -e "$VR/x" ] && { note "11: a write in the :ro volume is on the host"; fail=1; }
runuser -u "$U" -- sh -c "mkdir -p '$B/vol755' && chmod 0755 '$B/vol755'"
out=$(T -v "$B/vol755:/d" "$OCI" sh -c 'touch /d/x 2>&1; true' </dev/null 2>/dev/null)
echo "  a directory of mode 0755: $(echo "$out" | head -1 | cut -c1-80)" >>"$REPORT"

# ---- clause 12 ----------------------------------------------------------
I() { P "$@" --podvm-tier=incus; }
{
echo ""; echo "== 12. an instance that stays"
echo "  run -d   $(P run -d --name web434 --podvm-tier=incus "$OCI" sh -c 'echo started; while true; do sleep 1; done' </dev/null 2>/dev/null)"
sleep 2
echo "  ps       $(I ps 2>&1 | grep web434 | tr -s ' ')"
x=$(P exec --podvm-tier=incus web434 sh -c 'echo in-exec; exit 5' 2>&1); rc=$?
echo "  exec     $x exit=$rc"
echo "  logs     $(I logs web434 2>&1)"
echo "  stop     $(I stop web434 2>&1)"
echo "  ps -a    $(I ps -a 2>&1 | grep web434 | tr -s ' ')"
echo "  start    $(I start web434 2>&1)"
sleep 2
echo "  ps       $(I ps 2>&1 | grep web434 | tr -s ' ')"
x=$(I rm web434 2>&1); rc=$?
echo "  rm       $x exit=$rc"
x=$(I rm -f web434 2>&1); rc=$?
echo "  rm -f    $x exit=$rc"
echo "  create   $(P create --name made434 --podvm-tier=incus "$OCI" true </dev/null 2>/dev/null)"
echo "  ps -a    $(I ps -a 2>&1 | grep made434 | tr -s ' ')"
echo "  run -d   $(P run -d --name brief434 --podvm-tier=incus "$OCI" sh -c 'echo brief; exit 3' </dev/null 2>/dev/null)"
sleep 5
echo "  ps -a    $(I ps -a 2>&1 | grep brief434 | tr -s ' ')"
echo "  logs     $(I logs brief434 2>&1)"
echo "  rm       $(I rm made434 brief434 2>&1 | tr '\n' ' ')"
echo "  ps -a    $(I ps -a 2>&1 | grep -c 434) of 434"
} >"$WORK/c12.txt" 2>&1
cat "$WORK/c12.txt" >>"$REPORT"
c12() { grep -q "$1" "$WORK/c12.txt" || { note "12: $2"; fail=1; }; }
c12 "^  run -d   web434$" "run -d did not print the name"
c12 "^  ps       web434 Running" "ps did not list the instance Running"
c12 "^  exec     in-exec exit=5$" "exec did not give the output and the exit code"
c12 "^  logs     started$" "logs did not give the output of the command"
c12 "^  ps -a    web434 Stopped" "stop did not make it Stopped"
c12 "^  rm       .*the instance is Running.* exit=1$" "rm of a running instance did not refuse"
c12 "^  rm -f    web434 exit=0$" "rm -f did not remove it"
c12 "^  ps -a    made434 Stopped" "create did not make a Stopped instance"
c12 "^  ps -a    brief434 Stopped" "a command that ended did not stop its instance"
c12 "^  logs     brief$" "logs of the ended command"
c12 "^  ps -a    0 of 434$" "an instance of clause 12 stayed"

# ---- clause 13 ----------------------------------------------------------
# T-1722: `tier = incus` in the store's configuration selects the tier with
# no flag; the lifecycle verbs route a name of an instance of podvm.
{
echo ""; echo "== 13. tier = incus in the configuration"
runuser -u "$U" -- sh -c "printf 'tier = incus\n' >> '$B/store/config'"
echo "  entered  $(P system info --format '{{.EnteredRung}}' 2>&1)"
echo "  run      $(P run --rm "$OCI" sh -c 'tr "\000" " " < /proc/1/cmdline' </dev/null 2>/dev/null)"
echo "  run -d   $(P run -d --name cfg434 "$OCI" sh -c 'echo one; sleep 3; echo two; sleep 1' </dev/null 2>/dev/null)"
x=$(P exec cfg434 sh -c 'exit 6' 2>&1); rc=$?
echo "  exec     exit=$rc"
echo "  inspect  $(P inspect --format '{{.Name}} {{.Status}} {{.EnteredRung}}' cfg434 2>&1)"
echo "  follow   $(timeout 60 runuser -u "$U" -- env PODVM_STORE="$B/store" HOME="$B" INCUS_DIR="$B/state/lib" "$B/podvm" logs -f cfg434 2>&1 | tr '\n' ' ')"
echo "  exitcode $(P inspect --format '{{.ExitCode}}' cfg434 2>&1)"
echo "  rm       $(P rm cfg434 2>&1)"
runuser -u "$U" -- sed -i '/^tier = incus$/d' "$B/store/config"
echo "  ladder   $(P system info --format '{{.EnteredRung}}' 2>&1)"
} >"$WORK/c13.txt" 2>&1
cat "$WORK/c13.txt" >>"$REPORT"
c13() { grep -q "$1" "$WORK/c13.txt" || { note "13: $2"; fail=1; }; }
c13 "^  entered  incus$" "system info did not name incus as the entered rung"
c13 "^  run      .*guest-hold" "a run with no flag did not enter incus"
c13 "^  run -d   cfg434$" "run -d with no flag did not make an instance"
c13 "^  exec     exit=6$" "exec of the name did not route to incus"
c13 "^  inspect  cfg434 Running incus$" "inspect --format of the instance"
c13 "^  follow   one two $" "logs -f did not follow the log to the end"
c13 "^  exitcode 0$" "inspect did not give the exit code of the ended command"
c13 "^  rm       cfg434$" "rm of the name did not route to incus"
grep -q "^  ladder   incus$" "$WORK/c13.txt" && { note "13: the ladder still named incus with no configuration"; fail=1; }

# ---- clause 14 ----------------------------------------------------------
{
echo ""; echo "== 14. run -d -i"
echo "  run -d   $(printf 'sekret434\n' | P run -d -i --name in434 --podvm-tier=incus "$OCI" sh -c 'cat; ls /.podvm/stdin 2>/dev/null || echo gone' 2>/dev/null)"
sleep 5
echo "  logs     $(I logs in434 2>&1 | tr '\n' ' ')"
echo "  cmd      $(I inspect --format '{{.Cmd}}' in434 2>&1)"
echo "  rm       $(I rm -f in434 2>&1)"
} >"$WORK/c14.txt" 2>&1
cat "$WORK/c14.txt" >>"$REPORT"
c14() { grep -q "$1" "$WORK/c14.txt" || { note "14: $2"; fail=1; }; }
c14 "^  run -d   in434$" "run -d -i did not print the name"
c14 "^  logs     sekret434 gone $" "the command did not read the stdin, or the file stayed"
grep -q "^  cmd .*sekret434" "$WORK/c14.txt" && { note "14: the stdin is in the recorded command"; fail=1; }

# ---- clause 15 ----------------------------------------------------------
cat >"$B/srv434.py" <<'EOF'
import socketserver, http.server, sys
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        b = b"host434\n"
        self.send_response(200); self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def log_message(self, *a):
        sys.stderr.write("request %s\n" % self.path); sys.stderr.flush()
class S(socketserver.ThreadingMixIn, socketserver.UnixStreamServer): pass
S(sys.argv[1], H).serve_forever()
EOF
chown "$U" "$B/srv434.py"
runuser -u "$U" -- python3 "$B/srv434.py" "$B/ch434.sock" 2>"$B/ch434.log" &
SRV=$!
i=0; while [ "$i" -lt 50 ] && [ ! -S "$B/ch434.sock" ]; do sleep 0.1; i=$((i + 1)); done
# The root of the instance is an id of the user's range: the socket file
# allows it, and the directory of the caller is the guard (as podrunner's).
chmod 0666 "$B/ch434.sock"
{
echo ""; echo "== 15. --podvm-net none and --podvm-channel"
if RI network create br434 ipv4.address=10.234.0.1/24 ipv4.nat=false ipv6.address=none >/dev/null 2>&1 &&
	RI profile device add default eth0 nic network=br434 >/dev/null 2>&1; then
	echo "  profile  a NIC eth0 on br434"
else
	echo "  profile  no NIC: the incusd made no bridge"
fi
echo "  net      $(T "$OCI" ls /sys/class/net </dev/null 2>/dev/null | tr '\n' ' ')"
echo "  none     $(T --podvm-net none "$OCI" ls /sys/class/net </dev/null 2>/dev/null | tr '\n' ' ')"
echo "  channel  $(T --podvm-net none --podvm-channel "$B/ch434.sock:127.0.0.1:3128" "$OCI" sh -c 'ls /sys/class/net; wget -qO- http://127.0.0.1:3128/run434' </dev/null 2>/dev/null | tr '\n' ' ')"
echo "  run -d   $(printf 'x\n' | P run -d -i --name ch434 --podvm-tier=incus --podvm-net none --podvm-channel "$B/ch434.sock:127.0.0.1:3128" "$OCI" sh -c 'cat >/dev/null; sleep 2; wget -qO- http://127.0.0.1:3128/detach434' 2>/dev/null)"
sleep 10
echo "  logs     $(I logs ch434 2>&1 | tr '\n' ' ')"
echo "  rm       $(I rm -f ch434 2>&1)"
echo "  socket   $(tr '\n' ' ' <"$B/ch434.log")"
RI profile device remove default eth0 >/dev/null 2>&1
RI network delete br434 >/dev/null 2>&1
} >"$WORK/c15.txt" 2>&1
kill "$SRV" 2>/dev/null
cat "$WORK/c15.txt" >>"$REPORT"
c15() { grep -q "$1" "$WORK/c15.txt" || { note "15: $2"; fail=1; }; }
c15 "^  none     lo $" "--podvm-net none did not leave lo only"
c15 "^  channel  lo host434 $" "the run did not reach the host socket through the channel"
c15 "^  run -d   ch434$" "run -d -i with the flags did not print the name"
c15 "^  logs     host434 $" "the instance that stays did not reach the host socket"
c15 "request /run434.*request /detach434" "the host socket did not log both requests"
if grep -q "^  profile  a NIC" "$WORK/c15.txt"; then
	c15 "^  net      eth0 lo $" "a run with no flag did not show the NIC of the profile"
fi

# ---- clause 16 ----------------------------------------------------------
# TODO/machine.md T-1966 (PodUtils/podrunner#296 ask 3): --podvm-engine on
# the incus tier lets dockerd run in an unprivileged instance with no NIC.
DIND=${PODVM_434_DIND:-mirror.gcr.io/library/docker:dind}
echo ""; echo "== 16. --podvm-engine: a container engine in an instance" >>"$REPORT"
{
P pull -q "$DIND" >/dev/null 2>&1
echo "  run -d   $(P run -d --name eng434 --podvm-tier=incus --podvm-engine --podvm-net none "$DIND" dockerd --iptables=false --ip6tables=false --bridge=none </dev/null 2>/dev/null)"
i=0; drv=""
while [ $i -lt 60 ] && [ -z "$drv" ]; do drv=$(P exec --podvm-tier=incus eng434 docker info --format '{{.Driver}}' 2>/dev/null); i=$((i+1)); [ -n "$drv" ] || sleep 2; done
echo "  driver   ${drv:--}"
echo "  nesting  $(RI config get eng434 security.nesting 2>&1)"
echo "  root     $(RI exec eng434 -- stat -c %u / 2>&1) in the instance"
echo "  rm       $(P rm -f --podvm-tier=incus eng434 2>&1)"
} >"$WORK/c16.txt" 2>&1
cat "$WORK/c16.txt" >>"$REPORT"
c16() { grep -q "$1" "$WORK/c16.txt" || { note "16: $2"; fail=1; }; }
c16 "^  run -d   eng434$" "run -d --podvm-engine did not start the instance"
c16 "^  driver   [a-z]" "dockerd did not answer docker info in the instance"
c16 "^  nesting  true$" "the instance has no security.nesting"

# ---- clauses 17 and 18 --------------------------------------------------
# TODO/machine.md T-1964, T-1965 (PodUtils/podrunner#296 asks 1 and 2).
echo ""; echo "== 17. --storage-opt size= on a pool of the driver dir" >>"$REPORT"
{
out=$(P run -d --name sz434 --podvm-tier=incus --storage-opt size=1GiB "$OCI" sleep 30 </dev/null 2>&1); rc=$?
echo "  run -d   rc $rc: $(printf '%s' "$out" | tail -1)"
echo "  left     $(RI list --format csv -c n 2>/dev/null | grep -c '^sz434$')"
echo "  native   $(P run --rm --storage-opt size=1GiB "$OCI" true 2>&1 | tr '\n' ' ') rc $(P run --rm --storage-opt size=1GiB "$OCI" true >/dev/null 2>&1; echo $?)"
} >"$WORK/c17.txt" 2>&1
cat "$WORK/c17.txt" >>"$REPORT"
c17() { grep -q "$1" "$WORK/c17.txt" || { note "17: $2"; fail=1; }; }
c17 "^  run -d   rc 125: .*holds no size of its root disk" "a size that the pool does not hold did not refuse by name"
c17 "^  left     0$" "a refused size left an instance"
c17 "^  native   .*has no root disk to size.* rc 125$" "a native run did not refuse --storage-opt by name"
echo ""; echo "== 18. the CPU time and the peak memory in inspect" >>"$REPORT"
{
P run -d --name use434 --podvm-tier=incus "$OCI" sh -c 'i=0; while [ $i -lt 300000 ]; do i=$((i+1)); done; x=$(head -c 30000000 /dev/zero | tr "\0" a); sleep 60' </dev/null >/dev/null 2>&1
sleep 12
echo "  usage    $(P inspect --podvm-tier=incus use434 --format '{{.CpuSeconds}} {{.PeakMemoryBytes}} {{.OomKills}}' 2>&1)"
P rm -f --podvm-tier=incus use434 >/dev/null 2>&1
} >"$WORK/c18.txt" 2>&1
cat "$WORK/c18.txt" >>"$REPORT"
grep -Eq "^  usage    [0-9]+\.[0-9]+ [0-9]+ " "$WORK/c18.txt" || { note "18: inspect gave no CPU time or peak memory"; fail=1; }

# ---- clause 19 ----------------------------------------------------------
# TODO/machine.md T-1993 (PodUtils/podrunner#313 ask 1): the init of the
# image (systemd) as PID 1 of an instance that stays.
INIT_IMG=${PODVM_434_INIT:-registry.access.redhat.com/ubi9/ubi-init:latest}
echo ""; echo "== 19. --podvm-init=image: systemd as PID 1" >>"$REPORT"
{
P pull -q "$INIT_IMG" >/dev/null 2>&1
echo "  run -d   $(P run -d --name sd434 --podvm-tier=incus --podvm-init=image "$INIT_IMG" sh -c 'echo "state $(systemctl is-system-running)"; echo "pid1 $(cat /proc/1/comm)"; systemctl start systemd-tmpfiles-clean.service && echo "unit started"; sleep 20; exit 7' </dev/null 2>&1 | tail -1)"
echo "  exec     $(P exec --podvm-tier=incus sd434 systemctl is-system-running 2>&1 | tail -1)"
i=0; st=""
while [ $i -lt 30 ]; do st=$(RI list --format csv -c ns 2>/dev/null | sed -n 's/^sd434,//p'); [ "$st" = STOPPED ] && break; i=$((i+1)); sleep 2; done
echo "  stopped  ${st:--}"
echo "  exitcode $(P inspect --podvm-tier=incus --format '{{.ExitCode}}' sd434 2>&1)"
echo "  logs     $(P logs --podvm-tier=incus sd434 2>&1 | tr '\n' ' ')"
echo "  rm       $(P rm -f --podvm-tier=incus sd434 2>&1)"
out=$(P run --rm --podvm-init=image "$OCI" true </dev/null 2>&1); rc=$?
echo "  native   rc $rc: $(printf '%s' "$out" | tail -1)"
} >"$WORK/c19.txt" 2>&1
cat "$WORK/c19.txt" >>"$REPORT"
c19() { grep -Eq "$1" "$WORK/c19.txt" || { note "19: $2"; fail=1; }; }
c19 "^  run -d   sd434$" "run -d --podvm-init=image did not start the instance"
c19 "^  exec     (running|degraded)$" "exec did not reach a running systemd"
c19 "^  stopped  STOPPED$" "the instance did not stop when its command ended"
c19 "^  logs     state (running|degraded) pid1 systemd " "the command did not run after the boot of systemd as PID 1"
c19 "^  logs     .*unit started " "systemctl start of a unit of the image did not work"
c19 "^  exitcode 7$" "inspect did not give the exit code of the command"
c19 "^  native   rc 0: .*the run takes the .* (tier|rung)" "the flag off the incus tier did not name the route that it took (call C26)"

# ---- clause 20 ----------------------------------------------------------
echo ""; echo "== 20. the hosted snapshot: ids, /dev/fuse, a machine-id of each" >>"$REPORT"
{
for n in sna434 snb434; do
  echo "  run -d   $(P run -d --name $n --podvm-tier=incus --podvm-init=image "$INIT_IMG" sh -c 'echo "ids $(awk "{s+=\$3} END {print s}" /proc/self/uid_map)"; test -c /dev/fuse && (exec 3<>/dev/fuse) 2>/dev/null && echo "fuse open"; echo "mid $(cat /etc/machine-id)"; sleep 5' </dev/null 2>&1 | tail -1)"
done
for n in sna434 snb434; do
  i=0; while [ $i -lt 30 ]; do st=$(RI list --format csv -c ns 2>/dev/null | sed -n "s/^$n,//p"); [ "$st" = STOPPED ] && break; i=$((i+1)); sleep 2; done
  echo "  logs $n $(P logs --podvm-tier=incus $n 2>&1 | tr '\n' ' ')"
  P rm -f --podvm-tier=incus $n >/dev/null 2>&1
done
} >"$WORK/c20.txt" 2>&1
cat "$WORK/c20.txt" >>"$REPORT"
c20() { grep -Eq "$1" "$WORK/c20.txt" || { note "20: $2"; fail=1; }; }
c20 "^  run -d   sna434$" "run -d of the first instance did not start"
ids=$(sed -n 's/^  logs sna434 ids \([0-9]*\).*/\1/p' "$WORK/c20.txt")
[ "${ids:-0}" -ge 231072 ] || { note "20: the instance maps ${ids:-no} ids, fewer than 231,072"; fail=1; }
c20 "^  logs sna434 .*fuse open" "/dev/fuse did not open in the instance"
a=$(sed -n 's/^  logs sna434 .*mid \([0-9a-f]\{32\}\).*/\1/p' "$WORK/c20.txt")
b=$(sed -n 's/^  logs snb434 .*mid \([0-9a-f]\{32\}\).*/\1/p' "$WORK/c20.txt")
echo "  machine-ids ${a:--} ${b:--}" >>"$REPORT"
{ [ -n "$a" ] && [ -n "$b" ] && [ "$a" != "$b" ]; } || { note "20: the two instances have no machine-ids of their own"; fail=1; }

# ---- clause 21 ----------------------------------------------------------
echo ""; echo "== 21. a large import: the growth of the disk use" >>"$REPORT"
BIG=${PODVM_434_BIG:-mirror.gcr.io/library/golang:1.24-bookworm}
disk() { du -sb "$B/store" "$B/state/lib" 2>/dev/null | awk '{s+=$1} END {print s+0}'; }
{
P pull -q "$BIG" >/dev/null 2>&1; echo "  pull     rc $?"
# The tree is extracted before the measure, so the growth is the import.
P run --rm "$BIG" true >/dev/null 2>&1; echo "  extract  rc $?"
tree=0
for d in "$B"/store/rootfs/*/rootfs "$B"/store/trees/*/rootfs; do
  [ -d "$d" ] || continue
  n=$(du -sb "$d" 2>/dev/null | cut -f1); [ "${n:-0}" -gt "$tree" ] && tree=$n
done
base=$(disk)
: >"$WORK/peak"
( while :; do disk >>"$WORK/peak"; sleep 1; done ) &
sampler=$!
out=$(T "$BIG" true </dev/null 2>&1); rc=$?
kill "$sampler" 2>/dev/null; wait "$sampler" 2>/dev/null
peak=$(sort -n "$WORK/peak" | tail -1)
echo "  run      rc $rc $(printf '%s' "$out" | tail -1)"
echo "  tree     $tree"
echo "  base     $base"
echo "  peak     ${peak:-0}"
echo "  growth   $(( ${peak:-0} - base ))"
echo "  left     $(ls "$B/store/incus" 2>/dev/null | wc -l) files in store/incus"
} >"$WORK/c21.txt" 2>&1
cat "$WORK/c21.txt" >>"$REPORT"
c21() { grep -Eq "$1" "$WORK/c21.txt" || { note "21: $2"; fail=1; }; }
c21 "^  run      rc 0" "the run of the large image did not pass"
c21 "^  left     0 files" "a tarball file stayed in the store"
tree=$(sed -n 's/^  tree     //p' "$WORK/c21.txt"); growth=$(sed -n 's/^  growth   //p' "$WORK/c21.txt")
[ "${tree:-0}" -gt 100000000 ] || { note "21: the tree of the large image was not found (${tree:-0} bytes)"; fail=1; }
[ "${growth:-0}" -le $(( ${tree:-0} * 26 / 10 )) ] || { note "21: the import grew the disk by $growth bytes, more than 2.6 times the tree of $tree"; fail=1; }

# ---- clause 22 ----------------------------------------------------------
echo ""; echo "== 22. the capabilities of a file in the root of an instance" >>"$REPORT"
cat >"$B/mkcap.py" <<'PYEOF'
import hashlib, io, json, sys, tarfile
src, dst = sys.argv[1], sys.argv[2]
blobs, meta = {}, {}
with tarfile.open(src) as t:
    for m in t.getmembers():
        if m.isfile():
            blobs[m.name] = t.extractfile(m).read()
idx = json.loads(blobs["index.json"])
mdesc = idx["manifests"][0]
man = json.loads(blobs["blobs/" + mdesc["digest"].replace(":", "/")])
cfg = json.loads(blobs["blobs/" + man["config"]["digest"].replace(":", "/")])
# vfs_cap_data v2: magic with the effective flag, then cap_net_raw (13) permitted.
cap = (0x02000001).to_bytes(4, "little") + (1 << 13).to_bytes(4, "little") + bytes(12)
lay = io.BytesIO()
with tarfile.open(fileobj=lay, mode="w", format=tarfile.PAX_FORMAT) as t:
    for d in ("usr", "usr/bin"):
        i = tarfile.TarInfo(d); i.type = tarfile.DIRTYPE; i.mode = 0o755; t.addfile(i)
    data = b"cap434\n"
    i = tarfile.TarInfo("usr/bin/capfile"); i.size = len(data); i.mode = 0o755
    i.pax_headers = {"SCHILY.xattr.security.capability": cap.decode("utf-8", "surrogateescape")}
    t.addfile(i, io.BytesIO(data))
layer = lay.getvalue()
def put(b):
    d = "sha256:" + hashlib.sha256(b).hexdigest()
    blobs["blobs/" + d.replace(":", "/")] = b
    return d
blobs.pop("blobs/" + mdesc["digest"].replace(":", "/"))
blobs.pop("blobs/" + man["config"]["digest"].replace(":", "/"))
ld = put(layer)
man["layers"].append({"mediaType": "application/vnd.oci.image.layer.v1.tar", "digest": ld, "size": len(layer)})
cfg["rootfs"]["diff_ids"].append(ld)
cb = json.dumps(cfg).encode(); man["config"]["digest"] = put(cb); man["config"]["size"] = len(cb)
mb = json.dumps(man).encode(); mdesc["digest"] = put(mb); mdesc["size"] = len(mb)
mdesc.setdefault("annotations", {})["org.opencontainers.image.ref.name"] = "localhost/cap434:1"
blobs["index.json"] = json.dumps(idx).encode()
with tarfile.open(dst, "w") as t:
    for name, b in blobs.items():
        i = tarfile.TarInfo(name); i.size = len(b); t.addfile(i, io.BytesIO(b))
print(ld)
PYEOF
{
P save -o "$B/alp.tar" "$OCI" >/dev/null 2>&1; echo "  save     rc $?"
python3 -I "$B/mkcap.py" "$B/alp.tar" "$B/cap.tar" >"$WORK/c22.layer" 2>&1; echo "  build    rc $?"
chown "$U:$U" "$B/cap.tar" 2>/dev/null
echo "  load     $(P load -i "$B/cap.tar" 2>&1 | tail -1)"
echo "  run -d   $(P run -d --name cap434 --podvm-tier=incus localhost/cap434:1 sleep 30 </dev/null 2>&1 | tail -1)"
f="$B/state/lib/storage-pools/default/containers/cap434/rootfs/usr/bin/capfile"
echo "  cap      $(python3 -I -c 'import os,sys; print(os.getxattr(sys.argv[1], "security.capability").hex())' "$f" 2>&1 | tail -1)"
echo "  sidecar  $(grep -rh '"usr/bin/capfile"' "$B"/store/rootfs/*/.meta.jsonl "$B"/store/trees/*/.meta.jsonl 2>/dev/null | grep -o '"xattrs":[^]]*]' | head -1)"
P rm -f --podvm-tier=incus cap434 >/dev/null 2>&1
} >"$WORK/c22.txt" 2>&1
cat "$WORK/c22.txt" >>"$REPORT"
c22() { grep -Eq "$1" "$WORK/c22.txt" || { note "22: $2"; fail=1; }; }
c22 "^  run -d   cap434$" "run -d of the image with a capability did not start"
c22 "^  cap      0100000[23]00200000" "the root of the instance has no cap_net_raw on /usr/bin/capfile"

# ---- clause 23 ----------------------------------------------------------
echo ""; echo "== 23. the OOM kills in the run report" >>"$REPORT"
{
P run --rm --podvm-tier=incus --memory 32m --report "$B/r23.json" "$OCI" sh -c 'head -c 200000000 /dev/zero | tail' </dev/null >/dev/null 2>"$WORK/c23.err"; echo "  oom      rc $?"
echo "  report   $(grep -o '"oom_kills": *[0-9]*' "$B/r23.json" 2>/dev/null || echo none)"
P run --rm --podvm-tier=incus --report "$B/r23b.json" "$OCI" true </dev/null >/dev/null 2>&1; echo "  true     rc $?"
echo "  report   $(grep -o '"oom_kills": *[0-9]*' "$B/r23b.json" 2>/dev/null || echo none)"
} >"$WORK/c23.txt" 2>&1
cat "$WORK/c23.txt" >>"$REPORT"
c23() { grep -Eq "$1" "$WORK/c23.txt" || { note "23: $2"; fail=1; }; }
c23 "^  oom      rc 137$" "a command over --memory did not end with 137"
c23 '^  report   "oom_kills": *[1-9]' "the report of an OOM kill has no oom_kills of 1 or more"
c23 '^  report   ("oom_kills": *0|none)$' "the report of true has oom_kills above 0"

# ---- clause 24 ----------------------------------------------------------
echo ""; echo "== 24. an image whose Entrypoint is its init" >>"$REPORT"
cat >"$B/mkentry.py" <<'PYEOF'
import hashlib, io, json, sys, tarfile
src, dst = sys.argv[1], sys.argv[2]
blobs = {}
with tarfile.open(src) as t:
    for m in t.getmembers():
        if m.isfile():
            blobs[m.name] = t.extractfile(m).read()
idx = json.loads(blobs["index.json"])
mdesc = idx["manifests"][0]
man = json.loads(blobs["blobs/" + mdesc["digest"].replace(":", "/")])
cfg = json.loads(blobs["blobs/" + man["config"]["digest"].replace(":", "/")])
cfg.setdefault("config", {})["Entrypoint"] = ["/sbin/init"]
cfg["config"]["Cmd"] = None
blobs.pop("blobs/" + mdesc["digest"].replace(":", "/"))
blobs.pop("blobs/" + man["config"]["digest"].replace(":", "/"))
for extra in idx["manifests"][1:]:
    blobs.pop("blobs/" + extra["digest"].replace(":", "/"), None)
def put(b):
    d = "sha256:" + hashlib.sha256(b).hexdigest()
    blobs["blobs/" + d.replace(":", "/")] = b
    return d
cb = json.dumps(cfg).encode(); man["config"]["digest"] = put(cb); man["config"]["size"] = len(cb)
mb = json.dumps(man).encode(); mdesc["digest"] = put(mb); mdesc["size"] = len(mb)
mdesc.setdefault("annotations", {})["org.opencontainers.image.ref.name"] = "localhost/entry434:1"
idx["manifests"] = [mdesc]
blobs["index.json"] = json.dumps(idx).encode()
with tarfile.open(dst, "w") as t:
    for name, b in blobs.items():
        i = tarfile.TarInfo(name); i.size = len(b); t.addfile(i, io.BytesIO(b))
PYEOF
{
P save -o "$B/init.tar" "$INIT_IMG" >/dev/null 2>&1; echo "  save     rc $?"
python3 -I "$B/mkentry.py" "$B/init.tar" "$B/entry.tar" >"$WORK/c24.py" 2>&1; echo "  build    rc $?"
chown "$U:$U" "$B/entry.tar" 2>/dev/null
echo "  load     $(P load -i "$B/entry.tar" 2>&1 | tail -1)"
echo "  run -d   $(P run -d --name en434 --podvm-tier=incus --podvm-init=image localhost/entry434:1 sh -c 'echo "started $(cat /proc/1/comm)"; exit 3' </dev/null 2>&1 | tail -1)"
i=0; st=""
while [ $i -lt 30 ]; do st=$(RI list --format csv -c ns 2>/dev/null | sed -n 's/^en434,//p'); [ "$st" = STOPPED ] && break; i=$((i+1)); sleep 2; done
echo "  stopped  ${st:--}"
echo "  exitcode $(P inspect --podvm-tier=incus --format '{{.ExitCode}}' en434 2>&1)"
echo "  logs     $(P logs --podvm-tier=incus en434 2>&1 | tr '\n' ' ')"
P rm -f --podvm-tier=incus en434 >/dev/null 2>&1
} >"$WORK/c24.txt" 2>&1
cat "$WORK/c24.txt" >>"$REPORT"
c24() { grep -Eq "$1" "$WORK/c24.txt" || { note "24: $2"; fail=1; }; }
c24 "^  run -d   en434$" "run -d of an image whose Entrypoint is its init did not start"
c24 "^  logs     started systemd" "the command did not run beside the init (the Entrypoint was its prefix)"
c24 "^  exitcode 3$" "inspect did not give the exit code of the command"

# ---- clause 9 -----------------------------------------------------------
left_i=$(RI list --format csv -c n 2>&1 | grep -c .)
cleanup
left=$(ranged | wc -l)
{ echo ""; echo "== 9. nothing stays"; echo "  instances in incus              $left_i"; echo "  processes of $U and its range  $left"; } >>"$REPORT"
[ "$left_i" -eq 0 ] || { note "9: an instance stayed in incus"; fail=1; }
[ "$left" -eq 0 ] || { note "9: a process of the user stayed"; fail=1; }

# ---- clause 10 ----------------------------------------------------------
# The store stays for this clause: the cleanup above removed $B, so the
# image is pulled again into a store of root.
S10="$WORK/store10"
out=$(env PODVM_STORE="$S10" INCUS_DIR="$WORK/no-incus" "$REPO/target/x86_64-unknown-linux-musl/release/podvm" run --podvm-tier=incus "$OCI" true </dev/null 2>&1); rc=$?
{ echo ""; echo "== 10. no incusd"; echo "  exit    $rc"; echo "$out" | grep "incus tier" | sed 's/^/  stderr  /'; } >>"$REPORT"
[ "$rc" -eq 125 ] && echo "$out" | grep -q "no incusd answers at $WORK/no-incus/unix.socket" ||
	{ note "10: a run with no incusd did not refuse by name"; fail=1; }
rm -rf "$S10"

{
echo ""
if [ "$fail" -eq 0 ]; then echo "verdict           THE INCUS TIER RUNS"; else echo "verdict           A CLAUSE DISAGREED"; fi
} >>"$REPORT"
cp "$REPORT" "$REPO/experiments/results/incus-tier.txt"
cat "$REPORT"
[ "$fail" -eq 0 ]
