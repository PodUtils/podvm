# Outside pass, podvm candidate of the beta after 0.2.0-beta.40

The tenth outside pass (T-2100). A fresh agent of the Housekeeper role
(Sonnet 5.5, high) had the bundle of `scripts/qa-bundle.sh` only, in
`/var/tmp/qa-b41/bundle`. The lead wrote this file from the report of the
agent: the write tool of the agent refused the file.

Blindness: the brief forbade each source path. The agent ran as root in
the same host as the checkout, so the pass is not blind by a tool. The
agent says that it opened no source file. A `ps` listing showed the argv of
other processes (a build directory and the watchers), and a `find` printed
three file names of build output.

## Host and binary

- `sha256sum -c SHA256SUMS`: `podvm: OK`, `podvm.man: OK`, exit 0.
- `podvm --version`: `podvm 0.2.0-beta.40`. `version --verbose`: commit
  `7c2b161e9ba143a0950d7cbacbfac12a200f7d53`, rustc 1.99.1, target
  x86_64-unknown-linux-musl, crt-static yes. The SHA-256 of the binary:
  `b7f62948ca8cb88603337727145efba726bb39718cd7c8fd3fd8f74b9fad2b78`.
- Linux 6.18.44-fc-v880 x86_64, 4 CPUs, 16 GiB. cgroup v1 (hybrid), no
  delegated cgroup2 subtree. No `/dev/kvm` (the machine tier ran under
  TCG). `/dev/net/tun` gives EACCES.
- `probe`: rung `namespace`. `system info`: `.Rung` namespace,
  `.EnteredRung` namespace, 327 parity rows.
- The agent ran most tests as uid 1000 (`setpriv --reuid 1000 --regid 1000
  --clear-groups`, `HOME`, `PODVM_STORE` and `TMPDIR` under
  `/var/tmp/qa-b41/work`). Root only for the incus tier (a private incusd
  6.0.0 of apt), some `doctor` calls and two mistakes.
- Registry: `mirror.gcr.io/library/alpine:3.20`. The ECR mirror gave HTTP
  429.

## Incidents

1. The agent wrote to `work/bin/podman` as root. That path was a link of
   `system install-names` to the bundle binary, so the write cut the
   bundle binary to 2 bytes. The agent restored it from the copy of podvm
   in an incus image that the binary made. Its SHA-256 is the one of
   `SHA256SUMS`. The agent ran again each test of the time of the fault.
2. A run as root made directories of root in the home of uid 1000. One
   fetch of QEMU failed with `Permission denied`. The agent fixed the
   owner and tested again.

## Findings

### OR-B41-01 (P2) `doctor` says the machine tier is refused (exit 1) where it works

- Command: `podvm doctor; echo $?` as uid 1000, after `run
  --podvm-tier=machine` fetched QEMU 11.1.3 into the store tools.
- Observed: `fail qemu-system-x86_64 --version: ... no
  qemu-system-x86_64 on PATH`, `fail qemu-system-x86_64 -accel help`,
  `machine profile: refused`, exit 1. The same output has `ok
  scope.user-net: .../qemu-system-x86_64 has the user netdev`. `podvm
  windows doctor` names the emulator, and `run --podvm-tier=machine` works.
- Expected: a QEMU that podvm fetches by itself is no failure.
- Reproduced: 4 times. The same fault as OR-B40-07.

### OR-B41-02 (P2) The fetched QEMU and the machine records ignore `PODVM_STORE`

- Command: `HOME=<read-only> PODVM_STORE=<writable> podvm run --rm
  --podvm-tier=machine alpine:3.20 echo hi`.
- Observed: `the store copy of QEMU: <HOME>/.local/share/podvm/tools/
  qemu-11.1.part/.downloads: Permission denied`, exit 125. A detached
  Linux guest keeps its serial log under
  `$HOME/.local/share/podvm/windows/machines/NAME/`.
- Expected: `PODVM_STORE` names the store; the copy of QEMU is "the copy
  in the store". `PODVM_TOOLS_DIR` works around it.
