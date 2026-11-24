# Handoff

The state on 2026-11-24, 12:00 (UTC), at the end of a session. `main` is
at the commit that changes this file or later. `0.2.0-beta.39` is the last
release (2026-11-24, release run 45353882402 on `a43c9476`, podcrates
`v0.82.0`). `main` carries podcrates `v0.82.1` and three fixes for
podrunner after it (below). A fresh session with no memory reads this file
first, then `AGENTS.md`, `TODO/PROGRESS.md` and `docs/decisions.md`. The
handoffs of the sessions before 2026-11-23 are in git: `git show
836c0799:docs/HANDOFF.md`. Remove this file when each step below is done.

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

* podcrates is taken at `v0.82.1` (T-2084, `18160173` to `e7c19b16`;
  #656, COORD-240, closed: PodUtils/podcrates#257 lists the take). A podvm
  session can not write on PodUtils/podcrates issues (its GitHub access
  reaches podvm only): each take is a COORD issue of this repository.
* Each release publishes `ghcr.io/podutils/podvm:TAG` by the job `image`
  of `release.yml` (`image.yml`, T-1992).
* The issue-sync bot of a push sets the state of each issue from the
  record of THAT commit (`TODO/issues.md`, `TODO/github.txt`): close an
  issue in the record first, in one commit on `main`, before or with the
  close on GitHub, or the bot reopens it. A peer's own bot does the same
  on its side (podrunner#339 was reopened and closed again this way).
* The pkgforge mirrors answer for podvm and podcrates reads, with some
  403s; retry. Release assets of another repository: the GitHub MCP tool.
* The cloud host has cgroup v1 (hybrid), no `/dev/kvm`, and after a
  restart of the container NO running system incusd. Experiment 434
  (`PODVM_434_MIRROR=1 sh experiments/434-incus-tier.sh`) starts what it
  needs; for a measurement by hand, start a private incusd
  (`INCUS_DIR=DIR /usr/libexec/incus/incusd`) and point podvm at it with
  `PODVM_INCUS_SOCKET`. Clause 23 of experiment 434 (the `oom_kills` of the
  report of a foreground run, from incusd's metrics) fails now and then
  (a known flake of incusd, T-2076).
* `docs/maintenance.md`, "Releases", is the gate of each beta (T-1949):
  gate, `workloads.yml`, the uid-1000 tests, an outside pass with the
  binary and `podvm man` only, `arm.yml` and the nightly trial on the bump
  commit, and an entry "Known gaps". A script for the uid-1000 tests:
  build the test binaries of `podvm` and `podvm-supervise` (`cargo test
  --release --no-run --message-format=json`) and run each with `setpriv
  --reuid 1000 --regid 1000 --clear-groups`, `HOME` and `TMPDIR` in a new
  directory of mode 777 under `/var/tmp`, `CARGO_MANIFEST_DIR=$PWD/crates/podvm`.
* The size of the release `podvm` is 9 197 784 bytes at `cd51868e` (the
  ceiling of call C29 is 9 500 000).

## The sessions of 2026-11-23 and 2026-11-24

* Released `0.2.0-beta.38` (#648) and `0.2.0-beta.39` (#654), each behind
  the beta gate with an outside pass: the seventh (T-2070: two P1 and three
  P2 faults fixed, T-2071 to T-2075) and the eighth (T-2079: four P1 faults
  fixed, T-2080, T-2081 and the rule of `cp`; seven P2 faults fixed). Call
  C29: the size ceiling is 9 500 000 bytes.
* podcrates `v0.81.0`, `v0.82.0` (#645) and `v0.82.1` (#656) are taken.
* podrunner, each answered by a COORD issue: #339 (PAX ACLs, T-2065, closed),
  #344 (85 layers, T-2068, closed), #345 (the use of a stopped incus
  instance, T-2076; #649), #346 (a full pool, T-2078; #652), #347 (ask 2:
  SIGPIPE, T-2082; ask 1: one tree and a gzip upload, T-2085; #658), #349
  (the readiness of an init on the incus tier, T-2083; #655).
* Asks to podcrates that stay open: #651 (an answer over the bound keeps
  its status and head; the resources of a pool), #653 (the exit of an exec
  that ends with 126 or 127, a bound of each answer, the answer after a
  failed write), #657 (a stream of the layers as one tar, a writer of gzip
  members), and the older #602, #604, #584.
* After `0.2.0-beta.39`, `main` carries T-2083, T-2084 and T-2085; they
  wait for the next beta.

## Next steps, in order

1. Start the session (`AGENTS.md`, section 8), with the watcher.
2. Done (2026-11-24): T-2086 (podrunner#350). The AppArmor profile of
   incus refused the init's mount of the cgroup; the init now takes the
   cgroup mounts of the instance (`88e1a0d`). Proof: `kvm.yml` job
   `incus` (experiment 471, new) run 45384508724. Answered by #660
   (COORD-244); the mount reader is asked of podcrates by #659 (COORD-243).
3. The next beta (`0.2.0-beta.40`) behind the full beta gate, with
   T-2083, T-2084, T-2085 and T-2086: name the tag on #655, #658 and #660 and in
   a COORD "beta.40 is out" to podrunner.
4. Watch #583, #585, #593, #596, #599, #605, #636, #648, #649, #652, #654,
   #655, #658, #660 (podrunner) and #602, #604, #584, #651, #653, #657,
   #659 (podcrates); take the next podcrates tag that has #651, #653, #657
   or #659.
5. T-2085 remainder (a run on the hosted runner; the stream of the layers
   when podcrates gives it), T-2078 remainder (a check of the pool for an
   image that incus has), T-2069 (the staging files of the store), the
   open rows of T-2070 and T-2079, T-2054 (items 1, 2, 3 and 5 of OQ-08:
   close as limits in `docs/limits.md`, or fix).
6. T-2050 remainder (a machine run with the store's static QEMU), T-2048
   remainder (a SIGTERM during a save), T-2016, T-2002, T-2014, T-2015,
   then [the hard problems](history/hard-problems-2026-11-20.md) and the
   work order of `TODO/PROGRESS.md`.

## Rules learned

* The cloud host's incusd has no AppArmor: a fault of the incus tier that
  AppArmor makes shows only on a hosted runner. The job `incus` of
  `kvm.yml` (`only: incus`, about 4 min) runs experiment 471 against the
  system incusd of apt there (T-2086).
* `podvm-count --set T-NNNN STATUS` sets a status in `TODO/INDEX.md` and the entry; the counts line of `TODO/PROGRESS.md` is a copy by hand of the counts that it prints. With no argument it
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
* The record gate reads each `T-NNNN` of four digits as a podvm entry, also
  after `podcrates@`: write "podcrates task 1124" or name the issue.
* In the PID namespace of a run, a host pid names nothing for `pidfd_open`:
  a join uses the pid and the pidfd that it kept (T-2084).
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
