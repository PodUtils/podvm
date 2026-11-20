# podvm: the hard, blocked and "impossible" problems

Written 2026-11-20 from the repository state at commit `1f578d49` (branch `main`)
and the open GitHub issues of PodUtils/podvm read the same day.

This report stands alone. The reader needs the repository (`PodUtils/podvm`) and
the web. Nothing here depends on any earlier conversation.

How to read the citations:

* `T-NNNN` is an entry of the work record in `TODO/*.md`. Find it with
  `grep -n "^# T-NNNN" TODO/*.md`. Line numbers are given as `file:line` of the
  heading. Other people edit these files, so the numbers drift. The ID does not.
* `#NNN` is a GitHub issue of PodUtils/podvm, unless a repository is named.
* `COORD-NNN` is a message between the sessions of the PodUtils projects. It is
  a GitHub issue titled `COORD-NNN: ... (to X)`.
* "-" means the value is unknown or was never measured.
* "(estimate)" marks a number that nobody measured.
* A measured number names its host. "The cloud host" means the host of podvm's
  own sessions: Linux 6.18.44, x86_64, 4 CPUs, no `/dev/kvm`, no delegated
  cgroup2 subtree, cgroup v1 with cgroup2 at `/sys/fs/cgroup/unified`, an
  HTTPS egress proxy with an allowlist, no Yama
  (`docs/findings.md`, host condition C1; `TODO/enter.md:5183`).

---

## 1. Orientation

### 1.1 What podvm is

podvm runs Linux container images with a docker/podman-like command line on
hosts that deny what a container engine needs: user namespaces, mounts,
`chroot`, device creation, ownership changes, `ptrace`, root. It probes the
host, picks the strongest mechanism the host allows, and reports the mechanism
it entered (`AGENTS.md` section 1; `docs/architecture.md`). The owner's rule:
"everything must be statically compiled" (`AGENTS.md` section 2). Each release
is a static musl binary. This matters for every plan that carries a kernel or a
helper.

The owner widened the scope on 2026-11-06 to "everything sand-net does, but
without root". sand-net was a network and sandbox lab tool whose source is
gone. Its lessons live in the `scope` issues #178 to #193.

podvm is one of the PodUtils projects. Shared code lives in PodUtils/podcrates
(crates named `pod-*`). A rule of the owner (`AGENTS.md` section 2a) says:
search podcrates first, and ask podcrates for a capability instead of writing a
second copy. So many hard problems are "blocked on podcrates". PodUtils/podrunner
runs GitHub Actions jobs in podvm and is the main consumer that sets priorities.

### 1.2 The rungs and tiers

"Rung" is a step of the ladder that podvm selects from by probe. "Tier" is a
mechanism that the user can force with `--podvm-tier=NAME`.