- Reproduced: 3 times.

### OR-B41-03 (P2) `system abi` exits 125 for a file that cannot be read; the manual says 2

- Command: `podvm system abi /nonexistent musl; echo $?`.
- Observed: `podvm system abi: /nonexistent: No such file or directory (os
  error 2); nothing was measured`, exit 125.
- Expected: the manual: "2 a file could not be read".
- Reproduced: 4 times.

### OR-B41-04 (P2) `cp` refuses a path through an absolute link in the container, with a false cause

- Command: `podvm run -d --name c alpine:3.20 sleep 300; podvm cp f
  c:/var/run/f`.
- Observed: `the copy is refused: store: .../rootfs/var/run/f resolves to
  /run/f, which is outside the store at .../rootfs`, exit 1. A relative
  link works. No escape.
- Expected: docker resolves an absolute link in the container root.
- Reproduced: 6 times. The same fault as OR-B40-16.

### OR-B41-05 (P2) `--format json` prints the word `json` and exits 0

- Command: `podvm ps --format json` (also `images`, `inspect`).
- Observed: the line `json`, exit 0.
- Expected: docker prints JSON; podvm refuses or says that the text is a
  template.
- Reproduced: 4 times.

### OR-B41-06 (P2) A foreground `run --rm` that SIGPIPE ends keeps a `Created` record

- Command: `podvm run --rm --verbose IMAGE /bin/hello 2>&1 | head -8
  >/dev/null; podvm ps -a`.
- Observed: podvm exits 141. `ps -a` lists the record `Created`. `container
  prune` keeps it, and `rmi` of the image is refused while it stays.
- Expected: `--rm` removes the record.
- Reproduced: 2 of 3 runs.

### OR-B41-07 (P2) Incus tier: a `--rm` run that the OOM killer ends can keep its instance

- Command (root): `podvm run --rm --podvm-tier=incus --memory 16m
  alpine:3.20 sh -c 'x=$(head -c 60000000 /dev/zero | tr "\0" a)'`.
- Observed: exit 137. In 2 of 3 runs: `the instance podvm-XXXX stays
  (DELETE ...: 400 ... Instance is running): incus delete --force
  podvm-XXXX removes it`. `podvm ps -a --podvm-tier=incus` does not list
  it.
- Expected: `--rm` removes the instance.
- Reproduced: 2 of 3.

### OR-B41-08 (P2) `--device HOST:GUEST` with another guest path is visible only in part

- Command: `podvm run --rm --device /dev/null:/dev/mynull alpine:3.20 sh
  -c 'echo hi > /dev/mynull; ls -l /dev/mynull'`.
- Observed: a regular file `/dev/mynull` in the tree. `head -c 4 /dev/z`
  for `/dev/zero:/dev/z` gives ENOENT. A host file reads with `cat` but
  `test -e` says that it is not there.
- Expected: the manual: "map a host device or file into the container".
- Reproduced: 2 times each. The issue #473 class.

### OR-B41-09 (P3) An OCI archive with a repeated layer: the size counts the blob each time

- `load` of an archive that names one layer 2 or 5 times works, and `run`,
  `verify`, `save` and `rmi` work. `images` and `inspect .Size` count the
  blob each time (7,261,065 bytes for 2 names); `system df` counts it once.
- An archive whose manifest names 2 layers and whose config has 1
  `diff_id` loads and runs. Docker refuses it.
- Reproduced: 3 times.

### OR-B41-10 (P3) A name conflict: `create` exits 1, `run` exits 125

The same fault as OR-B40-10.

### OR-B41-11 (P3) No bound of the length of a reference

`tag alpine:3.20 <5000 characters>` exits 0. `pull` of it asks the
registry and prints the name twice.

### OR-B41-12 (P3) `login` prints `Login Succeeded` and checks nothing

The same fault as OR-B40-08.

### OR-B41-13 (P3) A missing argument exits 1 or 125 by verb

