# Progress

This page gives the state of the record and the only work order
(`AGENTS.md`, section 11). [INDEX](INDEX.md) lists the entries.
[RULES](RULES.md) gives the procedure. [RESUME](RESUME.md) gives the cold
start.

## State

653 entries: 23 open, 46 partial, 5 blocked, 579 done.

The state on 2026-11-23 (`docs/HANDOFF.md` has the detail):

* The newest release is `v0.2.0-beta.38` (2026-11-23, release run
  45212007696 on `2ae4b41`): podcrates `v0.81.0` and `v0.82.0` (T-2066:
  the ACLs of a layer, T-2065; 126 for E2BIG, T-2062; the bounded monitor
  connect, T-2051), the size ceiling of 9 500 000 bytes (call C29), a load
  of 85 layers (T-2068), the fixes of the sixth outside pass (T-2052 to
  T-2055, T-2060 to T-2064) and of the seventh (T-2070: T-2071 to T-2075).
  Its gate: the gate 45181778611, `arm.yml` 45181795967 and the nightly
  trial 45181788743 on `2ae4b41`, `workloads.yml` 45167359347, the
  integration tests as uid 1000 (36 test binaries), the seventh outside
  pass. Before it:
* `v0.2.0-beta.37` (2026-11-21, release run
  44846783317 on `c92ac65`): systemd as PID 1 on each route with
  `--podvm-init=image` (T-2002), the saved state of a machine guest
  (T-2008, T-2048), the faults of the sixth outside pass (T-2040 to
  T-2045), `logs` streams and takes `--limit-bytes` (T-2046), podcrates
  `v0.79.0`. Before it:
* `v0.2.0-beta.36` (2026-11-19, release run
  44466195422 on `94045a3`): systemd as PID 1 on the incus tier (T-1993),
  Go on the userland rung (T-1995), `apt-get install -u 0` on the userland
  rung (T-2000), the retry of `openat2` (T-1990), the image on ghcr.io
  (T-1992), podcrates `v0.74.0`. Before it:
* `v0.2.0-beta.35` (2026-11-18, release run
  44406181198 on `9e1e2e7`): the confine layer on the syscall tier
  (T-1982, T-1983), Fedora 42 (T-1981, T-1986), AlmaLinux 9's `ls -l`
  (T-1987), `stat` of a volume on the tier (T-1988), podcrates `v0.72.0`;
  call C25 names two limits of Landlock (T-1984, T-1985). Before it:
* `v0.2.0-beta.34` (2026-11-18, release run
  44359818961 on `54d17eb`): the syscall tier for podrunner's jobs (T-1976,
  T-1977, T-1978), a link in a volume on the loader family (T-1979), the
  OOM kills of a machine guest (T-1969 partial), `exec` after `--tmpfs`
  (T-1974). Its known gaps name T-1980 and T-1981. Before it:
* `v0.2.0-beta.33` (2026-11-18, release run
  44324846613 on `2bec174`): `--env-file0` (T-1971), the volume base and
  the Unix sockets of the syscall tier (T-1972 partial, T-1973), links
  across volumes (T-1975), the size and the usage of an incus instance
  (T-1964 partial, T-1965, T-1969 partial), the faults of the second
  outside pass (T-1974 partial), the exec of the syscall tier on a stack
  of its own (T-1976 partial) and podcrates `v0.71.0`. Before it:
* `v0.2.0-beta.32` (2026-11-18, release run
  44278551133 on `566f833`, the first behind the beta gate of T-1949): the
  asks of podrunner (T-1934, T-1945, T-1966, T-1967, T-1969 partial), the P1
  faults of the outside pass (T-1970) and podcrates `v0.70.0`. Before it:
* `v0.2.0-beta.31` (2026-11-17, release run
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
* podvm takes the crates of PodUtils/podcrates at `v0.81.0`
  (PodUtils/podcrates#254, COORD-151 of podcrates: the bounded connect of
  pod-guest, the streamed log of pod-supervise, `pod_core::files::replace`;
  before it `v0.80.0`, PodUtils/podcrates#250, COORD-150; before it
  `v0.79.0`, PodUtils/podcrates#245, COORD-149 of podcrates: the signals and the
  RUNPATH of pod-enter; before it `v0.78.1`,
  PodUtils/podcrates#241, COORD-146 of podcrates, which holds `v0.77.0`
  and `v0.78.0`, PodUtils/podcrates#240; before it `v0.76.0`,
  PodUtils/podcrates#239, COORD-205; `v0.75.0`,
  PodUtils/podcrates#238, COORD-204; `v0.74.0`,
  PodUtils/podcrates#237, COORD-201; `v0.73.0`,
  PodUtils/podcrates#231, COORD-198; `v0.72.0`,
  PodUtils/podcrates#229; `v0.71.0`, PodUtils/podcrates#228,
  COORD-191, #587; `v0.70.0`,
  PodUtils/podcrates#222; `v0.68.0`, PodUtils/podcrates#219, COORD-172: the source fixup reads no comment,
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

00. podrunner first (the owner's ruling, 2026-11-18, `docs/decisions.md`;
   PodUtils/podcrates#225): after a red run of podvm's CI, each ask of
   podrunner comes first (PodUtils/podrunner#277, #216, #189, #168, #110,
   and each new COORD issue `(to podvm)` of podrunner: #296 is T-1964 to
   T-1968, in the order the T-1964 remainder (a pool with a quota), then
   T-1968); #313 and #316 (the hosted snapshot)
   are T-1996, T-1999 (it waits for the xattrs of `pod-extract`,
   podrunner's COORD-107 to podcrates), the steps of T-1994, and
   T-1998; then the asks of
   githome and cfb for their CI, then the items below. A quality item
   that a podrunner unit meets counts as an ask of podrunner.
0a. The gate of the next beta (2026-11-23): the findings of the seventh
   outside pass that stay open, [T-2070](cli.md) (its P1 findings are
   done); the P2 and P3 findings of the sixth outside pass,
   [T-2044](cli.md).
0. Quality first (the owner's ruling, 2026-11-17; `docs/methodology/quality.md`):
   each beta passes the gate of `docs/maintenance.md`, "Releases". Verify and
   work the reports of the external
   QA (T-1937 to T-1944, T-1951 to T-1954), each by its class, with T-1947 (fail
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
