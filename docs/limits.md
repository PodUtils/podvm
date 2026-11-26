# Current limits

This page records constraints and unproved behavior.
A historical success applies to its captured revision and conditions.
Read [PROGRESS](../TODO/PROGRESS.md) for current verification.

## Host and payload

- A chroot changes paths. It does not provide process or network isolation.
- The namespace path provides mount behavior only.
- A userland entry retains the host root and shared host resources.
- A static x86_64 payload on a host that denies chroot runs on the
  native-static rung (T-1652): Syscall User Dispatch (Linux 5.11) gives
  each of its system calls to podvm, which resolves each path in the
  image with `openat2(RESOLVE_IN_ROOT)`. `/proc`, `/sys` and `/dev` are
  the host's, and `/proc/self/exe` names the guest program. Threads,
  `fork`, `vfork` and `execve` keep the rung; an `execve` of a dynamic
  program maps the image's interpreter beside it (T-1731), and podvm
  drops its own interposer from `LD_PRELOAD` there. A static child of a dynamic program on the
  loader family enters the same rung through podvm (T-1724). A volume
  (`-v HOST:GUEST`) is a host directory descriptor of the handler: a path
  under GUEST resolves in HOST with `RESOLVE_IN_ROOT` there, so `..` stays
  in the volume; `:ro` and `--read-only` are refused on the rung (T-1724). Where the kernel refuses the `prctl`, the payload
  refuses at 125, because its absolute paths would resolve on the host
  (T-1648). Under `--unsafe-host-paths` it runs from a staged memfd with
  the host root. It then reads its own exe as the staged memfd path, so a
  self-locating runtime cannot resolve itself there.
- A userland grandchild that execs a dynamic ELF or a `#!` script enters
  through the image's own loader or interpreter, resolved inside the
  rootfs (T-1646). `fexecve`, `execveat` with flags, `system()` and
  `popen()` take the same route (T-1703). A `memfd` enters through
  `/proc/self/fd/N`, so the started program keeps that descriptor. A
  child ELF of another machine or class (also i386) fails its exec with
  `ENOEXEC` and one line.
- A Unix socket path takes the path table (T-1710), also through `-v`. A
  socket whose host path is longer than 108 bytes binds as
  `/proc/self/fd/N/NAME`; the process that bound it reads its guest name
  back, and another process reads that kernel name.
- A raw call through libc's `syscall()` takes the path table, so libuv's
  `statx` resolves inside the rootfs (T-1697). A raw call with a path that
  the table cannot map, and `io_uring_setup`, answer `ENOSYS`. A program
  that issues the `syscall` instruction itself (a static program, Go,
  liburing) passes the interposer (T-1652).
- `--tmpfs GUEST:size=N` is a tmpfs in memory of N bytes on the
  namespace rung, for `run`, `run -d` and `create` (T-1968). A write past
  N fails with `ENOSPC`, `exec` sees the mount of the run, and `inspect`
  (`.Tmpfs`) and `--report` name it as memory. The chroot rung, the
  userland rung, a rung that `PODVM_MODE` forces, the init of the image
  and the syscall tier (other than its `/tmp`) refuse it by name with
  125. podvm never gives a directory with no limit in its place. A
  `--tmpfs` with no `size=` stays a directory of a foreground run. The
  option `mode=M` (octal, up to 7777) is the mode of that directory. A
  mount with `size=` has the mode 1777 of the kernel, and a `mode=` of it
  that is not 1777 is refused by name (OR-B38-16, T-2070).
- On the namespace rung a volume (`-v HOST:GUEST`) is a bind mount:
  `/proc/mounts` lists it, and `/proc/self/mountinfo` names the host path
  (T-1920). On the userland rung a volume, `--tmpfs` and `--read-only` are
  a path rewrite of the loader family, not a mount (T-1653). A run with
  one of them takes that family also where chroot works (T-1761). A write
  lands in HOST. A static program or a raw `syscall` instruction sees
  the image path, and `/proc/mounts` does not list the volume. Where the
  loader family cannot take the payload (a static program, a foreign
  image), podvm prints why and refuses the volume on the chroot rung; it
  refuses one on the memfd family too. A working directory or a first program in a volume resolves
  through the same table, to the file of the volume.
- `-v HOST:GUEST:overlay` (T-2101) holds on the namespace rung only, also
  with `run -d`, `create` and `start`: HOST, a directory, is the read-only
  lower layer of an overlayfs mount on GUEST, in a user namespace
  (`userxattr`, Linux 5.11). Each write, `chmod`, `unlink` and `rename`
  goes to the upper layer in `<store>/trees/<id>/overlay/<n>/`, beside the
  rootfs, and a whiteout there hides a removed file of HOST; `rm`, and the
  end of `run --rm`, remove it with the tree, and a stop and a `start` keep
  it, as docker keeps the layer of a container. pid 2 of a foreground run
  mounts it; the launcher of a detached run enters a user and a mount
  namespace of its own and mounts it there before the entry of pod-enter,
  whose `Plan::volumes` (podcrates `v0.85.0`) has no overlay. A lower file whose owner the
  user namespace does not map can be read, and a change of it fails
  (`EOVERFLOW` at the copy-up). No other volume may be at, over or under
  GUEST, and GUEST may not be at or under `/tmp`, `/dev`, `/proc` or
  `/sys`, where the rung mounts its own file systems after the volumes.
  HOST and the store must not hold each other. The userland rung (the path
  table cannot hold a copy-up layer), the native-static rung, the syscall
  and emulate tiers, the machine, incus and runsc tiers,
  `--podvm-init=image`, a foreground run of uid 0 and `PODVM_USERNS=0`
  refuse it by name at 125, never with a writable fallback. An `exec` that
  does not join the running unit refuses it too. `system info --format
  '{{json .VolumeOverlay}}'` gives `{"held":B,"rungs":["namespace"]}`.
  Under `--podvm-confine`, a `write=` and an `exec=` rule of GUEST hold on
  the merged view (experiment 485).
