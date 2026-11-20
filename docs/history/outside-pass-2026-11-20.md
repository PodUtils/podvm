# Outside pass: podvm candidate in /var/tmp/beta37 (reports itself as 0.2.0-beta.36, commit 2fc28af8)

Host: 4 CPUs, no /dev/kvm (TCG), cgroup v1, Landlock ABI 7. Each podvm command ran as uid 1000:
`setpriv --reuid 1000 --regid 1000 --clear-groups env HOME=/var/tmp/op6 PODVM_STORE=/var/tmp/op6/store TMPDIR=/var/tmp/op6/tmp PATH=/usr/local/bin:/usr/bin:/bin`
(below: `U=` is that prefix, `P=/var/tmp/beta37/bin/podvm`, `A=mirror.gcr.io/library/alpine:3.20`, `NU=/var/tmp/op6/nouserns`).
Default rung: namespace. With `$NU`: userland (the loader family, the native-static family, and the syscall tier with `--podvm-tier=syscall`).

## Findings

### OQ-01 (P0) The userland rung: a payload writes to a `:ro` volume through the volume's host path
- Command: `$U $NU $P run --rm --podvm-volume-base /var/tmp/op6/vb -v /var/tmp/op6/vb/real:/cfg:ro $A sh -c 'echo TAMPERED > /cfg/f; echo guest=$?; echo TAMPERED > /var/tmp/op6/vb/real/f; echo host=$?; chmod 600 /var/tmp/op6/vb/real/f; echo chmod=$?'`
- Observed: `can't create /cfg/f: Read-only file system`, `guest=1`, `host=0`, `chmod=0`, exit 0. On the host, `vb/real/f` holds `TAMPERED` and has mode 0600.
- The payload can find the host path: its environment has `PODVM_MAPS=/:<rootfs>,/data:/var/tmp/op6/vol/d,...` and `PODVM_RO=ro:/data`.
- The same occurs with no `--podvm-volume-base`; with `run -d` and then `exec` (wrote `T4`); and with a static payload (busybox image, the native-static family: wrote `S2`). The host path of a rw volume is also writable. Other host paths (`/var/tmp/op6/envf`, `..` forms) stay anchored in the rootfs.
- Not affected: the namespace rung (`nonexistent directory`), the syscall tier (`nonexistent directory`), and `--podvm-confine read=/` (`Permission denied`).
- Expected: the manual, `-v`: ":ro refuses a write"; the userland banner: "host pass-through happens only under --unsafe-host-paths". The userland banner also says "this mode does NOT provide: isolation of any kind". The bypass does not need a raw syscall: a plain shell redirect does it.
- Reproduced: yes, 4 times (loader family twice, exec into -d, native-static).

### OQ-02 (P1) Machine tier: a signal to a foreground run does not go to the payload
- Command: `$U $P run --rm --podvm-tier=machine $A sh -c "trap 'echo guest-got-TERM; exit 4' TERM; echo ready; while :; do sleep 0.2; done"`. After `ready`, `kill -TERM <podvm pid>`.
- Observed: podvm itself ends by the signal (shell: `Terminated`, rc 143). The guest trap does not run, and nothing goes to stderr. With INT: rc 130. With HUP: rc 129. Same result each time. The emulator was gone afterwards (good).
- Expected: the manual, `run`: "A SIGHUP, SIGINT, ... SIGTERM ... to a foreground run is forwarded to the payload and named on stderr with the payload's exit". The namespace rung does this correctly (`forwarded SIGTERM to the payload`, trap output, rc 9).
- Reproduced: yes, 3 signals (TERM, INT, HUP), and once more with TERM through `timeout`.