| Name | What it is | Needs from the host | Source of fact |
| --- | --- | --- | --- |
| namespace rung | mount namespace, tmpfs, rootfs entry; for a non-root caller also a user, PID and UTS namespace (uid 0 of a map) | user namespaces | `docs/architecture.md` "Selection and entry"; `docs/limits.md:312` |
| supervise rung | seccomp user notification mediation | listener, `process_vm_readv`, `ptrace` | T-0606 `TODO/supervise.md:384` |
| chroot rung | `chroot` into the extracted tree. No isolation | `chroot` | `docs/limits.md` "Host and payload" |
| userland rung | image's own dynamic loader plus an `LD_PRELOAD` interposer that rewrites paths of libc calls ("loader family"); a static program goes to the native-static rung | nothing | `crates/podvm-interpose/`; `docs/limits.md` |
| native-static rung / syscall tier (`--podvm-tier=syscall`) | Syscall User Dispatch (SUD, Linux 5.11): each system call becomes SIGSYS, handled in podvm code in the payload's address space; paths resolve with `openat2(RESOLVE_IN_ROOT)` | the SUD `prctl` | T-1652 `TODO/enter.md:1953`; `crates/podvm-dispatch/src/handler.rs` |
| emulate tier | static qemu-user on the native-static rung | `qemu-ARCH-static` | T-1651 |
| runsc tier | gVisor `runsc`, rootless | `runsc` binary, user namespaces | T-1729 `TODO/enter.md:3076` |
| machine tier | QEMU guest (KVM if `/dev/kvm`, else TCG), podvm is PID 1 of the guest | `qemu-system-*` (a pinned static QEMU 9.2.0 is fetched) | T-1656; `docs/limits.md` "Guests" |
| incus tier | an instance of an `incusd` that answers on a socket ("route A": incusd as root of the user's own user namespace) | a running `incusd` | `docs/incus.md`; T-1718 |
| Windows lane | `podvm run windows:NAME` under QEMU | QEMU, media | T-1861 and the T-18xx entries |

The userland rung and the syscall tier are "compatibility layers, not a
boundary" (`docs/limits.md:59` and `:88`). The handler shares the payload's
address space.

### 1.3 The host constraints that cause most limits

1. **No user namespace.** GitHub-hosted `ubuntu-24.04` runners restrict them by
   AppArmor (`TODO/enter.md:4431`, T-1935). Without one: one uid, no PID
   namespace, no bind mounts, no `mount`, no `chroot` (often).
2. **No second uid.** An unprivileged process may map only itself in a user
   namespace. More ids need the setuid helpers `newuidmap` and `newgidmap` plus a
   range in `/etc/subuid` and `/etc/subgid`. A host without them gives one id.
   `setresuid` to another uid fails (EPERM on the userland rung, EINVAL in a
   one-id namespace) (`TODO/enter.md:5183` T-2002; T-0711 `TODO/interpose.md:1186`).
3. **No delegated cgroup2 subtree.** An unprivileged caller can write
   `memory.max`, `cpu.max`, `pids.max` only inside a directory the host gave it.
   The cloud host gives none. Many container hosts have root-owned cgroups
   (`docs/limits.md:312`; T-1747 `TODO/cli.md:4032`).
4. **No `/dev/kvm`.** The cloud host runs guests under TCG (software). A guest
   is 3 to 21 times slower (T-1308, cited in `TODO/net.md:24`). `kvm.yml` and
   `arm.yml` give hosted runners that do have it (T-1682, T-1721).
5. **No `ptrace`, no `process_vm_readv`.** This kills the supervise rung on
   some hosts and removes tracer-based designs (T-0606 `TODO/supervise.md:384`).
6. **No root, no mounts, no device nodes, no loop devices, no FUSE.** `/dev/fuse`
   is mode 0600 root on the cloud host (`TODO/enter.md:5183`, premise 3).
7. **Egress through an HTTPS proxy with an allowlist.** A `cargo` in a payload
   cannot pass it (self-signed certificate). Several mirrors answer 403 or 429
   at times (`docs/HANDOFF.md`).

### 1.4 Where each kind of fact lives

| Kind of fact | Where |
| --- | --- |
| What each work item says, its status, proof, measured result | `TODO/*.md` (area files `enter`, `machine`, `net`, `cli`, `supervise`, `image`, `interpose`, `probe`, `gate`, `deps`, `complete`, `packaging`, `podssh`); `TODO/INDEX.md` lists them; `TODO/PROGRESS.md` is the state and work order |
| Current limits, in one place | `docs/limits.md` |
| Host traits and tool defects, with an `observed` or `inferred` mark | `docs/findings.md` |
| Rulings of the owner, calls made without a ruling (C1 to C26), questions | `docs/decisions.md` |
| Design of the rungs | `docs/architecture.md`, `docs/incus.md` |
| Newest handoff | `docs/HANDOFF.md` |
| Raw measurements | `experiments/NNN-*.sh` (scripts) and `experiments/results/*.txt` (transcripts) |
| Messages between sessions | GitHub issues `COORD-NNN` of PodUtils/podvm, podrunner, podcrates |
| Source | `crates/podvm` (CLI, `initns/`, `runsc/`, `machine/`, `incus/`, `userns/`), `crates/podvm-dispatch` (SUD handler), `crates/podvm-interpose` (`LD_PRELOAD` object, outside the Cargo workspace), `crates/podvm-windows` |

State of the record on 2026-11-19: 606 entries, 25 open, 37 partial, 6 blocked,
538 done (`TODO/PROGRESS.md:10`). The six blocked entries are T-1663, T-1664,
T-1929, T-2003, T-2005 and T-2009. CI is green on `1f578d49` (gate run
44644329858).

### 1.5 How "hard" is classified here

Most open work is ordinary: the CLI faults of outside passes (issues #472 to
#566, T-1937 to T-1954, T-1970, T-1974, T-1980, T-1989, T-2001) are small and
confirmed. This report skips them except where one hides a deeper limit. It
covers problems where the cause is a kernel rule, a host fact, or a missing
piece that another project must supply.

---

## 2. The hard problems, by theme

### Theme A. systemd as PID 1, and more than one uid, on hosts that deny namespaces

This is the newest and busiest area. The owner asked for a parity table with a
GitHub-hosted runner: an image whose services start with `systemctl start` and
run as their own users (mysql, postgres, www-data). The sources are
PodUtils/podrunner#313 and #316, and the owner's two research bundles of
2026-11-19 (not in the repository). Entries T-2002 to T-2010; experiments 465 to
468.

Measured routes for one image (Debian 12, MariaDB 10.11, PostgreSQL 15, systemd
252) on the cloud host as uid 1000 (`TODO/enter.md:5439` T-2010;
`experiments/results/init-bench.txt`):

| Route | Whole run | First output | Unit start | Restart MariaDB | Peak memory | Needs |
| --- | --- | --- | --- | --- | --- | --- |
| namespace rung | 5.4 s | 2.6 s | 6 ms | 0.15 to 0.17 s | 249 MiB | subuid range **and** a delegated cgroup2 directory (here a directory that root made as a stand-in) |
| runsc tier | 8.0 s | 3.7 s | 32 to 36 ms | 0.36 to 0.39 s | 925 to 978 MiB | `runsc`, user namespace |
| machine tier, TCG, 4 vCPU | 28.8 s | 16.3 s | 163 to 178 ms | 2.2 to 2.3 s | 3.7 GiB | QEMU only |
| UML seccomp mode (manual, not in the transcript) | 14.7 s | - | 89 to 133 ms | 1.8 to 1.9 s | - | nothing (see T-2005) |

The userland rung has no route of its own. Its only route today is the QEMU
guest. That is slow and big.

#### A1. T-2002: the init of the image on the namespace rung and the userland rung

* ID and state: T-2002 (`TODO/enter.md:5183`), P1, **partial**.
  Companions: T-2004 (partial), T-2006 (done), T-2007 (partial), T-2010 (partial).
* What fails. On the namespace rung a foreground `run` is uid 0 of a map with
  one uid. `systemctl is-system-running` gives `offline`. `setresuid` to another
  uid gives EINVAL. On the userland rung `setresuid` gives EPERM and no call
  makes a PID namespace (`TODO/enter.md:5183`, Problem).
* Measured evidence (cloud host, uid 1000, `TODO/enter.md:5183`, Premise 1 to 4):
  1. With the subordinate range (`newuidmap`) and a PID namespace, systemd as
     PID 1 stops at "Failed to create /init.scope control group: Permission
     denied". The cgroup2 of its cgroup namespace belongs to host root. With a
     cgroup2 directory owned by uid 1000, the same boot reaches
     `graphical.target`.
  2. systemd wants `/` and `/dev` to be mount points, wants a console that stays
     open (a pty with no open slave gives EIO and PID 1 waits 1 s at each status
     line: a boot of 54 s, 2 s with the slave held open), and refuses a
     devtmpfs mount in a user namespace.
  3. A tmpfs on `/sys/fs/cgroup` with the control files does not work.
     systemd 262 reads the file system type: "Found tmpfs on /sys/fs/cgroup/,
     assuming legacy hierarchy ... Detected unsupported legacy cgroup
     hierarchy, refusing execution". `/dev/fuse` is 0600 root. So no file
     system this caller can mount answers as cgroup2.
  4. A machine-tier guest boots under TCG as uid 1000 in 9 s.
* Why it is hard. Three kernel rules meet. (a) An unprivileged caller can map
  only itself without a setuid helper and a range. (b) Only a delegated cgroup
  directory lets an unprivileged process own a cgroup2 hierarchy. (c) systemd
  tests the file-system magic (`statfs` f_type) of `/sys/fs/cgroup`, so a fake
  directory does not pass.
* Tried:
  * Route 1, namespace rung from a delegated subtree. Works in clause 1 of
    experiment 465 (`experiments/results/init-image.txt`): state `running`,
    `uid_map 0 1000 1 / 1 100000 65536`, units run as uids 961, 962, 33, 65534.
    The delegated directory in the test was made by root: the real host case
    is not yet measured (T-2002 Remaining 1; wanted on `kvm.yml`).
  * Route 2, runsc tier (T-2007). Works.
  * Route 3, machine tier (T-1994 and T-2008 for speed).
  * "A PID 1 and a cgroup tree inside the interposer": the owner's second
    research bundle tried about 45 runs with `pid1.rs`, `cgroupfs.rs`,
    `fsmagic.rs`, `spawn.rs`, `rlimit_calls.rs`, `kmsg.rs`. None booted
    ("Can't run system mode unless PID 1", then RLIMIT_CORE). The code is not in
    this tree (`TODO/enter.md:5309`, premise 5).
  * Not routes, per T-2005 Approach 3: proot (the target filters `ptrace`),
    LKL (no bridge to processes), `systemd --user` (needs a delegated cgroup,
    no `User=`), cgroup v1 (same `statfs` check).
* Open leads:
  * A seccomp filter that gives ENOSYS for `clone3` makes systemd attach through
    `cgroup.procs` (`posix_spawn_wrapper` of systemd's `src/basic/process-util.c`),
    and only `statfs`/`fstatfs` of `/sys/fs/cgroup` need a false answer
    (T-2005 premise 6, a claim of its source, not measured). What stays wrong:
    `/proc/PID/cgroup` reads `/`, `cgroup.procs` keeps the last pid only,
    `cgroup.events` never reads `populated 0`.
  * The real delegated case: hosted runner and a job of `kvm.yml` (T-2002 Remaining 1).
* Questions for a researcher:
  1. Which systemd versions (252 to 262 and newer) check the `statfs` magic, and
     which paths read `cgroup.events` or `cgroup.procs`? Is any minimal fake that
     lets systemd reach `running` possible with no kernel support?
  2. Does a user manager of systemd give a usable delegated scope on GitHub-hosted
     runners (login shell in `session-N.scope`)?
  3. For `run -d`, `create` and `exec` with the init on the namespace rung
     (T-2002 Remaining 2), what is the cleanest way to keep a PID-1 supervisor
     alive across podvm exits?
* First experiment: `sh experiments/465-init-image.sh` as uid 1000 on a host
  with real delegation, for example a hosted `ubuntu-24.04` job through
  `kvm.yml` (job `cgroup`, experiment 459). Record whether clause 1 passes with
  no directory that root made.

#### A2. T-2004: a delegated scope from the user manager of systemd

* ID and state: T-2004 (`TODO/enter.md:5276`), P2, **partial**.
* What fails. On a host with systemd the login shell is in `session-N.scope`,
  owned by root. podvm finds no delegated directory and goes to the machine
  tier, although `user@UID.service` can give a delegated scope.
* Evidence: `experiments/results/init-image-scope.txt`. The host of the measure
  is a guest with systemd 261 under TCG (273 s). The user manager moved podvm
  into `app.slice/podvm-init-*.scope`; the init booted on the namespace rung
  (state `running`); the scope ended 24 s after the run. Code:
  `crates/podvm/src/initns/scope.rs:82` (`ask`), called from
  `crates/podvm/src/initns.rs:99` (`decide`).
* Why it is hard. The kernel's "no internal process" rule: a domain cgroup that
  holds a process cannot enable controllers for children. A fault found here:
  enabling `pids` in the scope while podvm itself sat in it gave "domain
  invalid" children and EOPNOTSUPP on the move. Fixed by enabling no controller
  in a directory with a process (`initns/cgroup.rs::enable`).
* Remaining (`TODO/enter.md:5276`): (1) a real login on a real systemd host;
  (2) the init in a scope gets no controller (memory, pids) because podvm is a
  process of the scope: move podvm into a leaf first; (3) a host with a user
  manager and no `busctl`.
* Questions: does `StartTransientUnit` with `Delegate=yes` work from a
  GitHub-hosted runner's `runner` user (user manager running or not)? Is
  `loginctl enable-linger` needed, and can an unprivileged user do it?
* First experiment: `sh experiments/466-init-image-scope.sh` on a hosted runner.

#### A3. T-2005 (blocked): an init with no guest, on a host with no cgroup delegation and no user namespace

* ID and state: T-2005 (`TODO/enter.md:5309`), P2, **blocked** on
  `podcrates@T-975` (PodUtils/podcrates#242, COORD-147; podvm's ask is COORD-206,
  #602).
* What fails. A host that denies user namespaces (the userland rung) or
  `ptrace` boots an init only in a QEMU guest. Under TCG on the cloud host a run
  with MariaDB and PostgreSQL takes 46 s, a unit start 250 ms, and the guest
  holds 2.6 GB (`TODO/enter.md:5309`).
* The candidate: **User-Mode Linux (UML) in its seccomp mode** (`seccomp=on`,
  Linux 6.16 or later). It needs no `ptrace`, no namespace, no KVM, no QEMU.
  Evidence, experiment 467 (`experiments/467-uml-seccomp.sh`,
  `experiments/results/uml-seccomp.txt`):
  * Stock Linux 7.3.5 for `ARCH=um`: 8,583,024 bytes stripped, built in 207 s,
    **linked to glibc (dynamic)**.
  * Under a launcher whose seccomp filter gives EPERM to `ptrace`, `unshare`,
    `setns`, `chroot`, `mount`, the kernel with `seccomp=off` ends at "ptrace:
    Operation not permitted" (clause 2). With `seccomp=on ncpus=4`, systemd 261
    is PID 1 and the check exits 7 in 15 s (clause 4).
  * A "builder UML" writes an ext4 base of 580 MiB in 27 s, keeping owners
    (`/usr/bin/mount` is 0:0 mode 4755). The run uses a 19 MiB copy-on-write
    file; the base keeps its SHA-256 (clause 3 and 5).
  * Debian 12 bench, manual: 14.7 s whole run with `ncpus=4`, 18.9 s with
    `ncpus=1`, against 45.6 s under TCG (table in T-2005 Done).
* Why it is hard. Four things.
  1. The rule "everything static": the measured kernel links to glibc. A static
     UML kernel build is not shown. A kernel build is shared code, so podcrates
     must own it (rule 2a). podcrates agreed (answer to COORD-206): x86_64 only,
     stock 7.3.5 needed no pocket_vm patch, the channel is the exit code plus
     stdout and stderr; volumes later.
  2. UML network and SMP console paths have known faults (pocket_vm's 8 patches
     on 7.2: an SMP fix in `chan_kern.c`, a deadlock of the vector driver). The
     measure used no network.
  3. UML is x86 only for this use (premise 1 to 4 of the entry).
  4. Block devices: UML `ubd` plus copy-on-write needs a base ext4 for each
     digest, built without root. The builder used hostfs, where owner ids are the
     host user's. Real-world ownership needs the builder trick.
* Tried: experiment 467 above. The second bundle of the owner got `ptrace:
  Operation not permitted` from a UML 6.12 (ptrace mode) and 48 s with MariaDB
  and PostgreSQL from a UML 6.13.12 ptrace mode on one CPU (T-2005 premise 3).
* Open leads: pocket_vm (github.com/flyinghorse0510/pocket_vm, Apache-2.0 or
  MIT) runs each guest with `seccomp=on` and `docker:dind` with cgroup2 as a
  user. UML SMP came in 6.19; 7.0 fixed SIGCHLD in seccomp mode; 7.1 added musl and
  a TLB fix (all claims of the entry's source, not measured here).
* Questions for a researcher:
  1. Can the UML kernel be built fully static (musl or glibc-static) and stay
     under the binary size ceiling of 9,000,000 bytes (call C24,
     `docs/decisions.md`) if it is embedded, or must it be fetched by digest?
  2. What are the seccomp-mode costs in the field, for example CPU-heavy
     compile workloads (errand-box measured UML or TCG as 18 to 53 times runc on
     compile work, `docs/incus.md:24`)? Does `seccomp=on` change that?
  3. Does UML reach GitHub-hosted runners (which block user namespaces)?
     `kvm.yml` measure: -.
  4. Does UML seccomp mode run on arm64? Entry says x86_64 only.
* First experiment: build a static `ARCH=um` kernel with a musl toolchain (zig
  cc), run experiment 467 clause 4 with it, and record size, boot time and
  check result.

#### A4. T-2003 (blocked): a library in `DT_RUNPATH` does not load on the userland rung

* ID and state: T-2003 (`TODO/enter.md:5243`), P2, **blocked** on
  `podcrates@T-976` (PodUtils/podcrates#243, COORD-148; ask COORD-207, #603).
* What fails. As uid 1000 with `PODVM_USERNS=0` and a volume, `systemctl` of
  Arch Linux fails: "error while loading shared libraries:
  libsystemd-shared-262.1-1.so: cannot open shared object file" (exit 127). The
  library is in `/usr/lib/systemd`, named in `DT_RUNPATH`. The same for
  `/sbin/init`, `journalctl`, `udevadm`, `loginctl`.
* Why. The loader of the image runs from its host path. An absolute directory in
  a `RUNPATH` names a host path there. `pod_enter::userland::host_env` only
  reads the loader configuration of the image, not `DT_RUNPATH` or `DT_RPATH`.
* Tried: nothing local; the fix is in pod-enter (podcrates).
* Open lead: pod-enter adds the `DT_RUNPATH` and `DT_RPATH` directories of the
  program and of each library, with `$ORIGIN`, in `ld.so` order.
* Questions: how does `ld.so` order `RUNPATH` against `LD_LIBRARY_PATH` for
  transitive libraries (RUNPATH does not apply to dependencies of dependencies,
  RPATH does)? What does the interposer do for a `dlopen` with `$ORIGIN`?
* First experiment: the Prove of the entry once podcrates has a tag:
  `run --rm -v DIR:/v ghcr.io/pkgforge-dev/archlinux:latest systemctl --version`
  as uid 1000, `PODVM_USERNS=0`, exits 0.

#### A5. T-2007 and T-2009: gVisor as the near-native route

* ID and state: T-2007 (`TODO/enter.md:5365`), P1, **partial**. T-2009
  (`TODO/enter.md:5406`), P2, **blocked** on `podcrates@T-977` (PodUtils/podcrates#243;
  ask COORD-208, #604).
* What works. `run --podvm-tier=runsc --podvm-init=image` boots systemd as PID 1
  in 7 to 10 s as uid 1000 (cloud host, gVisor `release-20261102.0`). Table in
  T-2007 Done: 10.2 s whole run, 3.6 s boot, 925 MiB peak.
* What fails or is missing.
  1. It needs a user namespace and, per the entry, `ptrace` for the threads of
     the `systrap` platform. So it does **not** serve the userland rung
     (T-2007 Remaining 5; claim of the entry, not re-measured here).
  2. runsc is not fetched. A host with no gVisor takes the machine tier (45 s
     against 10 s).
  3. Rootless gVisor serves a foreground `run` only: no `-t`, `--timeout`,
     `--tmpfs`, `--network broker`, `--podvm-confine`. The network is the host's
     (`docs/limits.md`, "`--podvm-tier=runsc`").
  4. Each write of the root goes to an overlay that ends with the run. A file
     whose owner changes is copied into the overlay.
  5. After SIGKILL of podvm the bundle and tree stay in the store.
  6. Facts of gVisor found by measure: with `/sbin/init` as the process of a
     bundle, systemd 262 ends at the mount of cgroup2 with
     `memory_hugetlb_accounting` (EINVAL) and of `/dev/shm` with `usrquota`;
     `runsc kill` takes no real-time signal; `systemd-sysctl.service` exits 1
     (the Sentry does not apply `readonlyPaths`); a `chown` to mysql holds with
     `--overlay2=root:self` (T-2007 premise).
* The pin: `https://storage.googleapis.com/gvisor/releases/release/20261102/x86_64/gvisor.tar.bz2`,
  75,332,313 bytes, SHA-256
  `6f6b1a8f0f4665a403238135167d0d990bfaeaff5544abf68f5e0d822b35484a` (T-2009
  Blocker). The tier needs `runsc` and `gvisor-bin/`.
* Tuning measured (T-2010 Done 3): none of `--overlay2=root:memory`,
  `--directfs=false`, `--host-settings=ignore`, `--app-huge-pages=false` gains
  more than run-to-run noise (8.0 to 8.3 s against 8.0 to 9.3 s). `--platform=ptrace`
  is slower (11.1 s). `--file-access=shared` is refused by runsc with the root
  overlay.
* Questions for a researcher:
  1. Does `systrap` really need `ptrace`? Is there a gVisor mode that runs
     rootless with neither user namespace nor `ptrace`?
  2. Which of gVisor's syscall gaps hit real hosted-runner workloads
     (Docker-in-gVisor, rootless podman, FUSE)?
  3. Why is the runsc route 0.9 s slower than the namespace rung before the first
     output (T-2010 Remaining 1)? Use `--boot-times-log`.
* First experiment: `runsc --boot-times-log` on the Debian 12 bench; compare the
  copy-up of owner changes with `--overlay2=root:self` vs `memory`.

#### A6. T-1994, T-1998, T-1996, T-1999: the image's own root disk, kernel, and incus VM

* IDs and state: T-1994 (`TODO/machine.md:5573`), P1, **open**. T-1998
  (`:5668`), P1, **open**. T-1996 (`:5602`), P1, partial. T-1999 (`:5697`), P1,
  partial. T-1997 (`:5635`) is done. Source: PodUtils/podrunner#313 and #316 (the
  "hosted snapshot": a 1:1 copy of the root file system of a GitHub-hosted
  runner, about 55 to 60 GB).
* What fails.
  * The machine tier's root is an initramfs that must fit twice in guest memory.
    A 584 MB image holds 2.6 GB of host memory in QEMU (`TODO/enter.md:5183`
    Done; finding P4, `docs/findings.md`: an initramfs larger than RAM gives no
    error, the guest resets, QEMU exits 0 with no output).
  * The image's own kernel and initrd (`/boot/vmlinuz-KVER`,
    `/boot/initrd.img-KVER`, `root=LABEL=cloudimg-rootfs`) cannot be booted.
  * incus runs an OCI image only as a container (lxc/incus#1360). A VM image of
    incus needs a qcow2 root, a GPT, and an ESP, or a direct kernel boot.
  * `security.capability` xattrs: podcrates `v0.76.0` (`pod-extract`) sets each
    `SCHILY.xattr.*` the kernel allows. The incus tier carries them
    (experiment 434 clause 22). The machine tier root does not yet (T-1999
    Remaining).
  * T-1996: `security.idmap.size` 231072, `/dev/fuse`, empty `/etc/machine-id`
    work (experiment 434 clause 20). Not measured: rootless `podman run` and a
    `fuse-overlayfs` mount of a user with `165536:65536`.
* Why it is hard.
  * Making an ext4 disk image from an OCI tree **without root** (no `mkfs` plus
    `mount`; no `chown`) while keeping owners. The UML builder trick (hostfs
    root, owners from the sidecar `.meta.jsonl`) is one answer; another is
    `mke2fs -d` plus an owner map: not measured.
  * Booting a stock distribution kernel needs matching modules (9p, virtio,
    vsock) from the image's `/usr/lib/modules`.
  * No `/dev/kvm` on the cloud host. T-1994 and T-1998 can be proved only on
    `kvm.yml`.
  * Nested KVM where the host has it: passes through only if the host allows.
  * Disk use of the import: measured at 2.38 times the tree for a streamed tarball
    (T-1997 clause 21).
* Tried: T-1997 streaming import (done). T-1993 systemd on incus (done).
* Questions for a researcher:
  1. Which tools build a bootable ext4 or qcow2 from a directory with exact
     owners, setuid bits and xattrs, with no root and no mount? (`mke2fs -d`,
     `guestfish`, `virt-make-fs`, `genext2fs`, `e2fsprogs` `fakeroot`-like.)
     Which are static or can be built static with zig?
  2. For incus VMs: what is the smallest legal image layout (`rootfs.img`,
     `metadata.yaml`, UEFI vs `kernel` direct)? Does incus 6.0.0 support direct
     kernel boot of a VM at all?
  3. `losetup -f` in a guest: the hosted kernel has `loop` built in. Does the
     machine tier's guest then give loop devices with the image's own kernel?
* First experiment: `kvm.yml`, Prove of T-1994: boot an image with systemd from
  a disk image, `systemctl is-system-running`, exit code from a unit with
  `SuccessAction=exit-force`.

#### A7. T-2008 and T-2010: a faster guest where there is no KVM

* ID and state: T-2008 (`TODO/machine.md:5730`), P3, open. T-2010
  (`TODO/enter.md:5439`), P1, partial.
* What fails. Under TCG with four vCPUs the machine tier boots systemd in 12 s of
  24 s; with one vCPU 37 s of 51 s (`TODO/enter.md:5183` Done). The guest
  spends about 10 s before systemd starts (kernel plus an initramfs of 800 MiB)
  and holds the root twice in memory (T-2010 Premise 3).
* Measured idea: restore of a saved guest takes 2 s (T-2010 Remaining 2). The
  setup: boot once per digest and shape, `savevm` or `migrate` to a file after
  `systemctl is-system-running`, start each run from it over a new overlay.
* Why it is hard. The root must be on a disk (T-1994), not an initramfs. State
  of another QEMU version or shape is invalid. NIC, MAC, host name, nonce and
  entropy must be fresh after restore (TH-35/FE-02 in issue #580). winquick
  (the entry's donor for Windows, T-1874) restores in 0.23 to 0.31 s on Apple
  M4 Pro with HVF and needs QEMU 11 for NVMe state; the hosted runner has QEMU
  10.2.1 and the pinned static QEMU is 9.2.0.
* Not measured (T-2010 Remaining 3): `mitigations=off`, TCG `tb-size`, multi-thread TCG.
* Questions: what gives the best TCG boot of a stock kernel and systemd (kernel
  config trimming, `-accel tcg,thread=multi,tb-size=`, `-machine microvm`, no
  PCI, `-cpu` choice, `-icount` off, 9p vs virtio-blk)? Finding F15
  (`docs/findings.md`): QEMU `microvm` with the virtkit 6.18 kernel crashed in 5
  of 6 boots until `pit=off,pic=off,tsc=reliable` (24 of 24 booted). Re-test.
* First experiment: `sh experiments/468-init-bench.sh` with `ROUTES=machine`,
  adding `PODVM_468_EXTRA` kernel arguments one at a time; record `first=`.

#### A8. T-2006 (done) and a leftover: process cleanup after SIGKILL

T-2006 (`TODO/supervise.md:2075`) is done: `PR_SET_PDEATHSIG` in pid 1 of the PID
namespace ends the tree after a SIGKILL of the outer podvm
(`crates/podvm/src/userns/sigrelay.rs:36`). Left: after a SIGKILL the tree of the
run, and the runsc bundle, stay in the store until `podvm rm` (T-2007 Remaining 4).
This is the general "no cleanup on SIGKILL" class, not a research question.

---

### Theme B. Identity, setuid and ownership

#### B1. T-0711: the identity calls on the userland rung

* ID and state: T-0711 (`TODO/interpose.md:1186`), P1, **done** by a ruling that
  keeps the problem open in effect.
* What fails. A payload that is already "root" calls `setuid`, `setgid`,
  `setgroups`. On the userland rung these fail (EPERM). Package managers that
  drop to an unprivileged user, `sshd`, build systems with a "do not run as
  root" guard take that path.
* The ruling (2026-09-11, the operator): **honest refusal by default, a flag turns
  on fakeroot-like behaviour, and the flag marks the container degraded so
  `--strict` refuses it.** The reason is the honesty rule of T-0804: no output
  may imply a property the runtime does not provide.
* Why it is hard. A kernel rule: changing uid needs CAP_SETUID in the user
  namespace and the target uid mapped. There is no honest user-space way to hold
  two uids with one real uid. A lie changes later code paths (the payload thinks
  it dropped privilege).
* Evidence: `experiments/results/init-image.txt:45`: "podvm: interpose:
  setresuid failed with errno 1 (EPERM); the payload keeps uid 1000, and podvm
  does not change it". Source: `crates/podvm-interpose/src/identity.rs:3` and
  `:277`.
* Open leads: only routes that run a **real second kernel** (UML, gVisor, QEMU) or
  a real user namespace with a range give real uids. See A3 and A5.
* Questions: what do apt, dpkg, `su`, `sshd`, `postgres` initdb do when the drop
  fails, per distribution (T-0711's "Approach" measure was owed)? Which of them
  accept a faked drop without behaving differently?
* First experiment: the one the entry names. `experiments/106-interpose-identity.sh`
  plus the matrix `experiments/125-across-distributions.sh`.

#### B2. T-1911: `-u UID` in the user namespace of a run

* ID and state: T-1911 (`TODO/probe.md:1936`), P3, partial.
* What fails. The user namespace of a foreground run maps only the caller
  (`0 UID 1`). `run -u 1000 IMAGE sh -c 'echo x > /root/qa'` cannot drop, so the
  write succeeds where docker refuses it.
* The fix exists but is opt-in: `PODVM_USERNS_RANGE=1` uses `newuidmap` and
  `newgidmap` with the caller's range (`crates/podvm/src/userns/range.rs`).
* The hard part is **the store rule**: a file the payload writes as uid 1000
  lands on the host as a subordinate uid (for example 100999). The caller
  cannot read, chmod or remove it outside the namespace (`run -d`, `create`,
  `PODVM_USERNS=0`). Partly solved: removal needs only the write right on the
  directory; `podvm rm` runs the removal in a fresh user namespace with the range
  (`RANGE_REMOVER`, `podvm --podvm-range-rm`); `cp` and `exec` of a non-running
  container run again in the range namespace.
* Remaining: `start` of such a tree (its detached path has a one-line map; `ls`
  gives "Permission denied"); then making the range the default.
* Tried and not kept: `start` again in the namespace with the range
  (`TODO/probe.md`, last paragraph of Done).
* Questions: how does rootless podman keep host-side removal working for
  subuid-owned trees (`podman unshare`, `podman system migrate`, and its
  `chown` cache)? Are there ways to keep files owned by the caller on the host
  while the payload sees other uids (fuse-overlayfs with `uidmapping`, idmapped
  mounts need `CAP_SYS_ADMIN`)?
* First experiment: the Prove of the entry, then a `run -d`, `stop`, `start`,
  `rm` cycle with `-u 1000` and the range.

#### B3. Files that a hosted image owns as setuid root

Related facts, not an entry: in the UML builder, on hostfs each file belongs to the
host user and a set-user-ID program loses root (T-2005 Done). `pod-extract` keeps
the intended owners in a sidecar, `.meta.jsonl`, and restores them only where the
rung allows it (`docs/architecture.md`). A researcher should check, for each new
route, whether a setuid-root file (`mount`, `sudo`, `ping` with `security.capability`)
keeps its power. Today this holds on the incus tier and the UML builder; the
machine tier root needs the xattrs (T-1999 Remaining).

---

### Theme C. cgroups, limits and disk quota with no root

#### C1. T-1747 and the delegation limit

* State: T-1747 (`TODO/cli.md:4032`) is done. It is the base for T-1969, T-1957.
* Facts (`docs/limits.md`, "Parked Windows lane" last paragraph, and 312ff):
  `--memory`, `--cpus`, `--pids-limit` hold on the incus tier (as `limits.memory`,
  `limits.cpu`, `limits.processes`) and on `run` and `run -d` of the native tiers
  **only where the host delegates a cgroup2 subtree to the caller**. podvm makes
  a leaf `podvm-ID`, writes `memory.max`, `cpu.max`, `pids.max`, and names an OOM
  kill. Every other case (no delegation, cgroup v1, `create`, the machine tier)
  refuses the flag with exit 125. `--ulimit nproc=` and `--ulimit as=` hold on
  every tier.
* Why it is hard. No unprivileged way to create or own a cgroup. `RLIMIT_AS`
  limits address space, not resident memory. CPU cannot be capped by rlimit.
* Evidence: experiment 459 on `kvm.yml` run 44285612361 (job `cgroup`).
* Questions: on cgroup v1 or hybrid hosts (the cloud host), is there any
  unprivileged way to account memory? What do other rootless runtimes (crun,
  bubblewrap, systemd-run `--user`) do when no subtree is delegated?

#### C2. T-1957: cgroup kill and byte accounting

* ID: T-1957 (`TODO/enter.md:4565`), P2, open. No code writes `cgroup.kill` or
  `memory.oom.group`, so an OOM kill ends one process and leaves a half-dead job.
  The syscall tier counts `/tmp` growth only for write-family calls; `mmap`,
  `sendfile`, `splice`, `copy_file_range` and a rename into `/tmp` are not
  counted (`docs/limits.md`). Not hard in itself; it needs a delegated leaf.

#### C3. T-1964 and T-1968: a sized writable disk for a unit

* IDs: T-1964 (`TODO/machine.md:5375`), P1, partial; T-1968 (`:5511`), P2, **open**.
  Source: PodUtils/podrunner#296 ask 1 and 5; githome's `limits.disk` need (a job
  past its quota gets `ENOSPC`, the pool stays bounded).
* State. incus: `--storage-opt size=N` copies the profile's `root` device with
  `size` and refuses if the pool reports no limit. The `dir` pool of route A
  reports none, so clause 17 of experiment 434 exits 125. A pool with quota
  (btrfs, zfs, lvm) is **not measured**: route A runs incusd in a user
  namespace that can mount none of them.
* T-1968 (no tier gives a sized disk): machine tier needs a flag for the
  writable disk; the namespace rung with no root has "no quota and no loop device";
  only a tmpfs with `size=` (memory) or a FUSE file system can hold a size.
* Why it is hard. Disk quota needs a block device, project quota, or a mount
  option. All need privilege. FUSE is blocked (`/dev/fuse` mode 0600 root).
* Questions for a researcher:
  1. Is there any unprivileged disk quota on ext4, xfs or overlayfs
     (`FS_IOC_FSSETXATTR` project quota on a user-owned tree; XFS project quota is
     root-set)? Does a user namespace unlock `tmpfs size=` and `quota` only?
  2. Is a sparse loop file mounted via `mke2fs` plus `fuse2fs` (static) workable as
     a bounded writable layer on a host with `/dev/fuse`?
  3. Is a size cap through the syscall tier handler (count write bytes at each
     boundary, including mmap) correct enough to claim `ENOSPC`? Today
     `--tmpfs /tmp:size=` caps only counted writes.
* First experiment: on a host with btrfs or zfs and root, measure
  `--storage-opt size=` on the incus tier (clause 17 variant).

#### C4. T-1969: OOM facts

T-1969 (`TODO/supervise.md:2041`), P1, partial. Native runs and the machine guest
give `oom_kills` (kvm.yml run 44285612361; machine guest under TCG measured).
Remaining: the field in the incus run report. Not hard.

---

### Theme D. The machine tier, KVM and the Windows lane

#### D1. The machine tier shape

See A6 and A7 for the Linux guest. Other limits (`docs/limits.md`, "Guests",
lines about 215 to 290):

* The guest does not honour `--podvm-confine`, `--user`, `--hostname`,
  `--add-host`, `--read-only`, `--tmpfs`, `--ulimit`, `--security-opt`,
  `--cap-drop`, `--label`: each is refused with exit 125 (T-1808). The incus
  tier refuses an even longer list (T-1854).
* stdout of a guest is text (T-1954, issue #537); `stop` powers the VM off with
  QMP `quit`, so the payload gets no SIGTERM and no exit code (#539); `logs`
  drops stderr (#540); machine records ignore `PODVM_STORE` (#511). All
  **open** under T-1954 (`TODO/machine.md:5314`). Each needs `kvm.yml` to prove.
* T-1941 (`TODO/machine.md:5285`): the `machine` profile asks for 1 GiB and a
  file-size ceiling under 1 GiB refuses it. The cure changes T-1305: a question
  for the owner. Not hard.

#### D2. The arm64 guest

* T-1890 is done (`TODO/machine.md:4730`): an arm64 guest on an x86_64 host runs
  under TCG with `qemu-system-aarch64 -M virt`. Its kernel has no pin in
  `pod-services` (COORD-128, #394), so `PODVM_MACHINE_KERNEL_ARM64` names it and
  the guest has no podvm binary: no `-t`, no 9p `-v`, no guest network module
  (`docs/limits.md`). Issue #543 notes that `docs/limits.md` said both "refused
  before boot" and "runs"; check that the text is now consistent.
* TCG of another architecture: "five to ten times native" is an estimate of the
  entry, not measured by podvm.

#### D3. The Windows guest

State of the Windows entries (all need KVM to prove, most are P2):

| Entry | State | The hard fact |
| --- | --- | --- |
| T-1861 (`TODO/machine.md:3232`) | partial, P1 | `windows:2025` and `windows:11` install under KVM; the setup ISO route is the open clause. Pins come from dockur/windows `src/define.sh`. |
| T-1870 (`:3977`) | partial | `run -d`, `ps`, `stop`, `rm` for DOS and ValidationOS under TCG done (experiment 454); the full desktop needs KVM. |
| T-1872 (`:4156`), P2, **open** | no `-v` | A live share has three candidates and none is proven: slirp `smb=DIR` needs a host `smbd` (not static); virtiofs needs `virtiofsd` and the WinFsp driver, and `virtiofsd` needs namespaces that the target hosts deny; an in-process WebDAV server on the slirp gateway needs the guest `WebClient` service and has a Windows file size limit the review did not check. 9p does not work on Windows. Plan: copy disk first. Needs a FAT writer with directories (COORD-123, #386). |
| T-1873 (`:4207`) | partial | Setup waits a fixed 240 s under TCG; under KVM a screen settle gave 37 s (kvm.yml run 43660534832). Open: whether `AutoRun` fires on the x64 image. winquick says it works on ValidationOS ARM64; `pod-guest/src/agent.rs` says on x64 `AutoRun`, `Run` and `RunOnce` do not fire and an `onstart` task as SYSTEM does. Conflict. |
| T-1874 (`:4268`), P2, **open** | no warm start | 29 s per boot under TCG (2026-09 harness); KVM: -. winquick restores in 0.23 to 0.31 s but needs QEMU 11. |
| T-1877 (`:4429`), P2, **open** | no Hyper-V enlightenments | Proposed `-cpu host,hv_relaxed,hv_vapic,hv_spinlocks=0x1fff,hv_time` untested; boot to RDP time: -. |
| T-1869 (`:3906`) | partial | `-p`, `screenshot` done; RDP check, VNC, `creds`, policy of 10.0.2.2 remain. Slirp gives no ICMP echo, so `ping` fails while `curl` works. |
| T-1900 (`:5072`), P3, open | no arm64 Windows | Needs a pinned medium (Windows 11 on Arm VHDX, source claim not checked), `qemu-system-aarch64 -M virt`, edk2, a virtio driver set. |

The docs limit "The Windows guest has no network in the current driver"
(`docs/limits.md:293`) predates `-p`; check which wording is current.

Parked lane (wsl-toolkit): nested KVM in the Windows base stopped the host on
2026-09-30 after an emulator that SIGKILL did not remove (`docs/limits.md:395`).
The lane is parked (`docs/decisions.md`, 2026-11-06). Not a research target.

Questions for a researcher (Windows):
1. Which shared-folder route works with no root, no `virtiofsd`, no `smbd`:
   a podvm-internal WebDAV on the slirp gateway, an SMB server in Rust, a FAT
   copy disk, or `hostfwd` of a small agent? What is the Windows file size limit
   for WebDAV through `WebClient`?
2. Does QEMU 9.2.0 (pinned) or 10.2.1 (hosted) support the migration needed for a
   Windows warm start without NVMe, using virtio-blk or SATA?
3. Does the Microsoft Windows Evaluation ISO still give fixed URLs, sizes and
   hashes (the review checked 2026-11-14)? Windows media drift is covered by
   `catalog-drift.yml` and question Q9 of the owner (`docs/decisions.md`,
   "Questions for the owner").
* First experiment: `kvm.yml`, `sh experiments/392-kvm-guest.sh` for the A/B of
  T-1877 (flags on and off), record the boot to RDP time.

#### D4. The cloud host has no KVM, so many proofs wait

Entries that say "the proof runs on `kvm.yml`": T-1873, T-1874, T-1877, T-1872,
T-1869, T-1994, T-1998, T-1954, T-1999 (machine part), T-2002 (a real delegated
subtree), T-2004. `kvm.yml` is dispatched by hand (T-1682, done). A researcher
who has a KVM host can settle these quickly.

---

### Theme E. The syscall tier and the interposer

#### E1. Raw system calls on the loader family: T-1740, T-1935, T-1956

* IDs and state: T-1740 (`TODO/enter.md:3327`), P3, partial. T-1935 (`:4431`),
  P1, partial. T-1956 (`:4531`), P2, **open** (donor study, issue #573).
* What fails. A dynamic program that issues the `syscall` instruction itself
  (Rust's `rustix`, used by `uv` and `ruff` through `tempfile`; Go with cgo;
  `liburing`; libuv with io_uring) passes the `LD_PRELOAD` interposer. Its path
  reaches the kernel as a host path. Measured (cloud host, uid 1000, under a
  seccomp filter that denies `unshare` and `clone(CLONE_NEWUSER)`, podvm
  0.2.0-beta.31): raw `renameat2(RENAME_NOREPLACE)`, `rename`, `link` give -13
  (EACCES) in `/root` and -2 (ENOENT) in `/tmp`; the libc calls work
  (`#470`, COORD-177).
* Why it is hard. An `LD_PRELOAD` object cannot see an inline `syscall`
  instruction. The only complete mechanisms need the kernel to cooperate:
  SUD (Linux 5.11), seccomp user notification, `ptrace`, a user namespace.
  The SUD handler costs a signal for each call.
* Measured cost (experiment 438, `experiments/results/raw-syscall-cost.txt`,
  alpine 3.20, ms): `exec` 1880 loader family vs 2392 tier (+27 %); `io` 631 vs
  609; `calls` (`dd bs=1`) 311 vs 667 (x2.1; the copy alone 0.12 s vs 0.50 s).
  The entry's text says "up to 4 times slower" for call-bound work.
* Tried:
  * The syscall tier as an opt-in (T-1731, T-1657). `run -d`, `create`, `exec`
    served on it since T-1935 (partial). In the tier, `uv venv` passes 5 of 5
    and `ruff check` writes its cache, where the loader family gives "Failed to
    rename temporary cache file ... Permission denied (os error 13)"
    (`TODO/enter.md:4431`, Done).
  * Retry of `openat2` on EAGAIN, up to 256 times (T-1990;
    `crates/podvm-dispatch/src/sys.rs:165` to `:195`).
  * Signals deferred out of podvm's code to fix Go's SIGURG hang (T-1995, done;
    `crates/podvm-dispatch/src/signals.rs`).
* Decision (call C19, `docs/decisions.md`): the loader family stays the default
  for a dynamic program; `--strict` names `--podvm-tier=syscall` as the remedy.
* Open leads from the donor study (issue #573; every claim is static analysis of
  other projects, no podvm run):
  * FA-03: answer ENOSYS for `io_uring_setup` on the tier when no broker hold is
    on (libuv then falls back to epoll). P1.
  * FA-02/TH-24: one table test that lists every path-taking syscall number and
    marks it mapped, refused or exempt. Candidates named: `fanotify_mark`,
    `name_to_handle_at`, `open_tree`, `move_mount`, `fsopen`, `acct`, `swapon`,
    `quotactl`.
  * FA-01/TH-01: a **seccomp-TRAP rung that traps only path calls**, used where
    the host refuses the SUD `prctl`, with the handler's own calls passed by
    instruction-pointer range and hand-emitted BPF. This would also lower the
    cost.
  * TH-02: a seccomp **user-notification rung with `ADDFD`**. T-0606 refused the
    notification tier as a per-call fallback (it fails silent when the read
    channel dies, and `seccomp_unotify(2)` says it is race-prone and must not be
    used for security policy). **The owner must rule first**.
  * TH-03/FA-07: pin each volume path with an `O_PATH` descriptor
    (`openat2` `RESOLVE_NO_SYMLINKS`).
  * FA-04: restricted io_uring rings (`R_DISABLED`, then `REGISTER_RESTRICTIONS`).
  * FA-06: raw-syscall engine for aarch64: check that SUD arms there first.
  * TH-56: evaluate or refuse the payload's own seccomp filters and SUD `prctl`.
    The handler already answers EINVAL to the payload's own SUD `prctl`
    (`crates/podvm-dispatch/src/handler.rs:440`). A payload's own seccomp filter
    is not handled.
* Questions for a researcher:
  1. How do gVisor `systrap`, `fake-chroot-ng`'s `monitor/sigsys.c`, `proot-next`,
     and linuxity restrict the trap to path calls? What is the cheapest BPF that
     returns `SECCOMP_RET_TRAP` for path-taking calls only and `ALLOW` for the
     rest, and what does it cost for `dd bs=1` (the worst case above)?
  2. Is a SIGSYS handler in the payload's address space ever safe against a
     payload that overwrites the handler's memory or alters the selector byte?
     (Today the tier is documented as not a boundary.)
  3. Does Linux (any version up to 7.x) have a mechanism to restrict `openat2`-less
     path calls inside an unprivileged process without a namespace, for example
     Landlock "scope" or a new `RESOLVE_` flag? (-: not checked.)
  4. What does AppArmor on `ubuntu-24.04` actually block and what does it leave?
     (`apparmor_restrict_unprivileged_userns`; entry T-1935 says "restricts user
     namespaces". A guess, not tested here: a path through a profile that
     Ubuntu ships as unconfined for user namespaces. The researcher should check
     whether any legitimate route exists.)
* First experiment: `sh experiments/438-raw-syscall-cost.sh` with a new row for a
  seccomp-TRAP rung prototype; compare `calls` and `exec` rows.

#### E2. T-1990 and T-1991: two unreproduced failures on podrunner's CI

* T-1990 (`TODO/enter.md:5088`), P1, partial: "errno 11" (EAGAIN) at the start of
  a dynamic program on the tier on GitHub's `ubuntu-24.04` runner (Node in a volume
  each time; `/usr/bin/bash` once). The retry went in (`b00c07c`). Not reproduced
  on the cloud host (kernel 6.18): 300 execs of `/usr/bin/true` under a loop of
  renames and tmpfs mounts did not fail. podrunner's CI measures it
  (beta.36 carries the retry).
* T-1991 (`TODO/enter.md:5121`), P1, **open**: `cargo doc` of ripgrep at
  `178cc027` prints "Compiling ripgrep v15.2.0" and then nothing for 45 min on the
  tier; passes with no tier and with `--runner-dir`. Not reproduced: 17.5 s with
  beta.34, 12.1 s with `main`, on the cloud host as uid 1000 with a Rust image.
  Possible link: T-1995 (SIGURG hang), fixed in beta.36. COORD-197 (#593) asks
  podrunner for `uname -r`, process list with `wchan`, `/proc/PID/stack`, and
  whether "errno 11" printed before the hang.
* Why it is hard. Both depend on the runner's kernel and timing, which the
  cloud host does not have.
* Questions for a researcher: which kernel version did the hosted
  `ubuntu-24.04` runner have on the failing run (-)? Is there a documented
  interaction between SUD and `vfork`/`posix_spawn` plus futex wait, for example
  glibc's `posix_spawn` stack of 36 KiB (T-1935 Done names one fix)?
* First experiment: run podrunner's `user-gate.yml` lane `proxy-only` with
  beta.36 and the tier on, and capture the hang's stacks.

#### E3. T-1972: `--podvm-volume-base` on the loader family

* ID: T-1972 (`TODO/enter.md:4597`), P1, partial. Source: PodUtils/podrunner#300
  ask 3. The race of PodUtils/podrunner#277: a step of the job turns a path under
  the runner root into a symbolic link after podrunner checked it and before
  podvm opens it.
* Done: namespace rung (pod-enter binds the descriptor, T-1945) and the syscall
  tier (`vols::beneath` with `openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)`,
  `crates/podvm-dispatch/src/vols.rs:26`). The loader family refuses the flag
  (`crates/podvm/src/run/binds.rs:88`).
* Hard part: the interposer maps a guest path to a host path at each call. A
  held `O_PATH` descriptor works through `/proc/self/fd/N/REST` but a program that
  closes all descriptors (`close_range`, Python `close_fds`) loses it. The entry
  proposes a reserved range restored after `close_range`.
* Question: is there a race-free way to keep a directory pinned across
  `close_range` and `exec`, short of a helper process?

#### E4. T-1984 and T-1985: the Landlock confine layer has two holes

* IDs: T-1984 (`TODO/enter.md:4929`), P2, open; T-1985 (`:4958`), P2, open.
  Announced as limits by call C25 (`docs/decisions.md`, 2026-11-18) and in
  COORD-196 (#592). podrunner says neither weakens its confined kind (podrunner#307).
* What fails.
  1. `/lib64/ld-linux-x86-64.so.2 /tmp/prog` runs a program that `exec=` denies
     when started directly (`direct=126`, `ldso=0`), on every rung. Landlock
     checks EXECUTE at `execve`, not at `mmap(PROT_EXEC)`, and the layer must let
     each run read the loader.
  2. With `read=/sc;write=/tmp` and `-v H:/no`, the payload changes `/no/s` from
     mode 600 to 666 and resets its mtime. Landlock governs no change of mode,
     owner or times (no access right for them up to the ABI the entry names, ABI 6).
* Fixed on the tier: `exec=` is enforced by the handler's own table (T-1983, beta.35).
* Planned: where podvm sees the call (interposer, handler of the tier), `chmod`,
  `chown`, `utimensat` of a path no `write=` rule names give EACCES. On the
  namespace and chroot rungs only the help text and the advice of a `:ro` volume
  stay.
* Questions: does a newer Landlock ABI (7 or later, as of 2026; -: not checked) add
  rights for attribute changes, or a way to deny `mmap(PROT_EXEC)` of a file? Does
  `LANDLOCK_RESTRICT_SELF_*` or a seccomp argument filter on `mprotect`/`mmap`
  stop `ld.so` from executing a file that `exec=` denies, without breaking
  normal `ld.so` use? What does `noexec` on the mount do with `ld.so`?
* First experiment: the Prove of T-1984 and T-1985 on a kernel with the newest
  Landlock ABI.

#### E5. Other syscall-tier gaps named in `docs/limits.md`

* Only x86_64 payloads (the interposer and SUD handler are x86_64;
  T-1721 ported the interposer to aarch64 in `arm.yml`, but the SUD tier on
  aarch64 is FA-06, unchecked).
* The entries of other processes in `/proc` are the host's, so `--podvm-confine` is
  what hides them (COORD-194, #590: measured: `/proc/PID/environ` of a host
  process of the same uid is readable on the tier with no confine layer).
* A static x86_64 payload on a host that refuses the SUD `prctl` exits 125
  (T-1648). No second rung exists for that host (TH-01/FA-01 would be one).
* `/tmp` cap counts only the writes of a descriptor opened under `/tmp`.

---

### Theme F. Networking, labs and observability

All of these come from the sand-net parity scope (issues #178, #180, #182,
index #193). They wait for `pod-netlab` (podcrates).

#### F1. T-1662: network labs inside a guest

* ID and state: T-1662 (`TODO/net.md:24`), P2, partial. Issue #178.
  `podvm lab scenarios` and `plan` run. The lab guest, its probes and the judge
  are `podcrates@T-345 to @T-347`.
* What fails. The target host has no network namespace and no `/dev/net/tun`, so
  a lab cannot be built on the host. Plan: boot one machine-tier guest whose
  kernel is root, and build the three namespaces (guest, router, world) with
  nftables and `tc netem` there.
* Gap against sand-net: 27 scenarios against 39. These 12 are absent because they
  need a WireGuard server, a live uplink or `podnet pod`: `vpn-full`, `vpn-split`,
  `vpn-v6-only`, `vpn-udp-blocked`, `vpn-bypass-policy`, `vpn-leak`,
  `vpn-killswitch`, `warp-uplink`, `warp-v6-only`, `warp-no-udp`, `warp-443-only`,
  `corp-proxy-pod` (`docs/limits.md`, "Network lab", about line 360).
  Also: `--loss` stops at 20 % (past it a TCP probe gives no verdict in its time
  limit); `--nat64` alone has no DNS64; knob sets of nine scenarios come from the
  names, because no source of sand-net's values exists (`podcrates@T-586`).
* Why hard. TCG cost (3 to 21 times native); the kernel rate-limits ICMP errors
  (L1 in `docs/findings.md`); an unconnected UDP socket never sees
  port-unreachable (L2); neighbour resolution after `up` (L3); IPv6 dead for tens
  of seconds after a VM used the veth in about 1 run in 5 (L7); tayga with the
  well-known prefix `64:ff9b::/96` answered every packet with ICMP unreachable
  (F18, `inferred`).
* Questions: what is the minimum-sized guest kernel that carries netns, nf_tables,
  netem, wireguard, tun, and boots under TCG in a few seconds? Is a user-space
  stack (`pod-netstack`, `podnet net`) enough for the policy-only labs?
* First experiment: `sh experiments/418-net-lab.sh` once the guest crate exists.

#### F2. T-1663 (blocked): tunnels and live uplinks

* ID: T-1663 (`TODO/net.md:149`), P2, **blocked** since 2026-11-11 with T-1662 on
  `podcrates@T-206` (pod-guest's modloop reader) and `@T-207` (pod-netlab).
  Issue #180. Parts that need no lab are in place: the kill switch of a
  rootfs-tier payload is the hold of T-1665.
* Hard facts quoted from issue 180 (`TODO/net.md:149` "Correction"): an nftables
  drop in OUTPUT is silent for TCP and EPERM for UDP and ping sockets (F22);
  `cfrs warp tun` kept working after its TUN moved to another namespace because
  the process keeps its descriptor (F23); quick-tunnel provisioning hit 429 with
  Retry-After 1379 on the eighth provision in about 70 minutes. The owner's rule
  "Test accounts: Not now" (2026-11-09) forbids a Cloudflare or Zero Trust account
  in CI, so the live WARP leg waits.
* Questions: can a kill switch for one payload be enforced without a namespace:
  the SUD handler refusing each `connect` but the broker's does it per payload
  (T-1665); what about `sendto` on an unconnected UDP socket, `AF_PACKET`, and a
  payload that does `connect` through a raw `syscall` instruction on the loader
  family? (Refused only on the tier.)

#### F3. T-1664 (blocked): capture, counters and connection logs without `CAP_NET_RAW`

* ID: T-1664 (`TODO/net.md:238`), P2, **blocked** with T-1662. Issue #182.
* What fails. `AF_PACKET` needs `CAP_NET_RAW`. A payload of the rootfs tiers shares
  the host network, so nothing can capture its packets.
* Leads. QEMU `filter-dump` writes a pcap of a guest's user-mode network with no
  privilege. A connection log at the syscall layer (address, port, result,
  errno) needs no lab and comes with the entry. Not a packet capture, and the
  output must say so. The gateway has no ICMP echo relay (`TODO/net.md:769`,
  T-1961, issue #578).
* Question: what does slirp (QEMU user networking) do for ICMP echo, and can
  `ping_group_range` plus an unprivileged ICMP socket give a relay on the host
  (finding F20)?

#### F4. T-1916: lab profiles for hostile host classes

* ID: T-1916 (`TODO/net.md:680`), P2, partial. Source: PodUtils/podrunner#180.
* Done: the host form with 16 profiles from `pod-lab` (v0.54.0): `userns-off`,
  `bind-denied`, `small-tmp`, `noexec-tmp`, `inherited-fds` and others
  (experiment 462 passes). Remaining: the guest form (`--podvm-lab PROFILE` of the
  machine tier). Seccomp caveat: a filter cannot read the family of a
  `sockaddr`, so `bind-denied` also refuses an `AF_UNIX` path of exactly 13 or 25
  bytes.

#### F5. The egress broker and the incus tier have no network

* Under route A the incusd sits in an empty network namespace; an instance has no
  network and `-p` is refused (`docs/incus.md` "The network"; "What stays open").
* A `proxy` device failed on a hosted runner with "Failed to safely open namespace
  file descriptor based on pidfd" (PodUtils/podrunner#90), so the channel is a
  disk device and a relay in podvm's init (`docs/incus.md`, "No network and one
  channel").
* Question: errand-box relays the network through a unix socket (its E-23). Is a
  user-space slirp relay to an incus instance, as a veth in the incusd's
  namespace, workable without root?

---

### Theme G. Platforms: arm64, Windows, foreign architectures

* arm64 userland rung: T-1721 is done. `arm.yml` runs the 87 interposer tests on
  `ubuntu-24.04-arm`. Only the x86_64 binary embeds the x86_64 objects
  (`docs/limits.md:60`).
* Foreign-architecture child exec through qemu-user (FA-05 in #573): open.
* SUD on aarch64 is unchecked (FA-06 in #573). If the arm64 kernel refuses
  it, the syscall tier has no engine there.
* Windows arm64: T-1900, open (see D3).
* An arm64 machine guest has no podvm inside (see D2).
* Foreign guest speed: see A7 and D2.

Question for a researcher: does `PR_SET_SYSCALL_USER_DISPATCH` exist and work on
arm64 for the kernel versions of GitHub's `ubuntu-24.04-arm` runner (-: not checked)?

---

### Theme H. The store, images and the copy of a root

#### H1. T-1749: a root with no copy

* ID: T-1749 (`TODO/enter.md:3457`), P3, partial. Source: PodUtils/podcrates#124 V-5.
* What fails. Each container gets its own tree: `run` and `create` copy the
  extracted image (T-1709). Cost measured (experiment 442): median `run --rm
  IMAGE true` is 77 ms for busybox, 2861 ms for ubuntu:24.04 (fill 681 ms, 23 %)
  and 12611 ms for golang:1.24.7-bookworm (fill 8377 ms, 66 %) on ext4, no
  `FICLONE`. With a spare tree (T-1781, experiment 448): golang 181 ms against
  8.5 s. The spare costs the disk of one more tree for each image.
* The goal: a read-only lower tree and a per-run upper layer. Overlay mount works
  only where mounts work. podcrates `v0.42.0` has `Root::Overlay`; `v0.43.0` adds a
  two-layer completion. podvm "can wire the overlay root now" (entry). The userland
  rung would need a copy-up of the path table.
* Open: the first run of an image and each `create` still copy.
* Questions: how much faster is a `FICLONE` or reflink-based copy on btrfs/XFS in
  practice for 800 MiB images? What does `fuse-overlayfs` give where `/dev/fuse` is
  open? Can the loader family's path table do copy-up (and listings that both
  layers hold) correctly?

#### H2. T-1960: images and the store

T-1960 (`TODO/image.md:4594`), P2, open (donor study #577). A base archive temp
file is named `partial-{pid}`, and a pid is not unique across PID namespaces (two
units of one store can write one file, with no digest check). The machine image is a
cpio so guest RAM holds the whole rootfs. Layers pull whole (no lazy pull, no range
index). This overlaps A6 and the quality rules in `AGENTS.md` section 2b.

#### H3. FUSE and tmpfs entry are "a limit of the measured host"

`docs/limits.md:152`: forced FUSE mode and the permitted-tmpfs entry arm prove
wherever the host grants `/dev/fuse` and `mount(2)`. The Windows base of the
parked lane granted neither. The prover is `experiments/358-ladder-rungs.sh`. A
cloud session has not run it (result: -). No task owns it. T-1929 (`TODO/enter.md:4328`,
P3, blocked on podcrates, COORD-174 #467): `PODVM_MODE=rundir` and `tmpfs` refuse a
tree of root because `pod_enter::stage::copy_tree` refuses a device node that
podvm's fixups made; a measurement switch, no user path.

---

### Theme I. Terminals, signals and PID 1 behaviour

* #584, COORD-188 (to podcrates): `podvm run -t` passes the pty slave as fds 0, 1, 2
  but makes no new session, so the slave is no controlling terminal: `tcgetpgrp(0)`
  gives ENOTTY (errno 25), `^C` sends no SIGINT, and the job control of an
  interactive shell does not work. podvm's interim code takes the `VINTR` byte out
  of the `-it` input where the pty has `ISIG`, and raises SIGINT; `VQUIT`, `VSUSP` and
  job control still need `setsid()` plus `ioctl(TIOCSCTTY)` in pod-enter's child.
  Status: open, waits for a podcrates tag.
* T-2002 Remaining 3 and 4: the pty of `-t` belongs to the devpts of the host
  (`tty` gives "No such device"); the command of `--user NAME` gets the primary
  group only, no supplementary group.
* T-1953 (partial) lists SIGHUP, SIGTERM to a Windows guest (#566), and `logs -f`
  after `restart` (#550).
* Hard background: a "pid is not unique" across PID namespaces
  (`docs/findings.md` lesson 5; T-1938).

---

### Theme J. CI, hosts and process

* The hosted `ubuntu-24.04` runner restricts user namespaces by AppArmor. It is the
  reason podrunner's units sit on the userland rung and the syscall tier (T-1935).
  What other restrictions it has (`ptrace`, `unshare` through an allowed profile,
  `/dev/kvm` by udev rule): partly documented in T-1682 premise ("a documented
  runner property, not measured here").
* The cloud host has no KVM and no delegated cgroup. `kvm.yml` (T-1682) and
  `arm.yml` (T-1721) run the missing legs on hosted runners.
* GitHub refuses a `GITHUB_TOKEN` a ref on a commit whose workflow files differ from
  the default branch (call C22): a release run must hold each push until it ends.
* The beta gate (`docs/maintenance.md`, "Releases") requires: gate,
  `workloads.yml`, the uid-1000 tests, an outside pass, `arm.yml`, a nightly trial.
  Open P1 entries (T-1935, T-1938, T-1943, T-1947, T-1951, T-1953, T-1964, T-1969,
  T-1970, T-1972, T-1974, T-1990, T-1991, T-1994, T-1996, T-1998, T-1999, T-2002,
  T-2007, T-2010) collide with the rule "no P1 stays open" that call C25 reads
  as "no documented feature fails" (a ruling that is the owner's).
* Pkgforge mirrors return 403 and 429 at times (`docs/HANDOFF.md`).
* podrunner's open register of unknowns (PodUtils/podrunner#9): U1 settled "no" for
  the **official runner on the userland rung** (exit 154, then 137), "yes" on the
  supervise rung (beta.3 and later). U26 settled "no": podvm had no guest of
  another architecture (now done by T-1890 for the machine tier).

---

## 3. Questions waiting for the owner (not research questions)

From `docs/decisions.md`, "Questions for the owner" and the entries:

* Q8 (2026-11-15, T-1824): should a run print one line on stderr by default where the
  rung gives no isolation (chroot, userland, supervise) and the output is a terminal?
  Recommendation recorded: yes, only then.
* Q9 (2026-11-15, T-1876): Windows catalog pins in the binary; keep, patch release
  on drift.
* T-1941: change T-1305's refusal of guest memory over `RLIMIT_FSIZE`.
* TH-02 / T-0606: whether a seccomp user-notification rung may exist at all.
* C26 (order of routes of `--podvm-init=image`) and C24/C23 (binary size ceiling,
  now 9,000,000 bytes) are calls "without a ruling" that the owner can change.

---

## 4. Ranked list: ten problems where outside research helps most

The order weighs: how many other problems it unblocks, whether the answer is
knowledge that a researcher can find (a kernel rule, a tool, a benchmark) rather
than work that only podvm can do, and how little podvm can measure alone (the
cloud host has no KVM, no delegation, no user-namespace-denied peer).

1. **A real PID 1 and more than one uid with no user namespace, no ptrace, no KVM**
   (T-2005, T-2002, T-0711; `TODO/enter.md:5309`). The only no-guest candidate is UML
   in seccomp mode: 14.7 s against 45.6 s under TCG, measured once, with a glibc
   (not static) kernel. Research: static UML build, size, field cost, correctness
   of network and SMP, arm64.
2. **A root disk and the image's own kernel, built with no root** (T-1994, T-1998,
   T-1999; `TODO/machine.md:5573`). Research: tools that build an exact-owner
   ext4 or qcow2 from a directory without mount or chown; the smallest incus VM
   image layout; direct kernel boot; `losetup` in the guest.
3. **A fast guest under TCG, and warm start** (T-2008, T-2010, T-1874; `TODO/machine.md:5730`).
   Research: TCG and kernel tuning, QEMU `savevm` or migration with a fresh
   identity, `microvm` crashes (F15), QEMU version limits (9.2.0 pinned, 10.2.1 hosted,
   11 for winquick).
4. **Complete path mediation for a dynamic program with raw syscalls, cheaply**
   (T-1740, T-1935, T-1956, #573; `TODO/enter.md:3327`). Research: a seccomp-TRAP rung
   that traps only path calls; io_uring handling; the soundness of an in-process
   SIGSYS handler; SUD on aarch64; and the two unreproduced hangs T-1990, T-1991.
5. **A bounded writable disk with no root** (T-1968, T-1964; `TODO/machine.md:5511`).
   Research: any unprivileged quota (project quota, tmpfs size, `fuse2fs` loop
   file, byte counting at the syscall layer including `mmap`), and a test on a
   btrfs/zfs host.
6. **cgroup control without delegation** (T-2004, T-1957, T-1747;
   `TODO/enter.md:5276`). Research: what hosted runners and common container hosts
   delegate; systemd user-manager scopes without linger; accounting on cgroup v1
   hosts; `cgroup.kill` and `memory.oom.group` use.
7. **Landlock holes: exec through the loader, mode and time changes** (T-1984, T-1985,
   T-1958; `TODO/enter.md:4929`). Research: newer Landlock ABIs, `mmap(PROT_EXEC)`
   control, a verdict report of what the kernel actually enforced.
8. **Network labs, tunnels and observability with no root** (T-1662, T-1663, T-1664,
   T-1916; `TODO/net.md:24`). Research: a minimal lab guest kernel, a user-space stack
   for policy labs, slirp ICMP echo, packet capture through QEMU `filter-dump`,
   a per-payload kill switch.
9. **gVisor on a host with no user namespace and a pinned fetch** (T-2007 Remaining 5,
   T-2009; `TODO/enter.md:5365`). Research: does `systrap` need `ptrace`; is there a
   mode with neither; boot cost of the 0.9 s gap; Docker, podman and FUSE inside
   gVisor.
10. **A Windows guest as a first-class tier: shared folder, warm start, offline setup,
    arm64** (T-1872, T-1873, T-1874, T-1877, T-1900; `TODO/machine.md:4156`).
    Research: a live share that needs no root, `virtiofsd`, `smbd` or `WinFsp`; the
    `AutoRun` conflict; Hyper-V enlightenment flags; a Windows 11 Arm medium.

Not ranked but worth a look: the controlling terminal of `run -t` (#584), the
sized overlay root of T-1749, and the `RUNPATH` fix of T-2003 (both are mostly
podcrates' work).

---

## 5. What a researcher can do first, in one afternoon

1. Read `docs/limits.md` "The init of an image" and `TODO/enter.md` from line 5183.
2. Read `experiments/results/init-bench.txt` and `experiments/results/uml-seccomp.txt`.
3. On a host with KVM and user namespaces, run `sh experiments/465-init-image.sh`
   and `sh experiments/468-init-bench.sh` as a non-root user (the script lists its
   inputs; it needs `qemu-system-x86_64`, `runsc`, the Debian 12 packages, and root
   to make the stand-in cgroup2 directory).
4. On a GitHub-hosted `ubuntu-24.04` job: run `kvm.yml` (job `cgroup`), and record
   which of `unshare -U`, `ptrace`, a delegated cgroup, `/dev/kvm`, `/dev/fuse` are
   open for the user `runner`. Put the answers next to T-2002, T-2004 and T-1935.