- podvm writes no file of its own into a volume (T-2072). A volume over a
  path that podvm completes (`/etc`, `/etc/ssl`, `/dev`) hides the
  fixups there: with `-v DIR:/etc` the payload gets no `hosts`,
  `hostname` or `resolv.conf` of podvm, where docker binds its own over
  the volume, and with `-v DIR:/dev` no device node. `--verbose` names
  each fixup that a volume covers.
- `:ro` and `--read-only` are a read-only layer of the same interposer
  (T-1655): a write through libc under a read-only guest path fails with
  `EROFS`, and `/dev` stays writable. The layer reads a host path of a
  volume or of the rootfs (`PODVM_MAPS` names them) by its guest path, and
  the path that a link reaches, so such a write fails the same way
  (T-2040). A host path of the rootfs resolves its links from the
  rootfs, and `..` stops there (T-2045). A raw `syscall` instruction passes
  the layer. Each entry with no interposer table refuses both by name.
- `--podvm-volume-base DIR` on the loader family (T-1972): at the start,
  each volume under DIR must open under it as a directory with no
  symbolic link (else exit 125 and one line). At each path call of libc
  in such a volume, the interposer opens the volume and the rest of the
  path under DIR again with `openat2(RESOLVE_BENEATH |
  RESOLVE_NO_SYMLINKS)`, and the call takes that descriptor
  (`/proc/self/fd/N`) until it returns. Before that, the walk of T-1725
  resolves each link of the path in guest names, also a link that the
  host makes in the volume after the start: an absolute target names a
  place of the image or of a volume, not of the host (T-2040). So a link
  that the host makes between the walk and the call gives the call
  `ELOOP` and one line, and no link gives a file out of DIR. No descriptor stays open between two calls. An exec
  route, `dlopen`, `glob`, the `ftw` family, `openat2` and `mount` of the
  payload, and a socket address get the check only: the same opens, then
  the host path, so a link that the host makes in the short time between
  the check and the call still moves them. A raw `syscall` instruction
  passes the interposer, as above.
- Interposition covers cooperative library calls, without a security boundary.
- The embedded interposer pair is x86_64 and aarch64, and each of those two binaries embeds the objects of its own machine (T-1721). Other payload architectures need another execution path.
- A foreign architecture needs the host interpreter support required by the selected path.
- A pidfd addresses a direct child. The launcher, and podvm for a foreground
  run, is the child subreaper of the payload's tree and kills each
  descendant at the end (T-1708); a process that reparents above it stays
  out of reach. In the PID namespace of a foreground run of a user with no
  root, podvm is no subreaper: PID 1 of the namespace reaps each orphan at
  once, and the end of the run kills each process of the namespace.
- podvm waits for each process that it starts, except the processes that
  outlive the verb by design: the launcher of `run -d` and `start`, the
  fill of a spare tree, and PID 1 of the namespace of a foreground run
  while that fill runs. The init or the subreaper of the caller reaps
  them; an init that reaps late shows them as zombies until then.
- A denied terminal operation produces a named refusal.
- Host CA completion changes payload trust data and can be disabled.
- `--network broker` (T-1665) holds a payload to an egress broker of the
  run: on the syscall tier the handler refuses each `connect` but the
  broker's, and on the machine tier the guest network is restricted to a
  forward to the broker. Each other rung refuses it. A payload that
  ignores the proxy variables is refused, not served through the broker.
- `--podvm-tier=syscall` (T-1657) runs an x86_64 program of the image
  on the native-static rung (a dynamic one with the image's interpreter,
  T-1731), and the handler also answers `uname` with the
  release `6.1.0-podvm-syscall`, gives `/tmp` as a directory of the run
  with a cap (`--tmpfs /tmp:size=`, 256 MiB by default; a write over it
  fails with `ENOSPC`), and names guest paths in the links of
  `/proc/self`. The entries of other processes in `/proc` are the
  host's. The cap counts the writes of a descriptor opened under `/tmp`;
  a write through `mmap`, `sendfile`, `splice` or `copy_file_range`, and a
  rename into `/tmp`, are not counted. `run -d` and `create` keep the
  tier (T-1935): the `/tmp` of a container is the `/tmp` of its own tree,
  with the same cap, and `rm` removes it; `exec` in such a container
  enters the tier, and `inspect --format '{{.Tier}}'` gives `syscall`.
  The tier sees each raw `syscall` instruction of a dynamic program (Rust's
  `rustix`, Go), which the loader family does not. The handler shares the payload's address space, so
  the tier is a compatibility layer, not a boundary.
  A `-v` of one file is a volume of one file (T-2001): the file opens with
  no `O_DIRECTORY`, and a call on its parent reaches the host file in its
  host directory. A path under it is `ENOTDIR`, and `unlink` or `rename`
  of it is `EBUSY`, as of a bound file; `:ro` refuses a write. The entry
  of the tier lays the strings of `argv` and of the environment in one
  block in rising order, as the kernel does, so that libuv can take
  `argv[0]` to the end of the last argument as the room of the process
  title: `npm` and `npx` of `node:22-slim` run on the tier. A listing of
  `/` does not name a volume whose guest path the image lacks.
