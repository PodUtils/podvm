# incus without root

This page answers the study of T-1717 ([the machine tasks](../TODO/machine.md)):
how podvm can use incus without root and without a large loss of speed, and
the form of the incus back end (T-1718). The source of each measured row is
[experiment 433](../experiments/433-incus-rootless.sh) and
[its run](../experiments/results/incus-rootless.txt) on the cloud host of
2026-11-11, or errand-box at `1ad2fc1` (reference issue 251), as each row
says.

"Without root" has two readings (`errand-box:docs/research/rootless.md`):

* **A, no root at run time.** Root prepares the host one time. After that
  no root daemon runs, and no setuid helper runs other than `newuidmap` and
  `newgidmap`.
* **B, no root ever.** A user with no root installs and runs everything.

## The routes

| Route | Preconditions | Cost | Source |
| --- | --- | --- | --- |
| A: incusd as root of the user's own user namespace | root, one time: the package `incus` (6.0.0 here) and `uidmap`; one subordinate id range of the user that is large enough (100000000 ids here); cgroup subtrees that the user owns (on cgroup v1 with no init, again at each boot); a state directory that the user owns. No `/dev/kvm` for system containers. | busybox work 0.6 to 0.9 times podvm's userland rung; a new instance 0.36 to 0.49 s; `exec` 45 ms; start of a stopped instance 0.1 s | experiment 433 |
| A, the same route on errand-box's host | the same, with btrfs | 0.6 to 1.1 times runc on spawn, files, git, cpu and compile; a new instance 0.9 to 3.5 s; no disk quota | `errand-box:docs/research/rootless.md` |
| A, incus virtual machines | `/dev/kvm` for the user | not measured: the cloud host has no `/dev/kvm`, and incusd says "KVM support is missing" | experiment 433, clause 1 log |
| B: podvm's own microVM (`run --podvm-tier=machine`) | QEMU on `PATH`; `/dev/kvm` makes it fast | about 0.6 s for each run under KVM, about 2.9 s under TCG | T-1656, experiment 412 |
| B: UML or QEMU TCG as a full system | none | 18 to 53 times runc on compile work | `errand-box:docs/research/rootless.md` |

Route A gives podvm incus with no root at run time and at the speed of a
shared kernel. Route B is what a host with no root at all gives; podvm has
it as its own machine tier, and incus does not add to it.

## What experiment 433 found

* incusd 6.0.0 runs as root of the user's namespace: uid 0 of the namespace
  is the user (uid 30033), and the ids 1 to 100000000 are the user's range
  from 2000000000. The `lxc` driver is up. The `qemu` driver is not
  (no `/dev/kvm`).
* An alpine 3.21 system container runs `/sbin/init` as PID 1, as an id of
  the user's range on the host (2000999999), never as host root.
* incusd sits in an empty network namespace: it cannot fetch an image, and
  an instance has no network. The client uploads images over the socket.
  errand-box relays the network through a unix socket (its E-23).
* An OCI image that podvm pulled becomes an incus image: a unified tarball
  of podvm's extracted rootfs with the owner 0 and a `metadata.yaml`. It
  imports and launches in 0.4 to 0.6 s. `raw.lxc` `lxc.init.cmd` makes the
  image's command PID 1, as docker does. `lxc.init.cmd` splits its value on
  spaces and takes no shell quoting.
* Three traps, as errand-box recorded them: `--map-auto` takes the first
  id range of the user, so `useradd`'s own range of 65536 ids must not come
  first; incus's own `/etc/subuid` inside the namespace must lie inside the
  namespace's map; and the guests outlive incusd, so a stop kills the whole
  id range.

Median milliseconds of 5 runs of the busybox workloads
(`experiments/lib/wl-busybox.sh`), in the same alpine 3.21:

| Load | rootless incus | podvm, userland rung |
| --- | --- | --- |
| spawn (300 exec) | 100 | 150 |
| files (3000 files) | 450 | 730 |
| cpu (awk loop) | 1310 | 1510 |

