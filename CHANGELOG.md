# Changelog

Current behavior and limits are in [README](README.md) and
[Limits](docs/limits.md). Task status is in [INDEX](TODO/INDEX.md).
Earlier repository change records are retained in
[the captured changelog](docs/history/CHANGELOG.md-before-2026-09-30.txt).
They describe their recorded revision, not the current build.

## Unreleased

### Fixed: an incus `--rm` run that the OOM killer ends, and the QEMU copy in the store, 2026-11-26

Record: T-2100 (OR-B41-02, OR-B41-07, OR-B41-13, OR-B41-14).
Deployed: not deployed; for the next beta after 0.2.0-beta.41.

- The incus tier stops and deletes the instance again where incusd says
  that it still runs, up to four times, so a `--rm` run that the OOM
  killer ends no longer keeps its instance.
- Where `PODVM_STORE` is set and `PODVM_TOOLS_DIR` is not, the machine
  tier keeps its copy of QEMU in `PODVM_STORE/tools`, not under `$HOME`.
- `notebook` with no notebook exits 1, the code of a missing argument.
- `system df` counts only whole extractions, as `inspect .Extracted` does.

## 0.2.0-beta.41 (2026-11-26)

### Known gaps: what the checks of 0.2.0-beta.41 prove, and what stays open, 2026-11-26

Record: T-1949, T-2054, T-2070, T-2078, T-2079, T-2087, T-2100.
Deployed: version 0.2.0-beta.41.

What the checks of this beta prove: the gate, `arm.yml`, the nightly
trial and `workloads.yml` on the version commit; the integration tests as
uid 1000 (37 binaries, on `7c2b161`); experiment 479 on a hosted runner
with cgroup v2 (`update` of a running incus instance, T-2093); experiment
473 under TCG (a saved guest state with a channel and a process limit,
T-2094); experiment 480 under TCG (a SIGTERM during a save, T-2048); and
the tenth outside pass with the binary of `7c2b161` and `podvm man` only
(T-2100): no P0 and no P1 finding.

Each open fault that a user can meet:

- The incus tier: the import expands the hard links of an image; a
  payload can remove its own log; `exec -w` to a missing directory runs in
  `/`; `--podvm-init=image` of an image with no init gives a raw error of
  incusd and leaves the instance; `wait` and `kill` have no path of the
  tier; a created instance reads `Stopped`; a `--rm` run that the OOM
  killer ends can keep its instance (T-2079, T-2087, T-2100 OR-B41-07).
- The incus tier: an image that an older podvm imported is not checked
  against the room of the pool (T-2078). Each call to incusd after the
  first of a verb has no time bound (T-2080).
- The machine tier: the fetched QEMU and the records of a detached guest
  go under `$HOME`, not `PODVM_STORE` (`PODVM_TOOLS_DIR` works around it);
  `doctor` calls the tier refused where `run` fetches its own QEMU; a
  guest with too little memory names no minimum (T-2087 OR-B40-07, T-2100
  OR-B41-02, OR-B41-15).
- The native tiers: the edits of a payload to `/etc/hosts` go at the next
  `exec`; `run -it sh` and `exec -it CTR sh` have no job control; a
  foreground run on the namespace rung reaps no orphan until it ends;
  `--device HOST:GUEST` with another guest path is visible only in part;
  a foreground `run --rm` that SIGPIPE ends keeps a `Created` record
  (T-2079, T-2070, T-2087, T-1939, T-2100 OR-B41-06, OR-B41-08).
- docker differences and texts of the outside passes: `login` contacts no
  registry; `--env-file` refuses a line with no `=`; a name in use exits 1
  from `create` and 125 from `run`; `run --rm` keeps the extracted tree of
  the image; `--network host`, `--format json`, `'{{json .}}'`, `table`
  and `index` are refused; a payload argument that is not UTF-8 is
  refused; `cp` refuses `..` and an absolute link of the container; `run
  -u` keeps the caller's privileges with no line; `--init` and `top` give
  a reason that is false on the namespace rung; a missing argument exits 1
  or 125 by verb; a reference has no bound of its length; `images` counts a
  repeated layer each time; `system df` counts an extraction that failed
  (T-2070, T-2079, T-2087, T-2100).
- The namespace rung: podvm's descriptors of the memo and the log, and the
  variables of the interposer reach the payload; a foreground run of uid 0
  has no PID namespace (T-2054 items 1 to 3).

### Fixed: a signal during the save of a guest state, and three outside-pass findings, 2026-11-26

Record: T-2048, T-2100 (OR-B41-03, OR-B41-05, OR-B41-16).
Deployed: version 0.2.0-beta.41.

- On the machine tier, a SIGTERM or another forwarded signal during the
  save of a guest state ends the run at once with one line. Before, the
  run waited for the end of the save (up to about 190 s, an estimate from
  the bounds) and kept a partial state file (experiment 480).
- `--format json` is refused by name. Before, it printed the word `json`
  and exited 0.
- `update --memory` with a value too large for a count of bytes says so,
  not "at least 6m".
- The usage of `system abi` says 125 for a file that podvm cannot read,
  the code that the verb gives.

### Fixed: no store path in the argv of the spare-tree helper, 2026-11-26

Record: T-2054 (item 5).
Deployed: version 0.2.0-beta.41.

- The detached `podvm tree-prefill` gets its store and digest on a pipe,
  so `ps` of another user shows no path of the store.

### Fixed: repeated OCI layer descriptors, 2026-11-26

Record: T-2098.
Deployed: version 0.2.0-beta.41.

- `podvm load` accepts repeated references to a blob verified from the
  same OCI archive and keeps the original layer order.

### Added: live Incus resource updates, 2026-11-26

Record: T-2093.
Deployed: version 0.2.0-beta.41.

- `podvm update --podvm-tier=incus` changes memory, CPU or process
  limits on a running podvm container. It uses the instance ETag and
  checks the limits before reporting success.

### Changed: podcrates `v0.85.0`, 2026-11-26

Record: T-2097.
Deployed: version 0.2.0-beta.41.

- Extraction and broker policy errors report the step and the underlying
  cause with the new podcrates error format.
- The binary packs the embedded interposers with gzip and decodes the
  selected object when needed. The placed ELF and reported hash are the
  original object's bytes and hash.

### Added: channels and process limits for saved Linux guests, 2026-11-25

Record: T-2094.
Deployed: version 0.2.0-beta.41.

- On the machine tier, a saved guest can use `--podvm-channel` and
  `--pids-limit`. Each run supplies its socket, port and limit after the
  save or restore. The guest applies the limit before the payload starts.

### Changed: podcrates `v0.84.0` and the shared gzip writer, 2026-11-25

Record: T-2085, T-2096.
Deployed: version 0.2.0-beta.41.

- The incus tier uses `pod_gzip::Members` for compressed image uploads.
  The same space check selects a plain or compressed upload.
- The Incus client bounds a write that stalls on the daemon socket.
- A fresh Incus import uses `pod_extract::flatten_with` to size and
  stream the image from its OCI layers. Images that require extraction
  use the tree-based path. The space check runs before the upload.

### Fixed: what the namespace rung opens, and the commit of a source build, 2026-11-25

Record: T-2091 (#531), T-2092 (#546).
Deployed: version 0.2.0-beta.41.

- `README.md`, `docs/architecture.md`, `SECURITY.md`, `docs/limits.md`,
  the probe field `rung_must_never_claim` and the refusal of `--pid`,
  `--ipc`, `--uts` and `--userns` say what a run on the namespace rung
  opens: a user, a PID and a UTS namespace by the caller and the verb, and
  no network namespace, as the banner says.
- `podvm-dev build` where `podvm-buildstate` is not built gives one line,
  and the build reads the commit from git: `version --verbose` no longer
  shows the error of the script as `commit:`.

### Changed: the room of a storage pool from incusd, for each driver, 2026-11-25

Record: T-2078.
Deployed: version 0.2.0-beta.41.

- On the incus tier, where the server has a driver other than `dir`, or
  podvm does not see the directory of the pool (route A), podvm takes the
  free space of the pool of the default profile from incusd, refuses a run
  that cannot fit by name with exit 125, and names the pool "(driver
  `NAME`, as incusd measures it)" (experiment 472).
- On a pool of clones (btrfs, ZFS, thin LVM, Ceph), an import needs the
  bytes of the tree for the volume of the image, and the line after a
  failure names that volume; a create of an image that incus has is not
  refused by size.

### Changed: podcrates `v0.83.0`, the bounds of each answer of incusd, 2026-11-24

Record: T-2089.
Deployed: version 0.2.0-beta.41.

- On the incus tier, each silence of incusd ends after 30 s with one line
  that names the socket, for each call, not only the first.
- An exec that ends with 126 or 127 gives its code from pod-incus, with no
  second read of the operation of incusd.
- On a full pool, the error of tar comes from incusd with its text cut;
  podvm reads its words of a full disk and gives the line of the room of
  the pool, as before.
- A run of an image that an older podvm imported (no record of its need)
  is checked against the room of the pool with the size of its image file
  in incusd, a lower bound (T-2078).

## 0.2.0-beta.40 (2026-11-24)

### Known gaps: what the checks of 0.2.0-beta.40 prove, and what stays open, 2026-11-24

Record: T-1949, T-2054, T-2070, T-2078, T-2079, T-2085, T-2087.
Deployed: version 0.2.0-beta.40.

What the checks of this beta prove: the gate, `arm.yml`, the nightly
trial, the integration tests as uid 1000, `workloads.yml`, experiment 434
(the incus tier, with the new case of clause 27), experiment 471 on a
hosted ubuntu-24.04 runner with cgroup v2 and AppArmor (T-2086), and the
ninth outside pass with the binary and `podvm man` only (T-2087), whose
three P1 faults are fixed.

Each open fault that a user can meet:

- The incus tier: the import expands the hard links of an image (busybox
  takes 420 MiB in each instance); a payload can remove its own log
  (`logs` then gives a 404 of incusd); `exec -w` to a missing directory
  runs in `/`; `--podvm-init=image` of an image with no init gives a raw
  error of incusd and leaves the instance; `wait` and `kill` have no path
  of the tier; an instance that was created and not started reads
  `Stopped` (T-2079, OR-B39-08, 09, 11, 17; T-2087, OR-B40-04, 18).
- The incus tier: an image that an older podvm imported has no record of
  its need, so its run is not checked against the room of the pool
  (T-2078).