Exit 1: `run`, `exec`, `extract`, `pull`, `probe-inside`, `compare`. Exit
125: `notebook`, `machine ssh`, `login` (no `-u`), `update`, `windows run`.

### OR-B41-14 (P3) `system df` counts an extraction that failed

After a refused extraction for the free space, `system df` says `Images:
1 (1 extracted)` and `inspect --format '{{.Extracted}}'` says `false`.

### OR-B41-15 (P3) The machine tier with too little memory gives a vague error

`--podvm-mem 64M` gives "the guest ended before it gave the exit code of
the payload ...", exit 125, and names no minimum.

### OR-B41-16 (P3) `update --memory 1000000000000g` says "takes at least 6m"

The value is too large, not too small. The exit code (125) is correct.

### OR-B41-17 (P3) Words and looks

`ps` gives `Up` with no time; a detached machine gives the command
`(linux guest)`; the note of `exec` on a created container has 14 spaces
in a line; the usage of `exec` says `<image>`; `exec nosuch` names an
image; `exec -w /nonexistent` says "docker creates it"; `load <
/dev/null` names an internal staging file; `cp - c:/x` names the other
direction; `save > /dev/full` prints two lines; `verify` gives
`docker.io/library/dup:1` where `images` gives `dup`; five concurrent
pulls each print `Downloaded newer image`; a store path that is a file
gives `File exists` with no word of the store; `images` on a read-only
store exits 125; `probe` and `doctor` print fixture paths of the builder;
the usage of `stop` offers `-t` with `--podvm-tier incus`, which refuses
it; `rmi -f` does not remove an image of a stopped container.

## What worked as the manual says

The help and the manual in each form; `probe` in each form; `doctor
store_exec`; `system info`; `system df`; `pull` (progress, `--platform`,
the errors of the proxy and the registry, the check of space); `images`,
`tag`, `rmi`, `image` verbs, `inspect`; `save` and `load` (refusals of a
truncated, a damaged and an empty archive, nothing in staging); `import`
and `extract` refusals of `..`, absolute paths, hard links, link escapes
and loops; the lifecycle verbs (`run` exit codes 0, N, 126, 127, 137, 143;
`run -d`, `ps`, `logs`, `exec` with each flag, `stop`, `kill`, `restart`,
`wait`, `rm`, `create`, `start`, `container prune`, `--timeout`); each
forwarded signal of a foreground run; `-v` and `:ro` on the namespace and
userland rungs and on the machine tier; `--report` (no value of the
environment); `--ulimit`; the refusals of the cgroup flags on cgroup v1;
`--read-only`, `--tmpfs`, `--hostname`, `--add-host`, `--entrypoint`,
`-u`, `--env-file`, `--label`, `--podvm-confine`; the incus tier (`update`
of memory, CPUs and processes, the lifecycle verbs, the limits); the
machine tier under TCG (the fetch of QEMU in about 10 s, the lifecycle
verbs, `--podvm-mem`, `--podvm-cpus`, a 9p volume, `--timeout`); five
concurrent pulls, six concurrent `run --rm`, four `run -d --name same`
(one wins), ten concurrent `run -d`, `tag` and `rm -f` (`verify` clean);
a non-UTF-8 argument refused with 125; a 130 KB argument, 50 MB of stdout
and 100 MB of stdin; the check of space of `pull`, `run` and `extract`;
`login` and `logout` files; `system install-names`; `lab`; `profile`.

## Not tested

KVM; foreign architectures, the emulate tier and runsc; the cgroup
limits on the namespace rung (cgroup v1); `machine ssh`, `remote ssh`,
`windows setup`, `fetch` and `run`, `notebook` with Jupyter;
`--podvm-init image`, `--podvm-sealed`, `--podvm-channel`,
`--podvm-engine`, `--network broker`; a glibc image on the loader family;
a raw system call against `:ro` on the userland rung.

## Clean-up

No container record, incus instance, incusd, dnsmasq, bridge, QEMU or
mount of the pass stays. The `incus` package of apt stays installed.