### OQ-03 (P1) Machine tier: most verbs cannot find a detached container
- Command: `$U $P run -d --name md2 --podvm-tier=machine $A sleep 600`
- Observed: the run prints `md2` (the name, not an id). Then:
  - `ps` and `ps -q` list it (CONTAINER ID `md2`, COMMAND `(linux guest)`).
  - `inspect md2` and `inspect --format '{{.State.Status}}' md2` give `podvm inspect: no such image: md2` and `[]`, rc 1.
  - `ps --format '{{.ID}} {{.Names}} {{.Status}}'` and `ps --filter name=md2` leave it out.
  - `kill md2` and `wait md2` give `no such container: md2. podvm ps -a lists what there is`, rc 1, while `ps -a` lists it.
  - `stop`, `rm -f`, `logs` and `exec` work.
- Expected: the manual: "inspect: images by reference and containers by name"; `kill` "signals the payload"; `wait` "blocks on the launcher". docker prints a container id for `run -d`.
- Reproduced: yes, 2 times (md, md2).

### OQ-04 (P1) `exec -t` gives no terminal, with no refusal
- Command: `$U $P run -d --name tt $A sleep 300`, then `$U $P exec -t tt tty`. The same in a real pty (`script -qec ...`), with `-t` and with `-it`.
- Observed: `not a tty`, rc 1, each time. `run -t` on the same host gives `/dev/pts/0`.
- Expected: the manual, `exec -t`: "REFUSED BY NAME where /dev/ptmx is unusable, rather than silently degraded". /dev/ptmx is usable here, so podvm must give a pty or refuse by name. docker gives a pty.
- Reproduced: yes, 3 times.

### OQ-05 (P2) `run -it`: a Ctrl-C of the terminal does not stop the payload's foreground child
- Command: in a pty, `$U $P run --rm -it $A sh -c 'echo ready; sleep 6; echo end-not-interrupted'`. Send `^C` (0x03) 4 s after the start.
- Observed: the run ends after about 6.5 s (the `sleep 6` completes), with `the payload died on SIGINT: exit 130`. With `-t` and no `-i`, the same `^C` ends the run at once (about 4.5 s). With a trap in `sh`, `got-INT` prints only after the sleep ends.
- Expected: docker: `^C` on an `-it` pty sends SIGINT to the foreground process group, so `sleep` ends at once. The manual: "a SIGINT or SIGQUIT of the terminal reaches the payload once".
- Reproduced: yes, 3 times.

### OQ-06 (P2) `inspect --format` gives list and map fields as one flat string
- Command: `$U $P inspect --format '{{json .Config.Env}}|{{.Config.Cmd}}' mirror.gcr.io/library/python:3.13-slim`
- Observed: `"PATH=... LANG=C.UTF-8 GPG_KEY=... PYTHON_VERSION=3.13.18 ..."` (one JSON string, space-joined) and `python3`. Plain `inspect` gives a correct JSON array for Env.
- For a container (`run -d --label team=ci`):
  - `{{json .Labels}}` gives `"team=ci"`.
  - The container JSON has `Labels`, `ExitCode` and `Pid` as strings, and `Config` as null.
- Expected: docker: `{{json .Config.Env}}` gives `["PATH=...","LANG=..."]`, `{{.Config.Cmd}}` gives `[python3]`, and `.Labels` is a map. The manual: "--format reads the same fields".
- Reproduced: yes (alpine and python images; two containers).

### OQ-07 (P2) The namespace rung: the banner and `--strict` say no /proc and no PID isolation, and the payload has both
- Command: `$U $P run --rm --verbose $A true`, and `$U $P run --rm --strict $A true`.
- Observed: the banner says "this mode does NOT provide: user, pid or network isolation, or a /proc filesystem" and "no /proc is mounted in the payload root". The same banner then says "podvm opened a user, a PID and a UTS namespace for this run ... and the payload has its own /proc". `--strict` warns "/proc is not mounted in the root". In the payload, `mount` lists `proc on /proc type proc`, and `ps` shows a private PID table.
- `inspect --format '{{.Exec.Mode}} | {{.Exec.Shares}}'` gives `fresh-chroot | filesystem`, but `exec c1 ps` lists the container's processes (it shares the PID namespace).
- Expected: one true statement of what the rung gives.
- Reproduced: yes, 2 times.

