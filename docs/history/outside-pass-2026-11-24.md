# Outside pass, podvm beta 40 candidate

## Host and binary

- Binary: `/var/tmp/beta40/bin/podvm`, `--version`: `podvm 0.2.0-beta.39`; `version --verbose`: commit `41025181feec95f67969b6bf7ffafa472e261ca2-dirty`, rustc 1.99.1, target x86_64-unknown-linux-musl, static-pie, crt-static yes.
- Kernel: Linux 6.18.44-fc-v819 x86_64, 4 CPUs, 16 GiB, no swap.
- cgroup: v1 (hybrid): `/sys/fs/cgroup` is tmpfs, no delegated cgroup2 subtree.
- No `/dev/kvm`. `/dev/net/tun` EACCES for uid 1000. No qemu on PATH (podvm fetched its own static QEMU 11.1.3 during one test, see OR-B40-07).
- Tester uid: 1000 (`setpriv --reuid 1000 --regid 1000 --clear-groups`), HOME `/var/tmp/beta40/op`, PODVM_STORE `/var/tmp/beta40/op/store`. Incus tests as uid 0 with PODVM_STORE `/var/tmp/beta40/rootstore`.
- `probe` (uid 1000): rung `namespace`; `system info`: `.Rung` namespace, `.EnteredRung` namespace; probe legs clone(NEWNS/NEWUTS), mount, fsmount, move_mount, chroot, ptrace ok; setgroups EPERM; supervise and supervision legs ok; machine tier refused (no qemu, no kvm); Landlock ABI 7, openat2 ok; syscall tier ok; emulate tier missing (no qemu-user).
- Incus: private incusd 6.0.0 (`/usr/libexec/incus/incusd --group root`, INCUS_DIR `/var/tmp/beta40/incus`, `incus admin init --minimal`, pool driver `dir`) started and worked. It is stopped now, with its dnsmasq and bridge.
- Prefixes used below: `U="setpriv --reuid 1000 --regid 1000 --clear-groups env HOME=/var/tmp/beta40/op PODVM_STORE=/var/tmp/beta40/op/store TMPDIR=/var/tmp/beta40/op/tmp PATH=/usr/local/bin:/usr/bin:/bin"`, `P=/var/tmp/beta40/bin/podvm`, `R="env PODVM_STORE=/var/tmp/beta40/rootstore PODVM_INCUS_SOCKET=/var/tmp/beta40/incus/unix.socket"` (as root). `A=mirror.gcr.io/library/alpine:3.20`.

## Findings

### OR-B40-01 (P1) `rmi` of a second tag is refused with exit 125 while a running container uses the image
- Command: `$U $P run -d --name cp1 $A sleep 300; $U $P tag $A local/alp:v2; $U $P rmi local/alp:v2; echo $?`
- Observed: `podvm rmi: store: local/alp:v2 is in use by a running container and was not removed. A GC that deletes an extraction ...`, exit 125. The tag stays. After `save`/`rmi`/`load` of the same tag, the same `rmi` untagged it with exit 0 (`Untagged: local/alp:v1`), so the result depends on how the tag was made.
- Expected: docker untags a second name of an image and keeps the image while another name and the container hold it (`Untagged: local/alp:v2`, exit 0). The manual's exit-code table (`system info --format '{{json .ExitCodes}}'`) puts "an image that a container uses (rmi)" under `cli-error`, code 1, not 125.
- Reproduced: yes, 3 times (v1 once, v2 twice).

