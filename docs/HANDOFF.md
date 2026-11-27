# Handoff

The state on 2026-11-27 after the second Claude Code session of that day.
A fresh session reads `AGENTS.md` first, then this file,
`TODO/PROGRESS.md` and `docs/decisions.md`. The handoffs before this one
are in git: `git show 5377e4a5:docs/HANDOFF.md` and `git show
acf6932:docs/HANDOFF.md`. Remove this file when each step below is done.

## The rulings that stay in force

* "Watchers" and "Work until done" (2026-11-24, `docs/decisions.md`): the
  session hook starts `watch.py` and `runs.py` of podcrates (`v0.83.0`) as
  background processes with logs (`.work/watch.log`, `.work/runs.log`);
  read the logs and the runs yourself, and never wait for a notice of the
  harness. Work each step below until it is done or truly blocked; ask the
  owner only for a decision that only the owner can make.
* "P1 before the beta" (2026-11-27): `0.2.0-beta.42` waits for the fix of
  T-2105 and T-2106 in pod-enter (COORD-268, #695).
* "podrunner first" (2026-11-18): after a red run of podvm's CI, each ask
  of podrunner comes first.
* "Schema ids": an added optional field keeps `run-report.v1`; a changed
  or removed field needs `v2` and a COORD issue `(to all)`.

## Where things are

* Workflow dispatch: a push from a Claude Code session starts no workflow,
  and the session cannot dispatch. Each run goes through an issue
  `[gh action] requested` (the owner's agent runs it). PR #707 (merged in
  `413990b`) adds the startup of Gemini / Antigravity Cloud sessions
  (`--no-watchers`, `--run-watcher watch|runs`); `scripts/dev/checks.sh`
  now sources `scripts/cloud-env.sh`.
* `v0.2.0-beta.41` is the newest release (2026-11-26). Each change after it
  is in the `## Unreleased` sections of `CHANGELOG.md`.
* Merged on `main` in this session (each with a table test):
  * T-1947: one bounded parser of each number flag (`crates/podvm/src/number.rs`),
    and `stop -t -1` refused with its own line (T-2108). COORD-270 (#698)
    asks podcrates for a shared parser.
  * An argument that is not UTF-8: one line with its position and class,
    125 (T-2087 OR-B40-09, T-2108 OR-B42-04). A payload word cannot pass as
    bytes until `pod_enter::Plan` takes bytes: COORD-271 (#701).
  * `--podvm-mem` under 256 MiB is refused with the minimum (OR-B41-15);
    `probe` gives the namespaces of a foreground `run` (OR-B38-10).
  * The pty of `-it` is the controlling terminal for a user with no root on
    the namespace rung (OR-B38-17, OR-B40-13). The root case and `-t`
    without `-i` need COORD-188 (#584, amended with the exact place in
    `launch::spawn_with`).
  * docker's `--format` templates (`{{json .}}`, `table`, `range`,
    `index`, `join`; `crates/podvm/src/format/`). COORD-272 (#703) asks
    podcrates to take the engine.
  * `inspect` gives docker's `.Mounts`, `.HostConfig.Binds` and
    `.HostConfig.Tmpfs` (T-2108 OR-B42-08).
  * T-2017: the init route of the image applies `--ulimit`,
    no-new-privileges and each `--device` that reads; `--podvm-confine` and
    a write-only device stay refused (call C28 amended in place: the rule
    "refuse what the route does not apply" stays, the list is shorter; the
    owner may review it).
  * T-1998: `--podvm-tier=incus --podvm-vm --podvm-init=image` (an image
    with a kernel as an incus VM; qcow2 writer, direct boot). COORD-273
    (#705) asks pod-guest to take the ext4 and qcow2 writers.
  * The tests that create a container set `PODVM_SPARE_TREE=0`, and the
    guard `crates/podvm/tests/spare_tree_off.rs` holds the class (T-1859).
* Gate 46003966264 passed on `008686e`. The gate before it (45988037109 on `2497a26`) failed in one test,
  `tty_ctty`: the hosted runner runs `-it` on the userland rung, where the
  shell cannot open `/dev/tty`. The cause was the session, not `/dev/tty`: a shell or a terminal starts podvm as the leader of its group, so its `setsid` failed and the payload stayed in the session of the caller. Merged in `af8abf0`: for `run` and `exec` with `-t` and `-i`, podvm forks once (`run/pty/session.rs`), the child leads a new session with the pty, and the parent relays signals and the exit code (`ps` shows two podvm processes). `tty_ctty` is a table over the rungs, with a row for a host with no user namespace. As root on the namespace rung the payload gets no terminal (job control off, Ctrl-C works) until COORD-188.
* The hosted proof of T-1998 (kvm 45991827383, #706) did not pass: incusd
  had the `qemu` driver, but the first `run -d` gave no line until the
  bound of the experiment (1500 s). The boot wait of podvm has a bound of
  180 s, so the time went before the boot. Experiment 492 now runs with
  `--verbose` and lists the state of incusd when clause 1 fails.
* Merged in `c6b199e`: each file of the machine tier goes to the store (`Store::configured_root`; sockets stay in `XDG_RUNTIME_DIR` for the length limit) (OR-B41-02); `ps` has docker's order, columns and words (`.RunningFor`, `lifecycle/ps_view.rs`); `verify` names an image as `images` does; `rm -f` of a missing container exits 0 (T-2079 is done). COORD-274 (#708) and COORD-275 (#709) ask podcrates for the rest.
* The integration tests pass as uid 1000 on `c6b199e` (47 binaries,
  each merge of this session included).
* Open asks to podcrates: COORD-268 (#695, P1, blocks the beta), #677,
  #678, #680, #683, #684, #689, #690, #692, #697, #698, #701, #703, #705, #708, #709,
  COORD-188 (#584). To podrunner: #672, #676, #693.

## Next steps, in order

1. The gate is green: 46003966264 passed on `008686e` (#710), with the fix of `tty_ctty` on the userland rung. Request the gate again after each merge.
2. T-1998: request `kvm.yml` with `-f only=incusvm` again, read the
   `--verbose` lines in the result file (the artifact `incus-vm`), find
   the stage that takes the time (the disk, the qcow2 write or the import),
   and give it a bound of its own with one line.
3. T-2017 Prove: experiment 465 clause 12 needs a host whose user has a
   subordinate id range (the cloud host refused the creation of one).
   Clauses 2 and 3 (cgroup2 limits, the runsc and machine tiers) remain.
4. `0.2.0-beta.42` waits for podcrates (COORD-268, #695): take the tag,
   prove experiments 489 and 490 with no OPEN row, run a blind QA of the
   candidate, and the gate of the beta. Then the COORDs to podrunner for
   each ask that the beta delivers (T-1968, T-2101, T-2103, T-2104,
   T-1991, T-1994, T-1998, T-2105, T-2106).
5. Sweep the open issues (`AGENTS.md`, section 8 item 7), and take each
   podcrates tag that answers a COORD above.
6. The open rows of T-2070, T-2079, T-2087, T-2100 and T-2108, then
   T-1990, T-2088, T-2054 items 2 and 3, and the work order of
   `TODO/PROGRESS.md`.

## Rules learned

* The hosted runner gives no unprivileged user namespace, so `run -it`
  there takes the userland rung: a test of a terminal that passes on the
  namespace rung of the cloud host can fail in the gate (45988037109).
  Make a test of the terminal a table over the rungs that CI gives.
* A test that imports an image and creates a container sets
  `PODVM_SPARE_TREE=0`: else a detached spare-tree fill writes into its
  directory after the test removed it, and the check T-1859 fails the gate
  (45960813246). The guard `spare_tree_off.rs` holds the class.
* While a request `[gh action] requested` pins the head of `main`, merge a
  worker's branch in its own worktree (merge `main` into it there) and keep
  record commits on a local branch `hold/rec` in a worktree; fast-forward
  `main` after the request ran. The stop hook asks to push each commit of
  `main`.
* `git cherry-pick` takes no `-q`: a chain `git cherry-pick -q X && ...`
  stops there.
* The hook `commit-msg` refuses the name of a tool or a model in a commit
  message (for example the name of a harness in a subject).
* `incus query` refuses `--project`: put `?project=NAME` in the path.
* An experiment on a hosted runner must print each stage of a long run
  (`--verbose`) and the state at a failure: kvm 45991827383 ended at the
  bound with no line to read.
* After a usage limit, a worker stops with its work saved: continue it with
  SendMessage, not a new worker.
* A test row that the record names (`number_flags`) and a later fix of the
  same value (T-2108's `stop -t -1`) can disagree: run the whole
  integration suite as uid 1000 after each merge (it found this one before
  the gate did).

* Run the whole integration suite as uid 1000 after each merge, not only
  the tests that a worker names: the fix of #547 (`de5e01b`) refused
  `create` of an image with no `/tmp` mount point, and two tests that the
  worker did not run (`cp_tree`, `container_prune`) failed on the merge.
* A worktree under `.claude/worktrees/` gets the interposer objects as
  symlinks (the two build directories of the interposer). Commit
  `5916373` tracked the symlink of the glibc 2.34 build (its ignore rule ended in
  `/`, which matches a directory only); the checkout of the merge on `main`
  replaced the directory of the built objects with a symlink to itself.
  The rule has no `/` now. After a merge, check `git ls-files
  crates/podvm-interpose` and that `podvm version --verbose` names three
  interposer digests.
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