- `--podvm-tier=runsc` (T-1729) needs a `runsc` (`PODVM_RUNSC` or
  `PATH`, with its `gvisor-bin/` sidecars) and a host that grants user
  namespaces; otherwise it refuses by name. It serves a foreground `run`
  only, with no terminal, a numeric `--user`, and no `--timeout`,
  `--tmpfs`, `--network broker` or `--podvm-confine`. The network is the
  host's. Where podvm runs as root, `runsc` keeps a mounted `null-netns`
  in `<store>/runsc/state`; unmount it before you remove the store. With
  `--podvm-init=image` the tier runs the init of the image as PID 1 of the
  sandbox (T-2007; [the init of an image](#the-init-of-an-image)).
- Each container has its own rootfs (T-1709): `run` and `create` copy
  the extracted image into `<store>/trees/<id>/`, and completion and the
  payload write only in that copy. A write in one container is not read
  by another container or by a later run. The copy is a `FICLONE` of each
  file where the store's filesystem supports it (XFS, btrfs), else a
  kernel copy of the bytes. The cost on the cloud host (experiment 430):
  alpine 3.20, 7.4 MiB, 12 to 114 ms by copy on ext4 and 5 ms by
  `FICLONE` on XFS; golang 1.24.7-bookworm, 814 MiB, 2.3 to 7.9 s by copy
  (two runs, the second on a loaded host) and 0.8 s by `FICLONE`. A copy uses the disk of the image again, a clone
  does not. `rm` and `--rm` remove the copy. The copy keeps no extended
  attribute, no time of a directory and no hard link inside the image.
  The image's own tree is the source. `exec` with an image reference,
  `cp` with an image reference, `extract`, and the machine and incus
  tiers use the image's own tree.
- The spare tree (T-1781): after a run takes its tree, a detached `podvm
  tree-prefill` fills the next tree of the same image beside the image's
  own tree, at the lowest CPU priority and in the idle I/O class, and the
  next run of the image takes it with one rename. The tree of a `--rm`
  run goes aside with one rename, and the same helper removes it after
  podvm's exit. golang 1.24, 811 MiB, on the cloud host (experiment
  448): a median `run --rm golang true` of 181 ms with the spare, 8.5 s
  without. The helper gets the store and the digest of the image on a pipe,
  so `ps` of another user shows no path of the store (T-2054, item 5). A spare uses the disk of one more tree for each image that
  ran; `rmi` and `image prune` remove it with the extraction.
  `PODVM_SPARE_TREE=0` turns the spare and the deferred removal off. podvm
  never writes the image's own tree after the extraction, so a spare
  stays its copy; a tool that edits that tree in place sets
  `PODVM_SPARE_TREE=0`, or a run takes a spare filled before the edit.
- The host name (T-1908): `--hostname NAME` names a UTS namespace of the
  run where podvm can open one (uid 0, or the user namespace of a
  foreground run, T-1909). Elsewhere the handler of the native-static
  rung and the interposer of the loader family answer `uname` and
  `gethostname` with NAME, and a raw `uname` of a dynamic program and
  `/proc/sys/kernel/hostname` give the host's name. `/etc/hostname`,
  `HOSTNAME` and `/etc/hosts` hold NAME on each entry. An entry with none
  of these, the machine tier and the incus tier refuse the flag with exit
  125.
- The namespace rung of an unprivileged caller (T-1909): a foreground
  `run` opens a user, a PID and a UTS namespace where a trial of the
  entry works, and the payload is uid 0 of that namespace. `run -d` and
  `create` enter the namespace rung through pod-enter's own user
  namespace (T-1912), with a PID namespace of pod-enter (T-1930) and a
  UTS namespace only with `--hostname`. A foreground `run` of uid 0, or
  with `PODVM_USERNS=0`, also takes the PID namespace of pod-enter, with
  `podvm-init` as PID 1 (T-2054); where pod-enter falls back to chroot,
  the run has no PID namespace. No entry of the rung makes a network or an
  IPC namespace.
  `exec` and `cp` join the namespaces of a running payload that differ
  from the caller's (T-1910). `-u UID` other than 0 is faked by the
  interposer view unless `PODVM_USERNS_RANGE=1` maps the subordinate
  range of the caller (T-1911, a foreground run only).
- The confine layer on the userland rung (T-1902): musl reads
  `/etc/resolv.conf`, `/etc/hosts` and `/etc/services` with raw system
  calls on the host path. Where the spec lets the payload read the guest
  path, the layer also gives read on the host file.
- The paths of the confine layer on the userland rung are host paths
  through the table (T-2014): a rule on a guest path that holds the guest
  path of a volume gives the same access to the host path of each volume
  under it, as Landlock holds a mount under the path of a rule on the
  namespace rung (`read=/` reads `/sc` of `-v S:/sc`). The guest paths
  are read lexically, as the interposer matches them: a link of the image
  on the path of the rule is not followed.
- Two limits of Landlock under the confine layer (T-1984, T-1985, call
  C25): Landlock checks the program that `execve` names, not one that the
  dynamic loader runs (`ld.so PROGRAM`), and it governs no change of mode,
  owner or times (`chmod`, `chown`, `utimes`). Where podvm sees the call,
  it answers `EACCES` itself: an exec of the loader (each name and link
  of the file `ld-*.so*`, `ld.so*`, `ld64.so*`) takes the `exec=` rule of
  each argument that the loader can take as its program, and a program
  name with no `/` is denied, as glibc's loader searches such a name as a
  library; a change of mode, owner or times of a file (the target of a
  link, a descriptor) needs a `write=` path. The kernel opens each
  interpreter of a `#!` script for execution, so `exec=` holds for it on
  each rung; on the native-static rung and the syscall tier, where each
  program runs through podvm, the handler reads the `#!` line (up to 4
  levels, as the kernel) and checks each interpreter, and the program of a
  dynamic loader on that line (T-2015). Where each rule holds:

  | Rung or tier | `exec=` through the loader | The interpreter of a `#!` script | Mode, owner, times |
  | --- | --- | --- | --- |
  | namespace and chroot rungs | no (Landlock only) | Landlock; a loader on the `#!` line runs its program | no: give the volume `:ro` |
  | loader family of the userland rung | the interposer; a raw system call passes | the interposer and Landlock | the interposer; a raw system call passes |
  | native-static rung | the handler | the handler | the handler |
  | syscall tier | the handler | the handler | the handler |
  | emulate tier | - | - | - |

  The interposer and the handler are not a boundary (T-1657). The cost,
  only where the run has the layer: a change of mode, owner or times
  opens its path with `O_PATH` and reads one link, about 2.5 microseconds
  for each call on the loader family and 1.9 on the syscall tier (an
  estimate: one run of 200000 calls of `chmod` from perl on the cloud
  host, 2026-11-20); an exec of the loader resolves each argument that
  it can run.

Forced FUSE mode enters through a rung-complete read-only server; its live
entry arm and the permitted-tmpfs entry arm prove automatically where a
host grants the device and the mount, and refuse naming them where it
does not. The embedded-rootfs format is the `save` OCI-layout tarball,
verified per blob on `load`. `load` accepts an OCI archive whose manifest
repeats the descriptor of a layer, and keeps the order of the layers (T-2098).

⛔ **Live FUSE and tmpfs entry are a limit of the measured host, not pending work.**
The prover exists: `experiments/358-ladder-rungs.sh` drives both entry arms
and asserts payload word, bytes, and cleanup wherever the host grants
`/dev/fuse` and `mount(2)`. The Windows base of the parked lane granted
neither, measured at the node, at `mount(2)`, and inside a user namespace.
A cloud session has not run the prover; its result is -. No task clears
the limit, so none owns it. T-1003 in [packaging](../TODO/packaging.md)
stays `done`: everything it claimed to deliver is delivered and its
`Prove` exits 0.

## SSH

The source build produces separate `node`, `operator`, `proxy`, and `shell` executables.
The remote CLI needs compatible helpers beside podvm, or on PATH named `podvm-ssh-node`, `podvm-ssh-operator` and `podvm-ssh-proxy` (T-1821): a plain `node` on PATH is Node.js, and podvm does not run it.
Each release target carries the helper archive (T-1405 in
[podssh](../TODO/podssh.md)).

The relay legs carry a 10 s bound on each connection, pings on each
socket, and a 10 s write bound on the server of each session; a stalled
peer ends its session or its socket loud instead of wedging the node. Node redial pairing is
proven on the loopback fake relay (T-1406). Pairing against a live relay
is `pod-relay`'s, in PodUtils/podcrates. The earlier T-1403 acceptance proves
concurrent sessions under its tested conditions.

The passwd shim compiles from retained source on a glibc-dynamic lane
and serves the restricted host to exit 42; no workspace or release
build links it. T-1401 records that proof (`experiments/382-restricted-sshd.sh`).

The server session does not provide a real pty.
It supplies its own line discipline under ForceCommand.
The server configuration controls exec and subsystem requests.

## Guests

`run --podvm-tier=machine IMAGE CMD` boots a disposable guest from the
image and gives the command's exit code (T-1656). The guest
runs under KVM where `/dev/kvm` opens and under TCG elsewhere;
`PODVM_MACHINE_ACCEL=kvm` asks for KVM and refuses by name without it.
The guest kernel is the pinned `vmlinuz-virt` of Alpine 3.22.6, which
podvm fetches into the store. The guest is x86_64 (linux/amd64), or
arm64 as the paragraph below says; an image of each other architecture
is refused by name with exit 125 before the boot (T-1889). The image
needs `/bin/sh`.
The root of an x86_64 guest with the pinned kernel is a read-only ext4
disk of the image (T-1994). podvm writes the disk once for each image
digest into `store/machine/root-DIGEST-v1.img`, with no tool of the
host. The disk keeps the owners and the modes of the image, hard links,
symbolic links, FIFOs, `security.capability`, `user.*` attributes and
POSIX ACLs. A run waits for the build of another run under a lock. A run
uses a disk only where its trailer, its size and its superblock agree and
where the file is the one of the last check (its inode, size and times),
else podvm hashes the disk; a disk whose SHA-256 is not the one of its
trailer is built again. The root of the guest is an overlay of podvm's
files, the disk and an upper layer on a tmpfs, so a write goes to the
memory of the guest and never to the disk. The initramfs holds only
podvm and its modules, so the memory of the guest does not grow with the
image. `PODVM_MACHINE_ROOT=initramfs` selects the root of earlier
releases, the image in the initramfs; `--podvm-engine` keeps that root
by default, because the overlay2 driver of an engine refuses a directory
on overlayfs. A guest whose root disk does not mount ends the run with
125 and the reason before the command. With no
`--podvm-net none` and no `--podvm-sealed`, the guest has QEMU's
user-mode network: it reaches the host's loopback at
10.0.2.2. `-v HOST:GUEST[:ro]` is a 9p mount of the host directory (T-1723): a write
in the guest lands there with the host user's ownership, and `:ro` refuses a
write with EROFS. The 9p modules come from the pinned modloop. With a kernel
of `PODVM_MACHINE_KERNEL` a volume is a copy, and the run says so. stderr comes back on its own
serial port. stdin reaches the guest where it is a pipe or a file (at
most 64 MiB, read before the boot). With `-t` the command runs on a terminal
of a virtio console with the size of the host terminal, stdin goes there
live, and stdout and stderr share it, as under docker (T-1723); a change of
the size during the run does not reach the guest. `create` with the
machine tier reports the profile and exits 125; `exec NAME` enters a
running guest of `run -d` (T-1750), and `exec --podvm-tier=machine`
with no such guest exits 125.
A `--platform linux/arm64` guest needs `qemu-system-aarch64` on PATH and a
kernel named by `PODVM_MACHINE_KERNEL_ARM64`, runs under TCG on an x86_64
host, and has no `-t`, 9p volume or podvm in the guest (T-1890).
The machine tier needs `qemu-system-x86_64` on the host; `podvm doctor`
checks it. Where the host has none, `run --podvm-tier=machine`, `podvm
windows` and `run windows:NAME` fetch a pinned static QEMU 11.1.3 and its
firmware into the store (T-1875); `podvm machine ssh` and `podvm doctor`
look on PATH only. `--podvm-mem` sizes the virtual machine, as QEMU's `-m` does,
and the guest kernel keeps a part of it: `MemTotal` is 85 % of 256M,
92 % of 512M and 95 % of 1G (T-1849).
`podvm machine ssh` and `podvm windows` boot a guest
([architecture](architecture.md)).

The init of each guest mounts cgroup2 at `/sys/fs/cgroup`, and
`podvm guest-net --load` brings `lo` up in each guest that carries podvm
(T-1780). These flags of the machine tier change the
guest; each other tier refuses them, except as the incus paragraph below
states:

- `--podvm-net none` boots the guest with `-nic none`, so `lo` is its
  only link (T-1751). It refuses `--network broker`.
- `--podvm-channel HOST_SOCKET:127.0.0.1:PORT` gives the guest one
  `virtio-serial` port on a chardev socket in a directory of mode 0700.
  In the guest, a connection to the loopback port reaches a new
  connection to `HOST_SOCKET` on the host (T-1751, T-1778).
- `--podvm-sealed` takes the run as a tar bundle on stdin: a file
  `.podvm-run.json` with `argv`, `env` and `workdir`, and the files of
  the guest (T-1751). The bundle reaches the guest through the one
  `virtio-serial` port only. The guest has no NIC, no 9p volume and no
  stdin file. A command, `-e`, `-w`, `-v`, `-t` and `--network broker`
  are refused. Experiment 445 finds no value of the bundle in a host
  process or a host file, during and after the run.
- `--podvm-engine` loads the modules of a container engine (a bridge,
  `veth`, netfilter, overlayfs) from the pinned modloop at the boot
  (T-1780). Docker 27.5.1 then runs with overlay2 on cgroup2 and a
  bridge network (experiment 447). The modules load only with the flag,
  because they add time to each boot. podvm adds no `iptables`: the
  image of the engine brings it.
- `--storage-opt size=N` (64m to 16T) gives the guest a disposable
  ext4 disk of N bytes (T-1968). podvm writes the empty file system into
  a sparse file in the directory of the run, with no tool of the host,
  and refuses the run where the host has less than N bytes free. The
  guest loads the ext4, virtio disk and overlay modules of the pinned
  modloop, and the disk is the upper layer of the overlay of its root,
  in place of the tmpfs (T-1994). A write to the root or to
  the working directory past the size gets ENOSPC. A `-v` volume is a 9p
  mount of a host directory, and the disk does not bound it. A guest
  that cannot mount its disk ends the run with 125 before the command.
  The run boots and does not use a saved state, and the disk goes at the
  end of the run. An arm64 guest and the kernel of `PODVM_MACHINE_KERNEL`
  refuse the flag.

The guest does not honour `--podvm-confine`, `--user`, `--hostname`, `--add-host`,
`--read-only`, `--tmpfs`, `--ulimit`, `--security-opt`, `--cap-drop` or
`--label`, so the machine tier refuses each by name with exit 125,
before the boot (T-1808).

`run --podvm-tier=emulate IMAGE CMD` runs the command under a static
qemu-user of `PATH` (`qemu-ARCH-static`, else `qemu-ARCH`), and the
emulator itself runs on the native-static rung (T-1651). So each path of
the guest resolves in the image: a path absent from the image is ENOENT
and never a file of the host, an absolute link stays in the image, and an
exec of the guest enters the emulator again. An image of another
architecture runs the same way, with `--platform`. The cost is that of
qemu's TCG (`go version` of the golang image: about 50 s, with the
extraction). `exec` and `create` refuse the tier.