### OR-B40-02 (P1) `inspect` gives `.Exec.Mode join` with pid sharing for a created container, but `exec` there runs in the host PID namespace
- Command: `$U $P create --name cr3 $A sleep 50; $U $P inspect --format '{{.State}} {{.Exec.Mode}} {{.Exec.Shares}}' cr3; $U $P exec cr3 sh -c 'ls /proc | grep -c "^[0-9]"; readlink /proc/self/ns/pid'`
- Observed: inspect prints `created join filesystem+user+pid+mount`. The exec prints a note ("podvm will enter it anyway, because a fresh chroot shares only the filesystem"), and the payload sees 108 host processes (incusd, the tester's shells, ...) and the host PID namespace `pid:[4026531836]`, the same as the host's.
- Expected: the manual: "`podvm inspect --format '{{.Exec.Mode}}'` says the same thing to a program". For a created container the field must say what exec does there (a fresh chroot that shares the filesystem only), not join with pid.
- Reproduced: yes, 2 times.

### OR-B40-03 (P1) `PODVM_MODE=tmpfs`, `rundir` and `cache` fail for uid 1000 with an internal copy error
- Command: `$U PODVM_MODE=tmpfs $P run --rm $A true` (also `PODVM_MODE=rundir`, and `PODVM_CACHE=1 PODVM_MODE=cache`)
- Observed: exit 125, `podvm run: PODVM_MODE=tmpfs was forced but /var/tmp/beta40/op/store/tmpfs/mnt-.../dev/random: the file could not be copied: the source path is neither a regular file nor a symlink to a regular file`. The rundir and cache forms fail with the same `dev/random` error. As root, `PODVM_MODE=tmpfs` and `rundir` run the payload (exit 0).
- Expected: the manual: "PODVM_MODE (memfd, fuse, tmpfs, rundir, cache) Forces the launch rung of a foreground run. A refusal names the rung when the host does not allow it." This host allows tmpfs mounts in the user namespace (probe: mount ok). The error is a copy fault of podvm's own `dev/random` shim, not a refusal of the host.
- Reproduced: yes, 2 times each for tmpfs, rundir and cache.

### OR-B40-04 (P2) `wait` and `kill` have no incus path; `wait NAME` says "no such container" for a listed instance
- Command: `$R $P run -d --podvm-tier=incus --name i1 $A sh -c '...'; $R $P wait --podvm-tier=incus i1; $R $P wait i1; $R $P kill --podvm-tier=incus i4`
- Observed: `wait --podvm-tier=incus` and `kill --podvm-tier=incus` exit 125 with `unknown option "--podvm-tier=incus"`. `wait i1` exits 1 with `podvm wait: no such container: i1. podvm ps -a lists what there is`, while `ps -a --podvm-tier=incus` lists `i1`. `logs i4`, `stop i4` and `inspect i4` without the flag give the same "no such container"/"no such image" line. A script that does `run -d` then `wait` has no way to get the exit code but a poll of `inspect`.
- Expected: docker's `run -d` / `wait` / `kill` sequence works on each container. The message must say that the name is an incus instance and that `--podvm-tier incus` is needed, not that it does not exist.
- Reproduced: yes, 2 times (wait), 1 time (kill).

### OR-B40-05 (P2) A foreground run on the namespace rung does not reap orphaned processes: zombies accumulate until the run ends
- Command: `$U $P run --rm $A sh -c 'for i in 1 2 3 4 5; do (sleep 0.2 &); done; sleep 5; ps -o pid,ppid,stat,comm | cat'`
- Observed: five `sleep` processes stay in state `Z` with PPID 2 (`podvm-run`, the subreaper) after 5 s. In a detached run (`run -d`, then `exec ps`) the same orphans are reaped.
- Expected: the `--init` refusal says the launcher "is the child subreaper of the payload's tree ... so no flag is needed". A subreaper must reap; docker `--init` reaps. A long foreground job (a CI script that starts daemons) fills the process table with zombies.
- Reproduced: yes, 3 times.

### OR-B40-06 (P2) `--init` and `top` are refused with a reason that is false on the namespace rung
- Command: `$U $P run --rm --init $A true`; `$U $P top cp1`
- Observed: `--init` exit 125: "no PID namespace and no init to run". `top` exit 125: "a chroot shares this machine's process table, so `top` would list the host's processes". But the same run has a PID namespace with `podvm-init` as PID 1 (`ps` inside shows `1 podvm-init`), and the manual's ISOLATION table says a foreground and a detached run of uid 1000 open `pid`.
- Expected: one true line. docker scripts pass `--init` often; as podvm already runs an init as PID 1, docker accepts it.
- Reproduced: yes, 2 times each.

### OR-B40-07 (P2) `doctor` and `probe` say the machine tier is refused while `run` uses it with podvm's own QEMU
- Command: `$U $P run --rm --podvm-init=image mirror.gcr.io/jrei/systemd-ubuntu:24.04 echo hi` (no subuid range, no runsc, no cgroup2: podvm fetched its static QEMU 11.1.3 into `$HOME/.local/share/podvm/tools` and booted a TCG guest, printed `hi`, exit 0); then `$U $P doctor; $U $P probe`
- Observed: doctor exit 1 with `fail qemu-system-x86_64 --version: ... no qemu-system-x86_64 on PATH`, `fix: install QEMU ...` and `machine profile: refused`, and in the same output `ok scope.user-net: /var/tmp/beta40/op/.local/share/podvm/tools/qemu-11.1.3/bin/qemu-system-x86_64 has the user netdev`. probe prints `machine tier refused: qemu-system-x86_64 --version=skip`.
- Expected: the manual: PODVM_QEMU "With no value, podvm uses PATH, then the copy in the store". doctor and probe must check the same program that run uses.
- Reproduced: yes, 2 times (doctor), 1 time (probe).

### OR-B40-08 (P2) `login` prints "Login Succeeded" for any password and does not check it
- Command: `echo wrongpw | $U $P login -u nobody --password-stdin mirror.gcr.io; echo $?`
- Observed: `podvm login: stored for mirror.gcr.io; the registry checks it at the next pull`, `Login Succeeded`, exit 0. The same for a host that does not resolve (`no.such.host.invalid`).
- Expected: docker login authenticates against the registry and exits 1 on a wrong password; a script `docker login ... || exit 1` relies on it.
- Reproduced: yes, 2 times.

### OR-B40-09 (P2) A payload argument that is not UTF-8 is refused
- Command: `$U $P run --rm $A echo $'\xff\xfe'`
- Observed: `podvm: argument 5 (��) is not UTF-8; podvm takes UTF-8 arguments only`, exit 125. The same for a `-e` value.
- Expected: docker passes the payload's argv and environment as bytes. A file name in Latin-1 cannot be given to a payload.
- Reproduced: yes, 2 times.

### OR-B40-10 (P2) A name conflict exits 1 from `create` and 125 from `run`
- Command: `$U $P create --name c1 $A true` (c1 exists); `$U $P run -d --name c1 $A true`; `$U $P run --name c1 $A true`
- Observed: create exit 1, run -d exit 125, foreground run exit 125, all with the same cause ("the name \"c1\" is already used by a container").
- Expected: one exit code for one case. docker gives 125 for both (`Conflict. The container name ... is already in use`).
- Reproduced: yes, 2 times.

### OR-B40-11 (P2) `run --rm` keeps the extracted rootfs of the image
- Command: `$U $P image prune -a; $U $P pull -q mirror.gcr.io/library/busybox:1.36; $U PODVM_SPARE_TREE=0 $P run --rm mirror.gcr.io/library/busybox:1.36 true; $U $P system df`
- Observed: after the run, no container record exists (`ps -a` is empty), and `system df` shows busybox ROOTFS 4.2 MiB and `Images: 1 (1 extracted)`; `store/rootfs/sha256_b7f3...` stays.
- Expected: the manual for `--rm`: "remove the extracted rootfs when the payload exits, unless a container record references it". No record references it.
- Reproduced: yes, 3 times (twice with the spare tree on, once with PODVM_SPARE_TREE=0).

### OR-B40-12 (P2) `--env-file` refuses a line with a bare NAME
- Command: `printf 'D\n' > /var/tmp/beta40/op/env2; $U D=fromhost $P run --rm --env-file /var/tmp/beta40/op/env2 $A sh -c 'echo D=$D'`
- Observed: `podvm run: --env-file "/var/tmp/beta40/op/env2" line 1 has no '=': "D"`, exit 125. `-e D` alone passes the host value.
- Expected: docker's env file takes a bare `NAME` line and passes the value of the host, as `-e NAME` does.
- Reproduced: yes, 2 times.

### OR-B40-13 (P2) `run -it` and `exec -it` give the payload a pty that is not its controlling terminal
- Command: in a pty: `$U $P exec -it d1 sh`, and `$U $P run --rm -it $A sh`, then `sleep 10 & jobs`
- Observed: `sh: can't access tty; job control turned off`. `/proc/self/stat` of the payload shows tty_nr 0; `ps -o tty` shows `?` for the shell.
- Expected: docker `-it` gives an interactive shell with job control (the pty is the controlling terminal of the payload's session).
- Reproduced: yes, 3 times (exec twice, run once).

### OR-B40-14 (P2) `--network host` is refused, although the payload always shares the host network
- Command: `$U $P run --rm --network host $A true`
- Observed: `podvm run: --network takes broker only, not "host": podvm has no network namespace to select`, exit 125.
- Expected: `--network host` is docker's name for exactly what podvm does (ISOLATION: "net: no entry of podvm opens one"). CI scripts often pass it; accepting it is true.
- Reproduced: yes, 2 times.

### OR-B40-15 (P2) `--format '{{json .}}'` and the `table` prefix are refused by `images`, `ps` and `inspect`
- Command: `$U $P images --format '{{json .}}'; $U $P ps --format 'table {{.Names}}\t{{.Status}}'; $U $P inspect --format '{{index .Config.Cmd 0}}' cp1`
- Observed: exit 1, `there is no field "."`, `"table" is not something podvm's --format understands`, `"index .Config.Cmd 0" is not something podvm's --format understands`. With a mixed template `{{.State.Running}} {{.Config.Image}} {{index .Config.Cmd 0}}` the error names `.State.Running` (a valid field) and lists the image fields.
- Expected: docker scripts use `{{json .}}` and `table` very often. The error must name the part that fails.
- Reproduced: yes, 2 times.

### OR-B40-16 (P2) `cp` refuses a container path with `..` or an absolute symlink that docker resolves in the container root
- Command: in container cp1: `ln -s / /root/hostroot`; `$U $P cp cp1:/root/hostroot/etc/hostname /var/tmp/beta40/op/cpt/hn`; `$U $P cp cp1:/etc/../../../../etc/shadow /var/tmp/beta40/op/cpt/sh`
- Observed: exit 1, "resolves to /etc/hostname, which is outside the store", and "has `..`, which podvm refuses". No host file leaks (good).
- Expected: docker cp resolves both inside the container root (the container's `/etc/hostname` and `/etc/shadow`). Absolute symlinks are common in images (`/etc/ssl1.1/certs -> /etc/ssl/certs` in alpine).
- Reproduced: yes, 2 times (one each, plus the write direction).

### OR-B40-17 (P2) `run -u` does not drop privileges and a default run says nothing
- Command: `$U $P run --rm -u nobody mirror.gcr.io/library/python:3.13-slim python3 -c 'import os; print(os.getuid()); open("/root/x","w")'`; `$U $P run --rm -u 1000:1000 $A sh -c 'touch /x; echo $?'`
- Observed: uid prints 65534, the write to `/root/x` and to `/` succeeds, exit 0, no line on stderr. `exec -u` prints one line ("--user is faking the identity"), `run -u` prints none.
- Expected: docker gives EACCES. The manual documents the identity memo and that "the banner names" it, but the banner is off by default, so a user who uses `-u` to lower privileges is not told. Print the one line on run too.
- Reproduced: yes, 2 times.

### OR-B40-18 (P2) An incus instance that was created and not started reads `Stopped`, and `--verbose` gives a false reason for the dashes
- Command: `$R $P create --podvm-tier=incus --name ic3 $A true; $R $P inspect --podvm-tier=incus --verbose --format '{{.Status}} {{.ExitCode}}' ic3`
- Observed: `Stopped -`, and `a usage value is a dash: ... (404) ...: a podvm before 0.2.0-beta.39 made the instance, or its init did not end on its own (a forced stop)`. Neither is true: this podvm made it, and it never ran. `ps -a --podvm-tier=incus` shows `Stopped`.
- Expected: docker shows `Created`; the manual: "a dash is a value that is not known, and --verbose says why".
- Reproduced: yes, 3 times (ic1, ic2, ic3).

### OR-B40-19 (P3) podvm's QEMU copy goes under $HOME, not under PODVM_STORE
- Command: the run of OR-B40-07 with `PODVM_STORE=/var/tmp/beta40/op/store`
- Observed: `the store copy of QEMU 11.1.3 is in /var/tmp/beta40/op/.local/share/podvm/tools/qemu-11.1.3` (41 MiB).
- Expected: PODVM_TOOLS_DIR "Names the directory of the copy of the emulator in the store"; with PODVM_STORE set, the store is that directory.
- Reproduced: yes, 1 time (a second fetch would boot a second guest; not repeated). The doctor lines of OR-B40-07 show the path twice.

### OR-B40-20 (P3) The log of a named foreground run is not in the order the payload wrote
- Command: `$U $P run --name fg2 $A sh -c 'echo out1; echo err1 >&2; echo out2; echo err2 >&2'; $U $P logs fg2`
- Observed: `out1 out2 err1 err2` (and once `err1 out1 out2`). A detached run of the same command logs `out1 err1 out2 err2`.
- Expected: logs: "the payload's stdout and stderr, interleaved into one file".
- Reproduced: yes, 3 times.

### OR-B40-21 (P3) `--memory` with a value that overflows says "takes at least 6m"
- Command: `$U $P run --rm --memory 99999999999g $A true`
- Observed: `--memory "99999999999g": takes at least 6m, as docker does`, exit 125 (also `17179869184g`, `17179869185g`).
- Expected: a message that says the value is too large.
- Reproduced: yes, 4 times.

### OR-B40-22 (P3) `load` of a bad file prints raw bytes of the file on stderr
- Command: `head -c 1000 /dev/urandom > junk.tar; $U $P load -i junk.tar`
- Observed: `podvm load: ... does not unpack: numeric field did not have utf-8 text: <raw binary bytes> when getting cksum for <raw binary bytes>`, exit 1. Control bytes of the file reach the terminal.
- Expected: one printable line.
- Reproduced: yes, 2 times.

### OR-B40-23 (P3) `stop -t -1` says `unknown option "-1"`
- Command: `$U $P stop -t -1 nosuch`
- Observed: `podvm stop: unknown option "-1"`, exit 125. `-1` is the value of `-t`.
- Expected: a line about the value of `-t` (docker takes -1 as "no timeout").
- Reproduced: yes, 2 times.

### OR-B40-24 (P3) A relative `-w` is taken as absolute
- Command: `$U $P run --rm -w tmp $A pwd`
- Observed: `/tmp`, exit 0.
- Expected: docker refuses: "the working directory 'tmp' is invalid, it needs to be an absolute path".
- Reproduced: yes, 2 times.

### OR-B40-25 (P3) `-e=A=1` is read as the value `=A=1`
- Command: `$U $P run --rm -e=A=1 $A printenv A`
- Observed: `-e takes NAME=VALUE or NAME, with a name, not "=A=1"`, exit 125. `--env=A=2` and `-eA=3` work.
- Expected: docker (pflag) reads `-e=A=1` as `A=1`.
- Reproduced: yes, 2 times.

### OR-B40-26 (P3) `run -t` / `exec -t` without `-i` writes `\r\r\n`
- Command: in a pty: `$U $P exec -t d1 echo hi`; `$U $P run --rm -t $A echo hi`
- Observed: the bytes `hi\r\r\n`. With `-it` the bytes are `hi\r\n`.
- Expected: `hi\r\n` (the outer terminal in raw mode, as with -it).
- Reproduced: yes, 2 times.

### OR-B40-27 (P3) The release binary names beta.39 and a dirty tree
- Command: `$P --version; $P version --verbose`
- Observed: `podvm 0.2.0-beta.39`, `commit: 41025181...-dirty`. Messages name the same version (for example the incus reason "a podvm before 0.2.0-beta.39").
- Expected: the beta 40 artefact names its own version and a clean commit.
- Reproduced: yes, 2 times.

### OR-B40-28 (P3) Small wrong or raw messages
- Command and observed:
  - `$U $P verify` lists `docker.io/local/alp:v2`; `images` lists the same image as `local/alp`.
  - `$U PODVM_STORE=/var/tmp/beta40/op/afile $P images` (a regular file): `podvm: /var/tmp/beta40/op/afile: File exists (os error 17)`; true cause: not a directory.
  - `$R $P run -d --podvm-tier=incus --name i1 $A true` (i1 exists): the raw incus line `POST /1.0/instances: incus refused the call (409, ...): This "instances" entry already exists`, exit 125; the native verbs say "the name ... is already used by a container".
  - `$U $P exec cr2 ...` into a created container: the note has runs of spaces inside the sentence (`its rootfs              is there`).
  - `exec --help` says that each `--podvm-tier` value but machine and emulate "is a flag error", but `exec --podvm-tier=incus NAME` works.
- Expected: one true, clean line each; the help that matches the verb.
- Reproduced: yes, 2 times each.

## Checked and fine

- Exit codes of a payload: 0, 7, 33 (incus), 127 not found, 126 not executable, 137 self-SIGKILL; flag errors 125; after-parse refusals 1 (most verbs); `--timeout 2` kills with 137 and a true line.
- Signal forwarding to a foreground run: SIGTERM, SIGINT, SIGHUP, SIGUSR1, SIGQUIT forwarded, named on stderr, 128+N returned or the trap's own code (42); the same on the incus tier (SIGTERM, SIGINT); Ctrl-C through `-it` gives 130; SIGKILL of the foreground podvm leaves no payload; SIGKILL of a detached launcher kills the payload and the record reads `dead`, `wait` refuses.
- run -d, exec (exit codes, -e, -w, -u, -i stdin, SIGTERM forwarding, exec killed with 137 when the container stops), exec into an exited container refused with 1, stop (TERM then KILL after -t), kill -s by name and number, wait (one and many), restart, start of created and exited containers, rm, rm -f, ps -a/-q/--no-trunc/--filter name=/label=, labels, container prune (deferred tree removal reaches 0 B), `ps` titles (`podvm run ...`, `podvm-init`, `podvm-run`, `podvm-exec`).
- logs, --tail, --limit-bytes, -f until the end, 40 MB log, `--report` JSON (no secret value in it).
- -e / --env-file / --env-file0 order and quoting, missing and unreadable files refused with 125, non-UTF-8 env file refused by line.
- -v rw and :ro (namespace and userland rungs), missing host path refused, named volume refused, --read-only (EROFS), --tmpfs, --hostname, --add-host, --security-opt no-new-privileges, --ulimit, --entrypoint (and empty), -u names from the image, huge arguments (15 x 120 KB) and environment.
- Concurrency: 10 parallel runs, 6 parallel `run -d --name same` (one wins, five refused), 4 parallel pulls of one image, `image prune -a` during runs (skips held images, runs finish).
- pull (HTTPS, errors from the mirror and the proxy named), --platform arm64/s390x pulls, a foreign-platform run refused by name, `--pull never/bogus`, tag, images (-q, --digests, --no-trunc, --filter), save (-o and stdout, identical bytes), load (file and pipe; truncated and junk refused), import (plain tar runs; gzip refused by name), rmi of a referenced image refused, image prune, verify, system df, system info, ExitCodes, probe (--json, --rows, --strict, --cached), doctor (store_exec, --strict), version, man (--json, --roff, --llms, a verb), each verb's -h/--help exits 0, install-names and the docker/podman names.
- cp: a file and a tree both ways, from an image, into an image refused, symlinks kept, special targets and `-` refused by name, no escape of the container root.
- Incus tier: first run imports the image (`imported now`), later runs reuse it (`from incus`), the store keeps no extra tree after the import of python:3.13-slim and golang:1.24-alpine, a native run of the same image works after it; `--podvm-init=image` with systemd: the command ran after systemd was `running`, and with a degraded image (a failing unit plus a 25 s oneshot) after the slow unit ended, with one line naming the failed unit; PODVM_INIT_BOOT_TIMEOUT bound (8 s) refuses with a full account and leaves the instance for a look; bad values (0, 3601, abc) exit 125; `inspect` CpuSeconds matched `time` (8.23 s vs 8.51 s user; 15.33 s vs 12.04+3.27 s with children), PeakMemoryBytes 162.9 MB for a 150 MiB buffer, OomKills 1 at `--memory 64m` with the shell alive (`child=137`, `shell alive`), ExitCode after the end, `--verbose` reasons for a forced stop and for an image init; --pids-limit enforced; -e, -w, stdin, exec, exec -t refused by name, logs, logs -f, start, stop, rm -f, run -d --rm removes the instance; `-u` refused by name.
- syscall tier (static busybox, run -d and exec), emulate tier refused by name, PODVM_USERNS=0 userland rung with -v, :ro and --read-only, PODVM_MODE memfd and fuse refused with true reasons, unsupported docker verbs (top, stats, pause, rename, diff, commit, build, export) refused by name with 125.
- No qemu, incusd, lxc or dnsmasq process of this pass remains; no container record or incus instance remains.