podvm ran as the same user and entered its userland rung, where the
interposer is the cost.

## The form of the back end (T-1718)

* **The daemon.** podvm drives an incusd that the host serves, or starts
  the host's packaged `incusd` in the user's namespace, as experiment 433
  does. Both need the root preparation of route A. A static build of
  `incusd` with liblxc (Go with cgo, built with zig) is not tried: it is a
  packaging task of its own, and podvm stays one static binary without it.
* **The verbs.** `run` is `POST /1.0/instances` with the image and
  `raw.lxc` `lxc.init.cmd`, then a start; `exec` is
  `POST /1.0/instances/NAME/exec`; `ps` is `GET /1.0/instances`; `stop`,
  `start` and `rm` are `PUT .../state` and `DELETE`; `logs` is the console
  log; `cp` is the files API; `-v` is a `disk` device (errand-box's x92
  adds one; not measured here).
* **The images.** incus 6.0 pulls no OCI image. podvm pulls it, extracts
  it, and imports a unified tarball (`POST /1.0/images`), as clause 5 does.
  The tarball carries the owners and modes that the image intended, from
  podvm's `.meta.jsonl`.
* **The command.** `lxc.init.cmd` takes no quoting, so it does not name
  the command. It names podvm itself (`/.podvm/podvm guest-hold`), which
  the imported image carries (T-1722); the command runs by the exec API
  with its argv as a list.
* **The network.** No network in the user's namespace without a relay
  (errand-box E-23). The first back end says so and refuses `-p`.
* **The crate.** The client of the incus REST API over a unix socket is
  `pod-incus` of podcrates.

## What podvm has (T-1718)

`podvm run --podvm-tier=incus IMAGE CMD` drives the incusd that answers on
`PODVM_INCUS_SOCKET`, or on `unix.socket` in `INCUS_DIR`
(`crates/podvm/src/incus/`). One run:

1. imports the image one time for each manifest digest, under the alias
   `podvm-HEX16-vN`;
2. creates an instance with `raw.lxc` `lxc.init.cmd = /.podvm/podvm
   guest-hold` (podvm is in the image);
3. starts it, and runs the command with the exec API and its websockets
   (T-2080), with the environment and the working directory of the image
   and the flags. podvm in the image (`podvm guest-exec`) makes the working
   directory where it is missing, starts the command, and names the cause
   of a start that fails (a program that is not there: exit 127; an ELF of
   another architecture: the two architectures, exit 126);
4. gives stdout and stderr apart as they come and the piped stdin to the
   command, sends a SIGHUP, SIGINT or SIGTERM of podvm to the command by
   the control socket (SIGKILL after a second signal or 10 s), exits with
   the exit code of the command (also 126 and 127, which incusd 6.0 ends as
   a failed operation), and deletes the instance.

Two runs of one store import an image once: the second waits for the lock
of the import (`locks/incus-ALIAS.key.lock` of the store), and a race with
another store on the same incusd ends in the image that is there (T-2081).
The first answer of incusd has a bound of 30 s (T-2080).

[Experiment 434](../experiments/434-incus-tier.sh) proves each clause of
this page against the incusd of route A ([its run](../experiments/results/incus-tier.txt)).
The image carries podvm at `/.podvm/podvm` (T-1722), so a run pushes no
binary: a run of `true` with the image in incus takes about 1.1 s with the
release binary (median of 5, 2026-11-12). `--podvm-mem` and
`--podvm-qemu-arg` are refused by name. The verbs of an instance that stays
and the selection by the ladder follow (T-1722).

`-v HOST:GUEST[:ro]` is a `disk` device of the instance (T-1722, clause 11
of experiment 434): a write in the instance lands in the host directory,
and `:ro` refuses a write. Under route A the root of the instance is an id
of the user's range, so it writes only in a directory that the range may
write (mode 0777 in the clause); a directory of mode 0755 of the user is
read-only to it.