`run --podvm-tier=incus IMAGE CMD` runs the command in a disposable
instance of an incusd that answers on its socket (`PODVM_INCUS_SOCKET`, or
`INCUS_DIR/unix.socket`), and gives its stdout, stderr and exit code
(T-1718, [incus](incus.md)). podvm imports its extracted image with the
owners that the image intended, and is the init of the instance. The
command is not PID 1, except with `--podvm-init=image`. `run -d` and `create` keep the instance, and `ps`,
`start`, `stop`, `rm`, `logs` and `exec` with the tier act on it; incusd
keeps its state, and podvm keeps no record of it (T-1722). `logs -f`
follows the log, and `logs --limit-bytes N` writes its first N bytes;
podvm reads the log file in pieces of 64 KiB from the offset that it
wrote last, so a log of any size is read (T-2046). `inspect` gives the state, and `tier = incus` in
`$store/config` selects the tier with no flag where an incusd answers;
`-v` is a disk device of the instance (T-1722), and `--podvm-mem` and `--podvm-qemu-arg` are refused by
name, and the instance has no network
under route A. `--podvm-net none` masks each NIC of the profile, and
`--podvm-channel HOST_SOCKET:127.0.0.1:PORT` mounts the host socket in the
instance, where podvm's init relays each connection to the loopback port
to that socket; it is not an incus `proxy` device (T-1779, [incus](incus.md)).
A run where no incusd answers exits 125 and names the
socket. The image carries podvm as the init of the instance (T-1722).
`run -d` and `create` with `--podvm-init=image` start the init of the
image (systemd) as PID 1 of the instance, and the command runs by the exec
API after `systemctl is-system-running` gives `running` or `degraded`
(T-1993, [incus](incus.md)). That instance takes 231,072 ids with its own
id map, `/dev/fuse` and an empty machine-id (T-1996), and the import of an
image streams the unified tarball into incusd with no file between
(T-1997). The tarball carries `security.capability` and `user.*` records
of the image (T-1999). The instance does not honour `--podvm-confine`, `--user`, `--hostname`, `--add-host`,
`--read-only`, `--tmpfs`, `--ulimit`, `--security-opt`, `--cap-drop`,
`--label`, `--timeout`, `-t`, `--device` or `--ports`, so the incus tier
refuses each by name with exit 125 (T-1854).

