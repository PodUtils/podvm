# Handoff

The state on 2026-11-23, 17:00 (UTC). `main` is at the commit that changes
this file or later. `0.2.0-beta.38` is the last release (2026-11-23,
release run 45212007696 on `2ae4b41`, podcrates `v0.82.0`). A fresh session
with no memory reads this file first, then `AGENTS.md`, `TODO/PROGRESS.md`
and `docs/decisions.md`. The handoffs of the sessions before 2026-11-23 are
in git: `git show 836c0799:docs/HANDOFF.md`. Remove this file when each
step below is done.

## The rulings of 2026-11-18 that stay in force

* "podrunner first" and "Coordinator" (PodUtils/podcrates#225, COORD-134;
  `docs/decisions.md`): after a red run of podvm's CI, each ask of
  podrunner comes first, then the asks of githome and cfb, then podvm's
  own order. The session of podcrates coordinates; COORD issues are the
  one channel (PodUtils/podcrates#227, COORD-136). Each session writes its
  state to the coordinator at its start: podvm's is COORD-184 (#571);
  amend it. (2026-11-23) The owner closed PodUtils/podcrates#225
  (COORD-134) on 2026-11-23 as complete.
* "Schema ids": an added optional field keeps `run-report.v1`; a changed
  or removed field needs `v2` and a COORD issue `(to all)`.


## Where things are

* podcrates is taken at `v0.82.0` (T-2066, `4fa56ec` to `a43e80cd`;
  COORD-229, #645, names the take; podcrates#254 still listed "podvm: not
  yet" at 2026-11-23 13:00). A podvm session can not write on
  PodUtils/podcrates issues (its GitHub access reaches podvm only): each
  take is a COORD issue of this repository.
* Each release publishes `ghcr.io/podutils/podvm:TAG` by the job `image`
  of `release.yml` (`image.yml`, T-1992); `image.yml` can be dispatched
  alone for a tag (`push: false` is the dry run).
* The issue-sync bot of a push reopens or closes an issue as the record of
  THAT commit says: a sync of an older commit can reopen an issue that a
  newer record closed; the sync of the newer commit closes it again.
  (2026-11-23) The issue events of 2026-11-18 to 2026-11-20 show the
  pattern: a session closed #428, #471, #586, #589, #595, #597, #600 and
  #601 on GitHub, the bot reopened each within minutes (the record still
  said open), and a later sync or the session closed it again. So close an
  issue in the record first: its row in `TODO/issues.md` and its line in
  `TODO/github.txt`, in one commit on `main`, before or with the close on
  GitHub.
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


## This session (2026-11-23)

* Call C29 (`4d8da41`): the size ceiling is 9 500 000 bytes. The release
  `podvm` of beta.38 is 9 046 232 bytes.
* `wip/podcrates-081` and `wip/oq-run` are merged (`51c16c5`, `02c7c429`);
  both branches stay on origin as history. podcrates `v0.82.0` is taken:
  T-2065 (PAX ACLs, podrunner#339) and T-2062 (126 for E2BIG) are done.
* podrunner#344 (COORD-122): `load` of 85 layers (T-2068, `4887fb72`);
  answer #647. T-2069 (open) has two faults of the staging files and
  OR-B38-05.
* The seventh outside pass (T-2070) found 18 faults; the two P1 faults and
  three P2 faults are fixed (T-2071 to T-2075); the rest are in "Known
  gaps" of beta.38.
* Released `0.2.0-beta.38`; #648 (COORD-232) names it to podrunner, #644
  and #647 name the tag, #640 is closed.
* COORD-230 (#646) asks podcrates for the title, the environment and the
  dumpable flag of pod-enter's init and join relay (T-2054, OQ-08).
* podrunner#345 (COORD-123): `inspect` of a stopped incus instance gives
  the last `CpuSeconds`, `PeakMemoryBytes` and `OomKills` (T-2076,
  `a4b75950`, `bafd6dec`; experiment 434 clause 26); answer #649
  (COORD-233). #650 (COORD-234) asks podcrates for one reader of the usage
  of a cgroup. The work found T-2077 (P1, done in `4ab994b9`): under
  `--memory` the OOM killer of the instance took PID 1 (12 of 44 runs, also
  with beta.38); PID 1 now sets -1000, or the command gets 1000 where the
  kernel refuses -1000; 24 of 24 runs keep the exit code. #649 says so.
* #631, #633, #639, #643, #644 and #645 are closed (podcrates#254 lists the
  take; podrunner closed podrunner#339).

## Next steps, in order

1. Start the session (`AGENTS.md`, section 8), with the watcher.
2. podrunner first: the next beta carries T-2076 and T-2077 for #649
   (the beta gate of `docs/maintenance.md`); watch #583, #585, #593, #596,
   #599, #605, #636, #648 and #649.
3. podcrates: watch #602, #604, #584, #646 and #650.
4. T-2069 (the staging files of the store), then the open rows of T-2070.
5. T-2050 remainder: a machine run with the store's static QEMU. T-2048
   remainder: a SIGTERM during a save waits for the save. Experiment 465 on
   a host whose QEMU does not crash under TCG (T-2016), or under KVM.
6. T-2002, T-2014, T-2015, then
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
* The sync tool writes only the block `pod-todo:state` and keeps each other
  byte of a body (`with_block` in the script issue-sync.py of podcrates,
  `v0.81.0` and `v0.82.0`). The source of a body text is the body on GitHub,
  not the record: the record holds the subject, the state and the entries.
* The GitHub write tool of a cloud session removes the footer "Generated by
  Claude Code" from an issue body, at the end or before the block
  (measured on #644 to #648, 2026-11-23). The sync does not remove it. A
  session writes the footer each time; it cannot make it stay.
* The watcher pin `f76099d` stops on a cut answer of the API
  (`IncompleteRead`); the pin `ed6d84c` of `AGENTS.md` retries it.
* A worktree under `.claude/worktrees/` is the place for record and docs
  commits while an agent holds the main checkout: `git push origin
  HEAD:main` from it, then tell the agent that `main` moved. The prebuilt
  `podvm-gate` finds the root of the repository from its own path: copy it
  into the target directory of the worktree.
* `podvm-image` `clock::tests::an_age_is_worded_the_way_docker_words_it`
  reads the clock twice and failed once on a loaded host ("2 seconds ago"
  for "1 second ago", 2026-11-23); it passes alone. A test of an age gives
  the clock as an input.
* Two agents that each need cargo run one after the other: the second
  waits for a marker file that the lead makes when the first ends.
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
