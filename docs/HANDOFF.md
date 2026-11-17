# Handoff

The state on 2026-11-17, 18:00 (UTC), during a session. `main` is at the commit that adds
this file. `0.2.0-beta.31` is the last release (release run 44069288805
on `a3370f59`). A fresh session with no memory reads this file first,
then `AGENTS.md`, `TODO/PROGRESS.md` and `docs/decisions.md`. Remove this
file when each step below is done.

## Where things are

* The repository is PodUtils/podvm on `main`. No work lives in a local
  worktree, a stash or a branch. The branch `archive/podbox-worktree` is
  an archive; never merge or delete it.
* **A cloud session can start workflow runs again.** A push to `main`
  starts the gate, `secrets` and `issue-sync`; `actions_run_trigger`
  dispatches `kvm.yml`, `arm.yml`, `nightly.yml` and `release.yml`. A
  newer push cancels the gate of an older commit. The issue `[gh action]
  requested` is only the fallback.
* `issue-sync.yml` runs on each push and writes the state of each issue
  from `TODO/issues.md` and `TODO/github.txt`. A wrong row reopens or
  closes an issue on GitHub, so keep the rows right in the same commit.
* The pkgforge mirrors (the owner's ruling "Web access") answer 403
  through the egress proxy of a podvm cloud session; use the GitHub MCP
  tools then.
* podcrates is taken up to `v0.68.0` (PodUtils/podcrates#219, COORD-172).
  `v0.67.0` (PodUtils/podcrates#218, COORD-169).
  `v0.66.0` (PodUtils/podcrates#216): the PID
  namespace of `run -d` is on (T-1930 done). Before it `v0.64.0`
  (PodUtils/podcrates#214, COORD-166, #459). Before it `v0.63.0` (PodUtils/podcrates#211). Since
  `v0.62.0` (PodUtils/podcrates#210): its
  `chroot_usable` is fixed (COORD-150) and `Plan::userns_ids` serves
  T-1927 (COORD-154). The way out of a network lab (T-1924), the
  installer core (T-1925) and `pod-qga` (T-1868) are taken; the served
  ports of a lab (`Options::serve`, with no entry of podvm) are not.
* The record is in the format of `pod-todo`. `podvm-count` writes the
  rows and the counts of `TODO/INDEX.md`; edit the count line of
  `TODO/PROGRESS.md` by hand to the numbers that it prints.
* Integration tests run as uid 1000: `setpriv --reuid 1000 --regid 1000
  --clear-groups env HOME=$T TMPDIR=$T PATH=$PATH
  CARGO_MANIFEST_DIR=$PWD/crates/podvm TESTBIN`, with `T` a new directory
  under `/var/tmp` with mode 777. Build the interposer objects first
  (`scripts/build-interpose.sh`), or each loader-family test fails. Three
  failures come from the host, not the code: `report_path_is_the_parsers`
  needs a writable working directory, and `podvm_buildstate` and
  `package_ssh` cannot run root's `rustc` and `cargo` as uid 1000.
* `public.ecr.aws` answered 429 to the cloud host on 2026-11-16, so
  experiment 356 runs only in `arm.yml`.

## Done in this session (2026-11-16, the second session)

* T-1917 partial: `nightly.yml` writes `podvm.1` and `podvm-llms.txt`
  from the release binary, lints the page with `mandoc -Tlint -Wwarning`,
  and signs and attaches both; `release.yml` checks them. The nightly
  trial 43896360464 on `8ade508` passed. Remaining: a release that carries
  them, and the writer, the facts and the checks of `pod-manual`.
* T-1924 done: `podvm lab run --way-out`. The stand-in proxy of a lab died
  of SIGPIPE in podvm (a fix in `lab/host.rs`). COORD-162 (#455) asks
  podcrates for the roots of the host in the certificate file of a lab
  with a way out, and for a stand-in that protects itself from SIGPIPE.
  The Prove parser of the gate stops at a bare `--`.
* T-1925 partial: the installer is on the core of podcrates `v0.63.0`;
  the publish job signs `SHA256SUMS` (`SHA256SUMS.sigstore.json`) on the
  tag ref only. Remaining: the first release run proves it.
* T-1867 partial: a writable log stick keeps the logs of WinPE
  (`PELOGS\` on `pelogs.img`). `kvm.yml` run 43898543916 (`only=install`,
  on `c31b708`) runs it; read `install-shots/winpe/PELOGS/` in its
  artifact.
* T-1868 partial: `exec`, `cp`, a graceful `stop` and the readiness probe
  of a running Windows machine on `pod-qga`, and virtio-win 0.1.302 in the
  install chain. COORD-163 (#456) asks podcrates to list
  `fedorapeople.org`. The install now has two more devices (a CD and the
  agent channel); if the next T-1867 run differs, suspect them.
* T-1911 partial: `-u` drops for real again with the range (T-1927 had
  nested a one-line namespace); `cp` and `exec` of a stopped tree of a
  subordinate id run again in a namespace with the range. Remaining:
  `start` (see the entry), then the default.
* T-1786 done (issue 330 closed): a run that `PODVM_MODE` forces holds its
  record through `spawn_ladder`; COORD-122 (#384) closed. T-1929 (new,
  P3): the forced `rundir` and `tmpfs` rungs refuse the device nodes of
  the fixups.
* The host of a cloud session lacks `uidmap`: install it before a T-1911
  measurement (`apt-get install -y uidmap`).

* **The Windows install works (T-1867).** `windows:2025` installs under
  KVM in about 9 min and `podvm run windows:2025 ver` answers (run
  44037861805). The causes, in order: the root disk in the tmpfs of
  `$XDG_RUNTIME_DIR` (1.5 GiB), `-no-reboot` of pod-guest, and the
  password of the built-in Administrator in the OOBE of Windows Server.
* T-1930 done, released in `0.2.0-beta.30`: `run -d` has a PID namespace;
  podrunner pins beta.30 and closed PodUtils/podrunner#200.
* podcrates taken up to `v0.67.0`.
* T-1931 (partial): podrunner#202, the `HEAD` of the source fixup;
  `--no-source-fixup` stops it; COORD-170 is open, COORD-171 is closed.

## Done in this session (2026-11-17, the afternoon)

* T-1933: a foreground `run windows:NAME` of a golden image goes through
  the guest agent of a short-lived machine (`windows/machine/once.rs`).
  The first `kvm.yml` run (44097296278) waited 900 s on both legs: the
  probe's client held the one client of the QGA channel; fixed in
  `89edd7f`, and run 44104748355 tests it. COORD-175 (#468) asks podcrates
  for evidence of a mailbox timeout.
* T-1934 (blocked): no pty in a unit on the namespace rung; COORD-176
  (#469) asks podcrates for a devpts instance and `/dev/ptmx`.
* T-1935 (partial, P1): `run -d --podvm-tier=syscall` for podrunner's
  hosted runners (raw `renameat2` of `uv` and `ruff`), with `exec`,
  `inspect` `Tier` and `:ro`; two crashes of the tier fixed
  (`posix_spawn`, `close_range`). COORD-177 (#470) answers podrunner.
* T-1936 (partial, P1): experiment 464 and `workloads.yml`, the tools of
  a CI job in a unit; COORD-178 (#471) answers podnet's COORD-102.
* To force the userland rung on the cloud host (no sysctl there), run
  podvm under a seccomp filter that denies `unshare` and
  `clone(CLONE_NEWUSER)`: `scripts/dev/nouserns.c`.

## Quality first (2026-11-17, the evening): read this before any work

* An external tester confirmed 95 faults of `0.2.0-beta.31` (issues 472
  to 566). The owner's ruling "Quality first" and the plan are
  `docs/methodology/quality.md` and `AGENTS.md` 2a and 2b. **No beta until
  T-1949 holds.**
* Each report is verified, labelled `confirmed` (564 closed by design),
  and mapped by class: T-1937 to T-1945 (the first two batches), T-1951
  (output contract), T-1952 (images and the store), T-1953 (signals and
  rungs), T-1954 (machine tier), T-1948 (true messages). The faults in
  crates of podcrates are COORD-181 (#568) and COORD-182 (#569); podcrates
  fixed #568 on its `main`.
* Done in this pass: the CLI contract test (T-1946, `tests/cli_contract.rs`),
  the panic hook (T-1947), the store races (T-1938), the syscall tier for
  `run -d` (T-1935, for podrunner's hosted runners), `exec --env-file`
  (T-1945), the Windows foreground run on the guest agent (T-1933 done),
  and about 30 of the reports. `scripts/dev/checks.sh` is the one command
  before each push.
* COORD-180 (#567, to all) says what went wrong. COORD-183 (#570) answers
  podcrates' COORD-131 (the copies of shared code, T-1955).

## Next steps, in order

1. Start the session (`AGENTS.md`, section 8), with the watcher.
2. A new ask of a peer first. T-1931 is done (COORD-170 and COORD-171
   closed); #439 (podrunner) and the takes
   #445, #446, #449, #453 (podcrates).
3. T-1867 is done: `windows:2025` (run 44037861805) and `windows:11`
   (run 44041116169) install under KVM and answer `ver`. A beta can
   carry it.
4. T-1867 and T-1868 are in `0.2.0-beta.31` (`exec`, `cp` and a
   graceful `stop` of a Windows machine, run 44059854830). T-1933 (P1):
   on 2 CPUs the foreground `run windows:NAME` through the mailbox fails
   at 600 s in 3 of 4 runs, while a detached machine passes on the same
   image. Now a foreground run of a golden image goes through the guest
   agent of a machine (`windows/machine/once.rs`); a `kvm.yml` run of
   experiment 452 proves it. COORD-175 (#468) asks podcrates for evidence
   of the mailbox timeout.
5. Then the work order of `TODO/PROGRESS.md` (T-1911 `start`, T-1929,
   T-1872, the Windows entries, T-1917 and T-1925 remainders).

## The state at the end of the session

* `main` is pushed; read the last gate run before work.
* The open issues of podvm that wait for a peer: #439 (podrunner), #445,
  #446, #449, #453, #455, #456 (podcrates).

## Rules learned

* A tool of the record (`podvm-gate`, `podvm-count`) finds the repository
  from its own path: in a worktree, build it in the worktree's own
  `target/`.
* Before an entry is set done, run its proof and the uid-1000 suite.
* Before a release, run `arm.yml` too: experiment 356 found T-1923, which
  the gate and the nightly trial do not cover.
* The sudo of a GitHub runner keeps `XDG_CONFIG_HOME` of the runner's
  user; an experiment that makes a second user removes each `XDG_*`.
* The disk is small: remove `target/debug` and stale `deps` when it fills,
  and the test directories under `/var/tmp`.
* A release takes the head of `main`: hold each push from the version bump
  until the release run ends, and move a change that must ship into the
  bump's section of the changelog with a new gate, `arm.yml` and nightly
  trial on its commit.
* When a test moves to another file, move the case of `podvm-plant` that
  pins it too (`crates/podvm-gate/src/plant/pins.rs` and the list in
  `plant.rs`), and run `podvm-plant` (no `--help`: any argument starts the
  full run, and a cut run can leave a plant in the tree).
