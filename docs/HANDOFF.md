# Handoff

The state on 2026-11-18, 22:45 (UTC), at the end of a session. `main` is
at the commit that adds this file. `0.2.0-beta.35` is the last release
(release run 44406181198 on `9e1e2e7`; the gate 44400983081, `workloads.yml`
44400995816, `arm.yml` 44400989122, the nightly trial 44400982967, the
uid-1000 tests and the fourth outside pass passed first). A fresh session
with no memory reads this file first, then `AGENTS.md`, `TODO/PROGRESS.md`
and `docs/decisions.md`. Remove this file when each step below is done.

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

* podcrates is taken at `v0.71.0` (`c213375`; `v0.70.0` in `6b5a6cb`).
  The takes are recorded in
  COORD-184 (#571): a podvm session can not write on PodUtils/podcrates
  issues (its GitHub access reaches podvm only).
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

## Next steps, in order

1. Start the session (`AGENTS.md`, section 8), with the watcher (filter
   its output to the event lines: a network retry prints a traceback).
2. podrunner first: watch #593 (its data of the hang of T-1991, and the
   measure of T-1990 with the next beta), #590 and #592. The next beta
   carries T-1990; its gate needs a new outside pass.
3. T-1972 remainder (the loader family), T-1969 remainder (the run report
   of an incus unit), then T-1968; the T-1964 remainder waits for a
   measure on a host with a quota pool (podrunner).
4. T-1984 and T-1985 (P2): enforce them where podvm sees the call.
5. When podcrates answers COORD-188 (#584): set the controlling terminal
   for `-t` and remove `run::pty::send`.
6. T-1989, T-1980, T-1974 remainders, T-1970 remainder, T-1935, T-1955
   remainder, then the work order of `TODO/PROGRESS.md`.

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
