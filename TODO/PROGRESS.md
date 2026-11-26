# Progress

This page gives the state of the record and the only work order
(`AGENTS.md`, section 11). [INDEX](INDEX.md) lists the entries.
[RULES](RULES.md) gives the procedure. [RESUME](RESUME.md) gives the cold
start.

## State

671 entries: 22 open, 40 partial, 6 blocked, 603 done.

* The newest release is `v0.2.0-beta.41` (2026-11-26). [CHANGELOG](../CHANGELOG.md)
  holds what each release ships, with its known gaps.
* Open first: the asks of podrunner (`docs/HANDOFF.md`, "Next steps").
* `v0.2.0-beta.41` carries the saved-state channel and process limits
  (T-2094, experiment 473 under TCG; PodUtils/podrunner#355) and the live
  memory, CPU and process update of an incus instance (T-2093,
  experiment 479 on a hosted runner; PodUtils/podrunner#354). COORD-257
  (#676) asks podrunner to take the beta.
* podvm takes the crates of PodUtils/podcrates at `v0.85.0`. A change of
  moved code goes to podcrates as a COORD issue.
* KVM is free (the owner, 2026-11-09). Probe `/dev/kvm` on the current host.
  `kvm.yml` (T-1682) gives a hosted runner with it, and `arm.yml` a hosted
  arm64 runner (T-1721).
* A push starts the gate, `secrets` and `issue-sync`. The session
  dispatches other workflows directly. A newer push cancels the
  gate of an older commit. The issue `[gh action] requested` is the
  fallback only (`docs/decisions.md`, 2026-11-13, "Workflow fallback").

Each partial entry names its remaining clauses. Each blocked entry names
its blocker and the condition that clears it.

## Work order

The goal is a complete release for the beta testers (the owner's ruling
"Complete release first", 2026-11-13). A tool that the cloud host lacks
is installed, not a blocker ("Missing tools", the same day). Take the
first entry that is not done. Read its Prove before you start. Check
each claim that an entry waits against the code and the crates. Set its
status with `podvm-count` in the same commit as the work.

1. Asks of peer sessions come first (the watcher of `AGENTS.md`, section
   9; the owner's ruling "podrunner first", 2026-11-18,
   `docs/decisions.md`): each new COORD issue `(to podvm)` of podrunner,
   then PodUtils/podrunner#277, #216, #189, #168, #110, #296 (the
   sized writable disk in [T-1968](machine.md), which needs the
   disk-backed guest writable layer and the pod-enter mount in #667), and #313
   and #316 (the hosted snapshot: [T-1996](machine.md), T-1999, which
   needs the machine-tier xattr proof, [T-1994](machine.md) and
   [T-1998](machine.md)). Then the asks of githome and cfb for their CI,
   then the items below. A quality item that a podrunner unit meets counts
   as an ask of podrunner.
2. The open findings of the outside passes: [T-2079](cli.md),
   [T-2070](cli.md), [T-2054](enter.md), [T-2044](cli.md).
3. Quality first (the owner's ruling, 2026-11-17;
   `docs/methodology/quality.md`): each beta passes the gate of
   `docs/maintenance.md`, "Releases". Work the open entries of the external
   QA of `0.2.0-beta.31` (T-2088, T-1938 to T-1941 and T-1952 to T-1954)
   by their class, with T-1947 (fail safe at the edges). The harness of the CLI
   (`tests/cli_contract.rs`) guards the surface.
4. [T-1911](probe.md): `-u UID` in the user namespace (the range is opt-in
   until each verb keeps the store rule).
5. The Windows guest plan (`podvm run windows:NAME`; the owner accepts the
   vendor terms, `docs/decisions.md`, "Windows media"). Take the P1 entry
   [T-1861](machine.md) first, then the open P2 entries T-1869, T-1870,
   T-1872, T-1873, T-1874 and T-1877, then T-1898 and T-1900 (P3). No
   entry waits for podcrates, except the warm start of T-1874
   (podcrates@T-596).
6. The scope entries that wait for a peer: [T-1662](net.md) (partial:
   `podvm lab scenarios` and `plan` run; the lab guest, its probes and the
   judge are podcrates@T-345 to podcrates@T-347), [T-1667](cli.md)
   (partial: its check of labs comes with T-1662), [T-1663](net.md) and
   [T-1664](net.md) (blocked: they run in the lab guest of T-1662).
7. [T-1740](enter.md) (partial: a cheaper syscall tier) and
   [T-1350](gate.md) (partial: the wrapper of the parked Windows lane).
8. Low priority: [T-1749](enter.md) (partial: podvm wires the overlay root
   of podcrates@T-486 with the completion of podcrates@T-567).

## History

The layers of this page before 2026-11-10 (the CI remainder of
2026-10-01, the host failure of 2026-09-30, the publication of beta.10 to
beta.12, the reconciliation, the standing decisions and the session tables
of 2026-10-02) are in
[progress-before-2026-11-10.txt](../docs/history/progress-before-2026-11-10.txt).
The per-release state of beta.31 to beta.39 is in
[progress-state-before-2026-11-24.md](../docs/history/progress-state-before-2026-11-24.md).