- Each call to incusd after the first of a verb has no time bound
  (#653 asks podcrates; T-2080).
- The native tiers: the edits of a payload to `/etc/hosts` go at the next
  `exec`; `run -it sh` and `exec -it CTR sh` have no job control; a
  foreground run on the namespace rung reaps no orphan until it ends
  (T-2079, OR-B39-15; T-2070, OR-B38-17; T-2087, OR-B40-05, 13).
- docker differences and texts of the outside passes: `login` contacts no
  registry, `--env-file` refuses a line with no `=`, `run -d IMAGE
  /nonexistent` exits 125, `rm -f` of a missing container exits 1, a name
  in use exits 1 from `create` and 125 from `run`, `run --rm` keeps the
  extracted tree of the image, `--network host`, `--format '{{json .}}'`,
  `table` and `index` are refused, a payload argument that is not UTF-8 is
  refused, `cp` refuses `..` and an absolute link of the container, `run
  -u` keeps the caller's privileges with no line, `--init` and `top` give
  a reason that is false on the namespace rung, and `doctor` calls the
  machine tier refused where `run` fetches its own QEMU (T-2070, T-2079,
  T-2087, OR-B40-06 to 17, and the P3 items of the ninth pass).
- A SIGTERM during the save of the state of a machine guest waits for the
  end of the save: at most about 7 minutes, an estimate (T-2048).

### Fixed: the three P1 faults of the ninth outside pass, and the pool for an image that incus has, 2026-11-24

Record: T-2087, T-2078; T-2079 (OR-B39-13).
Deployed: version 0.2.0-beta.40.

- `rmi` of one name of an image that a running container uses removes
  that name ("Untagged:"), where another name keeps the image, as docker
  does. The last name is refused with exit 1, the code of the exit table;
  before, each name was refused with exit 125 (OR-B40-01).
- `inspect` of a container that was created and not started gives
  `.Exec.Mode` `fresh-chroot` and `.Exec.Shares` `filesystem`, which is
  what `exec` does there; before, it gave `join` and the namespaces of a
  run (OR-B40-02).
- `PODVM_MODE=tmpfs`, `rundir` and `cache` run for a user with no root;
  before, each failed with exit 125 and a copy error of `dev/random`
  (OR-B40-03).
- On the incus tier, a run of an image that incus already has checks the
  room of the pool before the create, as an import does; before, a full
  pool gave incusd's raw error ("run out of disk space"). The need comes
  from a record that the import writes in the store; an image that an
  older podvm imported is not checked (OR-B39-13, T-2078).

### Fixed: a killed pull or load no longer leaves its staging file, 2026-11-24

Record: T-2069; T-2070 (OR-B38-05).
Deployed: version 0.2.0-beta.40.

- A `pull`, `load` or `import` that a SIGKILL ends leaves its staging
  file under `store/staging` for at most 10 s: the next command after that
  removes it. Before, it stayed for 10 minutes, and `system prune` and
  `image prune` left it (40 to 71 MiB measured).
- The name of a staging file holds 64 random bits beside the pid, so two
  writers in two PID namespaces do not take one name.

### Fixed: the usage of an ended incus instance on a host with AppArmor and cgroup v2, 2026-11-24

Record: T-2086; PodUtils/podrunner#350.
Deployed: version 0.2.0-beta.40.

- On a host with AppArmor (a hosted ubuntu-24.04 runner with incus of
  apt), `inspect --podvm-tier=incus` of a stopped instance gave a dash for
  `CpuSeconds` and `PeakMemoryBytes` and 0 for `OomKills`: the profile of
  incus refused the init's mount of the cgroup file systems. The init now
  takes the cgroup mounts that the instance has from its start. Measured
  on such a runner: a load that the shell counted at 13.22 s gives
  `13.270 136134656 0`, and an OOM kill of a child under `--memory 256m`
  gives `OomKills` 1.
- The peak is the high mark of the kernel for the cgroup of the instance
  (`memory.peak` of cgroup v2), not a sample: it counts the page cache
  and the kernel memory of the instance too.

### Fixed: the import of a large image keeps at most one full copy beside the upload on the incus tier, 2026-11-24

Record: T-2085, T-2078; PodUtils/podrunner#347 (ask 1).
Deployed: version 0.2.0-beta.40.

- `create` and `run` of the incus tier extract the tree of the image only
  for an import. After the upload, and also after a failed import, podvm
  removes the tree that the import extracted, where no other process of
  the store uses the image. A tree that was in the store before stays.
- Where incusd's images have less than three times the tree free, the
  upload is gzip: incusd keeps about a third of the tree. Before, one
  file system that held the store, incusd's images and the pool needed
  three copies of the tree (blobs 17 GB + 3 x 57 GB for an image of 85
  layers). Measured with a tree of 292 MB on a btrfs of 1000 MB: before,
  `create` failed with exit 125 at 838 MiB; now it passes at 394 MiB.
- The check of the room of a pool of the driver `dir` (T-2078) counts the
  new copies.
- A run of an image that incus has needs no tree in the store, so an
  absolute program that the image lacks is no longer refused before the
  start: podvm in the instance gives the same exit 127.

### Changed: podcrates `v0.82.1`, the processes of podvm in the PID namespace of a run, 2026-11-24

Record: T-2084, T-2054, T-2076.
Deployed: version 0.2.0-beta.40.

- The init of `run -d` on the namespace rung and the relay of `exec` are
  not dumpable, and the payload cannot read their memory, their
  environment or their namespaces as a user with no root. The process of
  podvm's own join (`podvm-exec`) is not dumpable either: before, a
  payload could read the variables of the caller of `exec` in its memory.
- `exec`, `exec -t` and `cp` into a run of a user with no root join the
  payload of the run, not its init. An `exec` of an older podvm does not
  join a run of this one.
- On the incus tier, `inspect` of a stopped instance takes the CPU time
  and the OOM kills from cgroup2 on Linux 5.8 to 5.18 too; the peak memory
  is then a dash, and `inspect --verbose` gives the reason.

### Fixed: the boot of `--podvm-init=image` on the incus tier, 2026-11-24

Record: T-2083.
Deployed: version 0.2.0-beta.40.

- `run -d --podvm-tier=incus --podvm-init=image` asks `systemctl
  is-system-running` again while it gives `offline`, `unknown`,
  `initializing` or `starting`, or while incusd refuses the exec of a
  running instance. Before, "offline" in the first second of the start
  ended the run with exit 125 and removed the instance
  (PodUtils/podrunner#349). The namespace rung and the runsc tier take
  "offline" and "unknown" with the same rule.
- `PODVM_INIT_BOOT_TIMEOUT` sets the bound of the boot, 1 to 3600 s (180 s
  with no value), on the incus tier, the namespace rung and the runsc tier.
- On `degraded`, podvm prints the failed units in one line and runs the
  command. Where the bound passes, the error gives each poll, and the
  instance stays, with the command that removes it.

## 0.2.0-beta.39 (2026-11-24)

### Known gaps: what the checks of 0.2.0-beta.39 prove, and what stays open, 2026-11-24

Record: T-1949, T-2054, T-2069, T-2070, T-2076, T-2077, T-2078, T-2079.
Deployed: version 0.2.0-beta.39.

What the checks of this beta prove: the gate, `arm.yml`, the integration
tests as uid 1000, `workloads.yml`, the nightly trial, experiment 434 (the
incus tier, with the new clauses 26 to 30), and the eighth outside pass with
the binary and `podvm man` only (T-2079), whose four P1 faults are fixed
(T-2080, T-2081, and the rule of `cp`), with seven of its P2 faults.

Each open fault that a user can meet:

- The incus tier: the import expands the hard links of an image (busybox
  takes 420 MiB in each instance); a payload can remove its own log
  (`logs` then gives a 404 of incusd); `exec -w` to a missing directory
  runs in `/`; `--podvm-init=image` of an image with no init gives a raw
  error of incusd and leaves the instance; the pool is not checked for an
  image that incus already has (T-2079, OR-B39-08, 09, 11, 13, 17; T-2078).
- The incus tier keeps three copies of a large image on one file system
  during `create` (the unpacked tree of the store, the upload of incusd, the
  root of the instance): a host needs room for about three times the tree
  (PodUtils/podrunner#347, ask 1).
- Each call to incusd after the first of a verb has no time bound
  (#653 asks podcrates; T-2080).
- The native tiers: the edits of a payload to `/etc/hosts` go at the next
  `exec`; `run -it sh` and `exec -it CTR sh` have no job control (T-2079,
  OR-B39-15; T-2070, OR-B38-17).
- A killed `pull` or `load` leaves a `.partial` file in `store/staging`
  (T-2069).
- On the namespace rung, pod-enter's init and join relay stay dumpable
  until podvm takes podcrates `v0.82.1` (#646; T-2054).
- docker differences and texts of the seventh and eighth outside passes:
  `login` contacts no registry, `probe` describes the probing process,
  `--env-file` refuses a line with no `=`, `run -d IMAGE /nonexistent`
  exits 125, `--tmpfs P:mode=` drops the mode, `rm -f` of a missing
  container exits 1 (T-2070, T-2079, OR-B38-08 to 18, OR-B39-19).
- A SIGTERM during the save of the state of a machine guest waits for the
  end of the save: at most about 7 minutes, an estimate (T-2048).

### Changed: the incus tier (the use after the end, the exit code under --memory, a full pool, 126 and 127, signals, one import), the rule of cp, 2026-11-24

Record: T-2076, T-2077, T-2078, T-2079, T-2080, T-2081, T-2082.
Deployed: version 0.2.0-beta.39.

- `podvm cp` puts a copy where `docker cp` puts it, in each direction and
  from an image: a directory onto an existing directory goes into it as
  DEST/basename(SRC) (before, its contents merged into DEST); `SRC/.`
  gives the contents. A destination whose parent directory is missing
  refuses the copy with exit 1; before, podvm made the parent. A missing
  source exits 1 and makes nothing (T-2079, OR-B39-04, OR-B39-14).
- `inspect --podvm-tier=incus` of a stopped instance gives `CpuSeconds`,
  `PeakMemoryBytes` and `OomKills`: the init writes them to `/.podvm/usage`
  after the command and each other process of the instance ended; before,
  each was a dash once the instance stopped. `inspect --verbose` says why a
  value is a dash (T-2076, PodUtils/podrunner#345).
- On the incus tier, the OOM killer of an instance no longer takes its
  init when a command passes `--memory`: the init sets its own
  `oom_score_adj` to -1000 (or, where the kernel refuses it, gives the
  command 1000) before the command starts. Before, in 12 of 44 runs, the
  instance stopped with no exit code and no usage for `inspect` (T-2077,
  PodUtils/podrunner#345).
- On the incus tier, an image that cannot fit in the storage pool of the
  default profile (a pool of the driver `dir`) is refused before the
  import, with one line that names the pool, its free space or inodes and
  the need of the image; below an estimate of the need, a warning. A
  create or an import whose answer goes over the bound of 64 MiB (on a
  full pool incusd puts the error of `tar` for each file into its answer)
  gives one more line with the free space of the pool and the need. Before,
  the only line was "the response body is larger than allowed" (T-2078,
  PodUtils/podrunner#346).

- On the incus tier, a command that ends with 126 or 127 gives its exit
  code and its output; before, `run` and `exec` gave "incus refused the
  call (400): Command not found", exit 125, and lost the output. A program
  that is not there gives docker's line "executable file not found in
  $PATH" and 127; an image of another architecture names both
  architectures (T-2080).
- On the incus tier, a SIGINT, SIGTERM or SIGHUP to a foreground `run` goes
  to the command ("forwarded SIGTERM to the payload in the instance"); a
  second signal or 10 s with no end sends SIGKILL, and the instance of the
  run is deleted. Before, podvm ended and the instance stayed running
  (T-2080).
- On the incus tier, a foreground `run` gives stdout and stderr as they
  come; `exec -i` gives stdin to the command, and `exec` has the
  environment and the working directory of the run; `run -w DIR` makes a
  missing DIR, as docker does; `run -d --rm` removes the instance when it
  stops; a foreground `--name` names the instance and refuses a name in use;
  the first call to an incusd that does not answer ends after 30 s;
  `container ls --podvm-tier incus` works as `ps` (T-2080).
- On the incus tier, parallel first runs of one image import it once: a run
  waits for the import of another run of the store, and a race with another
  store ends in the image that is there. Before, all runs but one ended with
  "Image with same fingerprint already exists" or a 409 of the alias
  (T-2081).

- On the incus tier, an import that incusd cuts (a full disk) gives
  incusd's error ("no space left on device") and the free space of incusd's
  images and of the pool with the need of the image, for each driver;
  before, podvm ended with SIGPIPE (exit 141) and no line (T-2082,
  PodUtils/podrunner#347).

## 0.2.0-beta.38 (2026-11-23)

### Known gaps: what the checks of 0.2.0-beta.38 prove, and what stays open, 2026-11-23

Record: T-1949, T-2048, T-2054, T-2065, T-2066, T-2068, T-2069, T-2070.
Deployed: version 0.2.0-beta.38.

What the checks of this beta prove: the gate, `arm.yml`, the integration
tests as uid 1000, `workloads.yml`, the nightly trial, `kvm.yml` (job
`init`, on the take of podcrates `v0.82.0` and on the fixes of the sixth
outside pass), and the seventh outside pass with the binary and `podvm man`
only (T-2070), whose two P1 faults and three P2 faults are fixed (T-2071 to
T-2075). Experiment 465 did not run on this beta (the QEMU of the cloud
host crashes under TCG with four vCPUs, T-2016).

Each open fault that a user can meet:

- `run -it sh` and `exec -it CTR sh` have no job control ("can't access
  tty; job control turned off"): the pty is not the controlling terminal of
  the payload (T-2070, OR-B38-17; #584, COORD-188, asks pod-enter for it).
- On the namespace rung, pod-enter's init of a PID namespace and the relay
  of an `exec` join stay dumpable, and the relay of `exec -t` as uid 0 keeps
  podvm's argv (T-2054, OQ-08; #646 asks podcrates).
- A killed `pull` or `load` leaves a `.partial` file in `store/staging` that
  no verb removes and that `system df` does not count (T-2069, OR-B38-05).
- `cp -r` onto an existing copy of a tree with symbolic links stops half way
  (T-2070, OR-B38-07).
- `login` prints "Login Succeeded" for a registry that it did not contact;
  the registry checks the login at the next pull (T-2070, OR-B38-08).
- `probe` describes the process that probes (mount only, no PID or user
  isolation, no /proc), not the namespace rung that `run` enters; `run
  --verbose` and `system info` name the entered mechanism (T-2070,
  OR-B38-10).
- docker differences: `--env-file` refuses a line with no `=` and keeps a
  UTF-8 BOM; `run -d IMAGE /nonexistent` exits 125 (docker: 127); `run` with
  no `-i` passes stdin; `run -it` with a stdin that is no terminal is
  accepted; `cp` makes missing parents of a file; `--tmpfs P:mode=` drops
  the mode; `-h` is the help of each verb, not `--hostname` (T-2070,
  OR-B38-09, 11 to 14, 16).
- Some texts are wrong or odd: `cp` names paths of the store, `system df
  --verbose`, `podvm help run`, `rmi -f` with stopped containers, `pull -a`
  stops at a schema-1 tag, the `/proc/mounts` of `--read-only` (T-2070,
  OR-B38-14, OR-B38-18).
- With `-v DIR:/etc`, the payload gets no `hosts`, `hostname` or
  `resolv.conf` of podvm: podvm writes no file into a volume, and docker
  binds its own files over such a volume (T-2072, `docs/limits.md`).
- The image of the incus tier keeps only `security.capability` and `user.*`
  extended attributes, so the ACLs of a layer do not reach an incus
  instance; a layer with GNU sparse files is refused (T-2065).
- `load` of an image of 85 layers is measured with small layers only; the
  hosted image of podrunner (about 16 GiB) is not measured here (T-2068).
- A SIGTERM that comes during the save of the state of a machine guest
  waits for the end of the save: at most about 7 minutes, an estimate from
  the bounds (T-2048).

### Changed: podcrates v0.81.0 and v0.82.0, a load of many layers, the faults of the sixth and seventh outside passes, 2026-11-23

Record: T-2046, T-2047, T-2050, T-2051, T-2052, T-2053, T-2054, T-2055, T-2060, T-2061, T-2062, T-2063, T-2064, T-2065, T-2066, T-2067, T-2068, T-2071, T-2072, T-2073, T-2074, T-2075.
Deployed: version 0.2.0-beta.38.

- On the machine tier, podvm sizes the memory of a guest with no
  `--podvm-mem` from 13/4 of its initrd and 512 MiB, not three times and
  512 MiB: under KVM an initrd of 2055 MiB needs 6656 MiB, one step under
  the old estimate (T-2047, experiment 470, `kvm.yml` run 44854125431).
- podvm takes podcrates `v0.80.0` (PodUtils/podcrates#250). `pod-enter` writes the write table of the confine layer (`PODVM_WRITE_HOST`) for the entry with no chroot, and each entry removes a value of the caller (podcrates@T-999). The payload of each entry starts with an empty signal mask (podcrates task 1010).
- podvm no longer writes the write table of the confine layer itself: `pod-enter` writes it from the spec of the plan (`podvm_supervise::table::set_write_table` is gone; T-2050).
- The open of a volume under `--podvm-volume-base` is the crate `pod-beneath` of podcrates on each route (the namespace rung, the syscall tier and the interposer of the loader family), in place of three copies of podvm; each open tries `EAGAIN` again (T-2050, #611).
- A chain of five `#!` scripts runs on the userland rung, the syscall tier and through the interposer, as the kernel runs it; a sixth level is `ELOOP` (T-2050). Before, podvm refused a chain of five.
- Under `--podvm-confine exec=`, the first program of a run on the userland rung takes `pod_enter::check_exec_guest`: each `#!` interpreter of its chain and each program of a dynamic loader take the rule (T-2050, #613). The rule of the program of a dynamic loader is `pod_confine::loader` on each route; glibc's `libc.so.6` is no loader.
- Each thread of a relay of a foreground run blocks the one list of forwarded signals of podcrates (`pod_enter::block_forwarded`); on aarch64 the relay of `-t` no longer calls a wrong system call to block them (T-2050, #616).
- The end of the tree of a detached run whose payload holds the init of a PID namespace and a second process of it takes milliseconds, not 5 s (`pod-supervise` of podcrates `v0.80.0`; T-2018, #624).
- podvm takes podcrates `v0.81.0` (PodUtils/podcrates#254; T-2066).
- podvm takes podcrates `v0.82.0` (T-2066). A layer of GNU tar `--acls` extracts: each ACL in a PAX record `SCHILY.acl.access` or `SCHILY.acl.default`, one entry on each line, is set as `system.posix_acl_access` or `system.posix_acl_default`, or the sidecar names it as not set, and the mode of each entry stays (podcrates task 1086; T-2065, #643, PodUtils/podrunner#339). Before, the newline byte refused the layer whole.
- A layer that holds a GNU sparse file (`GNU.sparse.*` records) is refused: the `tar` reader gives its data with no holes, under another name (podcrates task 1087).
- An `execve` that finds the program and fails with `E2BIG`, `EINVAL` or `ELIBBAD` exits 126, as with `EPERM`, `ENOEXEC`, `EACCES`, `EISDIR` and `ETXTBSY`; a refused file on `PATH` is the answer also when a later directory has no such file (`pod-enter`, podcrates task 1080, #639). podvm no longer reads the text of the error for E2BIG (T-2062).
- The save of the state of a Linux guest of the machine tier is `pod_guest::warm::line` of podcrates: one connection to the monitor of the emulator, whose connect has a bound (#633), the late answer of `query-migrate` read on it, the pipe after a refused `file:` save, and the guest goes on after each failure. The record of a state is the one of the crate (`STATE.record`); a state of the old layout is not loaded, and the guest boots once and saves a new state. A failed save names the first transport, the cause, the free space and the monitor in one line (T-2051, T-2048).
- The store copy of QEMU for a host with none is the static QEMU 11.1.3 of PodUtils/static-pkgs (release `qemu-11.1.3`, the service `static-pkgs-qemu` of `pod-services`), in place of QEMU 9.2.0 of pkgforge. It holds the fix of the use-after-free of `address_space_translate_for_iotlb` (T-2016, #618) and the migration of an NVMe device, so `podvm windows doctor` passes its check of a warm start (QEMU 11.1 or later, podcrates task 1035) with it. podvm checks each archive and each member that it takes by its size and sha256.
- `logs --podvm-tier=incus` reads the log in pieces of 64 KiB from the offset that it wrote last (`Client::get_file_to` of `pod-incus`), so a log larger than 64 MiB is no longer refused. `logs` takes the stream of a log of `pod-supervise`, and each read of a log refuses a symbolic link at its place (T-2046, #631).
- Each atomic replace of a file of podvm (the report of a run and of a container, the hold state of an init, the record of a machine, the probe cache, the cache of the completion, a pulled base) is `pod_core::files::replace`: a temporary file with a name of its own that goes away on each failure, mode 0644, a sync of the file and of its directory, and an error that names the step and the path (T-2066).
- `inspect --format` gives each field its docker type: `{{json .Config.Env}}` is a list, `{{.Config.Cmd}}` prints `[python3]`, `{{json .Labels}}` is a map, and `Pid`, `ExitCode` and the counts of `End` are numbers. Plain `inspect` and `--format` read one model of the fields. A container has docker's `Config` (T-2060, OQ-06).
- `run -t` sets `TERM=xterm` where neither the image nor `-e` sets `TERM`. `exec` into a container that exited or died is refused with exit 1. `logs --tail all` is the whole log. `-v H:G:z` and `:Z` are accepted and do nothing. `start -a`, `cp NAME:PATH -` and the `HOSTNAME` of a run with no `--hostname` are named as known differences (T-2061, OQ-09).
- An E2BIG of `execve` exits 126. `rmi` of a referenced image, a refused `cp`, and a `load` or `import` that refuses its archive exit 1, as the manual says (T-2062, OQ-10).
- A foreground run holds a lock on its memo. A SIGKILL of podvm no longer leaves `store/staging/memo-*.tmp`: the next command that opens the store, and `container prune`, remove the memo of a run that has ended (T-2063, OQ-12).
- Texts: `--timeout` names its bound, `--report` says that its directory does not exist, `--env-file` names the line of bytes that are not UTF-8, the lines of a forwarded signal and of a signal exit start with `podvm:`, the manual shows the usage of `container prune` and says that `--read-only` works on the namespace rung (T-2064, OQ-13).
- The interrupt key (`^C`) and the quit key of `run -it` and `exec -it` go to each process of the payload's tree in its process group, as a terminal sends them: `sh -c 'sleep 6'` ends at once, not after the `sleep` (T-2052, OQ-05 of the sixth outside pass). Before, only the pid of the payload got SIGINT, and the quit key went to the payload as a byte.
- On the namespace rung, the banner of `--verbose`, `--strict` and `inspect` say what the run opened, from one table (`podvm man`, ISOLATION): a foreground run of a caller that is not uid 0 shows `namespaces: user+pid+uts+mount` and `pids: private`, and the banner no longer says "no /proc"; `--strict` no longer warns of the mtab of `/proc`; `.Exec.Mode` and `.Exec.Shares` of a container of the rung give `join` and the namespaces that `exec` joins (T-2053, OQ-07 of the sixth outside pass, #557).
- On the namespace rung, the payload no longer reads podvm's argv (the values of `-e`) or its environment in `/proc`: pid 1 and pid 2 of a foreground run, the init of `run -d` and the process of `exec` show `podvm-init`, `podvm-run` or `podvm-exec` and no variable (`prctl(PR_SET_MM_MAP)`, no capability). The payload gets no descriptor of the image lock where a PID namespace holds its tree, and no `PODVM_MEMO_PATH` or `PODVM_DISPATCH_SELF` in an entry with a root (T-2054, OQ-08 of the sixth outside pass; partial: the payload is not pid 1, and the variables of the interposer stay).
- `--podvm-volume-base` on the loader family: a link in a volume, also one that the host makes after the start, resolves in guest names, as on the namespace rung, so a link to `/etc` reads the image's `/etc`; `ELOOP` is the answer for a link that the host makes between that walk and the call. The manual gives this rule since T-2040, and a test of the interposer holds it (T-2055, OQ-11 of the sixth outside pass).
- Where the kernel refuses `prctl(PR_SET_MM_MAP)` (no `CONFIG_CHECKPOINT_RESTORE`), pid 1 of podvm's user namespace writes NUL over its own command line and environment, and `--verbose` says that `/proc/2` still shows them; the launcher of `run -d` gives back its own name after the fork of the init (T-2054).
- On the machine tier, podvm puts the sockets of a run under `/tmp` where their path under `TMPDIR` passes the 108 bytes of `sun_path`; before, a `TMPDIR` of 73 bytes or more stopped the run with "AF_UNIX path too long" (T-2067).
- `load` of an image of 14 layers or more (13 from stdin) no longer exits 125 with "this process already holds 16 locks": the load commits each blob as soon as its bytes are written and holds one staging lock at a time, as `pull` does. `pull`, `load` and `import` write the record only where each blob that it names is in the store; where an `rmi` or a prune of another image removed a shared blob during the command, the command names the blob and writes no record (T-2068, PodUtils/podrunner#344).
- Tests: the test of the path of `--report` and each stand-in with a unix socket use a new directory of their own, and a socket path that fits `sun_path`, so they pass as uid 1000 with a long `TMPDIR` (T-2067).
- `exec -it CTR sh` (and `sh -i`, `set -m`) into a `run -d` container of a caller with no root takes each line typed: the payload no longer shares podvm's session, so a job-control shell cannot take the caller's terminal from the relay of the pty (T-2071, OR-B38-01 of the seventh outside pass; also in `v0.2.0-beta.37`).
- A foreground run of a caller with no root writes no fixup of podvm (`passwd`, `hosts`, `resolv.conf`, the CA bundle, the device stand-ins) through a volume into the caller's directory: `-v DIR:/etc` and `-v DIR:/dev` leave DIR as it was, and `--verbose` names each fixup that a volume covers (T-2072, OR-B38-06).
- A foreground `run` whose record fails gives its cause (a name in use, a container table that does not parse) and no longer the owner of a store root of another uid that it may write; the refusal of a `containers.json` that does not parse says to move it aside (T-2073, OR-B38-02).
- `--rm` of `run -d` and of `create` removes the container, its record and its tree at the end of its run, as docker does; `wait` still gets the code (T-2074, OR-B38-04). Before, podvm took the flag and kept the container.
- `exec` into a running container keeps the `/etc/hosts` of the run: the lines of `--add-host` and of `--hostname` stay, for `exec` and for the payload (T-2075, OR-B38-03).

## 0.2.0-beta.37 (2026-11-21)

### Known gaps: what the checks of 0.2.0-beta.37 prove, and what stays open, 2026-11-21

Record: T-1949, T-2002, T-2008, T-2016, T-2044, T-2046, T-2048.
Deployed: version 0.2.0-beta.37.

What the checks of this beta prove: the gate, `arm.yml`, the integration
tests as uid 1000, `workloads.yml`, the nightly trial, `kvm.yml` (experiment
468: the saved state of a guest under KVM; experiment 469), experiments 434
and 465, and the sixth outside pass with the binary and `podvm man` only
(T-2044), whose one P0 and three P1 faults are fixed (T-2040 to T-2043), with
T-2045 that the work found beside them.

Each open fault that a user can meet:

- `run -it`: a Ctrl-C does not stop a child of the payload (T-2044, OQ-05).
- `inspect --format` gives lists and maps as flat strings, and the JSON of
  a container gives `Labels`, `ExitCode` and `Pid` as strings (T-2044,
  OQ-06).
- On the namespace rung, the banner of `--verbose` names no /proc and no PID
  isolation beside the PID namespace that it opens; podvm's process is
  PID 1 of the payload with its argv; the payload inherits descriptors of
  the store (T-2044, OQ-07, OQ-08).
- docker differences that the manual does not name: `TERM` of `-t`,
  `HOSTNAME`, `exec` into an exited container, `logs --tail all`, `start
  -a`, `cp CONTAINER:path -` (T-2044, OQ-09).
- Exit codes: an E2BIG at `execve` gives 127, and the refusals of `rmi` of
  a referenced image, of `load` and `import` of a file of the caller, and
  of `cp` with `..` give 125 where the manual says 1 (T-2044, OQ-10).
- A SIGKILL of podvm leaves a file `store/staging/memo-*.tmp`, and some
  texts of the manual and of messages are wrong (T-2044, OQ-12, OQ-13).
- The saved state of a guest under KVM holds on a hosted runner (`kvm.yml`
  run 44808044333: the run that saves took 19.5 s, the run from the state
  3.4 s, the first byte of its command after 0.6 s). A SIGTERM that comes
  during a save waits for the end of the save: at most about 7 minutes, an
  estimate from the bounds (T-2048).
- `logs --podvm-tier=incus` holds the log in memory, 64 MiB at most (a
  larger log is refused), and refuses `--tail` (T-2046).
- The pinned static QEMU 9.2.0 is older than the upstream fix of a crash of
  a guest with more than one vCPU under TCG; podvm boots once more after
  such a crash (T-2016).
- With `--podvm-init=image`, a caller that is uid 0 gets no namespace
  rung, and `run -d` is refused on the runsc and machine tiers (T-2002).
- The faults of `0.2.0-beta.36`'s known gaps stay open where this list
  does not name them as fixed.

### Fixed: the faults of the sixth outside pass, the confine layer, the signals of a run, 2026-11-20

Record: T-1984, T-2001, T-2002, T-2006, T-2011, T-2012, T-2013, T-2014, T-2015, T-2016, T-2040, T-2041, T-2042, T-2043, T-2045, T-2048.
Deployed: version 0.2.0-beta.37.

- On the machine tier, a run of `--podvm-init=image` that saves the state of
  its guest no longer hangs with no line when the monitor of the emulator
  answers late (under KVM, while QEMU writes the last part of the state).
  podvm opened a second connection to the monitor while the first one was
  open; QEMU takes one client at a time, and the third connection waited
  with no bound, also past a SIGTERM. podvm now closes the first connection,
  and each connection has a bound: the save holds, or one line says why and
  the run boots a guest with no state (T-2048, `kvm.yml` runs 44786700578
  and 44808044333). Experiments 465 (clause 9) and 468 now fail where a run
  of the machine tier does not run there or gives no line.
- On the userland rung (the loader family), a path that names the host
  path of the rootfs (`PODVM_MAPS` gives it) resolves its links from the
  rootfs, and `..` stops there. Before, a link to `ROOTFS/../../etc`, or
  `ROOTFS/tmp/l/f` with a relative link `l` that climbs out, read a host
  file (T-2045). An absolute link of the image under such a path resolves
  in the image now, not on the host.
- On the userland rung (the loader family), a write through the host path
  of a `:ro` volume (`echo X > HOST/f`, also `chmod`, `touch`, `mv`, `rm`,
  `ln`, `mkstemp`, or a link to the volume by its host or its guest path)
  fails with `EROFS`, as a write through the guest path did. Before, it
  changed the host file (T-2040, OQ-01 of the sixth outside pass). A host
  path of a volume now resolves its links in guest names, and a host path
  of the rootfs takes `--read-only` by its guest path.
- `link`, `rename` and their `*at` forms give the errno of their first
  path when it fails; before, the second path could replace it (`EINVAL`
  for `EROFS`).
- On the machine tier, a guest whose emulator crashes (SIGSEGV, SIGBUS,
  SIGABRT) before the command runs boots once more, with one line that
  names the signal (T-2016: QEMU 8.2 under TCG with more than one vCPU).
- A SIGQUIT, SIGUSR1, SIGUSR2, SIGALRM or SIGPWR to a foreground `run` is
  forwarded to the payload, as SIGHUP, SIGINT and SIGTERM were; before, it
  ended podvm and left the payload and a record `Dead` (T-2001, OP-05;
  podcrates `v0.79.0`).
- `ld-musl-ARCH.so.1`, a link to musl's libc, counts as a dynamic loader
  for `--podvm-confine exec=` (T-1984).
- `--podvm-confine exec=` holds for the first program of a run on the
  userland rung and the syscall tier, also through the dynamic loader and
  for the interpreter of a `#!` script: a program out of `exec=` exits 126
  with one line, as on the namespace rung (T-2013). Before, the first
  program ran.
- `exec` into a `run -d` container of the namespace rung enters by the
  join of its namespaces, where its volumes are, also where the probe of
  the host denies chroot to the user; before, it took the entry with no
  chroot and refused each volume (T-2012, PodUtils/podrunner#326).
- `run --podvm-init=image` of an image whose Entrypoint is its init
  (`/sbin/init`, as podrunner's hosted snapshot declares) runs the command
  beside the init; before, the instance ran `/sbin/init CMD` as a second
  process, which printed "Too many arguments." and ran nothing (T-2011,
  PodUtils/podrunner#325). An explicit `--entrypoint` stays the caller's. A
  boot that does not come up names the failed units of systemd.
- `inspect` of a container gives docker's `.State.Status`,
  `.State.Running`, `.State.ExitCode`, `.State.Pid`, `.State.StartedAt`
  and `.State.FinishedAt`, and its JSON gives `State` as docker's object
  (T-2001, OP-06). `--format '{{.State}}'` stays the word.
- `podvm man` has an ENVIRONMENT section: each variable that a user can
  set, its values and its effect, from one table (T-2001, OP-01). The
  `probes` line of `--verbose` says that its rows are of the host outside
  a user namespace.
- The help of `--podvm-tier` states the cap of `/tmp` of the syscall tier:
  256 MiB, set by `--tmpfs /tmp:size=N` (T-2001, OP-07).
- A run makes the mount point of each volume in its own tree, so `ls /`
  names a volume whose guest path the image lacks (T-2001).
- A SIGKILL of podvm during a foreground run of a user with no root ends
  each process of the run; before, the payload stayed (T-2006).
- On the syscall tier, a `-v` of one file works: the path is the file,
  `:ro` holds, a path under it is ENOTDIR, and `rm` or `mv` of it is
  EBUSY, as for a bound file (T-2001, OP-02 of the fifth outside pass).
- On the syscall tier, `npm` and `npx` run: a program that sets its process
  title (Node's `process.title`) no longer writes past its stack (T-2001,
  OP-03 of the fifth outside pass).
- `exec -t` gives the command a terminal, as `run -t` does: in a running
  container of the namespace rung a pty of its own devpts, elsewhere a
  pty of the host; it refuses by name where `/dev/ptmx` is unusable and
  in a guest (T-2043, OQ-04 of the sixth outside pass).
- On the machine tier, a SIGTERM, SIGINT or SIGHUP to a foreground run
  goes to the command in the guest, and stderr names the forward; before,
  it ended podvm and the guest's trap never ran (T-2041, OQ-02).
- After `run -d --podvm-tier=machine`, `inspect`, `kill`, `wait` and the
  filters and the format of `ps` find the container by its name or its id
  (T-2042, OQ-03).
- On the rungs with no chroot, `--podvm-confine read=/` covers a volume,
  and on the syscall tier each interpreter of a `#!` chain takes the rule
  of `exec=` (T-2014, T-2015).
- A SIGTERM of a foreground run of `--podvm-init=image` on the namespace
  rung reaches the command, and `rm -f` of such a container no longer
  waits 5 s (T-2002).

### Added: systemd as PID 1 on each route, the saved state of a guest, the volume base on the loader family, 2026-11-20

Record: T-1969, T-1972, T-1984, T-1985, T-1996, T-1997, T-1999, T-2002, T-2004, T-2007, T-2008, T-2010, T-2046.
Deployed: version 0.2.0-beta.37.

- `logs` writes the log through one buffer of 64 KiB, not a copy of the
  whole log in memory (a log of 200 MB: 4,952 KiB of peak memory, before
  200,160 KiB), and `logs --limit-bytes N` stops after the first N bytes
  with exit 0, also with `--tail`, `-f` and `--podvm-tier=incus` (T-2046,
  PodUtils/podrunner#333).
- The command of `run --podvm-init=image --user NAME` on the namespace
  rung gets the supplementary groups that `/etc/group` of the image gives
  the user (T-2002); before, it had the primary group only. On the machine
  tier, an argument of the run that holds `"$@"` (`sh -c 'exec "$@"'`)
  reaches the command as it is.
- The machine tier saves the state of a guest that booted the init of an
  image (`--podvm-init=image`), and each later run of the image and guest
  shape starts from that state (T-2008): under TCG on the cloud host the
  first output of a run with MariaDB and PostgreSQL under systemd comes
  after 1.3 s to 1.6 s, against 19 s to 20 s for a boot. A state that is
  missing, stale or does not load gives a boot. `PODVM_MACHINE_STATE=0`
  turns the save off; [the limits](docs/limits.md#the-init-of-an-image)
  name what each run of one state shares.
- The run report of a foreground run of the incus tier has `oom_kills`,
  read from the metrics of incusd (T-1969, PodUtils/podrunner#296 ask 6).
- `--podvm-confine` on the userland rung (T-1984, T-1985): an exec of the
  dynamic loader (`ld.so PROGRAM`, each name and link of the loader) takes
  the `exec=` rule of its program, and a change of mode, owner or times
  (`chmod`, `chown`, `utimes` and their kin, also through a link or a
  descriptor) needs a `write=` path. The interposer of the loader family
  and the handler of the native-static rung and the syscall tier answer
  `EACCES` with one line; Landlock checks neither, so on the namespace and
  chroot rungs the advice of a `:ro` volume stays. Cost with the layer: about
  2.5 microseconds for each change of mode, owner or times on the loader
  family, 1.9 on the syscall tier (an estimate).
- `--podvm-volume-base DIR` holds on the loader family of the userland
  rung (T-1972, PodUtils/podrunner#300 ask 3): each volume under DIR must
  open under it as a directory with no symbolic link at the start (else
  exit 125), and the interposer opens the volume and the rest of each
  path under DIR again at each path call, with `openat2(RESOLVE_BENEATH |
  RESOLVE_NO_SYMLINKS)`, so that a link that the host makes after the
  start gives `ELOOP`, not a file out of DIR. Cost: about 3 microseconds for each
  path call in such a volume.
- A run of `--podvm-init=image` waits for the boot with `systemctl
  is-system-running --wait` and no poll, and where nothing of the run can
  stay (each volume read-only, and `--rm` on the namespace rung) the init
  ends at once when the command ends (T-2010): 8.0 s to 7.4 s on the runsc
  tier for MariaDB and PostgreSQL under systemd. `PODVM_RUNSC_ARGS` gives
  more flags of `runsc`.
- `run --podvm-init=image` holds on the runsc tier (T-2007): the init of
  the image is PID 1 of a gVisor sandbox, with a cgroup2 tree and each uid
  from gVisor's kernel, so the host needs no delegated cgroup and no id
  range. A run with no tier takes it before a guest of the machine tier
  where `runsc` is there (call C26): 10 s against 46 s under TCG on the
  cloud host for MariaDB and PostgreSQL under systemd.
- `run --podvm-init=image` holds on a foreground run of a user with no
  root (T-2002): the init of the image (systemd) is PID 1 of a PID
  namespace, the command runs after `systemctl is-system-running` gives
  `running` or `degraded`, and a unit with `User=` runs as its own uid. On
  the namespace rung where the host gives the caller a subordinate id
  range and a delegated cgroup2 subtree; else, and from the userland rung,
  in a guest of the machine tier (KVM or TCG), with one line that names the
  missing part (call C26). Where a user manager of systemd answers, podvm
  asks it for the delegated subtree (a scope with `Delegate=yes`, T-2004).
- The machine tier boots an image with a file that its owner cannot read
  (a helper of mode 4110): the archive of the guest reads it with the
  owner's bits (T-2002).
- The wait for the boot of `--podvm-init=image` on the incus tier reads the
  bus error of systemd 262 as "not up yet" (T-2002).
- On the incus tier, `--podvm-init=image` gives the instance an isolated
  map of 231,072 ids, `/dev/fuse` and an empty machine-id for the hosted
  snapshot (T-1996), and the image goes to incusd through a pipe, with no
  tarball file in the store (T-1997, PodUtils/podrunner#316).
- On the incus tier, the tarball of the image carries `security.capability`
  and `user.*` as PAX records, so a file capability reaches the root of the
  instance (T-1999).
- `run -d`, `create`, `start`, `stop`, `rm -f` and `exec` (with `-t`,
  `-u`, `-e`, `-w`) hold with `--podvm-init=image` on the namespace rung,
  for a caller that is not uid 0 with a subordinate id range and a
  delegated cgroup2 subtree; `run -t` and `exec -t` of such a run get a pty
  of the devpts of the init (T-2002).

### Changed: podcrates v0.79.0, the refusals of an init, the id of run -d of a machine, 2026-11-20

Record: T-2010, T-2017, T-2042.
Deployed: version 0.2.0-beta.37.

- podcrates `v0.79.0`.
- podcrates `v0.78.1` (with `v0.77.0`).
- podvm takes podcrates `v0.75.0` (PodUtils/podcrates#238) and `v0.76.0`
  (PodUtils/podcrates#239: `pod-extract` sets the extended attributes of a
  layer and names each one that it cannot set).
- A guest under TCG takes a translation cache of 512 MiB in place of
  QEMU's 1 GiB (T-2010): the peak memory of a run with MariaDB and
  PostgreSQL under systemd goes from 3,670 MiB to about 3,300 MiB on the
  cloud host, and its time does not change.
- With `--podvm-init=image` on the namespace rung, `--podvm-confine`,
  `--ulimit`, `--security-opt no-new-privileges` and `--device` refuse by
  name with 125, and so does `run -d` on a host with no delegated cgroup2
  subtree; before, the route dropped the flags with no line (T-2017, call
  C28).
- `run -d --podvm-tier=machine` and `run -d windows:NAME` print the id of
  the container (64 hex digits), as docker does, in place of its name; each
  verb takes the id, a prefix of 12 digits, or the name (T-2042).

## 0.2.0-beta.36 (2026-11-19)

### Known gaps: what the checks of 0.2.0-beta.36 prove, and what stays open, 2026-11-19

Record: T-1949, T-1991, T-2001.
Deployed: version 0.2.0-beta.36.

What the checks of this beta prove: the gate, the integration tests as uid
1000, `workloads.yml`, `arm.yml`, the nightly trial, experiment 434 of the
incus tier (clause 19: systemd as PID 1), and an outside pass with the
binary and `podvm man` only (T-2001), whose one P1 fault is fixed (T-2000).

Each open fault that a user can meet:

- On the syscall tier, a `-v` of one file is refused with "errno 20", and
  `npm` and `npx` of `node:22-slim` end with SIGSEGV while `node` runs
  (T-2001, OP-02 and OP-03). Use a `-v` of a directory, or no tier.
- A SIGUSR1 or SIGHUP to a foreground `podvm run` ends podvm, which
  forwards only SIGINT and SIGTERM, and leaves a record `Dead` also with
  `--rm` (T-2001, OP-05); `container prune` removes it.
- `inspect` of a container names the exit code `.ExitCode`, not
  `.State.ExitCode` (T-2001, OP-06).
- On the incus tier the exit code of an instance is read with `inspect`;
  the tier serves no `wait` (T-1722).
- The hang of `cargo doc` on the syscall tier that podrunner saw is not
  reproduced here (T-1991).
- The faults of `0.2.0-beta.35`'s known gaps stay open where this list
  does not name them as fixed.

### Fixed: Go on the userland rung, apt-get install, the image of the incus tier, 2026-11-19

Record: T-1990, T-1993, T-1995, T-2000.
Deployed: version 0.2.0-beta.36.

- On the syscall tier, a path walk that a rename or a mount on the host
  raced is tried again, as openat2(2) asks, in place of "errno 11" at the
  start of a dynamic program (T-1990, PodUtils/podrunner#308).
- The image of the incus tier keeps a file or a directory of mode 0000
  (`/etc/shadow` of ubi-init) in place of a failure with EACCES (T-1993).
- On the userland rung and the syscall tier, a signal that stops a system
  call of a static program runs its handler in the program, after the
  call, on the program's own signal stack. A Go program (`tsc` of
  TypeScript 7) hung at its first signal of preemption (T-1995,
  PodUtils/podrunner#315).
- On the userland rung, `apt-get install` as `-u 0` works: a process
  whose parent closed the descriptor of the ownership memo (apt marks
  each one close-on-exec before it runs dpkg) opens the memo again from
  its path, so dpkg's `tar` can set the owners (T-2000, found by the
  fifth outside pass).

### Added: systemd as PID 1 on the incus tier, the image on ghcr.io, 2026-11-19

Record: T-1992, T-1993.
Deployed: version 0.2.0-beta.36.

- `--podvm-init=image` with `run -d` or `create` on the incus tier boots
  the init of the image (systemd) as PID 1. The command runs after
  `systemctl is-system-running` gives running or degraded, with its log and
  its exit code, and the instance stops when it ends (T-1993,
  PodUtils/podrunner#313).
- Each release publishes `ghcr.io/podutils/podvm:TAG`: one image `FROM
  scratch` for each architecture with the static binary, under one index
  that is attested in the registry; the notes name each digest beside the
  SHA-256 of its asset (T-1992).

### Changed: podcrates v0.74.0, 2026-11-19

Record: T-1955.
Deployed: version 0.2.0-beta.36.

- podvm takes podcrates `v0.73.0` and `v0.74.0` (PodUtils/podcrates#231,
  #237).

## 0.2.0-beta.35 (2026-11-18)

### Known gaps: what the checks of 0.2.0-beta.35 prove, and what stays open, 2026-11-18

Record: T-1949, T-1984, T-1985, T-1989.
Deployed: version 0.2.0-beta.35.

What the checks of this beta prove: the gate, the integration tests as uid
1000, `workloads.yml`, `arm.yml`, the nightly trial and an outside pass with
the binary and `podvm man` only (T-1989), whose P1 faults are fixed (T-1983,
T-1986, T-1987, T-1988) or named in the manual (T-1984, T-1985, call C25).

Each open fault that a user can meet:

- `--podvm-confine` does not check a program that the dynamic loader runs
  (`ld.so PROGRAM`), nor a change of mode, owner or times of a file in
  reach (T-1984, T-1985): give a `-v` that must stay as it is `:ro`.
- The P2 and P3 faults of the third and the fourth outside pass (T-1980,
  T-1989): `..` after a link on the userland rung and at the root of a
  volume on the syscall tier; wrong exit codes of `system abi`, `load`,
  `import`, `profile show` and `cp`; a refused run of the syscall tier
  leaves `tmpfs/PID/0` in the store.
- The faults of `0.2.0-beta.34`'s known gaps stay open where this list does
  not name them as fixed; T-1981 (Fedora 42) is fixed.

### Fixed: the confine layer, Fedora 42 and AlmaLinux 9, 2026-11-18

Record: T-1981, T-1982, T-1983, T-1986, T-1987, T-1988.
Deployed: version 0.2.0-beta.35.

- On the syscall tier, `--podvm-confine exec=` holds: the handler checks
  each exec against the exec table of the layer, as the interposer does,
  and a payload can neither drop nor widen the table (T-1983).
- On the userland rung, the home of the run is writable to the payload, as
  to uid 0 of the image: `dnf` of Fedora 42 runs (T-1986).
- On the syscall tier, a `-v` volume whose guest path the image does not
  have can be `stat`ed: `ls -d`, `stat`, `cp -r` and `find` of it work
  (T-1988).
- On AlmaLinux 9, `ls -l` no longer dies with SIGSEGV on the namespace and
  the userland rung: the interposer makes the system call of each
  attribute call itself (T-1987).
- On a host with no `chroot(2)` and no user namespace, a program whose
  `PT_INTERP` lies past its first 64 KiB runs: the shells of
  `quay.io/fedora/fedora:42` (T-1981; podcrates `v0.72.0`).
- `run -d --podvm-tier=syscall --podvm-confine SPEC` starts its payload:
  the launcher gives podvm's own binary read and execute in the layer, as
  a foreground run does (T-1982, PodUtils/podrunner#304).

### Changed: podcrates v0.72.0, 2026-11-18

Record: T-1981.
Deployed: version 0.2.0-beta.35.

- podvm takes podcrates `v0.72.0` (PodUtils/podcrates#229).

## 0.2.0-beta.34 (2026-11-18)

### Known gaps: what the checks of 0.2.0-beta.34 prove, and what stays open, 2026-11-18

Record: T-1949, T-1980, T-1981.
Deployed: version 0.2.0-beta.34.

What the checks of this beta prove: the gate, the integration tests as uid
1000, `workloads.yml`, `arm.yml`, the nightly trial and an outside pass with
the binary and `podvm man` only (T-1980), whose P1 faults (T-1978, T-1979)
are fixed.

Each open fault that a user can meet:

- On a host with no `chroot(2)` and no user namespace, a program whose
  `PT_INTERP` lies past its first 64 KiB is refused with 125: the shells of
  `quay.io/fedora/fedora:42` (T-1981; COORD-193 asks podcrates).
- The P2 and P3 faults of the third outside pass (T-1980): on the default
  userland rung with alpine, `ls -l` shows a link that the payload makes in
  the tree as `-> /`; on the syscall tier a file as a `-v` source and a
  command that is not runnable exit 125 or 127 with a message that names
  the wrong cause, and a refused run leaves `tmpfs/PID/0` in the store;
  `--podvm-volume-base` takes a source with `..` that leaves the base;
  `-t` sets no `TERM`.
- The run report of an incus unit has no `oom_kills` (`inspect` gives
  `OomKills`, T-1969).
- The faults of `0.2.0-beta.33`'s known gaps stay open where this list does
  not name them as fixed.

### Fixed: the syscall tier for podrunner's jobs, 2026-11-18

Record: T-1974, T-1976, T-1977, T-1978, T-1979.
Deployed: version 0.2.0-beta.34.

- On the syscall tier, a child of `posix_spawn` runs a link in `/tmp` or
  in a volume: the handler keeps the descriptors of the volumes through
  the child's `close_range` (T-1978). `python3 -m venv` with pip works.
- On the default userland rung, a link that the payload makes in a volume
  keeps the guest's target, so it works in the next run and on the host
  (T-1979).
- The handler of the syscall tier runs each call on a stack of its own, not
  on the payload's alternate signal stack, which Rust's std sizes to 8 KiB
  on a CPU without AMX (T-1976).
- `exec` into a container whose foreground run had `--tmpfs` works again;
  since `0.2.0-beta.33` it refused the ended run's tmpfs directory with
  125 (T-1974).
- On the syscall tier, a relative path of a glibc program through an
  absolute link resolves in the root, not on the host: the handler takes
  `AT_FDCWD` as the kernel does, from the low 32 bits of the register
  (T-1977, PodUtils/podrunner#303).

### Added: the OOM kills of a machine guest, 2026-11-18

Record: T-1969.
Deployed: version 0.2.0-beta.34.

- The run report of a machine guest has `oom_kills`: the guest init gives
  the count `oom_kill` of its kernel with the exit marker (T-1969,
  PodUtils/podrunner#296 ask 6). Measured under TCG with `--podvm-mem
  256M`: a payload that fills the memory ends with 137 and `oom_kills` 1,
  for `run --rm` and for `run -d`; a payload that exits 3 has
  `oom_kills` 0.

## 0.2.0-beta.33 (2026-11-18)

### Known gaps: what the checks of 0.2.0-beta.33 prove, and what stays open, 2026-11-18

Record: T-1949, T-1974.
Deployed: version 0.2.0-beta.33.

What the checks of this beta prove: the gate, the integration tests as uid
1000, `workloads.yml`, `arm.yml`, the nightly trial, experiment 434 (the
incus tier, clauses 1 to 18) and an outside pass with the binary and
`podvm man` only (T-1974), whose P0 and P1 faults are fixed.

Each open fault that a user can meet:

- `--storage-opt size=` is measured only where the pool holds no size (a
  refusal); a pool with a quota (btrfs, zfs, lvm) is not measured.
- On the syscall tier: `getsockname` of a socket in the container names
  `/proc/self/fd/N/...` (OR-07); a file as a `-v` source, a directory as
  the command and an environment over the kernel's limit exit 127 with a
  message that names the wrong cause (OR-10 to OR-12).
- `--podvm-volume-base` accepts a `-v` source that is not under DIR with no
  word (OR-08); the loader family of the userland rung refuses the flag.
- The pty of `-t` is still no controlling terminal: Ctrl-C works, `^Z` and
  the job control of a shell do not (COORD-188).
- The faults of `0.2.0-beta.32`'s known gaps stay open where this list does
  not name them as fixed (OQ-04, OQ-06 to OQ-10).

### Fixed: the faults of the second outside pass, 2026-11-18

Record: T-1974, T-1975.
Deployed: version 0.2.0-beta.33.

- On the syscall tier, `exec` opens each volume under the base of the run:
  a link that replaced a source after the start is refused (exit 125), not
  followed out of `--podvm-volume-base`.
- Ctrl-C of `run -it` and of `run -t` reaches the payload, and the run
  exits with the code of the payload.
- `exec` with no `-w` starts in a `-w` of the unit that is in a volume.
- On the syscall tier, `-v HOST:/tmp` of a foreground run replaces the
  tier's own `/tmp`, and a link in a volume that names an absolute path or
  leaves the volume resolves as in the container: `python -m venv` works
  in a work directory of a volume.
- The ceiling of the size of the binary is 9 000 000 bytes (call C24).

### Fixed: a Unix socket of a volume on the syscall tier, 2026-11-18

Record: T-1973.
Deployed: version 0.2.0-beta.33.

- On `--podvm-tier=syscall`, `connect` and `bind` of a Unix socket take the
  path in the container, as each other path call does: a payload reaches
  a socket of a `-v` directory, and a socket that it binds stays in the
  container (PodUtils/podrunner#301).

### Added: the size of the root disk and the usage of an incus instance, 2026-11-18

Record: T-1964, T-1965.
Deployed: version 0.2.0-beta.33.

- `--storage-opt size=N` of the incus tier sizes the root disk of the
  instance; a storage pool that holds no size (the driver `dir`) refuses
  the run by name and leaves no instance (PodUtils/podrunner#296, ask 1).
- `inspect` of an instance of the incus tier gives `CpuSeconds`,
  `PeakMemoryBytes` and `OomKills` (asks 2 and 6).
- podcrates `v0.71.0`.

### Added: `--env-file0` for a value with a newline, 2026-11-18

Record: T-1971, T-1972.
Deployed: version 0.2.0-beta.33.

- `run`, `run -d`, `create` and `exec` take `--env-file0 PATH`: entries
  `KEY=VALUE` with a NUL after each, as `env -0` writes them, so that a
  multi-line secret stays out of the argv of podvm
  (PodUtils/podrunner#300).
- `--podvm-volume-base` holds on the syscall tier too
  (`--podvm-tier=syscall`, a host with no user namespace): each volume
  under the base opens with no symbolic link, and the handler resolves
  each path under it (PodUtils/podrunner#300, ask 3).

## 0.2.0-beta.32 (2026-11-18)

### Known gaps: what the checks of 0.2.0-beta.32 prove, and what stays open, 2026-11-18

Record: T-1949, T-1970.
Deployed: version 0.2.0-beta.32.

What the checks of this beta prove: the gate (fmt, clippy, the unit and
contract tests, the static build), the integration tests as uid 1000, a
run of `workloads.yml`, experiment 434 (the incus tier, with a container
engine in an instance), and an outside pass of about 30 minutes with the
binary and `podvm man` only (T-1970). Its three P1 faults are fixed.

Each open fault that a user can meet:

- Ctrl-C under `run -it` works through podvm, but the pty is still no
  controlling terminal: `^\`, `^Z` and the job control of a shell do not
  work (COORD-188 asks pod-enter for a session).
- `run -t` with no `-i` hangs where the payload reads stdin (OQ-04).
- `inspect` of a container has no `.Config.*` fields (OQ-06); some
  refusals exit 125 where the exit table says 1 or 2 (OQ-07); `pull -q`
  can hide a registry error (OQ-08).
- `run -d --rm` keeps the record and its tree until `rm` (OQ-09, #474).
- Each `rm` starts a copy of a spare tree, with no bound on the number at
  once (OQ-10); `PODVM_SPARE_TREE=0` turns the copies off.
- The incus tier has no size of the root disk and no CPU time or peak
  memory of an instance yet (PodUtils/podrunner#296, asks 1 and 2: they
  wait for `pod-incus`); no tier gives a sized writable disk (ask 5).
- `oom_kills` of the run report is measured by unit tests only; the run
  of a host with a delegated cgroup2 subtree (`kvm.yml`) comes next.
- A dynamic program that issues raw system calls on the userland rung
  needs `--podvm-tier=syscall` (T-1935, T-1740).
- The other reports of the external QA of `0.2.0-beta.31` that stay open
  are in the record (T-1937 to T-1954).

### Added: the asks of podrunner, 2026-11-18

Record: T-1934, T-1945, T-1966, T-1967, T-1969, T-1970.
Deployed: version 0.2.0-beta.32.

- Each unit has a terminal of its own: `run -d`, `create`, `start` and a
  foreground run on the namespace rung mount a new devpts instance and
  its `ptmx`, so `openpty`, Python's `pty.spawn` and `script` work
  (PodUtils/podrunner#216).
- `--podvm-volume-base DIR` of `run`, `run -d` and `create`: each `-v`
  source under DIR opens with no symbolic link, and the bind takes that
  descriptor. A link under DIR is a refusal with exit 125. The userland
  rung and each guest tier refuse the flag (PodUtils/podrunner#277).
- `--pids-limit N` on the machine tier: the guest holds `pids.max` for
  the command and each child (PodUtils/podrunner#296, ask 4).
- The run report has `oom_kills` where the run has a cgroup leaf: the
  count of OOM kills of `--memory` (PodUtils/podrunner#296, ask 6).
- A foreground `-v HOST:/tmp` (or a path under `/tmp`) is no longer
  hidden by the tmpfs on `/tmp`.
- Ctrl-C in `run -it` interrupts the payload (exit 130).
- A `run -d` refused for a name in use leaves no rootfs tree.
- `exec` starts in the working directory of the container where `-w`
  names none, as docker's does.
- `--podvm-engine` on the incus tier: the instance gets `security.nesting`
  and the intercepts of `mknod` and `setxattr`, so `dockerd` runs in it
  (PodUtils/podrunner#296, ask 3).
- podcrates `v0.70.0`.

### Fixed: the first reports of the external QA of `0.2.0-beta.31`, 2026-11-17

Record: T-1937, T-1938.
Deployed: version 0.2.0-beta.32.

- Two podvm commands at once no longer lose the table of containers: a
  fast `run -d` no longer ends `Dead` with no exit code, and `run` no
  longer fails with "containers.json: No such file or directory" (#472,
  #501). Parallel foreground runs no longer share one file of the exec
  probe.
- `--name` takes docker's rule of a name (#484). `logs` refuses a second
  container and `version` an operand (#486). `image bogus` is a usage
  error (#487). `exec --platform` of a container refuses another
  platform (#489). `doctor` names what the machine profile needs (#491).
- `exec --env-file PATH`, as in `run`: a secret stays out of the argv of
  podvm (PodUtils/podrunner#277).
- An argument that is not UTF-8 is refused by name with exit 125, and any
  other panic ends with one named line and exit 125, not SIGABRT (#505,
  #506). `--tls-verify=false` works (#492). `notebook` bounds its numbers
  (#493, #494, #506). `exec --platform`, `--name`, `logs`, `version`,
  `image`, `doctor` and `man` keep their help (#484 to #491, #497, #520).
- The syscall tier refuses `-u` (it fakes no identity, #508), and the
  `USER` warning names the real uid (#507). A refused run leaves no memo
  file (#475). `save -o` writes nothing on stdout (#522). `system abi`
  exits 125 for an unreadable file (#521).
- `podvm run IMAGE CMD | head -1` ends as a pipeline does: the payload
  gets the broken pipe, and the run keeps its record and its `--rm` (#534).
- `inspect A B` prints one JSON array, and a container comes before an
  image of the same name (#514, #523). A bare `profile`, `lab` and
  `windows` print their help and exit 0 (#532).
- `cp` into an image is refused: it changed the shared extraction that
  each later container copies (#498, #563). A copy out of an image stays.
- `--env-file` and `--report` keep the caller's own file permissions in
  podvm's user namespace (#552), and `--report` refuses a path that the
  writer cannot use before the payload runs (#513).
- `--` ends the options of each verb (#488). `import` names each
  compressed form and refuses a hard link that `extract` would refuse
  (#503, #515, #525). `rmi` of one of several names of an image is an
  untag, also while a container uses the image (#548). `man --roff` and
  `--llms` write each usage line once (#518).
- A SIGINT that a process sends to podvm reaches the payload (#535).
  `run -t` with no `-i` attaches no stdin (#545). A `cp` destination
  that ends with `/` names a directory (#555). `system df` and `image
  prune` count each extraction once and in full (#526).
- `pull` hashes a blob that the store holds and fetches a corrupt one
  again (#490). A record takes the platform that the registry served, so
  `linux/arm64` and `linux/arm64/v8` make one record (#528, #544).
  `image prune` removes the blobs of a pull that a signal stopped, and
  `system df` counts them (#527).
  `--add-host NAME:host-gateway` writes 127.0.0.1 (#480). `load` takes a
  layout that `tar` packed again (#477). The Windows rows of `images`
  take `--filter` (#476).

### Added: the syscall tier for a detached container, 2026-11-17

Record: T-1935.
Deployed: version 0.2.0-beta.32.

- `podvm run -d --podvm-tier=syscall` and `create` keep the tier. `exec`
  in such a container enters the tier, and `inspect` has the field
  `Tier`. On a host with the userland rung only (a GitHub-hosted runner),
  this tier gives a raw `rename`, `renameat2` or `link` of a dynamic
  program (`uv`, `ruff`) the paths of the image: on the loader family
  these reach the host and fail with EACCES (PodUtils/podrunner#189).
- The syscall tier takes a `:ro` volume: a write in it fails with EROFS.
- The tier no longer kills a child of `posix_spawn` (each process spawn of
  a Rust program, such as `uv venv`): the handler read past the end of
  the stack of the child. Python's `subprocess` no longer loses the count
  of `/tmp` (its `close_range` closed it), so its child no longer exits
  127.

### Fixed: the first run of a fresh Windows golden image, 2026-11-17

Record: T-1933.
Deployed: version 0.2.0-beta.32.

- `podvm pull windows:NAME` boots the installed disk once more before it
  becomes the golden image, and so proves that the installed disk boots.
- A foreground `podvm run windows:NAME CMD` runs the command through the
  QEMU Guest Agent of a short-lived machine, as `podvm exec` does. On a
  host with 2 CPUs the run through the mailbox agent timed out at 600 s
  in 3 of 4 runs. The mailbox stays the fallback for an image with no
  agent; `PODVM_WINDOWS_MAILBOX=1` selects it. Measured on `windows:2025` and
  `windows:11`, with 2 and 4 CPUs (kvm runs 44104748355, 44126060517).

### Fixed: a Windows run keeps its overlay in the store, 2026-11-17

Record: T-1932.
Deployed: version 0.2.0-beta.32.

- A foreground `podvm run windows:NAME CMD` writes the overlay of the guest
  beside the catalog, not into the runtime directory, which is a small
  tmpfs on many hosts.

## 0.2.0-beta.31 (2026-11-17)

### Added: `exec`, `cp` and a graceful `stop` of a Windows machine, 2026-11-17

Record: T-1868.
Deployed: version 0.2.0-beta.31.

- The install of a full Windows guest adds the virtio-win drivers and the
  QEMU Guest Agent. A detached machine (`podvm run -d windows:NAME`) then
  takes `podvm exec NAME CMD` (with the exit code of CMD), `podvm cp` in
  both directions, and `podvm stop`, which shuts the guest down through
  its agent.

### Fixed: the source fixup reads no comment, 2026-11-17

Record: T-1931 (podcrates `v0.68.0`).
Deployed: version 0.2.0-beta.31.

- The `HEAD` of the source fixup goes only to the hosts of the lines that
  the package manager reads; a URL in a comment of a sources file (the
  `help.ubuntu.com` of Ubuntu 24.04) gets no request.

### Fixed: `podvm pull windows:2025` and `windows:11` install a full Windows guest, 2026-11-17

Record: T-1867.
Deployed: version 0.2.0-beta.31.

- The root disk of an install is in the store, not in the runtime
  directory, which is a small tmpfs on many hosts; Setup failed when it
  filled.
- The install runs through the reboots of Setup, and the answer file
  sets the password of the built-in Administrator that Windows Server
  asks for.
- Under KVM, `windows:2025` installs in about 9 min and `windows:11` in 11
  to 13 min, and `podvm run windows:NAME ver` answers.

### Changed: the documents name the request of the source fixup, 2026-11-17

Record: T-1931 (PodUtils/podrunner#202).
Deployed: version 0.2.0-beta.31.

- The notes of `--no-source-fixup` say that, without the flag, podvm sends
  one `HEAD https://HOST/` to each host of an `http://` package source of
  the image before the payload starts, and that the flag stops it.

## 0.2.0-beta.30 (2026-11-17)

### Fixed: a detached run sees and signals only its own processes, 2026-11-17

Record: T-1930 (PodUtils/podrunner#200).
Deployed: version 0.2.0-beta.30.

- `podvm run -d`, `create` and `start` on the namespace rung run the
  payload in a PID namespace of its own, with its own `/proc`. A job no
  longer sees or signals the processes of the host, and `podvm exec`
  enters the same PID namespace.

### Changed: podcrates `v0.64.0` and `v0.65.0`, 2026-11-17

Record: T-1924, T-1868 (COORD-166).
Deployed: version 0.2.0-beta.30.

- In `podvm lab run --way-out`, a program in the lab checks a real server
  with the roots of the host; it needs no `SSL_CERT_FILE` of its own.
  An `SSL_CERT_FILE` of the host that does not read refuses the way out
  with a line that names the file.

## 0.2.0-beta.29 (2026-11-16)

### Fixed: a run that `PODVM_MODE` forces is `Up` while it runs, 2026-11-16

Record: T-1786 (issue 330).
Deployed: version 0.2.0-beta.29.

- A foreground run on a rung that `PODVM_MODE` forces is recorded
  running while its payload runs: `ps`, `system df`, `kill`, `wait` and
  `stop` see it, as on each other rung.

### Fixed: `-u` with the subordinate range, and `cp` and `exec` of its trees, 2026-11-16

Record: T-1911.
Deployed: version 0.2.0-beta.29.

- With `PODVM_USERNS_RANGE=1`, a foreground `run -u UID` drops to UID
  for real again; since beta.28 it ran as the owner of the files of root.
- `cp` and `exec` of a container that is not running read and write the
  files that a payload of another id left in its tree.

### Added: `exec`, `cp` and a graceful `stop` of a Windows machine, 2026-11-16

Record: T-1868 (podcrates `v0.63.0`, `pod-qga`).
Deployed: version 0.2.0-beta.29.

- `podvm pull windows:2025` (and `11`, `10`) attaches the virtio-win CD
  0.1.302 (pinned sha256) and installs the virtio-win drivers and the
  QEMU Guest Agent at the first logon.
- A machine of `run -d windows:2025` has the channel of the agent.
  `podvm exec NAME CMD` runs `cmd.exe /d /c CMD` in it, and the exit code
  passes through in 0 to 124. `podvm cp` copies one file either way.
  `podvm stop` sends `guest-shutdown` first. No guest ran this change.

### Added: `podvm lab run --way-out`, 2026-11-16

Record: T-1924 (podcrates `v0.61.0`).
Deployed: version 0.2.0-beta.29.

- `podvm lab run --way-out PROFILE -- CMD` lets the stand-in proxy of a
  network lab send a `CONNECT` that its class passes to the real network,
  directly or through the proxy of the host. CMD never gets the proxy of
  the host or its credential.

### Fixed: `podvm lab run proxy-only` did not hold, 2026-11-16

Record: T-1924.
Deployed: version 0.2.0-beta.29.

- The stand-in proxy of a network lab died when a client closed its
  socket before the answer, and each later `CONNECT` got "Connection
  refused". The lab process now keeps the stand-in alive, and CMD still
  gets the default SIGPIPE.

### Changed: the installer checks the signature of `SHA256SUMS`, 2026-11-16

Record: T-1925 (podcrates `v0.63.0`).
Deployed: version 0.2.0-beta.29.

- `podvm-install.sh` is on the installer core of podcrates `v0.63.0`.
  Where `cosign` is on `PATH` and the run is not offline, it checks the
  keyless signature of `SHA256SUMS` for `nightly.yml` of PodUtils/podvm
  at the tag, and compares each pin and the installer with
  `SHA256SUMS` before a download. A failed check is a refusal, and no
  file changes. `PODVM_COSIGN=none` turns the check off.
- Each release carries `SHA256SUMS` (the binaries, the installer and the
  bundle) and its signature bundle `SHA256SUMS.sigstore.json`, in place
  of `podvm-SHA256SUMS`.

### Added: the manual page and `llms.txt` with each release, 2026-11-16

Record: T-1917.
Deployed: version 0.2.0-beta.29.

- Each release carries `podvm.1` (`podvm man --roff`) and
  `podvm-llms.txt` (`podvm man --llms`), each with its SHA-256 and its
  signature bundle. The release binary writes them, with the date of the
  release commit. `mandoc -Tlint` checks the page in the build.

## 0.2.0-beta.28 (2026-11-16)

### Changed: the machine tier sizes the memory for a large image, 2026-11-16

Record: T-1922 (PodUtils/podrunner#199).
Deployed: version 0.2.0-beta.28.

- With no `--podvm-mem`, a guest whose initrd is over 100 MiB gets about
  three times the initrd and 512 MiB of memory, in place of 512 MiB. A
  smaller `--podvm-mem` gives a warning before the boot, and a guest that
  resets with no output names the memory to give.
- Where the console of the emulator closes, the message gives the exit
  code or the signal of the emulator.

### Changed: a foreground `run` runs as the user of the image with no root, 2026-11-16

Record: T-1927 (podcrates `v0.63.0`).
Deployed: version 0.2.0-beta.28.

- On the namespace rung of a user with no root, a foreground `podvm run`
  runs the payload as the image's `USER`, or as `--user`, as `run -d`
  does since `0.2.0-beta.27`. `--user 0` keeps uid 0.

### Added: `podvm man --llms`, 2026-11-16

Record: T-1917.
Deployed: version 0.2.0-beta.28.

- `podvm man --llms` prints the whole manual as one `llms.txt`. The date
  of `podvm man --roff` comes from `SOURCE_DATE_EPOCH` where it is set.

## 0.2.0-beta.27 (2026-11-16)

### Changed: `run -d` runs as the user of the image with no root, 2026-11-16

Record: T-1927 (PodUtils/podrunner#196, podcrates `v0.62.0`).
Deployed: version 0.2.0-beta.27.

- On the namespace rung of a user with no root, `podvm run -d` and
  `create` run the payload as the image's `USER`, or as `--user`, inside
  the user namespace, as docker does. `--user 0` keeps uid 0.

## 0.2.0-beta.26 (2026-11-16)

### Changed: the machine tier says why it stopped waiting, 2026-11-16

Record: T-1922 (PodUtils/podrunner#198).
Deployed: version 0.2.0-beta.26.

- Where the guest ends before the exit code of the payload, the message
  also says whether the console of the emulator closed or the bound of the
  run came, the seconds since the start, and the size of the initrd.

### Fixed: a file on descriptor 17 reads empty, 2026-11-16

Record: T-1928 (PodUtils/podrunner#197).
Deployed: version 0.2.0-beta.26.

- A program that closes descriptor 17 and opens a file there (Node under
  npm 6) no longer reads that file as empty, and podvm no longer writes
  into it: the interposer uses 17 for its ownership memo only while 17 is
  still the memo.

### Fixed: `run -d --hostname` with no root, 2026-11-16

Record: T-1926 (PodUtils/podrunner#196).
Deployed: version 0.2.0-beta.26.

- `podvm run -d --hostname NAME` and `create --hostname NAME` of a user
  with no root on the namespace rung no longer exit 125: the run gets a
  UTS namespace with NAME.

## 0.2.0-beta.25 (2026-11-16)

### Fixed: a run where a filter refuses `chroot` in each namespace, 2026-11-16

Record: T-1923.
Deployed: version 0.2.0-beta.25.

- Where `chroot` is refused also inside a user namespace (a seccomp
  filter of the caller), `podvm run` takes a no-chroot family in place of
  the namespace rung, which ended with "chroot into the rootfs failed with
  EPERM".

### Fixed: `exec` of a stopped run with no root, 2026-11-16

Record: T-1921 (podcrates `v0.59.0`, COORD-150).
Deployed: version 0.2.0-beta.25.

- On a host that refuses `chroot` to a user with no root but opens a user
  namespace, `podvm exec` of a run that ended no longer exits 125 with
  "chroot into the rootfs failed with EPERM": it takes the loader family.
  The ladder, `doctor`, `system df` and `system info` read the same row.

### Changed: the machine tier says how the emulator ended, 2026-11-16

Record: T-1922 (PodUtils/podrunner#193, #195).
Deployed: version 0.2.0-beta.25.

- Where the guest ends before the exit code of the payload, the message
  names the exit code or the signal of the emulator, whether the guest
  reached the command, its last console lines, and the last line of QEMU,
  also with no `--verbose`.

### Added: `podvm-install.sh`, 2026-11-16

Record: T-1918 (the owner's ruling "Installer", podcrates `v0.58.0`).
Deployed: version 0.2.0-beta.25.

- Each release carries `podvm-install.sh` and the offline bundle
  `podvm-VERSION.tar.gz`. The installer needs no root, checks each file
  against its pin, and a second run changes nothing.

### Fixed: a volume keeps the namespace rung, 2026-11-16

Record: T-1920 (PodUtils/podrunner#189).
Deployed: version 0.2.0-beta.25.

- A foreground `run -v` of a user with no root binds the volume and stays
  on the namespace rung, so a raw `rename` of `uv` or `ruff` works.
- `podvm lab run --allow-connect PORT` lets `bind-denied` reach a proxy.

### Changed: `podvm lab` runs the profiles of `pod-lab`, 2026-11-15

Record: T-1916 (PodUtils/podcrates#188).
Deployed: version 0.2.0-beta.25.

- `podvm lab profiles`, `run` and `check` take the 16 profiles of podcrates
  `pod-lab`. The lab checks the profile from inside before CMD runs.
- `podvm lab check` measures this host: exit 0 holds, 1 fails, 3 unseen.
- `inherited-fds` holds descriptors 3 to 9.

### Changed: `run -d` and `create` enter the namespace rung with no root, 2026-11-15

Record: T-1912.
Deployed: version 0.2.0-beta.25.

- Where an unprivileged user can open a user namespace, `run -d` and
  `create` enter the namespace rung through pod-enter (podcrates
  `v0.53.0`), with the host's device nodes.

## 0.2.0-beta.24 (2026-11-15)

### Added: `podvm lab run` and `podvm lab check` of hostile host classes, 2026-11-15

Record: T-1916 (PodUtils/podrunner#180, #182).
Deployed: version 0.2.0-beta.24.

- `podvm lab run PROFILE[,PROFILE...] -- CMD` runs CMD in a hostile host
  class with no root: `bind-denied`, `getsockname-denied`, `userns-off`,
  `small-tmp`, `noexec-tmp` and `inherited-fds`. `podvm lab check` proves
  each part on this host; `podvm lab profiles` lists them. A part that the
  host cannot give refuses with exit 125.

### Fixed: `kvm` under QEMU user mode, 2026-11-15

Record: T-1907 (PodUtils/podvm#401).
Deployed: version 0.2.0-beta.24.

- The machine tier and the Windows guest use KVM only where `/dev/kvm`
  runs a guest of podvm's architecture. An aarch64 podvm in an emulated
  container on an x86_64 host now runs TCG, or refuses
  `PODVM_MACHINE_ACCEL=kvm` by name.

### Fixed: `tar -x` of a link before its target on the userland rung, 2026-11-15

Record: T-1915 (PodUtils/podrunner#178).
Deployed: version 0.2.0-beta.24.

- `utimensat` and `fchmodat` with `AT_SYMLINK_NOFOLLOW` act on the link,
  so GNU tar extracts a link that comes before its target, or a dangling
  one (`actions/setup-python` with Python 3.13).

### Fixed: "a passed descriptor goes onto 17", 2026-11-15

Record: T-1914 (PodUtils/podrunner#177).
Deployed: version 0.2.0-beta.24.

- `podvm run` and `run -d` from a parent that holds descriptors 3 to 15
  open (podrunner under `podnet pod`) no longer exit 125: podvm holds the
  number of the memo (17) from its start.

## 0.2.0-beta.23 (2026-11-15)

### Added: `--store DIR`, 2026-11-15

Record: T-1913 (PodUtils/podrunner#168).
Deployed: version 0.2.0-beta.23.

- `podvm --store DIR VERB` (or `podvm VERB --store DIR`) uses DIR as the
  store, with the meaning of `PODVM_STORE`.

### Changed: podvm takes podcrates `v0.52.0`, 2026-11-15

Record: T-1908, T-1912.
Deployed: version 0.2.0-beta.23.

- `run -d --hostname` names the UTS namespace of the namespace rung
  through `Plan::hostname` of pod-enter; the probe of KVM checks the
  architecture of the guest that the device runs (PodUtils/podvm#407).
- Each blob of a pull uses the registry token of the manifest, with no
  token request of its own (T-1850).

### Added: `--memory`, `--cpus` and `--pids-limit` in a delegated cgroup2 subtree, 2026-11-15

Record: T-1747 (PodUtils/podrunner#110).
Deployed: version 0.2.0-beta.23.

- Where the host delegates a cgroup v2 subtree to the user (a systemd user
  session, for example), `podvm run` and `podvm run -d` hold `--memory`,
  `--cpus` and `--pids-limit` in a leaf of the run, and name an OOM kill.
  Elsewhere each flag still refuses with exit 125 and the reason.

### Changed: podvm takes podcrates `v0.51.0`, 2026-11-15

Record: T-1890, T-1875, T-1909.
Deployed: version 0.2.0-beta.23.

- An arm64 guest of the machine tier fetches its pinned kernel itself
  (`alpine-netboot-aarch64` of `pod-services`); `PODVM_MACHINE_KERNEL_ARM64`
  is an override now, not a need.
- The Windows agent of `pod-guest` tries its shutdown again when `ping`
  fails (PodUtils/podvm#380).
- A run in the user namespace of T-1909 gets `/dev/null`, `/dev/zero`,
  `/dev/random` and `/dev/urandom` of the host, leaves no tree behind, says
  nothing of the ids of its first extraction, and `probe` and `system
  info` report the rung that `run` enters. A process that the payload left
  ends with the run.
- Two runs at the same time, one in a user namespace and one outside it,
  no longer remove each other's tree while it is filled.
- `exec` into a container of `--hostname` gives the name.

### Added: `exec` and `cp` join a run in a user namespace, 2026-11-15

Record: T-1910.
Deployed: version 0.2.0-beta.23.

- `podvm exec` and `podvm cp` into a foreground run that podvm put in a
  user namespace (T-1909) join its user, UTS, PID and mount namespace:
  the command sees the `/tmp` and the `/proc` of the run, as uid 0 of it.

## 0.2.0-beta.22 (2026-11-15)

### Changed: an unprivileged foreground `run` enters the namespace rung, 2026-11-15

Record: T-1909 (PodUtils/podvm#397), call C21.
Deployed: version 0.2.0-beta.22.

- Where the host opens user namespaces, `podvm run` of a user with no
  capability opens a user, a PID and a UTS namespace and enters the
  namespace rung: the payload runs as uid 0 of that namespace, with its
  own `/proc`, and `--hostname` holds (as it now does for uid 0 on the
  namespace rung). Before, such a user got the
  userland rung. `run -d`, `create` and `exec` are unchanged.
  `PODVM_USERNS=0` keeps the old entry.

### Changed: `podvm probe` names why a stronger rung is not taken, 2026-11-15

Record: T-1903 (PodUtils/podvm#397).
Deployed: version 0.2.0-beta.22.

- `podvm probe` names on stderr the call that keeps the namespace rung and
  the chroot rung from the run, and says when the host opens a user
  namespace that the namespace rung does not use yet (T-1909).

### Fixed: name resolution under `--podvm-confine`, 2026-11-15

Record: T-1902 (PodUtils/podvm#396).
Deployed: version 0.2.0-beta.22.

- Under `--podvm-confine`, a musl payload on the userland rung resolves
  names again. musl reads `/etc/resolv.conf` with a raw system call, and
  the confine layer refused that file, so each name failed with
  `EAI_AGAIN`.

### Added: `--hostname` for `run` and `create`, 2026-11-15

Record: T-1908 (PodUtils/podvm#402, PodUtils/podrunner#115).
Deployed: version 0.2.0-beta.22.

- `podvm run --hostname NAME` gives the payload NAME for `uname -n`,
  `hostname`, `/etc/hostname`, `$HOSTNAME` and `/etc/hosts`, on the
  userland rung (native-static and loader family). The namespace rung, the
  machine tier and the incus tier refuse it with exit 125 by name.

### Added: `podvm exec` into a running guest of the machine tier, 2026-11-15

Record: T-1750 (PodUtils/podcrates#124, ask of podswarm).
Deployed: version 0.2.0-beta.22.

- `podvm run -d --podvm-tier=machine IMAGE CMD` keeps the guest running,
  and `podvm exec NAME CMD` runs a command in it in about 0.2 s, with its
  stdout, stderr and exit code. `ps`, `stop`, `rm` and `logs` serve the
  guest. Before this, `-d` with the machine tier ran in the foreground and
  was ignored.

### Added: a linux/arm64 guest of the machine tier on an x86_64 host, 2026-11-15

Record: T-1890 (PodUtils/podrunner#102).
Deployed: version 0.2.0-beta.22.

- `podvm run --podvm-tier=machine --platform linux/arm64 IMAGE` boots an
  aarch64 guest under TCG with `qemu-system-aarch64` of the host, and
  gives stdout, stderr and the exit code as for x86_64. Until podcrates
  pins the kernel, `PODVM_MACHINE_KERNEL_ARM64` names it.

### Added: `podvm windows view` shows the screen of a guest in a browser, 2026-11-15

Record: T-1899.
Deployed: version 0.2.0-beta.22.

- `podvm windows view NAME` prints a URL on 127.0.0.1; the page is the
  noVNC client (fetched once into the store, its digest pinned), connected
  to the screen of the running guest.

## 0.2.0-beta.21 (2026-11-15)

### Fixed: a `:ro` volume holds for a static image where chroot is denied, 2026-11-15

Record: T-1901 (PodUtils/podrunner#111).
Deployed: version 0.2.0-beta.21.

- `podvm run -d -v SRC:DST:ro` of a static image (the native-static rung)
  was refused with exit 125. The handler of the rung now answers EROFS for
  each write into the volume, also from a static program; a read works.
  `--read-only` of the rootfs stays on the loader family only.

### Added: the guests of the machine tier need no QEMU on the host, 2026-11-15

Record: T-1875.
Deployed: version 0.2.0-beta.21.

- A host with no QEMU needs no install for Windows and DOS guests and for
  the Linux guest of `--podvm-tier=machine`: podvm
  fetches a pinned static QEMU 9.2.0 and its firmware (54 MiB) into its
  store on the first run (T-1875). A QEMU of the host still wins.

### Added: `podvm run -d windows:dos`, with `ps`, `stop`, `start`, `rm` and `logs -f`, 2026-11-15

Record: T-1870 (partial), T-1869 (partial), T-1899 (partial).
Deployed: version 0.2.0-beta.21.

- `podvm run -d --name NAME windows:dos` (also `windows:validationos`
  and an installed setup ISO) starts a detached guest; `podvm ps` lists it
  beside the containers, `podvm stop` powers it off, `podvm start` boots
  it again from its own disk, `podvm rm` removes it, and `podvm logs -f`
  follows its serial line. A guest ends at the end of
  its lifetime (`--timeout`, else 24 h), and no emulator stays after
  `stop`, `rm -f` or the lifetime. Before this, `run -d windows:dos` asked
  Docker Hub for an image and failed.
- `podvm run -d -p 33890:3389 windows:2025` publishes a port of the guest
  on 127.0.0.1 (an address in the value changes it), `podvm port NAME`
  prints the forwards, and `podvm windows
  screenshot NAME FILE` writes a screen dump of a running guest.
- `podvm windows view NAME` serves the VNC stream of a running guest as a
  WebSocket on 127.0.0.1 for a noVNC client (T-1899).

## 0.2.0-beta.20 (2026-11-15)

### Added: `podvm pull windows:2025` installs Windows from its setup ISO, 2026-11-15

Record: T-1867 (partial), T-1861 (partial), T-1871 (partial).
Deployed: version 0.2.0-beta.20.

- `podvm pull windows:2025` (also `11` and `10`) fetches the setup ISO,
  installs it unattended under KVM, prints each phase with the elapsed
  time, and writes a golden image with no backing file. The password of
  the user `podvm` goes into a file of mode 0600 beside it. Under TCG the
  install is refused unless `PODVM_MACHINE_ACCEL=tcg` asks for it. The
  first install under KVM is not measured yet.
- `podvm run windows:2025 CMD` and `podvm run windows:validationos CMD`
  run the command in the guest and give its exit code. The first run of
  Validation OS provisions it one time.
- `podvm windows` takes `PODVM_MACHINE_ACCEL` and names the group `kvm`
  when `/dev/kvm` does not open.

### Fixed: `exec` takes the cached probe, and one exec table reaches a confined payload, 2026-11-15

Record: T-1850 (partial), T-1892, T-1823, T-1849, T-1845, T-1858, T-1837, T-1846 (partial).
Deployed: version 0.2.0-beta.20.

- `exec IMAGE true` read the full probe on each call: 77 ms here, now 14
  ms, as `run` reads the cache.
- The interposer checks a confined exec against the one table of host
  paths that `pod-enter` writes (`PODVM_EXEC_HOST`).
- The incus tier deletes the instance of a failed start, `rm` names the
  state of an instance, and `system info` refuses a `tier` value that
  `run` refuses.

### Changed: podcrates `v0.50.0`; the Windows guest under KVM, 2026-11-15

Record: T-1864, T-1894, T-1873 (partial), T-1861 (partial), T-1876.
Deployed: version 0.2.0-beta.20.

- `kvm.yml` run 43672193303: `windows setup` of Validation OS and the
  exit codes of `windows run` pass under KVM.
- `podvm run windows:dos ver` works end to end under TCG (experiment
  451), and a weekly job checks the pins of the Windows catalog.

### Fixed: a quiet run prints no line of the extraction, 2026-11-15

Record: T-1852 (partial).
Deployed: version 0.2.0-beta.20.

- The first run of an image printed `<layer>: Extracted (N entries, M
  whiteouts)` on stderr also with no `--verbose`. It now prints the line
  only with `--verbose`, or with a failed extraction.

### Added: limits of memory, CPU and processes on the incus tier, 2026-11-15

Record: T-1897 (PodUtils/podrunner#110).
Deployed: version 0.2.0-beta.20.

- `--memory`, `--cpus` and `--pids-limit` become the limits of the incus
  instance (`limits.memory`, `limits.cpu`, `limits.processes`), and the
  report names them. Each other tier refuses them by name; `--ulimit
  nproc=` and `--ulimit as=` hold on each tier.

### Fixed: refusals and help texts name the cause and what the binary does, 2026-11-15

Record: T-1829, T-1832, T-1836, T-1838, T-1851, T-1857.
Deployed: version 0.2.0-beta.20.

- `run` with no image prints the usage line and the cause (exit 1); a
  flag with no value names the spelling of the caller; `logs --tail -1`
  says that the count is zero or more.
- A pull of a repository that does not exist says that it does not exist
  or is private; the platforms of an index show each one once, with no
  attestation; a changed blob of `load` names its entry.
- A run on a store of another user names the owner and `$PODVM_STORE`.
- The help of `exec`, `extract` and `pull`, the manual heads and the help
  columns of `windows` and `machine ssh` say what the binary does; the
  gate fails a text that waits for a done entry.

## 0.2.0-beta.19 (2026-11-15)

### Added: `container prune` and the `container` group, 2026-11-15

Record: T-1848.
Deployed: version 0.2.0-beta.19.

- `podvm container prune` removes each exited or dead container and its
  rootfs tree, and prints the bytes that the trees held. A created or
  running container stays.
- `podvm container ls|rm|start|stop|...` runs the top-level verb of the
  same name.

### Fixed: a detached run of a static image where chroot is denied, 2026-11-15

Record: T-1896 (PodUtils/podrunner#100, case 2).
Deployed: version 0.2.0-beta.19.

- `run -d` and `create` of a static image (`busybox:1.37.0-musl`) on a
  host that denies chroot refused with "the memfd family has no detached
  driver". The native-static rung now has a detached driver; `wait`,
  `logs` and `stop` work.

### Fixed: `save` by an index digest after a pull by tag, 2026-11-15

Record: T-1895 (PodUtils/podrunner#104), T-1841.
Deployed: version 0.2.0-beta.19.

- A pull by tag and a pull by the index digest gave two records of one
  manifest, and `save` of the digest refused them as two images. They are
  now one image.
- `save` takes `--platform`, which its refusal named.

### Fixed: each long flag takes `--flag=value`, 2026-11-15

Record: T-1853, T-1833.
Deployed: version 0.2.0-beta.19.

- 50 long flags refused `--flag=value` as a bug in podvm, for example
  `stop --time=1`, `kill --signal=KILL` and `ps --all=true`. A valued
  flag takes `=VALUE`, and a boolean takes docker's `=true` and `=false`.
- `run --attach` and `--expose` take their value, as docker's do; `run
  --expose 80 alpine` read `80` as the image.
- Each command answers `-h` and `--help` with exit 0.

### Fixed: `cp` onto a directory, and the modes of `cp -r`, 2026-11-15

Record: T-1822.
Deployed: version 0.2.0-beta.19.

- `cp FILE NAME:/etc` put nothing and blamed the source. A file onto a
  directory now goes into it under its own name, in both directions.
- `cp -r` keeps the modes 700 and 711 of the directories.

### Changed: the banner and the report, 2026-11-15

Record: T-1824, T-1826, T-1825, T-1834, T-1835.
Deployed: version 0.2.0-beta.19.

- The notice of the machine tier follows `-q` and `--verbose`, as the
  banner does; a default run prints nothing.
- `--no-steps` and an image with no `pacman-key` leave a row in the
  `degradations` of the report.
- A `--report` that cannot be written exits 125, and the report of a
  detached run gets the end of its payload: `exit_code`, `signal` and
  `timed_out`.
- `system info --format '{{json .X}}'` prints counts and `StrictOk` as
  numbers and booleans, and `compare --strict` refuses an empty or
  partial report.

### Fixed: the parse of `run` and the help of its options, 2026-11-15

Record: T-1828, T-1831, T-1839, T-1840.
Deployed: version 0.2.0-beta.19.

- `--add-host` refuses a value that is not an address, `-e =V` is
  refused, `--env-file` keeps the quotes of a value, and the help of
  `run` and `create` says only what the verbs do.
- `load` refuses an unknown `schemaVersion`, and the `images` table
  shows the platform where two records share a tag.

### Fixed: the answers of the machine tier, `profile show` and the `ps` table, 2026-11-15

Record: T-1823, T-1849, T-1845, T-1858.
Deployed: version 0.2.0-beta.19.

- The machine tier makes a missing `-w` directory, refuses a file there
  by name, and refuses a missing entrypoint before the boot with 127; no
  answer reads "no exit marker" any more. Its help names
  `qemu-system-x86_64` and says what `--podvm-mem` sizes.
- `profile show` writes nothing on stdout when it refuses.
- `ps` and the other tables pad by the columns of the terminal, so a
  CJK command keeps its row in line.

### Fixed: piped input to a machine run with `-t`, 2026-11-15

Record: T-1885.
Deployed: version 0.2.0-beta.19.

- `printf 'x\n' | podvm run --podvm-tier=machine -t IMAGE cat` printed
  nothing: podvm read the input for the run archive, and the terminal of
  the guest got only the end of input. It now gets each byte.

### Changed: podcrates `v0.49.0`, 2026-11-15

Record: T-1893, T-1894 (partial).
Deployed: version 0.2.0-beta.19.

- A provision of a Windows guest stops its emulator on each error.
- A provision types after a fixed wait (TCG), or when the screen settles
  (KVM). Under KVM, `windows setup` of Validation OS passed in 37 s.

### Added: `images` and `rmi` know the Windows names, 2026-11-14

Record: T-1861 (partial), T-1864 (partial).
Deployed: version 0.2.0-beta.19.

- `podvm images` lists each Windows name that the store holds, and
  `podvm rmi windows:NAME` removes its files.
- A provision of a Windows guest waits longer for its console under TCG
  (60 s of a still screen, an estimate) before it types.

### Added: `windows:validationos` fetches only its disk, by a UDF reader, 2026-11-14

Record: T-1863.
Deployed: version 0.2.0-beta.19.

- `podvm pull windows:validationos` reads the UDF directory of the ISO by
  range requests and fetches only the VHDX (900 MiB of 2.4 GiB), checked
  against a pinned sha256.
- `podvm windows fetch --url URL --extent FILE` prints the byte range of
  FILE in a UDF image and downloads nothing.

### Added: `podvm pull windows:NAME`, a resumable download with progress, and a test of the broker, 2026-11-14

Record: T-1820, T-1861 (partial), T-1862.
Deployed: version 0.2.0-beta.19.

- `podvm pull windows:NAME` fetches and checks the medium of the catalog
  (and makes the DOS base) instead of asking a registry; an unknown name
  exits 1 with the list of names.
- A Windows download prints one progress line for each twentieth, and
  resumes with a range request after a break.
- A fixture upstream proxy tests the broker: an allowed host gets 200,
  and an upstream refusal gives a 502 that names its status.

### Added: the answer-file stick and the boot options of a Windows guest, 2026-11-14

Record: T-1865, T-1866, T-1883.
Deployed: version 0.2.0-beta.19.

- `podvm-windows` writes `autounattend.xml` into a FAT stick (mode 0600)
  and gives its read-only USB medium; golden files pin the answer file
  of 11, Server 2025 and 10.
- The boot options of a Windows guest: the mailbox run of ValidationOS
  keeps `-nic none`; a desktop gets a user-mode NIC, a USB tablet and
  the Hyper-V flags under KVM, for the install of T-1867.
- `create` and `run` say which entries the extraction could not make (a
  fifo, a device node), as `extract` does.

### Changed: podcrates `v0.45.0`, 2026-11-14

Record: T-1827, T-1842, T-1881, T-1882, T-1891, T-1862 (partial).
Deployed: version 0.2.0-beta.19.

- Extraction keeps the exact mode of each directory of the image, also
  0555, and makes a directory where an earlier layer had a link.
  A user who is not root can remove such a tree.
- `--ulimit` keeps the last value of each resource; `-1` takes the hard
  limit podvm has; a hard value over it is refused with both numbers.
- `--insecure-registry` with a bad host exits 1 and names the value.
- A Windows download resumes with a range request where it broke.

## 0.2.0-beta.18 (2026-11-14)

### Fixed: the machine tier refuses an image of another architecture by name, 2026-11-14

Record: T-1889 (PodUtils/podrunner#102).
Deployed: version 0.2.0-beta.18.

- `run --podvm-tier=machine --platform linux/arm64` booted the x86_64
  guest and ended with "the guest gave no exit marker". It now refuses
  before the boot, exit 125, and names the entry that plans a guest of
  another architecture (T-1890).

### Fixed: `--podvm-confine` on the userland rung resolves the links of the image inside it, 2026-11-14

Record: T-1887, T-1888 (PodUtils/podvm#364, a patch of Nemo-010).
Deployed: version 0.2.0-beta.18.

- A path of the spec that is an absolute link of the image (`/bin/sh`
  of alpine links to `/bin/busybox`) was opened from the host root and
  refused at 125 where the host lacked the target. Each path now
  resolves inside the tree that serves it.
- A required path that the image lacks, or whose link leaves the tree,
  refuses with the guest path the caller wrote; it took a rule on a
  host file before.

### Fixed: `exec` after a `--tmpfs` run, the object of the interposer, and a fetch that cannot fit, 2026-11-14

Record: T-1830, T-1843, T-1844, T-1862 (partial), T-1786 (partial).
Deployed: version 0.2.0-beta.18.

- `exec` into a container whose run had `--tmpfs` refused at 125 or saw
  a removed directory; it now sees the tree of the image at that path.
- The object of the interposer is written through a file name of its own
  with `O_EXCL` and `O_NOFOLLOW`, so a symlink in the root of the image
  cannot redirect the write, and two entries into one root cannot mix
  two objects.
- A fetch of a Windows catalog name refuses before the first byte when
  the disk or `RLIMIT_FSIZE` cannot hold the file.
- The native-static rung also shows a foreground run as `Up`.

### Changed: podcrates `v0.44.0`; a foreground run is live in its record, 2026-11-14

Record: T-1786 (partial), T-1807, T-1813, T-1881 (partial), T-1879.
Deployed: version 0.2.0-beta.18.

- A foreground `run` reads `Up` in `ps` while its payload runs, on the
  chroot, namespace and userland rungs: `kill`, `stop`, `wait` and `exec`
  reach it, and `system df` counts it running.
- Extraction keeps the setuid and setgid bits of a file of mode 4755,
  2755 or 6755.
- Security: extraction changes the mode of a directory through a
  descriptor that does not follow a symlink, so a layer cannot change
  the mode of a directory outside the tree. `tmp` keeps 1777, and a
  directory that a later layer declares again takes its new mode.
- A restarted container shows no exit code of the run before, and a
  `Dead` record names no pid.

### Fixed: `system df` prints the IMAGE ID of `images`, 2026-11-14

Record: T-1814, T-1852 (partial).
Deployed: version 0.2.0-beta.18.

- `system df` printed the manifest digest in its IMAGE ID column; it now
  prints the config digest, as `images` and `inspect` do. Two platform
  records of one reference show the platform after the tag.
- Experiment 443 has rows for the checks of the external QA report.

### Fixed: `podvm windows doctor` checks more, and the UEFI firmware of Debian and Ubuntu is found, 2026-11-14

Record: T-1871 (partial).
Deployed: version 0.2.0-beta.18.

- The doctor names the QEMU version, the firmware pair, the free disk,
  the file-size ceiling, a host that is a VM, the state of each Windows
  name, and the next command; it exits 1 with the install line where no
  QEMU is found.
- The firmware search took `OVMF_CODE.fd`, and Debian and Ubuntu ship
  `OVMF_CODE_4M.fd`; podvm now finds the code and the vars of one build.

### Added: the answer file of an unattended Windows install, 2026-11-14

Record: T-1866 (partial).
Deployed: version 0.2.0-beta.18.

- `podvm-windows` builds `autounattend.xml` for Windows 11, Server 2025
  and Windows 10 with a random password; its media and the install
  itself come with T-1866 and T-1867.

### Added: `podvm run windows:NAME`, and `windows:dos` with no setup, 2026-11-14

Record: T-1861 (partial).
Deployed: version 0.2.0-beta.18.

- A catalog of Windows guests with pinned URL, size and sha256 of each:
  `windows:dos`, `windows:validationos`, `windows:11`, `windows:2025`,
  `windows:2022`, `windows:2019` and `windows:10`. `podvm run
  windows:dos ver` now fetches FreeDOS itself, checks it and boots it,
  with no flag and no script. The other names are refused by name until
  the steps that turn their download into a disk exist.

### Fixed: the incus tier refuses the options that its instance ignored, 2026-11-14

Record: T-1854.
Deployed: version 0.2.0-beta.18.

- `run --podvm-tier=incus` took `--user`, `--read-only`, `--podvm-confine`,
  the limits, `--timeout`, `-t` and more, and dropped them; it now
  refuses each by name with exit 125, as the machine tier does
  (T-1808). A test holds the table of each option on each guest tier.

### Fixed: a restarted container shows no exit code of the run before, 2026-11-14

Record: T-1807 (partial).
Deployed: version 0.2.0-beta.18.

- After `restart` or `start`, `inspect` showed `running` with the exit
  code and the end time of the run before; they are cleared now while
  the new run runs.

### Fixed: concurrent `create` of an image that was never extracted, 2026-11-14

Record: T-1886, PodUtils/podvm#358.
Deployed: version 0.2.0-beta.18.

- Several `create`, `run`, `cp` or `extract` at once of a pulled image
  that was never extracted wrote one tree together, and the first to
  fail removed it under the others: about half of eight containers were
  lost. One extraction of an image runs at a time now; the others wait
  and use its tree. (A patch of the tester Nemo-010.)

### Fixed: the `--report` of a machine run, 2026-11-14

Record: T-1803.
Deployed: version 0.2.0-beta.18.

- The report of a run on the machine or incus tier read as a refusal,
  with no rung, image or exit code; it now names the tier, the image and
  its digest, the payload's exit code, and the timeout.

### Fixed: `--timeout` on the machine tier, 2026-11-14

Record: T-1809.
Deployed: version 0.2.0-beta.18.

- `run --podvm-tier=machine --timeout 2` killed the guest 10 s late and
  exited 125 with a line about QEMU's socket; it now kills at the bound
  and exits 137, as the other rungs do.

### Fixed: no emulator outlives podvm, 2026-11-14

Record: T-1884.
Deployed: version 0.2.0-beta.18.

- A machine run that podvm left on a signal (the SIGTERM of `timeout`,
  a SIGKILL) left its QEMU running. The emulator now dies with podvm.

### Fixed: `-t` ends the input of the payload, and `-d -t` is refused, 2026-11-14

Record: T-1815, T-1817.
Deployed: version 0.2.0-beta.18.

- `run -t` never gave the payload the end of its input, so `cat
  </dev/null` hung; podvm now sends the terminal's end of input (VEOF)
  when its stdin ends, on the rootfs rungs and on the machine tier.
- `run -d -t` and `create -t` took `-t` and gave the payload no
  terminal; they are refused by name now.

### Fixed: `logs` of a foreground run, `probe --cached`, and the `=` forms of two machine flags, 2026-11-14

Record: T-1806, T-1811, T-1812.
Deployed: version 0.2.0-beta.18.

- The log of a foreground run held stdout only; it now holds stderr too,
  each write in its order.
- `probe --cached` gave the rung that the probes select, where `probe`
  gives the rung that `run` enters; both now give the entered rung.
- `--podvm-net=none` and `--podvm-channel=V` were refused as "a bug in
  podvm"; they parse as the space forms. `start`, `ps` and `rm` refuse a
  tier other than incus by name.

### Fixed: `remote ssh` runs no `node` of PATH that is not its helper, 2026-11-14

Record: T-1821.
Deployed: version 0.2.0-beta.18.

- With no podvm-ssh helper beside podvm, `remote ssh serve` ran the
  first `node` on PATH, Node.js on most hosts. podvm now takes a helper
  beside itself, or on PATH only as `podvm-ssh-NAME`, and refuses by
  name otherwise.

### Fixed: an `rmi` after `rm` is no longer refused while the spare tree fills, 2026-11-14

Record: T-1781 (correction), PodUtils/podvm#357.
Deployed: version 0.2.0-beta.18.

- The detached fill of the spare tree took the image's payload hold, so
  an `rmi` or `image prune` in the seconds after `rm` was refused as "in
  use by a running container" while `ps -a` was empty. The fill now
  holds a lock of its own, which a removal does not wait for; a fill
  that loses its source removes its partial tree. (A patch of the
  tester Nemo-010.)

### Fixed: `exec`, `save`, `load` and `import` exit 1 for a missing image or an invalid name, 2026-11-14

Record: T-1842 (partial), PodUtils/podvm#356.
Deployed: version 0.2.0-beta.18.

- `exec` of an image that the store does not have, `save` of a missing
  image and `import` of an invalid name exited 125; they exit 1 now, as
  `inspect`, `rmi` and docker do. An I/O failure stays 125.

## 0.2.0-beta.17 (2026-11-14)

### Fixed: a missing container name exits 1, 2026-11-14

Record: T-1805.
Deployed: version 0.2.0-beta.17.

- `start`, `stop`, `restart`, `rm`, `logs`, `wait`, `kill` and `cp` on a
  name that resolves to nothing exited 125; they now exit 1 with one
  line, as docker does. `kill` of an exited container and `rm` of a
  running one exit 1, and `start` of a running container exits 0.

### Fixed: a glibc 2.34 image with no `libdl.so.2` on the userland rung, 2026-11-14

Record: T-1880, PodUtils/podrunner#100.
Deployed: version 0.2.0-beta.17.

- Where chroot is denied, `podvm run busybox:1.37.0` refused: the glibc
  object of the interposer needed `libdl.so.2`, which a glibc 2.34 image
  can omit. podvm now carries a second glibc object for glibc 2.34 and
  later, which needs no `libdl.so.2`, and takes it where the first is
  declined.

### Fixed: a full stdout, 2026-11-14

Record: T-1816.
Deployed: version 0.2.0-beta.17.

- With stdout on a full disk (`/dev/full`), `version`, `ps`, `probe`,
  `doctor` and `inspect` aborted with exit 134, and `images` and `logs`
  exited 0 with their answer lost. Each verb now exits 125 with one line
  on stderr that names the error.

## 0.2.0-beta.16 (2026-11-14)

### Fixed: `exec=` of `--podvm-confine` on the userland rung, 2026-11-14

Record: T-1819.
Deployed: version 0.2.0-beta.16.

- On the userland rung, `--podvm-confine` did not deny a script or a
  program outside its `exec=` paths and the system directories: the
  kernel executed the loader of the image, which the layer grants. The
  interposer now checks each program against the exec table of the
  layer and refuses with EACCES. A static program that `exec=` grants
  ran with 126; it now runs.

### Fixed: `--no-host-cas` on an image with no CA bundle, 2026-11-14

Record: T-1818.
Deployed: version 0.2.0-beta.16.

- On an image that ships no CA bundle, `--no-host-cas` still put the
  host bundle (or the file that `SSL_CERT_FILE` names) into the image.
  podvm now gives the completion no host bundle under the flag.

### Fixed: a sealed bundle with a member outside the guest root is refused, 2026-11-14

Record: T-1810.
Deployed: version 0.2.0-beta.16.

- `run --podvm-sealed` skipped a member such as `../x` and exited 0, and
  put a member `/x` at the root of the guest. A member with `..` or a
  leading `/` now refuses the bundle with exit 125 before the boot, and
  an empty stream says that the bundle is empty.

### Fixed: the machine tier refuses the flags that its guest ignored, 2026-11-14

Record: T-1808.
Deployed: version 0.2.0-beta.16.

- `run --podvm-tier=machine` took `--podvm-confine`, `--user`,
  `--add-host`, `--read-only`, `--tmpfs`, `--ulimit`, `--security-opt`,
  `--cap-drop` and `--label` and dropped them: the payload ran as root
  with each capability. It now refuses each by name with exit 125 before
  the boot.

### Fixed: `--read-only` kept a `:ro` volume with a trailing `/` writable, 2026-11-14

Record: T-1804.
Deployed: version 0.2.0-beta.16.

- `run --read-only -v HOST:/data/:ro` let the payload write to the
  volume: the table held `rw:/data/` and `ro:/data`, and the `rw:` entry
  won. podvm now keeps one spelling of each guest path, and in the
  interposer an `ro:` entry wins a tie.

### Fixed: a refused run writes no `-e` value into its report, 2026-11-14

Record: T-1802.
Deployed: version 0.2.0-beta.16.

- The report of a run that podvm refused before it prepared the run held
  the argv of podvm in `argv`, with each `-e NAME=VALUE`. It now holds
  the command of the payload where the parse read it, and `[]` before
  that, and it names the image of the parse.

### Fixed: `--report` after the image is the payload's, 2026-11-14

Record: T-1801.
Deployed: version 0.2.0-beta.16.

- `run` read `--report PATH` anywhere before `--`, also among the
  arguments of the payload, and replaced PATH with the report: `podvm run
  IMG true --report important.txt` overwrote the file. Only the parser
  of `run` reads `--report` now, so each word after the image is the
  payload's, as the help says. A refusal after the flag still writes its
  report.

### Fixed: `ps` with a multibyte character in the command, 2026-11-14

Record: T-1800.
Deployed: version 0.2.0-beta.16.

- `ps` cut the COMMAND column at byte 17 and aborted (exit 134) when
  the cut fell inside a multibyte character; the listing stayed broken
  until the record was removed. It now cuts between characters.

### Changed: podcrates v0.43.0, 2026-11-14

Record: PodUtils/podcrates#149, PodUtils/podvm#331, #335, #337.
Deployed: version 0.2.0-beta.16.

- podvm takes the crates of podcrates at `v0.43.0`. It carries the
  completion of two layers of `pod-rootfs` (podcrates@T-567), and the
  text fixes podcrates@T-568 to podcrates@T-570: an ABI reason with no
  verdict word, the `noexec` remedy only on a `noexec` mount, and the
  legs of a refusal joined with `; `.
- `probe` prints the refusal of the machine and supervise tiers once,
  with no second `refused:` before it, and `system abi` wraps a refusal
  at 76 columns.

## 0.2.0-beta.15 (2026-11-14)

### Fixed: `system df` counts each byte of the store, 2026-11-14

Record: T-1787, PodUtils/podvm#329.
Deployed: version 0.2.0-beta.15.

- `system df` prints the bytes of the container trees, of the spare
  trees and of the extractions that no image names, and counts the last
  in what `image prune` frees; `image prune` removes them.

### Fixed: two lines of the interposer about identity, 2026-11-14

Record: T-1784, T-1785, PodUtils/podvm#333, #332.
Deployed: version 0.2.0-beta.15.

- A refused `setuid` or `setgid` names the uid that the payload keeps,
  as the kernel answers it; before, it said uid 0 on a host where the
  payload ran as another id.
- The notice of `--user` that a refused identity call was faked comes
  once in a run, not once in each process of the payload.

### Fixed: a usage error goes to stderr, 2026-11-14

Record: T-1783, PodUtils/podvm#334.
Deployed: version 0.2.0-beta.15.

- `save`, `import`, `start`, `logs`, `stop`, `kill`, `restart`, `wait`
  and `rm` print a usage error to stderr, so `podvm save > image.tar`
  with no image no longer writes the usage into the file. `--help`
  stays on stdout.

### Fixed: the stdin spool of `load`, 2026-11-14

Record: T-1782, PodUtils/podvm#336.
Deployed: version 0.2.0-beta.15.

- `podvm load` from stdin spools into the store's own staging area, not
  to a predictable name of `$TMPDIR`. A link planted there could make
  podvm overwrite another file, the spool was readable by other
  accounts, and a missing or small `$TMPDIR` refused the load.

### Changed: podcrates v0.42.0, 2026-11-14

Record: PodUtils/podcrates#145.
Deployed: version 0.2.0-beta.15.

- podvm takes the crates of podcrates at `v0.42.0`, which carries the
  overlay root of `pod-enter` (podcrates@T-486) and the published schema
  of the report of `pod-hostprobe` (podcrates@T-107); podvm builds and
  tests with no change of code.

### Changed: a spare tree of each image, 2026-11-14

Record: T-1781.
Deployed: version 0.2.0-beta.15.

- After a run, podvm fills the next tree of the same image in the
  background, and the next run takes it with one rename; the tree of a
  `--rm` run is removed after podvm exits. A repeated `run --rm
  golang true` takes 181 ms in place of 8.5 s on the cloud host.
  `PODVM_SPARE_TREE=0` turns it off.
- `rmi` and `image prune` remove the extracted root of an image that no
  tag names; before, it stayed on disk.

### Changed: podcrates v0.41.0, 2026-11-14

Record: PodUtils/podcrates#143.
Deployed: version 0.2.0-beta.15.

- podvm takes the crates of podcrates at `v0.41.0` (the termios mode of
  `pod-term`, relays of `pod-link`); podvm builds and tests with no
  change of code.

## 0.2.0-beta.14 (2026-11-13)

### Added: a container engine in a guest of the machine tier, 2026-11-13

Record: T-1780, PodUtils/podrunner#92.
Deployed: version 0.2.0-beta.14.

- `run --podvm-tier=machine --podvm-engine` loads the modules of a
  container engine in the guest: the bridge, `veth`, nf_tables and the
  modules of `iptables`, and overlayfs, out of the pinned modloop. Docker
  27.5.1 runs there with overlay2, its bridge networks with their DNS,
  and its port mappings, with no NIC in the guest.
- Each guest mounts cgroup2 at `/sys/fs/cgroup`, and a guest with no NIC
  has `lo` up.

## 0.2.0-beta.13 (2026-11-13)

### Fixed: the channel of the incus tier needs no proxy device, 2026-11-13

Record: T-1779, PodUtils/podrunner#90.
Deployed: version 0.2.0-beta.13.

- `--podvm-channel` on the incus tier mounts the host socket into the
  instance, and podvm's init relays `127.0.0.1:PORT` to it. The `proxy`
  device of beta.12 did not start on the packaged incus 6.0.0 of Ubuntu
  24.04 on kernel 7.0. The caller gives the socket file a mode that the
  root of the instance can use (T-1779 has the detail).

### Changed: `run` takes the probe from the cache, 2026-11-13

Record: T-1746, PodUtils/podcrates#124 (ask V-2 of podswarm).
Deployed: version 0.2.0-beta.13.

- `run` and `create` take the findings of the probe from
  `$PODVM_STORE/probe.json` where its key matches the confinement and
  the same podvm build wrote it, and probe again where it does not. A
  warm cache saves about 80 ms of each run on the cloud host.
  `PODVM_DEBUG=1` prints whether the probe came from the cache.
- `probe --json` holds the key `hostprobe`: the findings and the
  selection in the versioned object of `pod-hostprobe`.

### Changed: podcrates v0.40.0, 2026-11-13

Record: PodUtils/podcrates#140.
Deployed: version 0.2.0-beta.13.

- podvm takes the crates of podcrates at `v0.40.0`. The release adds
  carriers of `pod-link` and fields of `pod-job` that podvm does not use
  yet; podvm builds and tests with no change of code.

## 0.2.0-beta.12 (2026-11-13)

### Added: no NIC and a host channel on the incus tier, 2026-11-13

Record: T-1779, PodUtils/podrunner#88.
Deployed: version 0.2.0-beta.12.

- `--podvm-net none` and `--podvm-channel HOST_SOCKET:127.0.0.1:PORT`
  serve the incus tier (`run`, `run -d`, `create`): the instance has no
  NIC, and each connection to the port in the instance reaches the host
  socket through a proxy device. podvm's init of the instance brings
  `lo` up.

### Added: a sealed run of the machine tier, 2026-11-13

Record: T-1751, PodUtils/podcrates#124 (ask V-7 of podswarm).
Deployed: version 0.2.0-beta.12.

- `run --podvm-tier=machine --podvm-sealed IMAGE < bundle.tar` takes the
  command, the environment, the working directory and the files of the
  run from a tar bundle on stdin (`.podvm-run.json` names the run). The
  bundle reaches the guest through the channel only: no host file, no
  argv and no environment of a host process holds a value of the run.
  The guest has no NIC. The guest's memory is locked where
  `RLIMIT_MEMLOCK` allows it.

## 0.2.0-beta.11 (2026-11-13)

### Fixed: a loopback proxy of a guest with no NIC, 2026-11-13

Record: T-1778.
Deployed: version 0.2.0-beta.11.

- With `--podvm-net none`, the machine tier keeps a proxy variable on
  `127.0.0.1` as it is, so the payload reaches `--podvm-channel` through
  it. Before, podvm wrote the gateway 10.0.2.2, which a guest with no NIC
  does not have (PodUtils/podrunner#87).

### Fixed: the devices of the userland rung, 2026-11-13

Record: T-1774.
Deployed: version 0.2.0-beta.11.

- On the userland rung under a user namespace, `/dev/urandom`,
  `/dev/random`, `/dev/zero` and `/dev/null` are the host's devices, not
  the regular files of 1 MiB that the completion writes where mknod(2)
  is refused. busybox `head -c 2000000 /dev/urandom` gives 2000000 bytes.
- A static program on the native-static rung keeps the files of the tree
  for these paths: that rung serves directory volumes only.

## 0.2.0-beta.10 (2026-11-13)

### Changed: podcrates v0.39.0, 2026-11-13

Record: PodUtils/podcrates#137.
Deployed: version 0.2.0-beta.10.

- podvm takes the crates of podcrates at `v0.39.0`. The channel of a guest
  (`--podvm-channel`) runs on its `pod-link`; a guest still shows `lo`
  only and reaches the host socket.

### Fixed: the help of the channel, and the directory of its socket, 2026-11-13

Record: T-1751.
Deployed: version 0.2.0-beta.10.

- `run --help` and `podvm man` list `--podvm-net` and `--podvm-channel`.
- The socket of the channel is in a directory of mode 0700, new for each
  run: no other user of the host can connect to it before podvm.

### Added: stdin for an incus instance that stays, 2026-11-13

Record: T-1777, PodUtils/podrunner#85.
Deployed: version 0.2.0-beta.10.

- `run -d -i --name NAME --podvm-tier=incus` gives the piped stdin to the
  command of the instance: a file of root, mode 0600, that goes once the
  command holds it. It is never in argv or the environment.

## 0.2.0-beta.9 (2026-11-13)

### Fixed: `tty` on the namespace rung, the knob flags of `lab`, the owners in a guest, 2026-11-13

Record: T-1760, T-1770, T-1776, PodUtils/podcrates#136, PodUtils/podrunner#81.
Deployed: version 0.2.0-beta.9.

- podvm takes the crates of podcrates at `v0.38.0`. The namespace rung
  binds `/dev/pts`, and each rootfs has a `dev/pts`: `tty` in a `run -t`
  names its pty.
- `podvm lab scenarios` prints lines that `lab plan` takes, and a bad
  value of a knob names its range or its words.
- In a guest of the machine tier, each file of the image has the owner and
  the group that the image gives it, not root's: a non-root user of the
  image can run its programs.

### Added: a guest with no NIC and one channel to a host socket, 2026-11-13

Record: T-1751, PodUtils/podrunner#83.
Deployed: version 0.2.0-beta.9.

- `run --podvm-tier=machine --podvm-net none` boots a guest with no
  network card: `ip link` shows `lo` only.
- `--podvm-channel HOST_SOCKET:127.0.0.1:PORT` carries each TCP
  connection to the port in the guest to the Unix socket of the host, over
  one `virtio-serial` port.

## 0.2.0-beta.8 (2026-11-13)

### Fixed: /proc on the namespace rung, `exec` in the run's mounts, `-w`, 2026-11-13

Record: T-1758, T-1765, T-1772, T-1773, T-1775, PodUtils/podcrates#134.
Deployed: version 0.2.0-beta.8.

- podvm takes the crates of podcrates at `v0.37.0`. The namespace rung
  mounts /proc: `go version` and `/proc/self/status` work there.
- `exec` in a running container of the namespace rung joins the payload's
  mount namespace: it sees the run's `/tmp` and `/proc`, and `cp` to or
  from such a container goes through the same mounts.
- `run -w DIR` makes a directory that the image lacks, as docker does.
- On the userland rung, a step of the completion (`openssl rehash`) is
  skipped with a note, not failed on each run.

### Fixed: volumes, the image's users and hosts, and environments of programs, 2026-11-13

Record: T-1761, T-1753, T-1756, T-1765.
Deployed: version 0.2.0-beta.8.

- `run -v HOST:GUEST`, `:ro`, `--tmpfs` and `--read-only` work on the
  default rung of a root user. Before, each exited 125 with a message that
  named the chroot rung.
- On the userland rung, `getpwnam`, `getpwuid`, `getgr*`, `getgrouplist`,
  `gethostbyname*` and `getaddrinfo` read the image's `/etc/passwd`,
  `/etc/group` and `/etc/hosts`. Before, `id` named the host's users, and
  `--add-host` did not resolve.
- A program that runs a child with an environment of its own (the
  triggers of apk, `env -i`) keeps the path table of the userland rung.
  `apk add` works there.
- `-e NAME` takes the caller's value of NAME, as docker. `exec` takes the
  environment that the container started with, then its own `-e`.

### Fixed: terminals, /dev/shm, the loader under confine, and the rung of probe, 2026-11-13

Record: T-1760, T-1759, T-1764, T-1766.
Deployed: version 0.2.0-beta.8.

- `run -t` gives the payload a pty, also where podvm has no terminal, and
  makes the caller's terminal raw for the run.
- Each rootfs has a `/dev/shm` (mode 1777): python's `multiprocessing`
  works on both rungs.
- `--podvm-confine` lets the dynamic loader read `/etc/ld.so.cache`.
- `probe` names the rung that `run` enters: `userland` where chroot is
  denied. `probe --strict` gates on it.

### Fixed: speed and noise, 2026-11-13

Record: T-1762, T-1763, T-1767, T-1768, T-1769.
Deployed: version 0.2.0-beta.8.

- The verdicts of the HTTPS probe of the source fixup are cached in the
  store. A run of ubuntu takes about 1.4 s, not 3.6 s.
- A step of the completion prints its output only where it fails. A quiet
  python run printed 108 lines of `openssl rehash`.
- `extract` no longer claims a chown EINVAL that did not happen; `pull -q`
  prints nothing on stderr; `restart` prints the name once.
- `cp -r` keeps hard links (busybox's `/bin` was 480 MiB, now 1.5 MiB).
- An unknown verb or flag prints a short refusal and the help command;
  `profile list` aligns its columns; a closed pipe ends podvm quietly.

### Fixed: `-u` with an ENTRYPOINT script, and the path calls of common tools on the userland rung, 2026-11-13

Record: T-1757, T-1754, T-1755.
Deployed: version 0.2.0-beta.8.

- `run -u USER` reaches a payload whose first program is a `#!` script
  (the ENTRYPOINT of the node image). Where `-u` cannot reach the payload
  (a static program), the run refuses with exit 125; before, it ran as
  the image's user.
- On the userland rung the interposer maps the paths of `scandir64`,
  `lutimes`, `futimesat`, `lchmod`, `mkfifo`, `mkfifoat`, `mknodat`,
  `pathconf`, `statfs64`, `statvfs64`, `truncate64`, `inotify_add_watch`,
  `execl`, `execlp` and `execle`. `apt-get install` in ubuntu works, and
  node reads the directories of the image, not the host's.

### Changed: podcrates v0.36.0, 2026-11-13

Record: PodUtils/podcrates#129.
Deployed: version 0.2.0-beta.8.

- podvm takes the crates of podcrates at `v0.36.0`. No behaviour of podvm
  changes.

### Added: the incus tier by configuration, `logs -f` and `inspect`, 2026-11-13

Record: T-1722.
Deployed: version 0.2.0-beta.8.

- A line `tier = incus` in `$store/config` makes `run` and `create` take
  the incus tier with no flag where an incusd answers, and `system info`
  names it as `.EnteredRung`. `exec`, `start`, `stop`, `rm`, `logs` and
  `inspect` act on an instance of podvm in incus by its name.
- `logs -f` follows the log of an incus instance, and `inspect` gives its
  state as JSON or through `--format`.

### Added: the crate podvm-api, a typed call for `run`, 2026-11-13

Record: T-1745, PodUtils/podcrates#124 (V-1).
Deployed: version 0.2.0-beta.8.

- A program that runs podvm can link `podvm-api` (by the git tag of this
  repository): a typed `Run` request starts the podvm binary of the same
  version and gives back the record of `--report`. The values of the
  environment go through a private env file, never through argv.

### Added: the JSON Schema of `run --report`, 2026-11-13

Record: T-1752, PodUtils/podcrates#124 (V-8).
Deployed: version 0.2.0-beta.8.

- Each release publishes `podvm-run-report.v1.schema.json`, the JSON
  Schema of the record of `run --report` at layout version 1. The field
  `schema` of each record names its version; a reader refuses a version
  that it does not know.

### Changed: podcrates v0.35.0, 2026-11-13

Record: PodUtils/podcrates#127.
Deployed: version 0.2.0-beta.8.

- podvm takes the crates of podcrates at `v0.35.0`. `pod-oci` sends no
  login to a registry over plain HTTP. No other behaviour of podvm
  changes.

### Fixed: a foreground payload ends when its podvm is killed, 2026-11-13

Record: T-1748, PodUtils/podcrates#124 (V-4).
Deployed: version 0.2.0-beta.8.

- When the `podvm` of a foreground `run` dies by SIGKILL, its payload now
  gets SIGKILL too, as the payload of `run -d` does with its launcher.
  Before, the payload ran on with no supervisor.

### Fixed: the open flags of the interposer on aarch64, 2026-11-13

Record: T-1744.
Deployed: version 0.2.0-beta.8.

- On aarch64 the interposer read the x86_64 values of `O_NOFOLLOW`,
  `O_DIRECTORY`, `O_DIRECT` and `O_LARGEFILE`. An `O_NOFOLLOW` open of a
  link in the rootfs followed the link there. Each flag now has the value
  of the target.

### Added: `podvm lab scenarios` and `podvm lab plan`, 2026-11-13

Record: T-1662 (partial), PodUtils/podcrates#103.
Deployed: version 0.2.0-beta.8.

- `podvm lab scenarios` lists the 27 offline network labs of sand-net,
  and `podvm lab plan SCENARIO` (or the knob flags `--stack`, `--nat`,
  `--policy` and the rest) prints the steps that build and remove the lab
  in a guest and the verdict that each probe must give. A bad knob exits
  125 by name. `lab up`, `down`, `check`, `try` and `matrix` refuse until
  the guest parts of `pod-netlab` exist.

## 0.2.0-beta.7 (2026-11-13)

### Fixed: GNU tar with an absolute `-C` where chroot is denied, 2026-11-13

Record: T-1743, PodUtils/podcrates#102, PodUtils/podrunner#70.
Deployed: version 0.2.0-beta.7.

- The interposer maps the paths of the fortified opens of glibc
  (`__open_2`, `__open64_2`, `__openat_2`, `__openat64_2`). Before, a
  program built with `_FORTIFY_SOURCE` that opened a path with flags known
  only at run time opened it on the host. GNU tar of Ubuntu 24.04 does so
  for the directory of `-C`: `tar -xf x.tar -C /abs/dir` failed, or wrote
  the members into a host directory of that name.

### Changed: podcrates v0.34.0, 2026-11-13

Record: PodUtils/podcrates#103, PodUtils/podcrates#104.
Deployed: version 0.2.0-beta.7.

- podvm takes the crates of podcrates at `v0.34.0` (through `v0.33.0`).
  No behaviour of podvm changes. The breaking changes of the two tags
  (`pod-session`, `pod-netstack`) touch no crate that podvm uses.

## 0.2.0-beta.6 (2026-11-12)

### Changed: the relay of podvm-ssh is pod-relay, 2026-11-12

Record: T-1678, PodUtils/podcrates#93.
Deployed: version 0.2.0-beta.6.

- podvm takes podcrates `v0.32.0`. The relay node and operator of
  podvm-ssh run over `pod-relay`'s blocking facade; the wire stays
  `reverse-v1`. A token under 8 characters, or a name that starts with
  `-` or `.`, now exits 125; a wrong node token ends the node with 125;
  the lines that the node and the operator print change.

### Added: the interposer on aarch64, 2026-11-12

Record: T-1721, PodUtils/podrunner#52.
Deployed: version 0.2.0-beta.6.

- `podvm` for aarch64 embeds aarch64 interposer objects (glibc and musl),
  so the userland rung runs arm64 payloads where chroot is denied.
  Experiment 356 exits 0 on a hosted arm64 runner. The native-static rung
  stays x86_64 only.

### Fixed: podcrates v0.31.0, 2026-11-12

Record: PodUtils/podcrates#90, T-1741, T-1742.
Deployed: version 0.2.0-beta.6.

- podvm takes the crates of podcrates at `v0.31.0`. `extract` and `run` no
  longer refuse a correct plain tar layer whose end padding is past the last
  read (issue 281), and the loader family finds the glibc of a Debian image
  on arm64 (issue 282).

### Changed: a volume of the incus tier, 2026-11-12

Record: T-1722.
Deployed: version 0.2.0-beta.6.

- `run --podvm-tier=incus -v HOST:GUEST[:ro]` maps the host directory into
  the instance as a disk device, so a write lands on the host.
- The incus image of an OCI image carries podvm, so a run pushes no binary
  into the instance.
- `run -d` and `create` with `--podvm-tier=incus` keep the instance; `ps`,
  `start`, `stop`, `rm`, `logs` and `exec` with the tier act on it.

### Changed: a volume of the machine tier is a 9p mount, and -t a terminal, 2026-11-12

Record: T-1723.
Deployed: version 0.2.0-beta.6.

- `run -t --podvm-tier=machine` runs the command on a terminal of the
  guest with the size of the host terminal, and stdin goes there live.
- `run --podvm-tier=machine -v HOST:GUEST[:ro]` mounts the host directory
  in the guest with 9p, so a write in the guest lands on the host, and
  `:ro` refuses a write. podvm takes the 9p modules out of the pinned
  modloop of the guest kernel. A kernel of `PODVM_MACHINE_KERNEL` keeps the
  copy of before.
- The guest keeps `-M pc,acpi=off`: under KVM the pinned kernel boots it in
  561 ms against 1 808 ms for `-M microvm` (experiment 440).

### Added: the runsc tier, 2026-11-12

Record: T-1729.
Deployed: version 0.2.0-beta.6.

- `run --podvm-tier=runsc` runs the image under gVisor's `runsc`, rootless
  with platform systrap, where the host grants user namespaces. Each `-v`
  is a bind mount, so a write lands on the host. Where the host has no
  `runsc` or denies user namespaces, the tier refuses by name with exit
  125. Experiment 436 is the proof.

### Changed: podcrates v0.30.0, 2026-11-12

Record: PodUtils/podcrates#89.
Deployed: version 0.2.0-beta.6.

- podvm takes the crates of podcrates at `v0.30.0`. No behaviour of podvm
  changes.

### Changed: podcrates v0.29.0, 2026-11-12

Record: PodUtils/podcrates#87.
Deployed: version 0.2.0-beta.6.

- podvm takes the crates of podcrates at `v0.29.0`. No behaviour of podvm
  changes. `run --help` names each family of the userland rung that takes
  `--podvm-confine`.

### Changed: podcrates v0.28.0, 2026-11-12

Record: T-1641, T-1661, PodUtils/podcrates#86.
Deployed: version 0.2.0-beta.6.

- podvm takes the crates of podcrates at `v0.28.0`. `pod-guest` writes
  the provisioned image of `podvm windows setup` with `qemu-img convert`
  (COORD-056): `qemu-img commit` took no output file.
- The pins of workerd and wasmtime are facts of `pod-services` now
  (`cloudflare-workerd-npm`, `wasmtime-release`); podvm keeps no copy.
  No host of another party waits for `pod-services`.
- `pod-confine` refuses a TCP list on a target with `socketcall(2)` (i686,
  powerpc64le) by name.

### Added: a dynamic program on the native-static rung, 2026-11-12

Record: T-1731.
Deployed: version 0.2.0-beta.6.

- The native-static rung and `--podvm-tier=syscall` run a dynamic
  program of the image: podvm's loader maps the image's interpreter
  beside it, and each library resolves in the image. A raw system call
  of the program (Go with cgo, liburing) also reads the image's files.

### Changed: podcrates v0.27.0, 2026-11-12

Record: T-1658, PodUtils/podcrates#85.
Deployed: version 0.2.0-beta.6.

- podvm takes the crates of podcrates at `v0.27.0`. On the namespace
  rung, the confine layer now covers the tmpfs at `/tmp`: a profile that
  asks to write `/tmp` (`errand`) writes it. The layer is complete on each
  rung, and T-1658 is done.
- `mcr.microsoft.com` (the image of `playwright-chromium`) is in
  `pod-services`.

### Added: the profiles workerd-test and wasmtime-wasi, 2026-11-12

Record: T-1661.
Deployed: version 0.2.0-beta.6.

- `podvm run --profile wasmtime-wasi -- MODULE` runs `wasmtime run
  --dir=/workspace` of a WASI module: a pinned wasmtime 50.0.1, checked
  by SHA-256 and kept in the store. podvm fetches the release with `gh
  release download` first, and takes the public URL where that call
  fails (T-1667).
- `podvm run --profile workerd-test` runs `workerd test` of
  `/workspace/worker.js`: a pinned workerd (the npm package
  1.20261029.1, checked by SHA-256 and kept in the store), a config with
  no outbound service, and the confine layer with no TCP connect.
- `net.ports = []` in a profile holds each TCP connect of the confine
  layer; with no `net.ports`, TCP stays free.
- A profile that names its image runs with no command.

### Fixed: Chromium's child processes on the loader family, 2026-11-12

Record: T-1739.
Deployed: version 0.2.0-beta.6.

- A child process that closed the interposer's memo descriptor read the
  loader's path from `/proc/self/exe`, and Chromium's children stopped
  with SIGTRAP. The interposer now gives the program's path also with no
  memo descriptor. The profile `playwright-chromium` runs Chromium's
  default mode, with child processes, and no longer `--single-process`.

### Added: the confine layer on the dispatcher rungs, 2026-11-12

Record: T-1658.
Deployed: version 0.2.0-beta.6.

- `--podvm-confine` runs on the native-static rung, the syscall tier and
  the emulate tier. podvm gives the host path of its own dispatcher, and
  of the host qemu, read and execute. Before this change, these rungs
  refused the layer by name. Experiment 414 clause 4 shows it.

## 0.2.0-beta.5 (2026-11-12)

### Changed: podcrates v0.26.0, 2026-11-12

Record: PodUtils/podcrates#77.
Deployed: version 0.2.0-beta.5.

- podvm takes the crates of podcrates at `v0.26.0`. No behaviour of podvm
  changes.

### Added: headless Chromium of Playwright, 2026-11-12

Record: T-1694, T-1738, T-1739.
Deployed: version 0.2.0-beta.5.

- `podvm run --profile playwright-chromium -- --dump-dom file:///workspace/page.html`
  runs Playwright's headless Chromium under the confine layer where
  chroot is denied, and `--screenshot=/workspace/s.png` writes a PNG.
  `--no-sandbox` and `--single-process` are set; Chromium under podvm is
  no boundary against a hostile page.
- Fixed: the interposer allocated the buffers that a caller frees with
  libc's `malloc`, so a program with its own allocator (Chromium) aborted
  in `free`.
- A profile takes `fs.exec`: paths that the confine layer gives read and
  execute.

### Added: a JSON report of each run, and image labels, 2026-11-11

Record: T-1695.
Deployed: version 0.2.0-beta.5.

- `run --report PATH` writes one JSON document of the run to PATH, also
  when podvm refuses: the rung, the exit code or the signal, the times,
  the CPU time, the peak resident set, the limits and the labels.
- `images --filter label=KEY[=VALUE]` matches the labels of the image's
  OCI config.

### Added: labels on container records, 2026-11-11

Record: T-1695.
Deployed: version 0.2.0-beta.5.

- `run --label KEY=VALUE` and `create --label` are kept in the record:
  `ps --filter label=KEY[=VALUE]` finds the container, and `inspect
  --format '{{.Labels}}'` prints them. Before, the flag did nothing.

### Added: the confine layer on the loader family, 2026-11-11

Record: T-1658; issue 170.
Deployed: version 0.2.0-beta.5.

- Where chroot is denied, a dynamic program of the image (the loader
  family of the userland rung) runs under `--podvm-confine` too: each
  guest path becomes its host path, so a `-v` volume can be granted.

### Added: the profile errand, 2026-11-11

Record: T-1658; issue 170.
Deployed: version 0.2.0-beta.5.

- `podvm run --profile errand IMAGE CMD` confines CMD with the policy of
  errand's bailey backend: the system directories, the ld.so files, the
  CA stores and a few files of `/etc` read, `/tmp` and two devices
  written, each other path denied. A SPEC path written `?/P` is optional.

### Added: the confine layer for run -d and exec, 2026-11-11

Record: T-1658; issue 170.
Deployed: version 0.2.0-beta.5.

- The container record keeps the SPEC of `--podvm-confine`, so a
  detached payload and each `exec` into its container run under the same
  layer.

### Changed: podcrates v0.25.0, 2026-11-11

Record: T-1658; PodUtils/podcrates#76.
Deployed: version 0.2.0-beta.5.

- The confine layer closes the Landlock scopes (signals, abstract Unix
  sockets), MPTCP under a port list and io_uring, and a confined root
  payload keeps docker's default capabilities only.

## 0.2.0-beta.4 (2026-11-11)

### Added: the confine layer, 2026-11-11

Record: T-1658; issue 170.
Deployed: version 0.2.0-beta.4.

- `run --podvm-confine='read=/etc/passwd;write=/out;connect=443'` (or a
  profile with `confine = true`) confines the payload with Landlock, a
  seccomp deny filter and no_new_privs on a rung with a chroot. Each path
  that no rule names is denied. Each case with no mechanism (another
  rung, `run -d`, no Landlock) refuses by name; a payload never runs
  without the layer that it asked for.

### Fixed: the profile lambda-al2023 runs, 2026-11-11

Record: T-1659; issue 172.
Deployed: version 0.2.0-beta.4.

- The layers of `public.ecr.aws/lambda/provided:al2023` apply, and the
  profile clears the image's entrypoint (a new key `fs.entrypoint`), so
  `podvm run --profile lambda-al2023 -- CMD` runs CMD as user 993 on a
  read-only root.

### Changed: podcrates v0.24.0, 2026-11-11

Record: T-1679; issue 263.
Deployed: version 0.2.0-beta.4.

- podvm takes the crates of podcrates at `v0.24.0`. `pod-services` lists
  each host that podvm names, and the sums of the NIC modules of the
  guest; podvm keeps no list of its own. A layer whose root entry has an
  empty name applies (`pod-extract`).

### Added: a payload that ignores the proxy variables reaches the broker, 2026-11-11

Record: T-1665; issue 184.
Deployed: version 0.2.0-beta.4.

- With `--network broker` on the syscall tier, each exact name of
  `--allow` is in the container's `/etc/hosts` with a fake address
  `198.18.0.N`. A `connect` to it becomes a tunnel of the broker, with the
  credential of the run, so a program that reads no proxy variable reaches
  the allowed host, and the broker's policy still applies.

### Changed: the SSH archive is packaged by `podvm-package-ssh`, 2026-11-11

Record: T-1737.
Deployed: version 0.2.0-beta.4.

- The release archive of the SSH helpers comes from a Rust binary of
  `podvm-release` in place of a shell script. The archive is
  byte-identical, with the same names, notices and checks.

### Added: provider shapes as profiles, 2026-11-11

Record: T-1659; issue 172.
Deployed: version 0.2.0-beta.4.

- Six built-in profiles put a tool in the shape of its production host:
  `lambda-al2023`, `vercel-sandbox`, `cloudflare-sandbox`, `busybox-bare`,
  `distroless-static` and `node-shell`. A profile names its image
  (`podvm run --profile busybox-bare -- sh`), its user and its variables,
  and lists what it leaves out. `needs` refuses an image that lacks a
  program. `--` ends podvm's flags.
- A static program of a volume runs on the native-static rung.

### Added: the egress broker, 2026-11-11

Record: T-1665; issue 184.
Deployed: version 0.2.0-beta.4.

- `run --network broker --ports P --allow HOST` holds the payload to an
  egress broker of the run (`pod-broker`), chained to the host's proxy:
  407 without the run's credential, 403 for a port, a host or an address
  off the lists, 200 through it. The syscall tier and the machine tier
  hold the payload to it; each other rung refuses the flag by name.

### Added: a loopback proxy reaches a guest of the machine tier, 2026-11-11

Record: T-1665; issue 184.
Deployed: version 0.2.0-beta.4.

- On the machine tier, a proxy variable of the payload that names the
  host's loopback (`127.0.0.1`, `localhost`, `::1`) names the gateway
  10.0.2.2 in the guest, with the scheme, the credentials and the port
  byte for byte. A proxy URL with no scheme or no port refuses the run by
  the name of the variable.

### Added: the scope group of the doctor, and the asset rule, 2026-11-11

Record: T-1667; issue 188.
Deployed: version 0.2.0-beta.4.

- `podvm doctor` checks each capability of the widened scope (the
  native-static rung by a run, the syscall tier, openat2, Landlock,
  user-mode networking, the emulate and incus tiers, the proxy, the space
  of the store, the assets) with ok, MISSING and a fix, or cannot be fixed
  on this host. `--strict` exits 1 on a MISSING.
- A kernel that `PODVM_MACHINE_KERNEL` names is recorded at its first use,
  and a change of it refuses. The fetch of the guest kernel checks the
  free space first.

### Added: the probe inside the payload, and compare, 2026-11-11

Record: T-1666; issue 186.
Deployed: version 0.2.0-beta.4.

- `podvm probe-inside [--podvm-tier T] [--profile P] -o FILE IMAGE` runs
  podvm's static probe as the payload and writes what it gets there:
  records `SECTION TARGET VERDICT DETAIL` for identity, escalation, escape
  surface, procfs, raw sockets, proxy, DNS and the network paths, with
  each secret-shaped value REDACTED. It needs no shell in the image.
- `podvm compare REPORT --reference REF [--expected FILE] [--strict]`
  compares two reports; `--strict` fails only on a difference in an
  invariant section that the expected-differences file does not explain.

### Fixed: a detached thread on the native-static rung, 2026-11-11

Record: T-1730.
Deployed: version 0.2.0-beta.4.

- A static payload whose thread ends detached (musl unmaps the thread's
  own stack) no longer dies with SIGSEGV on the native-static rung and
  the syscall tier.

### Added: the syscall tier, 2026-11-11

Record: T-1657; issue 169.
Deployed: version 0.2.0-beta.4.

- `run --podvm-tier=syscall` runs a static program on the native-static
  rung with the view of a syscall-interpreting tier: `uname -r` names the
  tier, `/tmp` is a directory of the run with a cap (`--tmpfs
  /tmp:size=`, 256 MiB by default), `/proc/self` names guest paths, and a
  `-v` volume writes through. It needs no ptrace, no qemu and no user
  namespace. It is a compatibility layer, not a boundary.

### Added: profiles, 2026-11-11

Record: T-1654; issue 166.
Deployed: version 0.2.0-beta.4.

- `podvm run --profile NAME [-s KEY=VALUE]` runs with a TOML recipe of run
  flags: built in (`default`, `workspace`, `readonly`, `machine`,
  `incus`) or under `$XDG_CONFIG_HOME/podvm/profiles`. `extends` merges a
  profile over its parent, and `-s` sets one key. An unknown key and a
  wrong type refuse, naming the key. `podvm profile list` and `podvm
  profile show NAME` print them.
- `--podvm-cpus N` gives a guest of the machine tier N vCPUs.

### Fixed: no terminal in a guest without -t, 2026-11-11

Record: T-1728.
Deployed: version 0.2.0-beta.4.

- On the machine tier, the command's stdout and stderr are pipes, as
  docker's are without `-t`. Before, they were the serial terminals, and
  `ls` wrote colour escapes into podvm's stdout.

### Changed: each container has its own rootfs, 2026-11-11

Record: T-1709; issue 234.
Deployed: version 0.2.0-beta.4.

- `run` and `create` copy the extracted image into the container's own
  tree, `<store>/trees/<id>/`, by `FICLONE` where the filesystem supports
  it, else by a kernel copy. A write in one container is not read by
  another container or by a later run, and `--add-host` values of two
  concurrent runs stay apart. `rm` and `--rm` remove the tree; `--rm`
  keeps the image's extracted tree. `--verbose` prints the cost of the
  copy.

### Added: the emulate tier, 2026-11-11

Record: T-1651; issue 94.
Deployed: version 0.2.0-beta.4.

- `run --podvm-tier=emulate` runs the payload under a static qemu-user of
  `PATH` on the native-static rung: each path stays in the image (a path
  absent from it is ENOENT), and an exec of the guest enters the emulator
  again. With `--platform`, an image of another architecture runs on it.
  qemu needs no patch and no `-L`.

### Changed: podcrates v0.21.0, and pod-incus for the incus tier, 2026-11-11

Record: T-1727; PodUtils/podcrates#63 and #65.
Deployed: version 0.2.0-beta.4.

- Each `pod-*` crate is at podcrates `v0.21.0`. The incus tier calls the
  REST API through `pod-incus`; podvm's own client is gone.
  Experiment 434 runs again against the rootless incusd.

### Fixed: absolute links of the image, and exec under :ro, 2026-11-11

Record: T-1725, T-1726; PodUtils/podrunner#61 and #62.
Deployed: version 0.2.0-beta.4.

- On the loader family, a path call resolves each link of the image in
  guest names: an absolute link such as `/usr/bin/python3 ->
  /usr/bin/python3.12` reads the file of the rootfs, not of the host.
- `podvm exec` into a container with a `:ro` volume or `--read-only`
  applies the read-only layer of its run; it wrote there before.

### Added: a native-static rung, 2026-11-11

Record: T-1652, T-1724; issue 109.
Deployed: version 0.2.0-beta.4.

- Where chroot is denied, a static x86_64 payload runs in a podvm process
  of its own: Syscall User Dispatch gives each of its system calls to
  podvm, which resolves each path in the image with
  `openat2(RESOLVE_IN_ROOT)` (crate `podvm-dispatch`). The golang image's
  `go version` and `go build` run on it. It was a refusal at 125 (T-1648),
  which stays where the kernel refuses the `prctl`; `--unsafe-host-paths`
  keeps the memfd family. `podvm probe --json` reports
  `measured_by.user_dispatch`.
- A static child of a dynamic program on the loader family (`go build`
  under `sh`) enters the native-static rung through podvm, where it wrote
  on the host's root before (T-1724).
- A volume (`-v HOST:GUEST`) serves a static payload on the native-static
  rung; `:ro` and `--read-only` are refused there by name (T-1724).

### Added: run on an incusd, 2026-11-11

Record: T-1718; issues 243, 251 and 253.
Deployed: version 0.2.0-beta.4.

- `run --podvm-tier=incus IMAGE CMD` runs the command in a disposable
  instance of an incusd that answers on `PODVM_INCUS_SOCKET` or on
  `INCUS_DIR`'s socket, with the image's owners from `.meta.jsonl`, and
  gives its stdout, stderr and exit code apart. Piped stdin, `-e` and `-w`
  reach the command. `create`, `exec`, `-v`, `--podvm-mem` and
  `--podvm-qemu-arg` are refused by name with the tier, and so is a run
  where no incusd answers ([incus](docs/incus.md), experiment 434).

## 0.2.0-beta.3 (2026-11-11)

### Changed: a binary embeds no interposer of another machine, 2026-11-11

Record: T-1721; PodUtils/podrunner#52.
Deployed: version 0.2.0-beta.3.

- `podvm-aarch64` embedded the two x86_64 interposer objects, so the
  userland rung declined each arm64 payload with a machine mismatch. A
  binary for another target now embeds none and says so; the aarch64 port
  of the interposer is T-1721. The smoke of each binary (`podvm-smoke`, in
  `nightly.yml` and `gate.yml`) takes `absent` off x86_64 and fails an
  embed of another machine.

### Fixed: the loader finds the image's library directories, 2026-11-11

Record: T-1719; issue 247.
Deployed: version 0.2.0-beta.3.

- On the userland rung the image's loader searches `/usr/local/lib`, each
  directory of the image's `/etc/ld.so.conf` or `ld-musl-ARCH.path`, and
  not the host's `ld.so.cache` (podcrates `v0.19.0`, `--inhibit-cache` for
  glibc 2.16 and later, also for each child).

### Added: a microVM from an OCI image, 2026-11-11

Record: T-1656 (partial), T-1682 (in progress); issue 168.
Deployed: version 0.2.0-beta.3.

- `run --podvm-tier=machine IMAGE CMD` boots a disposable guest from the
  image and exits with the command's code: under KVM where `/dev/kvm`
  opens, under TCG elsewhere (about 2.4 s for each boot under TCG on the
  cloud host). `PODVM_MACHINE_ACCEL=kvm` refuses by name without KVM.
  The guest kernel and its network modules are pinned and fetched into
  the store; the guest reaches the host's loopback at 10.0.2.2. `-v` is a
  copy into the guest, and the run says so. stderr comes back apart; piped
  stdin reaches the guest. About 0.6 s for each run under KVM on a hosted
  runner.
- `podvm machine ssh` prints the accelerator that it took.
- `.github/workflows/kvm.yml` runs the guest of experiment 390 on a hosted
  runner with `/dev/kvm`, dispatched by hand.

### Fixed: a relative program runs from `-w`, and SIGXFSZ has a reason, 2026-11-11

Record: T-1707, T-1706; issues 232 and 231.
Deployed: version 0.2.0-beta.3.

- `run -w /opt/x ./bin/prog` runs `/opt/x/bin/prog`, as docker does, on
  each rung; `argv[0]` stays `./bin/prog`. On the userland rung the
  interposer and `/proc/self/exe` take the same name.
- A foreground payload that dies on SIGXFSZ (exit 153) gets a line that
  names its soft `RLIMIT_FSIZE`; for a .NET payload, a second line names
  `DOTNET_EnableWriteXorExecute=0`.
- The official `actions/runner` v2.338.2 runs on the userland rung with
  no `--unsafe-host-paths`: `--version`, `config.sh --help`, and `run.sh`
  up to the long poll of a loopback stand-in of the service.
- An empty path reaches the kernel without a line of the interposer.

### Fixed: children that libc starts on the userland rung, 2026-11-11

Record: T-1703, T-1710; issue 235.
Deployed: version 0.2.0-beta.3.

- On the userland loader family, a child that the payload starts through
  `system()`, `popen()`, `fexecve()` or `execveat()` with flags enters
  through the image's loader, as an `execve` child does. Before, libc
  started these past the interposer, and the kernel read the loader of
  the host.
- A child ELF of another machine fails its exec with `ENOEXEC` and a
  line that names it, not a run through the host.
- A Unix socket path in `bind`, `connect`, `sendto` and `sendmsg` takes
  the path table (T-1710, issue 235): a payload's socket is inside the
  rootfs, and `-v HOSTDIR:/run/ctl` puts a host socket at a guest path.
  `getsockname` and the other answers give the guest name.

### Fixed: `wait` of a payload that ends at once, 2026-11-11

Record: T-1720; PodUtils/podrunner#49.
Deployed: version 0.2.0-beta.3.

- `wait` reads the record again, for up to 10 s, where the launcher went
  before the record had the exit code, so it no longer answers "it is Up"
  for a payload that ended.

### Added: `--read-only`, `:ro` volumes and `--tmpfs`, 2026-11-11

Record: T-1655, T-1653; issue 130.
Deployed: version 0.2.0-beta.3.

- On the userland loader family, `run --read-only` and a `-v
  HOST:GUEST:ro` volume make a write under the read-only guest path fail
  with `EROFS`, through the interposer; `/dev` and each writable volume
  stay writable. A raw system call passes the layer. Each other entry
  refuses both by name.
- `run --tmpfs PATH[:OPTIONS]` gives a foreground run an empty writable
  directory at PATH, removed at its end. It is not a mount: `size=` is
  not enforced. A detached run refuses it by name.

### Added: `--timeout`, `--ulimit` and `--security-opt no-new-privileges`, 2026-11-11

Record: T-1655 (partial); PodUtils/podcrates#58.
Deployed: version 0.2.0-beta.3.

- podvm takes the 11 crates at podcrates `v0.17.0`. `run --ulimit
  NAME=SOFT[:HARD]` sets a resource limit of the payload, and
  `--security-opt no-new-privileges` sets `PR_SET_NO_NEW_PRIVS` in it, on
  each rung, for a foreground and a detached run. Each other
  `--security-opt` is refused by name. `--cap-drop ALL` sets the same
  bit; a named capability and `--cap-add` are refused.
- `run --timeout SECONDS` (podman's spelling) kills the payload and its
  tree with SIGKILL at the bound, in a detached run and in a foreground
  run; `.End.Reason` reads `wall`.

### Fixed: the tree of a payload, 2026-11-11

Record: T-1708; issue 233; call C15.
Deployed: version 0.2.0-beta.3.

- `stop`, `kill`, `rm -f` and the end of a foreground `run` end each
  process of the payload's tree, also a grandchild that called `setsid`.
  `inspect` gives the end of the last run (`.End.Reason`, `.End.RunMs`,
  `.End.UserMs`, `.End.SystemMs`, `.End.MaxRssKiB`, `.End.Residual`).
  `rm -f` exits 125 and keeps the record where processes stayed after
  SIGKILL.
- A foreground `run` that got SIGTERM ended with 143 and did not forward
  the signal to the payload. It forwards it now.

### Changed: podcrates v0.16.0, 2026-11-11

Record: T-1708 (partial), T-1715; PodUtils/podcrates#57.
Deployed: version 0.2.0-beta.3.

- podvm takes the 11 crates at `v0.16.0`. The launcher of a detached run
  is the subreaper of the payload's tree and kills each process of the
  tree at the end of the payload, also a grandchild that left the process
  group. The container table and each log are 0600, and each container
  directory is 0700.

### Fixed: the paths that the loader gives, 2026-11-11

Record: T-1705; issue 230.
Deployed: version 0.2.0-beta.3.

- On the userland rung, a path that the loader gives (`dladdr`, the link
  map, `/proc/self/maps`) is a host path under the rootfs, and the
  interposer added the rootfs prefix to it a second time, so an open of it
  failed with `ENOENT` (CoreCLR of the official runner exited 137). A path
  under the host side of the table now passes unchanged.

### Fixed: realpath of the exe on the userland rung, 2026-11-11

Record: T-1704; issue 229.
Deployed: version 0.2.0-beta.3.

- On the userland rung, `realpath` and `canonicalize_file_name` of
  `/proc/self/exe` gave the image's loader, while `readlink` gave the
  program; the .NET apphost of the official runner exited 154. Both now
  give the program. `/proc/PID/exe` with the process's own pid is the same
  link, so GNU `realpath /proc/self/exe` prints the program too.
- A program built with `_FORTIFY_SOURCE=2` called `__realpath_chk`,
  `__readlink_chk`, `__readlinkat_chk` and `__getcwd_chk` past the
  interposer and read host paths. The interposer exports the four, with
  glibc's length check.

### Breaking and fixed: the entered rung, 2026-11-11

Record: T-1712, T-1713, call C14 of `docs/decisions.md`; issues 237 and
238.
Deployed: version 0.2.0-beta.3.

- `--podvm-tier` of `run`, `create` and `exec` takes `machine` only.
  `--podvm-tier=chroot` had no effect: it ran the ladder, and on a
  chroot-denied host it entered the userland rung with exit 0. It is a
  flag error at 125 now, and the message names the value and
  `.EnteredRung`.
- `system info --format '{{.EnteredRung}}'` reads `userland` wherever
  chroot(2) is denied, whatever rung the probe selects. It read `chroot`
  where the probe selected `supervise`, and `namespace` where it selected
  `namespace`, while `run` entered the userland rung.

### Changed, 2026-11-11

Record: T-1716; PodUtils/podcrates#52 and #54.
Deployed: version 0.2.0-beta.3.

- podvm takes the 11 shared crates of `PodUtils/podcrates` at tag
  `v0.15.0`. The volume rules live in `pod-enter` only: a `-v` pair under
  `/proc` or `/.podvm` is refused by name, and a missing `-w` directory
  inside a volume names the volume `HOST:GUEST`, not the image.

## 0.2.0-beta.2 (2026-11-10)

### Breaking: podbox is podvm, 2026-11-10

Record: the owner's ruling of 2026-11-05, a clean break with no
compatibility with the released binaries; commit `98112cb`
(`rename: podbox is podvm`, 2026-11-06); T-1671.
Deployed: version 0.2.0-beta.2 is the first release with the name podvm.
The version moves to 0.2 because the rename breaks each caller.

- The binary is `podvm`, not `podbox`, and there is no `podbox` alias.
  The crates are `podvm` (was `podbox-cli`), `podvm-probe`, `podvm-image`,
  `podvm-extract`, `podvm-complete`, `podvm-enter`, `podvm-gate`,
  `podvm-buildstate`, `podvm-release`, `podvm-supervise`, `podvm-windows`,
  `podvm-ssh`, `podvm-interpose`, and `podvm-measure` (was `podbox-podvm`,
  bins `podvm-parity` and `podvm-workload`). The tool binaries follow:
  `podvm-gate`, `podvm-dev`, `podvm-count`, `podvm-plant`, `podvm-smoke`,
  `podvm-verify` and the rest.
- Every `PODBOX_*` variable is `PODVM_*`, with no fallback to the old
  name. `PODBOX_LANE_INSTANCE` and `PODBOX_WSL_INSTANCE` are both
  `PODVM_WSL_INSTANCE`.
- Every own flag `--podbox-*` is `--podvm-*`: `--podvm-tier`,
  `--podvm-qemu-arg`, `--podvm-mem`, `--podvm-timeout` and the rest.
- The name the binary is invoked under no longer selects the machine tier.
  `podvm` was an argv0 alias that defaulted to it; it is now the binary's
  own name and takes the ladder like every other name. Only
  `--podvm-tier=machine` selects the machine tier. `system install-names`
  installs `docker` and `podman` only.
- State and guest paths move with the name: `/.podbox/` is `/.podvm/`, the
  store, configuration and runtime directories are under `podvm`, not
  `podbox`. Nothing migrates existing state.
- The WSL instance is `podvm` (`wsl-toolkit --instance podvm`,
  `wsl-toolkit-podvm`), not `podbox`.
- The releases up to `v0.1.0-beta.12` keep their `podbox-*` asset names and
  their signing identity, the nightly workflow of `Azathothas/podbox`;
  `podvm-verify` checks each tag through `v0.1.0-beta.13` against that
  identity, and each later tag against the identity of `PodUtils/podvm`.
- The repository is `PodUtils/podvm`. From this release, the assets are
  `podvm-ARCH` and `podvm-ssh-ARCH.tar.gz`, and the nightly workflow of
  `PodUtils/podvm` at the tag signs each one.

### Security: a layer whiteout cannot delete outside the rootfs, 2026-11-10

Record: T-1698; commit `c89b769`.
Deployed: version 0.2.0-beta.2. Each earlier podbox and podvm release has
the defect.

- A whiteout entry of an image layer skipped the path check that every
  other layer entry gets. A whiteout of `..` at the root of a layer
  deleted the contents of the directory above the destination. On a
  kernel with no `openat2`, `../.wh.NAME` reached past the destination.
  Thus a crafted image could delete files outside its rootfs.
- Now podvm refuses such a layer, and the refusal names the layer digest
  and the entry. Do not pull an image that you do not trust with an
  earlier release.

### Fixed, 2026-11-10

Record: the release run 42800267302 and its nightly run 42800606497 on the
tag `v0.2.0-beta.1`; T-1671.
Deployed: version 0.2.0-beta.2.

- The licence inventory of a release lists only the packages that a
  release triple builds. Before, it read the packages of each platform and
  refused `r-efi`, a UEFI crate with no licence file.
- The nightly build installs `qemu-user-binfmt` on Ubuntu 26.04, where
  `qemu-user-static` is only a virtual package.

### Fixed in the code, 2026-11-10

Record: the task and the issue on each line, done on 2026-11-09 and
2026-11-10.
Deployed: version 0.2.0-beta.2.

- A foreground run sometimes exited 0 with no output. podvm closed the
  saved standard output before its copy thread wrote the last bytes. Each
  rung had the defect. T-1700, commit `72523d7`.
- npm can find its modules on the loader family. The interposer now sends
  a raw `syscall` of a path through the path table, because libuv calls
  `statx` that way. The `mkstemp` family keeps the guest name. T-1697,
  T-1699, T-1701.
- A payload larger than 128 MiB runs: podvm reads a bounded head to
  classify it and stages it in bounded parts. Before, podvm refused
  `node:alpine` and other large runtimes. T-1647, #90.
- On the loader family, `-v HOST:GUEST` and `-v HOST:GUEST:rw` work with
  no mount and no `/dev/fuse`. A working directory (`-w`) and a first
  program inside a volume work: each resolves to the file of the volume.
  podvm refuses `:ro`, named volumes and the other rungs by name. T-1653
  (partial), #130.
- On a host that denies `chroot`: a relative path gets the rootfs prefix
  once; a glibc payload is admitted against its full `NEEDED` closure,
  read inside the rootfs; `dlopen` and `dlmopen` load guest objects; and a
  dynamic child program runs through the image's loader. T-1643 to T-1646,
  #86 to #89, #127 to #129, #131.
- A static payload on a host that denies `chroot` refuses at exit 125 and
  names the opt-in `--unsafe-host-paths`. Before, a static Go payload ran
  into a `GOROOT` error. T-1648, #91.
- A `#!` payload runs its interpreter from the image, and `--strict` knows
  the capability gaps of the loader family. T-1649, T-1650, #92, #93.
- `doctor` measures whether a memfd can execute and names
  `vm.memfd_noexec`; the disk-free row reads the soft `RLIMIT_FSIZE`. The
  probe remedy agrees with its stance. `stop -t 0` sends SIGKILL at once.
  The `windows --podvm-mem` row states what the verb does. README and
  SECURITY name `v0.1.0-beta.12` as the last podbox release. T-1688 to
  T-1693, #212 to #217.

### Changed, 2026-11-10

Record: T-1681, T-1677, T-1669, T-1671; the commits `deps: podcrates ...`.
Deployed: version 0.2.0-beta.2.

- podvm takes the 11 shared crates of `PodUtils/podcrates` at tag `v0.12.0`:
  `pod-core`, `pod-hostprobe`, `pod-tls`, `pod-proxy`, `pod-term`,
  `pod-supervise`, `pod-guest`, `pod-enter`, `pod-oci`, `pod-extract` and
  `pod-rootfs`.
  Each HTTP request goes through `pod-proxy`, and `ureq` is gone. With
  `v0.12.0`, the table of a run (`pod-enter`'s `Maps`) resolves each guest
  path, the `#!` parser is `pod-enter`'s, and one `statfs` names a
  `noexec` volume source.
- The workspace builds with Rust 1.99 and edition 2024.
- A release starts from the dispatched workflow `release.yml`, which
  creates the tag and then starts the nightly workflow on it. A pushed tag
  no longer starts a release.

## 0.2.0-beta.1 (2026-11-10)

The tag `v0.2.0-beta.1` is at `16c9ec6` and has no release: its nightly
build failed before the publish (run 42800606497). Version 0.2.0-beta.2
carries each change of that version.

## 0.1.0-beta.13

The tag `v0.1.0-beta.13` has no release: its build failed. Version
0.2.0-beta.2 carries each change below.

- The static build compiles under Rust 1.99: the `open` family's fixed
  third argument is deliberate, so the deny-by-default
  `invalid_runtime_symbol_definitions` lint is allowed at the definition
  and stays a no-op on older toolchains through `unknown_lints`. See
  T-1601.
- The registry fixture's manifest, config, config-digest and layer fields
  are asserted on as bytes, not only as digests, in every test target
  that compiles the fixture. See T-1602.
- The secret-shape regex check is gone. Credential scanning is
  `.github/workflows/secrets.yml`, a pinned trufflehog binary over the
  working tree and the whole history. See T-1603.
- The detached-stdio serialisation mutex no longer turns one test's panic
  into the next test's failure, and a failed detached start now reports
  the exit it took instead of only counting. The one-of-three start
  failure is still open. See T-1604.
- Nine Batch 3 closure paragraphs read as nine paragraphs again, the
  source maps name the four crates the port added and mark each retired
  script as a shim, and the limits page stops describing closed gaps as
  open work. See T-1608, T-1611, T-1612.

## 0.1.0-beta.12

- The release tag names the workspace version. Beta.11's binary
  reports beta.10; the nightly build now refuses a mismatched tag
  before one arch builds. See T-1423.

## 0.1.0-beta.11

Superseded: its binary reports 0.1.0-beta.10. Beta.12 carries the
same source with the version string corrected. See T-1423.

- The no-chroot loader family is complete: anchored maps, loader-symlink
  resolution, payload-visible exe, image and container exec, a detached
  launcher, and script refusal naming the file and its shebang. See
  T-1407 through T-1413.
- Exit codes follow docker: absent reads 127, not invocable reads 126,
  podbox failures stay 125. Strict classifies degradations. Runs are
  quiet by default with a verbose banner. Inspect carries the OCI
  config. See T-1414 through T-1417.
- Store gates refuse on a noexec store, verify and heal blobs, and name
  the host once on transport errors. See T-1418 through T-1420.
- A foreground run leaves a container record. See T-1421.
- SSH carries DNS and write deadlines with reconnect, and the compiled
  shim serves exit 42 to a restricted sshd. See T-1406 and T-1401.
- The ladder runs FUSE, tmpfs, cache, rundir, and memfd rungs with an
  OCI-tarball pack flow. See T-1003 and T-1411.
- The gate plants its kept-session check with a batch-file vehicle on
  Windows. See T-1422.
- Known limits: the static golang binary answers exit 2 naming GOROOT;
  the full exit-code drive needs an engine; KVM and ReactOS stay
  deferred to an operator-present run. See T-1409, T-1414, T-1350,
  and T-1112.

## 0.1.0-beta.10

- The source-state page is generated from manifests, enum declarations, and helper entry points. The gate rejects a changed snapshot.
- Development builds verify input bytes, conditions, and all five output binaries. Interposer objects are built before the CLI.
- The Windows job can export the CLI, helpers, and build record. The KVM proof takes explicit binary and disk inputs.
- Release packaging adds four static SSH helpers, dependency licence texts, a checksum, and a signature for each target.
- Current documents separate source behavior from historical proof. T-1003 and T-1401 are partial because their original acceptance is incomplete.
- The read-only pg-toolkit review supplied the build-freshness method. No runtime code from that tree is linked.
- `podbox windows run` no longer pipes the emulator's output without reading it. A noisy emulator could block and read as a guest timeout. A failed run now names the end of `emulator.log`. See T-1112.
- The KVM proof driver refuses without `--accept-host-risk`, beside another emulator, or below 6144 MiB available. A 2026-09-30 run preceded a Windows host failure. See T-1350.

The full Linux gate and fault plants pass. KVM repetition and publication are
being checked. [PROGRESS](TODO/PROGRESS.md) owns their current state.

## 0.1.0-beta.9

The published assets contain the standalone CLI. The source now has relay,
interactive, remote, and machine SSH paths. The later source commits were
not part of the beta.9 assets. Helpers must be supplied separately for that
release. See the captured changelog for the earlier version sequence.
