# Progress

This page gives the state of the record and the only work order
(`AGENTS.md`, section 11). [INDEX](INDEX.md) lists the entries.
[RULES](RULES.md) gives the procedure. [RESUME](RESUME.md) gives the cold
start.

## State

552 entries: 10 open, 25 partial, 4 blocked, 513 done.

The state on 2026-11-16 (`docs/HANDOFF.md` has the detail):

* The newest release is `v0.2.0-beta.31` (2026-11-17, release run
  44069288805 on `a3370f59`; the gate 44066665515, `arm.yml` 44066666416
  and the nightly trial 44066671601 passed first): T-1867 (`windows:2025`
  and `windows:11` install), T-1868 (`exec`, `cp` and `stop` of a Windows
  machine), T-1931, podcrates `v0.67.0` and `v0.68.0`. Before it, beta.30
  (T-1930). `release.yml` needs a successful gate run
  on its commit; dispatch `nightly.yml` with `trial=true` and `arm.yml` on
  `main` first, and push nothing to `main` until the release run ends
  (call C22).
* GitHub Actions works again for a cloud session (2026-11-16): a push
  starts the gate, `secrets` and `issue-sync`, and `actions_run_trigger`
  dispatches each workflow. A newer push cancels the gate of an older
  commit. The issue `[gh action] requested` is the fallback only
  (`docs/decisions.md`, 2026-11-13, "Workflow fallback").
* podvm takes the crates of PodUtils/podcrates at `v0.68.0`
  (PodUtils/podcrates#219, COORD-172: the source fixup reads no comment,
  T-1931; `v0.67.0`, PodUtils/podcrates#218, COORD-169; `v0.66.0`, PodUtils/podcrates#216:
  the PID namespace of `run -d`, T-1930;
  `v0.65.0`, PodUtils/podcrates#215; before it `v0.64.0`, PodUtils/podcrates#214, COORD-166: the roots of the host in a lab with a
  way out, the stand-in that ignores SIGPIPE, `fedorapeople.org`; before it
  `v0.63.0`, PodUtils/podcrates#211: the nested user namespace of a foreground run,
  T-1927; before it `v0.62.0`, PodUtils/podcrates#210, and `v0.61.0`): `Plan::volumes` of pod-enter, the row
  `chroot(/tmp) in unshare(NEWUSER+maps)` of pod-hostprobe, and its
  `chroot_usable` fixed for COORD-150 (#443, T-1921). The way out of a
  network lab is taken (T-1924, done; COORD-162, #455, asks podcrates for
  the roots of the host in the certificate file of the lab). The
  installer core is taken at `v0.63.0` (T-1925, partial: a release must
  sign `SHA256SUMS`).
  A change of moved code goes to podcrates as a COORD issue.
* KVM is free (the owner, 2026-11-09). The cloud host has no `/dev/kvm`;
  `kvm.yml` (T-1682) gives a hosted runner with it, and `arm.yml` a
  hosted arm64 runner (T-1721).
* PodUtils/podrunner takes the betas for its kinds. It closed each of its
  asks to podvm (PodUtils/podrunner#85, #87, #88, #90); podvm closed its
  answers (issues 303, 307, 311, 316, 319) on 2026-11-13.

Each partial entry names its remaining clauses. Each blocked entry names
its blocker and the condition that clears it.

## Work order

The goal is a complete release for the beta testers (the owner's ruling
"Complete release first", 2026-11-13). A tool that the cloud host lacks
is installed, not a blocker ("Missing tools", the same day). Take the
first entry that is not done. Read its Prove before you start. Check
each claim that an entry waits against the code and the crates. Set its
status with `podvm-count` in the same commit as the work.

0. Quality first (the owner's ruling, 2026-11-17; `docs/methodology/quality.md`):
   no beta until T-1949 holds. Verify and work the reports of the external
   QA (T-1937 to T-1945, T-1951 to T-1954), each by its class, with T-1947 (fail
   safe at the edges) and T-1948 (messages from the entered mechanism).
   The harness of the CLI (`tests/cli_contract.rs`) guards the surface.
1. A new ask of a peer session comes first (the watcher of `AGENTS.md`,
   section 9). The QA pass of
   experiment 443 is worked, and each of its entries is done. The copy
   of the rootfs on each run is T-1749: since 2026-11-14 a spare tree
   takes it off each run after the first. The crates that podvm takes
   carry the overlay root of the namespace rung (podcrates@T-486,
   `v0.42.0`) and the completion of two layers (podcrates@T-567,
   `v0.43.0`); podvm wires them next.
1b. The live run of 2026-11-15 (issues 396 to 402, PodUtils/podrunner#115):
   [T-1911](probe.md) (`-u UID` in the user namespace; the range is
   opt-in until each verb keeps the store rule).
2. The external QA report of `v0.2.0-beta.14` (2026-11-14): the open
   entries that it gave. Each P0 entry is done. Take the P1
   entries, each with the guard of its class (the guard entries of
   `TODO/gate.md`), then P2 and P3.
2a. The Windows guest plan of the review of 2026-11-14: T-1861 to T-1877 and T-1898 to T-1900
   (`podvm run windows:NAME`). The owner accepts the vendor terms
   (`docs/decisions.md`, "Windows media"). Take the P1 entries first:
   [T-1861](machine.md) (the install of a setup ISO is done: `windows:2025`
   and `windows:11` install under KVM); then the P2 entries and T-1898 to T-1900 (P3). podcrates
   `v0.45.0` carries the parts of the crates that they asked for
   (podcrates@T-590 to podcrates@T-595, podcrates@T-610; issues 350 and
   360), and podvm takes that tag: no entry of the plan waits for
   podcrates, except the warm start of T-1874 (podcrates@T-596, open).
3. The scope entries that wait for a peer: [T-1662](net.md) (partial:
   `podvm lab scenarios` and `plan` run; the lab guest, its probes and the
   judge are podcrates@T-345 to podcrates@T-347), [T-1667](cli.md)
   (partial: its check of labs comes with T-1662), [T-1663](net.md) and
   [T-1664](net.md) (blocked: they run in the lab guest of T-1662).
   podcrates names them after podcrates@T-486 and @T-107, which `v0.42.0`
   carries (PodUtils/podcrates#142).
4. [T-1740](enter.md) (partial: a cheaper syscall tier) and
   [T-1350](gate.md) (partial: the wrapper of the parked Windows lane).
5. Low priority, the asks of podswarm (PodUtils/podcrates#124):
   [T-1749](enter.md) (partial: podvm wires the overlay root of
   podcrates@T-486 with the completion of podcrates@T-567).
   V-1, V-2 (the probe cache of `run`), V-4, V-7 (`--podvm-sealed`), V-8
   and the warm guest of the machine tier are done.

The entries done since the resume of 2026-11-11 are in the record; the
handoff (`docs/HANDOFF.md`) names them.

## History

The layers of this page before 2026-11-10 (the CI remainder of
2026-10-01, the host failure of 2026-09-30, the publication of beta.10 to
beta.12, the reconciliation, the standing decisions and the session tables
of 2026-10-02) are in
[progress-before-2026-11-10.txt](../docs/history/progress-before-2026-11-10.txt).
