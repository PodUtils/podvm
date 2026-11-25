# Handoff

The state on 2026-11-25 after work resumed in Codex. The local checkout
contains the commit that changes this file. A fresh session reads
`AGENTS.md` first, then this file, `TODO/PROGRESS.md` and
`docs/decisions.md`. The handoffs before this one are in git: `git show
0cb62f18:docs/HANDOFF.md`. Remove this file when each step below is done.

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

* `v0.2.0-beta.40` is released (release run 45436996748 on `e6bf9aeb`,
  the second attempt; the first stopped in the bootstrap of four builds of
  the nightly run of the tag, a slow download of zig). Its gate
  (45420327645), the nightly trial (45420327893), `arm.yml` (45420337077),
  the job `incus` of `kvm.yml` (45405003660), `workloads.yml`
  (45390483251), the integration tests as uid 1000 (37 binaries) and the
  ninth outside pass (T-2087, its three P1 faults fixed) passed. #662
  (COORD-246) tells podrunner.
* podcrates `v0.83.0` is taken on `main` after the tag (T-2089, `a39f4c4f`;
  #663, closed: PodUtils/podcrates#259 lists the take). podrunner pins
  beta.40 in its `4fc871c` (#662, closed).
* `main` takes podcrates `v0.84.0` and its shared gzip writer (T-2096,
  `04c8b7f4`, #666, PodUtils/podcrates#260). T-2085 has a stand-in
  proof. Hosted Incus run 45619039653 on `6bb0ae96` runs experiment 475
  for the fresh and cached layer-stream assertions of experiment 434
  clause 31.
* T-2054's spare-tree helper now clears its environment. Both
  `tree-prefill` paths worked with an empty environment in a private
  Alpine store. T-2054 still tracks the helper's store path in argv.
* The saved-state and layer-stream follow-up passed `sh scripts/dev/checks.sh`.
  The candidate, a static binary of 9,488,600 bytes, passed experiment 473
  under TCG. An earlier check during a QEMU run had one broker test fail.
  The test passed alone and the full check passed with QEMU stopped.
* Hosted Incus run 45616608586 on `ddb185b1` passed experiments 471 and
  474. T-1964 is done. A 1 GiB Btrfs root refused the large write with
  `Quota exceeded` and exit 7 while the pool still had 7,507,083,264
  bytes free. See `experiments/results/incus-root-quota.txt`.
* On `main` after beta.40, not released (CHANGELOG, "Unreleased"): the
  room of an incus pool from incusd for each driver (T-2078 remaining 1,
  `a033f62`, experiment 472), what the namespace rung opens (T-2092, #546),
  the commit of `podvm-dev build` (T-2091, #531). Gate 45536609832 on
  `a033f62` and gate 45539122840 on `07416ff` are green.
* The start sweep found 52 open issues in GitHub and in the record.
  #664 (the pod-incus request) is open. Direct dispatch superseded #665.
  Podcrates lists the take under Takes in PodUtils/podcrates#260.
  The record closes #666 when it is pushed and issue-sync runs.
* The issue cleanup of 2026-11-24 is done: four agents checked each open
  issue against the code, the record and a run of the beta.40 binary. 87
  issues are closed with a comment of evidence; 56 stay open, each with a
  body that says what is true now (the bot block stays under it). GitHub
  and `TODO/issues.md` agree: 56 open in both. The new entries are T-2088
  (the QA faults of beta.31 that stay open) and T-2090 (the QA reports that
  beta.40 passes).
* The cause of the stale issues: an issue maps to an umbrella entry (for
  example T-1951) that stayed partial while most of its findings were
  fixed; the sync bot keeps an issue open while its row says open, and the
  body keeps the text of the day it was opened. Map each new issue to the
  entry of its own finding, and close its row in the same commit as the
  fix.
* The bot issue-sync sets each issue from the record of the pushed commit:
  close an issue in the record first (`TODO/issues.md` and
  `TODO/github.txt`, pushed), then on GitHub, or the bot reopens it.
* The Claude Code host measured in this handoff has cgroup v1, no
  `/dev/kvm`, and an incusd with no AppArmor; a fault that AppArmor or
  cgroup v2 makes shows only in the job `incus` of `kvm.yml`
  (experiment 471, about 4 min).

## Next steps, in order

1. Two asks of podrunner (2026-11-25): PodUtils/podrunner#354
   (COORD-132, `podvm update --podvm-tier=incus` of the memory, CPU and
   process limits of a running instance: T-2093) and PodUtils/podrunner#355
   (COORD-133, a saved state of the machine tier for `--podvm-channel` and
   `--pids-limit`: T-2094). T-2093 waits for pod-incus instance config
   PATCH and ETag support (#664, COORD-248 to podcrates). T-2094 passed
   experiment 473 under TCG. Answer each ask with a COORD issue
   `(to podrunner)` when its work is in a beta.
2. T-2078 remaining 3: a run on the hosted runner of
   PodUtils/podrunner#346 with the next beta (remaining 1 is done,
   experiment 472).
3. Sweep the current open issues at each session start (`AGENTS.md`, section 8
   item 7), and map each new issue to the entry of its own finding.
4. podrunner: #583 (T-1968, a sized writable disk). The machine tier
   needs the disk-backed root of T-1994, and the namespace tier needs
   the pod-enter mount requested in #667. Then #596 and #599 (the
   hosted snapshot: T-1994, T-1996, T-1998, T-1999), #593 (T-1990,
   T-1991). Then T-2088 and the open rows of T-2070, T-2079, T-2087.
5. T-2085 remainder (run the layer stream proof against incusd,
   PodUtils/podcrates#260, the answer to #657),
   T-2054, T-2050, T-2048, then the work order of
   `TODO/PROGRESS.md`.

## Rules learned

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