## An instance that stays (T-1722)

`run -d` and `create` with `--podvm-tier=incus` make an instance with the
label `user.podvm=1`, podvm's init `guest-hold --run`, and the command in
`/.podvm/cmd`; the init writes the command's output to `/.podvm/log` and
its exit code to `/.podvm/exit`, and the instance stops when the command
ends. `ps`, `start`, `stop`, `rm`, `logs` and `exec` with the tier call the
API on the instance of that name. incusd keeps the state: a record of
podvm's container table belongs to a launcher, and its reconcile calls a
running record with no launcher dead. Clause 12 of experiment 434 runs each
verb against the incusd of route A. `logs -f` follows `/.podvm/log` until
the instance stops, and `logs --limit-bytes N` stops after its first N bytes
(T-2046, clause 25). Each read is a ranged `GET .../files` from the
offset that podvm wrote last, which pod-incus copies in pieces of 64 KiB
to stdout (`Client::get_file_to`), so a log of any size is read. `inspect` gives the state of an instance as JSON
or through `--format` (`.Name`, `.Status`, `.ExitCode`, `.EnteredRung`,
`.CpuSeconds`, `.PeakMemoryBytes`, `.OomKills`).

The usage of a stopped instance (T-2076, PodUtils/podrunner#345): incusd
gives the CPU time and the memory only while an instance runs, and the
cgroup of the instance goes at the stop. So the init, as PID 1, opens the
files of the instance's cgroup before the command starts (the meter of
`pod_hostprobe::usage`: cgroup v2 where its memory controller is there,
else the v1 hierarchies `memory` and `cpuacct`; on Linux 5.8 to 5.18 the
cgroup v2 has no `memory.peak`, and the peak is a dash with its reason).
The init first mounts the cgroup file systems of the instance at
`/.podvm/.meter`. The AppArmor profile of incus allows a cgroup mount only
under `/sys/fs/cgroup`, so on a host with AppArmor (a hosted ubuntu-24.04
runner with incus of apt) that mount fails with EACCES. There the init
takes the cgroup mounts that LXC made in the cgroup namespace of the
instance: a mount of mountinfo that shows the root of its hierarchy, with
the file system type of its mount point checked (T-2086,
PodUtils/podrunner#350; experiment 471 in the job `incus` of `kvm.yml`).
The peak is the high mark of the kernel for the cgroup of the instance
(`memory.peak` of cgroup v2, `memory.max_usage_in_bytes` of cgroup v1),
not a sample, and the OOM kills are the `oom_kill` count of that cgroup,
which also counts a kill of a child of the command. When the command ended, the init sends SIGKILL to each other
process of the instance and reaps them, takes back its own root, reads the
files, and replaces `/.podvm/usage` atomically (it first takes away a
mount, a link or a directory that the command left at the path). Then PID 1
ends. `inspect` of a stopped instance reads that file; a value that the
file does not give is a dash, and `inspect --verbose` says why (an
instance of an older podvm, a forced stop, `--podvm-init=image`).
A caller takes the values from the one `inspect` that also gives the
status `Stopped` (`--format '{{.Status}} {{.CpuSeconds}} ...'`): the list
of incus says STOPPED about 0.5 s before the instance does, and the status
of an instance that stops can go from Stopped back to Running for one read.
Limits: a command with root in the instance can reset the peak of cgroup
v1 (`memory.max_usage_in_bytes`) through the kernel; a command that traces
PID 1 (`CAP_SYS_PTRACE` in the instance) can change any state of the init.

PID 1 and the OOM killer (T-2077): under `--memory`, a command over the
limit can cause two OOM events a few milliseconds apart. The second event
can come before the memory of the first victim is free. The OOM killer of
the instance then takes PID 1, the largest task that stays, and the
instance stops with no exit file and no usage file (12 of 44 runs on the
cloud host, before the init set its score). So, before the command starts,
the init sets its own `oom_score_adj` to -1000, and the command gets back
the value that PID 1 had. The kernel refuses -1000 in an instance of an incusd that has no
`CAP_SYS_RESOURCE` of the host (route A: EACCES). There, PID 1 keeps its
value and the command gets 1000, so that the OOM killer takes each process
of the command before PID 1, and the init writes the reason on the console
of the instance (`incus console --show-log`). Limits: with 1000, the
command reads 1000 in `/proc/self/oom_score_adj`, and a host-wide OOM takes
it before other processes of the host; a command can lower its own value
back to 0, and a process that `exec` starts has the value of incusd. With
such processes, the OOM killer can take PID 1 again.

## The tier by configuration (T-1722)

A line `tier = incus` in `$store/config` (the file of the key `banner`)
selects the incus tier with no flag:

* `run` and `create` take it where the socket of the incusd is there; where
  it is not, they say so and the ladder selects the rung;
* `system info --format '{{.EnteredRung}}'` names `incus`;
* `exec`, `start`, `stop`, `rm`, `logs` and `inspect` take it for a name
  that is an instance of podvm in incus;
* an explicit `--podvm-tier` wins, and another value of `tier` is refused by
  name.

Clause 13 of experiment 434 runs each of them.

## No network and one channel (T-1779)

`run` with `--podvm-tier=incus` takes `--podvm-net none` and
`--podvm-channel HOST_SOCKET:127.0.0.1:PORT`, also with `run -d`
(`crates/podvm/src/incus/net.rs`):

* `--podvm-net none` adds a device of type `none` for each NIC of the
  expanded profile, so the instance has `lo` only.
* `--podvm-channel` adds a `disk` device that mounts `HOST_SOCKET` at
  `/.podvm/channel.sock`, and podvm pushes the port to
  `/.podvm/channel.port` before the start. `guest-hold` brings `lo` up,
  binds `127.0.0.1:PORT`, writes `/.podvm/channel.ready`, and relays each
  connection to the mounted socket. A foreground run waits for the ready
  file before its command starts.
* The caller gives the socket file a mode that lets the root of the
  instance connect, and keeps the file in a directory that guards it.

The channel is not an incus `proxy` device: on a hosted runner of
podrunner, the start of a `proxy` device failed with "Failed to safely
open namespace file descriptor based on pidfd" (PodUtils/podrunner#90).
Clause 15 of experiment 434 holds the tier.

## The init of the image (T-1993)

`run -d --podvm-tier=incus --podvm-init=image IMAGE CMD` (also `create`)
starts the instance with no `lxc.init.cmd`, so `/sbin/init` of the image
(systemd) is PID 1 of the instance. A foreground run takes another route
(call C26 of [the decisions](decisions.md)); the flag with the incus tier
and no `-d` is a flag error, exit 125. The code is
`crates/podvm/src/incus/systemd.rs`. A run does this:

1. podvm asks `systemctl is-system-running --wait` by the exec API each
   half second, for 180 s at most (`PODVM_INIT_BOOT_TIMEOUT` sets 1 to
   3600 s), until it gives `running` or `degraded`. On `degraded`, podvm
   prints the failed units in one line and continues. `offline`,
   `unknown`, `initializing`, `starting`, an answer before PID 1 opens its
   bus ("Failed to connect to bus") and an exec that incusd refuses while
   the instance runs are not final: podvm asks again
   (PodUtils/podrunner#349, T-2083). `maintenance`, `stopping`, another
   word and an instance that stopped end the run with exit 125, the polls
   and the failed units, and podvm removes the instance. Where the bound
   ends, the run ends with exit 125, the error names PID 1 and each poll,
   and the instance stays for a look, with the command that removes it.
2. podvm runs `guest-hold --run --detach` by the exec API, with the
   command, the user and the environment of the run. The command is not
   PID 1; the init of the image is.
3. When the command ends, `guest-hold` sends SIGRTMIN+4 to PID 1, and the
   instance stops. `inspect --format '{{.ExitCode}}'` gives the exit code of
   the command.

The unified tarball reads each file with the owner's bits and keeps the
mode of the image, so an image with files of mode 0000 (`ubi-init`) imports
(`incus/image.rs`). Experiment 434, clause 19, runs `ubi9/ubi-init` and
starts `systemd-tmpfiles-clean.service` in it
([its run](../experiments/results/incus-tier.txt)).

## The hosted snapshot (T-1996, T-1997, T-1999)

An image of a hosted runner is large and holds users, `fuse-overlayfs` and
files with capabilities. Three changes serve it on the incus tier.

* **Ids and devices (T-1996).** With `--podvm-init=image` the instance takes
  `security.idmap.isolated=true` and `security.idmap.size=231072`, the
  device `podvm-fuse` (`/dev/fuse` of the host, type `unix-char`) and an
  empty `/etc/machine-id` before it starts. Two instances then have two
  id maps and two machine-ids (clause 20). Where incusd has fewer free
  ids, its error names the flag and `/etc/subuid`; the user of incusd needs
  a larger range there. A host with no `/dev/fuse` starts the instance with
  no device. A rootless `podman run` and a `fuse-overlayfs` mount of a user
  with the range `165536:65536` are not measured: `ubi-init` does not carry
  them (T-1996 keeps the rest).
* **No third copy (T-1997).** `incus::run::ensure_image` writes the
  unified tarball into a pipe, and the import of incusd reads the pipe in
  chunks: no tarball file lies in the store. Clause 21 measures a tree of
  850,228,783 bytes (`golang:1.24-bookworm`): the first run grows the disk
  use of the store and of incusd by 2.38 times the tree, and no file stays
  in `store/incus`. A tarball file would add the tree once more (about 3.4
  times).
* **At most one full copy beside the upload (T-2085).** An import of an
  image of 85 layers (17.0 GB of blobs, a tree of 56.9 GB) on one btrfs
  that held the store, `/var/lib/incus` and the pool kept three copies of
  the tree at the peak: podvm's tree in the store, incusd's upload in its
  images, and the image volume in the pool (PodUtils/podrunner#347 ask 1).
  So the incus tier extracts the tree only for an import (a run of an
  image that incus has needs no tree); it removes the tree that the import
  extracted after the upload and before the create (also after a failed
  import), where no other process of the store holds the image; and where
  incusd's images have less than three times the tree free, the upload is
  gzip (`incus/gzip.rs`: blocks of 2 MiB, one gzip member each, compressed
  by up to eight threads; the bytes do not depend on the count of
  threads). A tree that was in the store before the import stays. Measured
  on a btrfs of 1000 MB (817 MB free) with the store, `INCUS_DIR` and a
  btrfs pool, `golang:1.24-alpine` (a tree of 292 MB, 80 MB of blobs):
  before, `create` failed with exit 125 at a peak of 838 MiB over the
  blobs; after, it passes at a peak of 394 MiB (the upload is 101 MiB of
  gzip; the tree is gone). On a btrfs of 4 GB the upload stays plain: 565
  MiB after, 852 MiB before. The image volumes of the old path, the plain
  path and the gzip path have the same 16,716 entries, modes, owners,
  bytes and attributes. Experiment 434 clause 31 checks the tree. A native
  run after the import extracts the tree again. With no tree, `run` does
  not refuse an absolute program that the image lacks before the start
  (T-1823): podvm in the instance gives the same exit 127.
* **Capabilities (T-1999).** The tarball carries the extended attributes of
  the image as PAX records (`incus/xattrs.rs`): `security.capability` and
  `user.*` of the tree, and each attribute of the sidecar that
  `pod-extract` writes (podcrates `v0.76.0`). A v3 capability of the tree
  goes as v2, so that the kernel of the instance sets its own root id.
  Clause 22 checks `cap_net_raw+ep` on a file of an alpine layer in the
  root of the instance. The root of the machine tier is not proven (T-1999
  keeps it for `kvm.yml`).

## The room of the pool (T-2078)

On a pool of the driver `dir`, incusd unpacks the image into the root of
each new instance with `tar`. Where the pool fills, incusd 6.0 puts the
whole stderr of `tar` into the error of the operation, one line for each
file that it could not write: 17 MB for 150,000 files and 108 MB for
400,000 files on a pool of 359 MiB (measured on 2026-11-23). An answer over
64 MiB is over the bound of the client, which then gives "the response
body is larger than allowed" (PodUtils/podrunner#346). incusd removes what
it wrote, so after the failure the pool has its space again.

* **Before an import**, `incus::space` compares what the image needs with
  the free space of the pool of the `root` disk of the default profile (its
  `containers` directory, where the root of an instance goes) and of
  incusd's images: the bytes of each file of the tree and one inode for
  each entry for the root of the instance, and the bytes of the tree for a
  plain upload (twice the bytes where both are on one file system). A gzip
  upload (T-2085) has no lower bound before it is written: its estimate is
  the bytes of the layers, for the warning only. Where the import removes
  the store's tree from the file system of the pool before the create, the
  root of the instance takes its place, and its lower bound is 0. Where the
  need is over the free space for a user (`f_bavail`) or over the free
  inodes, the run is refused with one line and exit 125; where it is over
  77 % of the free space (1.3 times the need, experiment 434 clause 21), a
  warning comes before the import. Experiment 434 clause 27 measures the
  refusal on a pool of 8 MiB.
* **After a create or an import** that failed in its transport, one more
  line gives the free space and the need of the pool: each time for a
  refusal by size, else where the image does not fit. Where podvm has no
  tree of the image (incus had it, T-2085), the need is a dash.
* **Where the free space comes from** (`incus::target`). Where each driver
  of the server is `dir` and podvm sees the `containers` directory of the
  pool at `INCUS_DIR/storage-pools/NAME`, podvm reads it with `statfs`.
  Else podvm asks incusd (`GET /1.0/storage-pools/NAME/resources`, podcrates
  `v0.83.0`): incusd measures each driver in its own mount namespace, also
  a pool of route A. incusd gives the free blocks of root (`f_bfree`). The
  line names such a pool "(driver `NAME`, as incusd measures it)". podvm
  does not know the file system under it: the store's tree that an import
  removes counts as room only where the pool can share a file system (a
  `dir`, a dataset of ZFS, a directory on btrfs), not on a loop file of
  incusd, a block device or a volume group.
* **A pool of clones.** Each driver other than `dir` and thick `lvm`
  unpacks the image once into a volume of the image, and makes the root of
  each instance as a clone of it. An import then needs the bytes of the
  tree there for that volume, and a create of an image that incus has needs
  no bytes that podvm can bound. On a btrfs pool of 200 MiB that a file
  fills, incusd gives 74.5 MiB free while a write fails with "No space left
  on device" (experiment 472): the check cannot refuse there, and the line
  after the failure names the pool and the volume with the bytes of the
  tree.
* A run of an image that incus already has is checked before its create
  against the need that the import wrote in the store; an image that an
  older podvm imported has no such record and is checked with the size of
  its image file in incusd, a lower bound. Experiment 472 measures the
  refusal on a pool of the driver `dir` of 8 MiB that podvm does not see.

## What stays open

* incus virtual machines and their speed: they need `/dev/kvm`; the runner
  of `kvm.yml` (T-1682) has it.
* An OCI image with a kernel as an incus virtual machine (T-1998).
* The network of an instance under route A.
* A static `incusd`.
* A push of podvm that each instance does not repeat (a shared file, or a
  smaller release binary).
* A run on the hosted runner of PodUtils/podrunner#346 with a beta that
  has the check of the room (T-2078, remaining (3)).
