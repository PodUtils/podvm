# Handoff

The state on 2026-11-21, 08:30 (UTC), at the end of a session. `main` is
at the commit that changes this file. `0.2.0-beta.37` is the last release
(2026-11-21, release run 44846783317 on `c92ac65`, podcrates `v0.79.0`).
After it, `main` carries podcrates `v0.80.0` (T-2050), the CLI fixes of
the sixth outside pass (T-2060 to T-2064) and the memory rule of T-2047.
Two branches of work wait on origin (below, "WIP branches"). A fresh
session with no memory reads this file first, then `AGENTS.md`,
`TODO/PROGRESS.md` and `docs/decisions.md`. Remove this file when each
step below is done.

## The rulings of this day

* "podrunner first" and "Coordinator" (PodUtils/podcrates#225, COORD-134;
  `docs/decisions.md`): after a red run of podvm's CI, each ask of
  podrunner comes first, then the asks of githome and cfb, then podvm's
  own order. The session of podcrates coordinates; COORD issues are the
  one channel (PodUtils/podcrates#227, COORD-136). Each session writes its
  state to the coordinator at its start: podvm's is COORD-184 (#571);
  amend it.
* "Schema ids": an added optional field keeps `run-report.v1`; a changed
  or removed field needs `v2` and a COORD issue `(to all)`.

## Where things are

* podcrates is taken at `v0.80.0` (`a5e4936` to `4dc5744`, COORD-223,
  #635; CI green in #637). A podvm session can not write on
  PodUtils/podcrates issues (its GitHub access reaches podvm only): each
  take is a COORD issue of this repository.
* Each release publishes `ghcr.io/podutils/podvm:TAG` by the job `image`
  of `release.yml` (`image.yml`, T-1992); `image.yml` can be dispatched
  alone for a tag (`push: false` is the dry run).
* The issue-sync bot of a push reopens or closes an issue as the record of
  THAT commit says: a sync of an older commit can reopen an issue that a
  newer record closed; the sync of the newer commit closes it again.
* The pkgforge mirrors answer for podvm and podcrates reads, with some
  403s; retry. Release assets of another repository: the GitHub MCP tool.
* The cloud host runs incus (route A): `PODVM_434_MIRROR=1 sh
  experiments/434-incus-tier.sh` passes (public.ecr.aws answers 429 here,
  so use the mirror switch). It has no `/dev/kvm` and no delegated
  cgroup2 subtree: a machine guest runs under TCG
  (`PODVM_MACHINE_ACCEL=tcg`), and the cgroup clauses go to `kvm.yml`
  (job `cgroup`, experiment 459).
* `docs/maintenance.md`, "Releases", is the gate of each beta (T-1949,
  done): gate, `workloads.yml`, the uid-1000 tests, an outside pass with
  the binary and `podvm man` only, `arm.yml`, the nightly trial, and an
  entry "Known gaps" in the notes.
* The uid-1000 integration tests: build the test binaries of `podvm` and
  `podvm-supervise` (`cargo test --release --no-run --message-format=json`)
  and run each with `setpriv --reuid 1000 --regid 1000 --clear-groups`, with
  `HOME`, `TMPDIR` in a new directory of mode 777 under `/var/tmp` and
  `CARGO_MANIFEST_DIR=$PWD/crates/podvm`. Build the interposer objects
  first (`scripts/build-interpose.sh`).

## Done in this session

* The red gate of `7634814` (a test left scratch stores in `/tmp`).
* podrunner: PodUtils/podrunner#216 (T-1934, a pty in each unit),
  #277 (T-1945, `--podvm-volume-base`), #168 and #110 (done before);
  answered in COORD-185 (#572), released in beta.32.
* PodUtils/podrunner#296 (COORD-187, #583): ask 3 (T-1966,
  `--podvm-engine` on the incus tier), ask 4 (T-1967, `--pids-limit` in a
  machine guest) done; ask 6 (T-1969, `oom_kills`) partial: native run
  reports proven in `kvm.yml` run 44285612361, `OomKills` in `inspect` of
  an instance; ask 2 (T-1965) done; ask 1 (T-1964) partial: a pool with a
  quota is not measured (route A mounts none); ask 5 (T-1968) open; ask 7
  was done (T-1890).
* PodUtils/podrunner#300 (COORD-189, #585): `--env-file0` (T-1971) done
  for the next beta; `--env-file` holds on the machine and incus tiers;
  ask 3, `--podvm-volume-base` off the namespace rung, is T-1972 (P1,
  partial): the syscall tier holds it (`a8e856f`); the loader family of
  the userland rung remains.
* The outside pass (T-1970): the three P1 faults and OQ-05 fixed; the
  others are in "Known gaps" of beta.32. COORD-188 (#584) asks pod-enter
  for a controlling terminal (the `-it` relay turns `^C` into SIGINT until
  then).
* T-1955 partial: the calendar of `podvm-image` is `pod_core::clock`.
* The donor study (the zip of the owner): issues #573 to #581, entries
  T-1956 to T-1963. #573 names an owner decision: a seccomp
  user-notification rung, which T-0606 refused once.
* PodUtils/podrunner#301 (COORD-190, #586): the syscall tier maps the
  path of a Unix socket for `connect` and `bind` (T-1973, done).
* The second outside pass of 2026-11-18 (T-1974, partial): OR-01 to OR-06
  fixed (exec opens volumes under the base, Ctrl-C of `-t` and `-it`,
  `exec` in a bound `-w`, `-v H:/tmp` of the syscall tier, links across
  volumes: T-1975); OR-07, OR-08 and OR-10 to OR-17 are in "Known gaps"
  of beta.33.
* C24 (`docs/decisions.md`): the size ceiling of the binary is 9 000 000
  bytes.
* T-1976 (partial): the gate of beta.33 died of SIGSEGV in
  `exec_loads_the_next_program_in_the_root` on CI only. The SIGSYS
  handler runs on the payload's alternate signal stack (8 KiB from Rust's
  std on a CPU without AMX; the cloud host has AMX, so the test passed
  there). `exec::execveat` now runs on a stack of its own; the remainder
  is a stack of its own for each call of the handler.
* Released `0.2.0-beta.33`; #583, #585 and #586 name the tag for
  podrunner; #587 (COORD-191) is closed (podcrates lists the take).
* After beta.33: podrunner took it (PodUtils/podrunner#296, #300, #301
  closed). Its CI found PodUtils/podrunner#303: T-1977 (glibc's
  `AT_FDCWD` in the low 32 bits; answer COORD-192, #588). T-1969: a
  machine guest gives `oom_kills`. T-1976 done: each call of the
  syscall-tier handler runs on a stack of its own (the gate died of
  SIGSEGV on CI twice; the cloud host's CPU has AMX, so its tests passed).
* The gate of beta.34: the uid-1000 tests found a regression of OR-04
  (`exec` after `--tmpfs`, fixed in T-1974); the third outside pass (T-1980)
  found two P1 faults, fixed: T-1978 (`posix_spawn` of a link in a volume
  on the syscall tier) and T-1979 (a link in a volume on the loader
  family). T-1981 (P1, blocked): Fedora 42's `PT_INTERP` lies past the
  64 KiB that pod-enter reads; COORD-193 (#589) asks podcrates.
* Released `0.2.0-beta.34`; #588 and #583 name the tag.
* After beta.34: podrunner closed #303. #304 (COORD-194, #590): T-1982,
  `run -d` with `--podvm-confine` on the tier started nothing. podcrates
  `v0.72.0` taken (`b3ff9c3`, COORD-195): T-1981, Fedora 42's `PT_INTERP`
  past the head. The fourth outside pass (T-1989) found six P1 faults, each
  in beta.34 already: T-1983 (`exec=` on the tier), T-1986 (`/root` of
  Fedora), T-1987 (AlmaLinux's `ls -l` in the interposer), T-1988 (`stat`
  of a volume on the tier), all fixed; T-1984 and T-1985 (Landlock checks
  no program through `ld.so`, no `chmod` or times) named in the help by
  call C25 and set to P2. COORD-196 (#592) told podrunner; podrunner's
  kind does not rest on them (PodUtils/podrunner#307).
* Released `0.2.0-beta.35`; #590 and #592 name the tag.
* #308 (COORD-197, #593): "errno 11" on the tier is an `openat2` that a
  rename or a mount raced; `b00c07c` tries again (T-1990, partial: not
  reproduced here, podrunner's CI measures it with the next beta). The
  hang of `cargo doc` of ripgrep (T-1991, P1) does not reproduce here;
  #593 asks podrunner for its processes. podrunner keeps the tier off
  until both are fixed.

* This session (2026-11-19): experiment 434 clause 19 passes, T-1993
  done (systemd as PID 1 on the incus tier; the tarball reads files of mode
  0000; the wait for the bus of systemd). T-1992 done (beta.35 on ghcr.io,
  #595 closed). podcrates `v0.73.0` and `v0.74.0` taken. podrunner's
  #315 (COORD-105): `tsc` of TypeScript 7 hung on the userland rung at
  Go's SIGURG of preemption; T-1995 holds a signal that stops the SIGSYS
  handler until the call ends (COORD-202, #598). podrunner's #316
  (COORD-106, the hosted snapshot): T-1996 to T-1999 and T-1994 amended
  (COORD-203, #599). The fifth outside pass (T-2001): one P1, T-2000
  (`apt-get install -u 0` on the userland rung: the memo reopens from
  `PODVM_MEMO_PATH`), fixed; the P2 and P3 findings are in T-2001 and in
  "Known gaps" of beta.36.
* Released `0.2.0-beta.36`; #593, #596 and #598 name the tag.
* After beta.36: T-1996 partial (231,072 ids, `/dev/fuse`, an empty
  machine-id with `--podvm-init=image`; clause 20 of experiment 434
  passes; rootless podman is podrunner's measure, #599). T-1997 partial
  (the tarball goes to incusd through a pipe; experiment 434 passes; the
  peak of the disk use of a large import is not measured yet). podcrates
  `v0.75.0` taken (`f3beec1`, COORD-204, #600, closed). The gate on
  `5971b00` passed.

* The table of the owner for systemd as PID 1 (2026-11-19, two research
  bundles): T-2002 (partial). `run --podvm-init=image` of a foreground run
  boots the init of the image as PID 1 of a PID namespace: on the
  namespace rung with the subordinate range and a delegated cgroup2
  subtree, else in a guest of the machine tier, also from the userland
  rung (call C26). `sh experiments/465-init-image.sh` passes on the cloud
  host as uid 1000 (clause 1 with a cgroup2 directory that root made, as
  the host delegates none). T-2004 (partial): podvm asks the user manager
  of systemd for the delegated scope; `sh experiments/466-init-image-scope.sh`
  passes (the host of that measure is a guest with systemd, 5 minutes
  under TCG). T-2003 (RUNPATH on the userland rung: an ask for pod-enter)
  and T-2005 (a cgroup2 stand-in, User-Mode Linux) are open. T-2006 (done): a SIGKILL of podvm ends each process of a
  foreground run in the user namespace.

* The owner's direction of 2026-11-19: a near-native route before TCG.
  T-2007 (partial): `--podvm-init=image` on the runsc tier (gVisor): the
  cloud host as it is boots systemd as PID 1 in 7 s to 10 s as uid 1000;
  the route of a run with no tier is the namespace rung, then runsc, then
  the machine tier (call C26). The pinned gVisor of experiment 436 is not
  on PATH: set `PODVM_RUNSC`. Open: T-2009 (podvm fetches runsc), T-2005
  (User-Mode Linux in its seccomp mode for the userland rung: measured
  by `sh experiments/467-uml-seccomp.sh`, 15 s for a run with `ptrace`
  and each namespace denied; ask podcrates before a kernel that podvm
  carries), T-2008 (a saved state of the guest).

* COORD-206 to COORD-209 (#602 to #605) are open: to podcrates the UML
  kernel (T-2005), RUNPATH in pod-enter (T-2003) and the pinned fetch of
  runsc (T-2009); to podrunner the routes of `--podvm-init=image`
  (T-2002, T-2007). The owner's round 2 (2026-11-20): make each route as
  fast as possible, with a measured baseline first.
* podcrates answered (2026-11-20): T-2005 waits for `podcrates@T-975`
  (`pod-microvm`: the UML kernel and its launcher, PodUtils/podcrates#242),
  T-2003 for `podcrates@T-976` (RUNPATH in pod-enter) and T-2009 for
  `podcrates@T-977` (the `fetch` module of `pod-release`),
  PodUtils/podcrates#243. podvm carries no kernel and writes no fetch.

* T-2010 (partial): `sh experiments/468-init-bench.sh` gives the baseline
  of each route (the run, the first output, the memory). Done: the wait
  with `--wait`, the stop at once of a run that keeps nothing,
  `PODVM_RUNSC_ARGS`. T-2008 is measured by hand: a saved guest runs again
  2 s after the start of QEMU under TCG, against 16 s for a boot; its
  design is in the entry, and no code is written.
* The session of 2026-11-19 and 20: T-1993 to T-1999 for the hosted
  snapshot (#599), T-1995 (Go on the userland rung; podrunner closed #315),
  T-2000 (the memo of apt), T-2001 (OP-02, OP-03), beta.36 released. Two
  agent rounds on the owner's systemd table: T-2002 to T-2010 (the
  namespace rung with a delegated cgroup: 5.3 s; runsc: 7.4 s; TCG: 28.8 s;
  UML seccomp measured 14.7 s). podcrates took the UML kernel, RUNPATH and
  the runsc fetch (#602 to #604; podcrates#242, #243). A housekeeping pass
  fixed the docs and closed #428.

## This session (2026-11-20, from 01:30 UTC)

* CI: a push of this session starts no workflow. Each run is a
  `[gh action] requested` issue (AGENTS.md, section 9): #607, #610, #614,
  #615 are closed (gate, `arm.yml`, secrets, issue-sync green on
  `369f2d6`); #617 (gate, arm, secrets, issue-sync and `kvm.yml` job `init`
  on `ec55ca9`) is open.
* podcrates `v0.78.1` (COORD-210, #606) and `v0.79.0` (COORD-216, #616)
  taken. `v0.79.0` gave the signals of pod-enter (T-2001 OP-05) and RUNPATH
  (T-2003, done).
* podrunner: #325 (T-2011, COORD-212 #609: the Entrypoint `/sbin/init` of
  the snapshot ran as a prefix of the command), #326 (T-2012, COORD-214
  #612: `exec` into a namespace-rung container joins its namespaces),
  ask 3 of #300 (T-1972 done, #585 amended), ask 6 of #296 (T-1969 done,
  #583 amended), #605 amended (T-2008, T-2010).
* Done: T-1969, T-1972, T-1984, T-1985, T-2001, T-2003, T-2011, T-2012,
  T-2013. Partial: T-2008 (the saved guest state: first output 1.5 s
  against 19 s under TCG; call C27, owner question Q10), T-2010, T-2002
  (the groups of `--user`). New open: T-2014, T-2015, T-2016 (QEMU 8.2
  SIGSEGV with four vCPUs under TCG).
* To podcrates: COORD-213 (#611, one beneath-open: `pod-beneath` is on
  podcrates' `main`), COORD-215 (#613, the loader rule and a write table:
  on podcrates' `main`), COORD-217 (#618, a warm save with no UEFI `Plan`;
  a pinned QEMU 10.0.4).
* `podvm man` has an ENVIRONMENT section from one table
  (`crates/podvm/src/environ.rs`); a test fails on a `PODVM_*` name with
  no row.

## This session (2026-11-20, from 18:00 UTC, to 2026-11-21)

* Released `0.2.0-beta.37` (#634). Its gate: #627 (the nightly trial failed
  on the licence text of asn1-rs-impl 0.2.0: `vendor/licence-texts/`,
  `3924ccd`), #628, #632 (all green on `7324fe0`). COORD-224 (#636) names
  the tag to podrunner.
* T-2048 (partial): the save of a machine guest hung with no line. It
  opened a new QMP connection while the old one was open; QEMU queues one
  client (listen backlog 1), and the third `connect()` waited with no
  bound, past SIGTERM. Fixed in `867f79e`; KVM proof in `kvm.yml` 44808044333
  (a run from the saved state 3.4 s against 12.2 s). Experiment 468 now
  fails on a row with no bench line (`7324fe0`): before, its hung rows
  passed the step. COORD-222 (#633) asks podcrates for a bounded
  `Monitor::connect`.
* T-2046 (done, podrunner#333): `logs` streams through one buffer of 64 KiB
  and takes `--limit-bytes N` (COORD-220, #630). The two test modules under
  `crates/podvm/src/{incus,lifecycle}/logs/` were hidden by an unanchored
  `logs/` of `.gitignore` (now `/logs/`). COORD-221 (#631) asks podcrates for
  a streamed log read and a ranged incus file read.
* T-2047 (done): experiment 470 (`kvm.yml` job `mem`) measured the memory
  of a guest for an initrd of 263 to 2055 MiB under KVM (run 44854125431):
  about 3.25 times the initrd at 2 GiB. `mem.rs` takes 13/4 of the initrd
  and 512 MiB. podrunner closed #334 (COORD-219, #629).
* T-2060 to T-2064 (done, the sixth outside pass, OQ-06, 09, 10, 12, 13):
  typed `inspect` JSON, exit codes (refusals after the parse are 1, E2BIG
  126), `exec` into an exited container refused, `TERM=xterm` with `-t`,
  the memo flock, texts. Gate 44862250138 green after three integration
  tests took the new behaviour. COORD-226 (#640) tells podrunner;
  COORD-225 (#639) asks podcrates for E2BIG in pod-enter.
* T-2065 (blocked): podrunner#339, a PAX record `SCHILY.acl.access` with a
  newline refuses a layer in `pod-extract`; COORD-227 (#643) to podcrates
  (in work for its next tag), COORD-228 (#644) to podrunner.
* Experiment 465 clause 11: `-t` sets `TERM=xterm`, and the image's bash
  writes the escapes of bracketed paste; the check now strips them (the
  last commit of the session). Clause 9 fails on this cloud host only
  because its QEMU 8.2.2 crashes under TCG with four vCPUs (T-2016).
* podcrates `v0.80.0` taken (T-2050 partial, T-2051 blocked: `warm::line`
  waits for #633). Five `#!` levels; `pty.rs` no longer calls
  `rt_sigprocmask` by the x86_64 number on each arch. #611, #613, #618 and
  #624 closed. T-2018 done.
* Not confirmed: experiment 434 clause 23 (the `oom_kills` of an OOM run)
  failed once in the T-2046 agent's run, likely a timing of the metrics of
  incusd. Run it again before it is called a fault.

## WIP branches (pushed by the owner's word "save them to a wip branch")

* The entries of a branch (2052 to 2055, 2066) are in the `TODO/` files of
  that branch; the index of `main` has them after the merge.
* `wip/podcrates-081` (`1b98db4`, 5 commits on `ac02435`): the take of
  podcrates `v0.81.0` (PodUtils/podcrates#254): entry 2066 partial, T-2051 done
  (`pod_guest::warm::line` with the bounded `Monitor::connect`), T-2046
  (`pod_supervise::stream`, the incus log in pieces with no 64 MiB bound),
  the static QEMU 11.1.3, `pod_core::files::replace`. `checks.sh`, 1,111
  tests and the aarch64 check pass. NOT merged: the binary is 9,005,240
  bytes, over the ceiling of call C24 (9,000,000). Its 465 run failed clause
  11 only (fixed on `main` since). Left: the load wait `save::running`
  stays podvm's (the crate needs the emulator's `Child`).
* `wip/oq-run` (`57463ac`, rebased on `ac02435`): OQ-05 (entry 2052, Ctrl-C of
  `-it` reaches the payload's process group), OQ-07 (entry 2053, one isolation
  table `podvm_probe::isolation` for the banner, `--strict`, `inspect` and
  the manual), OQ-11 (entry 2055, the manual was wrong; tests added) done;
  OQ-08 (entry 2054) partial (process titles and an empty environment through
  `PR_SET_MM_MAP`; the image lock is not handed to the payload). The last
  commit is a `WIP:` size cut to 8,997,080 bytes: fold it into the four
  before it. `cargo test --workspace --no-fail-fast` stopped after 66
  suites with no failure. A COORD ask is in its entry 2054 text: pod-enter's
  PID-namespace init and join relay set a title, an empty environment and
  `PR_SET_DUMPABLE` 0.
* The two branches together are far over the ceiling. First decide the
  size: a call C29 that raises the ceiling with its delta (the release
  profile is already `opt-level = "z"`, LTO, one codegen unit, stripped),
  or cuts. Then merge one branch at a time, run
  `cargo test --workspace --no-fail-fast`, and request gate, arm and
  `kvm.yml` (`init`) for each.

## Next steps, in order

1. Start the session (`AGENTS.md`, section 8), with the watcher.
2. podrunner first: watch #583, #585, #593, #596, #599, #605, #636, #640
   and #644; podrunner#339 waits for #643.
3. The size call, then `wip/podcrates-081` (close #631 and #633 with its
   take; write the COORD of the take), then `wip/oq-run` (write the COORD
   of entry 2054; amend T-2044 with OQ-05 to OQ-13).
4. podcrates: watch #602, #604, #639 and #643; take the tag that has #639
   and #643, then answer #644 with the version.
5. T-2050 remainders: the integration tests as uid 1000; a machine run
   with the store's static QEMU.
6. T-2048 remainder: a SIGTERM during a save waits for the save (at most
   about 7 minutes, an estimate).
7. Experiment 434 clause 23, once more (above).
8. T-2016, T-2002, T-2014, T-2015, then
   [the hard problems](history/hard-problems-2026-11-20.md) and the work
   order of `TODO/PROGRESS.md`.

## Rules learned

* `podvm-count --set T-NNNN STATUS` sets a status; with no argument it
  only writes the counts. A new entry needs its row in `TODO/INDEX.md` by
  hand.
* In a Python script, read a file before you open it for writing: an
  `open(f,'w').write(open(f).read()+x)` empties the file first.
* A run of an experiment uses the release binary of the workspace: do not
  build the release while it runs.
* The release takes the head of `main`: hold each push from the version
  bump until the release run ends (call C22).
* A test that passes on the cloud host and fails on CI can depend on the
  CPU: the cloud host has AMX, which makes `AT_MINSIGSTKSZ` and each
  alternate signal stack of a Rust thread larger. Measure with the probe
  verb `exec-alt SIZE` of `podvm-dispatch`.
* `scripts/dev/checks.sh` does not run `cargo fmt` of
  `crates/podvm-interpose` (it is not in the workspace); the gate does.
  Run `cargo fmt --manifest-path crates/podvm-interpose/Cargo.toml --all`
  after an edit there.
* A release commit is the version bump: run the nightly trial and
  `arm.yml` on it, not on the commit before.
* A `cargo` in a payload cannot pass the proxy of the cloud host (a
  self-signed certificate): fetch the crates on the host into a
  `CARGO_HOME` in the volume, then `cargo ... --offline` in the payload.
* `PODVM_USERNS=0` gives the userland rung only with a volume (`-v`);
  with no volume the run takes the namespace rung (a chroot), where a
  host path is not there. Check the rung with `run --verbose` (`mode=`).
* A probe verb or a test helper in `global_asm!` must carry
  `#[cfg(target_arch = "x86_64")]`: the nightly builds each arch.
* An issue body that the sync tool rewrote has lost its footer: an amend
  puts it back before the block `pod-todo:state`.
* A subagent in a worktree under `.claude/worktrees/` cannot run cargo
  of `crates/podvm-interpose` there: cargo finds `/workspace/podvm/Cargo.toml`
  above it ("believes it's in a workspace"). Build the interposer in the
  main checkout, or from a copy.
* A new module of `podvm-dispatch` carries `#[cfg(target_arch =
  "x86_64")]`, as each module that it uses: `arm.yml` 44699859975 failed
  on it. Check with `cargo check --target aarch64-unknown-linux-musl
  --workspace` before a push.
* A thread of a foreground run blocks `pod_enter::WAIT_FORWARDED`: a
  thread that leaves a signal unblocked takes it with the default action.
* A step of an experiment that only prints a bench line can pass while its
  run hangs: give each experiment an exit code that fails on a missing row.
* A test stand-in of QEMU's monitor must act as QEMU does: one client, a
  listen backlog of 1, the late answer on the same connection.
* A `.gitignore` rule with no leading `/` matches a source directory of
  the same name: check `git check-ignore -v` when a new module "is not
  found" in CI or after a cherry-pick.
* `release.yml` checks for a green gate run on its own commit: a request
  runs the gate, waits for it (`gh run watch --exit-status`), then
  dispatches the release, in one block.
* `scripts/dev/checks.sh` does not run the integration tests of
  `crates/podvm/tests/`; the gate does (`cargo test --workspace`, which
  stops at the first test binary that fails). Before a push that changes
  a behaviour, run `cargo test --workspace --no-fail-fast` with the
  interposer objects built (gate 44859009048: three tests held the old
  exit codes and texts).