A session can start a KVM guest where `/dev/kvm` exists, with a timeout,
and no emulator may stay after the run ([decisions](decisions.md), 2026-11-09).
Check for usable KVM on this host. The machine tier uses TCG when KVM
is unavailable and no accelerator is required.
`kvm.yml` (T-1682) runs the KVM leg on a hosted runner.

A ValidationOS or DOS guest has no network (a mailbox run with `-nic none`).
A desktop guest (`windows:11`, `10`, `2025`) has a user-mode NIC (T-1865).
`podvm pull windows:NAME` fetches and verifies the pinned medium
([decisions](decisions.md), "Windows media"). The operator can give an own
disk image with `windows fetch --url`.
T-1112 records the TCG, KVM, and DOS proofs separately. An earlier KVM
success does not establish current reliability.
[The earlier text](history/limits-before-2026-11-24.md) says that the Windows guest has no network.

## The init of an image

`run --podvm-init=image IMAGE CMD` boots `/sbin/init` of the image
(systemd) as PID 1 and runs the command after `systemctl
is-system-running` gives `running` or `degraded` (180 s at most,
`PODVM_INIT_BOOT_TIMEOUT` sets 1 to 3600 s; `offline` and `unknown` are
not final; else 125 with the state and the last lines of the console of
the init). The end of the command ends the init: SIGRTMIN+4, 10 s, then SIGKILL (T-2002, call
C26). Experiment 465 measures each line below as uid 1000.

