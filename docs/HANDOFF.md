# Handoff

The state on 2026-11-18, 10:30 (UTC), at the end of a session. `main` is
at the commit that adds this file. `0.2.0-beta.32` is the last release
(release run 44278551133 on `566f833`). A fresh session with no memory
reads this file first, then `AGENTS.md`, `TODO/PROGRESS.md` and
`docs/decisions.md`. Remove this file when each step below is done.

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

* podcrates is taken at `v0.70.0` (`6b5a6cb`). The take is recorded in
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
  machine guest) done; ask 6 (T-1969, `oom_kills`) partial; asks 1 and 2
  (T-1964, T-1965) wait for COORD-186 (#582) to podcrates; ask 5 (T-1968)
  open; ask 7 was done (T-1890).
* PodUtils/podrunner#300 (COORD-189, #585): `--env-file0` (T-1971) done
  for the next beta; `--env-file` holds on the machine and incus tiers;
  ask 3, `--podvm-volume-base` on the supervise and userland rungs, is
  T-1972 (P1, open).
* The outside pass (T-1970): the three P1 faults and OQ-05 fixed; the
  others are in "Known gaps" of beta.32. COORD-188 (#584) asks pod-enter
  for a controlling terminal (the `-it` relay turns `^C` into SIGINT until
  then).
* T-1955 partial: the calendar of `podvm-image` is `pod_core::clock`.
* The donor study (the zip of the owner): issues #573 to #581, entries
  T-1956 to T-1963. #573 names an owner decision: a seccomp
  user-notification rung, which T-0606 refused once.

## Next steps, in order

1. Start the session (`AGENTS.md`, section 8), with the watcher. Amend
   COORD-184 (#571) with the state of the session.
2. A new COORD issue `(to podvm)` of podrunner first. Watch #572, #583
   and #585 for podrunner's answers (it pins beta.32 and closes its
   asks). Then T-1972 (the volume base on the supervise rung first).
3. Dispatch `kvm.yml` with `only=cgroup` on `main`: experiment 459 clause 5
   proves `oom_kills` (T-1969).
4. When podcrates answers COORD-186: T-1964 and T-1965, then T-1969 on
   the incus tier; then T-1968.
5. When podcrates answers COORD-188: set the controlling terminal for `-t`
   and remove `run::pty::send`.
6. T-1970 remainder (OQ-04, OQ-06 to OQ-17), T-1935 (the syscall tier by
   default after its cost, T-1740), T-1955 remainder, then the work order
   of `TODO/PROGRESS.md`.

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
