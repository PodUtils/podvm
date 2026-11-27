# Outside pass, podvm candidate of 0.2.0-beta.42

The eleventh outside pass (T-2108). A fresh agent of the Housekeeper role
(Sonnet 5.5, high) had a read-only bundle of `scripts/qa-bundle.sh` in
`/var/tmp/qa-b42/bundle` and used a copy of the binary. The lead wrote this
file from the report of the agent: the write tool of the agent refused the
file. The pass is not blind by a tool: the agent ran as root on the host of
the checkout; it says that it read no source, and a `ps` listing showed
other processes with paths of the checkout.

## Host and binary

- `sha256sum -c SHA256SUMS`: OK. `--version` `podvm 0.2.0-beta.41`;
  `version --verbose`: commit `40c59343b400c7a16f7993727cc8bd45f1cc8bb9`;
  SHA-256 of the binary
  `6a1be9dc72e7b0d20a8873498ac5ebb086a044a466ad83d168874640e2033bfb`.
- Linux 6.18.44-fc-v880, cgroup v1 (hybrid), no `/dev/kvm`, 4 CPUs, 16 GiB.
- Most tests as uid 1000 with a private store; root for a private incusd
  6.0.0 and the incus tier. podvm fetched its static QEMU 11.1.3; the
  machine tier ran under TCG.

## Findings

- OR-B42-01 (P1 by the tester): `login` prints "Login Succeeded" for a
  registry that it did not contact (`registry.invalid:5000`), after the line
  "stored ...; the registry checks it at the next pull". Reproduced 2 of 2.
- OR-B42-02 (P1): `--read-only` puts a foreground run of uid 1000 on the
  userland rung (`mode=userland`, host PID table), where the same host gives
  the namespace rung with `-v` or `--tmpfs`; `--read-only --tmpfs /w:size=1m`
  is refused for the userland rung; `run -d --read-only` gives `.Rung
  userland`. A dynamic musl payload that issues `openat` by the `syscall`
  instruction created `/tmp/rr2` in the host's `/tmp` (mode 000, uid 1000).
  The manual's `--read-only` text promises a read-only root on the namespace
  rung; `--strict` refuses the run. Reproduced 3 of 3.
- OR-B42-03 (P2): `--tmpfs P:size=N,mode=M` refuses each mode but 1777;
  the manual does not name the limit.
- OR-B42-04 (P2): a payload argument, `-e`, `--name`, `-w`, `--label` or an
  image reference that is not UTF-8 is refused with 125.
- OR-B42-05 (P2): a copy-up of an `:overlay` lower file whose owner the user
  namespace does not map fails with EOVERFLOW ("Value too large for data
  type"); the manual names no limit of the owners.
- OR-B42-06 (P2): `cp NAME:PATH -`, `cp - NAME:PATH`, `cp` of a directory
  with no `-r`, `cp` of a FIFO and `wait` of a created container exit 125
  where the exit table says 1 for a refusal after the flags.
- OR-B42-07 (P3): `cp` does not keep the modification time.
- OR-B42-08 (P3): words and looks: the warning of a store of mode 755 that
  other users can read the table of containers (the files are 0600 and
  0700); `-q` does not silence the `--user` line; `ps` order, CREATED and
  STATUS; `{{json .}}`, `range` and `table` texts; `stop -t -1`; `-v
  H:/a:ro,rw` accepted; an env-file line `export I=9`, `FOO BAR=2`,
  `1BAD=1`, a trailing space trimmed; `verify` and `images` name an import
  differently; `doctor` fails a missing QEMU that the first run fetches;
  the payload sees `LD_PRELOAD`, `PODVM_ACTIVE_MODE`, `PODVM_GUEST_EXE`,
  `PODVM_MEMO_FD` and `PODVM_MEMO_ID`, and holds descriptor 17 open on its
  own `ownership.memo` with a host path; the mount table of an overlay run
  shows the store path in `upperdir=`.

## What worked as the manual says

Images (pull, images, tag, rmi, save, load, import, verify, system df,
image prune; a damaged archive and a flipped blob byte refused);
`UnpackedBytes` equal to the tester's own count for alpine (7,802,737) and
busybox (4,417,150, each hard link once); the exit codes of `run`;
`--timeout`; `--report` (no value of the environment); `-v` rw, `:ro` and
`:overlay` on the namespace rung; a `:ro` volume against a static probe
that tried remount, umount, a new user namespace, `open_tree` and raw
`openat`, `chmod`, `unlink`, `truncate` and `utimensat` (the host
directory stayed the same); the overlay layer and its removal; `--tmpfs`
with a size (ENOSPC); `--env-file`, `--env-file0`, `-e NAME`; the
lifecycle verbs; concurrency (12 runs, 6 racing names, 4 first pulls);
`cp` containment and links; `-u`, `--init`, `--network host`, the
refusals by name; the machine tier under TCG (`--storage-opt size=`, `:ro`,
`:mapped`, `mapped,ro`, the detached verbs, `--podvm-init=image` with
systemd as PID 1 and a saved state that loads in about 1 s); the incus
tier (run, `-d`, ps, logs, inspect, update, stop, wait, rm, `volume rm`);
`probe`, `doctor store_exec`, `install-names`, `man`, `profile`.

## Not tested

`windows`, `notebook`, `lab run`, `remote ssh`, `machine ssh`,
`--podvm-sealed`, `--podvm-channel`, `--podvm-engine`, `--network broker`,
`pull -a`, registries with auth, `--podvm-volume-base`; KVM, runsc, the
emulate tier and cgroup2 limits (the host lacks them); `-t` on a real pty.

## Clean-up

No QEMU, incusd, dnsmasq or payload of the pass stays; `incusbr0` is gone.
One `[podvm-init] <defunct>` of a launcher that the tester killed with
SIGKILL may stay under the PID 1 of the host. The tester ran two commands
as root with no store variable once, which added a container record to the
default store of root.