- The namespace rung holds it for a foreground `run` of a caller that is
  not uid 0, where the host gives two things. The first is the
  subordinate id range of the caller (`newuidmap`, `newgidmap`,
  `/etc/subuid`, `/etc/subgid`): a unit with `User=mysql` then runs as
  the uid of mysql, and its files belong to that uid (a subordinate id
  on the host). The second is a delegated cgroup2 subtree: a directory at
  or above the cgroup of podvm that the caller can write. systemd as PID 1
  ends without one ("Failed to create /init.scope control group"). Where
  the caller has none and a user manager of systemd answers on the user
  bus, podvm asks it for a scope with `Delegate=yes` (`busctl` of the
  host, T-2004, experiment 466); the init of the image then gets no
  controller of cgroup2.
- `run -d` and `create` with no tier take the namespace rung only, for a
  caller that is not uid 0 (T-2002, experiment 465, clause 10). The
  supervisor of the init is PID 1 of a PID namespace of its own, under a
  child of the launcher in a user namespace with the range; `run -d`
  returns when the boot is complete, and a boot
  that fails ends it with 125 and the line of the failure, with no record
  left. `exec` joins the namespaces of the init: it sees the units and
  the bus, runs as the user of `-u` with the groups of the image, and
  with `-t` gets a pty of the init. `stop` gives the command SIGTERM;
  when the command ends, the init gets its power off (SIGRTMIN+4, 10 s),
  and the end of the supervisor ends each process of the PID namespace.
  `start` and `restart` boot the init again, from a cgroup that the
  caller can write. Where the host lacks a part, the verb refuses by
  name: no other route keeps an init for a container that stays, except
  `--podvm-tier=incus`. A `-v` at or under `/tmp` lands on the tmpfs
  that the init gets on `/tmp`. An `exec` with no `-t` has the
  descriptors of the caller, and a terminal among them has no name in
  the image. A command of `exec` runs in the cgroup of the run where
  the host lets the caller move a process there, else in the cgroup of
  the caller. A `rm -f`, a `kill` or the bound of `stop` ends the
  supervisor with SIGKILL, and each process of the run with it; the init
  then gets no power off.
- A run with the init on the namespace rung, in the foreground or as a
  container that stays, refuses `--podvm-confine`, `--ulimit`,
  `--security-opt no-new-privileges` and `--device` by name with 125
  (T-2017): the supervisor of the init starts the init and the command
  with no entry of pod-enter. The runsc tier and the machine tier are not
  measured for these flags.