### OQ-08 (P2) The namespace rung: podvm's own process is visible in the payload, with its whole argv
- Command: `$U $P run -d --name pv -e SECRET=hunter2 $A sleep 30`, then `$U $P exec pv ps -o pid,args`
- Observed: `1 /var/tmp/beta37/bin/podvm run -d --name pv -e SECRET=hunter2 ...` and `2 sleep 30`. The payload is not PID 1. In foreground runs, the payload is PID ~104.
- The payload environment also shows host paths: `PODVM_DISPATCH_SELF`, `PODVM_MEMO_PATH`, `LD_PRELOAD`.
- The payload inherits a read-write fd 5 on `store/locks/<digest>.lock` and fd 17 on `store/staging/memo-*.tmp`.
- Expected: docker: the payload is PID 1, and the CLI argv is not visible in the container. The manual warns against `-e` for secrets because of the record, not because of this.
- Reproduced: yes (run and run -d).

### OQ-09 (P2) docker differences that the manual does not name
- `run -t`: TERM is not set in the payload (docker sets `TERM=xterm`). The host name is the host's (`vm`), and `HOSTNAME` is not set (docker uses the short container id).
- `exec` into an EXITED container runs (`exec u1 echo into-exited` gives rc 0). docker refuses: "container ... is not running".
- `logs --tail all` gives `--tail takes a non-negative line count, not "all"`, rc 125. docker's default is `all`.
- `start -a NAME`: `unknown option "-a". It has no row in the parity table`, rc 125.
- `cp NAME:PATH -` (a tar to stdout): `unknown option "-"`, rc 125.
- `-v H:G:rw,z` is refused: "the option "z" has nothing to act on: a volume here is a path rewrite, not a mount". On the namespace rung that is not true: the manual says "A bind mount on the namespace rung".
- Reproduced: yes, each one at least once. TERM was checked 2 times.

### OQ-10 (P2) Exit codes are not the documented ones
- `--env-file` with a 1 MB value: `podvm run: "sh": the payload could not be started: execve ... failed with E2BIG (7)`, rc **127**. The manual says 126 is "the command found and not invocable" and 127 is "not found". The result is the same on the userland rung.
- The GLOBAL text says "a refusal after the flags parsed exits 1". Some refusals after the parse use 1 and some use 125:
  - 125: `rmi` of a referenced image, `load` with a digest mismatch, `load -i /etc/shadow`, `import` of a .gz, and `cp CONTAINER:/../../etc/shadow`.
  - 1: `cp FILE IMAGE:/x`.
- Reproduced: E2BIG on 2 rungs. The others were seen once each.

### OQ-11 (P2) `--podvm-volume-base` on the loader family: a link under DIR does not give ELOOP
- Command: `$U $NU $P run --rm --podvm-volume-base /var/tmp/op6/vb -v /var/tmp/op6/vb/real:/d $A sh -c 'cat /d/sub/hostname; sleep 3; cat /d/sub/hostname'`. During the sleep, the host replaces `vb/real/sub` with a symlink to `/etc`.
- Observed: the second `cat` succeeds and prints `localhost`: the link resolves into the image's /etc, not the host's. A link that the payload made (`ln -s /etc /d/esc; head -1 /d/esc/os-release`) also resolves into the image's root.
- Expected: the manual: "A link under DIR is a refusal (exit 125 at the start, ELOOP for the call after it)". No host data leaked.
- Reproduced: yes, 2 times.

### OQ-12 (P3) A SIGKILL of podvm leaves a memo file in the store
- Command: `$U $P run --name fk $A sh -c 'sleep 77'`, then `kill -9` of the podvm process. On the userland rung: the same command with `$NU`.
- Observed: the payload ends (good), and `ps -a` shows `Dead (exited while the launcher was not watching)`. `store/staging/memo-<pid>-<n>.tmp` stays, also after `rm -f` and `container prune`.
- Reproduced: yes, 2 times (namespace and userland).

