# The state section of TODO/PROGRESS.md before 2026-11-24

Verbatim: the per-release narrative of beta.31 to beta.39 and the take of podcrates tags, removed from the live page on 2026-11-24 (`CHANGELOG.md` holds what each release shipped).

The state on 2026-11-24 (`docs/HANDOFF.md` has the detail):

* `main` after `v0.2.0-beta.39`, for the next beta: podcrates `v0.82.1`
  (T-2084: a non-dumpable init and join relay, `pod_hostprobe::usage`),
  the readiness of an init on the incus tier (T-2083, podrunner#349), an
  import of a large image with one tree and a gzip upload (T-2085,
  podrunner#347), the record of the stop on a host with AppArmor and
  cgroup v2 (T-2086, podrunner#350; #660, COORD-244).
* The newest release is `v0.2.0-beta.39` (2026-11-24, release run
  45353882402 on `a43c9476`): the incus tier for podrunner (the use after
  the end, T-2076; the exit code under `--memory`, T-2077; a full pool,
  T-2078; exit 126 and 127, signals, one import, a cut upload, T-2080 to
  T-2082), the rule of `cp` and the faults of the eighth outside pass
  (T-2079). Its gate: the gate 45349587998, `arm.yml` 45349602183 and the
  nightly trial 45349597748 on `a43c9476`, `workloads.yml` 45324901359,
  the integration tests as uid 1000 (36 test binaries), the eighth
  outside pass. Before it:
* `v0.2.0-beta.38` (2026-11-23, release run
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