- With `-t`, the command of a run with the init on the namespace rung
  gets a pty of the devpts instance of the init, as the leader of a
  session of its own: `tty` names `/dev/pts/N`, and job control works.
  podvm relays the terminal of the run to that pty, with its window size
  (looked at each 250 ms). In a guest of the machine tier the terminal
  is `/dev/hvc0`, as with podvm's init.
- The init is PID 1 of a PID namespace, not of the host. It has a mount,
  a cgroup, an IPC and a UTS namespace of its own and the network of the
  host. `/sys` is read-only, `/dev` is a tmpfs with the nodes `null`,
  `zero`, `full`, `random`, `urandom` and `tty` of the host and a devpts
  instance, and the capabilities CAP_NET_ADMIN, CAP_SYS_MODULE,
  CAP_SYS_RAWIO and CAP_SYS_TIME are out of the bounding set, as they
  hold nothing in a user namespace.
- Where the host lacks a part (no range, no delegated cgroup, a caller
  that is uid 0) and has gVisor's `runsc` (`PODVM_RUNSC`, else PATH) and
  user namespaces, the run takes the runsc tier and names the part on
  stderr (T-2007). The init is PID 1 of a sandbox: gVisor's kernel gives
  it a cgroup2 tree and each uid, and the code of the image runs on the
  CPU of the host. The owners of the image go onto the files in the
  sandbox. Each write of the root goes to an overlay that ends with the
  run; a volume is a bind mount. `systemd-sysctl.service` is masked, as
  the sandbox takes no sysctl. `uname -r` names gVisor, and a program
  that needs a call that gVisor lacks fails there. The tier takes no
  `-t`, `--timeout` or `--tmpfs`: such a run goes to the guest. podvm
  fetches no runsc (T-2009 waits for a fetch module of podcrates,
  podcrates@T-977). On the cloud host a run with MariaDB and
  PostgreSQL takes 10 s on this tier, as on the namespace rung, and 46 s
  in the guest under TCG.
- Where the host has no runsc or no user namespace (the userland rung),
  the run takes the machine tier
  and names the part on stderr. A run that the saved state below serves
  takes the state first (call C27). Each other run on the root disk of
  the image (T-1994), where `/sbin/init` of the image is systemd, boots
  systemd as PID 1 of the guest itself. podvm writes the unit `podvm-run.service` into the root,
  masks the getty units of the console and `systemd-firstboot.service`,
  and turns the status of the boot off. The unit waits until
  `systemctl is-system-running` gives `running` or `degraded`, then runs
  the command in its cgroup, and the exit code comes back on the serial
  line. One bound from the start of the init (`PODVM_INIT_BOOT_TIMEOUT`,
  180 s by default) ends a run whose unit does not start or whose init
  does not get ready, with one line and exit 125 before the command.
  From a saved state, with an init that is not systemd and with
  `PODVM_MACHINE_ROOT=initramfs`, the init is PID 1 of a PID namespace of the
  guest, with real root and each controller of cgroup2 (T-2002).
  The guest has the limits of "Guests" above: the saved state of T-2008
  keeps its root in the initramfs; a volume is a 9p mount, and the
  network is QEMU's user-mode network. Under TCG on the cloud host a run
  of `true` takes 24 s to 27 s with four vCPUs, and QEMU holds 2.6 GB of
  memory for an image of 584 MB; the namespace rung takes 3 s.
- The machine tier saves the state of a guest that booted the init of an
  image, and each later run of the image starts from that state
  (T-2008). Before it starts the payload or channel relay, the first run
  boots the guest and saves its state in the store
  (`machine/state-DIGEST-KEY.vm`, as large as the memory the guest uses).
  It then gives the guest its command, and
  a later run loads the state and gives its command, its environment,
  its volumes, the time of the host and a seed for the random pool of the
  guest. With `--podvm-channel`, the state has a channel device. Each run
  supplies its own host socket and guest port. The guest sets the run's
  `--pids-limit` before it starts the payload (T-2094). A state is used
  only with the same image digest, podvm, QEMU
  (its file and its version), CPU (under KVM also the kernel of the
  host), guest kernel, memory, vCPUs and arguments of the emulator; each
  other run saves a state of its own, and the store keeps the two that
  were used last for an image. Requests with `-t`, `run -d`,
  `--podvm-engine`, `--podvm-sealed`, `--network broker`, more than four
  volumes, more than 64 KiB of stdin, an arm64 guest or a kernel from
  `PODVM_MACHINE_KERNEL` use a fresh boot, with systemd as PID 1 of the
  guest on the root disk where the image has it (T-1994). Each run of one state has the machine-id,
  the boot id, the monotonic clock and the service processes of the
  save: a secret that a service made at its start (a session key, the
  state of a random generator in its memory) is the same in each run.
  With `--verbose` the console of the boot is not shown for such a run.
  A SIGTERM or another forwarded signal during the save of a state ends the
  run at once with one line and leaves no partial state file (T-2048,
  experiment 480).
  A state that does not load, or whose guest does not take the command,
  gives a boot with one line on stderr, and podvm removes that state.
  After three such failures in a row podvm tries no state of that shape
  for a day (the marker `state-DIGEST-KEY.bad` holds the count and the
  last reason; remove it to try again). podvm checks the length of a
  state file, not its bytes. `PODVM_MACHINE_STATE=0` boots each guest;
  `pipe` saves and loads through a pipe of `cat`, which podvm also takes
  by itself where QEMU has no `file:` transport (before QEMU 8.2).
  `rmi` removes no state and no base archive of an image. Measured under
  TCG on the cloud host with MariaDB and PostgreSQL of Debian 12 under
  systemd (experiment 468): the first output of the command comes 1.3 s to
  1.6 s after the start of a run from a state, and 19 s to 20 s after the
  start of a run that boots; the state is 1,539 MiB, the save takes 2 s,
  and QEMU holds 2.3 GiB in place of 3.7 GiB. KVM is not measured.