### OQ-13 (P3) The manual and messages
- The candidate in `beta37` reports `0.2.0-beta.36` (`version`, the manual header).
- The manual's `container prune` section says "No usage to show: `podvm container prune` is not a command this binary takes". The verb works, and `container prune --help` prints its usage.
- The manual says `--read-only` is refused outside the loader family. On the namespace rung it works (EROFS).
- `--timeout 99999999999999999999` gives "takes whole seconds over 0". The value is over 0; it is too large.
- `--report tmp/nodir/r.json`: "the directory tmp/nodir takes no file of the caller". The directory does not exist.
- The lines for forwarded signals and for a payload signal exit (`forwarded SIGINT to the payload (pid 104)`) have no `podvm` prefix.
- `--env-file` with non-UTF-8 bytes: "stream did not contain valid UTF-8" names no line.

## What worked
- pull (alpine, debian, busybox, ubuntu, python, node, jrei/systemd-debian through mirror.gcr.io); concurrent pulls of one image; images (--filter, --format, -q); tag/rmi; verify; system df; image prune; container prune.
- run on the namespace and userland rungs:
  - exit codes 0, 42, 126 and 127, and the payload's code;
  - `--rm`, `-d`, `--name` (a duplicate, also when two runs race, is refused), `-e` (repeat, `NAME` pass-through), `--env-file` (order, quotes, comments, size ceiling), `-w`, `-u` (faked as documented), `--entrypoint` (`""` too), `--hostname`, `--ulimit`, `--timeout`, `--label`, `--report` (also on a refusal; no env value in it), `--strict`.
  - `-v` with a directory, a file and `:ro` on the guest path, on both rungs and on the machine tier (9p); clear refusals for a missing host path, a named or anonymous volume, a relative guest path, and `/`; `--tmpfs`, `--read-only` (both refuse `-d` or work as documented).
- Signals SIGINT/TERM/HUP/USR1 to a foreground run on the namespace rung: forwarded, trap runs, 128+N without a trap.
- `-i` and piped stdin; `-t` and `-it` give a pty and the window size.
- exec into a `-d` container (namespace, userland, syscall tier, machine); ps, logs (-f, --tail N), stop (trap runs, exit 3), start, restart, kill (137), wait, rm and rm -f, create/start.
- inspect `.State.Status/.Running/.ExitCode/.Pid`; image `.Config.*`.
- cp file and `-r` in both directions; `..` and symlink escapes out of the rootfs are refused; a copy into an image is refused by name.
- save (-o and stdout, byte-identical), load (a corrupt blob is refused), import (plain tar; .gz refused by name).
- `--podvm-tier=syscall`: fixed uname, the /tmp cap of 256 MiB and `--tmpfs /tmp:size=10m` give ENOSPC at the cap; -d and exec.
- `--podvm-confine` on both rungs:
  - read/write/exec rules, /etc denied, an empty spec denies all, read=/ also covers a -v, and -d and exec keep the rules;
  - the interpreter of a #! script out of exec= is denied;
  - on userland, `ld.so PROGRAM` and chmod out of write= are denied (on the chroot rung both pass, as the manual says);
  - libc `syscall(openat)` is still confined;
  - bad specs are refused with clear text.
- `--podvm-volume-base`: a link under DIR is refused at the start with exit 125 on both rungs.
- Machine tier (TCG): run in about 5 s, exit code, -e, -v rw and :ro; `--podvm-init=image` with systemd as PID 1 (16 s cold, 2 s from the saved state); a bad PODVM_MACHINE_STATE is refused; `--podvm-confine`, `--read-only`, `-u`, `--hostname` and `--podvm-volume-base` are refused by name; `--podvm-mem 999T` fails with the emulator's reason.
- No emulator stayed after any guest run, after a SIGKILL of podvm, or after stop/rm -f.
- A SIGKILL of podvm mid-run on both rungs ends the payload, and ps shows Dead.
- 4 concurrent runs of one image, and 3 concurrent first runs of ubuntu: correct output.
- PODVM_USERNS=0, PODVM_MODE (bogus value, fuse/memfd on userland, -d), PODVM_DEBUG, probe, probe --strict (exit 1 on userland), doctor, system info .Rung/.EnteredRung.
