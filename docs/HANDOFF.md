# Handoff

The state on 2026-11-26 after the Claude Code session that released
`0.2.0-beta.41`. A fresh session reads `AGENTS.md` first, then this file,
`TODO/PROGRESS.md` and `docs/decisions.md`. The handoffs before this one
are in git: `git show 5377e4a5:docs/HANDOFF.md`. Remove this file when
each step below is done.

## The rulings that stay in force

* "Watchers" and "Work until done" (2026-11-24, `docs/decisions.md`): the
  session hook starts `watch.py` and `runs.py` of podcrates (`v0.83.0`) as
  background processes with logs (`.work/watch.log`, `.work/runs.log`);
  read the logs and the runs yourself, and never wait for a notice of the
  harness. Work each step below until it is done or truly blocked; ask the
  owner only for a decision that only the owner can make.
* "podrunner first" (2026-11-18): after a red run of podvm's CI, each ask
  of podrunner comes first.
* "Schema ids": an added optional field keeps `run-report.v1`; a changed
  or removed field needs `v2` and a COORD issue `(to all)`.

## Where things are

* Workflow dispatch: in the Claude Code session of 2026-11-26, the GitHub
  tool refused each dispatch ("Actions has been disabled for this user"),
  and a push from the session started no workflow (no gate, no
  `issue-sync`, no `secrets`). Each run went through an issue `[gh action]
  requested` (#674). The scheduled `issue-sync` (06:17 UTC) writes the
  blocks of the issues that the session opened.
* `v0.2.0-beta.41` is released (release run 45777325284 on `332efaa`, 59 assets, with the image on ghcr.io). Its gate 45772974200, `workloads.yml`
  45772978726, the nightly trial 45772982947 and `arm.yml` 45772987551
  passed on the version commit `332efaa`. The integration tests as uid
  1000 (37 binaries) passed on `7c2b161`. The tenth outside pass (T-2100,
  [the report](history/outside-pass-2026-11-26.md)) found no P0 or P1
  finding in the binary of `7c2b161`. The pass was not blind by a tool:
  the tester ran as root on the host of the checkout.
* The beta carries T-2093 (podrunner#354), T-2094 (podrunner#355), T-2098
  (#671), T-2048 (a signal during the save of a guest state, experiment
  480), T-2054 item 5 and three findings of the tenth pass.
* On `main` after the beta (CHANGELOG, "Unreleased"): OR-B41-02 (the copy
  of QEMU in `PODVM_STORE`), OR-B41-07 (an incus `--rm` run after the OOM
  killer), OR-B41-13 (`notebook` with no notebook), OR-B41-14 (`system
  df`).
* PodUtils/podrunner#359 (COORD-135, bytes read through each volume):
  T-2099 is partial. COORD-255 (#672) gives the feasibility: no counter
  exists on either tier; the recommended route is a counting FUSE
  pass-through in the guest and the container. It waits for podrunner's
  choice of the route and the field.
* COORD-256 (#675) asks podcrates for the path of the emulator in the
  machine legs of `pod-hostprobe` (OR-B41-01, OR-B40-07).
* The measured Claude Code host: cgroup v1, no `/dev/kvm`, the `incus`
  package of apt is installed (incusd 6.0.0 runs as root with
  `INCUS_DIR` in a private directory). The 37 integration test binaries
  pass there as uid 1000, also `pid_init_hidden`, `store_gates` and
  `refusal_text`, which failed on the host of 2026-11-25.
* T-2078 remaining 3 still needs a run on the hosted runner of
  PodUtils/podrunner#346 with the beta.

## Next steps, in order

1. Make the docs true for beta.41 (README, `docs/limits.md`, the
   per-release state of `TODO/PROGRESS.md`), as after beta.40. Watch
   COORD-257 (#676) for podrunner's take of beta.41 and its rerun of the
   runner-image build (#671).
2. T-2078 remaining 3 (a hosted run of podrunner#346 with beta.41).
3. Sweep the open issues (`AGENTS.md`, section 8 item 7). Watch #672
   (COORD-255) for podrunner's choice, and #675 (COORD-256) for the tag
   of podcrates.
4. The open rows of T-2100: OR-B41-06 (a `run --rm` that SIGPIPE ends
   keeps its record: podvm dies with SIGPIPE by design, T-1769, and
   `eprintln!` panics on EPIPE, so the fix needs a design), OR-B41-08
   (T-1939), OR-B41-09 (needs `diff_ids` in `pod_oci::spec::Config`), the
   P3 rows.
5. podrunner: T-1968 is done on `main` (the machine tier's sized disk,
   experiment 481; `--tmpfs GUEST:size=N` on the namespace rung with
   podcrates `v0.86.0`, T-2102). Podrunner's 9p work mount stays unbounded
   by the disk (#583 says so). T-1994 steps 1 to 3 are on `main` (a root
   disk of each image, experiment 482; systemd as PID 1 of the guest,
   experiment 483); the saved state on the disk root is in progress.
   T-2101 (`-v HOST:GUEST:overlay`, podrunner#368, experiment 485) and
   T-1991 (the syscall tier's `brk`, experiment 484) are on `main`. Each
   waits for the next beta, then a COORD to podrunner. T-2103
   (podrunner#371) is in progress. Then #596 and #599 (T-1996, T-1998,
   T-1999), T-1990, T-2088 and the open rows of T-2070, T-2079, T-2087,
   T-2100.
6. T-2054 items 2 and 3, then the work order of `TODO/PROGRESS.md`.

## Rules learned

* `scripts/dev/checks.sh` runs the unit tests as the user of the session
  (root on the cloud host); the gate runs them as a user with no root.
  Before a push of a change that reads files of an image, run the unit
  test binary as uid 1000 too (`setpriv --reuid 1000 --regid 1000
  --clear-groups`, absolute path to the binary): gate 45812348716 failed
  on a file of mode 0000 that root could open.
* `kvm.yml` has the concurrency group `kvm`: GitHub keeps one pending run
  of a group, so a third dispatch cancels the second while the first runs
  (#681: the `init` job, run 45812371884). Dispatch one `kvm.yml` job, or
  run each job of one dispatch (`only` empty), not several dispatches at
  once.
* Write the full SHA in a request block from `git rev-parse HEAD`, never
  from memory: #673 did not run, because its head check named a wrong
  full SHA.
* `sh scripts/dev/checks.sh && git commit ...`, never `;`: a `;` commits
  after a failed check.
* A blind tester that runs as root can damage the bundle: `system
  install-names` makes links to the binary, and a write to a link wrote
  the binary. Give the tester a copy of the bundle, and check
  `SHA256SUMS` after the pass.
* The keyless reads of the forge refused `runs.py` (403) for the whole
  session; `gh run view` without a token and the GitHub tool read the
  runs.
* The record gate refuses a closed issue whose entry is not done: where a
  fix closes one item of an umbrella entry (T-2088), give the item an entry
  of its own (T-2091, T-2092) and map the issue to it in the same commit.
* pod-incus cuts a long error of incusd with `…` (U+2026): an experiment
  that copies stderr into `experiments/results/` puts `...` in its place,
  or `check-markers` fails.
* btrfs of a small loop file gives incusd more free space than a write can
  use (74.5 MiB free while a write fails, experiment 472): a check of the
  room on btrfs refuses only a need over that number.
* The cloud host can run a root incusd of apt (`/usr/libexec/incus/incusd
  --group root`) with a btrfs pool on a loop file; experiment 472 uses it.

* The measured Claude Code host's incusd has no AppArmor: a fault of the incus tier that
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
* Two watchers and a `--until` at 30 s spent the keyless reads of the
  forge in minutes (the rule is in `AGENTS.md`, section 9).
* The bot issue-sync writes each body back with a new block at each push:
  a body that a session writes while that run is in progress can be lost
  (#546 and #567 were). Write bodies when no issue-sync run is in
  progress, and read them back after the next one.
* The GitHub write tool of a cloud session drops the footer of an issue
  body (each body of 2026-11-24); the footer stays on a comment.
* A job of a workflow can end at its time limit while its bootstrap waits
  for ziglang.org (6 to 15 min on 2026-11-24): a cancelled run is not a
  pass; read the step, and run the failed jobs again.
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
