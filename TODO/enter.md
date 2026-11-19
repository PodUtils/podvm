Area: enter.

Record semantics: [task rules](RULES.md#5-entry-closure).


`crates/podvm-enter`. `TOOL.md` section 6.5, milestone M3.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

Five steps, and step 2 is the one that gets skipped.

```
1. resolve the rootfs path; refuse it if it is a symlink
2. open every fd the child needs WHILE THE OUTER ROOT IS STILL CURRENT
3. chroot(rootfs); chdir("/")
4. resolve the program path, now, inside the new root, in this process
5. exec
```

# T-0501: Open every descriptor before the root changes

**Source:** `TOOL.md` section 6.5, `paper_final.md` section 10.5
**Category:** enter
**Milestone:** none
**Priority:** P0
**Effort:** M
**Status:** done

## Problem

A chroot cuts off every path outside the new root. A child with no
explicit stdio opens `/dev/null`, which an extracted rootfs does
not have, and the failure names a missing file rather than a
missing step.

## Premise

Read, and the same trap is measured from the other side by T-0401:
the shell that redirects into a non-existent `/dev/null` creates a
growing file.

## Approach

Open stdio, the log sinks, any host device the config exposes via
`--device`, and a PTY pair if `-t` was asked for and T-0503 says it
is available, **before** `chroot`. A descriptor opened before the
change keeps working after it.
⭐ This is the only route by which a PTY can reach a chrooted
payload at all: allocate the pair outside, pass the descriptors in,
`TIOCSCTTY` in the child.

## Decision

Pass descriptors rather than bind-mounting anything. There is no
attach path on this runtime, so a descriptor is the only thing that
crosses the boundary.

## Prove

`podvm run --rm public.ecr.aws/docker/library/alpine:3.20 sh -c 'echo hi' | grep -qx hi`

## Done

**Done 2026-09-09.** `crates/podvm-enter/src/lib.rs`. Every buffer the child
touches after the `chroot`, the argv, the environment, the working directory and
the candidate program paths, is built **before** the fork, because between
`clone` and `execve` only async-signal-safe work is permitted and nothing there
may allocate.

⚠ **stdio is inherited rather than re-opened, deliberately**, and that is what
makes `podvm run <image> cmd | consumer` give the consumer the payload's bytes.
The `/dev/null` trap this entry names is real and is why podvm never *closes*
stdio: a child with none opens `/dev/null`, which an extracted rootfs does not
have. ⛔ The PTY pair is **not implemented**; it is
[T-0503](#t-0503-probe-devptmx-and-refuse--t-by-name-where-it-is-absent)'s and
M4's, and `-t` is refused by name rather than degraded. `--device`
maps since 2026-09-26, below.

**Partial, 2026-09-25.** Issue 55 reopens the `--device` half: the
Approach names `--device` among the descriptors opened before the
root changes, the Done records it not implemented, and the parity
table carried no row for it, so `run --device` answered `unknown
option` with no reason. Either implement `--device
host[:container[:perms]]` through the device plan, or add a `None`
row naming why and let the parser refuse it by name. The parser keeps
asking the table (T-0801), not a second list. The `None` row half
landed with T-0801 (`--device` refused naming shims, not devices);
the device-map run below stays open. Fix area is
`crates/podvm-enter`'s device plan. Risk if wrong is a docker flag
answered as an omission rather than a decision. Prove is one run
mapping a host device and reading it inside, with the refusal arm
(`experiments/355-parity-curated.sh` clause 1, `--device` at 125
naming status None) beside it.

**Done 2026-09-26.** `--device HOST[:GUEST[:PERMS]]` maps through
the device plan both halves name: `src/device.rs` of `pod-enter` (PodUtils/podcrates)
parses (both paths absolute, perms from r, w and m) and opens the
host paths before the root changes (child descriptors from 41, host
descriptors relocated clear of them), and
`crates/podvm-interpose/src/device.rs` serves the exact guest
spelling as a duplicate where the real open fails ENOENT. The
access mode is checked against the row first (EACCES, before
creation, as the kernel answers it); creation is then the node's
own semantics on devices (a no-op duplicate, EEXIST for
exclusive), and a fall-through everywhere else, including a
creating open the image satisfies: that lands in the image, not
the device, because the serve only ever answers failures, and
the banner says so. `run` and `create` share the parser; the
spec travels in the container record so `start` (the launcher)
and `exec` re-open it on every entry; the machine tier refuses
naming the chroot tier; `m` parses and grants nothing, said on
the banner. `openat2` is an honest gap: it carries no serve hook,
as the procfs emulation before it.
Prove: `experiments/363-device-map.sh` exits 0
(`experiments/results/device-map.txt`): 11 clauses green on the
lane (`rust:1.98.1-bookworm` job containers, host kernel
7.2.0-WSL2-STABLE, lane-built musl release binary with both
interposer objects) - a host file and `/dev/zero` read back
byte-identical, the opener matrix (six numeric flag shapes:
EACCES on the three ungranted, descriptors on the three
granted), served creation and EEXIST through a missing parent,
the image-shadow boundary pinned by read-back, stat honestly
ENOENT, a missing host path at 125, the machine tier at 125,
the musl payload served with the static opener honestly ENOENT,
the create/start/logs round trip through the launcher, and
`exec` re-serving the record's mapping with no flag of its own
(the spec is configuration, read from the record past T-0505's
refusal of the original's environment).
`experiments/355-parity-curated.sh` exits 0 beside it with
`--device` off the refused list (40) and malformed shapes
refused in its clause 5. Unit-pinned: enter parse and round
trip (8 tests), interpose find and answer (6 tests), the CLI
spellings (1 test); lane suites green (cli 151, enter 85,
interpose 44, 0 failed).

# T-0502: Resolve the program inside the new root, in the process that changed it

**Source:** `TOOL.md` section 6.5; `paper_final.md` section 5.5
**Category:** enter
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

A prior implementation resolved the program in a child whose
environment had been replaced. The child recomputed its store path
from an empty environment, got a **relative** path, created a fresh
empty rootfs under its working directory, chrooted into **that**,
and reported `stat /bin/sh: no such file or directory` from a
rootfs where that path exists.

## Premise

Read, in the account of a shipped bug. The general shape is in the
corpus too: `references/89luca89__lilipod` is the Go implementation
the account is about, and it is refused as a seed on language
grounds and on licence grounds ([reference-map.md](reference-map.md)).

## Approach

Resolve the program path **after** `chroot` and `chdir("/")`, in
the process that did them, never in the parent and never at command
construction time. Do not strip the environment the child needs to
find the store.
⚠ A relative store path is the specific hazard. Compute every store
path from an absolute root captured before any environment change,
and assert it is absolute.

## Decision

Resolve in the child rather than pass a pre-resolved absolute path
from the parent. A parent-resolved path names the outer tree, and
the outer tree is gone.

## Prove

`env -i "$(command -v podvm)" run --rm public.ecr.aws/docker/library/alpine:3.20 /bin/sh -c 'echo ok' | grep -qx ok`

## Done

**Done 2026-09-09.** `podvm_enter::run` resolves the program **after**
`fchdir`, `chroot(".")` and `chdir("/")`, in the process that did them.
`Plan::program_candidates` builds the list before the fork and the child tries
each with `execve`; nothing is resolved in the parent.

⭐ Driven: `podvm run <image> sh -c 'echo onpath'` finds `sh` along the image's
own `PATH` **inside the new root**, and `experiments/300-run.sh` clause 4 asserts
it. A parent-resolved path would have named the outer tree, which is gone.

⚠ The relative-store hazard this entry names is closed from the other side too:
`Store::default_root` is computed before any environment change, and clause 7 of
`300-run.sh` runs with `PODVM_STORE` set to a path inside `/workspace` and
enters a rootfs under it.

# T-0503: Probe `/dev/ptmx`, and refuse `-t` by name where it is absent

**Source:** `TOOL.md` section 0, section 6.5, section 6.8; `paper_final.md` section 10.5
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`-t` either works or it does not, and the corpus disagrees with
itself about which. A degraded PTY that cannot open a terminal is
worse than a refusal, because the payload waits.

## Premise

⚠ **Unresolved, and stated as unresolved.** The target's mount
table shows exactly six device nodes bind-mounted into `/dev`
(`full`, `null`, `random`, `tty`, `urandom`, `zero`), no `ptmx` and
no `devpts`, and `mknod` cannot create one. An earlier account of
the same runtime asserts the host `/dev/ptmx` works and published no
capture. A mount table does not list plain files, so neither source
settles it. One `stat("/dev/ptmx")` on the target would, and nobody
has run one.

## Approach

`stat("/dev/ptmx")` in the **outer** environment, before the chroot,
as part of T-0101's probe set. If present, allocate the pair
outside and pass the descriptors in per T-0501. If absent, `-t` is a
**named-reason refusal**: `-t requires /dev/ptmx in the outer
environment; it is absent`.
⚠ A payload that reopens `/dev/pts/N` by name is unsupported either
way, and the banner says so rather than letting it fail deeper.

## Decision

Refuse rather than degrade. `-t` is a request for a terminal, and a
terminal that is not there cannot be approximated by a pipe without
changing what every interactive program does.

## Prove

`podvm probe --json | jq -e 'has("ptmx")' && { podvm run --rm -t public.ecr.aws/docker/library/alpine:3.20 true || podvm run --rm -t public.ecr.aws/docker/library/alpine:3.20 true 2>&1 | grep -q 'ptmx'; }`

## Done

**Done 2026-09-22.** Both verbs ask one shared predicate
(`podvm-probe/src/probes.rs` `ptmx_usable`): only the OPEN row with `Ok`
promises a pty. `run` and `exec` refuse by name otherwise, through two
call sites that shared one had three copies of the check. A five-case
unit test pins the predicate: denied, skipped, absent, and
stat-Ok-beside-open-denied all refuse. Driven on host podman 6.1.2 with
the shipped binary: the probe names ptmx present and usable, and
`run --rm -t` exits 0 with no refusal text. The refusal arm cannot fire
on this machine, which is the environmental skip 250 records rather
than a gap in the wiring. The full lane check is green.

**Partial, 2026-09-08.** The probe half is implemented and measured; the refusal
half needs `run`, which is M3.

**What holds now.** Two rows in T-0101's set, in the outer environment, before
any chroot: `stat(/dev/ptmx)` and `open(/dev/ptmx, O_RDWR)`. The first half of
the `Prove` runs and exits 0. Readings taken on 2026-09-08:

| where | `.ptmx` |
| --- | --- |
| this host, unconfined | `present: true, usable: true`, chardev 5:2 mode 666 |
| inside `experiments/20-enter-target.sh` | `present: false, usable: false`, `ENOENT` both |

⭐ **Existence is not function, so there are two rows and not one.** A device
node that stats and will not open is exactly the degraded `-t` this entry
refuses, and `.ptmx.usable` is the OPEN. ⚠ The open passes `O_NOCTTY`: without
it, opening a terminal can make it the prober's controlling terminal, which is
a mutation of the process doing the measuring.

⛔ **THE RECONSTRUCTION DOES NOT SETTLE THE PREMISE, AND MUST NOT BE READ AS
HAVING DONE SO.** It reproduces the target's `/dev` from the same mount table
this entry says cannot answer the question, so its `ENOENT` is that mount table
repeated back, not an independent measurement. ⭐ What HAS changed is that the
question is now one command on the machine that matters: `podvm probe --json |
jq .ptmx` on the target, by anyone who can reach it. Until somebody runs it
there, the premise stays unresolved and is stated as unresolved.

**Partial, 2026-09-25.** Issue 37 reopens the ordering half: where
both chroot and ptmx are denied, `run --rm -t` exits 125 at the
T-1317 gate (`run.rs` `prepare`) naming `chroot(2)` denied and never
reaches the ptmx refusal (`run.rs` below it), so the T-0503 arm is
unreachable on exactly the constrained hosts it serves. Check the
flag-specific refusal before the generic chroot gate where the flag
alone decides (ptmx before chroot for `-t`), or append the masked
reasons to the chroot message. Exit stays 125, one line per the
honesty rules. Fix area is `run.rs` `prepare` order, the `lifecycle`
gate against the `ptmx_usable` predicate. Risk if wrong is a second
ordering mask beside the first. Prove is `run --rm -t` on a fixture
denying both, naming ptmx, beside the chroot-only arm naming
chroot.

**Done, 2026-09-25.** The flag-specific refusal moved ahead of the
generic gate: `run.rs` `prepare` asks `probes::ptmx_usable` first
where `-t` was given, naming ptmx at 125 whatever else holds, and
prints the chroot sentence beside it where chroot is denied too, so
neither reason masks the other. `create` keeps the strict chroot
gate; foreground `run` takes the two-tier `ensure_entry_possible`
gate; the late `-t` check in `prepare` is gone and `exec` keeps its
own. Prove is `experiments/356-no-chroot-rung.sh` (exit 0,
`experiments/results/no-chroot-rung.txt`), whose fixture is a mount
namespace hiding `/dev/pts` plus `experiments/deny-chroot.c`
denying `chroot(2)` with EPERM:

```
$ fixture podvm run --rm -t ALPINE true; echo $?
podvm run: -t was asked for and /dev/ptmx is not usable on this machine [...]
podvm run: chroot(2) is denied on this machine as well [...]
125
```

The chroot-only arm is the same drive's `fx-refuse` (125 naming
`chroot(2) is denied`, rootfs byte-identical after), and the capable
arm is `sane-tty` (`run --rm -t` exits 0, `system info`
`.EnteredRung` reads `chroot`). The guard is the order itself with
the late check deleted: no `-t` path reaches a gate that can mask
it.

# T-0504: Refuse a rootfs path that is a symlink

**Source:** `TOOL.md` section 6.5 step 1
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`chroot` follows a symlink. A rootfs path that is one puts the
payload somewhere the runtime did not choose, and every containment
claim after that is about a different directory.

## Premise

Read.

## Approach

`lstat` the resolved rootfs path and refuse if it is a symlink,
naming the path and its target. Then hold a directory fd on it and
use `fchdir` plus `chroot(".")`, so nothing between the check and
the change can swap the path.

## Decision

Refuse rather than resolve. Resolving accepts a rootfs the store
does not own, and the store is where the ownership sidecar and the
lock (T-0204) live.

## Prove

`ln -sfn "$(podvm inspect --format '{{.RootfsPath}}' public.ecr.aws/docker/library/alpine:3.20)" /tmp/rootlink && ! podvm run --rm --rootfs /tmp/rootlink public.ecr.aws/docker/library/alpine:3.20 true`

## Done

**Done 2026-09-09.** `podvm_enter::RootDir::open` `lstat`s with
`AT_SYMLINK_NOFOLLOW`, refuses a symlink **naming its target**, refuses anything
that is not a directory, and then holds the directory open with
`O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC`. The entry uses `fchdir` on that descriptor
and `chroot(".")`, so nothing between the check and the change can swap the path.

⚠ Two tests, and the second is what makes the first mean something: a symlinked
rootfs is refused, **and the directory it points at is accepted**, so the check
is about the symlink rather than about the path being rejected wholesale.

# T-0505: `exec` is a fresh chroot, and `inspect` says so

**Source:** `TOOL.md` section 4.2, section 6.8; `paper_final.md` section 10.6
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`docker exec` enters the container's namespaces. podvm has none to
enter, so its `exec` re-runs the section 6.5 sequence against the same
rootfs. It shares the filesystem tree with the original process and
nothing else, and a caller that assumes otherwise gets a process
that cannot see the original's `/proc`, its signals or its
environment.

## Premise

Read.

## Approach

Implement `exec` as a fresh entry, and mark it **Degraded** in the
parity table with the difference stated: "a fresh chroot re-entry;
shares only the filesystem". `inspect` carries the true mode per
container so a caller can check rather than assume.
⛔ Do not imply containment. A pidfd addresses one process; it does
not contain descendants and it is not a PID namespace, and
grandchildren that reparent are outside podvm's reach.

## Decision

A fresh chroot rather than refusing `exec`. `exec` is load-bearing
for M4's lifecycle test and for every agent workflow, and the
filesystem-only sharing is enough for the overwhelming majority of
uses provided it is stated.

## Prove

`./experiments/320-cli-contract.sh` clause 4 and `./experiments/300-run.sh` clause 8: exec's stdout is the payload's, the banner says it is a fresh chroot, and `inspect --format '{{.Exec.Shares}}'` reads `filesystem`

## Done

**Done, 2026-09-09.** `crates/podvm/src/exec.rs`, `Exec.Mode` and
`Exec.Shares` on `inspect`, and clause 8 of `experiments/300-run.sh`.

⛔ **The `Prove` above was rewritten, and the original is here because the
rewrite is the finding.** It read

```
podvm run -d --name execprobe alpine:latest sleep 30 && podvm exec execprobe ... && podvm rm -f execprobe
```

and every verb in it except `exec` is M4's: `run -d`, `--name` and `rm` need
container state, which [T-1105](milestones.md) creates. ⛔ The work order puts
this entry BEFORE M4 because M4 needs it, so a `Prove` that needs M4 could never
have run in the order it was written for. The mechanism does not depend on that
half: `exec` re-enters a rootfs, and today a rootfs is named by an image
reference and at M4 by a container name that resolves to the same directory.

⭐ **The degradation is stated in three places and they cannot disagree**,
because all three read one pair of constants in `crates/podvm/src/images.rs`:
the banner on every `exec`, `podvm inspect --format '{{.Exec.Mode}}'` for a
program, and the `Degraded` row for the verb in [T-0801](cli.md)'s parity table.
A unit test asserts the banner contains what `inspect` reports, and clause 8
asserts it on the shipped binary.

⛔ **`exec` never pulls and never extracts.** A verb that creates the thing it
claims to attach to is exactly the lie `TOOL.md` section 4.1 forbids, so an
image the store does not hold, or holds and has never extracted, is a refusal
naming `podvm run`. ⚠ It also has no default command: an image's `Cmd` is what
`run` starts, and re-running it from `exec` is a process nobody asked for.

⛔ **`--format` had to learn a dotted name for this.** `{{.Exec.Shares}}` was
refused as an unsupported traversal, because the walker rejected any `.` inside
a name. A dotted name is now ONE registered name, so an unregistered one is
still refused rather than resolving half of itself and rendering a blank.

# T-0506: A foreign-architecture container, and never a rung measured by the emulator

**Source:** Found on 2026-09-09 by running `podvm probe` under `qemu-aarch64`, while closing [T-0911](deps.md)
**Category:** enter
**Milestone:** none
**Priority:** P0
**Effort:** M
**Status:** done

## Problem

⛔ **A probe run under `qemu-user` measures the emulator, and
printing its rung as the machine's is the exact lie podvm exists
to refuse.** Measured on 2026-09-09,
`experiments/results/multiarch.txt` clause 4: `qemu-aarch64`
answers `EINVAL` to `clone(CLONE_NEWNS)`, `ENOSYS` to `fsmount` and
`move_mount`, and reports `Seccomp: 0` and `NoNewPrivs: 0` however
the host is confined. Every one of those is a statement about QEMU.
⚠ It was harmless while podvm ran only where it was built. The
multiarch work makes it reachable by an ordinary user: `podvm run`
on a `linux/arm64` image on an amd64 host executes its payload,
and any `podvm probe` inside it, under precisely this emulator.

## Premise

⭐ **Measured, in this tree.** The aarch64 binary reports rung
`namespace` under qemu on a host that is also `namespace`, so the
two agree today and the agreement is a coincidence of this host
rather than evidence. The rows that disagree are printed in the
same result file, and they are the ones a caller would act on.
⚠ It is the same shape as `experiments/20-enter-target.sh`'s `/dev`
answering out of the mount table the question doubts
([T-0503](#t-0503-probe-devptmx-and-refuse--t-by-name-where-it-is-absent)),
and it has the same remedy: say which instrument answered.

## Approach

Read the image's platform and the host's, and where they differ:
1. **Say so, once, on stderr**, naming both. ⛔ Never silently.
2. Read `/proc/sys/fs/binfmt_misc` for an interpreter registered
   for the image's architecture, and whether its flags carry `F`.
   ⭐ Measured 2026-09-09, clause 5: an `F` registration holds the
   interpreter open, so a **bare chroot with no qemu inside it**
   runs the foreign binary. Without `F` the interpreter must be
   reachable by path inside the rootfs, which is the copy-in below.
3. Where an interpreter is registered without `F`, copy the static
   interpreter into the rootfs, the operator's ruling of
   2026-09-09, and ⛔ **disclose the write**: podvm put a file in
   the payload's tree, which is a thing the payload can see, and
   the honesty rules do not have an exception for convenience.
4. Where no interpreter is registered at all, **refuse** with exit
   125, naming the host platform, the image platform, whether
   `binfmt_misc` is mounted, and what would fix it. ⛔ Never exec
   into a bare `Exec format error`.
5. ⛔ **Mark the probe answer as the emulator's** wherever podvm
   runs under one, in the evidence block and in `--json`, and
   refuse to write it into `$store/probe.json` under a key that
   does not name the emulator. [T-0111](probe.md)'s cache key is
   about confinement; this adds the interpreter.

## Decision

Report and refuse before falling back. A runtime that guesses which
architecture a payload wanted has the same problem as one that
guesses a rung. ⚠ The copy-in is deliberate and disclosed rather
than refused, because refusing it would make podvm useless on
every machine whose binfmt registration predates `F`, which is most
of them.

## Prove

`podvm run --rm --platform linux/arm64 <image> /bin/true` succeeds where an interpreter is registered and exits 125 naming both platforms where none is, and `podvm probe --json` run inside it carries the interpreter in `measured_by`

## Done

**Done 2026-09-09.** `src/binfmt.rs` of `pod-enter` (PodUtils/podcrates) and the `run` verb,
driven by `experiments/300-run.sh` clause 5 and
`experiments/260-multiarch.sh` clause 5.

| | |
| --- | --- |
| `podvm run --platform linux/arm64 <image> /bin/uname -m`, on an amd64 host | **`aarch64`**, exit 0 |
| what podvm said on stderr | it named the image's platform, the host's, and the interpreter, and that the binfmt `F` flag makes it reachable inside the chroot |
| `--platform linux/riscv64`, nothing registered | exit **125**, naming the ELF machine it looked for, how many registrations are enabled, and that installing `qemu-user-static` is what docker and podman rely on too |

⛔ **podvm reads the registrations and never writes one.** Registering an
interpreter is a machine-wide change and podvm is not the machine's owner.
What it does write, on the operator's ruling of 2026-09-09, is the interpreter
**into the rootfs** where the registration lacks `F`, and that write is
disclosed in the banner naming the exact path: the payload can see that file,
and the honesty rules have no exception for a helpful edit.

⚠ **The `F` flag is read out of the registration rather than assumed.** Without
it the kernel opens the interpreter by path **at exec time**, and that path is
inside the new root; with it the interpreter is opened when the registration is
made and held. Measured, `experiments/results/multiarch.txt` clause 5: an `F`
registration ran an aarch64 binary in a bare chroot containing nothing but that
binary.

⚠ **The `e_machine` is parsed out of the registration's own magic**, at byte 18
of the ELF header, so podvm answers "is there an interpreter for THIS image"
rather than "is there any interpreter at all". A magic too short to reach byte
18 does not select on the architecture, and podvm treats that as no match
rather than as a match.

**Point 5 done, 2026-09-09.** `pod_hostprobe::interp`,
`measured_by` in `report::document`, a line in the banner, and `interpreter` as
the eighth component of the cache key. Driven by
`experiments/260-multiarch.sh` clause 6.

| | |
| --- | --- |
| `measured_by.emulated`, aarch64 podvm under qemu on this amd64 host | **`true`** |
| `measured_by.interpreter` and `cache_key.interpreter` | both `/usr/bin/qemu-aarch64-static` |
| what the banner says, beside the rung | `⛔ MEASURED BY /usr/bin/qemu-aarch64-static, NOT BY THIS MACHINE` |
| that answer offered to a native podvm sharing the store | refused: `the instrument changed` |

⭐ **What can be detected was measured rather than assumed**, by running the
arm64 rootfs's own tools under `qemu-aarch64-static`:

| asked | under the emulator | usable |
| --- | --- | --- |
| `/proc/cpuinfo` | `model name: ARMv8 Processor rev 0 (v8l)` | ⛔ no, emulated |
| `readlink /proc/self/exe` | the guest binary's own path | ⛔ no, emulated |
| `/proc/self/maps` | guest mappings only; qemu's own are filtered out | ⛔ no |
| `ls /proc/sys/fs/binfmt_misc` | the **host's** registrations | ⭐ yes |

⛔ **So the claim is conditional and says so.** podvm cannot prove it is
running natively; what it establishes is that this machine routes binaries of
podvm's **own** ELF machine through an interpreter. The other state is
`NoEvidence`, it carries the list of what was checked, and it is never printed
as "measured natively": `TODO/probe.md` T-0109 rule 1 forbids collapsing "no
evidence" into an answer.

⚠ **Two routes in, and only one leaves a `binfmt_misc` trace.** An explicit
`qemu-aarch64-static ./podvm` involves no registration at all, so the
environment is checked for the variables qemu-user reads. That is weaker
evidence and is reported as its own sentence rather than merged with the first.

⛔ **The reader moved to `podvm-probe` rather than being copied.**
`podvm-enter::binfmt` asks "can this machine execute a FOREIGN image"; this
asks the mirror question about podvm's OWN architecture. One parser answers
both, and `podvm-enter` re-exports it.

⚠ **A defect this found in `experiments/260-multiarch.sh`**: clause 4's reading
depended on whatever binfmt state the machine happened to be in, because
`podvm probe` re-execs itself once per probe and an explicitly-invoked aarch64
podvm cannot start its own children without a registration. The rung read
`namespace` on a machine that had one and `unsupported` on a machine that did
not, and the committed reading recorded only the first. The registration is now
made once, before clause 4, and the trap removes it.

# T-1317: A chroot-denied host gets an up-front refusal naming chroot, not a 125 after the work

**Source:** issue 12, client beta testing 2026-09-22 (every rung dies at
  `chroot(".")` EPERM while probe selects `interpose`);
  `crates/podvm-enter/src/lib.rs` (fchdir/chroot/chdir entry
  sequence, T-0502)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

On a host that denies `chroot(2)` itself, probe selects
`interpose` and then every entry path (`run`, forced memfd,
`start`) dies at `chroot(".")` EPERM, exit 125, after fixups,
placement and the banner have all run. The ladder rungs
change where payload bytes come from, not whether a chroot
happens, so nothing reaches the payload on exactly the class
of host the README names first.

## Premise

Measured by the reporter (uid 0, empty capability bounding
set, seccomp 2): probe prints `chroot=EPERM` beside "would
permit interpose", and the run fails post-banner. The entry
sequence design (chroot always happens) confirms the
mechanism on this tree.

## Approach

Refuse `run`/`start`/`create` up front where the entered
rung's chroot is denied, naming `chroot(2)` before any
fixup mutates the rootfs, reusing the probe's own chroot
leg as the gate. Out of scope: a no-chroot rung (rejected
below), rewriting the ladder, touching the threat model.

## Decision

Up-front refusal, for three reasons. A no-chroot rung would
be strictly weaker than today's chroot rung while costing a
subsystem, and the banner machinery would have to describe a
mode weaker than anything it names today. Documenting
chroot-denied hosts as unsupported contradicts the README's
first paragraph, which names these hosts as the audience.
The refused-rung table already has the legs to gate this.

## Prove

`podvm run` on a chroot-denied host (or a fixture that
denies chroot) exits 125 naming `chroot(2)` denied with no
rootfs mutation, and `podvm probe` maps its chroot leg to
the same refusal; on a chroot-capable host every rung runs
as before. Close issue 12 with a comment showing both
outputs and the probe-gated refusal as the guard that stops
recurrence.

## Done

**Done 2026-09-23.** Up-front refusal, as decided. One helper,
`lifecycle::ensure_chroot_usable` (`crates/podvm/src/lifecycle.rs`),
beside the T-1112 gate: it admits where `probes::chroot_usable` finds an
`Ok` chroot row and otherwise refuses at 125 naming `chroot(2)` denied,
the probe's chroot leg, and that no fixup has run. Three call sites.
`run`/`run -d`/`create` share `run::prepare`, where the probe moved ahead
of the fetch, the lock, the extraction and every fixup (its findings are
reused for the banner, so the probe still runs once); `start` probes
after the record lookup and refuses before the launcher starts. The
machine tier returns before the gate and never chroots, so it is
unaffected. `exec` is deliberately ungated: it re-enters a running
container, and with `start` gated no running container exists on a
chroot-denied host; the helper is one call away if that changes. The
refusal precedes the banner on purpose, like the other pre-banner gates:
the banner names fixups, and none have run.

The gate test was watched fail first: with `chroot_usable` stubbed
always-true, `only_an_ok_chroot_promises_entry` fails (the `Denied` and
`Skip` rows promise entry); with the real predicate it passes. The
helper's own test (`lifecycle::a_denied_chroot_is_refused_before_entry`:
`Ok` admits, `Denied`/`Skip`/absent refuse at 125) passes by name in the
lane (`rust:1.98.1-bookworm` through host podman 6.1.2), with full suites
green beside it and clippy `-D warnings` clean.

Driven on the shipped binary in the same lane with a seccomp fixture (a
C launcher denying `chroot(2)` with EPERM, then execing podvm),
`public.ecr.aws/docker/library/alpine:3.20`:

```
$ podvm probe            # unfiltered:  chroot=ok
$ fixture podvm probe    # filtered:    chroot=EPERM
$ fixture podvm run --rm IMG echo hi; echo $?
podvm run: chroot(2) is denied on this machine, so the chroot tier
cannot be entered. [...]
125
```

`create` and `start` refuse the same way at 125 under the fixture. On a
fresh store the refused `run` fetches nothing (no blobs); on the main
store all 93 rootfs files verify unchanged after all three refusals
(`sha256sum -c`), and unfiltered `run` still runs the payload. (A first
pass at that check snapshotted before the capable run and blamed one
file; the file was `etc/hosts`, rewritten by the capable run's own T-0403
fixup, not by any refusal. The rerun snapshots after every unfiltered
step, so the check isolates the refusals.) The guard that stops
recurrence is the probe-gated refusal: no entry path reaches a fixup or
an entry sequence without an `Ok` chroot row.

**Partial, 2026-09-25.** Issue 30 reopens this entry: the up-front
refusal is the correct last resort and the wrong whole answer. On a
chroot-denied host no entry path runs, including forced memfd. Two
corrections ride here. The Done names `exec` among the gated paths;
`exec` is deliberately ungated (no running container exists where
`start` is gated, and the helper is one call away if that changes).
Implement one no-chroot entry rung: userland-exec with the interposer
and no chroot for dynamic payloads, memfd exec with no chroot for
static payloads, or an automatic machine-tier bridge where the T-1301
legs hold. The banner names the entered rung and what it does not
isolate; the up-front gate stays as the last resort with the tried
rungs named. Acceptance names the target shape: chroot `EPERM` with
kvm, tun and ptmx absent, one of the two families runs with the
banner naming the rung, and refusal stays only where neither family
can run with both refusals named. Fix area is
`crates/podvm-enter` entry sequence, the `lifecycle` gate, and the
T-1003 ladder. Risk if wrong is weaker isolation stated as equal.
Prove is `run --rm alpine echo hi` at exit 0 with the banner naming
the rung on the chroot-denying fixture, beside the current refusal
test as the strict arm, with no rootfs mutation on the refusal path.

**Done, 2026-09-25.** Both no-chroot families run, decided in
`lifecycle::decide_entry` before any fixup mutates the rootfs:
dynamic payloads through the image's own loader with the image's
library directories and the interposer preloaded by host path
(`podvm-enter/src/userland.rs` `loader_plan`, `host_env`), static
payloads from a staged memfd exactly as the ladder stages it
(`Family::Memfd`). `Rung::Userland` orders after chroot and never
claims isolation: the banner, `PODVM_ACTIVE_MODE=userland` and
`system info` `.EnteredRung` read `userland` on every userland run.
Refusals name every tried rung with its missing leg at 125: scripts,
foreign images, unresolvable payloads, `--strict` against the weaker
rung, `run -d` and `create` staying chroot-gated, forced non-memfd
rungs, and forced memfd over a dynamic payload naming the unforced
loader family. Three candidates were tested for the alpine 127 seen
mid-drive (unresolvable payload; loader argv shape; applet dispatch);
the lane refuted the first two and confirmed the third: alpine's
`/bin/sh` points at the absolute `/bin/busybox`, which dangles on
the host side, so the resolved file opens and `loader_argv_for`
passes the invoked name as the applet (`busybox sh ...`), the same
selection a chrooted kernel would make. Prove is
`experiments/356-no-chroot-rung.sh` (exit 0,
`experiments/results/no-chroot-rung.txt`) on the both-denied
fixture (pinned alpine `d9e853e8`, pinned debian `833d7afe`,
locally built static hello):

```
$ fixture podvm run --rm DEBIAN sh -c 'echo $PODVM_ACTIVE_MODE'; echo $?
entering without chroot on the loader family: [...] runs [...]/usr/bin/sh [...]
userland
0
$ fixture podvm run --rm ALPINE sh -c 'echo $PODVM_ACTIVE_MODE'; echo $?
entering without chroot on the loader family: [...] runs [...]/bin/busybox
[...] the invoked name "sh" rides as the applet [...]
userland
0
$ fixture podvm run --rm static-hello:1 /hello; echo $?
entering without chroot on the memfd family: [...]
static-hi
0
$ fixture podvm run --rm --strict ALPINE true; echo $?
podvm run: --strict, and this run is degraded in 6 way(s). [...]
125
$ fixture podvm run --rm ALPINE /nonexistent-probe-target; echo $?
podvm run: chroot(2) is denied [...] no no-chroot family runs [...]
125         (rootfs byte-identical after)
```

Forced `PODVM_MODE=memfd` enters the static hello at 0, refuses
over dynamic alpine naming the unforced family at 125, and forced
`fuse` refuses naming the force at 125. Unit guards:
`userland.rs` (loader plan, lib dirs, host env, invocation name,
applet argv), `lifecycle.rs` (rung decision), `ladder.rs` (forced
no-chroot drives and refusals), `system.rs` (`.EnteredRung`
`userland` where chroot is denied); lane `cargo test` green over
the three crates beside the drive (podvm 143, podvm-enter 64,
podvm-probe 97 passed, 0 failed).

# T-1339: Enter the namespace rung where the probe permits it

**Source:** issue 59, beta.7 drive 2026-09-25 (`README.md:60-69`
  promised it, `run` never did); `crates/podvm-enter/src/lib.rs:47-61`
  (`ENTERED_RUNG` is `Chroot` on every machine),
  `pod_hostprobe::select` (returns
  `Rung::Namespace`), `crates/podvm/src/system/df_info.rs:213-216`
  (asserts the two differ)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** L
**Status:** done

## Problem

No TODO entry mentions the `namespace` rung, and nothing
enters it. Where the probe succeeds at
`unshare(CLONE_NEWNS)` and mount, `select.rs` returns
`Rung::Namespace` and `run` still enters a plain chroot.
The binary reports both facts (`system info` carries
`.Rung` and `.EnteredRung` with the guard asserting they
differ); the rung itself has no backlog entry.

## Premise

Read at file and line on 2026-09-25: the constant, the
selection, and the assert are as cited. No production
path calls `unshare` or `clone` with a `CLONE_NEW*`
flag; the only `unshare` in the tree is the
interposer's emulation. `docs/architecture.md:32-40`
stays carefully worded and needs no change.

## Approach

Enter the rung the probe already selects: where the
namespace legs hold, `unshare` with mount and ID maps,
then the existing chroot sequence inside; where they do
not, the chroot rung as today. The banner names the
achieved rung with what it does not isolate, and
`.EnteredRung` reports `namespace` where entered. The
`system.rs` guard stays: the two facts must never be
conflated again. The README correction of 2026-09-25
(run enters chroot, both facts carried) stands until
this lands, then reads the new behavior.

## Decision

Implement behind the probe, not beside it. No flag
selects the rung; the legs do, the way every other rung
is selected. A host that loses a leg between probe and
entry falls back to chroot with the banner naming it,
never failing a run the chroot rung could carry.
Out of scope: a network or pid namespace (mount and ID maps
first), changing the probe legs, touching the machine
tier.

## Prove

on a namespace-capable host `podvm probe` reports
`namespace` and `podvm system info --format
'{{.EnteredRung}}'` reports `namespace` after a run
that isolates a mount the host cannot see; on a denied
host both report `chroot` as today.

## Done

**Done 2026-09-26.** `run` enters the rung the probe selects.
Where it selects `namespace`, the child unshares a mount
namespace, remounts `/` recursively private, mounts a private
tmpfs on the image's `/tmp`, and runs the chroot sequence
inside; `.EnteredRung` reads `namespace`. The defect the drive
found is fixed: the entry descriptor opens pre-fork in the base
mount namespace, so `fchdir` plus `chroot(".")` anchors the new
root in that mount and hides the private tmpfs. The namespace
rung chroots by path, which resolves under the private mount,
and a (d,i) guard past the chroot compares the opened
descriptor against `/` (`chroot_landed`, `EXDEV` step 10): a
mismatch falls back to chroot with the step named, never
failing a run the chroot rung carries. No user, pid or network
namespace exists at any rung, and the banner says so
(`mount-only`, `private, tmpfs on /tmp`, `host-shared` network
and pids, the never-claim naming user, pid, network and
`/proc`). An image without a real `/tmp` directory falls back
with the cause named. Detached `start` enters through the same
gate (`spawn_selected`: a fresh probe at entry, the fallback
line in the container log). The README rung row reads the new
behavior.
Prove: `experiments/365-namespace.sh` exits 0
(`experiments/results/namespace.txt`): 3 lane clauses green
(probe below namespace, run enters chroot with no fallback
line, `EnteredRung` chroot).
`experiments/366-namespace-base.sh` exits 0
(`experiments/results/namespace-base.txt`): 5 base clauses
green on the wsl-toolkit base (kernel 7.2.0-WSL2-STABLE,
lane-built musl release binary, pinned alpine 3.20 digest) -
probe selects namespace, a run enters it with the honest
banner and `EnteredRung` namespace, the payload's `/tmp` file
reads back inside and is absent from the host rootfs with no
leaked mount, the no-`/tmp` image falls back naming the cause,
and a detached container runs with its logs carrying the
output. Unit guards: `the_entered_rung_follows_the_selection`,
`the_tmp_mount_point_is_a_real_directory`,
`the_fallback_line_names_the_cause_and_the_rung`,
`the_setup_steps_name_themselves`,
`the_chroot_guard_compares_device_and_file`,
`the_namespace_rung_reports_mount_only`,
`the_namespace_banner_names_no_procfs_mount`, and the
`/dev/null` dev/ino asserts the (d,i) guard reads. The
mid-sequence setup-failure arms (steps 7 through 9) have no
driven trigger on any reachable host: the step mapping is
unit-pinned and the sibling no-`/tmp` arm is driven green by
ns-4, but a failing `unshare` or mount past a permitting probe
is exercised nowhere. Full lane `dev.sh check` green beside
the drives.

# T-1407: Anchor userland paths inside the rootfs or refuse

**Source:** issues 69 and 70; `src/userland.rs` of `pod-enter` (PodUtils/podcrates);
  `crates/podvm-interpose/src/map.rs`
**Category:** enter
**Milestone:** none
**Priority:** P0
**Effort:** L
**Status:** done

## Problem

On the userland rung an absolute guest path resolves on the
host. A write lands on the host tree. A shell grandchild
execs host binaries and mixes libc. The package-install
warning in issue 69 is host mutation through the runtime.

## Premise

The rung states the pass-through in `userland.rs:15`. An
empty map table rewrites nothing (`map.rs:68`). Nothing in
`crates/podvm/src` sets `PODVM_MAPS` for a userland
run (measured by grep: zero hits). `host_env` rewrites
`LD_LIBRARY_PATH` (`userland.rs:257`) and leaves `PATH` to
the caller. First-level lookup is image-side through
`resolve_payload` (`ladder.rs:148`).

## Approach

Set a root-anchoring `PODVM_MAPS` for every userland run,
minus the never-rewrite paths that T-0706 and T-0707 name.
Refuse with a named errno where a call cannot be
virtualized. Add an explicit `--unsafe-host-paths` opt-in
for callers that want the current pass-through.

## Decision

Refusal beats a silent host write. The table is owned by
`run` and set before the payload starts. No caller edits
the table by hand.

## Prove

`cargo test -p podvm-enter` exits 0 with the new unit
guards. A tracked drive on a chroot-denying fixture reads
back a rootfs marker through an absolute path, writes no
host `/tmp` file, and runs `sh -c` compound commands with
image binaries only.

## Correction

**Open 2026-09-30 (history).** Filed from issues 69 and 70. Read the current
source before implementation. The two issues close together; keep
one acceptance clause per issue.

## Done

**Done 2026-09-30.** Every userland run sets a root-anchoring
`PODVM_MAPS` table (`userland_maps_table`: guest `/` inside the
rootfs, minus the never-rewrite paths), the caller value scrubbed
first, through one `userland_env` path for run and both exec entries;
`--unsafe-host-paths` opts into the pass-through with its own parity
row, banner line, and `--strict` refusal. `cargo test -p podvm-enter`
exits 0. `sh experiments/356-no-chroot-rung.sh` exits 0
([the drive](../experiments/results/no-chroot-rung.txt)): the banner
names the anchoring, a `PODVM_MAPS=/:evil` caller value never reaches
the payload, the absolute-path read sees the image release, and a
`/tmp` write never lands on the host. Issue 69 (host mutation through
the runtime) and issue 70 (libc mixing) close on those two clauses.

# T-1408: Resolve the loader symlink inside the rootfs

**Source:** issue 73; `src/userland.rs` of `pod-enter` (PodUtils/podcrates);
  `src/memfd.rs` of `pod-enter` (PodUtils/podcrates);
  `src/abi/search.rs` of `pod-enter` (PodUtils/podcrates)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

Debian's `/lib64/ld-linux-x86-64.so.2` is a symlink to an
absolute target. The loader check follows it on the host,
where it dangles. Debian and gcc images refuse at 125 on
the userland rung.

## Premise

`is_executable` uses metadata, which follows symlinks
(`memfd.rs:49`). `resolve_in` already walks
rootfs-contained links with a bound (`abi.rs:713`).
`loader_plan` does not use it.

## Approach

Resolve `interp_guest` through `resolve_in`, bounded and
staying inside the rootfs, then run `is_executable` on the
result. Keep the refusal naming the guest path where it
still fails.

## Decision

The rootfs is the resolution root for every guest path on
this rung. The host root is never a fallback.

## Prove

`cargo test -p podvm-enter` exits 0. A tracked drive runs
`public.ecr.aws/debian/debian:bookworm-slim echo hi` and
`public.ecr.aws/docker/library/gcc:13-bookworm gcc --version`
on a chroot-denying fixture. Both images join
`experiments/356-no-chroot-rung.sh`.

## Correction

**Open 2026-09-30 (history).** Filed from issue 73. Read the current source
before implementation.

## Done

**Done 2026-09-30.** `loader_plan` resolves the guest interpreter
through `resolve_in` (bounded, rootfs-contained) before
`is_executable`; the refusal still names the guest path.
`cargo test -p podvm-enter` exits 0 with the absolute-link loader
fixture test. `sh experiments/356-no-chroot-rung.sh` exits 0
([the drive](../experiments/results/no-chroot-rung.txt)): debian runs
`sh` on the fixture (clause 4) and `gcc --version` answers on the gcc
image (clause 19), both recorded with their pulled digests.

# T-1409: Answer the payload-visible exe on the loader family

**Source:** issue 74; `crates/podvm-interpose/src/path_calls.rs:63`;
  `src/ladder.rs` of `pod-enter` (PodUtils/podcrates);
  `crates/podvm/src/run/foreground.rs:310`
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

Entering through the loader leaves `/proc/self/exe`
pointing at the loader. Go reads `GOROOT` from
`os.Executable` and fails. The same hazard hits any
self-locating runtime.

## Premise

The exe emulation answers only where the real call fails
with `ENOENT` (`lib.rs:1256`). On the userland rung the
host `/proc` is mounted, so the real `readlink` succeeds
with the loader path and `PODVM_GUEST_EXE` (`run.rs:798`)
is never consulted. `ladder.rs:187` documents the
chroot-only assumption.

## Approach

On loader-family entries answer `/proc/self/exe` and the
`readlinkat`, `open`, and `stat` shapes from
`PODVM_GUEST_EXE` even where the real call succeeds.
State the substitution in the banner.

## Decision

The payload sees its own path, not the loader's. The
emulation is named, never silent.

## Prove

`cargo test -p podvm-enter` exits 0. A tracked drive runs
`go version`, `go env GOROOT`, and a hello build on
`public.ecr.aws/docker/library/golang:1.24.7-bookworm`
over a chroot-denying fixture with no extra `-e`.

## Correction

**Open 2026-09-30 (history).** Filed from issue 74. Read the current source
before implementation.

## Done

**Done 2026-09-30.** The loader entry resolves the payload to its
guest path (`ladder.rs:206`), carries it in `PODVM_GUEST_EXE`
(`run.rs:996`), and the interposer answers `/proc/self/exe` and the
`readlinkat`, `open`, and `stat` shapes from it even where the real
call succeeds (`lib.rs:1541`); the banner names the substitution.
`cargo test -p podvm-enter` exits 0 with the `guest_exe` and
`push_guest_exe` arms pinned. `sh experiments/356-no-chroot-rung.sh`
exits 0 ([the drive](../experiments/results/no-chroot-rung.txt)):
clause 12 forces the exe and reads it back, clause 18 prints
exactly `/usr/bin/readlink` for `readlink /proc/self/exe` as a
first-level payload on debian, and clause 19 runs `gcc --version`
on the gcc image.
Amendment in writing: the entry's go trio is superseded. The golang
`go` binary is statically linked (`file(1)`: ELF static, no INTERP),
so the tier enters it on the memfd family, where no interposer loads
and the kernel reports the staged memfd path: `go version` answers
exit 2 naming GOROOT, and go's own telemetry child fails re-exec'ing
the memfd path. The drive pins that answer as the boundary rather
than as a regression.
Flake on record: the first rerun (lane job `dd382b1867b42856`)
missed the debian payload line once on the identical tree and
inputs; the control (lane job `c057b086fb067005`) is green with no
code change between them.

# T-1410: Exec through the no-chroot family

**Source:** issue 80; `crates/podvm/src/exec/command.rs:227`;
  `crates/podvm/src/run/prepare.rs:49`; [T-0505](enter.md)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** L
**Status:** done

## Problem

`run` works on a chroot-denied host but `exec` on the same
extracted rootfs fails with `EPERM`. `exec` only knows the
chroot path.

## Premise

`exec` calls `podvm_enter::run` directly (`exec.rs:444`).
It never calls `decide_entry`, which is the `run` path
(`run.rs:1078`). T-1317 scoped `exec` out deliberately and
left the helper one call away.

## Approach

Route `exec` through `decide_entry` and the loader and
memfd drive. Keep the fresh-entry-shares-filesystem
contract from T-0505. Refusals name the blocking call.

## Decision

`exec` and `run` share the entry decision. They keep
separate banners and separate records.

## Prove

`cargo test -p podvm` exits 0. A tracked drive
extracts `public.ecr.aws/docker/library/alpine:3.20` and
runs `exec echo hi` with rc 0 on a chroot-denying fixture.

## Correction

**Open 2026-09-30 (history).** Filed from issue 80. Read the current source
before implementation.

## Done

**Done 2026-09-30.** Both exec paths (container and image) route
through `decide_entry` and the loader and memfd drive, keeping the
fresh-entry-shares-filesystem contract with separate banners and
records; refusals name the blocking call. `cargo test -p podvm`
exits 0. `sh experiments/356-no-chroot-rung.sh` exits 0
([the drive](../experiments/results/no-chroot-rung.txt)): clause 15
extracts the image and runs image-exec and container-exec at 0 on the
fixture.

# T-1411: A detached launcher for the no-chroot family

**Source:** issue 72; `crates/podvm/src/run/prepare.rs:50`;
  `crates/podvm/src/lifecycle.rs:214`; [T-1317](enter.md)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** L
**Status:** done

## Problem

`run -d` and `create` refuse where chroot is denied, so
`start`, `ps`, `logs`, `stop`, `rm`, `wait`, `kill`,
`restart`, and `cp` have nothing to act on. `ps` prints an
empty table, which reads as no containers rather than an
unavailable mode.

## Premise

The refusal is deliberate (`run.rs:1079`). `start_one`
refuses at the chroot gate (`lifecycle.rs:213`). T-1317
records `run -d` staying chroot-gated as the decided last
resort. The pidfd and waitid supervision primitives are
present.

## Approach

Drive the userland loader family detached: fork, hold the
pidfd, record the container, keep the foreground path
unchanged. Where a call genuinely blocks detached
operation, the refusal names the call and the affected
verbs.

## Decision

This entry reverses T-1317's last-resort decision in
writing once the drive passes. Until then the refusal
stays.

## Prove

`cargo test --workspace` exits 0. A tracked drive runs
`run -d`, `ps`, `logs`, `stop`, and `rm`, plus `create`
and `start`, on one container over a chroot-denying
fixture.

## Correction

**Open 2026-09-30 (history).** Filed from issue 72. Read the current source
before implementation.

## Done

**Done 2026-09-30.** `create` records a userland entry where chroot
is denied (rung `userland`, exact loader argv in `userland_exec`,
library dirs, `PODVM_MAPS` and `PODVM_GUEST_EXE` in the stored
environment); detached `start` drives it through `spawn_userland`
with pidfd supervision as today; the foreground path is unchanged.
This reverses T-1317's last-resort decision in writing at the three
changed gates, and the strict `ensure_chroot_usable` gate is retired:
only entries with genuinely no detached driver still refuse, naming
the call with the affected verbs. `cargo test --workspace` exits 0.
`sh experiments/356-no-chroot-rung.sh` exits 0
([the drive](../experiments/results/no-chroot-rung.txt)): clause 9
runs `run -d`, `ps`, `logs`, `stop`, `rm` on one container and
`create` plus `start` with `ps`, `logs`, `stop`, `rm` on a second,
over the fixture. Detached payloads stay builtin-only (no grandchild
exec); the `-w /tmp` join reads back exactly `/tmp`; the grandchild
absolute-exec limit from issue 80's sibling reports pins at 127
against the chroot control at 0.

# T-1412: A missing workdir refuses naming the path

**Source:** issue 81; `src/launch.rs` of `pod-enter` (PodUtils/podcrates);
  `crates/podvm/src/run/parse.rs:226`
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`run -w` to a missing directory is accepted with exit 0.
The payload starts in the rootfs root. A script that
depends on the working directory runs in the wrong place
and reads success.

## Premise

Both rungs discard the `chdir` error: the chroot arm runs
from `/` saying so, and the no-chroot arm runs from the
rootfs with the banner naming the start (`lib.rs:1009`).
The flag parses with no validation (`run.rs:301`).

## Approach

Check the workdir before the fork on both rungs. Refuse
naming the path and the image. Keep `-w /tmp` printing
`/tmp` as the passing control.

## Decision

Refusal beats a silent wrong directory. docker creates the
directory; podvm refuses and names it, and the refusal
says so.

## Prove

`cargo test -p podvm-enter` exits 0. A tracked drive
refuses `-w /no/such/dir` with the path named and prints
`/tmp` for `-w /tmp`.

## Correction

**Open 2026-09-30 (history).** Filed from issue 81. Read the current source
before implementation.

## Done

**Done 2026-09-30.** The workdir is checked before the fork on both
rungs and both exec paths (`check_workdir`: resolved under the
rootfs, refused naming the path and the image, noting that docker
creates the directory while podvm refuses); the child race arrives
as a named step-11 error instead of a silent wrong directory.
`cargo test -p podvm-enter` exits 0. `sh experiments/356-no-chroot-rung.sh`
exits 0 ([the drive](../experiments/results/no-chroot-rung.txt)):
clause 10 refuses the missing directory on the fixture and the sane
host and prints exactly `/tmp` for the control.

# T-1413: A script refusal names the file and its shebang

**Source:** issue 78; `crates/podvm/src/lifecycle/entry.rs:219`;
  `src/memfd.rs` of `pod-enter` (PodUtils/podcrates);
  `src/ladder.rs` of `pod-enter` (PodUtils/podcrates)
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`node:*-alpine` refuses as a `#!` script without naming the
file or its shebang. The caller cannot tell whether podvm
picked the entrypoint, a wrapper, or the wrong binary.

## Premise

`eligible` returns the unit variant `RoutePastScript`
(`memfd.rs:147`), so path and first line are already
dropped. `payload_bytes` resolves through `resolve_payload`
(`ladder.rs:148`) but returns bytes only. The refusal
(`lifecycle.rs:1779`) cannot name what it never received.

## Approach

Thread the resolved path and first line into the
`no_chroot` message. Longer term, route the interpreter
inside the rootfs where the map table covers it; that half
belongs to T-1407.

## Decision

The refusal names the exact payload path and its `#!`
line. An anonymous refusal is a defect in the message, not
a separate capability.

## Prove

`cargo test -p podvm-enter` exits 0. A tracked drive shows
the node refusal naming the resolved file and its `#!`
line, or the payload runs.

## Correction

**Open 2026-09-30 (history).** Filed from issue 78. Read the current source
before implementation.

## Done

**Done 2026-09-30.** `RoutePastScript` carries the resolved path and
first shebang line into the `no_chroot` refusal, so the node payload
refuses naming the exact file and its `#!` line instead of
anonymously. `cargo test -p podvm-enter` exits 0.
`sh experiments/356-no-chroot-rung.sh` exits 0
([the drive](../experiments/results/no-chroot-rung.txt)): clause 13
refuses at 126 naming `/hello.sh` and its shebang. Routing the
interpreter inside the rootfs stays with T-1407.

# T-1604: Fix the detached-stdio serial mutex and the 1-of-3 detached start failure

**Source:** CI run 36894578988, job 110478236087, 2026-10-01;
  `crates/podvm/tests/detached_stdio.rs` `:138`, `:292`;
  `experiments/340-detached-stdio.sh`
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

`cargo test --workspace` exits 101 on `detached_stdio` with
`test result: FAILED. 1 passed; 2 failed`. Read in order,
this is one defect with two symptoms, not two defects. The
test at `:292` fails first on its own assertion:
`only 1 of 3 detached starts ran`, `left: 1`,
`right: 3`. The test at `:138` then dies on
`called `Result::unwrap()` on an `Err` value:
PoisonError`, because `SERIAL.lock().unwrap()` at `:138`
inherits the poison left by the test that panicked while
holding the same `Mutex` (`:25`). The same
`SERIAL.lock().unwrap()` shape is at `:167` and `:196`.

## Premise

A serialisation mutex must not propagate one test's panic
into the next. `unwrap()` on a poisoned lock makes the
second failure a report of the first, which is why the two
rows read as one confusing cascade instead of one clear
failure. The underlying 1-of-3 failure is engine- or
runner-dependent and identical on both toolchains, so it is
not a Rust 1.99 regression and not a Batch 3 change.

## Approach

Two fixes, because the cascade hides the real failure. First,
make the serialisation mutex poison-tolerant at `:138`,
`:167`, and `:196`, so a failure in one test is reported as
itself and the next test still runs. Second, find and fix
why two of three detached starts did not run on a loaded
runner; drive `experiments/340-detached-stdio.sh` in the
Linux lane under CI-like conditions before changing the
assertion, because a test may not depend on a scheduling
order it cannot control
(`docs/methodology/authoring.md`, entry fields).

## Decision

The cascade is the reporting defect and the 1-of-3 is the
product defect. Fix the mutex first so the second is
measurable, then measure the second. Do not weaken the
assertion at `:292` to accept 1 of 3: that would record a
green run over a real failure.

## Prove

`cargo test -p podvm --test detached_stdio` passes in
the Linux lane, run twice; a deliberately failing test in
the same binary does not poison its neighbours (the plant
this task owes); the saved lane log shows all three
detached starts running;
`./target/x86_64-unknown-linux-musl/release/podvm-gate` exits 0.

## Done

**Done 2026-11-11.** [The lane log](../experiments/results/detached-stdio-lane.txt)
holds each clause, on the cloud host as uid 0. The plant is the test
`a_planted_panic_with_the_lock_held_poisons_no_neighbour` of
`crates/podvm/tests/detached_stdio.rs`: with `PODVM_PLANT_T1604=1` it
panics while it holds the serial lock, and its name sorts first, so with
`--test-threads=1` each neighbour takes the lock after the panic, in each
run (the order that the earlier control met only by chance). Under the
plant the plant fails and its three neighbours pass; the control, with
`unwrap()` back in the three neighbours, fails each of them on
`PoisonError`. `cargo test -p podvm --test detached_stdio -- --nocapture`
then passes two times, and each run prints its three detached starts,
each with exit 0, an id and an `inspect` row `running PID LAUNCHER -`.
`./target/x86_64-unknown-linux-musl/release/podvm-gate` exits 0.

**Partial 2026-10-02.** The cascade is fixed and measured; the
one-of-three failure is not reproduced and is not root-caused, and the
entry stays open on it.

The reporting defect is gone. All three takes are now
`SERIAL.lock().unwrap_or_else(|e| e.into_inner())`, which is the shape
`tests/store_gates.rs` and `tests/curated_refusals.rs` already use in
this crate, so the file is no longer a holdout. `cargo test --workspace`
in the lane on `rustc 1.99.0` runs `detached_stdio` green, all three:
`detached_run_without_an_image_is_a_cli_error`, `run_help_names_the_detach_flag`,
and `detached_start_returns_before_the_payload_ends`
([the drive](../experiments/results/ci-lint-1.99.txt)).
`./target/x86_64-pc-windows-msvc/debug/podvm-gate.exe` exits 0.

The second defect also got a fix, and it is the one that matters for the
next attempt. The loop breaks on the first non-zero `run -d` and threw
that start's stderr away, so the suite could only report
`only 1 of 3 detached starts ran` and never name the exit that failed.
Each start now carries its stderr and the count assertion prints every
start: which iteration, which name, its exit code, its elapsed time, its
id, what `inspect` returned, and what `run -d` said. A failure that
cannot say what it saw is a failure the next session re-derives.

**The assertion was not weakened.** `:292` still requires three starts,
and the entry's decision stands.

⚠ **The plant's control failed and the plant is not claimed.** A
temporary test that panicked while holding the guard left its two
neighbours green, which is the result the task wanted, and the control
that put `unwrap()` back did **not** reproduce the cascade, so the green
is not evidence. The measured reason: a thread already parked on a
poisoned `std::sync::Mutex` is handed the lock and its `lock()` returns
`Ok`; only a thread arriving after the poison is set gets `Err`. All
three tests call `lock()` on their first line, so all three are normally
queued before any panic. CI saw the cascade because one test reached
`lock()` after the panic completed. It is a scheduling accident, the fix
removes the accident, and no test buildable on this host demonstrates it
reliably. The temporary test is not in the tree.

**CI then named the exit, and it is not a scheduling accident.** Run
36974640619 on this commit, 2026-10-02, four lane runs after the lane was
green here. The stderr this entry added is what settled it:

```
assertion `left == right` failed: only 1 of 3 detached starts ran:
  start 1 (`run -d` for det1-5332-1) exited Some(126) after 5, id "",
  inspect exited Some(1) reading []
    run -d stderr: podvm run: chroot(2) is denied on this machine, and
    no no-chroot family runs this payload: the loader family declines the
    payload: this podvm binary carries no glibc interposer object, so the
    tier is declined; ./scripts/build-interpose.sh builds them.
```

Not a flaky start and not a scheduling order: the start takes 5 seconds
and exits 126 with a refusal naming its own cause, and the refusal is
correct for the tier. What is wrong is the test, because it asserts a
detached start succeeds in an environment where the product refuses. So
the entry's own rule applies, that a test may not depend on a scheduling
order it cannot control, and here it depends on a *tier* it cannot
control: the CI job that builds the workspace tests does not build the
interposer objects, so the binary under test carries no glibc interposer
and the no-chroot family declines. Every start in that job fails the same
way; CI reported 1 of 3 only because the loop breaks on the first.

The three questions this session could not answer from the lane are now
answered from CI, and they change the fix: it is not in the mutex, not in
the launcher, and not in the store. It is that the assertion is
conditional on the environment and does not say so.

What remains, precisely: make the assertion skip with a named reason
where the tier legitimately declines, the way
`tests/store_gates.rs` already does for a host that refuses namespaces,
and keep it strict where the tier is available. The other half of the
answer, whether the CI job should build the interposer before the
workspace tests, belongs with T-1641's owner because the same
`scripts/build-interpose.sh` path defect this session measured is what
leaves the object out of the binary in the first place. Ruled out with evidence, not by assertion: image-lock
contention, the store lock being held by the payload, name collision, and
the deadline. Duration does not discriminate, because failing CI runs at
44.18 s, 48.20 s and 51.05 s overlap passing lane runs at 46.86 s,
48.01 s, 59.44 s and 65.14 s. With the stderr now carried, the next red
run names the exit instead of asking a session to guess it, and
`experiments/340-detached-stdio.sh` under a loaded runner is the
measurement the Approach asked for and this session could not drive.

**Partial 2026-10-02, the tier skip and the CI decision are in.** The
remaining clause is the lane run, not the design. The record gate,
`cargo fmt`, the four prose checks and the workflow's YAML parse all
exit 0.

The assertion is now conditional on the tier, in the way
`tests/store_gates.rs` already conditions on namespaces. Where the no-
chroot family legitimately declines, the test skips and says why:
`crates/podvm/tests/detached_stdio.rs:93-112` requires exit 126
**and** both halves of the refusal, the `decide_entry` sentence from
`lifecycle.rs:1837-1845` and the absent-object leg from
`interpose.rs:222-226`. The exit code alone is not enough and the
comment says why: 126 is also what podvm returns for a loader this
machine cannot run and for a payload that is not an ELF, and those are
real failures. Every recorded start is checked, not the first, because
the loop breaks on the first non-zero. Where the tier is available the
three-start assertion is byte for byte what it was; the entry's
decision to not weaken it stands and was not traded away.

The CI half is decided and done. The `workspace and interposer tests`
job builds the interposer before `cargo test --workspace`
(`.github/workflows/gate.yml:231-244`), mirroring the static-build job.
The premise is verified in source, not assumed: `crates/podvm/
build.rs:86-91` writes an empty placeholder object when the real one is
absent and `:101-122` records that it is empty so the runtime refuses,
so without the step the binary under test interposes nothing. The
second half of the cause is `scripts/build-interpose.sh`, which resolved
only `target/release/` and exited 2 with the binary built, because
`.cargo/config.toml` sets the musl target and the binary lands in
`target/<triple>/release/`. It now searches the flat directory and the
per-triple ones with the same first-match order `release-notes.sh` and
`verify-release.sh` already use, and its exit 2 names where it looked.

Three defects in this entry's own history were false and are corrected
here rather than carried. The mutex cascade was a scheduling accident,
already recorded above; the 1-of-3 was never a scheduling order; and
the entry's earlier statement that the test could be fixed in the mutex
was wrong, because CI's own stderr named the tier as the cause. What
the previous session measured still holds and is what made this fix
findable: a failure that cannot say what it saw is a failure the next
session has to re-derive.

What still owes: the lane run of `cargo test -p podvm --test
detached_stdio` in CI, where the skip and the strict path are both
exercised in place. It could not run from this host because `ring`'s
build script fails here through `scripts/zig-cc.sh`, which a Windows
process spawner cannot execute (`os error 193`); the failure reproduces
on the clean tree with these edits stashed. The predicate was driven
directly against the captured CI stderr and its negatives instead, and
the writer's first attempt at it was caught by those drives: a
text-only match would have skipped a start that exited 0 having
started a container. Adding the exit-code guard turned the plant red
when removed, which is the direction that matters.


**2026-11-06, the first Linux cloud session.** The lane run reproduced the
failure where chroot is permitted (uid 0): `run -d` exited 0 and took
5.1 s to 5.3 s, over the 5 s bound. A strace with stack traces put 5.4 s
of the 6.05 s in `run::trust_record`, which hashed each blob of the
record (one 193,731,527-byte layer) before each start
(`Store::verify_record_blobs`, T-1419). `trust_record` now calls
`Store::absent_record_blobs`, a `stat` for each blob: T-1419 asks for
presence, `podvm_extract::extract` hashes each layer before it writes
(T-1315), and `podvm verify` hashes all of them. The refusal says
"missing" in place of "missing or corrupt". After the change
`cargo test -p podvm --test detached_stdio` passes, 3 of 3 tests, and
`absent_record_blobs_names_a_deleted_layer_and_does_not_hash` passes.
CI, 2026-11-06: gate run 42177673037 at `0496ce7`: the step
"workspace tests" of the job "workspace and interposer tests" exits 0,
`detached_stdio` included. The steps after it had never run in CI; the
next commit fixes the two that then failed (`podvm-buildstate` not
built for experiment 393, and the diagnostics fixture of `podvm-smoke`
with no stub for `check-build-lane`). Gate run 42179344015 at
`7f87f24` is green in each of its four jobs. Remaining of the Prove: the
plant (a failing test in the same binary does not poison its
neighbours), a second lane run, and the saved lane log.

# T-1647: Stream a payload into a memfd: the 128 MiB read ceiling refuses large runtimes

**Source:** issue 90; issue 114 section 3.3 and section 6 item 6;
  `src/ladder.rs` of `pod-enter` (PodUtils/podcrates);
  `src/abi/elf.rs` of `pod-enter` (PodUtils/podcrates);
  `src/memfd.rs` of `pod-enter` (PodUtils/podcrates);
  A-V-04 of PodUtils/podcrates#39
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

On a chroot-denied host podvm refuses a payload over
134217728 bytes at 125 before it runs. `node` in node:alpine
is 149234536 bytes, and a zig toolchain binary is 172641672
bytes. `doctor` and `probe` do not report the cap, so the
image is pulled first and the run fails after.

## Premise

Two 128 MiB ceilings read the whole file into memory.
`Elf::read` (`abi.rs:157`) serves the tier classifier
(`crates/podvm/src/interpose.rs:194`). `payload_bytes`
(`ladder.rs:238`) serves `decide_entry`
(`crates/podvm/src/lifecycle/script.rs:117`), so the loader
family refuses too, though it never stages the bytes.
`memfd::stage` (`memfd.rs:108`) takes one contiguous buffer.
`memfd::eligible` (`memfd.rs:262`) reads the ELF header and
the program headers, not the whole file.

## Approach

Classify from a bounded read of the ELF header and its
tables. Stage the memfd family's payload by a copy from the
file into the memfd in bounded chunks, and hash during the
copy where a digest is needed. Bound the bytes held in
memory, not the file size. Where a size limit must stay,
report it in `doctor` and `probe` and refuse before the pull.

## Decision

File size is not a refusal reason; memory held is bounded.
A static runtime that locates its install tree through
`/proc/self/exe` cannot run from a memfd (issue 90 measured
zig). That case is out of this entry: T-1648 refuses it by
name and T-1652 serves it.

## Prove

`cargo test -p podvm-enter` exits 0 with a test that
classifies and stages a sparse 200 MiB fixture through a
bounded buffer. `cargo test -p podvm` exits 0.
`sh experiments/356-no-chroot-rung.sh` exits 0 with a clause
that runs `node -e 'console.log(1)'` from the node image
with `--entrypoint node`, past the size check.

## Correction

**Blocked 2026-11-10.** Blocker: the code of the two ceilings and of
`memfd::stage` moved to pod-enter with podcrates v0.9.0 (podvm `423e817`),
so the fix belongs to podcrates. COORD-016 (issue 222) asks for it. The
condition that clears the block: a podcrates tag whose pod-enter
classifies from a bounded read and stages in bounded chunks, taken by
podvm. Then the clause of experiment 356 (`node -e` past the size check)
proves it here.

**Filed 2026-11-06.** Quoted from issue 90 and its comments, measured on
release 0.1.0-beta.12 on the errand host:

```text
$ podvm run --entrypoint node node:alpine -e 'console.log(1)'
podvm run: chroot(2) is denied on this machine, and no no-chroot family
runs this payload: node resolved but its bytes could not be read:
.../rootfs/usr/local/bin/node is 149234536 bytes, over the
134217728-byte ceiling podvm reads a payload within.   [exit 125]
streamed into a memfd in 128 KiB chunks (sandhome shims/memexec.c):
$ time /tmp/sandhome-memexec /state/home/noexec-test/zig version
0.16.0
real    0m0.126s
$ /tmp/sandhome-memexec /state/home/noexec-test/zig env
error: unable to find zig installation directory '/memfd:sandhome (deleted)'
```

**Amended 2026-11-09.** For T-1647, the ask A-V-04 of PodUtils/podcrates#39 (COORD-015,
a message from a peer session) wants a Node CI job to run from an OCI image
with no root. This entry is one step of that chain: the `node` binary is
over the read ceiling (T-1661). Its clauses do not change. T-1646 holds the
end-to-end clause.

## Done

**Done 2026-11-10.** For T-1647, the last clause of the partial record below is run. `PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh` exits 0 on the cloud host, verdict "NO-CHROOT FAMILY RUNS" ([the run](../experiments/results/no-chroot-rung.txt)): the images come from `mirror.gcr.io` by the switch of 2026-11-10, each pulled digest is in the conditions, and the capable host enters the namespace rung.

**Partial, 2026-11-10.** The block cleared: podvm took pod-enter 0.11.0
(podcrates podcrates@T-139, PodUtils/podcrates#46). pod-enter classifies from a
bounded head (`payload_head`, `interp_of_file`) and stages the memfd
family's payload from the open file in bounded chunks (`stage_file`);
the 128 MiB refusal is gone, and the crate tests a sparse 200 MiB fixture.
podvm's three call sites (`ladder.rs`, `lifecycle/userland.rs`,
`lifecycle/script.rs`) use the head. `cargo test --workspace` exits 0.
[The drive](../experiments/results/payload-over-128mib-1647.txt) on the
chroot-denying fixture: `node -e` runs at 0 three times, and node padded
to 161523528 bytes, above the old ceiling of 134217728, runs at 0.
Remaining: the full run of experiment 356 (`public.ecr.aws` answered
HTTP 429).

# T-1648: A static Go payload runs into a GOROOT error instead of a named refusal

**Source:** issue 91; issue 114 section 3.4;
  `crates/podvm/src/lifecycle/entry.rs:353`;
  `crates/podvm/src/lifecycle/entry.rs:384`;
  `crates/podvm/src/interpose.rs:365`;
  `experiments/356-no-chroot-rung.sh:610`;
  A-V-04 of PodUtils/podcrates#39
**Category:** enter
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

On a chroot-denied host `go version` from the golang image
exits 2 with the payload's own error: "cannot find GOROOT
directory". The error blames the image. The real reason
prints only under `--verbose`. The payload sees the host
tree: `go build -o /tmp/gohello` wrote a 2238818-byte file
into the host `/tmp`. That is a host write by a container
payload, with the cause hidden.

## Premise

`decide_entry` sends every static payload to the memfd
family (`lifecycle.rs:1895`), where no interposer loads. Its
banner says that absolute paths resolve on the host
(`lifecycle.rs:1919`), and the quiet default of T-1416 hides
the banner. `interpose::apply` records the Go decline as a
banner note (`crates/podvm/src/interpose.rs:365`),
which the quiet default also hides. Clause 18 of experiment
356 pins exit 2 naming GOROOT as the boundary
(`356-no-chroot-rung.sh:574`).

## Approach

On a chroot-denied host, refuse a static payload at 125
before the exec, unless `--unsafe-host-paths` opts in. That
is T-1407's flag for this exact host pass-through. The
refusal says the payload is static, says its absolute paths
resolve on the host, and names the routes: a host with
chroot, the opt-in, and the rungs of T-1651 and T-1652 where
they exist. Under the opt-in, the host-root line prints even
under `-q`. Every clause of experiment 356 that drives the
memfd family passes the opt-in. Clause 18 changes from exit
2 to exit 125.

## Decision

Refusal beats a silent host write, T-1407's rule applied to
the memfd family. podvm does not inject `GOROOT`: clause 18
already refused payload-specific magic. The measured
host-path `GOROOT` recipe stays a caller choice under the
opt-in.

## Prove

`cargo test -p podvm` exits 0 with a test: a static
fixture payload refuses at 125 without the opt-in and
enters the memfd family with it.
`sh experiments/356-no-chroot-rung.sh` exits 0 with clause 18
changed: `go version` from the golang image refuses at 125
naming the static reason, and never exits 2 naming GOROOT.

## Correction

**Filed 2026-11-06.** Quoted from issue 91 and its comments, measured on
release 0.1.0-beta.12 on the errand host:

```text
$ podvm run golang:1.24-alpine go version
go: cannot find GOROOT directory: 'go' binary is trimmed and GOROOT is not set
[exit 2]
--verbose: podvm run: interpose: declined for "go": go carries Go build
markers, and Go issues syscalls directly rather than through libc, so the
tier cannot reach it
$ podvm run -e GOROOT=<store>/rootfs/<golang>/usr/local/go \
    golang:1.24-alpine /usr/local/go/bin/go version
go version go1.24.13 linux/amd64          # the host path works
$ podvm exec ... gb /usr/local/go/bin/go build -o /tmp/gohello main.go
$ ls -l /tmp/gohello                      # 2238818 bytes on the HOST
```

**Amended 2026-11-09.** For T-1648, the ask A-V-04 of PodUtils/podcrates#39 (COORD-015,
a message from a peer session) wants a Node CI job to run from an OCI image
with no root. This entry serves the static tools of such a job, for example
`go`. It is not on the Node path. Its clauses do not change.

## Done

**Done 2026-11-10.** For T-1648, the last clause of the partial record below is run. `PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh` exits 0 on the cloud host, verdict "NO-CHROOT FAMILY RUNS" ([the run](../experiments/results/no-chroot-rung.txt)): the images come from `mirror.gcr.io` by the switch of 2026-11-10, each pulled digest is in the conditions, and the capable host enters the namespace rung.

**Partial, 2026-11-09.** `decide_entry` takes the decided
`--unsafe-host-paths`. On a chroot-denied host a static payload refuses at
125 before the exec without it; the sentence names the static link, the
host paths, the host write and the routes (a host with chroot, the
opt-in, T-1651, T-1652). Under the opt-in, `userland_env` writes the
host-root line to stderr at once, so it prints under the quiet default
and `-q`. `--strict` still refuses the opt-in. Experiment 356 passes the
opt-in on its memfd clauses, and clause 18 (`fx-go-refuse`,
`356-no-chroot-rung.sh:574`) now asks for 125 naming the static reason
and no GOROOT. `cargo test -p podvm` exits 0 (274 passed) with
`a_static_payload_refuses_without_the_opt_in`. [A hand drive](../experiments/results/no-chroot-static-refusal.txt)
through the deny-chroot fixture of experiment 356 gave 125 for the static
hello and for `go version` (no GOROOT), 0 with the opt-in under the
quiet default, `-q` and `--verbose`, and 125 for `--strict
--unsafe-host-paths`. Remaining: one full run of `sh
experiments/356-no-chroot-rung.sh` (its pulls from `public.ecr.aws`
answered HTTP 429 on 2026-11-09). Known gaps, outside this entry: on
`run -d` and `create` the refusal names an opt-in that the detached
refusal of T-1411 then refuses, and `ensure_entry_possible` lets the pull
and the extract run before the static refusal.

# T-1651: An emulate tier on a prefix-preserving, fail-closed qemu-user for static payloads

**Source:** issue 94; issue 114 section 3.1 and section 4;
  `crates/podvm/src/tier.rs:26`;
  `crates/podvm-probe/src/report/document.rs:23`;
  `src/safety.rs` of `pod-extract` (PodUtils/podcrates); [T-0506](enter.md)
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

On a host that denies chroot, user namespaces, mount and
ptrace, a static payload has no rung that keeps it inside
the image. qemu-user with `-L` runs one process in the image
and leaks in three ways. A child it execs runs from the
host. A path absent from the rootfs is read from the host.
An absolute link in the image resolves on the host.

## Premise

`--podvm-tier` takes `machine` or `chroot` only
(`tier.rs:26`). The probe marks an emulator's answers in
`measured_by` (`report.rs:451`), the T-0506 rule. In qemu,
`do_execve` hands the raw guest path to the host, and
`util/path.c` `path()` returns the unprefixed host path
after an `access()` miss. `openat2(RESOLVE_IN_ROOT)` works
unprivileged on the errand host's kernel 7.2.2, and podvm
already wraps it in `podvm_extract::safety`
(`safety.rs:261`).

## Approach

Add an `emulate` tier. podvm invokes a qemu-user as its own
process, patched in three places. `do_execve` applies the
prefix and re-execs the emulator with the same `-L`.
`path()` fails closed with `ENOENT`. Path resolution, `stat`,
`access` and `readlink` included, goes through a root
descriptor with `RESOLVE_IN_ROOT`. The tier answers
`/proc/self/exe` with the guest path (T-1409's rule). The
banner names the tier, and the probe marks its answers as
the emulator's (T-0506).

## Decision

The emulate tier is a rung the caller selects or the ladder
names, never a hidden fallback. qemu stays a separate GPL
process, the boundary the machine tier's `qemu-system`
already keeps. podvm refuses an unpatched qemu, because its
fallback serves host files. For a native static payload
T-1652 is the first choice; this tier covers other
architectures and payloads whose exec or threads that loader
cannot follow. The cost is an estimate of about 2x native
(sandhome measured 0.421 s against 0.845 s on one payload).

## Prove

`cargo test -p podvm` exits 0 with tests for the tier
value and its banner. `sh experiments/356-no-chroot-rung.sh`
exits 0 with an emulate clause: `go version` from the golang
image and a shell pipeline exit 0 under the tier, a path
absent from the rootfs is refused and not read from the
host, and `podvm probe` under the tier reports
`measured_by.emulated` true.

## Correction

**Filed 2026-11-06.** Quoted from issue 94 and its comments, measured
with `qemu-x86_64` 11.1.1 (static-pie) on the errand host:

```text
$ qemu-x86_64 -L <alpine-rootfs> <alpine-rootfs>/bin/busybox cat /etc/os-release
NAME="Alpine Linux"                     # absolute paths are prefixed
$ qemu-x86_64 -L <golang-rootfs> <golang-rootfs>/usr/local/go/bin/go version
go version go1.24.13 linux/amd64        # a static payload runs
$ qemu-x86_64 -L <alpine-rootfs> <alpine-rootfs>/bin/busybox sh -c 'cat /etc/os-release'
cat: /etc/os-release: No such file or directory   # the child left the image
$ qemu-x86_64 -L <alpine-rootfs> <alpine-rootfs>/bin/busybox cat /workspace/hostonly.txt
LEAK-PROBE                              # a host file served on a miss
openat2(rootfd, "/bin/cat", RESOLVE_IN_ROOT)             -> fd 4, magic 7f454c46
openat2(rootfd, "../../etc/os-release", RESOLVE_IN_ROOT) -> fd 4, clamped
openat2(rootfd, "/no/such/file", RESOLVE_IN_ROOT)        -> -1 ENOENT
```

The effort of T-1651 was XL before call C17 of `docs/decisions.md`; pod-todo has no XL.

## Done

**Done 2026-11-11.** `cargo test -p podvm` exits 0 with the tests of the tier
value (`--podvm-tier=emulate`) and of the emulator on `PATH`, and
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh` exits 0 on the
cloud host ([the run](../experiments/results/no-chroot-rung.txt), verdict "NO-CHROOT FAMILY
RUNS", qemu-user 8.2.2 of Ubuntu): in clause 36 `go version` of the golang
image exits 0 under the tier with its banner, an arm64 alpine pipeline
(`uname -m; cat /etc/alpine-release | tr . -`) exits 0 and prints `aarch64`,
a file that only the host has is ENOENT and not read, and `podvm probe
--json` as the payload reports `measured_by.emulated` true. The paragraph
below has the design, which needs no patch of qemu.

**Partial, 2026-11-11.** The design changes, and the Decision holds: qemu
needs no patch. The emulator runs on the native-static rung (T-1652), so
each of its system calls comes to podvm's handler, which resolves each path
in the image with `openat2(RESOLVE_IN_ROOT)`; podvm passes no `-L`, so a miss
is ENOENT in the image, never the host's file (the second leak), and an
absolute link resolves in the image (the third). The loader of the rung puts
the emulator in front of an ELF that it serves (`-0 ARGV0 GUEST ARGS`), and
the `execve` of a guest re-executes podvm with the emulator
(`PODVM_DISPATCH_EMU`), so a child stays in the image (the first leak).
`--podvm-tier=emulate` (`crates/podvm/src/lifecycle/emulate.rs`) finds a
static `qemu-ARCH-static` or `qemu-ARCH` on `PATH`, as the machine tier finds
`qemu-system-*`, and serves a foreign image (its ELF machine) or an image of
this machine (each ELF); `exec` and `create` refuse the tier. The task's
name is the emulator's, so `podvm probe` under the tier reports
`measured_by.emulated` true. A defect found on the way: qemu's host signal
handler writes a full mask into the frame, so the wrapper of the rung
clears SIGSYS there after each handler (Go under qemu-x86_64 died on signal
31). `cargo test -p podvm` and `cargo test -p podvm-dispatch` exit 0 with
tests of the tier value, the emulator on `PATH` and the emulator's route
through an exec and a spawn. On the chroot-denied fixture: `go version` of
the golang image under qemu-x86_64-static, `uname -m; cat
/etc/alpine-release | tr . -` of an arm64 alpine under qemu-aarch64-static,
a host-only path gives ENOENT, and the probe reports `emulated: true`.
Remaining: the run of experiment 356 with clause 36.

# T-1652: A native-static rung: userland execve, Syscall User Dispatch and openat2(RESOLVE_IN_ROOT)

**Source:** issue 109; issue 114 section 1, section 4 and section 6
  item 1; `crates/podvm-probe/src/select.rs:35`;
  `pod_hostprobe::probes`;
  `src/memfd.rs` of `pod-enter` (PodUtils/podcrates);
  A-V-04, G-09 of PodUtils/podcrates#39
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

On the errand host the supervise rung is out of reach. A
tracer reads a path argument through `process_vm_readv`,
which is `EPERM`, or `/proc/<pid>/mem`, which is `EACCES`.
A static payload therefore has no native rung that keeps it
inside the image.

## Premise

Syscall User Dispatch (Linux 5.11 and later) delivers
`SIGSYS` to a handler in the payload's own address space.
The handler reads the path argument with a plain
dereference. Issue 109 measured the trap and an in-root open
on kernel 7.2.2 as uid 966. Dispatch does not survive
`fork`, `clone` or `execve`: a forked child did not trap. The
rung list (`select.rs:35`) has no such rung. The probe's
supervise leg measures `process_vm_readv` (`probes.rs:266`).
The memfd driver stages a payload (`memfd.rs:108`).

## Approach

Build the rung from four parts. A userland execve loader maps
the static payload into its own address space and jumps to
it. `PR_SET_SYSCALL_USER_DISPATCH` with
`PR_SYS_DISPATCH_EXCLUSIVE_ON` makes the loader's own code
the trusted region, so every payload syscall dispatches. The
handler serves path calls through `openat2(rootfd, path,
RESOLVE_IN_ROOT)`. The handler intercepts `clone` and `fork`
to arm dispatch in the child, and `execve` to load again.
Add a probe leg for the `prctl`, a rung word, and
`measured_by.user_dispatch`. Stage the bytes through T-1647.

## Decision

For a native static payload this rung is the first choice
over T-1651: native speed, no extra binary, no privilege. A
refused `prctl` or an older kernel falls back to the emulate
tier by name, never silently. Dynamic payloads stay on the
loader family.

## Prove

`cargo test -p podvm-enter` and `cargo test -p podvm-probe`
exit 0 with tests for the trap, the in-root open and the
re-arm in a child. `sh experiments/356-no-chroot-rung.sh`
exits 0 with a native-static clause: `go version` and a
`go build` of a hello program exit 0 from the golang image,
the probe reports `measured_by.user_dispatch` true, and with
the `prctl` refused the run names the emulate tier.

## Correction

**Filed 2026-11-06.** Quoted from issue 109, measured on the errand
host (kernel 7.2.2, uid 966, no chroot, user namespace, mount or ptrace):

```text
prctl(PR_SET_SYSCALL_USER_DISPATCH, PR_SYS_DISPATCH_EXCLUSIVE_ON, 0, 0, &selector) -> 0
traps=1 raw_getpid_via_sud=4242 real=16567
host has /etc/os-release: no
SUD-CHROOT traps=1 rewritten=1 fd=4 line=NAME="Alpine Linux"
selector: 0 = SYSCALL_DISPATCH_FILTER_ALLOW, 1 = BLOCK
```

**Amended 2026-11-09.** The asks A-V-04 and G-09 of PodUtils/podcrates#39
(COORD-015, a message from a peer session) touch this entry. A-V-04: this
rung serves the static tools of a Node CI job. The `node` binary goes
through the loader family, and T-1646 holds that end-to-end clause. G-09
wants a container engine inside a unit, for `services:` and Docker actions.
No rootfs tier can hold an engine: there is no namespace, no mount and no
bridge. Inside a unit the names `docker` and `podman` answer podvm itself
(T-0803), so an action that runs one container can work. A `services:`
network between containers cannot work on a rootfs tier, because the payload
shares the host network. A real engine runs only in a machine-tier guest,
where the guest kernel is root (T-1656). This entry changes no clause.

The effort of T-1652 was XL before call C17 of `docs/decisions.md`; pod-todo has no XL.

## Done

**Done 2026-11-11.** `cargo test -p podvm-dispatch` exits 0 with seven
tests (the trap and `/proc/self/exe`, the in-root open and `..`, an
absolute link, the re-arm in `fork`, a thread and `posix_spawn`, the exec,
a script); the tests are in `podvm-dispatch`, the crate of the engine, and
not in `podvm-enter` and `podvm-probe` as the Prove first wrote.
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh` exits 0 on the
cloud host ([the run](../experiments/results/no-chroot-rung.txt), verdict
"NO-CHROOT FAMILY RUNS"): on the chroot-denied fixture the static hello
runs on the native-static rung with its banner; with the `prctl` denied
(`--deny-dispatch`) it refuses at 125 naming the static reason, the `prctl`
and the emulate tier; in the golang image `go version` exits 0, `go build`
of a hello program exits 0, the built program prints `g356-ok`, nothing
lands on the host; and `podvm probe --json` reports
`measured_by.user_dispatch` true. The paragraph below has the parts and the
four defects found on the way.

**Partial, 2026-11-11.** The rung is in the crate `podvm-dispatch`
(`crates/podvm-dispatch/src/`): a loader of a static x86_64 program
(ET_EXEC or static PIE, and a `#!` script of one) into podvm's own process,
a SIGSYS handler with podvm's code as the region of
`PR_SET_SYSCALL_USER_DISPATCH`, the path calls through
`openat2(RESOLVE_IN_ROOT)` with `/proc`, `/sys` and `/dev` the host's,
`/proc/self/exe` as the guest program, a clone trampoline that arms each
thread, `fork` and `vfork` children that arm again, a wrapper for each
caught signal, the program break, and `execve` as an exec of podvm itself.
`decide_entry` takes the rung for a static payload where chroot is denied
and `--unsafe-host-paths` is absent (`crates/podvm/src/lifecycle/native.rs`);
`podvm probe --json` reports `measured_by.user_dispatch`, and the fixture
`experiments/deny-chroot.c` takes `--deny-dispatch`. The tests are in
`podvm-dispatch`, not in `podvm-enter` (the engine is a crate of its own):
`cargo test -p podvm-dispatch` exits 0 with seven tests (the trap and
`/proc/self/exe`, the in-root open and `..`, an absolute link, the re-arm in
`fork`, a thread and `posix_spawn`, the exec, a script). On the cloud host,
on the chroot-denied fixture, `podvm run --rm GOLANG go version` exits 0,
and with `--deny-dispatch` the run refuses at 125 naming the `prctl` and the
emulate tier; `go build -a` of a hello program takes 4.3 s and its output
runs. Four defects were found and fixed on the way, each with its
measurement in the source: a vfork child on musl's 5 KiB stack, the mask and
the alternate stack that `rt_sigreturn` restores, and an alternate stack
that the kernel keeps while a handler runs on it. Remaining: the run of
experiment 356 with its new clauses (its first run failed only on the
`--rm` of the source step, which extracts the image again; fixed). T-1724
holds a static child of the loader family and `-v`.

# T-1653: `-v` volumes on a host with no /dev/fuse and no mount

**Source:** issue 130 (130 reposts the hidden issue 104); issue 114
  section 3.6; `crates/podvm/src/parity.rs:222`;
  `src/userland.rs` of `pod-enter` (PodUtils/podcrates);
  `crates/podvm-interpose/src/map/rewrite.rs:75`;
  A-V-06, G-07 of PodUtils/podcrates#39
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

`run -v` is refused on every rung. The errand host has no
`/dev/fuse` and denies `mount(2)`, so podvm can make neither
an overlay nor a bind mount. A payload has no way to see a
host directory at a guest path.

## Premise

The parity table refuses `-v, --volume` with status None:
"a volume would be a copy pretending to be a mount"
(`parity.rs:335`). The userland table is one pair, `/` to
the rootfs (`userland.rs:323`). The interposer takes the
longest matching prefix (`map.rs:325`), so a pair for a guest
directory wins over the root pair. T-0703 drove a mapped pair
end to end and left `-v` out of scope.

## Approach

On the loader family, turn each `-v HOST:GUEST` into a
`GUEST:HOST` pair in the `PODVM_MAPS` table beside the root
pair. Apply the T-1407 spelling checks to both sides. Refuse
`:ro` by name until podvm enforces it. The memfd family has
no interposer, so it keeps the refusal until T-1652 maps a
volume prefix to a host directory descriptor. The parity row
moves from None to Degraded and names the family that
serves it.

## Decision

A volume on this host is a path rewrite. It is not a copy and
not an overlay. A write lands in the host directory, as with
a docker bind mount. The two-root view of issue 114 copies
executables, which is the copy the parity row refuses, so
podvm does not use it. A payload execs from a volume only
where the host mount allows exec, and the banner names a
noexec source.

## Prove

`cargo test -p podvm-enter` and `cargo test -p podvm`
exit 0 with table tests for a volume pair and a refused
spelling. `sh experiments/356-no-chroot-rung.sh` exits 0 with
a volume clause on the fixture: a host file reads back at
the guest path, and a guest write lands in the host
directory. A `:ro` volume gives `EROFS` for a guest write and leaves the
host file unchanged, once the read-only layer of T-1655 exists.

## Correction

**Filed 2026-11-06.** Issue 130 rebuilds the hidden issue 104 from
issue 114, which states the case in one table row:

```text
overlay/volumes | no -v (issue #104) | no /dev/fuse, no mount |
two-root view (copy regular executables, symlink data back), not an overlay
references: containers/fuse-overlayfs (needs /dev/fuse), mpartel/bindfs,
trapexit/mergerfs; sandhome sh_promote_tree copies execs, links the rest
```

**Amended 2026-11-09.** The asks A-V-06 and G-07 of PodUtils/podcrates#39
(COORD-015, a message from a peer session) want `-v HOST:GUEST` through the
path map, `:ro` with `EROFS`, and caches kept apart for each trust level.
The path map is the Approach. One clause changes: `:ro` is refused by name
only until the read-only layer of T-1655 exists. After that, `:ro` maps to
it, and a guest write fails with `EROFS`. G-07 needs no cache store in
podvm. podvm keeps no mutable cache between runs. The runner picks one host
directory for each trust level. It mounts that directory `:rw` for a trusted
job and `:ro` for an untrusted job. Two jobs of different trust levels never
share a writable volume source, and the runner is the actor that enforces
this.

**Amended 2026-11-10, for podrunner.** The podrunner design review (the
owner's design review of PodUtils/podcrates#47; the ruling "podrunner",
2026-11-10; SPEC section 10.1 and its WP-V6) says that podvm refuses a workdir
inside a volume. That is stale since podcrates v0.12.0 (5e5629a8): the
`-v` row says that a working directory or a first program in a volume
resolves through the table (`crates/podvm/src/parity.rs:222`), and `exec`
checks its workdir through the container's table
(`crates/podvm/src/exec/enter.rs:203`). One clause is new: the drive adds
`exec -w` into a volume of a `run -d` container. The remaining clause,
`:ro` with `EROFS`, is the one podvm gap of the runner's job-container
table (`/__e` read-only, T-1655, T-1711). A relative first program in a
volume workdir is T-1707.

## Done

**Done 2026-11-11.** The read-only layer of T-1655 exists (the record of
T-1655), and a `:ro` volume uses it: `-v HOST:GUEST:ro` puts `ro:GUEST` in
`PODVM_RO`, and the interposer answers `EROFS` for a write under GUEST.
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh` exits 0
([the run](../experiments/results/no-chroot-rung.txt)): clause 26 on the
chroot-denied fixture reads `vol-in` from a `:ro` volume, `touch` in it
prints "Read-only file system", and no file appears in the host
directory; the other volume clauses hold as before. `cargo test -p podvm`
exits 0 with `the_read_only_table_names_each_volume` and the parse of
`:ro` in `volumes_parse_in_each_spelling`.

**Still partial, 2026-11-10 (podcrates v0.12.0).** podvm takes the 11
crates at v0.12.0; one `pod_enter::userland::Maps` carries the root and
each volume through the workdir check, the entry, the `#!` follow and the
interposer. `-w` and a first program inside a volume now run.
`cargo test --workspace` exits 0 (649 passed), the interposer tests exit
0 (69 passed), and `PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh`
exits 0 with the steps fx-vol-workdir, fx-vol-copy and fx-vol-prog
([the drive](../experiments/results/volumes-1653-v0120.txt)). Remaining:
`:ro` with EROFS (T-1655).

**Still partial, 2026-11-10.** `PODVM_356_MIRROR=1 sh
experiments/356-no-chroot-rung.sh` exits 0 with clause 26
([the run](../experiments/results/no-chroot-rung.txt)). Remaining: `:ro`
with EROFS (T-1655), and `-w` and a first program inside a volume, which
wait for the pod-enter of podcrates v0.12.0 (COORD-018, #224).

**Partial, 2026-11-10.** On the loader family each `-v HOST:GUEST[:rw]`
becomes a `GUEST:HOST` pair in `PODVM_MAPS` after the root pair
(`lifecycle/volume.rs`); the host side is canonical, so the reverse map
holds after a `cd` into the volume. The T-1407 spelling checks run on
both sides in podvm-enter's glue. Refused by name: `:ro` (T-1655), other
options, named volumes, the memfd family (T-1652), the chroot rung, the
machine tier, and `-w` or a first program inside a volume (pod-enter
resolves those inside the rootfs; COORD-018). A container record keeps
the table for `exec`. The banner names each volume and a noexec source.
The parity row is Degraded and names the loader family. `cargo test
--workspace` exits 0 (737 passed) and the interposer tests exit 0.
[The drive](../experiments/results/volumes-1653.txt): before, each `-v`
exited 125; after, a read, a write that lands in HOST, a relative write
after `cd /data`, grandchildren from the volume, and `run -d` then `exec`
work, and each refusal above exits 125. Clause 26 of experiment 356 holds
it. Remaining: `:ro` with EROFS (T-1655), `-w` and a first program inside
a volume (COORD-018), and the full experiment 356 run (`public.ecr.aws`
answered HTTP 429).

# T-1657: A syscall-interpreting tier: the gVisor capability without user namespaces

**Source:** issue 169; #193; the owner's ruling of 2026-11-06
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

gVisor's `runsc` answers a payload's syscalls in user space, so
the payload does not reach the host kernel for files. On the
target host `runsc` cannot start: it needs a user namespace,
mount and ptrace, and the host denies all three. podvm has no
tier that gives the same view: a fixed kernel identity, a
private `/proc`, a sized `/tmp` and a placed `/workspace` that
writes through.

## Premise

T-1652 builds the native-static rung: a userland execve loader
with Syscall User Dispatch, whose handler serves path calls
through `openat2(RESOLVE_IN_ROOT)`. T-1651 is the emulate tier
for foreign and other payloads. `--podvm-tier` takes `machine`
or `chroot` only (`crates/podvm/src/tier.rs:22`). The rung list
is `crates/podvm-probe/src/select.rs:35`. The issue names the
podcrates crate `pod-sud` (wave M7).

## Approach

On top of the T-1652 handler, answer `uname` with a fixed
string that names the tier. Serve a private `/proc` view for
the payload's own entries. Serve `/tmp` from a store directory
with a size cap. Place `/workspace` as a path-map entry whose
writes reach the host directory, and plant the F14 trap: a
write that lands in a private overlay fails the test. Resolve
`..`, absolute links and a missing file in the root or fail
closed. Where a host grants user namespaces, offer `runsc` as
an optional backend from the namespace rung.

## Decision

An in-process handler shares the payload's address space. It
is a compatibility layer, not a boundary, and the banner says
so, as it does for the interposer. podvm does not copy gVisor's
network stack. The optional `runsc` backend is not the answer
for the target host. Build on `pod-sud` once podcrates
releases it.

## Prove

`cargo test -p podvm-enter` exits 0 with tests for the `uname`
answer, the `/tmp` cap and fail-closed resolution of `..`, an
absolute link and a missing file.
`sh experiments/413-syscall-tier.sh` exits 0 on a
chroot-denied fixture: a static payload reads `/etc/os-release`
from the image with no ptrace and no qemu, its write to
`/workspace` is on the host after the run, `uname -r` names the
tier, and the banner says the layer is not a boundary.

## Correction

**Filed 2026-11-06.** Quoted from issue 169:

```text
runsc --root RUN/runsc-root --rootless --network=none|host --ignore-cgroups
      --platform=systrap run -bundle RUN/bundle sand-ID
inside: uname -r is 4.19.0-gvisor; /usr is not writable; /workspace writes
      through to the host
F14: `runsc do -volume SRC:/workspace` wrote into gVisor's in-memory overlay
      and never reached the host; a bundle with a bind mount did
F14: runsc refuses rootless create ("Rootless mode not supported with
      "create""), so an image under gVisor needed rootful podman
--ignore-cgroups is required with no cgroup delegation
```

## Done

**Done 2026-11-11.** `cargo test -p podvm-dispatch` exits 0 with
`the_syscall_tier_answers_uname_with_its_release`,
`the_syscall_tier_caps_tmp_across_a_fork` (a cap of 1000 bytes: 800 bytes
fit, 400 more from a child fail with ENOSPC, an unlink gives the space
back), `a_missing_file_of_the_root_is_enoent_and_not_the_hosts`, the unit
test `the_tier_counts_growth_and_names_its_release`, and the resolution
of `..` and an absolute link of T-1652.
`PODVM_413_MIRROR=1 sh experiments/413-syscall-tier.sh` exits 0 on the
chroot-denying fixture (`experiments/results/syscall-tier.txt`):

| clause | reading |
| --- | --- |
| 1 | the static busybox of `busybox:musl` reads `ID=podvm413` from the image's `/etc/os-release` |
| 2 | `TracerPid: 0`, no qemu process, the banner names the syscall tier |
| 3 | the write to `/workspace` is on the host after the run |
| 4 | `uname -r` prints `6.1.0-podvm-syscall` |
| 5 | the banner: "The handler shares the payload's address space: it is a compatibility layer, not a boundary" |
| 6 | `--tmpfs /tmp:size=1m`: `dd` stops with "No space left on device" at 1048576 bytes, and no directory of `/tmp` stays |
| 7 | a dynamic program and `run -d` refuse by name (exit 125) |

Decisions and deviations:

* The tier is built in `podvm-dispatch` (`tier.rs`), as T-1652 built the
  rung: podcrates has no `pod-sud`, and the handler is podvm's. The tests
  are in `podvm-dispatch`, where the handler is, not in `podvm-enter`.
* `/tmp` is a directory of the run (the `--tmpfs` of T-1655). The handler
  counts the growth of each file that a descriptor opened under `/tmp`
  writes, in a shared page of a memfd, so the count holds across `fork`
  and `execve`. `docs/limits.md` names what the count does not see.
* The private `/proc` view is the links of `/proc/self` in guest paths
  (T-1652). The entries of other processes stay the host's.
* The tier runs a static x86_64 program; a dynamic one refuses and names
  the emulate tier. A detached run refuses, because `/tmp` belongs to a
  foreground run.
* The optional `runsc` backend of the Approach is T-1729: the Decision
  says it is not the answer for the target host.
* A dynamic program on the rung is T-1731, and the `--strict` gap of the
  loader family names it (the gap named T-1657 until this entry was done,
  and CI run 387 failed on `each_listed_gap_names_an_entry_that_is_not_done`).

# T-1658: A confine layer: Landlock, seccomp and no_new_privs on every rootfs tier

**Source:** issue 170; #193; the owner's ruling of 2026-11-06;
  A-V-01 of PodUtils/podcrates#39
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

bwrap and bailey confine a host process with user and mount
namespaces, which the target host denies. Landlock needs no
privilege, but podvm applies no Landlock ruleset, no seccomp
deny filter and no built environment to a payload. A profile
cannot say "read these paths, write these, connect to port 443
only".

## Premise

The probe measures `landlock_create_ruleset(VERSION)`
(`pod_hostprobe::probes`, body at
`probes.rs:1331`) and sets `PR_SET_NO_NEW_PRIVS` in a child
(`probes.rs:1287`). The Landlock and seccomp sweeps are T-0903
and T-0902. Landlock domains stack, so a payload can add a
ruleset over the host's, but whether the target host's outer
policy allows a nested ruleset is not measured. The issue
names the podcrates crate `pod-confine` (wave M7).

## Approach

Before the exec, set `PR_SET_NO_NEW_PRIVS`, add a Landlock
ruleset from the profile's read, write and execute lists and
TCP ports (ABI 4 or later for the network rules), load a
seccomp deny filter for namespace, mount, ptrace, keyctl, bpf
and similar calls, build the environment, and set rlimits.
Report the Landlock ABI and the nested-ruleset answer in
`podvm probe`. Place `/workspace` through the path map, where
landrun could not (F30). Ship the bailey policy of issue 170
as a built-in `errand` profile.

## Decision

Where Landlock is absent, a profile that asks for it refuses by
name; it never runs unconfined. The default-route trap (F2)
and the trust-parent trap (F3) are causes to probe and name
before the start. Build on `pod-confine` once podcrates
releases it.

## Prove

`cargo test -p podvm-enter` and `cargo test -p podvm-probe`
exit 0 with tests for the ruleset builder and the probe's ABI
answer. `sh experiments/414-confine.sh` exits 0 on a host that
grants Landlock: a profile with `fs.read` and `net.ports =
[443]` makes `cat /etc/hostname` fail with `EACCES`, a connect
to port 22 fail with `EACCES` and port 443 pass;
`/proc/self/status` shows `NoNewPrivs 1` and `Seccomp 2`; with
Landlock absent the profile refuses by name.

## Correction

**Filed 2026-11-06.** Quoted from issue 170:

```text
landrun: --best-effort, --rox system paths, --ro /etc files, --rwx project,
      --rw state, --connect-tcp PORT per net.ports
F1: bailey v0.2.4 musl cannot start where mounts are relatime in a user
      namespace (MS_STRICTATIME, EPERM): nix 0.29 drops ST_RELATIME on musl
F2: bailey's brokered modes refuse without an IPv4 default route
F3: bailey refuses a policy under a group-writable parent with no sticky bit
F4: /usr/bin/cc points into /etc/alternatives, which the read list omitted
F30: landrun: `ls /` and /etc/hostname give EACCES; /tmp is not writable;
      no TCP unless net.ports
L4: bailey's port rule refuses bind() to a source port outside net.ports
reference compare: no capabilities, NoNewPrivs 1, seccomp mode 2, every
      namespace and mount call refused; 382 same, 9 differ
```

**Amended 2026-11-09.** The ask A-V-01 of PodUtils/podcrates#39 (COORD-015,
a message from a peer session) wants one profile to build the confine layer:
Landlock, a seccomp deny filter, `PR_SET_NO_NEW_PRIVS` and rlimits. The
Approach holds each part. Two clauses are new. The rlimit part reads the
same profile as T-1655, so the limits have one source. A refusal for a
missing Landlock names the missing facility: Landlock itself, or the ABI
level that a network rule needs. The library is `pod-confine` (podcrates
podcrates@T-108). The peer asks for P0. The priority stays P2, because nothing in
podvm is wrong without the layer (the priority rules of `TODO/INDEX.md`).
The owner can change it.

**Amended 2026-11-10, for podrunner.** The podrunner design review (the
owner's design review of PodUtils/podcrates#47; the ruling "podrunner",
2026-11-10; SPEC sections 4.2, 6.5, 10.1 and its question DR-4) puts a trusted
job only on a unit whose boundary is at least `confined`, unless the owner
accepts `redirect`; no podvm rung is `confined` today. This layer is the
route by which a podvm rung can be `confined` on a host with no namespaces
(ask 2.7 of COORD-021, issue 227, names `pod-confine`, podcrates podcrates@T-108).
Two clauses are new. First, the report of T-1695 and `podvm probe --json`
state whether the layer was applied, with the Landlock ABI, the seccomp
mode and `NoNewPrivs`, so that a caller reads it and does not infer it.
Second, the experiment adds a canary file outside `fs.read` that the
payload cannot read, a host loopback TCP listener that it cannot reach
(Landlock ABI 4 or later), and records whether a host Unix socket is
reachable.

**Blocked 2026-11-11 on podcrates.** The confine step must run in the
payload's child after the root change, the `chdir`, the rlimits and
`no_new_privs`, and before the exec. That child is `pod-enter`'s, which
podvm takes frozen, and its `Plan` has no step for a ruleset or a filter.
COORD-040 (issue 259) asks podcrates for `pod-confine` and for
`Plan.confine`, as the Decision says. Measured here: the cloud host
(Linux 6.18) answers Landlock ABI 7. The condition that clears the block:
a podcrates tag with `pod-confine` and `Plan.confine`.

**Unblocked 2026-11-11.** podcrates `v0.23.0` has `pod-confine` and
`Plan.confine` (the answer to COORD-040, PodUtils/podcrates#69), and podvm
takes `v0.24.0` (T-1679). Each `Plan` of podvm names `confine: None` until
this entry sets it from the profile.

## Done

**Done 2026-11-12.** podvm takes podcrates `v0.27.0`: its podcrates@T-227 opens
the path of each rule of `Plan.confine` in the child after the mounts, so
a rule on `/tmp` covers the tmpfs of the namespace rung (COORD-048, issue
267). `sh experiments/414-confine.sh` exits 0 on the namespace rung with
each clause; clause 6 (the profile `errand`) writes `/tmp` (`tmp=0`)
and is denied `/etc/shadow`. With the dispatcher families of the same
day (clause 4), the layer runs on each rung with a chroot and on each
family of the userland rung.

**Partial 2026-11-11.** `run --podvm-confine=SPEC` applies the layer of
`pod-confine` through `Plan.confine` of `pod-enter`: SPEC is
`read=P,...;write=P,...;exec=P,...;connect=PORT,...` with guest paths
(`crates/podvm-enter/src/confine.rs`). The spec adds read and execute of
the system directories of the image that exist (`/bin`, `/sbin`, `/usr`,
`/lib`, `/lib64`, `/lib32`, podvm's `/.podvm`), read of `/proc` and three
device nodes, and read and write of `/dev/null`; `/etc` is not among them.
A profile takes `confine = true` with `fs.read`, `fs.write` and
`net.ports`. The layer runs on a rung with a chroot (namespace, chroot),
for a foreground run, and is decided after the fixups, so a node that the
completion layer made gets its rule. Each other case refuses by name with
exit 125 before anything runs: a rung with no chroot, `run -d`, a kernel
with no Landlock, an ABI under 3, and `connect` under ABI 4
(`crates/podvm/src/lifecycle/confine.rs`, the test
`each_case_with_no_mechanism_refuses_by_name`). The probe of
`probe-inside` reads no_new_privs and the seccomp mode with prctl(2), so
they hold where no `/proc` is mounted (`crates/podvm-probe/src/inside/checks.rs`).

`sh experiments/414-confine.sh` exits 0 on the cloud host (Linux 6.18,
Landlock ABI 7, the namespace rung): `/etc/passwd` reads, `/etc/shadow`
and a write to `/tmp` fail with "Permission denied", a TCP connect to the
allowed host listener answers and the other fails; the control without
the layer reads `/etc/shadow` and reaches both ports; `probe-inside` reads
NoNewPrivs 1 and Seccomp 2 under the layer and 0 and 0 without it; the
userland rung and `run -d` refuse by name. `cargo test -p podvm-enter` and
`cargo test -p podvm` exit 0 with the tests of the spec, the profile key
and the refusals. Two readings of the Prove changed: busybox:musl has no
`/etc/hostname`, so the outside read is `/etc/shadow`; the network of the
cloud host is a proxy only, so the TCP clause uses two listeners of the
host loopback in place of ports 22 and 443.

podvm takes podcrates `v0.25.0` (COORD-051, PodUtils/podcrates#76): the
layer now closes the Landlock scopes of ABI 6, denies MPTCP under a TCP
list and io_uring, and `apply` drops each capability outside docker's
default set (`Spec::new()`), so a confined root keeps no `CAP_SYS_ADMIN`
with which it read the environment of its unconfined ancestors past the
domain. Experiment 414 exits 0 again at that tag; `cargo test --workspace`
exits 0.

`run -d` and `exec` take the layer too (later the same day): the record
keeps the SPEC (`Hardening.confine` of
`crates/podvm-supervise/src/table.rs`), the launcher of `run -d` builds the
spec against the container's tree, and `exec` applies it on a rung with a
chroot and refuses it by name elsewhere. Experiment 414 clause 5 (exit 0):
the detached payload and an `exec` into it both fail to read
`/etc/shadow`, and the `exec` reads `/etc/passwd`.

The built-in profile `errand` carries bailey's policy of issue 170:
`confine = true`, bailey's read list (the ld.so files, the CA stores,
`nsswitch.conf`, `services`, `protocols`, `/etc/alternatives` of F4,
`passwd`, `group`) and write of `/tmp`, `/dev/full` and `/dev/tty`. Each is
written `?/P`, a new form of the SPEC: an optional path gets no rule where
the image lacks it, so one profile serves many images. Experiment 414
clause 6 (exit 0): NoNewPrivs 1, Seccomp 2, `unshare-user`,
`mount-tmpfs`, `bpf` and `keyctl` refused, `/` unlistable, `/etc/shadow`
denied, `/etc/passwd` read, the verdicts of bailey's reference report.
Found: on the namespace rung the payload cannot write `/tmp`, because the
child mounts a tmpfs there after `pod-enter` opens the rules; COORD-048
(issue 267) asks podcrates to open the rules in the child.

The loader family of the userland rung takes the layer too: `pod-enter`
has no chroot there, so each guest path of the spec becomes its host path
through the table of the interposer (the rootfs, and the volume of a `-v`;
`host_spec` of `crates/podvm-enter/src/confine.rs`), and the host's
`/proc` and device nodes join it. The launcher of `run -d` builds the same
host spec from the record's table. Experiment 414 clause 4 (exit 0, the
chroot-denying fixture): alpine on the loader family reads `/etc/passwd`,
is denied `/etc/shadow` and `/tmp`, and reads and writes a `-v` volume at
`/workspace`; by hand, `run -d` and an `exec` into it hold the same.

The dispatcher families take the layer too. On the native-static rung and
the syscall tier the payload's process runs podvm's own dispatcher, and on
the emulate tier a host qemu: the host spec gives each of those host paths
read and execute (`decide` of `crates/podvm/src/lifecycle/confine.rs`).
Experiment 414 clause 4 (the chroot-denying fixture): the static busybox
on the native-static rung and on the syscall tier, and the arm64 busybox
on the emulate tier, read `/etc/passwd` and are denied `/etc/shadow`.

The report of
T-1695 (`run --report`) names the confine SPEC of the run. The ABI is in `podvm probe` already (the
probe `landlock_create_ruleset(VERSION)`), and `podvm doctor` names what
the layer does with it: under ABI 3 it is MISSING, under ABI 4 it runs
with no TCP rule (`crates/podvm/src/doctor/scope.rs`).


# T-1688: The memfd rung measures whether a memfd executes, names `vm.memfd_noexec`, and refuses before staging

**Source:** issue 212; T-1667 (the doctor row shape); T-1003 (the fd exec)
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

`kernel_takes_memfd()` proves that `memfd_create` answers, not
that the descriptor executes. Under `vm.memfd_noexec=1` the
ladder promises the memfd rung and the launch fails at step 5,
"execveat of the memfd". Under `vm.memfd_noexec=2` the entry
refuses with a text that does not name the sysctl. `podvm
doctor` has no row for it.

## Premise

The probe is `src/memfd.rs` of `pod-enter` (PodUtils/podcrates): it creates
and closes a memfd and never executes one. The callers are
`crates/podvm/src/ladder.rs:202` and
`crates/podvm/src/lifecycle/entry.rs:356`. The late step is
`src/launch.rs` of `pod-enter` (PodUtils/podcrates). The census row
`memfd_create+exec` of `pod_hostprobe::probes` runs a shell
script through `/proc/self/fd/N`, not `execveat` of a static
payload, and podvm does not read it. Under value 2, `MFD_EXEC`
is denied too, so "pass `MFD_EXEC`" (the issue) fixes value 1
only.

## Approach

Add one probe in `podvm-enter`: create a memfd, write a small
static trampoline, fork, `execveat` it, read the exit status.
It returns `Exec`, `NoExec` (with the sysctl value when
`/proc/sys/vm/memfd_noexec` is readable) or `NoMemfd`. The
doctor row `memfd_exec`, the ladder and the entry check read
this one probe. The refusal comes before staging and names the
sysctl, with a fix line for each value: 1 allows `MFD_EXEC`; 2
needs the sysctl set to 0 or another rung.

## Decision

The doctor row and the refusal read one probe, so they cannot
disagree. A denial with no readable sysctl reads as "denied,
cause unknown", not as the sysctl.

## Prove

`cargo test -p podvm-enter -p podvm` exits 0 with tests for the
pure mapping (measured result and sysctl value to the answer
and the fix line), and `podvm doctor` prints the row
`memfd_exec` on the cloud host.

## Correction

**Filed 2026-11-09.** From issue 212 (the owner). The triage of
2026-11-09 confirmed each claim at `2be31b2` and corrected two: the census
row exists in `pod-hostprobe` but tests another exec path, and `MFD_EXEC`
does not help under value 2.

## Done

**Done 2026-11-09.** `memfd::probe::memfd_exec` stages a static
132-byte trampoline (x86_64 and aarch64) the way the launch stages a
payload, executes it through `execveat(AT_EMPTY_PATH)` in a forked child,
and answers `Exec`, `NoExec`, `NoMemfd` or `Unmeasured`, each with the
value of `vm.memfd_noexec`. The ladder, the entry check and the new doctor
row `memfd_exec` read this one probe, and `kernel_takes_memfd` is gone.
`memfd_create` passes `MFD_EXEC`, so value 1 no longer blocks the rung;
under value 2 the refusal comes before staging and names the sysctl. The
pure function `probe::answer` gives the text and the fix line. `cargo
test -p podvm-enter -p podvm` exits 0 (404 passed), with
`the_mapping_names_the_sysctl_only_where_it_is_the_cause` and
`a_memfd_sealed_against_exec_reads_as_no_exec`. [The drive](../experiments/results/memfd-exec.txt):
`podvm doctor` prints `ok memfd_exec` at value 0 and 1, and the note with
the fix at value 2, each value set in a child pid namespace. Limits: the
forced-memfd refusal was not driven end to end (no static image in the
store), and the aarch64 trampoline was assembled and compiled, not run.

# T-1700: A run on the userland rung sometimes exits 0 with no output

**Source:** T-1653 (its drive of 2026-11-10)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

On the userland rung (chroot denied), `podvm run --rm alpine
cat /etc/alpine-release` sometimes exits 0 and prints
nothing. It lost its output in 1 of 20 runs on `e99fc9b`
and in 2 of 15 runs after T-1653. The chroot rung (0 of 20)
and the deny-chroot fixture alone (0 of 60) never lost it.
A run that loses its output and exits 0 is a wrong answer
with no error.

## Premise

The defect is older than T-1653. Experiment 356 saw the same
shape once before (`fx-gc-script` exited 0 with no output,
recorded with T-1650). The cause is not measured.

## Approach

Reproduce in a loop on the fixture, then trace the payload's
stdout from the loader family's launch to the caller: the
descriptor podvm gives the payload, a close or a flush before
exit, and the exit path of the launcher. Fix the cause, not
the symptom.

## Decision

A lost output is a defect even when the exit code is 0.

## Prove

A loop of 200 runs of `cat /etc/alpine-release` on the
chroot-denying fixture prints the release each time, and
`cargo test --workspace` exits 0 with a test of the cause.

## Correction

**Filed 2026-11-10.** Found by the drive of T-1653.

## Done

**Done 2026-11-10.** The cause is podvm's own `untee`
(`crates/podvm/src/run/foreground.rs`), and it touches every foreground
run on every rung, not only the userland rung. `untee` closed the saved
stdout before it joined the pump thread. When the payload wrote and exited
just before the restore, the pump wrote the bytes after the close: the
write to stdout failed with EBADF, the bytes went to the container log
only, and podvm exited 0. A trace of the losing runs (strace
`--seccomp-bpf`, write and sendfile only) shows the payload's `sendfile`,
its exit, then `write(8, ...) = -1 EBADF` and `write(9, ...) = 8`. The fix
restores fd 1, joins the pump, then closes the saved descriptor. The test
`untee_delivers_every_byte_before_the_saved_descriptor_closes` fails 20 of
20 times with the old order and passes 20 of 20 with the fix; `cargo test
--workspace` exits 0. [The loop](../experiments/results/lost-output-1700.txt)
lost 1 to 12 runs in 150 before (the userland rung with stdout a file, a
pipe or a tty, the memfd family, the namespace rung) and none after (200
of 200 on the fixture).

# T-1707: A relative first program resolves against the image root, not against `-w`

**Source:** issue 232; the podrunner design review (the owner's design
  review of PodUtils/podcrates#47), its measurement M2
  (`03-runner-payload.log`); `src/ladder.rs` of `pod-enter`
  (PodUtils/podcrates); `src/userland.rs` of `pod-enter`
  (PodUtils/podcrates); T-1653 (the table);
  `crates/podvm/src/parity.rs:211`
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`podvm run -w /opt/runner IMAGE ./bin/Runner.Listener` exits
127 with "./bin/Runner.Listener names no file in the image".
docker runs `/opt/runner/bin/Runner.Listener`: the kernel
resolves a relative path against the working directory. An
image with a `WORKDIR` and a relative command fails the same
way. The runner starts as `./run.sh` from its root.

## Premise

`resolve_payload` in `src/ladder.rs` of `pod-enter` v0.12.0
(line 147) resolves an argv0 that holds a `/` from the image
root. `src/userland.rs` of `pod-enter` v0.12.0 (line 128) has
the same shape. The `-w` row says "chdir inside the new root"
(`crates/podvm/src/parity.rs:211`), so the working directory
exists before the exec. M2 observed exit 127, 3 of 3.

## Approach

Join a relative argv0 that holds a `/` to the working
directory before the resolution, on each rung and through
the T-1653 table, so that a workdir in a volume works too.
Send the change to podcrates as a COORD issue (`AGENTS.md`,
section 9, item 4), take the release, and add the podvm test.

## Decision

docker's rule: a relative program is relative to the working
directory. A bare name still searches `PATH`.

## Prove

`cargo test -p podvm` exits 0 with a test that `-w /opt/x
./prog` resolves to `/opt/x/prog`.
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh`
exits 0 with a new clause: `-w /bin ./busybox true` exits 0
on the userland rung, and a relative program in a volume
workdir runs.

## Correction

**Filed 2026-11-10.** From issue 232 and the podrunner design review
(the ruling "podrunner", 2026-11-10), M2 surprise 9: "the first program
of `run` is resolved against the image root, not `-w`". docker's `-w`
with a relative program does not work, so the entry is P1.

## Done

**Done 2026-11-11.** podvm needs no change of `pod-enter`: each lookup of
the first program is a call of podvm. `podvm_enter::program_in`
(`crates/podvm-enter/src/lib.rs`) joins a relative program that holds a
`/` to the working directory; a bare name still searches `PATH`, and the
payload's `argv[0]` keeps the caller's spelling. The joined name goes to
`decide_entry` and `script::follow` (resolution, head, `invocation_path`,
`interpose::classify`), to `interpose::apply` (the preload), to
`push_guest_exe` (`/proc/self/exe`), to the memfd stage of a userland run,
and to a forced ladder rung, in `run` and in both paths of `exec`. The
chroot and namespace rungs exec after `chdir`, as the kernel resolves it.
A first drive found that `interpose::apply` still took the raw `argv[0]`:
the payload ran with no preload, and its children ran the host's
binaries; the clause of experiment 356 now runs a child that sees the
image only through the interposer, not `./busybox true`.

Proof: `cargo test -p podvm` exits 0 with
`a_relative_program_resolves_against_the_working_directory`.
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh` exits 0 with
clause 31 ([the run](../experiments/results/no-chroot-rung.txt)):
`-w /bin ./busybox sh -c 'ls /etc/alpine-release'` lists the image's
file, and `-w /data ./tool356.sh` runs a script of a volume workdir.
Experiment 428 runs `-w /opt/runner ./bin/Runner.Listener --version`.

# T-1716: Take the `pod-enter` change of podcrates podcrates@T-155: `Maps::new` takes `&Names` and names the volume of a missing `-w`

**Source:** PodUtils/podcrates#52 (COORD-027, the answer to issue 226);
  podcrates podcrates@T-155 (commit d8b77b5, no tag on 2026-11-10);
  PodUtils/podcrates#53 (COORD-028: `v0.15.0` carries it);
  `crates/podvm-enter/src/lib.rs:26`,
  `crates/podvm-enter/src/lib.rs:172`,
  `crates/podvm-enter/src/lib.rs:177`,
  `crates/podvm-enter/src/lib.rs:188`
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

podvm keeps its own volume rules in `podvm-enter`
(`volume_maps`, `MAX_VOLUMES`, `NEVER_MAPPED`), while
`pod-enter` now refuses a reserved guest path and an
oversize volume count itself, and names the volume of a
missing `-w` directory. Two copies of one rule can drift.

## Premise

COORD-027 gives the change: `Maps::new(&names, rootfs,
&volumes)` takes `&Names` first; `Names::reserved` is new;
`GuestRoot::volume_label` is new; `check_workdir` names the
volume `HOST:GUEST` of a missing `-w`. podvm uses podcrates
`v0.12.0` (5e5629a); `v0.14.0` is the last tag on
2026-11-10 and needs no podvm change (podcrates#50).

## Approach

At the podcrates tag that carries d8b77b5 (planned
`v0.15.0`): set `NAMES.reserved` to `["/proc", "/.podvm"]`
and `NAMES.product` to `podvm`, call `Maps::new(&NAMES,
rootfs, &volumes)`, drop `volume_maps`, `MAX_VOLUMES`,
`NEVER_MAPPED` and `under`, and keep `volumes_of`. Follow
the adoption map of the `pod-enter` README. Move each pin of
the 11 crates to that tag in one commit.

## Decision

The volume rules live in `pod-enter` only; podvm fills the
names.

## Prove

`cargo test --workspace` exits 0 with a test that a `-v`
pair to `/proc` is refused by name and that a missing `-w`
inside a volume names the volume; `rg -n "NEVER_MAPPED|
MAX_VOLUMES" crates/` prints nothing.

## Correction

**Blocked 2026-11-10.** Blocker: no podcrates tag carries d8b77b5.
Clears when: a podcrates tag (planned `v0.15.0`) carries podcrates podcrates@T-155.

**Unblocked 2026-11-11.** podcrates `v0.15.0` (`27a4ccd`) carries podcrates@T-155
(PodUtils/podcrates#54, COORD-029).

**Filed 2026-11-10.** From COORD-027 (PodUtils/podcrates#52), the answer
of podcrates to the two asks of issue 226 (T-1653).

## Done

**Done 2026-11-11.** podvm takes the 11 crates of PodUtils/podcrates at
`v0.15.0` (`27a4ccd`). Each pin of `crates/*/Cargo.toml` moves from
`v0.12.0` in one commit, and `Cargo.lock` changes only the version and
the git revision of each `pod-*` package (no new package). `NAMES.reserved` is `["/proc", "/.podvm"]`;
`NAMES.product` was `podvm` before. Each caller builds the table with
`Maps::new(&NAMES, rootfs, &volumes)` (`run/parse.rs`,
`lifecycle/volume.rs`, the launcher of `podvm-supervise`, and the tests),
and `volume_maps`, `MAX_VOLUMES`, `NEVER_MAPPED` and `under` are gone;
`volumes_of` stays. podvm had no volume rule for `-w` left in its own code
since T-1653 (the take of `v0.12.0`), so `check_workdir` alone names the
volume now. The Prove, on the cloud host, 2026-11-11: `cargo test
--workspace` exits 0 (650 tests in 53 suites, 0 failed), with
`a_volume_the_table_cannot_spell_is_refused` (a `-v` pair to
`/proc/self` and to `/.podvm` is refused by name: "podvm never rewrites a
path under /proc") and `a_guest_path_in_a_volume_resolves_in_the_volume`
(a missing `-w /data/none` inside a volume says "in the volume
HOST:/data is not a directory in the volume"; a missing `-w /none` still
names the image). `rg -n "NEVER_MAPPED|MAX_VOLUMES" crates/` prints
nothing (exit 1). `cargo clippy --workspace --all-targets -- -D
warnings` and `cargo fmt --all --check` exit 0.

# T-1719: The loader's library directories leave out `/usr/local/lib` and the image's `ld.so.conf`

**Source:** the work of T-1705, 2026-11-11; `src/userland.rs` of
  `pod-enter` (PodUtils/podcrates), `lib_dirs`
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

On the userland rung podvm starts the image's loader with
`--library-path` and the standard library directories of the
image. A library that the image keeps in `/usr/local/lib`, or
in a directory that only its `/etc/ld.so.conf` names, is not
found by that path. The loader runs on the host, so it reads
the host's `/etc/ld.so.cache`, not the image's (Inferred, not
run).

## Premise

`lib_dirs` of `pod-enter` v0.15.0 pushes the loader's own
directory, then `/lib`, `/lib64`, `/usr/lib`, `/usr/lib64` and
the multiarch directories of five machines; it reads no
`ld.so.conf` and no `/usr/local/lib`. An image that builds a
program from source with `make install` and `ldconfig` keeps
its libraries in `/usr/local/lib`. A program with an
`$ORIGIN` run path is not affected.

## Approach

Measure first: an image with a library in `/usr/local/lib`
and an `ld.so.conf.d` entry for it, and a program with no run
path that links it, on the chroot-denied fixture of
experiment 356. If it fails, ask podcrates by a COORD issue to
add `/usr/local/lib`, `/usr/local/lib64` and each directory of
the image's `/etc/ld.so.conf` (with its `include` lines, read
through the table, bounded) to `lib_dirs`, and take the tag.

## Decision

The loader searches the directories that the image's own
`ldconfig` would search; the host's cache plays no part.

## Prove

`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh`
exits 0 with a clause: the program of that image runs at 0.

## Correction

**Filed 2026-11-11.** Found during T-1705, which put its test library in
`/usr/lib/x86_64-linux-gnu` because `lib_dirs` has no `/usr/local/lib`.

**Measured 2026-11-11; blocked on podcrates.** An image of alpine 3.20
with `libt1719.so` in `/usr/local/lib`, and a dynamic musl program that
needs it with no run path (built with zig cc): on the chroot-denied
fixture of experiment 356 the run exits 127 with "Error loading shared
library libt1719.so: No such file or directory"; on the namespace rung
the same image prints `lib-42` at 0. With a file of that name in the
host's `/usr/local/lib` (by a mistake of the drive, removed after it),
the userland rung printed `lib-42`: the guest loader took the host's
library. Thus the premise holds, and a miss also reaches the host's
directories. COORD-029 (issue 247) asks podcrates for `/usr/local/lib`,
the image's `ld.so.conf` and `ld-musl-ARCH.path`, and `--inhibit-cache`
for glibc. The blocker clears when a podcrates tag carries it; then podvm
takes the tag and adds the clause of the Prove.

## Done

**Done 2026-11-11.** podvm takes the 11 crates of podcrates at `v0.19.0`
(`9a11bbd`, PodUtils/podcrates#62), which carries its podcrates@T-182 for COORD-029
(issue 247). `lib_dirs` of `pod-enter` now gives `/usr/local/lib`,
`/usr/local/lib64` and their multiarch forms, then each directory of the
image's `/etc/ld.so.conf` (with `include`) or `/etc/ld-musl-ARCH.path`,
each resolved through the table; `LoaderPlan::inhibit_cache` is true for
a glibc loader of 2.16 or later. podvm gives that loader
`--inhibit-cache` (`crates/podvm/src/lifecycle/entry.rs`), and the
interposer gives it to the loader of each glibc child
(`loader_takes_inhibit_cache` in
`crates/podvm-interpose/src/exec/head.rs`), so the host's `ld.so.cache`
plays no part. A library that no directory of the image holds is still
searched in the loader's built-in host directories.

Proof: `PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh` exits 0
with clause 32 ([the run](../experiments/results/no-chroot-rung.txt)): on
the fixture, a musl program of alpine with its library in
`/usr/local/lib`, and a glibc program of debian with its library in a
directory that only `/etc/ld.so.conf.d` names, each print `lib-42`. The
interposer tests pass (83), with
`a_glibc_loader_from_2_16_takes_inhibit_cache`.

# T-1724: A static child of the loader family, and a volume, on the native-static rung

**Source:** T-1652 (measured on the cloud host, 2026-11-11);
  `crates/podvm-interpose/src/exec/launch.rs` (`Tail::Direct`);
  `crates/podvm/src/lifecycle/native.rs`
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

A dynamic shell of the golang image on the loader family ran
`go build` as its child: the interposer executes a static
child directly (`Tail::Direct`), so go ran with the host's
root and wrote `/tmp/g/g` on the host (measured, 2026-11-11).
The capability gap names it ("a statically linked child keeps
the direct exec"). The native-static rung also refuses a run
with `-v`: its handler has no table of volumes.

## Premise

T-1652 gives `podvm --podvm-dispatch-enter ROOT EXE ARGV...`,
which enters a static program in the root. The interposer
reads its table (`PODVM_MAPS`) and already knows the guest
path of each child (`PODVM_GUEST_EXE`).

## Approach

Give the interposer the host path of podvm (a variable of the
loader family's environment). For a static ELF child of the
machine, the exec route runs podvm with `--podvm-dispatch-enter`,
the rootfs and the guest path, in place of the direct exec. Give
the handler of `podvm-dispatch` the volumes of the run: a guest
prefix of a volume resolves in the volume's host directory, with
`RESOLVE_IN_ROOT` at that directory.

## Decision

A static child keeps the direct exec only where the native-static
rung cannot arm, and the capability gap says so then.

## Prove

`RUSTFLAGS="-C target-feature=-crt-static" cargo test --manifest-path crates/podvm-interpose/Cargo.toml --target x86_64-unknown-linux-gnu -- --test-threads=1`
exits 0 with a test of the route of a static child, and
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh`
exits 0 with a clause where a dynamic shell of the golang image
runs `go build`, the output is in the rootfs and not on the host,
and a clause where a static payload reads a file of a `-v`
volume on the native-static rung.

## Correction

**Filed 2026-11-11.** From the measurement of T-1652.

## Done

**Done 2026-11-11.** The interposer's tests exit 0 (85) with
`a_static_child_enters_the_native_static_rung_through_podvm`, and
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh` exits 0 on the cloud
host ([the run](../experiments/results/no-chroot-rung.txt), verdict "NO-CHROOT FAMILY RUNS"): in clause 33 the golang
image's `sh` runs `go build` and the program it built prints `g724-ok`, with
no `/tmp/g724` and no `/go` on the host; a static payload on the
native-static rung reads and writes a file of `-v HOST:/data` (the write lands
in HOST), and `-w /data` reads back as `/data`.

**Partial, 2026-11-11.** The static child: the interposer reads a static
ELF of the machine as `File::Static`, and where podvm names itself in
`PODVM_DISPATCH_SELF` (`podvm_dispatch::SELF_VAR`, set by podvm where Syscall
User Dispatch arms) the exec route runs `podvm --podvm-dispatch-enter ROOTFS
GUEST ARGV...` (`Tail::Dispatch` in `crates/podvm-interpose/src/exec/launch.rs`);
a `#!` script with a static interpreter takes the same route with the
kernel's arguments. The interposer's tests exit 0 (84, with the new test of
the route). On the chroot-denied fixture of experiment 356, `sh -c 'go build
-o h main.go && ./h'` in the golang image printed `g724-ok`, and nothing
landed on the host (no `/tmp/g724`, no `/go`). The `--strict` gap of the loader family names
this entry now.

The volume, the same day: the handler reads the volume pairs of
`PODVM_MAPS` at the entry, opens each host directory, and resolves a guest
path under a volume with `openat2(VOLFD, REST, RESOLVE_IN_ROOT)`
(`path::base_of`, `path::open_in`); a host path in a volume reads back as
its guest path (`path::guest_of_host`, for `getcwd` and `/proc/self/fd`).
`refuse_unserved` lets a volume through on the rung; `:ro` and
`--read-only` stay refused by name (T-1655). On the fixture, a static probe
read and wrote a file of `-v HOST:/data`, the write landed in HOST, and
`-w /data` read back as `/data`. Remaining: the run of experiment 356 with
clause 33.

# T-1729: An optional `runsc` backend where a host grants user namespaces

**Source:** T-1657 (its Approach, the part that it did not take)
**Category:** enter
**Milestone:** none
**Priority:** P3
**Effort:** M
**Status:** done

## Problem

gVisor's `runsc` answers a payload's system calls in its own
kernel. podvm cannot offer it on the target host, which denies
user namespaces, mount and ptrace. A host that grants user
namespaces could run it from the namespace rung.

## Premise

Issue 169 quotes `runsc --rootless --network=none|host
--ignore-cgroups --platform=systrap run`, and F14: `runsc do
-volume` wrote into gVisor's overlay, and rootless `create` is
refused. The syscall tier of T-1657 gives the view on the
target host.

## Approach

Where the probe reports user namespaces and a `runsc` on
`PATH`, `--podvm-tier=runsc` builds an OCI bundle from the
container's own tree (T-1709) and runs it, with `/workspace`
as a bind mount of the bundle (F14).

## Decision

Optional, and never a fallback of the syscall tier: a caller
asks for it by name.

## Prove

`sh experiments/436-runsc.sh` exits 0 on a host with user namespaces and `runsc`: `uname -r` names gVisor, a write to `/workspace` is on the host after the run, and on a host without them the tier refuses by name

## Correction

**Filed 2026-11-11.** Split from T-1657. Measured 2026-11-12: rootless
`runsc create` is not needed; `runsc --rootless run` of a bundle works,
and a bind mount of the bundle writes on the host. gVisor 20261102 needs
its sidecars of `gvisor-bin/` beside `runsc`.

## Done

**Done 2026-11-12.** `crates/podvm/src/runsc.rs` and
`crates/podvm/src/run/runsc.rs`: `--podvm-tier=runsc` checks for `runsc`
(`PODVM_RUNSC`, else `PATH`) and for user namespaces (`unshare` in a
child) before the pull, copies the image into the container's own tree,
writes the bundle (each `-v` a bind mount, `:ro` and `--read-only`
read-only, a numeric `--user`), and starts `runsc --rootless
--network=host --ignore-cgroups --platform=systrap --overlay2=none run`.
The bundle, the tree and the state of the container go at the end. `sh
experiments/436-runsc.sh` exits 0 on this host (Linux 6.18, gVisor
20261102 by its SHA-512): `uname -r` gives `4.19.0-gvisor`, the exit code
3 of the payload is the run's, a write with a line of stdin is on the
host, `--read-only` and `:ro` give EROFS, nothing of the run stays, and
the tier refuses at 125 by name with no `runsc` and where `unshare(2)` is
denied ([the run](../experiments/results/runsc.txt)). The tests of
`runsc_tests.rs` hold the bundle and the arguments.

# T-1732: Take podcrates v0.22.0: a detached payload ends with its launcher

**Source:** PodUtils/podcrates#71 (COORD-046, a message from a peer
  session); podcrates podcrates@T-205 and podcrates@T-202; podrunner's test 26;
  `crates/podvm-supervise/src/launcher.rs:78`
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

A payload of `podvm run -d` outlived a launcher that was
killed. The container then ran with no supervisor: no wall
bound, no log, no exit code.

## Premise

podcrates `v0.22.0` (`93de04a`) gives `Plan.die_with_parent`:
the child sets `PR_SET_PDEATHSIG(SIGKILL)` first after the
fork, and ends where its parent ended before the call. The
signal fires at the exit of the thread that forked, so it is
opt-in. The launcher forks from its one thread.

## Approach

Move each pin of `crates/*/Cargo.toml` from `v0.21.0` to
`v0.22.0` in one commit. Set `die_with_parent: true` in the
plan of the launcher, and `false` at each other site, where
the behaviour does not change.

## Decision

Only the launcher opts in. A foreground run keeps its
behaviour: a docker client that is killed does not stop its
container either.

## Prove

`sh experiments/437-payload-dies-with-launcher.sh` exits 0:
after SIGKILL to the launcher of `podvm run -d
public.ecr.aws/docker/library/busybox:musl sleep N`, the
payload is gone or a zombie within 5 s, and `ps -a` names
the container Dead.

## Done

**Done 2026-11-11.** The 9 manifests name `v0.22.0`, and `Cargo.lock`
changes only the version and the git revision of each `pod-*` package.
`PODVM_437_IMAGE=mirror.gcr.io/library/busybox:musl sh
experiments/437-payload-dies-with-launcher.sh` exits 0 on the cloud host
(`experiments/results/payload-dies-with-launcher.txt`): the payload is a
child of the launcher, it is a zombie (state Z) right after the launcher's
SIGKILL, and `ps -a` says "Dead (exited while the launcher was not
watching)". The control, the same build with `die_with_parent: false` in
the launcher, leaves the payload alive (state S), and the experiment
exits 1.

# T-1731: A dynamic program on the native-static rung: its loader under the handler

**Source:** T-1657 (the syscall tier refuses a dynamic program); the `--strict` gap of the loader family (T-1650)
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

The native-static rung and the syscall tier run a static
program only: an `execve` of a dynamic one fails with
`ENOEXEC`, and `--podvm-tier=syscall` refuses it. A dynamic
program runs on the loader family instead, where a raw system
call (Go with cgo, liburing) passes the interposer and its
absolute path resolves on the host. That is the gap that
`--strict` names for the loader family.

## Premise

The loader of `podvm-dispatch` maps a static ELF
(`crates/podvm-dispatch/src/elf.rs`). A dynamic ELF names an
interpreter (`PT_INTERP`), which is itself a static program of
the image (`ld-musl-*.so.1`, `ld-linux-*.so.2`).

## Approach

Map the interpreter of the image beside the program, give the
auxiliary vector its `AT_BASE`, `AT_PHDR`, `AT_ENTRY` and
`AT_EXECFD` values, and start at the interpreter's entry. Each
library that it opens resolves through the handler, so each
path of the program and of its libraries is in the image.
Let the syscall tier take a dynamic program then.

## Decision

The interposer is not needed on this rung: the handler sees
each call, raw or not.

## Prove

`cargo test -p podvm-dispatch` exits 0 with a test that runs a dynamic program of a root under the handler; `sh experiments/413-syscall-tier.sh` exits 0 with a clause where a dynamic program of an image runs on the syscall tier and a raw `openat` of an absolute path reads the image's file

## Correction

**Filed 2026-11-11.** Split from T-1657, so that the `--strict` gap of the
loader family names an entry that is not done.

## Done

**Done 2026-11-12.** `elf::load` of `crates/podvm-dispatch/src/elf.rs`
maps a dynamic program, then reads its `PT_INTERP`, opens that path in
the root, and maps the interpreter (a shared object with no interpreter of
its own) as a second image; `enter.rs` gives `AT_BASE` the interpreter's
load address, keeps `AT_ENTRY` at the program's entry, and starts at the
interpreter's entry. The check of an `execve` takes a dynamic program, and
`--podvm-tier=syscall` takes one (`podvm_dispatch::host_program`). Found
on the way: the payload of this rung inherited podvm's interposer in
`LD_PRELOAD`, which turned each guest path into its host path before the
handler saw it, so each `execve` of a dynamic shell failed with `ENOENT`;
the entry now drops that object from `LD_PRELOAD` (each other object
stays).

`cargo test -p podvm-dispatch` exits 0 with
`a_dynamic_program_of_the_root_runs_with_its_own_interpreter` (the
host's dynamic `cat`, its interpreter and its libraries in a root read
the root's marker) and
`the_interposer_leaves_ld_preload_and_each_other_object_stays`.
`PODVM_413_MIRROR=1 sh experiments/413-syscall-tier.sh` exits 0 with
clause 7: alpine's dynamic busybox prints `3.20.10`, and `rawcat`, a
dynamic glibc program of the image, reads the image's `/etc/os-release`
through a raw `openat` system call (`ID=podvm413`).

# T-1730: A detached thread of a static payload ends with SIGSEGV on the native-static rung

**Source:** Found by T-1666: the probe inside the payload died on signal 11 on the chroot-denying fixture
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

musl ends a detached thread with `__unmapself`: `munmap` of
the thread's own stack, then `exit`. On the native-static rung
the `munmap` traps into the SIGSYS handler, which runs on that
stack (a thread has no alternate stack). The handler unmapped
it, and its return faulted: the whole payload died with
SIGSEGV. Each static program that drops the handle of a thread
(Rust's `std::thread::spawn`, a DNS resolve with a bound) hits
it.

## Premise

Measured on 2026-11-11 with `strace -f`: the thread called
`munmap(0x7f61d59fd000, 2109440)` and took SIGSEGV at
`0x7f61d5bfec50`, in the range it had unmapped.

## Approach

Where a `munmap` takes the stack that the handler runs on, the
handler makes the `munmap` and the `exit` of the thread in one
routine of assembly that touches no stack
(`podvm_dispatch_unmap_exit`).

## Decision

A call that unmaps the handler's own stack cannot return, so
its only valid next call is the exit; musl makes it.

## Prove

`cargo test -p podvm-dispatch` exits 0 with `a_detached_thread_ends_and_the_process_goes_on`, and the test fails with the new arm taken out

## Correction

**Filed 2026-11-11.** Found by T-1666.

## Done

**Done 2026-11-11.** `cargo test -p podvm-dispatch` exits 0, 9 tests of
`tests/dispatch.rs`. The plant: with the arm of `MUNMAP` taken out
(`if false &&`), `a_detached_thread_ends_and_the_process_goes_on` failed;
with it back, it passed.

# T-1740: Raw system calls of a dynamic program on the loader family: measure the syscall tier, then choose the default

**Source:** T-1731 (a dynamic program on the native-static rung); the `--strict` gap of the loader family (T-1650)
**Category:** enter
**Milestone:** none
**Priority:** P3
**Effort:** M
**Status:** partial

## Problem

A dynamic program on the loader family can issue the `syscall`
instruction itself (Go with cgo, liburing). That call passes the
interposer, so its absolute path resolves on the host, and a write lands
on the host tree. The loader family is the default for a dynamic program
on a host that denies chroot.

## Premise

Since T-1731, the native-static rung and `--podvm-tier=syscall` run a
dynamic program with each of its system calls given to podvm's handler,
raw or not, where Syscall User Dispatch arms. Each call then costs a
signal (SIGSYS) and a return through the handler. Not measured: the cost
against the loader family for a build or a test run.

## Approach

Measure the wall time of one workload (a `go build`, an `npm test`) on
the loader family and on the syscall tier on one host. Make the syscall
tier the default for a dynamic program where the cost is small, or keep
the loader family and let `--strict` name `--podvm-tier=syscall` as the
rung that runs a payload that hits the gap.

## Decision

Until the measurement, the loader family stays the default, and the gap
names `--podvm-tier=syscall` as its remedy.

## Prove

`sh experiments/438-raw-syscall-cost.sh` exits 0 with the two wall times of each workload, and `cargo test -p podvm` exits 0 with the default that the measurement chose

## Correction

None.

## Done

**Partial 2026-11-12.** `PODVM_438_STORE=... sh
experiments/438-raw-syscall-cost.sh` exits 0 on the chroot-denying
fixture (4 CPUs, Linux 6.18), alpine 3.20, wall times in ms:

```text
workload  loader-family  syscall-tier
exec      1880           2392   (2000 runs of cat: +27 %)
io        631            609    (md5sum of the image, 20 times)
calls     311            667    (dd bs=1 of 200000 bytes: x2.1; the copy alone 0.12 s against 0.50 s)
```

Decision (the call of this session, AGENTS.md section 10): the loader
family stays the default for a dynamic program. A workload that is made
of system calls runs up to 4 times slower on the syscall tier, and the
tier changes what the payload sees (`/tmp` capped, `uname` fixed, no
`run -d`). The `--strict` gap names `--podvm-tier=syscall` as the rung
that runs a payload with raw system calls
(`crates/podvm/src/complete/capability.rs`).

Remaining: the gap stays open while the tier costs this much; a cheaper
mechanism (a filter that gives the handler only the calls with a path)
would let the tier be the default. Not measured: Go with cgo and
liburing themselves.


# T-1742: The loader family does not find the glibc of a Debian image on arm64

**Source:** issue 282 (COORD-061); T-1721 (experiment 356 on arm64,
  clauses 18, 19 and 33); pod-enter v0.30.0 `abi::libc_beside`
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

On arm64, the loader family declines each dynamic glibc program of a
Debian bookworm image (gcc, golang): "/lib/ld-linux-aarch64.so.1 names no
C library podvm can find in the image".

## Premise

`/lib` is a link to `usr/lib`, and `libc.so.6` is in
`/usr/lib/aarch64-linux-gnu/`. The search of pod-enter walks with a stack,
so it walks the last name of a directory first, and it stops after 512
entries. `x86_64-linux-gnu` sorts last and is found; `aarch64-linux-gnu`
sorts first and the search stops before it. Measured 2026-11-12 on a
synthetic tree: 800 entries in the other directories of `/usr/lib` give
`None`, 500 give the libc.

## Approach

pod-enter belongs to podcrates: a breadth-first walk that walks each real
directory one time (issue 282). podvm takes that tag.

## Decision

The fix belongs to pod-enter (issue 282); podvm keeps no second copy of
the search.

## Prove

`cargo test -p podvm` exits 0, and clauses 19 and 22 of experiment 356
exit 0 on arm64 (`.github/workflows/arm.yml`)

## Correction

**Unblocked 2026-11-12.** podvm takes podcrates `v0.31.0` (`45b145a`,
PodUtils/podcrates#90): `libc_beside` looks first at the multiarch places
of each library root, and walks a real directory once. Its test
`the_multiarch_libc_is_found_beside_large_trees_of_a_merged_usr` holds the
tree of issue 282 for aarch64 and x86_64. Remaining: the run of `arm.yml`.

## Done

**Done 2026-11-12.** `cargo test -p podvm` exits 0 on podcrates
`v0.31.0`, and `.github/workflows/arm.yml` run 43228497851 runs experiment
356 on arm64 with exit 0: clauses 19 and 22 (the gcc bookworm image, with
the glibc object) and the dladdr and library clauses of the debian image
pass; the loader family finds `/usr/lib/aarch64-linux-gnu/libc.so.6`.

# T-1749: A read-only image as the lower layer of the root, with a tmpfs upper layer, and no copy

**Source:** PodUtils/podcrates#124 (the asks of podswarm to podvm, a message from a
  peer session; PodUtils/podcrates#107 is its tracker), ask V-5
**Category:** enter
**Milestone:** none
**Priority:** P3
**Effort:** L
**Status:** partial

## Problem

`run` copies the image into a tree for each container. A podswarm task
for each attempt pays the copy each time.

## Premise

A user with no root has no overlay mount where namespaces are denied. On
the userland rung the path table of the interposer can map a path to a
read-only lower tree and a per-run upper tree; on the namespace rung an
overlay mount in the user namespace can do it (Linux 5.11 and later).

## Approach

Measure the cost of the copy first. Then, for each rung, choose the way:
an overlay mount in the user namespace where it mounts, else a copy-up
of the path table on the loader family. Each rung that cannot do it keeps
the copy and says so.

## Decision

Measure before the design: the entry can close with a measurement that
shows that the copy is not the cost.

## Prove

`cargo test -p podvm` exits 0, with a test that a second `run` of the same
image writes no copy of the image (the store holds one tree of the image
after two runs)

## Done

**Partial 2026-11-13.** The first step of the Approach, the measurement.
`PODVM_442_MIRROR=1 sh experiments/442-run-copy-cost.sh` exits 0
([the run](../experiments/results/run-copy-cost.txt)) on the cloud host
(ext4, no FICLONE, so each fill copies the bytes): the median of 5 runs
of `run --rm IMAGE true` is 77 ms for busybox:musl (1.5 MiB, fill 5 ms,
6 %), 2861 ms for ubuntu:24.04 (74.6 MiB, fill 681 ms, 23 %) and 12611 ms
for golang:1.24.7-bookworm (810.3 MiB, fill 8377 ms, 66 %). The copy is
the cost of a large image, so the entry goes on. Remaining: the root with
no copy for each rung (an overlay mount in the user namespace where it
mounts; else a copy-up of the path table on the loader family), and the
Prove. The overlay mount belongs to `pod-enter`'s namespace rung: COORD-071
(issue 294) asks podcrates for it. The loader family is podvm's part.

**Partial 2026-11-14.** T-1781 takes the copy off each run after the
first: a spare tree of the image, filled after the run before, and the
removal of a `--rm` tree after podvm's exit (golang: 181 ms with the
spare against 8.5 s, experiment 448). The root with no copy stays the
goal of this entry: the first run of an image and each `create` still
copy, and a spare uses the disk of one more tree.

**Partial 2026-11-14, later.** podcrates `v0.42.0` carries the overlay
root of the namespace rung (podcrates@T-486, `Root::Overlay` with
`run_namespace_root`), and podvm takes the tag. The remaining step is
the completion: `pod_rootfs` completes one root path, and with an
overlay its writes must land in the upper layer while its reads see the
image below. A copy-up of a fixed list of paths would break the first
time `pod_rootfs` touches a new path, and it cannot serve a listing that
both layers hold. COORD-090 (issue 326) asked podcrates for a two-layer
completion; podcrates `v0.43.0` carries it (podcrates@T-567), so podvm
can wire the overlay root now.


# T-1758: The namespace rung has no /proc in the root: go cannot find GOROOT, /proc/self/status is missing

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the QA pass of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

On the namespace rung `go version` fails with `readlink /proc/self/exe: no such file or directory`, and `/proc/self/status` is missing, although the rung can mount.

## Premise

The rung mounts a tmpfs on /tmp and nothing on /proc; the mount of proc needs a PID namespace or a bind of the host's /proc (podcrates' `pod-enter`).

## Approach

Mount proc in the namespace rung (a new PID namespace where it is permitted, else a read-only bind of /proc), in `pod-enter` through a COORD issue if the step is the crate's.

## Decision

The fix in the entry is podvm's; a cause in a crate of podcrates goes there as a COORD issue.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `imgs.go-version.ns`, `tools.go-mod.ns`, `run.proc.ns`, and a test of the entry holds the fix

## Done

**Done 2026-11-13.** podcrates `v0.37.0` (PodUtils/podcrates#134, its
podcrates@T-491) mounts a fresh proc on the /proc of the image in the namespace rung
of `pod-enter`, after the tmpfs on /tmp; a refused mount names itself and
the rung goes on. podvm takes the tag. On the namespace rung of the cloud
host, `run --rm alpine head -2 /proc/self/status` prints `Name: head`, and
`run --rm golang:1.24-bookworm go version` prints `go version go1.24.13
linux/amd64`. `cargo test --workspace` passes (798 tests). The rows of the
Prove come with the next rerun of experiment 443.

# T-1759: No /dev/shm on either rung: python multiprocessing fails

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the QA pass of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

`multiprocessing.Pool(2)` fails with FileNotFoundError on the namespace rung; `/dev/shm` does not exist on both rungs. On the userland rung python works only because `sem_open` reaches the host's /dev/shm.

## Premise

The rootfs has no /dev/shm, and podvm makes none.

## Approach

Give each run a private /dev/shm: a tmpfs on the namespace rung, a directory of the run mapped by the path table on the userland rung.

## Decision

The fix in the entry is podvm's; a cause in a crate of podcrates goes there as a COORD issue.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `imgs.python-mp.ns`, `run.shm.ns`, `run.shm.ul`, and a test of the entry holds the fix

## Done

**Done 2026-11-13.** `podvm-complete` makes `dev/shm` in the rootfs after
the completion of `pod-rootfs`, with mode 1777 (`crates/podvm-complete/src/shm.rs`,
the row `dev-shm`). It does not write through a `dev` or `dev/shm` of the
image that is a link or not a directory. The namespace rung runs on its
private copy of the rootfs, and the path table of the userland rung maps
/dev/shm into it. On both rungs, `echo x > /dev/shm/F` works and
`multiprocessing.Pool(2).map` gives `[1, 2]` in `python:3.12-slim`; the
rows of the Prove pass in the rerun of experiment 443 before the release.
The test `dev_shm_is_made_once_and_never_through_a_link` holds the step.
It is a directory, not a tmpfs: `--shm-size` stays unserved.

# T-1764: `--podvm-confine` without /etc in the read list breaks the loader on the namespace rung

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the QA pass of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`--podvm-confine 'read=/usr;write=/tmp'` with python exits 127: `libpython3.12.so.1.0: cannot open shared object`.

## Premise

ld.so reads `/etc/ld.so.cache`, which the spec does not allow, and its search does not reach /usr/local/lib.

## Approach

Allow `/etc/ld.so.cache` and the library directories of the image in each confine spec, as the system directories are.

## Decision

The fix in the entry is podvm's; a cause in a crate of podcrates goes there as a COORD issue.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `run.confine-py.ns`, and a test of the entry holds the fix

## Done

**Done 2026-11-13.** `SYSTEM_READ` of `crates/podvm-enter/src/confine.rs`
now holds the files of /etc that the dynamic loader reads before `main`:
`/etc/ld.so.cache` (glibc) and `/etc/ld-musl-x86_64.path` and
`/etc/ld-musl-aarch64.path` (musl). Each gets a read rule where the image
has it. Without the cache, glibc's loader searched only /lib and /usr/lib,
and python of /usr/local/lib did not start. By hand on the cloud host:
`run --podvm-confine 'read=/usr;write=/tmp' python:3.12-slim python3 -c
'open("/etc/hostname").read()'` now raises `PermissionError` on
/etc/hostname, as the row expects. The test
`the_loader_reads_its_cache_under_each_spec` holds the rule, and the old
test still holds that no other file of /etc gets one. The row of the Prove
comes with the rerun of experiment 443 before the release.


# T-1804: `--read-only` with `-v HOST:/GUEST/:ro` leaves the volume writable

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), F-04 (CODE-04; verify item 5 and V-4)
**Category:** enter
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

`podvm run --read-only -v $PWD/vol:/data/:ro alpine:3.20 sh -c 'echo CHANGED > /data/f'`
succeeds and the file changes on the host. Without the trailing slash,
or without `--read-only`, the write is refused. The flag that makes a run
stricter makes a `:ro` volume writable, and `--strict` does not see it.
The table is `PODVM_RO=ro:/,rw:/dev,rw:/data/,ro:/data`.

## Premise

Two causes. `crates/podvm/src/run/parse/values.rs:104` records the `:ro`
guest without its trailing slash, and the volume keeps `/data/`; so
`read_only_table` (`crates/podvm/src/lifecycle/volume.rs:331`) emits both
`rw:/data/` and `ro:/data`. `denied_in`
(`crates/podvm-interpose/src/map/readonly.rs:102`) strips one trailing
slash from each prefix and keeps the first entry on a tie of length,
which is the `rw` entry.

## Approach

Normalize each guest path once, where the volume is parsed, and build
each table from that one spelling. In `denied_in`, let an `ro` entry win
a tie of length, so that a duplicate can never widen access.

## Decision

Both changes: the first removes the duplicate, the second makes a
duplicate safe. The interposer is podvm's own crate.

## Prove

`cargo test -p podvm-interpose denied_in` exits 0 and `cargo test -p podvm --bin podvm read_only_table` exits 0, with tests that `ro:/,rw:/data/,ro:/data` denies `/data/f` and that `--read-only -v H:/data/:ro` gives no `rw:` entry for `/data`

## Done

**Done 2026-11-14.** Both causes. `run/parse/values.rs` gives the guest
path of each `-v` one spelling (`lifecycle::volume::guest_spelling`: no
trailing `/`, except `/`), and `read_only_table` compares each path in
that spelling, also for a record of an older podvm. `denied_in`
(`podvm-interpose`, `map/readonly.rs`) lets an `ro:` entry win a tie of
length. Tests: `denied_in_lets_ro_win_a_tie` (run in
`crates/podvm-interpose` with `cargo test --release --target
x86_64-unknown-linux-gnu denied_in`, because the crate is outside the
workspace), `read_only_table_has_one_spelling` and
`read_only_table_of_a_volume_with_a_trailing_slash`. By hand: `run
--read-only -v H:/data/:ro alpine:3.20 sh -c 'echo CHANGED > /data/f'`
exits 1 and the host file keeps its text, on the namespace rung and on
the fixture of the userland rung, with and without the `/`; beta.15
exits 0 and changes the file on both. Experiment 443 has the row
`run.readonly-ro-slash`.


# T-1819: An `exec=` rule of `--podvm-confine` does not deny a program on the loader family

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), O-03 (MACH-07; verify V-1; REPORT, "Findings that were rejected or corrected", rows 1, 2 and 6)
**Category:** enter
**Milestone:** none
**Priority:** P0
**Effort:** M
**Status:** done

## Problem

On the userland rung, `--podvm-confine 'read=/etc;write=/tmp;exec=/bin;connect=443'`
lets `/tmp/s.sh` (a script) and `/tmp/bb` (busybox, copied) run; a
script and a program in a volume also run, and `exec=` empty runs
`/vol/bb`. The same spec on the namespace rung denies the script (126).
The read, write and connect rules are enforced. The results for a static
program disagree: the report owner says that a static program in `/tmp`
runs under `read=/tmp;exec=/bin`, and the verifier says that a static
child is denied under each of `exec=/bin`, `exec=/tmp`, `exec=/tmp/pv`
and `exec=/` (126), so a rule that grants `/tmp` denies it.

## Premise

On the loader family the interposer starts a dynamic program or a script
through the loader of the image, which is in a system directory that
the layer always grants; Landlock then checks EXECUTE on the loader and
only READ on the program. The layer is built in `pod-enter` and
`pod-confine` (podcrates v0.42.0), reached from
`crates/podvm/src/lifecycle/confine.rs`. The static case is not settled.

## Approach

First measure the static case on both rungs with a static ELF, not a
script. Then the interposer checks the exec table against the resolved
program path before it starts the loader, and refuses with EACCES; and
podvm asks podcrates to check that `LANDLOCK_ACCESS_FS_EXECUTE` is in the
handled set and granted for each `exec=` path (proposed COORD issue).
Until both hold, refuse `exec=` on the loader family by name.

## Decision

A confine rule that the binary cannot enforce is refused, not accepted
(the recommendation of the report, step 3).

## Prove

`cargo test -p podvm --test confine_exec` exits 0, with tests on the userland rung that a script, a dynamic ELF and a static ELF outside each `exec=` rule are denied, and that a static ELF inside `exec=/tmp` runs

## Correction

**Amended, 2026-11-14.** The test target `confine_exec` of the Prove was
not made: the measurement needs an image and the fixture of the userland
rung, so it is in experiment 443 (rows `run.confine-exec-script`,
`-copy`, `-static` and `-in`, both rungs), with the unit tests
`exec_allowed_in_takes_each_prefix` and
`exec_table_names_the_system_dirs_and_each_exec_path`.

## Done

**Done 2026-11-14.** Two causes, both in podvm's own code; no wait for
podcrates. (1) On the userland rung the kernel executes the loader, so
Landlock never checked EXECUTE on the program: podvm now gives the
interposer the exec table of the layer, `PODVM_EXEC` (the system
directories of the image and each `exec=` path, as guest paths;
`lifecycle/confine.rs`, `exec_table`, set by `decide`, which drops a
caller's own), and `plan` of the interposer (`exec.rs`) refuses with
EACCES each level (the program, a script and its interpreter) whose
resolved guest path no prefix holds (`map/readonly.rs`,
`exec_allowed`). (2) The static case of the report: a static child of
the loader family enters the native-static rung through podvm's own
binary (T-1724), and the layer gave that binary execute only on the
other families, so a static program that `exec=` granted got 126; it
now gets read and execute on each family. Tests:
`exec_allowed_in_takes_each_prefix` (interposer) and
`exec_table_names_the_system_dirs_and_each_exec_path`. A deviation from
the Prove: the measurement is in experiment 443, not in a test of
`cargo test --test confine_exec`, because it needs an image and the
fixture of the userland rung; the rows `run.confine-exec-script`,
`-copy`, `-static` and `-in` run on both rungs, and the section run
gave 237 pass and 0 fail. By hand, on the fixture
and on the namespace rung: outside `exec=`, a script, a copied busybox
and a static program give 126 with the line "the confine layer denies
the exec of this path (exec=)"; inside `exec=/vol`, the script and the
static program run. The ask of COORD-098 (issue 344, podcrates@T-583)
stays for the check of the crate. ⚠ The interposer is not a boundary:
a raw `execve` system call passes it, as the module doc of
`map/readonly.rs` says of each check of the table.

# T-1887: `--podvm-confine` on the userland rung resolves an absolute link of the image against the host root

**Source:** PodUtils/podvm#364 (Nemo-010, 2026-11-14), with a patch
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

On the userland rung, `host_spec` (`crates/podvm-enter/src/confine.rs`)
maps each guest path of the spec to a host path by a lexical join onto
the rootfs. A link of the image whose target starts with `/` is then
opened from the host root. `podvm run --podvm-confine exec=/bin/sh
alpine:3.20 /bin/sh -c 'echo hi'` exits 125 with "does not exist":
`bin/sh` links to `/bin/busybox`, which the image has and the host does
not. 81 of the 82 names in `/bin` of the image are such links. A link to
a path that only the host has compiles a rule on the host inode.

## Premise

On a rung with a chroot, `compile_in` opens each rule with
`RESOLVE_IN_ROOT`, so a link resolves inside the image. The loader
family has no root descriptor, so podvm must resolve the links itself,
as the interposer resolves them for the payload.

## Approach

Take the patch of the report: `host_resolved` walks the guest path one
component at a time inside the tree that serves it (the rootfs or a
volume), restarts at that tree on an absolute target, stops `..` at its
root, and bounds the walk at 40 links. `host_spec` uses the result for
the existence test and for the rule. Leave out the debug print of the
patch.

## Decision

A guest path resolves inside the tree that serves it, on every rung.
The host decides no verdict of the spec. A required path whose walk
leaves the tree is T-1888.

## Prove

`cargo test -p podvm-enter confine` exits 0 with the three tests of the report, and on the userland fixture `podvm run --rm --podvm-confine exec=/bin/sh` of `mirror.gcr.io/library/alpine:3.20` with `/bin/sh -c 'echo hi'` prints `hi` and exits 0

## Done

**Done 2026-11-14.** The patch of PodUtils/podvm#364 (Nemo-010) is in
`crates/podvm-enter/src/confine.rs` (`host_resolved`), without its debug
print. `cargo test -p podvm-enter confine` exits 0 with the three tests
of the report. On the userland fixture with
`mirror.gcr.io/library/alpine:3.20` and `/bin/sh -c 'echo hi'`:
`exec=/bin/sh`, `read=/bin/cat`, `exec=?/bin/sh`, `read=/bin`,
`read=/etc/mtab` and `exec=/bin/busybox` each print `hi` and exit 0;
beta.17 on the same fixture refused `exec=/bin/sh` at 125 with ENOENT on
the host path of the tree.

# T-1888: A required confine path whose link leaves the tree still grants a host inode

**Source:** PodUtils/podvm#364, "What the patch does not change"
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

After T-1887, a required path (no `?`) whose walk finds nothing inside
the tree falls back to the lexical host path. A link of the image to a
path that only the host has (`etc/abs-host -> /bin/sh`) then compiles a
rule on the host's inode, and the run exits 0.

## Premise

A rule that names a host inode is the class of surprise that T-0405
refuses for `etc/mtab`. The payload's own path calls go through the
interposer and see the tree, so the rule grants more than the tree
holds.

## Approach

Make `host_spec` fallible: a required path that does not resolve inside
the tree refuses by name, with the guest path the caller wrote, and an
optional path takes no rule.

## Decision

A required path that the image lacks refuses, wherever the host has it.

## Prove

`cargo test -p podvm-enter confine_required_link` exits 0, with a test that a required rule on a link to a host-only path refuses with the guest path in the message

## Done

**Done 2026-11-14.** `host_spec` returns a `Result`: a required path
that does not resolve inside its tree refuses with the guest path, and
the message says whether the image lacks the path or holds a link out
of the tree. `create` keeps the SPEC and leaves the check to `start`,
as before. `cargo test -p podvm-enter confine_required_link` exits 0.
By hand on the userland fixture: `create --podvm-confine
read=/etc/abs-host`, then a link `etc/abs-host -> /usr/bin/python3` in
the tree (a path of the host only), and `start` refuses at 125 with
"the path \"/etc/abs-host\" of the spec is a link of the image that
leaves the tree"; a link to `/bin/busybox` starts and exits 0; `run
--podvm-confine read=/nosuch` refuses at 125 with "is not in the image",
and `read=?/nosuch` runs.

# T-1892: Two exec tables reach the payload: podvm's guest paths and the crate's host paths

**Source:** the take of podcrates `v0.45.0` (2026-11-14), podcrates@T-583
**Category:** enter
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

Since podcrates `v0.45.0`, `pod-enter` writes the exec table of the
confine layer into the environment of a payload with no chroot, as host
paths, under `Names::exec_table`. podvm's interposer reads its own table
of guest paths in `PODVM_EXEC` (T-1819). podvm names the crate's table
`PODVM_EXEC_HOST`, so a confined payload gets two variables, and the
interposer reads one of them.

## Premise

`pod_confine::ExecTable::allows_in` checks a resolved host path against
the table; the interposer of podvm resolves each program through the
table of the rootfs and can check the host path too.

## Approach

Move the check of the interposer to the crate's table of host paths
(`PODVM_EXEC_HOST`, the format of `ExecTable::encode`), remove
`exec_table` of `lifecycle/confine.rs`, and keep one variable.

## Decision

One table of each layer reaches the payload.

## Prove

`cargo test -p podvm --bin podvm exec_table` exits 0, and a run on the userland fixture with `--podvm-confine exec=/vol` has `PODVM_EXEC_HOST` and no `PODVM_EXEC` in its environment, and still gives 126 for a program outside the table

## Done

**Done 2026-11-15.** The interposer reads the one table that `pod-enter`
writes, `PODVM_EXEC_HOST` (the host paths of the layer, `:` separated, as
`pod_confine::ExecTable::encode` gives them), and checks the resolved host
path of each level of an exec against it (`podvm-interpose/src/map/readonly.rs`,
`exec.rs`); a path with `..` is never allowed, as in
`pod_confine::exec::allows_in`. podvm writes no `PODVM_EXEC` of its own,
and drops a caller's table of either name (`lifecycle/confine.rs`).
`cargo test -p podvm --bin podvm exec_table` exits 0, and the interposer
tests pass (98). On the userland fixture, `run --podvm-confine
'write=/tmp;exec=/vol' -v DIR:/vol alpine:3.20` has `PODVM_EXEC_HOST` and
no `PODVM_EXEC`, runs `/vol/s.sh`, and gives 126 for `/tmp/bb` ("the
confine layer denies the exec of this path").


# T-1896: `run -d` of a static image where chroot is denied refuses: the native-static rung has no detached driver

**Source:** PodUtils/podrunner#100, case 2 (podrunner@T-158, 2026-11-14); the answer is issue 376
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

On a host that denies chroot, `podvm run -d busybox:1.37.0-musl sleep 5`
exits 125 with "the memfd family has no detached driver". A container of
podrunner runs detached, so a static image cannot be a container there.

## Premise

The foreground entry of the native-static rung (`lifecycle/native.rs`)
is `podvm --podvm-dispatch-enter ROOT EXE ARGV...` through
`podvm_enter::spawn_userland`. The launcher of a detached run already
execs a recorded argv through `spawn_userland` (T-1411).

## Approach

`detached_drive` records the foreground argv of the native-static rung,
and `refuse_detached_userland` refuses only an entry with no drive. The
emulate and syscall tiers keep no detached driver.

## Decision

A rung whose foreground entry is one argv through `spawn_userland` has a
detached driver: that argv.

## Prove

`cargo test -p podvm --bin podvm the_record_carries` exits 0, and on the deny-chroot fixture `run -d` of `busybox:1.37.0-musl` with `exit 7` gives 7 from `wait`, and `stop` of a detached `sleep 60` gives `Exited (143)` with no `sleep` left

## Done

**Done 2026-11-15.** `native_drive` in `lifecycle/userland.rs` gives the
drive. `cargo test -p podvm --bin podvm the_record_carries` exits 0. On
the deny-chroot fixture with `mirror.gcr.io/library/busybox:1.37.0-musl`:
`run -d` of `sh -c 'echo hello-detached; ls /bin/busybox; exit 7'` gives
7 from `wait`, `logs` prints both lines, and `/bin/busybox` resolves in
the image; `stop -t 2` of a detached `sleep 60` gives `Exited (143)`, and
`pgrep` finds no `sleep`.


# T-1901: A `:ro` volume is refused on the native-static rung, so a static job container of the runner fails where chroot is denied

**Source:** PodUtils/podrunner#111 (COORD-051 of podrunner, podrunner@T-158, 2026-11-15)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

The official runner mounts its externals at `/__e` with `:ro`. `podvm run
-d -v SRC:/__e:ro busybox:1.37.0-musl ...` where chroot is denied refused
with exit 125 ("--read-only and :ro need the read-only layer of the
interposer"), as only the loader family read `PODVM_RO`. A static image
takes the native-static rung (T-1652), whose payload makes raw system
calls past the interposer.

## Premise

The handler of the native-static rung sees each system call of the payload
(Syscall User Dispatch) and maps each volume to a host directory
descriptor (T-1724). It can refuse a write into a `:ro` volume before the
call runs, as a read-only bind mount does.

## Approach

The handler marks the volumes that `PODVM_RO` names `ro:`, and a call that
writes through a path in one, or through a descriptor of a file in one,
answers `EROFS`. podvm stops refusing a `:ro` volume on that rung;
`--read-only` of the rootfs stays the interposer's (podrunner does not need
it). The noise of the destroy (ask 3) is the background removal of the
trash of podvm racing the `chmod -R` of podrunner; `PODVM_SPARE_TREE=0`
makes the removal synchronous.

## Decision

A write through a mapping of a file that was opened before is no system
call and passes, as on a bind mount that turns read-only later.

## Prove

`cargo test -p podvm --test ro_native` exits 0 where chroot is denied (CI,
or uid 1000 on the cloud host), with a write that fails with errno 30, a
read that works and a host file that stays.

## Done

**Done 2026-11-15.** `crates/podvm-dispatch/src/ro.rs`: the handler marks
the `ro:` volumes of `PODVM_RO` (`state::Vol::ro`), and its gate answers
`EROFS` for `open`/`openat` with a write flag, `creat`, `mkdir`, `mknod`,
`unlink`, `rmdir`, `rename`, `link` (both names: a hard link is a writable
name), `symlink`, `chmod`, `chown`, `truncate`, the times and the xattrs,
by path, and for `ftruncate`, `fchmod`, `fchown`, `fsetxattr`,
`fremovexattr` and `fallocate` by descriptor (`/proc/self/fd/N`). The
volume with the longest guest prefix decides, so a writable volume in a
read-only one stays writable. `lifecycle/volume.rs` takes `:ro` on the
native-static rung for `run`, `run -d` and `exec`. On the cloud host
under the deny-chroot fixture, a static busybox got `Read-only file
system` for a write, `touch`, `rm`, `mkdir`, `chmod` and `ln`, wrote to
`/tmp`, and the host file stayed `orig`; `run -d -v SRC:/__e:ro` of
`busybox:1.37.0-musl` read `/__e/f` and its write failed the same way.
`cargo test -p podvm --test ro_native` exits 0 as uid 1000 (the
native-static rung: errno 30, the read works, the host file stays) and
says SKIP as root (the chroot rung). The rung of a run is already the
field `rung` of `--report` (ask 2).

# T-1902: `--podvm-confine` stops name resolution in the payload, while TCP to an address works

**Source:** PodUtils/podvm#396 (the live run of 2026-11-15, podvm `0.2.0-beta.18`, WSL2, kernel 7.2.0, Landlock ABI 10)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

Under `--podvm-confine "read=/;write=...;exec=..."`, `getent hosts github.com` and `curl https://github.com/` fail, while `curl -k https://140.82.112.3/` answers. Without the confine layer the same image resolves. Under podrunner each job gets `SocketException (11)` (EAI_AGAIN).

## Premise

The resolver sends UDP to the name server of the payload. A Landlock ruleset that handles a network right of a newer ABI with no rule for it refuses each use of that right. Not measured here: the cloud host has kernel 6.18.

## Approach

Find which right of the ruleset refuses the resolver (the ruleset of `pod-confine`, podcrates), allow the name server of the payload or handle only the rights that the spec names, and document what the confine layer refuses.

## Decision

The confine layer never breaks name resolution silently: it allows it, or it refuses the run by name.

## Prove

On a host with Landlock ABI 6 or more: `podvm run --rm --podvm-confine "read=/;write=/tmp" IMAGE getent hosts github.com` prints an address and exits 0; a new test of the ruleset holds the case.

## Done

**Done (2026-11-15).** The premise was wrong: no network right refused the resolver. musl opens `/etc/resolv.conf` (and `/etc/hosts`, `/etc/services`) in `__fopen_rb_ca` with its own `syscall` instruction, past each symbol of the interposer, so on the loader family the kernel resolves the host path. The confine layer of the userland rung allows only the host paths of the spec (the tree), so Landlock refused that open with `EACCES`; musl then found no name server, asked 127.0.0.1 and failed with `EAI_AGAIN`. `crates/podvm/src/lifecycle/confine.rs` (`raw_reads`) now gives read on each of the three host files (link resolved) where the spec lets the payload read the guest path; podvm writes the same `resolv.conf` and `hosts` into the tree. A spec that does not cover `/etc` still denies them, as on the namespace rung. Evidence (Landlock ABI 7, kernel 6.18, uid 1000, userland rung): `run --rm --podvm-confine "read=/;write=/tmp"` resolves github.com with alpine 3.20 (`getent hosts`), debian bookworm-slim (`getent hosts`) and busybox 1.36-musl (`nslookup`), each exit 0; before the change alpine exited 2 and strace showed `open("/etc/resolv.conf") = -1 EACCES` and `sendto(... 127.0.0.1:53)`. The test `a_rule_over_etc_reads_the_raw_files_of_musl` holds the rule.


# T-1908: A payload has the host name of the host: no `--hostname`

**Source:** PodUtils/podvm#402 and PodUtils/podrunner#115 (COORD-053 of podrunner, podrunner@T-182)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

A podvm unit shares the UTS name of the host, so the official runner writes the host name of the machine of the operator into the job log of github.com (`Machine name`). `podvm run` has no `--hostname`.

## Premise

The namespace rung can unshare a UTS namespace. The native-static rung sees each system call (`uname` there is a call of the handler); the loader family has the interposer, which can answer `uname` and `gethostname`.

## Approach

Add `--hostname NAME` to `run` and `create`: a UTS namespace where the rung has one; else the handler or the interposer answers `uname` (nodename), `gethostname` and a read of `/proc/sys/kernel/hostname` and `/etc/hostname` with NAME. Where podvm cannot hold it, exit 125 with the reason.

## Decision

The release, the version and the machine of `uname` stay those of the host.

## Prove

`podvm run --rm --hostname unit1 IMAGE uname -n` prints `unit1` on the namespace rung and on the native-static rung (CI), and a rung that cannot hold it exits 125 by name.

## Done

**Done (2026-11-15).** The namespace rung holds the name too: a foreground `run` of an unprivileged caller owns a UTS namespace of the entry of T-1909, and a `run --hostname` of uid 0 unshares one at the start of `main` (`crates/podvm/src/userns.rs`); `sethostname` names it, and the host keeps its name. Evidence: as root, `run --rm --hostname unit1 alpine:3.20` prints `unit1` for `uname -n`, `hostname` and `/etc/hostname`, and `hostname` of the host is unchanged; as uid 1000, experiment 458 clause 3. COORD-130 is not needed and is closed. The first half, the same day: `run` and `create` take `--hostname NAME` (and `--hostname=NAME`): 1 to 63 letters, digits, dots and hyphens. The handler of the native-static rung (`crates/podvm-dispatch/src/uts.rs`) and the interposer of the loader family (`crates/podvm-interpose/src/uts_calls.rs`, `uname` and `gethostname` in `interpose.map`) answer the name of `PODVM_HOSTNAME` in the nodename field. `/etc/hostname` of the tree holds it, `HOSTNAME` names it (a `-e HOSTNAME` of the caller wins), and `/etc/hosts` resolves it to 127.0.0.1. Each other entry, the machine tier and the incus tier refuse with exit 125 by name. Evidence: as uid 1000 on the userland rung, `run --rm --hostname unit1` of alpine 3.20 (loader family) and of busybox 1.36-musl (native-static) print `unit1` for `uname -n`, `hostname`, `/etc/hostname` and `$HOSTNAME`; as root, the namespace rung refused at 125 until the UTS namespace of uid 0 above.

**Done, amended (2026-11-15, later).** `exec` into a container of `--hostname` gives the name too, as with docker: the environment of the record keeps `PODVM_HOSTNAME` (the userland rung), and on the namespace rung `exec` joins the UTS namespace of the payload. Evidence: `run -d --hostname podrunner-u1` of alpine 3.20 and busybox 1.36-musl, as uid 1000 and as root: the payload (`logs`) and `exec ID sh -c 'uname -n; hostname'` each print `podrunner-u1`, and the host keeps its name.

# T-1910: `exec` and `cp` do not join a run that podvm put in a user namespace

**Source:** T-1909 (the audit of the pids of a record, 2026-11-15)
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

A foreground `run` of an unprivileged caller lives in a user, a PID and a UTS namespace that podvm opened (T-1909). `exec` and `cp` from outside try to join its mount namespace with the pidfd of the payload; the kernel refuses (`setns` of a mount namespace needs `CAP_SYS_ADMIN` in the user namespace that owns it), and they take an entry of their own. Measured 2026-11-15: `exec` into such a run did not see a file that the payload wrote in its `/tmp`, and ran as uid 1000.

## Premise

The caller owns the user namespace, so it has each capability in it. `setns(pidfd, CLONE_NEWUSER | CLONE_NEWNS | CLONE_NEWUTS)` in a child with one thread joins all of them in one call (Linux 5.8).

## Approach

In `exec` and `cp`, where the record of a running container names a payload in another user namespace (`/proc/PID/ns/user` differs), fork a child that joins the user, the mount and the UTS namespace through the pidfd, then enters as the join of T-1765 does.

## Decision

The join is all or nothing: a refused join names the call and keeps the entry of its own, as now.

## Prove

As uid 1000 on a host where `unshare -Urm true` exits 0: with `podvm run --rm --name a IMAGE sh -c 'echo hi > /tmp/m; sleep 30'` running, `podvm exec a cat /tmp/m` prints `hi` and `podvm exec a id -u` prints 0.

## Done

**Done (2026-11-15).** Where `exec` or `cp` finds that the running payload of the namespace rung is in another user namespace (`/proc/PID/ns/user` differs), podvm starts itself again with the same arguments and `PODVM_USERNS_JOIN=PID` (`crates/podvm/src/userns.rs`, `rejoin`). First in `main`, with one thread, the new podvm opens a pidfd of the payload, joins its user, UTS and PID namespace with one `setns`, keeps the pidfd for the join of the mount namespace (the host pid of the record names nothing in the PID namespace), and forks: after `setns(CLONE_NEWPID)` a process makes no thread, so a child goes on with the verb as a member of the namespace, and the parent waits and relays the shutdown signals and the exit code. Evidence (kernel 6.18, uid 1000): with `run --rm --name j4 alpine:3.20 sh -c 'echo hi > /tmp/m; sleep 30'` running, `exec j4 sh -c 'cat /tmp/m; id -u'` prints `hi` and `0`, `exec j4 sh -c 'exit 5'` exits 5, `exec` with `--hostname box3` on the run prints `box3`, `ps` in it lists the processes of the run, `cp j4:/tmp/m` copies `hi`, and no podvm stays after `stop`; experiment 458 clause 8.


# T-1914: `run -d` refuses "a passed descriptor goes onto 17" when the parent holds descriptors 3 to 15

**Source:** PodUtils/podrunner#177 (COORD-059 of podrunner, podrunner@T-227)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

Under `podnet pod`, `podvm run -d` from podrunner exits 125: "a passed descriptor goes onto 17, the number of the root file system or of the memfd".

## Premise

podvm passes the ownership memo on descriptor 17 of the payload (`MEMO_CHILD_FD`), and pod-enter refuses a root directory whose descriptor in podvm holds that number. `RootDir::open` takes the lowest free number.

## Approach

Hold 17 in each podvm process from its start with a close-on-exec placeholder where it is free, so that no open takes it; the `dup2` of the memo in the child replaces it.

## Decision

podvm keeps the number 17 (the environment of each payload names it, `PODVM_MEMO_FD=17`); a parent that holds 17 itself is untouched.

## Prove

`sh experiments/460-memo-descriptor.sh` exits 0

## Done

**Done (2026-11-15).** `names::start` holds 17 (`hold_memo_number`). Measured with a parent shell that holds descriptors 3 to N (N in 2..16) and `run -d alpine:3.20 true` then `run --rm`: podvm `0.2.0-beta.22` (the release binary) fails exactly at N = 15, where the root opens on 17; this build passes at each N. Experiment 460: PASS (`experiments/results/memo-descriptor.txt`).


# T-1920: A volume moves a run off the namespace rung, and a raw syscall there sees host paths

**Source:** PodUtils/podrunner#189 (COORD-066 of podrunner), COORD-147 (PodUtils/podvm#439)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

On a host where a caller with no root can open user namespaces, `run -v HOST:GUEST` takes the loader family of the userland rung (T-1761), also where the namespace rung works. There the `LD_PRELOAD` interposer maps the paths of libc calls only. `uv` and `ruff` persist a temporary file with a raw `syscall` instruction (`tempfile` through `rustix`), so `rename` and `renameat2` reach the kernel with a host path and give EACCES. Measured as uid 1000 on the cloud host, with podvm `0.2.0-beta.24` and `main`: a glibc program with an inline `syscall` gives "Permission denied" for `rename` and `renameat2(RENAME_NOREPLACE)` in `/root/.cache/uv`; the same calls through libc work.

## Premise

A foreground run of such a caller has a mount namespace of its own (T-1909, pid 2), where it binds the host's device nodes onto the tree (`userns::bind_devices`). The mounts of pid 2 reach pod-enter's mount namespace, which copies them.

## Approach

In the user namespace of a foreground run, bind each volume onto the tree (read-only for `:ro`) before pod-enter starts, and keep the run on the namespace rung. Before any removal of a tree, unmount each mount under it, deepest first, so that no removal walks into a host directory. `run -d` (pod-enter's own user namespace, T-1912) needs a bind of a volume in pod-enter's plan: a COORD issue to podcrates.

## Decision

⛔ A tree with a bind of a host directory in it is never removed while the bind holds.

## Prove

As uid 1000 on a host where `unshare -Urm true` exits 0: `run --rm -v DIR:/work IMAGE` with a dynamic program that calls `renameat2` as a raw `syscall` instruction in `/root/.cache` prints "ok", `inspect` of the run names the namespace rung, a file written to `/work` lands in DIR, and after the run DIR keeps each of its files.

## Done

**Done (2026-11-16).** `run -d` and `create` bind each volume through pod-enter's `Plan::volumes` on the namespace rung. `run/binds.rs` decides it (`served`: a detached run or `create`, outside podvm's own user namespace, on a host where the namespace rung is reachable, and no `,` or `:` in a path), and writes the volumes to the record as `PODVM_BINDS` (`GUEST:HOST[:ro]`). The launcher (`podvm-supervise`) reads them into `Plan::volumes` and removes them from the env of the payload. Such a run no longer asks for the loader family, and `lifecycle/volume.rs` accepts the volume with no `PODVM_MAPS`. Measured as uid 1000 on the cloud host (mirror.gcr.io/library/python:3.13-slim): `run -d -v VOL:/data -v DIR:/work:ro` gives `"Rung":"namespace"`; the glibc program of the foreground proof prints "ok" for `rename` and `renameat2(RENAME_NOREPLACE)` as raw `syscall` instructions in `/data`; a file written to `/data` lands in VOL; a write to `/work` fails; `exec` reads `/data`; after `rm -f`, VOL keeps each of its files. `create -v VOL:/data` then `start` gives `"Rung":"namespace"` and writes to VOL.

**Partial (2026-11-16).** A foreground run in the user namespace of T-1909 binds each volume onto the tree in the mount namespace of pid 2 (`userns::bind_volumes`, from `run/acquire.rs`), and keeps the namespace rung: `run/prepare.rs` asks for the loader family for a volume only outside that namespace. A `:ro` volume is remounted read-only, with the flags that the kernel locks kept; a guest path with a symbolic link of the image in it is refused by name. `podvm_extract::live::unmount_under` detaches each mount under a tree, the deepest first, and `remove_plain` refuses to remove a tree while a mount stays under it. Measured as uid 1000 on the cloud host (mirror.gcr.io/library/python:3.13-slim): with `-v DIR:/work -v VOL:/data`, a glibc program that calls `rename` and `renameat2(RENAME_NOREPLACE)` as raw `syscall` instructions prints "ok" in `/root/.cache/uv` and in `/data`, `inspect` gives `"Rung":"namespace"`, a file written to `/data` lands in VOL; `:ro` reads and refuses a write with "Read-only file system"; after `rm` of a kept run, three `--rm` runs and one with `PODVM_SPARE_TREE=0`, VOL keeps each of its files; `-v VOL:/bin/data` refuses ("bin of the image is a symbolic link"). The 25 integration test binaries pass as uid 1000. Remaining: `run -d` and `create`, whose entry is pod-enter's (T-1912), need a bind of a volume in pod-enter's plan: COORD-148 (PodUtils/podvm#440), which podcrates takes as podcrates@T-738 (P1): the volumes in `Plan`, a bind after the device binds and before the `chroot`, a read-only remount with the locked flags, a named refusal for a failed bind, a link of the image, or a missing host path. Then `run -d` and `create` move onto it, at the tag that carries it. **Partial, amended (2026-11-16).** podvm takes podcrates `v0.59.0` (COORD-118, PodUtils/podcrates#204), whose `Plan::volumes` binds each volume on the namespace rung, also through pod-enter's user namespace (podcrates@T-738). Each `Plan` of podvm names `volumes`, empty for now. Remaining: the record of a detached run on the namespace rung keeps its volumes, the launcher fills `Plan::volumes` from it and drops them from the env of the payload, and a volume of `run -d` stops asking for the loader family.

# T-1926: `run -d --hostname` refuses on the namespace rung that pod-enter enters

**Source:** PodUtils/podrunner#196 (COORD-070 of podrunner, podrunner@T-317)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

As a user with no root on a host with user namespaces, `podvm run -d --hostname NAME IMAGE CMD` exits 125 with "--hostname needs the handler of the native-static rung or the interposer of the loader family, and this entry has neither and no UTS namespace". The run enters the namespace rung through pod-enter, whose plan opens a UTS namespace and sets the name before the exec (`Plan::hostname`, podcrates `v0.52.0`, T-1908). The check of `run/acquire.rs` counted only a UTS namespace of podvm's own (the foreground `run` of T-1909), so it refused each detached run with a name. podrunner started its units with no name, and the job log showed the name of the host.

## Premise

The launcher of `podvm-supervise` passes `PODVM_HOSTNAME` as `Plan::hostname` where the record names the namespace rung, and pod-enter refuses a plan with a name where it falls back to the chroot sequence.

## Approach

`hostname_env` takes the probe findings: a detached run (`run -d`, `create`) with no userland family whose rung is the namespace rung (`entered_rung(Selection::choose(findings).rung)`) holds the name through pod-enter.

## Decision

The same rung function as the banner and the record, so the check cannot disagree with the entry.

## Prove

The integration test `crates/podvm/tests/hostname_detached.rs` as uid 1000 and as root.

## Done

**Done (2026-11-16).** `hostname_env` counts the UTS namespace of pod-enter's entry for a detached run on the namespace rung. Measured as uid 1000 on the cloud host: `run -d --hostname podrunner-t` of an imported image exits 0, the record names the namespace rung, and the payload reads `podrunner-t` from `/proc/sys/kernel/hostname` and `/etc/hostname`, where the host is named `vm`; before the change the run exited with the refusal. `hostname_detached.rs` passes as uid 1000 and as root.

# T-1927: On the namespace rung of a user with no root, the payload is uid 0 of a one-uid namespace, not the user of the image

**Source:** PodUtils/podrunner#196 (COORD-070 of podrunner, podrunner@T-301, podrunner@T-182), T-1911; COORD-154 (#447), COORD-155 (#448)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

The entry of pod-enter through a user namespace maps the caller to uid 0 (`uid_map` `0 UID 1`). The payload of `run -d` is root of that namespace, also where the image names a `USER`, and docker runs the image's `USER` by default. podrunner's runner image has `USER runner` (uid 1001): the official runner refuses root, GNU tar as root fails at each chown to an id that is not mapped, and npm 6 as root runs its scripts as `nobody`. podrunner works around the three with variables.

## Premise

A one-line map can map the caller to any id: `1001 UID 1`. The process that makes the namespace holds each capability in it whatever its id, so the mounts of the rung still work, and the `execve` of the payload as a uid that is not 0 drops them. Each file of the tree (extracted by the caller) shows as owned by the mapped id; uid 0 is not mapped. T-1911 covers a map with a range of `/etc/subuid`.

## Approach

1. pod-enter takes the id inside the namespace in its plan (COORD-154 to podcrates); the default stays 0.
2. podvm resolves the image's `USER`, or `-u`, against the tree as `apply_user` does, and gives that id to the plan of `run -d` and `create` on the namespace rung, and to the user namespace of a foreground `run` (T-1909, podvm's own code).
3. `--user 0` keeps the map of today.

## Decision

The id of the image by default, as docker: a payload that needs root names `--user 0`.

## Prove

As uid 1000: `podvm run -d IMAGE id -u` of an image with `USER 1001` logs 1001, and `/proc/self/uid_map` reads `1001 1000 1`; with `--user 0` it logs 0.

## Done

**Done (2026-11-16).** podcrates answered COORD-159 with `v0.63.0` (podcrates@T-787): where the caller is uid 0 of a user namespace that is not the initial one, pod-enter nests a user namespace with the ids of the plan after the mounts. podvm takes the tag, and a foreground `run` records and hands the ids as `run -d` does (`run/user_ns.rs`, `run/foreground.rs`); a host root (`userns::host_root`) records none, as pod-enter changes nothing for it. Measured as uid 1000 on the cloud host: a foreground `run` of an image with `USER runner` gives the payload `uid_map` `1001 0 1` (1001 over podvm's 0, which is the caller); `--user 1001` gives the same; `--user 0` gives `0 1000 1`; `run -d` gives `1001 1000 1`; as host root the map stays the host's. `crates/podvm/tests/user_ns_detached.rs` holds the detached and the foreground cases as uid 1000.

**Partial (2026-11-16).** podcrates answered COORD-154 with `v0.62.0` (`Plan::userns_ids`, podcrates@T-786), and podvm takes the tag. `run -d` and `create` on the namespace rung resolve `--user`, else the image's `USER`, against the tree (`run/user_ns.rs`), record `PODVM_USERNS_IDS=UID:GID`, and the launcher gives the ids to the plan; uid 0 records nothing, and the banner no longer says that the run keeps uid 0 where the ids apply. Measured as uid 1000 on the cloud host: an image with `USER runner` (1001) gives the payload `uid_map` `1001 1000 1`; `--user 0` gives `0 1000 1`; `--user runner` and `--user 1001:1001` give 1001. The integration test `crates/podvm/tests/user_ns_detached.rs` holds the three cases as uid 1000 and skips for root. Remaining: the user namespace of a foreground `run` (T-1909, podvm's own code) still maps the caller to 0. pod-enter gives no change to a caller that is uid 0, and the payload of a foreground `run` is spawned from inside podvm's own namespace as uid 0 (measured: `run --rm --user 1001` gives `uid_map` `0 1000 1`). COORD-159 (#452) asks podcrates for a nested user namespace in that case.

# T-1929: `PODVM_MODE=rundir` and `tmpfs` refuse each tree of root: the device nodes of the fixups

**Source:** the proof of T-1786 (2026-11-16)
**Category:** enter
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** blocked

## Problem

As root, a run with `PODVM_MODE=rundir` or `PODVM_MODE=tmpfs` exits 125 before the payload, also as the first run of an image: "PODVM_MODE=rundir was forced but .../rootfs/dev/random: not a file, directory or symlink: refusing rather than staging a tree that is not the image's". Measured on the cloud host with `mirror.gcr.io/library/busybox:musl` and alpine:3.20. As uid 1000 the same modes refuse on the copy of the node. The fixups of the tree put real device nodes into `dev/` where `mknod(2)` passes, and `pod_enter::stage::copy_tree` refuses each node that is not a file, a directory or a link.

## Premise

`PODVM_MODE` is a switch for measurements; no user path forces these rungs. The device nodes are podvm's own, not the image's, so the refusal names the wrong cause.

## Approach

The staging skips the device nodes that podvm's fixups made (the set of `dev/` that the fixups record), and the staged root gets them again by the same fixups; or `copy_tree` of pod-enter recreates a character device where `mknod(2)` passes (a COORD issue to podcrates).

## Decision

-

## Prove

As root on the cloud host: `PODVM_MODE=rundir podvm run --rm IMAGE true` and `PODVM_MODE=tmpfs podvm run --rm IMAGE true` exit 0 for `mirror.gcr.io/library/busybox:musl`, and the payload reads 16 bytes from `/dev/urandom`.

## Blocker

`pod_enter::stage::copy_tree` refuses each node that is not a file, a directory or a link, and the staging is podcrates' code. COORD-174 (PodUtils/podvm#467) asks it to make a device again with `mknod(2)` where that passes, else to skip it and count it. Clears when a podcrates tag carries it.

# T-1930: `run -d` on the namespace rung has no PID namespace: a job sees and signals the processes of the host

**Source:** PodUtils/podrunner#200 (COORD-074 of podrunner, podrunner@T-318), COORD-164 (PodUtils/podvm#457), COORD-165 (PodUtils/podvm#458)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

podrunner's fourth QA measured `podvm run -d` of a user with no root on the namespace rung (`v0.2.0-beta.27`): from a step of a job, `ps -eo user,pid,args` lists each process of the host, `kill -TERM` ends a `sleep` of the same user on the host, and `pkill -KILL -f "^podrunner run"` ends the podrunner that started the unit. The entry of `pod-enter` makes a user namespace and a mount namespace and no PID namespace, and the `/proc` of the rung is a bind of the host's `/proc` (T-1758). A foreground `run` has podvm's own PID namespace (T-1909).

## Premise

A user with no root can make a PID namespace in the user namespace of the run. `CLONE_NEWPID` applies to the children of the caller, so the child of `pod-enter` makes it before it forks the payload: a change of `pod-enter`.

## Approach

podcrates gives a plan option for a PID namespace with its own `/proc` (COORD-164). podvm sets it for `run -d`, `create` and `start` on the namespace rung, names it on the banner, records the host pid as now, and makes `exec` and `cp` join the PID namespace of a running unit (the join of T-1910).

## Decision

-

## Prove

As uid 1000 on the cloud host: in `podvm run -d IMAGE sleep 600`, `podvm exec NAME ps -o pid,args` lists only the processes of the run, `podvm exec NAME kill -0 PID` of a host process fails, and `podvm stop NAME` ends the run.

## Done

**Done (2026-11-17).** podcrates `v0.66.0` (PodUtils/podcrates#216, podcrates@T-811) closes the descriptors of the caller in the init of the PID namespace (COORD-167, #460), and the launcher of `podvm-supervise` sets `Plan::pid_namespace` (podcrates `v0.65.0`, podcrates@T-806) for each run of the namespace rung that it starts (`run -d`, `create`, `start`). Measured as uid 1000 on the cloud host (alpine:3.20): `run -d --name pd IMAGE sleep 600` returns in under 1 s; `podvm exec pd ps -o pid,args` lists only the init (1), the payload (2) and the `exec`; `podvm exec pd kill -0 PID` of a `sleep` of the same uid on the host gives "No such process"; `stop` ends the unit with 143; `start` starts it again in a new PID namespace. In `v0.65.0` the same run -d did not return (COORD-167). The 28 integration test binaries of podvm and podvm-supervise pass as uid 1000.

Before: the blocker was the plan option of `pod-enter` (COORD-164, #457), then the descriptors that its init kept (COORD-167, #460).

# T-1934: A unit on the namespace rung has no usable pty: no `/dev/ptmx`, and the host's `/dev/pts`

**Source:** PodUtils/podrunner#216 (COORD-079 of podrunner, finding F3 of a user QA of podrunner `v0.1.0-beta.7`)
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

In a unit on the namespace rung, `openpty(3)` fails: Python's `pty.spawn` gives "out of pty devices", `script` fails, and the pager tests of pallets/click fail (24 of 2628). Measured as uid 1000 on the cloud host with `public.ecr.aws/docker/library/python:3.13-slim` and podvm `0.2.0-beta.31`, in a foreground run and in `run -d` (`inspect`: `namespace fresh-chroot`): `/dev/ptmx` does not exist, and `/dev/pts` is a bind of the host's devpts (`ptmxmode=000`), so an open of `/dev/pts/ptmx` gives EACCES. With user namespaces denied (the userland rung), `os.openpty()` passes (`/dev/pts/0`).

## Premise

pod-enter (podcrates `v0.68.0`) binds the host's `/dev/pts` on the `/dev/pts` of the image (`ns_optional`) and the host's nodes of `ns_dev_bind::DEVICES` (`null`, `zero`, `full`, `random`, `urandom`, `tty`); `ptmx` is not in that list. Measured: as uid 1000 in `unshare -Urm`, `mount -t devpts -o newinstance,ptmxmode=0666,mode=0620 devpts DIR` passes and `DIR/ptmx` opens.

## Approach

pod-enter mounts a new devpts instance (`newinstance,ptmxmode=0666,mode=0620`) on `/dev/pts` in its user namespace, and makes `/dev/ptmx` a bind of `/dev/pts/ptmx`; where the mount is refused, it binds the host's `/dev/ptmx` beside the host's `/dev/pts`, and writes its refused line. podvm then names the line in the run report, so that podrunner's `doctor --run` can name a unit with no pty.

## Decision

-

## Prove

As uid 1000 on the cloud host, `run -d` of `python:3.13-slim` and `podvm exec UNIT python3 -c 'import pty; pty.spawn(["true"])'` exit 0, `script -qec 'echo pty-ok' /dev/null` prints `pty-ok` (an image with `script`), and the foreground run gives the same.

## Done

**Done (2026-11-18).** podcrates `v0.70.0` (PodUtils/podcrates#222, podcrates@T-864 and @T-868) mounts a new devpts instance (`newinstance,ptmxmode=0666,mode=0620`) on the `/dev/pts` of the image and binds its `ptmx` on `/dev/ptmx` in the user namespace of `run -d`, `create` and `start`; where the kernel refuses the instance, pod-enter binds the host's `/dev/pts` and writes one line. A foreground run enters no user namespace of pod-enter (C21): podvm mounts the instance with the same options on the `/dev/pts` of the mount namespace of its pid 2, which pod-enter binds on the tree, and binds its `ptmx` on the `/dev/ptmx` of the tree (`userns::host::new_pts`). Measured as uid 1000 on the cloud host with `python:3.13-slim`: `run -d` then `podvm exec UNIT python3 -c 'import pty; pty.spawn(["true"])'` gives 0, and so does the payload of `run -d` itself (`logs`: `pty 0`); the foreground run gives `pty 0`, and `script -qec 'echo pty-ok' /dev/null` prints `pty-ok`; `run -t` still gives the payload `/dev/pts/0`. The host's mount table is the same before and after.

Before: blocked on COORD-176 (PodUtils/podvm#469). Remaining for a later entry: the run report does not name a unit whose devpts the kernel refused; the line of pod-enter on the standard error of the payload names it.

# T-1935: `run -d` on the userland rung: a raw path call of a dynamic program reaches the host

**Source:** PodUtils/podrunner#189 (COORD-066 of podrunner, its amendment of 2026-11-17), T-1740, T-1920; the answer is COORD-177 (PodUtils/podvm#470)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** partial

## Problem

On a host where only the userland rung works (a GitHub-hosted `ubuntu-24.04` runner, whose AppArmor restricts user namespaces), a unit of podrunner (`run -d`) runs a dynamic program on the loader family. `uv` and `ruff` persist a temporary file through `tempfile` and `rustix`, whose backend issues the `syscall` instruction itself; the interposer does not see that call, so the kernel resolves the path on the host. Measured as uid 1000 on the cloud host under a seccomp filter that denies `unshare` and `clone(CLONE_NEWUSER)`, with podvm `0.2.0-beta.31` (`inspect`: `userland fresh-chroot`): a glibc program with raw `renameat2(RENAME_NOREPLACE)`, `rename` and `link` gets -13 (EACCES) in `/root` and -2 (ENOENT) in `/tmp`; the libc calls of the same program work. T-1920 moved such a run onto the namespace rung where that rung works; here it does not.

## Premise

The syscall tier (`--podvm-tier=syscall`, T-1657, T-1731) gives each system call of a dynamic program to podvm's handler through Syscall User Dispatch, raw or not. `run/prepare.rs` refuses it for `run -d` and `create`, because its `/tmp` is a directory of the run that the end of the run removes.

## Approach

The syscall tier serves `run -d`, `create`, `start`, `exec` and `rm`: the `/tmp` of the run lives in the record of the container and goes with `rm`. Then, on a host where the userland rung is the best rung and Syscall User Dispatch arms, a detached run of a dynamic program takes the syscall tier (T-1740 measures the cost).

## Decision

-

## Prove

As uid 1000 on the cloud host under the seccomp filter of the Problem: `run -d` of `python:3.13-slim` with the glibc program of the Problem prints 0 for each raw call in `/root` and `/tmp`, `inspect` names the tier, `exec` sees the files, and `rm -f` leaves no directory of the run.

## Done

**Partial (2026-11-17, the tier for `run -d`).** `run -d --podvm-tier=syscall` and `create` keep the tier: `run/prepare.rs` no longer refuses them, and the `/tmp` of the container is the `/tmp` of its own tree (the handler caps it by its guest path, and `rm` removes the tree); `lifecycle/userland.rs` gives the tier the detached drive of the native-static rung; `exec` in such a container enters the tier (`lifecycle::syscall::exec_entry`, from the variable of the tier in the record); `inspect` has the field `Tier`. Two faults of the tier were found on the way and fixed in `podvm-dispatch`: the vfork path of the handler copied 256 bytes above the `sp` of the child, past the end of the 36 KiB stack of glibc's `posix_spawn` and into the guard page of Rust's alternate stack, so each spawn of a Rust program died of SIGSEGV (`uv venv`; now `window_of` stops at a page that `process_vm_readv` cannot read); and a `close_range` closed the memfd of the count of `/tmp`, so a child of Python's `subprocess` exited 127 (now `range_gaps` goes around it). Measured as uid 1000 on the cloud host under the seccomp filter of the Problem (`inspect`: `userland syscall`), `public.ecr.aws/docker/library/python:3.13-slim`: the glibc program prints 0 for raw `renameat2`, `rename` and `link` in `/root`, `/tmp` and a volume, in the main payload and through `exec`; `uv venv` passes 5 of 5, `uv pip install six` and `uv cache clean` pass; `ruff check` writes its cache. On the loader family the same `ruff check` gives "Failed to rename temporary cache file ... Permission denied (os error 13)", as in PodUtils/podrunner#189. podrunner's argv (`-w`, `--hostname`, `--label`, `--no-source-fixup`, `--report`, `-e`, two volumes) runs, and `stop` then `start` keeps the tier. The 35 integration test binaries of podvm, podvm-supervise and podvm-dispatch pass as uid 1000. Remaining: a detached run of a dynamic program takes the tier with no flag where the userland rung is the best rung and Syscall User Dispatch arms (after the measure of T-1740).

**Partial (2026-11-17, `:ro`).** Experiment 464 (T-1936) found that the tier refused a `:ro` volume ("--read-only and :ro need the read-only layer of the interposer"), and podrunner mounts its tool cache `:ro`. The tier runs the handler of the native-static rung, whose read-only layer holds a `:ro` volume (T-1901); `lifecycle/volume.rs` now gives it the tier too. Measured under the seccomp filter: in a foreground run and through `exec` of `run -d`, a read of `/ro/f` passes, a write and a `touch` give "Read-only file system", `/tmp` stays writable, and the host directory keeps its one file. Experiment 464 passes each clause on the tier, on both hosts.

# T-1939: The external QA of `0.2.0-beta.31`: `--device` and `run -d --rm`

**Source:** the external QA of `0.2.0-beta.31` (2026-11-17), issues 473, 474
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

- 473: `run --device HOSTFILE:/dev/x`: `ls /dev/x` gives ENOENT on both rungs, a read gives ENOENT on the userland rung, and a write (`>`, `>>`) creates a file in the image, while the host file does not change.
- 474: `run -d --rm` keeps the record and the tree of the container after its end, on both rungs.

## Premise

Measured as uid 1000 on `main`. 473: the gate of the interposer compares the pointer of the path (`create_calls.rs`), which the table of the userland rung has rewritten, and `device::answer` sends `O_CREAT`, `O_TRUNC` and an `O_APPEND` mismatch to the kernel. 474: the detached branch never reads `rm`; the launcher of pod-supervise has no removal at the end.

## Approach

473: the gate takes "the path matched no `-v` pair" in place of the pointer, a write of a regular device file goes to the host file, and `stat` answers from the descriptor. 474: the launcher removes the record and the tree after the end and the waiters (a COORD issue to podcrates), or `-d --rm` is refused by name until then.

## Decision

-

## Prove

As uid 1000 on both rungs: the repro of 473 reads `hostdev`, a write lands in the host file, and `ls -l /dev/hostdev` lists it; `run -d --rm IMAGE sleep 1` leaves no record and no tree after 3 s.

# T-1953: The external QA of `0.2.0-beta.31`, third batch: signals, rungs and runs

**Source:** the external QA of `0.2.0-beta.31` (2026-11-17), issues 512, 534, 535, 536, 547, 549, 550, 552, 553, 554, 557, 558, 560, 561, 562, 565, 566, verified by T-1950
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** partial

## Problem

- Signals: SIGPIPE ends a foreground run before its record and its `--rm` (534); SIGINT is ignored in a run of the user namespace (535); SIGHUP leaves the payload and the reaper (562); SIGTERM to a Windows or DOS guest leaves QEMU (566).
- Rungs: `-w /tmp/NEW` fails on the namespace rung (512); an image with no `/tmp` falls back to chroot and the record says namespace (547); `--unsafe-host-paths` does nothing on the namespace rung (553); `create --podvm-tier=emulate` gives a false chroot refusal (554); `--strict=all` refuses each run for the mtab of `/proc` (557); the probe cache never matches (558); `/tmp/execprobe` of the census (561); `-v` of a file over a directory of the image (565).
- Files: the run is root of its user namespace before it reads `--env-file` and writes `--report`, so it reads a file that the caller cannot (552); the log of a container follows a symbolic link (536); `logs -f` ends at a `restart` (550); `install-names` makes `podman` and then refuses `docker` (549); the source fixup follows redirects (560).

## Premise

T-1950 verified each signal and rung item as uid 1000, on the namespace rung and under `scripts/dev/nouserns.c`.

## Approach

podvm's items in podvm; pod-enter, pod-supervise, pod-rootfs, pod-hostprobe and pod-guest items as a COORD issue to podcrates (COORD-182). 552 first: a read of the caller's files before podvm enters its user namespace.

## Decision

-

## Prove

`cargo test --release -p podvm --bin podvm` passes, and for each issue of this entry its repro, on the rung of the report, gives the fix.

## Done

**Partial (2026-11-17).** 534: the pump of stdout blocks SIGPIPE in its own thread (the payload keeps the default), and at a broken pipe it stops and closes its end, so the payload gets the broken pipe as in a pipeline with no podvm. As uid 1000: `run --rm --name sp alpine sh -c 'yes hello' | head -1` prints `hello`, podvm exits 141 in 0.3 s, and `ps -a` lists nothing; with no `--rm` the record is `Exited (141)` (before: podvm died, the record stayed `Created`). COORD-182 (#569) asks podcrates for the items of pod-enter, pod-supervise, pod-rootfs, pod-hostprobe and pod-guest. 552: `--env-file` and `--report` keep the caller's own rule in podvm's user namespace (`run/parse/access.rs`: on a file of the caller, its owner bits decide); as uid 1000 a mode-000 env file of the caller and a report into a directory of mode 0500 are refused before the payload. 553: `--unsafe-host-paths` is refused by name where no loader family runs (a rung with its own root). 565: a `-v` whose host and guest sides differ in kind names both paths and both kinds, before the bind. 549: `install-names` decides the refusal of `docker` before any link, and names `install-names podman`. 535: in podvm's user namespace a SIGINT or SIGQUIT that a process sent (`si_code` 0 or below) is forwarded to the payload, and one of the terminal (SI_KERNEL) is not, as the terminal reached the group already (`userns/sigrelay.rs`; where `sigaction` fails, the old rule stays). As uid 1000, a SIGINT to podvm alone made the payload's trap exit 7. A `kill -INT` to the whole group still reaches the payload twice. 554: a detached entry of the emulate tier is refused with a line that names the tier, not the text of a denied chroot. Remaining: 560, 566's podvm half.

# T-1956: The donor study: syscall tiers

**Source:** PodUtils/podvm#573, the donor study of 2026-11-17
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** open

## Problem

A raw `syscall` of a dynamic program reaches the host on the userland rung (T-1935, T-1740). The handler sends each unlisted call to the host, and no test lists the path calls. io_uring passes the handler when no broker hold is on. A host that refuses the Syscall User Dispatch prctl gets exit 125 (T-1648).

## Premise

Items that serve podrunner (rootless CI units) come first. The donor evidence is static analysis. Re-verify each claim in the tree before work.

## Approach

1. FA-03: answer ENOSYS for io_uring on the syscall tier when no hold is on.
2. FA-02/TH-24: add one coverage table test of path-taking syscall numbers.
3. FA-01/TH-01: add a seccomp-TRAP rung that traps only path calls; use it where the SUD prctl is refused.
4. TH-02: study a user-notification rung; the owner rules first (T-0606).
5. TH-03/FA-07: pin each volume path with an O_PATH descriptor.
6. FA-04, FA-05, FA-06, TH-07, TH-56, TH-06, TH-22, TH-08 as the issue lists them.

## Decision

-

## Prove

A table test lists each path-taking kernel syscall number of the dispatch table and the interposer table, each mapped, refused or exempt with a reason; a raw `io_uring_setup` on the syscall tier with no hold returns ENOSYS and libuv falls back to epoll.

# T-1957: The donor study: limits and cgroup kill

**Source:** PodUtils/podvm#574, the donor study of 2026-11-17
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

No code writes `cgroup.kill` or `memory.oom.group`, so an OOM kill ends one process and leaves a half-dead job. The syscall tier counts `/tmp` growth only for write-family calls. `--cpu-shares` and related flags have no mapping. The `wasmtime` profile passes no limit.

## Premise

T-1747 gives the cgroup2 leaf. The donors show how to end a whole job and how to count bytes at each boundary.

## Approach

1. FG-01: write `memory.oom.group=1` and `cgroup.kill` where a leaf exists.
2. TH-20: count `/tmp` bytes for mmap, sendfile, splice, copy_file_range and rename.
3. FH-01: pass fuel, memory and time limits to the `wasmtime` profile.
4. FG-03, FA-08, FH-02, FH-04 as the issue lists them.

## Decision

-

## Prove

On a host with a delegated cgroup2 subtree, a shell that forks and reparents a sleeper ends with no sleeper left after `rm -f`; mmap growth in `/tmp` gives ENOSPC at the cap.

# T-1972: `--podvm-volume-base` holds on the namespace rung only, and job containers of podrunner run on the userland and supervise rungs

**Source:** PodUtils/podrunner#300 (COORD-090 of podrunner, ask 3, podrunner@T-403)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** L
**Status:** partial

## Problem

podrunner starts job containers (the route of the container hooks) where podvm gives volumes to them, which includes the userland and supervise rungs (a host with no user namespace). There `--podvm-volume-base` is a refusal, so the race of PodUtils/podrunner#277 stays: a step of the job makes a path under the runner root a symbolic link after podrunner checked it and before podvm opens it.

## Premise

On the namespace rung the flag holds (T-1945): pod-enter binds the descriptor. The userland rung maps a guest path to a host path at each call of the interposer, and the supervise rung resolves each path of a notified call in the supervisor; neither holds one descriptor of a tree today. A host path `/proc/self/fd/N/REST` of an inherited `O_PATH` descriptor resolves under the opened directory and ignores a later link at its path, but a program that closes each descriptor (`close_range`, `close_fds` of Python) loses it.

## Approach

1. The namespace rung: tell podrunner that the route of containers with the flag holds there since `0.2.0-beta.32`, where the host gives user namespaces.
2. The supervise rung: the supervisor opens each source under the base with `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)` at the start, keeps the descriptors, and resolves each notified path of a volume with `openat2` relative to its descriptor (`RESOLVE_BENEATH`), so that no link under the base is followed.
3. The userland rung: the same check at the start, and each mapping of the interposer resolves the rest of a path under a descriptor that the interposer holds in a reserved range and restores after a `close_range`; measure the cost before the default.

## Decision

-

## Prove

As uid 1000 under the seccomp filter that denies user namespaces (`scripts/dev/nouserns.c`), on the supervise and the userland rung: with `--podvm-volume-base BASE -v BASE/w:/w`, a link that the host makes at `BASE/w/sub` to a directory out of BASE after the start gives the payload `ELOOP` or `EXDEV` for `/w/sub/f`, never the file out of BASE; a plain file under `BASE/w` reads and writes.

## Done

**Partial (2026-11-18, the syscall tier).** On a host with no user namespace, podrunner's units take `--podvm-tier=syscall` (T-1935), which `--podvm-volume-base` now serves: the handler of `podvm-dispatch` opens each volume under `PODVM_VOLUME_BASE` with `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)` (`vols::beneath`), where it opened the canonical path before, and resolves each later path under that descriptor with `RESOLVE_IN_ROOT` as before; `run::binds::refuse_base_off_binds` refuses only the loader family now. The "supervise" selection enters through the chroot sequence (`pod_enter::spawn_selected`), so no entry of a supervise rung holds volumes of its own. Measured as uid 1000 under `scripts/dev/nouserns.c` (`inspect`: `userland syscall`) with `docker.io/library/python:3.13-slim`: the loader family refuses the flag with exit 125 and one line; `-v BASE/work:/w` writes and reads; `-v BASE/link:/w` (a link out of BASE) exits 125; in a `run -d`, a link that the host makes at `BASE/work/sub` to a directory out of BASE after the start gives the payload `No such file or directory` for `/w/sub/f`, not the file out of BASE. `a_volume_opens_under_its_base_with_no_link` pins the open. Remaining: the loader family (step 3 of the Approach).

# T-1973: On the syscall tier, a payload cannot connect to a Unix socket of a volume

**Source:** PodUtils/podrunner#301 (COORD-091 of podrunner, podrunner@T-261)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

podrunner's units on a host with no user namespace take `--podvm-tier=syscall` (T-1935). In such a unit, `curl --unix-socket /run/podrunner/ctl.sock` fails with "Couldn't connect to server": the handler maps the path of each file call, but it passed `connect(2)` and `bind(2)` of an `AF_UNIX` address as given, so the kernel looked up the guest path on the host. podrunner removed the flag again.

## Premise

A socket path in `struct sockaddr_un` holds at most 108 bytes, and the kernel resolves it as a path, also through `/proc/self/fd/N`.

## Approach

The handler takes `connect` and `bind` of an `AF_UNIX` address with a path: it resolves the directory of the socket in the root or its volume (`calls::split`, as each path call), and gives the kernel `/proc/self/fd/N/NAME`. An abstract or unnamed address, and a path of the host, pass as they are.

## Decision

-

## Prove

As uid 1000 under `scripts/dev/nouserns.c`, with a host process that listens on `D/ctl.sock`: `run --podvm-tier=syscall -v D:/run/podrunner IMAGE` connects to `/run/podrunner/ctl.sock` and reads the answer; `exec` of a `run -d` unit does the same; a socket that the payload binds in `/tmp` takes a connection and is not in the `/tmp` of the host.

## Done

**Done (2026-11-18).** `podvm-dispatch/src/sock.rs`, called first in `calls::path_call`. Measured as uid 1000 under `scripts/dev/nouserns.c` (`inspect`: `userland syscall`) with `docker.io/library/python:3.13-slim`: the payload of a foreground run and an `exec` of a `run -d` unit connect to the host's socket in the volume and read its answer; the payload binds `/tmp/own.sock`, connects to it and reads `own-ok`; the host has no `/tmp/own.sock`. `a_socket_path_of_the_guest_becomes_a_short_proc_path` pins the address and the refusal of a name that does not fit.

# T-1975: On the syscall tier, a link inside a volume that names an absolute path or leaves the volume does not resolve

**Source:** OR-06 of the second outside pass of 2026-11-18 (T-1974)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

The handler resolves a path at the root or at its volume with `RESOLVE_IN_ROOT`, so a link in a volume (or in the `/tmp` of a foreground run, a directory of the store) that names `/usr/bin/python3`, or `../x` out of the volume, resolves against the volume. `python -m venv` in such a directory makes `v/bin/python` that does not run: the venv of a CI job in its work directory (a volume of a unit) breaks.

## Premise

A prefix of a path that holds no link folds `.` and `..` by its text.

## Approach

Where a resolution at a base fails with ENOENT, ENOTDIR or ELOOP and the run has a volume, the handler walks the guest path one name at a time with the rules of the guest (`follow::walk`): each name opens with `O_NOFOLLOW` at its base; a link starts again at `/` (absolute) or at its directory (relative); `.` and `..` fold by their text; at most 40 links. Then it tries the new path. A path that resolves at once costs nothing more.

## Decision

-

## Prove

As uid 1000 under `scripts/dev/nouserns.c`, `--podvm-tier=syscall`: `cd /tmp; ln -s /etc/hostname h; cat h` reads the file; `python3 -m venv v --without-pip; v/bin/python` runs in `/tmp` and in a volume; a link `../w/v/bin/python` in the volume `/w` runs; a missing name still gives ENOENT.

## Done

**Done (2026-11-18).** `podvm-dispatch/src/follow.rs`, called from `path::open_in`. Measured as in the Prove with `docker.io/library/python:3.13-slim`: `link-ok`, `venv-tmp-ok`, `venv-vol-ok /w/v`, `rel-ok`, and `ls /w/nope` gives "No such file or directory". The walk opens only at the root and at the volumes (`path::base_of`), never at the host.

# T-1976: The handler of the syscall tier overflows a small alternate signal stack of the payload

**Source:** gate run 44312445769 (2026-11-18, `bab0ea1`): the test `exec_loads_the_next_program_in_the_root` died of SIGSEGV on CI twice, and passed on the cloud host
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

The SIGSYS handler has `SA_ONSTACK`, so it runs on the alternate signal stack of the payload's thread. Rust's std gives each thread one of `max(SIGSTKSZ, AT_MINSIGSTKSZ)` bytes: 8 KiB on a CPU without AMX, more on a CPU with AMX (the cloud host). The kernel puts its frame (about 3 KiB with AVX-512) on the same stack. Measured with the probe verb `exec-alt SIZE`: before the fix, the exec path needed more than 8 KiB in a debug build and more than 4 KiB in a release build. T-1975 added a few hundred bytes to `path::open_in`, and the debug test went over the edge on CI.

## Premise

`signals::sigaltstack` arms each alternate stack of the payload with `SS_AUTODISARM`, so while the handler runs on it, the kernel delivers a nested signal on the current stack, not at the top of the alternate stack.

## Approach

`exec::execveat` runs its body on an anonymous map of 256 KiB with a guard page (an exec is rare; a successful exec drops the map). Where the map fails, it runs on the stack that it has. The remainder: the other calls of the handler, which use about 5 KiB in a debug build before an exec, move to a stack of their own too, from a pool as `pool::Buf`, so that a payload with an alternate stack of `MINSIGSTKSZ` plus the frame of the kernel is safe.

## Decision

-

## Prove

`cargo test -p podvm-dispatch`: `exec_runs_on_a_small_alternate_signal_stack` (8 KiB and 16 KiB) passes in a debug build. The sweep of `exec-alt` from 4 KiB to 64 KiB: after the fix, a debug build passes from 6 KiB and a release build from 5 KiB (before: from 9 KiB and from 5 KiB). The remainder: each call of the handler passes on an alternate stack of 4 KiB.

## Done

**Done (2026-11-18).** The exec path first ran on a map of its own (`exec::execveat`, 256 KiB). Then gate 44345876471 died of SIGSEGV in the probe's `write` (a rename) on CI: the same fault on another path. Now each call of the handler that returns runs on a stack of a pool (`stacks.rs`: 64 stacks of 128 KiB, each above a guard page, taken with a compare-and-swap; the payload's stack where none is free). `clone`, `fork`, `vfork`, the exits, `munmap` (it reads the stack that it runs on), `rt_sigreturn` and an exec stay on the stack that they have. The probe takes `PODVM_PROBE_ALT=SIZE`: each verb on an alternate stack of SIZE bytes. `each_call_runs_on_a_small_alternate_signal_stack` runs exec, write and rename, a link, `cat-zx`, spawn, fork and a thread on 8 KiB, with and without the tier: it passes, and it fails with the pool off. As uid 1000 under `scripts/dev/nouserns.c` on the syscall tier: podrunner's link case of T-1977 reads the file, and `python:3.13-slim` makes a venv in `/tmp` whose Python runs 8 threads that each spawn a process.

# T-1977: On the syscall tier, a relative path of a glibc program through an absolute link resolves on the host

**Source:** PodUtils/podrunner#303 (COORD-093 of podrunner); answer COORD-192 (#588)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

podrunner's `links.rs` failed under `--podvm-tier=syscall` on its runner image (Ubuntu): `cat link`, with `link -> /NAME` in the work directory and `/NAME` in the rootfs, gave ENOENT. `cat /podrunner/link` worked, and a static program or an alpine (musl) program worked with the same relative path.

## Premise

A descriptor is an `int`: the kernel reads the low 32 bits of its register. glibc fills the register of `AT_FDCWD` as `0x00000000FFFFFF9C`; musl and Rust's std give it sign-extended.

## Approach

`path::fd_link` compared the whole register with `AT_FDCWD`, so for glibc it read `/proc/self/fd/4294967196`, which does not exist, and `path::target` passed the call to the kernel, which resolved the link against the host's root (`strace`: `readlinkat(AT_FDCWD, "/proc/self/fd/4294967196") = -ENOENT`). `fd_link` now takes the descriptor as the kernel does (`fd as i32 as i64`). Each path call of the handler reads its descriptor through it.

## Decision

-

## Prove

`cargo test -p podvm-dispatch`: `a_relative_path_through_an_absolute_link_resolves_in_the_root` (the probe verb `cat-zx` calls `openat` with the register as glibc fills it; it fails before the fix and passes after it). As uid 1000 under `scripts/dev/nouserns.c`, `run -d --podvm-tier=syscall -w /podrunner public.ecr.aws/ubuntu/ubuntu:24.04`, a host-made `/NAME` and `podrunner/link -> /NAME`: the payload's `cat link` reads the file.

## Done

**Done (2026-11-18).** The test above fails before the fix and passes after it. Measured on the cloud host with `mirror.gcr.io/library/ubuntu:24.04`: podrunner's case reads the body; `cat`, `grep` and `stat -L` through a relative absolute link pass in a directory of the rootfs and from `/`; `cd /tmp; ln -s /etc/hostname h; cat h` passes.

# T-1978: On the syscall tier, a child of posix_spawn cannot run a link in a volume

**Source:** OS-01 of the third outside pass of 2026-11-18 (T-1980)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`python3 -m venv` with pip failed on the syscall tier: `[Errno 9] Bad file descriptor: '/tmp/venv/bin/python3'`. `subprocess.run` (glibc `posix_spawn`) of a link in `/tmp` or in a `-v` volume gave EBADF; `os.execv` and a shell exec of the same link worked.

## Premise

Python's `posix_spawn` with `close_fds=True` runs `close_range(3, ~0)` in the child before its exec. The handler keeps the descriptor of each volume above the payload's usual range.

## Approach

The guard of `close`, `dup2`, `dup3` and `close_range` kept the root's descriptor and the count of `/tmp` only, so the child closed the descriptors of the volumes, and the exec's `openat2` at a volume gave EBADF. `handler::kept` now names the root's, the count's and each volume's descriptor; `range_gaps` closes the parts of the range around each of them.

## Decision

-

## Prove

`cargo test -p podvm-dispatch`: `close_range_goes_around_the_kept_descriptors` (a table of ranges with a root, a count and volumes). As uid 1000 under `scripts/dev/nouserns.c`: `podvm run --rm --podvm-tier=syscall -v W:/vol quay.io/almalinuxorg/almalinux:9 python3 -c 'import os,subprocess; os.symlink("/usr/bin/python3","/tmp/l1"); os.symlink("/usr/bin/python3","/vol/l2"); print(subprocess.run(["/tmp/l1","-c","1"]).returncode, subprocess.run(["/vol/l2","-c","1"]).returncode)'` prints `0 0`.

## Done

**Done (2026-11-18).** The Prove prints `spawn 0 0` with `quay.io/almalinuxorg/almalinux:9`. With `mirror.gcr.io/library/python:3.13-slim` (the case of the pass): the spawn of `/tmp/l1 -> /usr/local/bin/python3` exits 0, and `python3 -m venv` with pip gives a `pip --version` of 26.2.2 in `/tmp/venv` and in `/vol/venv`.

# T-1979: On the default userland rung, a link that the payload makes in a volume holds the host path of the tree of the run

**Source:** OS-02 of the third outside pass of 2026-11-18 (T-1980)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

On the loader family of the userland rung, `ln -s /etc/hostname /vol/a1` stored `a1 -> $HOME/.local/share/podvm/trees/ID/rootfs/etc/hostname` on the host. The tree goes with the container, so the link dangled on the host and in each later run, and it named the store's path. A venv made in a volume in one run did not run in the next.

## Premise

The interposer resolves a guest target of a link when the payload uses it (podrunner's `links.rs` test): a host-made link `x -> /etc/hostname` in a volume reads, stats, runs and spawns in the run.

## Approach

`symlink` and `symlinkat` of the interposer rewrote each absolute target through the table, so that the kernel could follow it. A link whose path lands in a volume (a pair whose guest side is not `/`, `map::in_volume`) now keeps the guest's target; a link in the tree of the run keeps the rewrite, as before.

## Decision

-

## Prove

The interposer tests: `a_volume_is_a_pair_whose_guest_side_is_not_the_root`. As uid 1000 under `scripts/dev/nouserns.c` (the loader family): `podvm run --rm -v W:/vol quay.io/almalinuxorg/almalinux:9 sh -c 'ln -s /etc/hostname /vol/a1; python3 -m venv --without-pip /vol/venv'` leaves `W/a1 -> /etc/hostname` on the host, and `podvm run --rm -v W:/vol quay.io/almalinuxorg/almalinux:9 /vol/venv/bin/python -c 1` exits 0.

## Done

**Done (2026-11-18).** The Prove holds with `quay.io/almalinuxorg/almalinux:9` (`a1 -> /etc/hostname`; the second run prints its `sys.prefix`, `/vol/venv`). With `mirror.gcr.io/library/python:3.13-slim`: the host shows `a1 -> /etc/hostname`, `rel -> /usr/bin` (a relative link path) and `venv/bin/python3 -> /usr/local/bin/python3`; the second run prints `venv-run2-ok /vol/venv`; a link in `/tmp` of the tree still reads in its run.

# T-1981: On the userland rung, a program whose PT_INTERP lies past the first 64 KiB is refused (Fedora 42)

**Source:** found on the gate of beta.34 (2026-11-18), on the way to the Prove of T-1978; COORD-193 (#589) asks podcrates
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

On a host that denies `chroot(2)` and user namespaces, `podvm run quay.io/fedora/fedora:42 sh -c true` exits 125: "the PT_INTERP path lies past the 65536 bytes read, so the reader cannot tell a static file from a dynamic one". Fedora 42's `/usr/bin/bash` has `PT_INTERP` at file offset `0xf2000`. The fault is older than `0.2.0-beta.33`.

## Premise

`pod_enter::abi` (its `Elf::interp`, podcrates `v0.71.0`) reads a head of the file and answers "unknown" where the path of `PT_INTERP` lies past it.

## Approach

podcrates reads the path at its `p_offset` from a file source (COORD-193, #589; podcrates `v0.72.0`, podcrates@T-944: `memfd::eligible_payload`, `ladder::Payload::interp`). podvm classifies each payload with the open file: `lifecycle::script::Followed` keeps the `Payload`, and `lifecycle::entry`, `lifecycle::userland` and `ladder::memfd_payload` call `eligible_payload`.

## Decision

-

## Prove

As uid 1000 under `scripts/dev/nouserns.c`: `podvm run --rm quay.io/fedora/fedora:42 sh -c 'echo ok'` prints `ok` and exits 0.

## Done

**Done (2026-11-18).** podvm takes podcrates `v0.72.0`. Measured as in the Prove with `quay.io/fedora/fedora:42`: `sh -c 'echo ok'` prints `ok` and exits 0; the same with `--podvm-tier=syscall`; alpine still runs.

# T-1982: `run -d` with `--podvm-confine` on the syscall tier cannot start its payload

**Source:** PodUtils/podrunner#304 (COORD-094 of podrunner); answer COORD-194 (#590)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`run -d --podvm-tier=syscall --podvm-confine SPEC` exited 125: "the payload could not be started: execve of the userland entry argv failed with EACCES (13)". A foreground run with the same flags started. podrunner's self-test of the confine layer failed under the tier for this reason: its held run never started.

## Premise

On the tier, podvm starts the payload by an exec of podvm itself, so the layer must give podvm's own binary read and execute.

## Approach

`lifecycle::confine::decide` gave podvm's binary, the emulator and musl's raw reads to a foreground run; the launcher of `run -d` built the layer with `confine_host_spec`, which had none of them. Both now take `podvm_supervise::table::userland_host_spec`.

## Decision

-

## Prove

`cargo test -p podvm-supervise`: `the_userland_layer_runs_podvm_itself_and_the_emulator`. As uid 1000 under `scripts/dev/nouserns.c`: `podvm run -d --podvm-tier=syscall -w /podrunner --podvm-confine 'read=/;write=/podrunner,/tmp;exec=/podrunner,/tmp,/usr,/bin' public.ecr.aws/ubuntu/ubuntu:24.04 sleep 60` exits 0, and `podvm exec` into it cannot read `/proc/PID/environ` of a host process of the same uid.

## Done

**Done (2026-11-18).** Measured as in the Prove with `mirror.gcr.io/library/ubuntu:24.04`: with the layer, `run -d` exits 0, the payload writes its own file, and neither it nor `exec` reads the host process's canary (`Permission denied`); with no layer, both read it. A foreground run on the tier and on the default rung give the same answers.

# T-1983: On the syscall tier, `--podvm-confine exec=` is not enforced

**Source:** OT-01 of the fourth outside pass of 2026-11-18 (the gate of beta.35, T-1989)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

With `--podvm-tier=syscall`, `--podvm-confine 'write=/tmp;exec=/usr/bin/sh,/usr/bin/cp'` lets `cp /usr/bin/id /tmp/i2; /tmp/i2` run (exit 0), in `run`, `run -d` + `exec` and `create` + `start`. The loader family denies it (the interposer checks the exec table), and the namespace rung denies it (Landlock). The fault is in `0.2.0-beta.34` and before.

## Premise

On the tier each program runs through podvm itself, which the layer allows, so Landlock never checks EXECUTE on the program. `pod-enter` writes the exec table of the layer (`PODVM_EXEC_HOST`) at the spawn.

## Approach

The handler keeps the exec table of the layer from the environment of its entry (`exec::table`; a table that does not fit is a refusal, 125). `exec::check` resolves the program's host path (the link of its descriptor) and answers EACCES where `pod_confine::exec::allows_in` (the rule of `pod-enter` and the interposer) does not cover it. On each exec the handler drops the payload's `PODVM_EXEC_HOST` and sets its own, so `env -i` neither loses the table nor widens it.

## Decision

-

## Prove

As uid 1000 under `scripts/dev/nouserns.c`: `podvm run --rm --podvm-tier=syscall --podvm-confine 'write=/tmp;exec=/usr/bin/sh,/usr/bin/cp' public.ecr.aws/ubuntu/ubuntu:24.04 sh -c 'cp /usr/bin/id /tmp/i2; /tmp/i2'` exits 126 with "Permission denied".

## Done

**Done (2026-11-18).** `the_exec_table_of_the_confine_layer_holds_on_each_exec` passes (a program in the table runs; one out of it and each one under an empty table get EACCES; with no table each runs). Measured as in the Prove with `mirror.gcr.io/library/ubuntu:24.04`: the copy of `id` in `/tmp` gives "Permission denied" (126), `/usr/bin/echo` of `exec=` runs, `env -i /tmp/i2` and `PODVM_EXEC_HOST=/ /tmp/i2` give 126; the same through `run -d` + `exec` and `create` + `start`; with no `--podvm-confine` the copy runs.

# T-1984: `--podvm-confine exec=` is bypassed through the dynamic loader

**Source:** OT-02 of the fourth outside pass of 2026-11-18 (the gate of beta.35, T-1989)
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

`/lib64/ld-linux-x86-64.so.2 /tmp/prog` runs a program that `exec=` denies when started directly, on each rung (`direct=126`, `ldso=0`). The fault is in `0.2.0-beta.34` and before.

## Premise

Landlock checks EXECUTE at `execve`, not at `mmap(PROT_EXEC)`, and the layer lets each run read the files of the dynamic loader.

## Approach

Where podvm sees the exec (the interposer, the handler of the tier), an exec of the loader with a program argument takes the exec rule of that program. Where only Landlock holds (the namespace and chroot rungs), the manual names the limit.

## Decision

Call C25 (`docs/decisions.md`, 2026-11-18): the help of `--podvm-confine` names the limit, and the entry goes from P1 to P2 for the enforcement where podvm sees the call.

## Prove

As uid 1000: `podvm run --rm --podvm-confine 'write=/tmp;exec=/usr/bin/sh,/usr/bin/cp' public.ecr.aws/ubuntu/ubuntu:24.04 sh -c 'cp /usr/bin/id /tmp/m; /lib64/ld-linux-x86-64.so.2 /tmp/m'` exits 126 on the userland rung and the syscall tier; the manual of `--podvm-confine` names what holds on the other rungs.

# T-1985: A payload under `--podvm-confine` changes the mode and the times of a file that no rule names

**Source:** OT-03 of the fourth outside pass of 2026-11-18 (the gate of beta.35, T-1989)
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

With `read=/sc;write=/tmp` and `-v H:/no`, the payload changes `/no/s` from mode 600 to 666 and resets its mtime, on each rung; reads, writes, `rm`, `mv` and `link` are denied. The fault is in `0.2.0-beta.34` and before.

## Premise

Landlock governs no change of mode, owner or times (no access right for them up to ABI 6).

## Approach

Where podvm sees the call (the interposer, the handler of the tier), `chmod`, `chown`, `utimensat` and their kin of a path that no `write=` rule names give EACCES. The seccomp filter of the layer cannot judge a path; on the namespace and chroot rungs the manual names the limit and the advice of a `:ro` volume.

## Decision

Call C25 (`docs/decisions.md`, 2026-11-18): the help of `--podvm-confine` names the limit, and the entry goes from P1 to P2 for the enforcement where podvm sees the call.

## Prove

As uid 1000 under `scripts/dev/nouserns.c`: `podvm run --rm --podvm-confine 'read=/sc;write=/tmp' -v H:/no -v S:/sc public.ecr.aws/ubuntu/ubuntu:24.04 sh -c 'chmod 666 /no/s'` exits 1 with "Permission denied", and the mode of `H/s` stays 600.

# T-1986: On the userland rung, `dnf` of Fedora 42 cannot write its state under `/root`

**Source:** OT-04 of the fourth outside pass of 2026-11-18 (the gate of beta.35, T-1989)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`podvm run --rm quay.io/fedora/fedora:42 dnf --version` exits 1 on the userland rung and the syscall tier: "cannot create directories: Permission denied [/root/.local/state]". `/root` is mode 0550 in the image, and the extraction makes it uid 1000's with that mode. The fault is in `0.2.0-beta.34` and before.

## Premise

On a rung with no user namespace the payload is uid 1000, the owner of the tree; a directory of mode 0550 denies its owner a write.

## Approach

On the userland rung, `run::home::writable` gives the home of the run (`HOME` of the environment, else `/root`, as `pod-enter` sets it) the owner's write bit, in the container's own copy of the tree (T-1709). A home that is a link, under a link, or a path with `..` stays as it is; each other mode stays the image's.

## Decision

-

## Prove

As uid 1000 under `scripts/dev/nouserns.c`: `podvm run --rm quay.io/fedora/fedora:42 dnf --version` exits 0, and with `--podvm-tier=syscall`.

## Done

**Done (2026-11-18).** `the_home_of_the_run_gets_the_owners_write_bit_and_nothing_else` passes. Measured as in the Prove with `quay.io/fedora/fedora:42`: `dnf --version` exits 0 on the userland rung and with `--podvm-tier=syscall` (`/root` is `drwxr-x---` in the tree of the run); on the namespace rung `/root` keeps `dr-xr-x---` and `dnf` works as before.

# T-1987: On AlmaLinux 9, `ls -l` dies with SIGSEGV in the interposer's `lgetxattr`

**Source:** OT-05 of the fourth outside pass of 2026-11-18 (the gate of beta.35, T-1989)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`podvm run --rm quay.io/almalinuxorg/almalinux:9 ls -l /etc/hostname` exits 139 on the namespace and the userland rung; the kernel names `interpose.so`, and `LD_DEBUG=bindings` ends at `lgetxattr` from `libattr.so.1`. The syscall tier, `ls -1` and `stat` work. The fault is in `0.2.0-beta.34` and before.

## Premise

AlmaLinux 9's `libattr.so.1` exports its own `lgetxattr` (and the other calls of an attribute), which calls the symbol `lgetxattr` again. The interposer's wrapper took that one as its "next" symbol (`RTLD_NEXT`), which bound back to the wrapper: a recursion until the stack ended.

## Approach

The eight wrappers of an attribute (`getxattr`, `lgetxattr`, `listxattr`, `llistxattr`, `setxattr`, `lsetxattr`, `removexattr`, `lremovexattr`) map the path and make the system call themselves (`path_syscall!`, `raw_calls::forward`), with no next symbol.

## Decision

-

## Prove

As uid 1000: `podvm run --rm quay.io/almalinuxorg/almalinux:9 ls -l /etc/hostname` exits 0, under `scripts/dev/nouserns.c` too.

## Done

**Done (2026-11-18).** Measured as in the Prove with `quay.io/almalinuxorg/almalinux:9`, on the namespace rung and under `scripts/dev/nouserns.c`: `ls -l /etc/hostname` exits 0; `getfattr -d`, `setfattr -n user.a` and `getfattr -n user.a` work. The interposer tests (100) pass.


# T-1988: On the syscall tier, a volume whose guest path is not in the image cannot be stat-ed

**Source:** OT-06 of the fourth outside pass of 2026-11-18 (the gate of beta.35, T-1989)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

With `--podvm-tier=syscall -v H:/pkg` and no `/pkg` in the image: `ls -d /pkg`, `stat /pkg`, `cp -r /pkg` and `find /pkg` give ENOENT, while `ls /pkg`, `cd /pkg` and `stat /pkg/x` work. A guest path that the image has works. The fault is in `0.2.0-beta.34` and before.

## Premise

`path::parent` split a path into its parent and its last name; for a volume's own guest path, the parent was the root of the image, which has no such name. `open` resolves the whole path at the volume, so `ls /pkg` and `cd /pkg` worked, and `statx` with `AT_SYMLINK_NOFOLLOW` (coreutils' `ls -d` and `stat`) went through the parent.

## Approach

`path::parent` takes a path that equals a volume's guest path as the volume itself, as for `.`: the parent is the volume's directory and the name is `.`. `mkdir` of it gives EEXIST.

## Decision

-

## Prove

As uid 1000 under `scripts/dev/nouserns.c`: `podvm run --rm --podvm-tier=syscall -v H:/pkg public.ecr.aws/ubuntu/ubuntu:24.04 stat -c %F /pkg` prints `directory`.

## Done

**Done (2026-11-18).** `a_volume_whose_guest_path_is_not_in_the_root_stats_as_a_directory` (the probe verb `kind` calls `statx` as coreutils does) fails before the fix and passes after it. Measured as in the Prove with `mirror.gcr.io/library/ubuntu:24.04`: `ls -d /pkg`, `stat -c %F /pkg` (`directory`), `test -d /pkg`, `cp -r /pkg /tmp/p2` and `find /pkg` work; `mkdir /pkg` gives "File exists".


# T-1990: On the syscall tier, a dynamic program sometimes fails to start with "errno 11"

**Source:** PodUtils/podrunner#308 (COORD-098 of podrunner), items 1 and 2; also seen twice by the fourth outside pass (T-1989)
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** partial

## Problem

On GitHub's `ubuntu-24.04` runner, `--podvm-tier=syscall` gave "podvm: native-static rung: .../bin/node: /lib64/ld-linux-x86-64.so.2: errno 11 (TODO/enter.md T-1652)", exit 127: for Node in a volume each time, and for `/usr/bin/bash` once.

## Premise

errno 11 is EAGAIN. `openat2` with `RESOLVE_IN_ROOT` or `RESOLVE_BENEATH` answers EAGAIN where a rename or a mount anywhere on the system raced a `..` of the walk, and the caller may try again (openat2(2)). Ubuntu 24.04's loader path has one: `/usr/lib64/ld-linux-x86-64.so.2 -> ../lib/x86_64-linux-gnu/ld-linux-x86-64.so.2`.

## Approach

`sys::openat2` of the handler tries a call that asked for `RESOLVE_IN_ROOT` or `RESOLVE_BENEATH` again on EAGAIN, up to `EAGAIN_TRIES` (256) times. Each path that the handler resolves goes through it.

## Decision

-

## Prove

podrunner's `user-gate.yml`, lane `proxy-only`, executor podvm, with the tier: the express `lint` and `test` jobs run Node from `_work/_tool` and pass.

## Done

Partial (2026-11-18): the retry is on `main`. On the cloud host (kernel 6.18) neither the old nor the new binary failed in 300 execs of `/usr/bin/true` on the tier under a loop of renames and of tmpfs mounts, so the race is not measured here. Remaining: the measure on podrunner's CI.

# T-1991: On the syscall tier, `cargo doc` of ripgrep prints "Compiling ripgrep" and then nothing for 45 min

**Source:** PodUtils/podrunner#308 (COORD-098 of podrunner), item 3
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** open

## Problem

podrunner's job `docs` (`cargo doc` of ripgrep at `178cc027`) on the syscall tier printed `Compiling ripgrep v15.2.0` and then nothing until it was stopped at 45 min. It passes on the podvm kind with no tier and with `--runner-dir`.

## Premise

Not reproduced on the cloud host: ripgrep at `178cc027` in a volume, `rust:1-slim`, `cargo doc --offline` on the tier ends with exit 0 in 17.5 s with `0.2.0-beta.34` and in 12.1 s with `main`. COORD-197 (#593) asks podrunner for `uname -r`, the processes of the unit with their `wchan`, and the stack of the one that waits.

## Approach

Reproduce it as uid 1000 under `scripts/dev/nouserns.c` with a Rust image on the tier; find the process that waits and its call.

## Decision

-

## Prove

As uid 1000 under `scripts/dev/nouserns.c`: `cargo doc` of ripgrep at `178cc027` in `--podvm-tier=syscall` ends with exit 0 within the time it takes with no tier, plus a bound.

# T-1995: A Go program hangs on the userland rung at its first signal of preemption

**Source:** PodUtils/podrunner#315 (COORD-105 of podrunner): `npx tsc` of TypeScript 7 (a static Go binary) hung for 30 min in podrunner's `user-gate.yml` run 44438881119
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

On the userland rung, `tsc --strict a.ts` of `typescript@7.0.2` printed nothing and did not end. Its threads waited on futexes. With `GODEBUG=asyncpreemptoff=1`, or with `GOMAXPROCS=1`, it ended at once.

## Premise

Go preempts a goroutine by `tgkill` of SIGURG to its thread. A static program runs under the SIGSYS handler of `podvm-dispatch`, which runs each call on a stack of its pool (T-1976) while the alternate stack of the payload is disarmed (`SS_AUTODISARM`). A SIGURG that stopped the handler ran Go's handler there: Go found a stack that is neither its signal stack nor `g0`, and no alternate stack, took the path of a thread that is not Go's (`needm`), and waited for ever.

## Approach

`signals::wrap`, the wrapper of each handler of the payload, puts off a signal whose saved `rip` lies in podvm's code: it blocks the signal in the frame that it returns to and on the thread, and queues it again with its siginfo (to the thread where `tgkill` sent it, else to the process). The `rt_sigreturn` of the SIGSYS handler restores the payload's mask, and the kernel delivers the signal in the payload's code. A blocking call that it stopped returns EINTR or restarts as `SA_RESTART` says, before the payload's handler runs. `keep_thread_state` computes the mask of a `rt_sigprocmask` from the frame and the call, not from the thread. Before an exec, each held signal that is still pending takes its default action, as the exec gives it, and the thread takes the payload's mask back.

## Decision

-

## Prove

The test `a_signal_that_stops_a_call_runs_on_the_alternate_stack_of_the_payload` (3000 SIGURG by `tgkill` while the thread makes calls; each handler runs on the alternate stack of the payload); as uid 1000 with `PODVM_USERNS=0` (the userland rung): `tsc --strict a.ts` of `typescript@7.0.2` in `node:22-bookworm` ends with exit 0.

## Done

**Done (2026-11-19).** `signals::{in_podvm, defer, before_exec, after_failed_exec, install}`, the exact mask in `keep_thread_state`, and the frame's mask passed to `exec::execveat`. Without the change the test counts 334 handlers off the alternate stack, and with it 0. As uid 1000 with `PODVM_USERNS=0` on the cloud host: `tsc --strict a.ts` and `npx tsc --strict a.ts` of `typescript@7.0.2` end with exit 0 three times in three (before: the bound of 90 s each time); `go build` of a program of 64 goroutines in `golang:1.24-bookworm` and its run end with exit 0. The dispatch tests pass.

# T-2002: `--podvm-init=image` holds only on the incus tier: the namespace rung maps one uid, and the userland rung has no PID 1

**Source:** the owner (2026-11-19, the table of the kinds for an image with systemd as PID 1, and two research bundles); PodUtils/podrunner#313, PodUtils/podrunner#316; T-1911, T-1993, T-1994
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** L
**Status:** partial

## Problem

An image of a hosted runner starts its services with `systemctl start`, and each service runs as its own user (mysql, postgres, www-data). `--podvm-init=image` boots the init of the image only on the incus tier (T-1993). A foreground `run` on the namespace rung is uid 0 of a map with one uid, with podvm as PID 1 of its PID namespace: `systemctl is-system-running` gives `offline`, and `setresuid` to another uid gives EINVAL. The userland rung has the PID 1 of the host and the uid of the caller: `setresuid` gives EPERM (T-0711), and no call there makes a PID namespace.

## Premise

Measured on the cloud host as uid 1000 (kernel 6.18.44, no `/dev/kvm`, `ubuntu:100000:65536` in `/etc/subuid`, cgroup v1 with cgroup2 at `/sys/fs/cgroup/unified`, no delegated directory), with systemd 262 of `mirror.gcr.io/library/archlinux:latest`:

1. In a user namespace with the subordinate range (`newuidmap`, T-1911) and a PID namespace, systemd as PID 1 ends at "Failed to create /init.scope control group: Permission denied": the cgroup2 of its cgroup namespace belongs to host root. With a cgroup2 directory that uid 1000 owns, the same boot reaches `graphical.target`.
2. systemd wants `/` as a mount point ("Failed to remount root directory as MS_SLAVE: Invalid argument" after a `chroot`), `/dev` as a mount point (it mounts devtmpfs else, which a user namespace refuses), and a console that stays: a pty with no open slave gives EIO, and PID 1 then waits 1 s at each status line (a boot of 54 s, and 2 s with the slave held open).
3. With a tmpfs on `/sys/fs/cgroup` that holds the control files, systemd 262 ends at "Found tmpfs on /sys/fs/cgroup/, assuming legacy hierarchy" and "Detected unsupported legacy cgroup hierarchy, refusing execution": it reads the type of the file system. `/dev/fuse` of the host is mode 0600 of root. So no file system that this caller can mount answers as cgroup2.
4. A guest of the machine tier boots under TCG as uid 1000 in 9 s, with a kernel of its own, real root and a cgroup2 with each controller.

## Approach

One supervisor (`podvm guest-init`, `crates/podvm/src/initns/`) for two places:

1. The namespace rung. Where the host gives the caller the subordinate range and a delegated cgroup2 subtree (`pod_hostprobe::cgroup`, with a real write), a foreground `run --podvm-init=image` takes the range with no variable. The supervisor opens a PID namespace; `/sbin/init` of the image is its PID 1, in a mount, a cgroup, an IPC and a UTS namespace, with `/proc`, a read-only `/sys`, a `/dev` on tmpfs with the nodes of the host and a devpts instance, a tmpfs on `/run` and `/tmp`, cgroup2 of its own leaf, `container=podvm`, an empty machine-id, and the tree as `/` by `pivot_root`. The supervisor asks `systemctl is-system-running` until `running` or `degraded` (180 s), then runs the command in the namespaces of the init with the ids of `--user`, forwards the shutdown signals, gives the exit code, and ends the init (SIGRTMIN+4, 10 s, then SIGKILL). The end of PID 1 ends each process of the PID namespace.
2. A guest of the machine tier. The same supervisor runs the command of `/podvm-init`, as root of the guest; podvm's init stays PID 1 of the guest, with the markers, the 9p volumes and the exit code as before. A `run --podvm-init=image` with no tier takes this route where the host lacks a part of the first (the userland rung, no range, no delegated cgroup), with one line that names the part.

## Decision

Call C26 (2026-11-19): the fallback to the machine tier is automatic and prints one line; a caller that wants no guest names `--podvm-tier`. A guest that boots an init gets the CPUs of the host, at most four, where the caller names no `--podvm-cpus`. A change of this call belongs to the owner.

## Prove

`sh experiments/465-init-image.sh` as a user with no root (or as root with `PODVM_465_USER`), with `ghcr.io/pkgforge-dev/archlinux:latest` at the digest of `scripts/common/distro-matrix.sh`: clause 1 (the namespace rung from a delegated cgroup2 directory), clause 2 (no delegated cgroup: the machine tier) and clause 3 (the userland rung, `PODVM_USERNS=0`: the machine tier) each give exit 7 from the check: `/proc/1/comm` is `systemd`, the state is `running` or `degraded`, the units with `User=` of mysql, postgres, www-data and nobody run as those uids and write files of those uids, and `systemctl stop` ends a unit. Clause 6: no emulator, no systemd and no cgroup `podvm-init-*` stays. On a host with `/dev/kvm` and a delegated subtree: the job `cgroup` of `kvm.yml`.

## Done

Partial (2026-11-19). `crates/podvm/src/initns.rs` and `initns/{boot,mounts,join,cgroup}.rs`; the flag holds on a foreground `run` with no tier and with `--podvm-tier=machine` (`run/parse/opts.rs::init_check`, the table test `where_the_init_of_the_image_holds`); `initns::decide` runs before the user namespace, and `initns::route` takes the machine tier after the parse. The unit tests of `initns` pass, also as uid 1000.

Measured on the cloud host as uid 1000, experiment 465 (`experiments/results/init-image.txt`, "THE INIT OF THE IMAGE RUNS"):

* Clause 1, the namespace rung, with a cgroup2 directory that root gave to uid 1000 as a stand-in of a delegating host: exit 7 in 17 s with the extraction (3 s after it); state `running`; `uid_map` `0 1000 1` and `1 100000 65536`; the four units run as 961, 962, 33 and 65534.
* Clauses 2 and 3, the host as it is: the run names the missing part and takes the machine tier under TCG; exit 7; state `running`. With four vCPUs the run of `true` takes 24 s (one vCPU: 51 s, of which the boot of systemd is 37 s); a run of the machine tier with podvm's init takes 10 s. For the image of the Prove (a tree of 584 MB): 27 s against 14 s, a guest of 2304 MiB, and 2.6 GB of host memory in QEMU at the peak (`VmHWM`), as the root is an initramfs (T-1994).
* Debian 12 with `mariadb-server` and `postgresql` of apt (an image made with `podvm run` and `podvm import` as uid 1000): `mariadb.service` runs as uid 100 and `postgresql@15-main.service` as uid 102, both answer a query, and `/var/lib/mysql` and `/var/lib/postgresql/15/main` belong to them: 3 s on the namespace rung, 28 s through the machine tier from `PODVM_USERNS=0`.
* `-u http`, `-e`, `-w`, `-i`, `-t` and a `:ro` volume under `/tmp` hold on the namespace rung; a missing program gives 127 and a SIGTERM of the command 143.

Three faults that the runs found are fixed: the base archive of a guest reads a file that its owner cannot read (`dbus-daemon-launch-helper` of mode 4110; `machine/guest/cpio.rs`, as T-1993 did for the tarball of the incus tier; the test fails as uid 1000 without the fix); the wait for the boot reads the bus error of systemd 262 ("Failed to connect to system scope bus") as "not up yet", also on the incus tier (`incus/systemd.rs::step`); a second cgroup2 mount on a cgroup2 mount point gives EBUSY, so the mount goes to a scratch directory and moves.

Remaining:

1. A run on a host with `/dev/kvm` and a real delegated subtree (`kvm.yml`); the stand-in of clause 1 is a directory that root made.
2. `run -d`, `create` and `exec` with the init on the namespace rung; a caller that is uid 0 of the host takes the machine tier.
3. The pty of `-t` belongs to the devpts of the host: `tty` in the command gives "No such device".
4. The command of `--user NAME` gets the primary group only, and no supplementary group that the image gives the user; the init and a command of root get no group of the caller.
5. A volume under `--podvm-volume-base` at or under `/tmp` binds by its path, not by the held descriptor.
6. An image whose init is not systemd: the command starts at once, as no state can be asked. Measured with `public.ecr.aws/docker/library/alpine:3.20` (the digest of the matrix, from `mirror.gcr.io`): busybox `init` is PID 1, and the run gives the exit code 9 of its command. `mirror.gcr.io/library/debian:12` has no `/sbin/init`: exit 125 and "the image has no /sbin/init: it carries no init". OpenRC, runit and s6 are not measured.
7. T-2003 (a program with a library in a directory of its RUNPATH on the userland rung), T-2004 (a delegated subtree from the user manager of systemd: partial, experiment 466) and T-2005 (the routes with no guest and no delegated cgroup).

# T-2003: On the userland rung, a program with a library in a directory of its RUNPATH does not load (`systemctl`)

**Source:** the measure of T-2002 (2026-11-19); the research bundle of the owner (2026-11-19, `run/libpath.rs` of a branch that is not in this tree)
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

As uid 1000 with `PODVM_USERNS=0` and a volume (the loader family of the userland rung), `systemctl` of `mirror.gcr.io/library/archlinux:latest` ends with "error while loading shared libraries: libsystemd-shared-262.1-1.so: cannot open shared object file" (exit 127), and `/sbin/init` with the same line for `libsystemd-core`. The libraries lie in `/usr/lib/systemd`, which the programs name in `DT_RUNPATH`. The loader of the image runs from its host path, and an absolute directory of a RUNPATH names a path of the host there. `journalctl`, `udevadm`, `loginctl` and each other program of systemd fail the same way.

## Premise

The entry gives the loader the library directories of the image as host paths (`podvm_enter::userland::host_env`, `Userland::lib_dirs`), from the loader configuration of the image; it reads no `DT_RUNPATH` or `DT_RPATH` of the program. pod-enter of podcrates owns that entry.

## Approach

Ask podcrates (a COORD issue) for the directories of `DT_RUNPATH` and `DT_RPATH` of the program, and of each library that it needs, in the library path that pod-enter gives the loader, each resolved in the tree (`$ORIGIN` too). A child that the interposer starts through the loader takes the same path.

## Decision

-

## Prove

As uid 1000 with `PODVM_USERNS=0`: `run --rm -v DIR:/v ghcr.io/pkgforge-dev/archlinux:latest systemctl --version` prints the version of systemd and exits 0.

# T-2004: A delegated cgroup2 subtree from the user manager of systemd, for an init of the image on the namespace rung

**Source:** T-2002 (2026-11-19)
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** partial

## Problem

On a host with systemd, the shell of a login is in `session-N.scope`, which belongs to root: `run --podvm-init=image` finds no delegated directory and takes the machine tier, while the user manager of the caller (`user@UID.service`) can give a delegated scope. The caller must start podvm with `systemd-run --user --scope -p Delegate=yes` by hand today.

## Premise

rootless podman and crun ask the user manager for a scope with `Delegate=yes` over the user bus (`StartTransientUnit`), and move the container into it. `systemd-run --user --scope -p Delegate=yes -- CMD` does the same for a command and gives its exit code. Not measured here: the cloud host runs no systemd.

## Approach

Where `initns::decide` finds the range and no delegated directory, and the user bus answers (`$XDG_RUNTIME_DIR/bus`, else `/run/user/UID/bus`): ask the user manager for a scope `podvm-init-NONCE.scope` with `Delegate=yes` that holds the pid of podvm (`StartTransientUnit` with `PIDs=`, by `busctl --user` of the host, 5 s at most), and read `/proc/self/cgroup` until it names the scope. podvm runs no second time, and its descriptors stay. The scope ends with its last process. Where a step fails, the machine tier as before, and the line names the step.

## Decision

-

## Prove

`sh experiments/466-init-image-scope.sh` as a user with no root, with `ghcr.io/pkgforge-dev/archlinux:latest` at the digest of `scripts/common/distro-matrix.sh`: in a guest with systemd, a user with a user manager runs podvm from a cgroup of root; the check of experiment 465 exits 7 with the range in the `uid_map`, the run names no machine tier, and no scope `podvm-init-*` stays 90 s after it. On a host with systemd and a login of a user (a hosted runner, a job of `kvm.yml`): `sh experiments/465-init-image.sh` passes clause 1 from the shell of the login, with no directory that root made.

## Done

Partial (2026-11-19). `crates/podvm/src/initns/scope.rs` (`ask`, called by `initns::decide` before the user namespace); the unit test `the_scope_of_the_user_manager_is_asked_for_and_read` passes. Experiment 466 on the cloud host as uid 1000 gives "THE USER MANAGER GIVES THE SUBTREE" (`experiments/results/init-image-scope.txt`): the host of the measure is a guest of the machine tier under TCG with systemd 261 as its init (T-2002), as the cloud host runs no systemd. In it, uid 1000 starts podvm from `/system.slice/run-*.service` (a transient service of the system manager, as the scope of a login); the user manager moves podvm into `app.slice/podvm-init-*.scope`; the init of the image boots on the namespace rung (state `running`, `uid_map` `0 1000 1` and `1 100000 65536`, the four units as their users, exit 7); the scope ends 24 s after the run, with the helpers of podvm. A fault that the run found is fixed: podvm enabled the controllers of the scope for its children while podvm was a process of the scope; the kernel accepts that for a threaded controller (`pids`), each child cgroup is then "domain invalid", and the move of the supervisor gave EOPNOTSUPP. podvm now enables no controller in a directory with a process (`initns/cgroup.rs::enable`). Remaining: (1) the same from the shell of a real login on a host with systemd (`kvm.yml`); the setup of the guest made the user, started `user@1000.service` by `systemctl`, and gave `newuidmap` its set-user-ID bit; (2) the init of the image in a scope gets no controller of cgroup2 (memory, pids), as podvm is a process of the scope: move podvm into a leaf of the scope first; (3) a host with a user manager and no `busctl`.

# T-2005: An init of the image with no guest on a host that delegates no cgroup: a cgroup2 stand-in, or User-Mode Linux

**Source:** T-2002 (2026-11-19); the two research bundles of the owner (2026-11-19); the owner's direction of 2026-11-19 ("look for a near-native route first; TCG is the last fallback") and the hints of the scout of that day
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** partial

## Problem

The userland rung (a host that denies user namespaces), and a host that denies `ptrace`, boot the init of the image only in a guest of QEMU: under TCG on the cloud host a run with MariaDB and PostgreSQL takes 46 s, a start of a small unit 250 ms, and the guest holds 2.6 GB (T-2002). The namespace rung (3 s to 10 s) and the runsc tier (7 s to 10 s, T-2007) need a user namespace. A host with no QEMU that podvm can fetch or run has no route.

## Premise

Lines 1 and 4 are measured here by experiment 467 (the Done section); each other line is a claim of its source until a run here reads it.

1. User-Mode Linux in its seccomp mode (`seccomp=on`, Linux 6.16) needs no `ptrace`, namespace, KVM or QEMU: `os_early_checks()` of `arch/um/os-Linux/start_up.c` at v6.18 returns before `check_ptrace()` when `init_seccomp()` passes. The code of the guest runs on the CPU of the host, and the guest kernel gives a PID 1, a cgroup2 and each uid. The cost is the creation of a process.
2. UML SMP came in 6.19 (`ncpus=N`, only with `seccomp=on`); 7.0 fixed SIGCHLD in the seccomp mode; 7.1 added musl and an SMP fix of the TLB. pocket_vm (Apache-2.0 or MIT, github.com/flyinghorse0510/pocket_vm) runs each guest with `seccomp=on`, carries 8 patches on 7.2 (an SMP fix in `chan_kern.c`, a deadlock of the vector driver), makes `ptrace` give EPERM in its CI, and runs `docker:dind` with cgroup2 as a user.
3. The first bundle of the owner booted systemd as PID 1 of a UML kernel 6.13.12 in the ptrace mode on one CPU: 48 s with MariaDB and PostgreSQL, against 38 s under KVM on that host. The second bundle got `ptrace: Operation not permitted` from a UML 6.12 on its host.
4. UML takes a root with copy-on-write (`ubd0=run.cow,base.ext4`): the base of T-1994 for each digest keeps the owners of the image with no `chown` on the host. pocket_vm writes the layers in a "builder UML".
5. The second bundle of the owner tried a PID 1 and a cgroup tree in the interposer on the userland rung (`pid1.rs`, `cgroupfs.rs`, `fsmagic.rs`, `spawn.rs`, `rlimit_calls.rs`, `kmsg.rs`): about 45 runs, none with a boot ("Can't run system mode unless PID 1", then RLIMIT_CORE). Measured for T-2002: systemd 262 refuses a tmpfs in place of cgroup2, and starts each service "via CLONE_INTO_CGROUP".
6. For a namespace route with no gVisor: a seccomp filter that gives ENOSYS for `clone3` makes systemd attach through `cgroup.procs` (`posix_spawn_wrapper` of `src/basic/process-util.c`), and only `statfs` and `fstatfs` of `/sys/fs/cgroup` need a false answer. What stays wrong: `/proc/PID/cgroup` reads `/`, `cgroup.procs` keeps the last pid only, and `cgroup.events` never reads `populated 0`.

## Approach

In this order, each with experiment 465 as the check and its time for a run and for a unit start against the three routes of T-2002 and T-2007:

1. A UML tier for the userland rung: a static UML kernel of 7.1 or later with `seccomp=on ncpus=N`, the root from the ext4 base of T-1994 with a copy-on-write file, the exit code and the volumes as on the machine tier. ⛔ A kernel build is shared code (rule 2a): ask podcrates first (`pod-guest`, a `pod-microvm`), with pocket_vm as the donor.
2. Only where a namespace route with no gVisor is wanted: the seccomp filter and the `statfs` answer of premise 6, with its three faults named in the limits.
3. Not routes: proot (the target filters `ptrace`), LKL (no bridge to processes), `systemd --user` (it needs a delegated cgroup and has no `User=`), cgroup v1 (the same `statfs` check), and a PID 1 in the interposer (premise 5).

## Decision

-

## Prove

`sh experiments/465-init-image.sh` on the cloud host as uid 1000, with a new clause for each route that this entry adds: with `PODVM_USERNS=0` and a launcher that makes `ptrace` and `unshare` give EPERM, the check exits 7 with no emulator of QEMU, and the transcript names the time of the run.

## Done

Partial (2026-11-19): the measure, with no code of a tier. `sh experiments/467-uml-seccomp.sh` on the cloud host as uid 1000 gives "UML IN SECCOMP MODE BOOTS THE INIT WITH NO PTRACE AND NO NAMESPACE" (`experiments/results/uml-seccomp.txt`). A stock Linux 7.3.5 for `ARCH=um` (by the SHA-256 of kernel.org, built in 207 s, 8.6 MB stripped, linked to glibc) runs under a launcher whose seccomp filter gives EPERM for `ptrace`, `unshare`, `setns`, `chroot` and `mount`. Under it the same kernel with `seccomp=off` ends at "ptrace: Operation not permitted". With `seccomp=on ncpus=4`: a builder guest, with the tree of `ghcr.io/pkgforge-dev/archlinux` as its hostfs root, writes an ext4 base of 580 MiB in 27 s, with the owners of the sidecar (`/usr/bin/mount` is 0:0 with its set-user-ID bit; on hostfs each file belongs to the user of the host, and a set-user-ID program loses root); the run boots systemd 261 as PID 1 from the base behind a copy-on-write file (19 MiB) and gives exit 7 from the check of experiment 465 in 15 s; the base keeps its SHA-256, and no process stays.

A manual run of the same harness with Debian 12, MariaDB 10.11 and PostgreSQL 15 (the image and the bench of T-2007; not in the transcript), under the launcher:

| the route | the whole run | the boot of systemd | a start of a unit of `/bin/true` | a restart of MariaDB | a restart of PostgreSQL |
| --- | --- | --- | --- | --- | --- |
| UML 7.3.5, `seccomp=on ncpus=4` | 14.7 s | 5.5 s | 89 ms to 133 ms | 1.8 s to 1.9 s | 3.5 s |
| UML 7.3.5, `seccomp=on ncpus=1` | 18.9 s | 8.6 s | 110 ms to 116 ms | 1.8 s | 4.1 s |
| the machine tier under TCG, four vCPUs (T-2007) | 45.6 s | 8.8 s | 244 ms to 252 ms | 3.2 s to 3.4 s | 6.7 s |

The builder wrote the base of that image (834 MiB) in 32 s. Remaining: each part of the Approach. The kernel of the measure is dynamic and has no network device, no volume and no exit code to the host; podcrates has no answer yet on a kernel that podvm carries (no COORD issue is written).


# T-2007: The init of the image as PID 1 of a gVisor sandbox: the runsc tier, for a host with a user namespace and no delegated cgroup

**Source:** the owner's direction of 2026-11-19 ("look for a near-native route first; TCG is the last fallback"), hint 1 of the scout; T-1729, T-2002
**Category:** enter
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** partial

## Problem

A host with user namespaces and no delegated cgroup2 subtree (the cloud host, each container whose cgroup belongs to root) booted the init of the image only in a guest: 24 s to 51 s under TCG, with 2.6 GB of memory (T-2002). The runsc tier (T-1729) ran a command with no capability: `setresuid` and `chown` gave EPERM in it.

## Premise

gVisor runs systemd as PID 1 since 2026-09 (gvisor.dev/blog/2026/09/17/systemd-in-gvisor), with `--in-sandbox-cgroup=v2` and `--allow-suid`; both flags are in `release-20261102.0`, which experiment 436 pins. The Sentry gives a cgroup2 tree and each uid in its own kernel. Measured here as uid 1000 with that release, rootless on `systrap`: (1) with `/sbin/init` as the process of a bundle, systemd 262 ends at the mount of cgroup2 with `memory_hugetlb_accounting` (EINVAL) and of `/dev/shm` with `usrquota`; with those mount points in the bundle it boots in 2.5 s to 5 s; (2) `runsc kill` takes no real-time signal, and after SIGRTMIN+4 systemd stops each unit and stays as `systemd-shutdown`; (3) `systemd-sysctl.service` ends with exit 1, and the Sentry does not apply `readonlyPaths`; (4) with `--overlay2=root:self` a `chown` to mysql holds.

## Approach

`run --podvm-tier=runsc --podvm-init=image` (`crates/podvm/src/runsc/init.rs`). The bundle starts `podvm guest-init --pid1` (`initns/sandbox.rs`; podvm and the sidecar of the image are bind mounts): it gives the files of the tree the owners of the sidecar, masks `systemd-sysctl.service` in `/run`, writes an empty machine-id, and becomes `/sbin/init`. The bundle has the capabilities of a container without CAP_NET_ADMIN, CAP_SYS_MODULE, CAP_SYS_RAWIO and CAP_SYS_TIME, and the mount points `/dev/shm`, `/dev/pts`, `/sys/fs/cgroup`, `/run` and `/tmp`. podvm asks `systemctl is-system-running` by `runsc exec` (180 s), runs the command by `runsc exec` with `--user`, the environment and the working directory, then sends SIGRTMIN+4 from inside, waits until PID 1 is no longer the manager (10 s), and kills the sandbox. `runsc run` ends with podvm (`PR_SET_PDEATHSIG`), and its sandbox with it. A `run --podvm-init=image` with no tier takes this tier before the machine tier, where `runsc` is there (`PODVM_RUNSC`, else PATH), the host gives a user namespace, and the run has no flag that the tier does not serve (`-t`, `--timeout`, `--tmpfs`); call C26 has the order.

## Decision

Call C26, amended 2026-11-19: the namespace rung, then the runsc tier, then the machine tier.

## Prove

`sh experiments/465-init-image.sh` as a user with no root, with `ghcr.io/pkgforge-dev/archlinux:latest` at the digest of `scripts/common/distro-matrix.sh`, clause 8: on a host with no delegated cgroup, with the pinned runsc, the run names the runsc tier and the check exits 7 (systemd is PID 1, the state is `running` or `degraded`, the units of mysql, postgres, www-data and nobody run as those uids and own their files), also with `PODVM_USERNS=0`; after a SIGKILL of podvm no process of gVisor stays.

## Done

Partial (2026-11-19). `runsc/init.rs`, `initns/sandbox.rs`, `userns/owners.rs::apply_text`, `run/runsc.rs::unserved`, the route in `initns::route`; the unit tests of `runsc::init` and `initns::sandbox` pass. Measured on the cloud host as uid 1000 (no delegated cgroup, no `/dev/kvm`, gVisor `release-20261102.0`):

| Debian 12 with MariaDB 10.11 and PostgreSQL 15 under systemd 252 | the whole run | the boot of systemd | a start of a unit of `/bin/true` | a restart of MariaDB | a restart of PostgreSQL |
| --- | --- | --- | --- | --- | --- |
| the namespace rung (a cgroup2 directory that root gave) | 9.8 s | 2.4 s | 7 ms to 21 ms | 0.17 s to 0.19 s | 2.4 s |
| the runsc tier (the host as it is) | 10.2 s | 3.6 s | 42 ms to 45 ms | 0.54 s to 0.59 s | 2.9 s |
| the machine tier under TCG, four vCPUs | 45.6 s | 8.8 s | 244 ms to 252 ms | 3.2 s to 3.4 s | 6.7 s |

`mariadb.service` runs as uid 100 and `postgresql@15-main.service` as uid 102, both answer a query, and their data directories belong to them (the owners of the sidecar, applied in the sandbox). The check of experiment 465 with systemd 262 of `mirror.gcr.io/library/archlinux:latest` gives exit 7 in 7 s, state `running`. A missing program gives 127 before the boot; a SIGTERM of podvm gives 143; after a SIGTERM and a SIGKILL of podvm no process of gVisor stays. Remaining: (1) podvm fetches no runsc: the tier is there only where the host has one (T-2009); (2) `-t`, `--timeout` and `--tmpfs` go to the machine tier; (3) each write of the root ends with the run (the overlay), and a file whose owner changes is copied into the overlay; (4) after a SIGKILL of podvm the bundle and the tree of the run stay in the store; (5) the tier needs a user namespace and `ptrace` for the threads of `systrap`, so it does not serve the userland rung (T-2005); (6) a run on `kvm.yml` and on arm64.

# T-2009: podvm fetches no runsc: the gVisor route of an init is there only where the host has one

**Source:** T-2007 (2026-11-19); the owner's rule of 2026-11-05 ("users download and use")
**Category:** enter
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

The runsc tier takes `runsc` of `PODVM_RUNSC` or of PATH. A host with a user namespace and no gVisor takes the machine tier for an init of the image (45 s against 10 s on the cloud host, T-2007), and the user must install gVisor by hand, with its sidecars of `gvisor-bin/`.

## Premise

podvm fetches a pinned static QEMU into the store where the host has none, and checks it by its digest (T-1875). gVisor publishes `gvisor.tar.bz2` for each release with a SHA-512; experiment 436 pins `20261102` and its sum. `runsc` and its sidecars are static.

## Approach

Where no `runsc` is found, fetch the pinned release for the architecture of the host into the store (as T-1875 does: the pin and its sum in the binary, a partial file, a resume, the proxy and the CA of the host), unpack `runsc` and `gvisor-bin/`, and use it. `podvm doctor` names it. Ask podcrates first for the fetch of a pinned tool (rule 2a): `pod-guest` holds the one of QEMU.

## Decision

-

## Prove

On a host with a user namespace and no `runsc` on PATH, as a user with no root: `run --rm --podvm-init=image ghcr.io/pkgforge-dev/archlinux:latest systemctl is-system-running` fetches gVisor once, names the runsc tier, and prints `running` or `degraded`; a second run fetches nothing.