- The userland rung itself boots no init: its PID 1 is the PID 1 of the
  host, `setresuid` to another uid gives EPERM (T-0711), and the kernel
  gives a PID namespace only with a user namespace or CAP_SYS_ADMIN. Its
  route is the guest. Experiment 467 measured User-Mode Linux in its
  seccomp mode on a host that denies `ptrace` and each namespace: 15 s
  for a run with systemd as PID 1. T-2005 plans it as a tier; it waits
  for the UML kernel and its launcher in a crate of podcrates
  (podcrates@T-975), as podvm carries no kernel.
- When the command ends, the init gets SIGRTMIN+4 and 10 s for the stop of
  its units. Where nothing of the run can stay (each volume read-only;
  on the namespace rung also `--rm`), it gets SIGKILL at once (T-2010).
  `PODVM_RUNSC_ARGS` gives more flags of `runsc`, each a flag, after the
  flags of the tier; a flag that breaks the tier is the caller's.
  Experiment 468 has the times of each route.
- `run -d`, `create` and `exec` with the init hold on the incus tier only
  (T-1993). On the namespace rung, the command of `--user NAME` gets the
  primary group of the user and each group of `/etc/group` of the image
  that lists the name; `tty` in a command of `-t` names no device.

## Network lab

`podvm lab scenarios` lists 27 scenarios of `pod-netlab` (checked at
podcrates `v0.85.0`); sand-net listed 39 (TODO/net.md T-1847). These 12 are absent,
as each needs a knob of issue 180 (`--vpn`, `--killswitch`, `--vpn-down`,
`--uplink warp`, `--cfrs-pod`) and a WireGuard server, a live uplink or
`podnet pod`, so none is an offline lab (podcrates@T-348): `vpn-full`,
`vpn-split`, `vpn-v6-only`, `vpn-udp-blocked`, `vpn-bypass-policy`,
`vpn-leak`, `vpn-killswitch`, `warp-uplink`, `warp-v6-only`,
`warp-no-udp`, `warp-443-only` and `corp-proxy-pod`. The differences that
stay, each with its reason in the README of `pod-netlab`: `--loss` stops
at 20 % (past it a TCP probe gives no verdict in its time limit);
`--nat64` alone has no DNS64 (`--dns64` is a flag of its own, and
`nat64-dns64` sets both); the knob sets of `symmetric-nat`, `pmtu-ok`,
`pmtu-blackhole`, `cgnat`, `cgnat-dnat`, `nat64-dns64`, `nat64-web`,
`locked-443` and `corp-proxy` come from the names of the scenarios, as no
source of sand-net's values exists (podcrates@T-586). `--icmp allow` is a
word for `pass`.

## Parked Windows lane

The Windows lane on wsl-toolkit is parked since 2026-11-06
([decisions](decisions.md)). These limits apply to that lane only.

Nested KVM in the Windows base can stop the Windows host. On 2026-09-30 a
KVM proof left an emulator that SIGKILL did not remove, and the host then
failed. The KVM proof driver requires `--accept-host-risk`, and refuses
beside another emulator or below 6144 MiB available. With `--unattended`
it starts the proof detached and holds a T-1609 watchdog on its session.
It refuses unless the watchdog answers a probe first.

`/dev/kvm` in the base is `crw-rw---- root kvm`, set by hand because the
node shipped as `crw------- root root` and the guest account is uid 1000.
That mode is per boot: the base's own
`/usr/lib/tmpfiles.d/static-nodes-permissions.conf` replays
`z /dev/kvm 0666 - kvm -` on every boot. After a reboot, reapply the mode
or run the guest as root.

A Windows base probe can report usable without cgroup delegation.
On the recorded host, engine memory and CPU limits are accepted but not enforced.
Read the current `base status --probe` result before a resource-limit claim.
The delegation is absent from the host's WSL configuration, and the
toolkit does not install it. `base ensure --repair` reports the condition
and does not repair it (T-1610). No task owns it, because only a change
to the host clears it.

⚠ **`podvm` does not need cgroup delegation; it uses a delegated subtree where the host gives one.**
`--memory` (`-m`), `--cpus` and `--pids-limit` hold on the incus tier, as
the keys `limits.memory`, `limits.cpu` (or `limits.cpu.allowance`) and
`limits.processes` of the instance ([machine](../TODO/machine.md) T-1897),
and for `run` and `run -d` of the native tiers where the host delegates a
cgroup v2 subtree to the caller: podvm makes one leaf `podvm-ID` for the
run, writes `memory.max`, `cpu.max` and `pids.max`, and names an OOM kill
([cli](../TODO/cli.md) T-1747). Each other case (no delegation, a cgroup
v1 host, `create`, the machine tier) refuses each of them with exit 125
and a named reason, instead of an accepted flag that does nothing.
`podvm update --podvm-tier=incus` changes the three limits of a running
incus container (T-2093; [the incus tier](incus.md#change-live-resource-limits-t-2093));
`update` has no other tier.
`--ulimit nproc=` and `--ulimit as=` hold on each tier, also for
`run -d`. The hazard is the engine underneath podvm, which
accepts those limits silently; [RULES](../TODO/RULES.md) section 8 says an
option accepted by an engine does not prove enforcement.
