Area: probe.

Record semantics: [task rules](RULES.md#5-entry-closure).


`crates/podvm-probe`. `TOOL.md` section 6.1, milestone M0.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

Probing is where every prior account went wrong, and the corpus contains both
halves of the lesson: `pathshim` refuses a tier it cannot install and says why,
`sandlock` continues per call and reports success for mediation it never
performed. The rules below are each paid for by one of them.

⭐ **Two entries carry the whole discipline.** [T-0102](#t-0102-separate-a-filtered-syscall-from-an-executed-one-with-a-bogus-argument)
is the discriminator, and [T-0109](#t-0109-a-verdict-is-the-operations-and-could-not-run-never-reads-as-denied)
is the rule that a probe reports the verdict of the operation it names.

# T-0101: The probe set, one disposable child per probe, errno not boolean

**Source:** `TOOL.md` section 6.1, `paper_final.md` section 10.2
**Category:** probe
**Milestone:** none
**Priority:** P0
**Effort:** M
**Status:** done

## Problem

Without this nothing downstream can branch. Every mode the ladder
offers has prerequisites that differ per machine, and a runtime
that assumes any of them has hard-coded one machine.

## Premise

Read, not measured here. `TOOL.md` section 6.1 lists the minimum set;
`paper_final.md` section 10.2 is the same list with its reasoning.
`references/Azathothas__container-research/tree/verification/probe/`
implements it in Go and is the reference, not a sketch.

## Approach

Port the set, do not redesign it. Each probe runs in a freshly
forked child, because a successful `unshare`, `chroot` or `setuid`
mutates the prober. Record the **errno**, never a boolean: `EINVAL`
from `setuid` or `chown` means an unmapped id and points at a
mapping; `EPERM` points at a policy, and they are different
remedies.
The disposable-child shape to copy is
`references/compforge__pathshim/tree/src/main.rs:103-118`, which
runs its probe as `Command::new("/proc/self/exe")` with a private
argument and takes the verdict from the child's exit status.
⛔ Never probe `unshare(CLONE_NEWUSER)` from a multithreaded
process: the kernel refuses it to any such caller with `EINVAL`
regardless of policy. In Rust this is free, and the rule survives
the day somebody adds a thread pool.

## Decision

Fork per probe rather than a thread per probe. A thread cannot be
disposed of after `chroot` and `syscall(SYS_unshare)` mutates only
the calling thread, which leaves the process half-transitioned.
The alternative, running probes in-process and undoing them, loses
because several of them have no undo.

## Prove

`podvm probe --json | jq -e '.probes | length >= 24 and (map(select(.errno == null and .verdict != "ok")) | length == 0)'`

## Done

**Done 2026-09-08.** The `Prove` command was run and exits 0: **48 probes**, and
no probe reports a verdict other than `ok` without an errno beside it.
`pod_hostprobe::probes` is the set and
`pod_hostprobe::child` is the child.

⭐ **The fork is `clone(2)`, not `std::process::Command`**, because the minimum
set needs `clone(CLONE_NEWNS)` and `clone(CLONE_NEWUSER)` attempted *with those
flags* and the standard library cannot set them. One spawn path serves both
kinds of probe: a `Kind::Clone` row's verdict is the clone's own errno, and a
`Kind::Child` row re-executes `/proc/self/exe --probe-child <name>` with the
namespace flags that row needs.

⭐ **The multithreaded rule is checked, not assumed.**
`pod_hostprobe::probes`'s `unshare(CLONE_NEWUSER)` body reads
`/proc/self/status`'s `Threads:` line first and reports a `skip` where it is
above one, because the kernel refuses that call to any multithreaded caller
with `EINVAL` whatever the policy. That is what makes the rule survive the day
somebody adds a thread pool rather than being a comment nobody re-reads.

⭐ **The declared syscall numbers were checked against the kernel's own
headers**, because a wrong one would report a verdict for a syscall nobody
named and would look exactly like a measurement. The command is one `gcc` away
and is reproducible:

```sh
printf '#include <sys/syscall.h>\n#include <stdio.h>\nint main(void){printf("%%d\\n",SYS_move_mount);}\n' \
  | gcc -x c - -o /tmp/nr && /tmp/nr   # against `pod_hostprobe::sys`
```

All 46 declared numbers and all 19 flag constants matched on 2026-09-08. Two
tests in `pod_hostprobe::sys` additionally cross-check the trap, the
ABI and the negative-return window against facts the process already knows by
another route, and the sixteen attribution numbers are exercised on every run of
`experiments/130-probe-parity.sh`.

⚠ **The verdict channel survives a caller with fd 1 closed.** `pipe2(2)` hands
out the lowest free descriptors, so with stdout closed one end of the pipe lands
on fd 1, where `dup2(1, 1)` is a no-op that returns without closing and an
unconditional `close` after it would shut the channel every child answers
through. Driven on 2026-09-08 with `podvm probe 1>&- 2>err`: the evidence block
is identical to an ordinary run, two skips either way.

⚠ **The errno comes from the raw syscall, not from a libc wrapper.**
`pod_hostprobe::sys` issues `syscall` directly, so the value in
`-4095..=-1` *is* `-errno` with no thread-local in between. glibc's `setuid(3)`
runs a multi-threaded id-change dance and musl's runs another; either answer
would be the library's rather than the kernel's.

⚠ **Four probes of the Go instrument were changed on purpose**, and the module
header of `pod_hostprobe::probes` carries all four with the reason.
The one worth naming here: the bare `mount(MS_SLAVE,/)` row is not ported,
because in the caller's mount namespace it changes the propagation of `/` for
the machine, permanently. The same operation is measured inside
`clone(CLONE_NEWNS)`, which is also where bubblewrap performs it.

# T-0102: Separate a filtered syscall from an executed one with a bogus argument

**Source:** `TOOL.md` section 2.1 and section 6.1 rule 7, `paper_final.md` section 10.2 rule 7
**Category:** probe
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

Two mechanisms return the same errno for the same call. A seccomp
filter and a path-scoped LSM both answer `EPERM`, and a runtime
that cannot tell them apart reports the wrong remedy.

## Premise

Measured, in the reconstruction this repository seeds
`experiments/` from. A filter sees the syscall number and six
argument registers, cannot dereference a pointer, and runs before
the syscall body. So an argument the kernel rejects *inside* the
body separates them:

| call | answer | meaning |
| --- | --- | --- |
| `mount(2)`, nonexistent target | `ENOENT` | the syscall executed |
| `mount(2)`, nonexistent target | `EPERM` | a filter refused it |
| `move_mount` to a nonexistent dest | `ENOENT` | executed; an `EPERM` elsewhere is an LSM |
| `process_vm_readv` on pid 999999 | `ESRCH` | executed |
| `process_vm_readv` on pid 999999 | `EPERM` | filtered |

## Approach

Implement the four rows above plus their controls. The controls
are not optional: `pidfd_getfd(-1, -1)` must answer `EBADF` and
`kcmp(-1, ...)` must answer `ESRCH`. A probe whose control has
stopped answering has stopped discriminating and must say so
rather than reporting a mechanism.
The independent witness for the clone-versus-mount half is
bubblewrap: `references/containers__bubblewrap/tree/bubblewrap.c:3034`
sets `clone_flags = SIGCHLD | CLONE_NEWNS` unconditionally, and
`references/containers__bubblewrap/tree/bubblewrap.c:3257-3258`
is the `MS_SLAVE` remount that dies with `Failed to make / slave`.
Reaching that message proves the clone succeeded and the mount did
not.

## Decision

Carry the controls in the same probe run rather than in a separate
self-test. A control that runs at a different time answers about a
different machine state.

## Prove

`./experiments/30-attribution-census.sh` exits 0 or 2, never 1, and `podvm probe --json | jq -e '.controls.pidfd_getfd == "EBADF" and (.controls.kcmp == "ESRCH" or (.controls.kcmp == null and .controls_answered == false))'`

## Done

**Done 2026-09-08.** `./experiments/30-attribution-census.sh` exits **2** (34 ok,
0 failed, 3 skipped: the Landlock rows this kernel cannot run), and the `jq`
above exits 0. `crates/podvm-probe/src/select.rs`'s `Selection::controls`
evaluates the controls in the same run as the rows they qualify.

⛔ **The `Prove` line above was amended, and the amendment is the finding.** It
read `.controls.kcmp == "ESRCH"`, which asserts one host's answer as a property
of every host. `kcmp(2)` exists only where the kernel was built with
`CONFIG_CHECKPOINT_RESTORE`. Two readings settle it and they disagree with each
other, which is the point:

| where | `kcmp(-1,-1,...)` | source |
| --- | --- | --- |
| the target | `errno=3 ESRCH`, executed | `references/Azathothas__container-research/tree/verification/real/extkernel-newapi.txt:26` |
| the host this repository is worked on | `errno=3 ESRCH` | `experiments/results/attribute.txt:6`, re-captured in commit `6eb941f`, and the corpus' own capture agrees |

So the entry's own premise holds on the target and the `Prove` did not. ⭐ The
rule underneath it is unchanged and is what podvm implements: **a control that
cannot answer must say so rather than letting the rows beside it read as
attributions.** `ENOSYS` is the kernel saying the control is absent, which is
not the control answering, so podvm records it as `skip` and sets
`controls_answered` false. `podvm probe`'s evidence block then says, in as many
words, that the mechanism attributions beneath it are unconfirmed.

⚠ **`controls_answered` does not move the exit code**, and that is deliberate.
Folding it into `--strict` would fail every kernel without
`CONFIG_CHECKPOINT_RESTORE` where podvm is otherwise fully capable, and one
exit code cannot then carry two different statements.
`crates/podvm-probe/src/select.rs`'s `Selection::exit_code` says so at the site.

⚠ A control that answers the **wrong** errno is caught as well as an absent one:
that is the subtler failure, and it is the one where the rows around it silently
stop being attributions.

# T-0103: Probe creation and attachment separately

**Source:** `TOOL.md` section 6.1 rule 8, `paper_final.md` section 9.4 and F13
**Category:** probe
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

A runtime that asks only "does `mount(2)` work" learns less than
one syscall's worth more effort would have told it, and then
reports "no mounts" where the truth is "mounts you can create and
never attach".

## Premise

Measured on the target, one session, and half of it reproduced in
the reconstruction. `fsopen`, `fsconfig(CMD_CREATE)`, `fsmount`,
`open_tree(OPEN_TREE_CLONE)` and `mount_setattr` succeed;
`move_mount` is denied inside the kernel; `openat` on a detached
mount fd answers `EACCES`.

## Approach

Probe the create half, the configure half and the attach half as
three separate verdicts, and probe **use** as a fourth: a detached
mount that cannot be opened through is not a usable private
filesystem, so a runtime that stops at "creatable" reports a
capability it does not have.
⭐ `fsmount` succeeding proves `may_mount()` passes, and therefore
that no mount denial on that machine is a capability problem. Put
that in the diagnostic: it stops the next reader looking for a
capability to acquire.

## Decision

Report four verdicts rather than one `mounts: no`. The four-way
answer is what makes the diagnostic actionable and it costs three
extra syscalls.

## Prove

`podvm probe --json | jq -e '.mounts | has("create") and has("configure") and has("attach") and has("use")'`

## Done

**Done 2026-09-08.** The `Prove` command was run and exits 0.
`pod_hostprobe::mounts` derives the four from the attribution rows
that already ran, rather than issuing a second set of calls that would answer
about a different moment.

⭐ **The `may_mount()` sentence is in the output**, not only in this entry:
where `fsmount` succeeded and the attach did not, the evidence block prints that
no capability will clear the denial. That is the line that stops the next reader
hunting for one.

⚠ On the reconstruction of 2026-09-08 all four read `ok`, because this host has
no Landlock and `move_mount` therefore attaches. On the target the attach half
is `EPERM` from the LSM. The reading is the machine's, and the four-way shape is
what makes the difference legible instead of collapsing to `mounts: no`.

# T-0104: Probe the write allowlist by writing, for blocks and inodes

**Source:** `TOOL.md` section 6.1 rule 10 and section 2.0
**Category:** probe
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

`{/tmp, /dev/shm, /workspace, /state}` is one machine's allowlist.
A runtime that hard-codes it has hard-coded that machine, and on
the machine it was written for `/tmp` is 64 MiB.

## Premise

Read, and partly measured. `TOOL.md` section 0 says the allowlist has
never been verified anywhere a reader can re-run, and instructs
that it be probed rather than assumed. The capacity half is
separate and belongs to T-0203.

## Approach

For each directory the workload needs, attempt a real write and
record the errno, then `statvfs` it for **blocks and inodes both**.
Report the set obtained, not the set expected. An `EACCES` where
the directory's owner is mapped is a path-scoped policy; an
`EACCES` where it is unmapped is the user namespace, and
`/proc/self/uid_map` (T-0105) tells them apart in one read.

## Decision

Write a real file and remove it, rather than `access(W_OK)`.
`access` consults the capability set, which reports every bit here
and answers the wrong question.

## Prove

`podvm probe --json | jq -e '.writable | type == "array" and length > 0 and all(has("path") and has("blocks_free") and has("inodes_free"))'`

## Done

**Done 2026-09-08.** The `Prove` command was run and exits 0.
`pod_hostprobe::writable` creates a real file `O_EXCL`, writes bytes
to it, removes it, then `statfs(2)`s the directory for blocks **and** inodes.

⭐ **The set reported is the set obtained.** Every candidate carries its own
verdict and errno, and `writable: true` marks the ones that answered. Six of
eight on this host unconfined; four of eight inside the reconstruction, and
those four are `/tmp`, `/dev/shm`, `/workspace` and `/state`, obtained rather
than assumed. `/var/tmp`, `/run` and `$HOME` are `skip (ENOENT)` there, which is the
absence of a directory and not a denial.

⚠ The candidates are the fixed list plus `$PODVM_STORE`, `$XDG_RUNTIME_DIR`,
`$XDG_DATA_HOME`, `$TMPDIR`, `$HOME` and the working directory, and each row
says which named it. A path is probed because something would use it, not
because a specification listed it.

# T-0105: Read the ID maps directly rather than inferring them

**Source:** `TOOL.md` section 6.1 rule 6, `paper_final.md` section 3.1
**Category:** probe
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

A dozen probes infer what one read answers, and the inference is
wrong in the case that matters: `groups=0,65534` looks like a
supplementary group and is `overflowgid`, which is what
`getgroups(2)` returns for a group with no mapping.

## Premise

Measured on the target. `/proc/self/uid_map` reads `0 1000 1`,
`/proc/self/gid_map` the same, `/proc/self/setgroups` reads `deny`.

## Approach

Read all three, plus `/proc/self/status` for `CapEff` and
`Seccomp`, and put the contents in the diagnostic. A single-entry
map is the fact that explains every `EINVAL` from `chown` and
`setuid`, and quoting it turns a confusing errno into an
explanation.
⚠ A full `CapEff` proves nothing on its own: a process that is
root in a new user namespace receives the complete set by
construction, whatever its parent held.

## Decision

Read the maps even where every probe already ran. They cost one
`open` each and they are what the section 6.9 diagnostic quotes.

## Prove

`podvm probe --json | jq -e '.identity.uid_map != null and .identity.setgroups != null and .identity.cap_eff != null'`

## Done

**Done 2026-09-08.** The `Prove` command was run and exits 0.
`pod_hostprobe::identity` reads the three files and selects the
`/proc/self/status` lines **by name**, never by position.

⭐ **Two sentences in the diagnostic are the whole reason for the entry**, and
both fired inside the reconstruction: that a single-range uid map is what
explains every `EINVAL` from `chown(2)` and `setuid(2)`, and that gid 65534 is
`overflowgid` rather than a supplementary group. Reading
`/proc/self/uid_map: 0 1000 1` beside `groups=[0, 65534]` is what turns two
confusing errnos into one explanation.

⚠ A file that could not be read is `null` with the reason in
`.identity.unreadable`, never an empty string that would read like a value.

# T-0106: The `mknod` pair, because one of them tests nothing

**Source:** `TOOL.md` section 6.1 rule 4, `paper_final.md` section 3.5 and F11
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`mknod(path, S_IFCHR|0600, 0)` is in common use as a `CAP_MKNOD`
probe and it tests nothing: `makedev(0,0)` is `WHITEOUT_DEV` and
`vfs_mknod` exempts whiteouts from the capability check.

## Premise

Measured. Under a user namespace with a partial map the whiteout
succeeds while `mknod(S_IFCHR, makedev(1,3))` returns `EPERM`.

## Approach

Run **both**, in the same directory, and report the pair. A
whiteout that succeeds where a real device number fails proves the
denial is capability-based and not path-based, because no
path-scoped policy can tell two device numbers apart at one path.
That is a free mechanism attribution and it costs one syscall.

## Decision

Keep both rows in the reported output rather than collapsing them
to `mknod: denied`. The pair is the evidence; the collapse throws
away the attribution that makes it useful.

## Prove

`podvm probe --json | jq -e '.probes | map(select(.name | startswith("mknod"))) | length == 2'`

## Done

**Done 2026-09-08.** The `Prove` command was run and exits 0. Both rows are in
the reported output and neither is collapsed.

⭐ **The pair did its job inside the reconstruction on the first run**:
`mknod(chr 1:3 /tmp/nodprobe)` answers `EPERM` and `mknod(chr 0:0 = whiteout)`
answers `OK`, at the same path in the same directory. No path-scoped policy can
tell two device numbers apart at one path, so the denial is capability-based.
That is one syscall's worth of extra effort for a mechanism attribution.

⚠ Neither row deletes a file that is already at its path. An existing
`/tmp/nodprobe` is a `skip` naming the file, because an errno about a node this
probe created says nothing about the one that was there.

# T-0107: Mode selection, and one switch that turns every degradation into a refusal

**Source:** `TOOL.md` section 4.1, section 6.1; `paper_final.md` section 10.1
**Category:** probe
**Milestone:** none
**Priority:** P0
**Effort:** M
**Status:** done

## Problem

A weaker mode must never satisfy a stronger request. A user who
believes they have namespaces when they have a `chroot` is worse
off than one who is told the truth, and an automated user cannot
notice the difference the way a human skimming a log might.

## Premise

Read in the specification, and both halves are measured in the
corpus. `ruri` degrades per operation and refuses on one named
floor; `udocker` selects a mode per container and persists it.

## Approach

Derive the rung from the probe results, never from a privilege
that usually implies it. Two mechanisms transfer:

1. ⭐ `references/RuriOSS__ruri/tree/src/include/ruri.h:237-248`.
   `ruri_warn_on_error(ret, expect, show, fmt, ...)` is non-fatal
   by default and, under `ruri_flag(force_panic)`, warns and then
   exits at the same site. **One switch converts every degradation
   into a refusal.** podvm needs that switch because its audience
   is automated and has to be able to demand strictness.
   The floor beside it is
   `references/RuriOSS__ruri/tree/src/unshare.c:53-54`, the single
   namespace `ruri` refuses to degrade on, against
   `references/RuriOSS__ruri/tree/src/unshare.c:55-77` where five
   others warn and continue.
2. `references/indigo-dc__udocker/tree/udocker/engine/execmode.py:43-56`.
   `get_mode()`'s precedence is force, then the per-container file,
   then a config override, then a per-architecture default, then
   `DEFAULT`. Persisting the selection per container is what
   `TOOL.md` section 4.1 asks for at container granularity.

⚠ `ruri`'s `show` argument is `!ruri_flag(disable_warnings)`, so a
flag can silence its degradation notices. podvm suppresses the
banner by config and never by default, because a silenced
degradation is `sandlock`'s failure mode with a flag in front of
it.

## Decision

Take `ruri`'s inversion and `udocker`'s precedence chain, and take
neither project's architecture. The alternative considered and
rejected is a single global strictness level: it cannot express
"this container asked for isolation and that one did not", which
is the distinction the ladder exists for.

## Prove

`! podvm run --network=none --rm public.ecr.aws/docker/library/alpine:3.20 true` and `podvm probe --strict` exits non-zero on any degraded rung

## Done

**Done 2026-09-09.** The selection and the switch were implemented and measured
on 2026-09-08; the first half of the `Prove` above was earned under M3 on
2026-09-09, and the reading is below.

**What holds now.** `crates/podvm-probe/src/select.rs` derives the rung from
the probe rows and nothing else. Measured on 2026-09-08: `namespace` unconfined
and `chroot` inside `./experiments/20-enter-target.sh`, which is
[T-1101](milestones.md)'s acceptance. `podvm probe --strict` exits **1** inside
the reconstruction and **0** unconfined, and `Rung`'s ordering is what enforces
that a weaker mode never satisfies a stronger request.

⭐ **`supervise` is refused on this runtime for a reason `TOOL.md` section 4.1
does not carry**, and the refusal is at the tier rather than per call:
[T-0606](supervise.md) establishes from `references/multikernel__sandlock` issue
#27 that a race-safe `Continue` needs `PTRACE_SEIZE` on every thread of every
process in the sandbox, and `ptrace` is filtered here. So the third leg is
`ptrace(PTRACE_TRACEME)`, and the reconstruction is rejected on it with the
errno printed.

⛔ **What is NOT done, and it is not out of scope.** The `podvm run` half needs
`run`, which is [T-1104](milestones.md), M3. ⚠ Running it today exits 125
because every verb but `probe` is unimplemented, so the command would *pass
vacuously*; that is not evidence and is not recorded as any. This entry closes
when M3 lands and the first half is run against a real `run`.

⭐ **The remaining half closed 2026-09-09 by M3.** This was `partial` because
rung selection had no caller that entered anything: it was measured and never
acted on. `podvm run` now selects the rung for a real entry, and
`experiments/300-run.sh` clause 7 is the reading that matters, because it is the
one taken somewhere the answer differs:

| where | rung |
| --- | --- |
| this host | `namespace` |
| inside `20-enter-target.sh`'s reconstruction | **`chroot`** |

⛔ A selection that answered the same thing in both would have proven nothing.

# T-0108: The mode banner

**Source:** `TOOL.md` section 6.1 and section 6.8, `paper_final.md` section 10.1 and section 10.8
**Category:** probe
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

An agent cannot notice that its "container" was a `chroot`. The
banner is the product, not a disclaimer.

## Premise

Read. `TOOL.md` section 6.1 gives the exact shape, on stderr, once per
`run` and `exec`, suppressible by config and never by default.

## Approach

Print the achieved mode, what it does not provide, and the probe
verdicts that decided it:

```
podvm 0.1: mode=chroot (namespaces: uts-only; mounts: none; devices: shimmed;
ownership: virtualized+sidecar; network: host-shared; pids: host-shared)
probes: clone(NEWNS)=ok mount=EPERM fsmount=ok move_mount=EPERM
        clone(NEWUTS)=ok sethostname=ok chroot=ok ptrace=EPERM
```

⛔ The banner follows [T-0804](cli.md), the single home of the
output-honesty rule.

## Decision

stderr, not stdout. A payload's stdout is data to whatever
consumes it, and a banner on it corrupts every pipeline. The same
split `pathshim` uses at
`references/compforge__pathshim/tree/src/main.rs:134-138`.

## Prove

`podvm run --rm public.ecr.aws/docker/library/alpine:3.20 /bin/echo hi 2>banner.txt >out.txt && grep -q '^hi$' out.txt && grep -q 'mode=' banner.txt`

## Done

**Done 2026-09-09.** The banner existed on stderr, derived, on 2026-09-08; the
`Prove` above was earned under M3 on 2026-09-09, and the reading is below.

**What holds now.** `crates/podvm-probe/src/report.rs` emits it, and
`podvm probe` prints it on stderr while stdout carries the rung alone. Inside
the reconstruction it reads:

```
podvm 0.1.0: mode=chroot (namespaces: uts-only; mounts: none; devices: shimmed;
ownership: virtualized+sidecar; network: host-shared; pids: host-shared)
probes: clone(NEWNS)=ok mount=EPERM fsmount=ok move_mount=ok
        clone(NEWUTS)=ok sethostname=ok chroot=ok ptrace=EPERM
this mode does NOT provide: process, network, IPC or mount isolation
```

⭐ **Every field is derived from a row that ran**, which is why
`clone(CLONE_NEWUTS)` and `sethostname(in NEWUTS)` were added to the probe set:
the specification's banner names `sethostname=ok` and the Go instrument has no
such row, so `namespaces: uts-only` would otherwise have been an assertion.
`move_mount=ok` where the specification's example shows `EPERM` is the measured
difference: this reconstruction has no Landlock.

⚠ **`mounts:` and `network:` state what the RUNG provides, not what the machine
permits.** On 2026-09-08 the reconstruction attaches mounts and the machine's
own four verdicts all read `ok`, and the banner still says `mounts: none`,
because the `chroot` rung mounts nothing. The machine's reading is in the
evidence block underneath, where it is a measurement rather than a claim about
the mode.

⛔ **What is NOT done, and it is not out of scope.** "Once per `run` and `exec`"
needs `run` and `exec`, which are [T-1104](milestones.md), M3. The config switch
that suppresses it needs the configuration surface of [cli.md](cli.md).

⭐ **The remaining half closed 2026-09-09 by M3.** The banner existed and was
printed by `podvm probe`, which is a diagnostic nobody pipes. It now prints on
**every `podvm run`**, on stderr, before the payload's first byte, and
`experiments/300-run.sh` clause 1 asserts both halves: a `mode=` line **is** on
stderr and **is not** on stdout.

⛔ The second assertion is the load-bearing one. A banner on stdout corrupts
every pipeline the payload is in, and a test that only checked it was printed
somewhere would pass with the bug in place.

⚠ Inside the reconstruction the banner also carries
`this mode does NOT provide: process, network, IPC or mount isolation`, which is
the sentence the whole honesty argument rests on: it is printed where it is
true, and not printed on this host where podvm reaches `namespace`.

# T-0109: A verdict is the operation's, and "could not run" never reads as "denied"

**Source:** `TOOL.md` section 6.1 rule 9, `paper_final.md` section 3.7 and F17
**Category:** probe
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

Both mistakes were live in the research harness this project
inherits its `experiments/` from, and both produced a published
row that was wrong: a mount row reported the child's exit code,
and the child exited 0 whether the mount failed or not; and a probe
whose fixture was missing reported the resulting `ENOENT` as a
denial.

## Premise

Measured, and recorded in the reference itself.
`references/Azathothas__container-research/tree/verification/README.md:91-107`
carries all three verdict bugs and their fixes.

## Approach

Three rules in the probe type, not in review:

1. A child sets its exit status **from the operation** or the
   parent does not read a verdict from it.
2. A missing precondition is a third state, `skip`, with the reason,
   and it is never rendered as a denial.
3. The process-level exit code distinguishes them: **0** ran and
   matched, **1** ran and something failed, **2** could not run.
   That is the contract every script in `experiments/` already
   holds to.

## Decision

Model the verdict as a three-variant enum rather than a boolean
plus a comment. A boolean cannot carry "not measured", and every
instance of this defect in the corpus is a boolean that had to.

## Prove

`./experiments/30-attribution-census.sh; test $? -ne 1` and `podvm probe --json | jq -e '[.probes[].verdict] | all(. == "ok" or . == "denied" or . == "skip")'`

## Done

**Done 2026-09-08.** Both were run: the census exits **2**, never 1, and the
`jq` exits 0.

⭐ **Rule 1 is enforced by the parent, not by review.** A probe child writes its
verdict on stdout *and* sets its exit status from the operation, and
`pod_hostprobe::child`'s `interpret` refuses the pair when they
disagree. A child that contradicts itself has established nothing, so that is a
`skip`. A child that says nothing, or dies on a signal, is the same. There is no
path by which an exit code alone becomes a verdict.

⭐ **Rule 2 fired four times on the first real runs**, which is what a third
state is for: the missing squash fixture, `kcmp(2)` absent from this kernel,
`/mnt` absent (before the mount probe was corrected to read its own errno), and
`$HOME` absent inside the reconstruction. None of them printed as a denial and
none of them was counted as one when the rung was chosen; `podvm probe` lists
them under a heading that says so.

⭐ **Rule 3 is the type, not a convention.** `Verdict::exit_code` and
`Verdict::from_exit_code` are one another's inverse, so 0, 1 and 2 mean the same
thing in the child, in the parent and in every script in `experiments/`.

⚠ The corrected mount probe is worth naming: it used to `stat` `/mnt` first and
report a `skip` when it was absent. That is a precondition check in the wrong
place: a seccomp filter answers `EPERM` before the syscall body, so on the
reconstruction the policy would have gone unmeasured. It now runs the call and
separates the precondition from the **errno**: only `ENOENT` is a statement
about the target.

# T-0111: Cache the probe result, and key it on what actually decides it

**Source:** `TOOL.md` section 6.1, last paragraph
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

The probe forks 50 children, and everything downstream branches on
its answer. Re-running the whole set for every `run` and every
`exec` is the cost `TOOL.md` section 6.1 asks to avoid by caching.
⛔ **A cache keyed wrongly is worse than no cache**, and this is the
one component where a stale answer is the exact failure the project
exists to prevent: a `namespace` verdict served to a process that
has a `chroot` is podvm telling the lie it was built to refuse.

## Premise

⭐ **Measured, and it disagrees with the specification's key.**
`TOOL.md` section 6.1 says to key on the boot id and re-probe when
it changes. Read on 2026-09-08, one command in three places:

```
host                     01703f0e-380a-462e-ab15-36460e5a8be7
the reconstruction       01703f0e-380a-462e-ab15-36460e5a8be7
a plain docker container 01703f0e-380a-462e-ab15-36460e5a8be7
```

`/proc/sys/kernel/random/boot_id` is the **kernel's**, and every
namespace on that kernel reads the same value. The three above
produce three different probe answers: `namespace` on the host and
`chroot` inside the reconstruction, measured by
`experiments/130-probe-parity.sh`. So the boot id cannot tell them
apart, and a cache keyed on it alone would serve the host's answer
to a confined process on the first `run` after a probe.

## Approach

Key on the boot id **and** on what the probe measured about the
confinement. ⚠ Three of the four below are already read by
`pod_hostprobe::identity`; the fourth is not and is
new work this entry carries:
1. the boot id, which catches a reboot;
2. `/proc/self/uid_map`, `/proc/self/gid_map` and
   `/proc/self/setgroups`, which `pod_hostprobe::identity`
   already reads and which change with the user namespace;
3. `Seccomp` and `Seccomp_filters` from `/proc/self/status`, which
   change when a filter is installed;
4. ⭐ **the mount namespace's inode**, `readlink("/proc/self/ns/mnt")`,
   which is what actually differs between the three readings above
   and is the one component **nothing in the tree reads today**.
   It is the whole reason this entry is not a two-line change.
Write `$store/probe.json` as the existing `--json` document plus
that key, and re-probe when any component differs.
⚠ The document is already versioned by `"podvm"`, and
`crates/podvm-probe/src/report.rs` is the one writer. Adding a
second serializer for the cache is the copy-paste this project's
`docs/conventions/code.md` forbids: the cache stores what `--json`
prints.

## Decision

Refuse to use a cache whose key does not match, rather than
refreshing part of it. A partial refresh means two halves of one
answer measured under two confinements, which is the class of
defect [T-0109](#t-0109-a-verdict-is-the-operations-and-could-not-run-never-reads-as-denied)
is about. ⚠ The alternative considered and rejected is no cache at
all: 50 forks is small, but `run` and `exec` are the hot path and
`TOOL.md` section 6.1 asks for it by name.
⛔ **Not blocked, and not startable either.** There is no `$store`
until [T-1102](milestones.md), M1, so this lands beside the store
and not before it.

## Prove

`podvm probe --json > /tmp/a.json && podvm probe --json > /tmp/b.json && jq -e --slurpfile a /tmp/a.json '.rung == $a[0].rung' /tmp/b.json` and, inside `./experiments/20-enter-target.sh`, a probe run after a host run selects `chroot` rather than reading the host's cached `namespace`

## Done

**Done 2026-09-08**, beside the store, as the entry's `Decision` said it would
be. `crates/podvm-image/src/probe_cache.rs` holds the store I/O and the
comparison; `pod_hostprobe::identity` holds the one reader of the
key. `experiments/170-probe-cache.sh` is both halves of the `Prove` and exits 0.

⭐ **The measurement that made this entry necessary reproduced, on one kernel,
in one run:**

```
host      rung=namespace  boot_id=b538b475-930e-4dd1-9dec-3098dd77212f
confined  rung=chroot     boot_id=b538b475-930e-4dd1-9dec-3098dd77212f
measured now, because the confinement changed: mnt_ns, uid_map, gid_map,
setgroups, seccomp, seccomp_filters differ(s) from the cached run
```

Equal boot ids, different rungs. A cache keyed on the boot id alone serves the
host's `namespace` to a `chroot` process, which is the lie podvm exists to
refuse. `experiments/results/probe-cache.txt` is that reading.

⭐ **The cache stores what `--json` prints, verbatim**, and the key travels
inside the document as `cache_key`. `crates/podvm-probe/src/report.rs` stays
the one writer, so there is no second serializer to drift against the first.
Five of the seven components also appear in the document's `identity` block;
`docs/conventions/forbidden-patterns.md` forbids a value in two places **with
no check that they agree**, and that check is the test
`the_cache_key_agrees_with_the_identity_block_it_is_derived_from`.

⛔ **An incomplete key never matches.** A component that could not be read is
`null`, and `differences()` reports every missing component as differing on
either side, so two `None`s can never produce a cache hit established by the
absence of evidence.

⚠ **`podvm probe` does not touch the store; `podvm probe --cached` is the hot
path.** A diagnostic that can answer from a file is not a diagnostic, so the
plain verb always measures. `--cached` is what `run` and `exec` take at M3, and
it is already on a real hot path: `podvm pull` uses it to get the write
allowlist T-0203 ranks by free space, so the cache is not dead code waiting for
M3. Where it serves, the evidence line on stderr says so by name, and where it
re-measures it says which components differed.

⚠ **`--cached` and `--rows` are refused together**, exit 2: the stored document
carries no per-probe rows, and letting one flag silently win is the class of
defect this entry is about.

# T-0110: `podvm probe` exit-code and channel contract

**Source:** `TOOL.md` section 5 M0; `references/compforge__pathshim/tree/README.md:65`
**Category:** probe
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

A probe whose result has to be parsed out of prose is not a gate.
The audience is automated and needs one line on stdout and an exit
code it can branch on.

## Premise

Measured in the corpus, at file and line. `pathshim` prints the
machine-readable verdict on **stdout**, the reason on **stderr**,
and exits 1 when a normal invocation would degrade:
`references/compforge__pathshim/tree/src/main.rs:134-138`. Its
README states the contract at
`references/compforge__pathshim/tree/README.md:65`: `bind-view`
and 0, `passthrough` and 1, invalid input and 2.

## Approach

`podvm probe` prints the selected rung on stdout, the evidence on
stderr, and exits 0. `podvm probe --strict` exits non-zero when
the selected rung is below `namespace`, so a caller can gate on it
without parsing.
⚠ podvm diverges from `pathshim` in one place and it is
deliberate: `pathshim`'s `run` degrades to an unmapped passthrough
and executes the command anyway
(`references/compforge__pathshim/tree/src/main.rs:160-172`). That
is honest and it still lets a weaker mode satisfy the request.
podvm refuses instead, because a `-v` that silently became a
passthrough shows the payload the host tree.

## Decision

Add `--json` beside the one-word stdout line rather than replacing
it. The word is what a shell branches on; the JSON is what a
harness diffs against `experiments/results/`.

## Prove

`test "$(podvm probe)" = chroot && podvm probe >/dev/null; test $? -eq 0`

## Done

**Done 2026-09-08.** Run inside `./experiments/20-enter-target.sh`, where the
selected rung is `chroot`: stdout is the single word `chroot` and the exit code
is 0. Unconfined the same command prints `namespace` and exits 0, which is the
other half of [T-1101](milestones.md)'s acceptance.

⭐ **Three stdout formats, one per flag, and the evidence never shares the
channel.** Bare prints the rung; `--json` prints one document; `--rows` prints
every probe row in the format `verification/probe` emits, which is what makes
the milestone's row-for-row comparison a mechanical diff rather than a reading.
`--json` and `--rows` together are refused with exit 2 rather than interleaved.

⚠ Invalid input exits **2**, adopting `pathshim`'s contract at
`references/compforge__pathshim/tree/README.md:65`. The divergence this entry
names is unchanged and is not yet reachable: podvm refuses rather than
degrading into a passthrough, and that decision lands with `run` at M3.

# T-0112: A virtual machine answers the two questions this host's kernel cannot

**Source:** Asked by the operator on 2026-09-09: "can we not run a microvm to get these features anyway"
**Category:** probe
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

⛔ **Three of podvm's own measurements have been "cannot be
answered here" since M0**, and every session has re-reported them
to the operator as open questions. This host's kernel has neither
`CONFIG_SECURITY_LANDLOCK` nor `CONFIG_CHECKPOINT_RESTORE`, so
`landlock_create_ruleset` reports `SKIP` and makes
`experiments/30-attribution-census.sh` exit 2, the `kcmp(2)`
control of [T-0102](#t-0102-the-bogus-argument-discriminator-and-its-two-controls)
cannot answer on any run, and M0's own acceptance could never
exercise `move_mount(-> /tmp/mm-probe)` against a kernel where the
LSM was present.

## Premise

⭐ **Measured on 2026-09-09, and the answer is yes.**
`experiments/290-microvm.sh` boots a stock Alpine `virt` kernel
under QEMU with an initramfs carrying busybox and podvm and
nothing else, runs `podvm probe --json` inside it, and reads the
document back off the serial console.

| | this host | the VM | the target |
| --- | --- | --- | --- |
| `kcmp(-1,-1,...)` control | ⛔ cannot answer, `ENOSYS` | **`ESRCH`** | `ESRCH` |
| `controls_answered` | `false` | **`true`** | - |
| `landlock_create_ruleset(VERSION)` | `denied ENOSYS` | **`ok`, ABI 6** | - |
| `move_mount(-> /tmp/mm-probe)` | `ok` | `ok` | `EPERM` |
| LSMs | - | `lockdown,capability,landlock` | - |

⭐ **The VM's `kcmp` answer is `ESRCH`, which is the target's**,
read at `references/Azathothas__container-research/tree/verification/real/extkernel-newapi.txt:26`.
The control that could not answer now answers, and it agrees.

⚠ **No `/dev/kvm` and no `vmx` or `svm` on this host**, so the VM
runs under TCG, pure software emulation. Measured: the whole
boot-probe-poweroff cycle is **about 6 seconds**, which is cheap
enough to belong in the acceptance rather than in a session's
notes.

## Approach

`experiments/290-microvm.sh`, and
`scripts/common/bootstrap-env.sh` grows two components so a session
does not discover the tools are missing halfway through: `qemu`
(both `qemu-user-static` for the foreign-architecture work of
[T-0911](deps.md) and `qemu-system-x86` for this) and `vmtools`
(`busybox-static` and `cpio`).
⛔ The initramfs is built here rather than reusing Alpine's:
theirs expects to find a repository and mount a root, and each of
those is a step that can fail for a reason unrelated to the
question.
⛔ The JSON is validated with `jq -e` before anything is read from
it, because it is carved out of a serial console and a `jq` over a
truncated document answers `null` to every question, which reads
exactly like a measured absence.

## Decision

A second **machine**, never a second **target**. ⚠ The VM answers
"what does a kernel with Landlock say", and it can never answer
"what does the target say": it is an ordinary kernel with every
capability, which is the **easy** case and proves nothing about
confinement. `experiments/20-enter-target.sh` remains the thing
that models the target, and clause 5 says so in the result file so
a later reader cannot mistake the VM's rung for a finding.

## Prove

`./experiments/290-microvm.sh` exits 0

## Done

**Done 2026-09-09.** Five clauses, all green, in about 6 seconds.

⚠ **One line of this script was wrong in a way worth recording**, because it is
the failure mode this project's third state exists for. `.controls.kcmp` is
`null` on this host, and that is **correct**: the control could not answer, so
podvm records a dash rather than a value (`AGENTS.md` absolute 3) and sets
`controls_answered` false. Printing a bare `null` into a result file reads like a
defect in podvm, so the row's own errno is printed beside it and the line now
says "none: the control could not answer (ENOSYS...)".

⚠ **`/dev/pts` is mounted in the initramfs deliberately.** Without it
`open(/dev/ptmx)` answers `ENOENT` and the row is a statement about this
initramfs rather than about the kernel, which is exactly the mistake
[T-0503](enter.md) records about the reconstruction's `/dev`.

# T-0113: Two probes in one process took each other's scratch name

**Source:** Found on 2026-09-09 by `scripts/dev.sh check` (now `podvm-dev check`), on its first run
**Category:** probe
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

⛔ **The write allowlist reported a path this machine CAN write to
as a `skip`.** [T-0104](#t-0104-the-write-allowlist-by-writing)
measures the allowlist by writing, and every scratch file is
`O_EXCL` so a probe never overwrites what it finds
([T-0101](#t-0101-the-probe-set)). The name it writes carried the
**pid** to keep two concurrent probes apart. ⚠ `cargo test` runs
tests in parallel **threads of one process**, so two concurrent
`probe()` calls shared a pid and therefore a filename: the loser
read `EEXIST` and reported `/tmp` as a skip.

## Premise

⭐ **Measured on 2026-09-09**, and it surfaced as
`probe_cache::tests::the_writable_set_comes_out_of_the_document_the_probe_wrote`
failing for a reason that had nothing to do with the store. It did
not reproduce in eight consecutive full-workspace runs afterwards,
which is the reading to quote rather than "flake": the race needs
two probe calls to overlap, and whether they do depends on how
cargo schedules the suites.
⛔ **It is not confined to tests.** `podvm run` calls
`podvm_probe::run()` for its banner while another thread may be
measuring for `$store/probe.json`, and the failure mode there is a
container told it cannot write to `/tmp`.

## Approach

The tag becomes `<pid>-<counter>`, with a process-wide
`AtomicU64`. ⭐ The pid is unique per machine and the counter is
unique per process, so the pair is unique on both axes.
⚠ A random suffix was rejected: it needs an entropy source, and
this crate calls neighbouring code between `fork` and `execve`
where nothing may allocate or block. A counter needs neither.

## Decision

Fix the name, not the test. Serialising the suite would hide a race
that is real outside it, which is the same ruling
[T-0211](image.md) took and for the same reason.
⛔ The test forks eight **threads**, not processes, because threads
are what shares the pid and processes would pass with the defect in
place.

## Prove

`cargo test -p podvm-probe two_concurrent_probes_do_not_take_each_other_s_scratch_name` passes, and it fails with the tag reduced to `std::process::id()`

## Done

**Done 2026-09-09.** Both halves of the `Prove` were driven: green with the
counter, and red with the tag reduced to the pid, reporting
`a concurrent probe reported /tmp as Skip: the scratch names collided`.

⭐ **`scripts/dev.sh check` found this on its first run** (now `podvm-dev check`), which is
[T-1004](packaging.md)-adjacent evidence for that script existing:
`cargo test --workspace` had been run many times in this session and had not
surfaced it, and the one thing `check` does differently is run clippy over every
target first, which changes what is compiled and therefore how the suites are
scheduled.

# T-1333: The probe cache key carries the capability set

**Source:** issue 28, client beta testing 2026-09-22 (same key across
  a capability change serves a stale verdict);
  `pod_hostprobe::identity`
  (`ConfinementKey`: eight components, no capability field;
  `cap_eff`/`cap_bnd` are read at `:26`-`:28` but never keyed)
**Category:** probe
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`ConfinementKey`'s own rule is "everything a probe verdict
depends on that podvm can read without re-running the
probe", but `CapEff`/`CapBnd` are read into `Identity`,
printed by the probe, and missing from the key, while the
chroot verdict is a live `chroot(2)` turning on
`CAP_SYS_CHROOT`. A process that drops capabilities under
the same mount namespace and seccomp filter produces the
same key and is served the stale verdict.

## Premise

Argued from the key's fields and the probe report, not
driven (the reporter could not stage a live capability
change: `CapBnd` already 0). Stated as read, per the issue's
own scope note: a gap against the cache's stated rule.

## Approach

Add `cap_eff` and `cap_bnd` to the key (and its fixed-order
components); separately decide whether `seccomp_filters` (a
count) can stand for the filter's identity, since two
different filters read identically. Out of scope: changing
what the probe measures, invalidating strategies beyond the
key.

## Decision

Key the two capability sets; the seccomp-identity question
is answered by measurement in the change (two filters, one
count, different strictness) and keyed too if they collide.

## Prove

`cargo test -p podvm-probe` green with a new test
building two identities differing only in `cap_eff` and
asserting different keys (plus the reporter's scenario as a
fixture where stageable). Close issue 28 with a comment
showing the test and the widened key as the guard that stops
recurrence.

## Done

**Done 2026-09-23.** Key the two capability sets, as decided.
`ConfinementKey` (`pod_hostprobe::identity`) gains `cap_eff`
and `cap_bnd`, read from the `Identity` the probe already carries; the
fixed-order `components()` goes from eight to ten, so `differences()`,
`missing()`, `is_complete()` and the document's `cache_key` block (written
through `components()` in `report.rs`) all follow with no second list.
`probe_cache::key_of` reads the two fields from stored documents, and a
document written before they were keyed carries neither: it stays `None`
against a live key that always has them and is re-measured rather than
served, which is the intended behaviour and not a migration (an answer
taken under unknown capabilities is exactly the one that must not be
served). The identity/key agreement test covers the new pair, and the two
prose counts move with the shape (seven-hole to nine, eight-host to ten).

The new test was watched fail first: two live identities differing only
in `cap_eff`/`cap_bnd` read as the same key (`a dropped capability set
reads as the same key`), then green after the widening
(`identity::tests::two_identities_differing_only_in_capabilities_are_different_keys`,
lane `rust:1.98.1-bookworm` through host podman 6.1.2). Full suites green
beside it: podvm-probe 96, podvm-image 117, podvm 107,
podvm-extract 47, 0 failed.

On the seccomp-identity half of the Decision: no measurement in this
change stages two same-count filters, and none is claimed. `/proc` exposes
a filter count (`Seccomp_filters`), never a filter identity, so two
different filters read identically and there is nothing further to key
from readable state; the count stays the keyed component. What would
reopen it is a readable filter identity from the kernel, or a cache
validation that re-runs a filter-sensitive discriminator probe on each
read. The reporter's scenario (a live capability drop) is covered by the
widened key above, which is the guard that stops recurrence.

# T-1629: Record `95-podman-vfs-ignorechown.sh` as KEEP-SHELL, a deployment proof under VC-3 and VC-6

**Source:** `refactor/recon-c.md:176`;
  `refactor/06-entries/PLAN.md:171-173` and `:185-188`
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

The ledger marks this script RUST-TEST and its own planned
proof was ignored for a reason. Both the verdict and the
level are wrong in the machine-readable file.

## Premise

Its own exit-2 path does not exist. The subject is the
engine's own storage option rather than podvm code, so no
Rust test can replace it.

## Approach

Record KEEP-SHELL and the level: a deployment proof needing
a live engine, not a unit test. Record the ignored proof so
a later session does not read the ignore as an oversight.

## Decision

Both the verdict and the level go in the same record,
because correcting one leaves the other wrong.

## Prove

`grep -n "95-podman-vfs-ignorechown" TODO/probe.md` names
KEEP-SHELL, the deployment level, and the live engine it
needs.

## Correction

**Recorded 2026-10-02.** The ledger row
`experiments/95-podman-vfs-ignorechown.sh 4 RUST-TEST group-4.md:498`
(`refactor/06-entries/verdict-ledger.tsv:105`) is wrong on both the verdict
and the level. `refactor/06-entries/PLAN.md:171-173` (VC-3) corrects the
verdict to **KEEP-SHELL** and `PLAN.md:185-188` (VC-6) corrects the level to
a deployment proof. Both stand in the same record because correcting the
verdict alone leaves the level wrong.

⭐ **The level is deployment proof, not a unit test**, under
`docs/development.md:46`, "Use integration and deployment proof for the
actual default path." The subject is podman's own storage option, not podvm
code, so no Rust test can replace it.

The script's own header settles the level without an inference.
`experiments/95-podman-vfs-ignorechown.sh:19-21` reads "Exit: 0 the
combination opens the path, 1 it does not. Never 2: this script runs where
the session's engine host is". **Its exit-2 path does not exist**, which is
what the Premise claimed and what a unit test could not stage.

⛔ **It needs a live engine.** The script reaches the engine through a
machine, at `:33` `MACHINE="podman-machine-default"` and `:75`
`podman machine ssh "$MACHINE" 'id; podman --version'`. An unreachable
engine is recorded as "the combination is not shown here" at `:76`, not as
a failure. Every load runs under `podman unshare` with a staged seccomp
filter (`:150`, `:166`), and the control at `:112` is labelled "the fixture
is valid (rootful control)". A rootless engine cannot stand in for it:
`:11-12` records that rootless maps every uid into the subuid range, so
nothing fails and the chown wall never appears.

⚠ **The ignore is a level consequence, not an oversight.** The script is
the live `Prove` of `TODO/image.md:499` T-0205, and `TODO/image.md:502`
records it as done with exit 0 under host podman (client 6.1.2, machine
5.8.6). The ignore that `PLAN.md:187` describes belongs to the **Rust port
that was never written**: a `#[ignore]` cannot exist here, because the tree
carries no Rust test for `ignore_chown_errors` at all. Searching
`crates/` for `ignore_chown_errors` and for an `#[ignore]` beside any vfs or
ignore-chown subject returns nothing. So the ignore is not a defect in the
tree to repair; it is the reason a unit test must not be added, and it
already had its correct answer from the shell script.

⛔ **Where the machine-readable file still disagrees.** The ledger at
`verdict-ledger.tsv:105` is the only machine-readable row and it still reads
RUST-TEST. This record is the correction; `refactor/` is untracked, so no
tracked ledger row carries the verdict yet.

## Done

**Done 2026-10-02.** The ledger row `experiments/95-podman-vfs-ignorechown.sh 4
RUST-TEST group-4.md:498` (`refactor/06-entries/verdict-ledger.tsv:105`) is
corrected on both the verdict and the level. It is recorded as KEEP-SHELL
and as a deployment proof, not a unit test, because the script tests podman's
own `vfs.ignore_chown_errors` storage option and reaches it through
`podman machine ssh "$MACHINE"` at `:75`. The Premise is confirmed from the
script header: `experiments/95-podman-vfs-ignorechown.sh:17-18` says that
the script never exits 2, because it "runs where the session's engine host
is", so no exit-2 path exists. The ignore described in `PLAN.md:187` belongs to a Rust port that was
never written. `grep -rn "ignore_chown_errors" crates/` exits 1 and returns
nothing. The `Prove` matches: `grep -c "95-podman-vfs-ignorechown"
TODO/probe.md` prints 4, and each line names the KEEP-SHELL verdict or the
deployment level. `verdict-ledger.tsv:105` still reads RUST-TEST; that row is
in the untracked `refactor/` tree.

# T-1666: A security probe inside the payload, and a compare against a reference report

**Source:** issue 186; #193; the owner's ruling of 2026-11-06
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

`podvm probe` measures the host's mechanisms before a run. No
tool reports what a payload actually gets inside a tier: which
escalations it can make, which host paths it can reach, what
`/proc` and identity it sees. An honest banner needs both
halves.

## Premise

The probe runs one disposable child for each probe and records
the errno, not a boolean (T-0101, T-0109). It marks an
emulator's answers in `measured_by`
(`crates/podvm-probe/src/report/document.rs:23`). sand-net's probe was
Python, which busybox and distroless images do not have. The
issue's crate is podcrates `pod-sandprobe` (wave M7).

## Approach

Build a static musl probe that runs inside the payload and
writes records `SECTION<TAB>TARGET<TAB>VERDICT<TAB>DETAIL` in the
sandprobe format and verdict vocabulary. Add `podvm
probe-inside [--podvm-tier T] [--profile P] -o FILE` and
`podvm compare REPORT --reference REF [--strict]`, with an
expected-differences file in the repository. `--strict` exits
1 only for an unexplained difference in an invariant section.
Scrub secret-shaped values. Run the matrix of every tier on
the target host class and commit it.

## Decision

A verdict is emitted only for something observed; "could not
test" is UNKNOWN with the reason, the T-0109 rule. A compare
between hosts is never a pass by itself: only the invariant
sections can fail, and only without an observed cause. The
interposer and dispatch tiers are not boundaries, and the
report shows where a host path stays reachable. Take
`pod-sandprobe` when podcrates releases it.

## Prove

`cargo test -p podvm-probe` exits 0 with tests: a planted token
is REDACTED, and the compare exits 0 on two reports of one tier
and 1 on a planted escape in an invariant section.
`sh experiments/422-probe-inside.sh` exits 0: the static probe
writes a report inside a musl row of DISTRO_ROWS_M5, a busybox
image and a distroless image, and inside a machine-tier guest, and
the per-section matrix is saved under `experiments/results/`.

## Correction

**Filed 2026-11-06.** Quoted from issue 186:

```text
verdicts: ALLOW, DENY, ABSENT, UNKNOWN, TIMEOUT, UNRESOLVED, REFUSED,
  DROPPED, EXHAUSTED
invariant sections: escalation, escape-surface, procfs, raw-socket, proxy,
  tcp-direct, tcp-direct-ip, udp-direct, loopback, metadata, dns
errand-bailey-rust against the reference (2026-11-03): 382 same, 9 differ
  (each explained), 26 only in the reference, 86 only here; --strict exit 0;
  646 records (ABSENT 349, ALLOW 167, DENY 84, UNRESOLVED 30, DROPPED 9,
  UNKNOWN 6, EXHAUSTED 1)
F10: kernel.yama.ptrace_scope 0 here and 1 in the reference, so /proc/1/mem
  opens
matrix vs errand-bailey: podman-none 307 / 92, docker-none 305 / 86,
  docker-open 306 / 85; containers differ most in fs (54)
```

## Done

**Done 2026-11-11.** `cargo test -p podvm-probe` exits 0 with
`a_planted_token_is_redacted`, `the_compare_passes_one_tier_and_fails_a_planted_escape`
and `a_line_parses_back_to_its_record` (`crates/podvm-probe/src/inside/tests.rs`).
`PODVM_422_MIRROR=1 sh experiments/422-probe-inside.sh` exits 0
(`experiments/results/probe-inside.txt`, and each report under
`experiments/results/probe-inside/`):

| clause | reading |
| --- | --- |
| a report in each row | alpine (the musl row), busybox and distroless on the ladder's rung; alpine on the chroot-denying fixture, on the syscall tier there, and in a TCG guest of the machine tier: exit 0, 60 or 61 records each |
| two reports of one tier | 60 same, 0 differ; `--strict` exit 0 |
| a planted escape (`escape-surface host-store ALLOW`) | `--strict` exit 1, the line reads UNEXPLAINED |
| the chroot rung against the machine tier | 18 differ, 13 in invariant sections, each explained by `experiments/probe-inside-expected.tsv`; `--strict` exit 0 |
| the matrix | saved, one column for each row |

How it is built, and what differs from the Approach:

* The probe is podvm's own static binary: `probe-inside` copies it into
  the container's own tree (T-1709) at `/.podvm/sandprobe`, or into the
  guest of the machine tier, and runs it as the payload with
  `--podvm-sandprobe`. So no image needs a shell or an interpreter.
* The checks (`crates/podvm-probe/src/inside/`) are the sections of the
  issue: identity, escalation, escape-surface, procfs, raw-socket, proxy,
  dns, tcp-direct, tcp-direct-ip, udp-direct, loopback, metadata, and env.
  A check that can change a state runs in a disposable child; no check
  attaches to, stops or signals another process (`ptrace` is measured by
  `process_vm_readv` of one word of PID 1), and a mount that succeeds is
  unmounted at once. `escape-surface host-store` asks whether the host's
  store path is reachable from the payload.
* The env section is in the report and not in the saved results: the
  variables of a host do not belong in the repository.
* podcrates has no `pod-sandprobe` (its M7, P3, after a study), so the
  probe is built in `podvm-probe`, as T-1652 and T-1657 built the rung in
  `podvm-dispatch`.
* The emulate and incus tiers are not in the matrix: the lane installs no
  qemu-user and no incusd.
* The probe found T-1730: a detached thread of a static payload ended
  with SIGSEGV on the native-static rung.
* Findings of the matrix: the chroot rung mounts no `/proc` and no `/dev`
  in the image (the identity fields that come from `/proc/self/status`
  read UNKNOWN there), and on this host the payload of the ladder's rung
  can unshare, mount and open raw sockets, because it is root of the host.

# T-1670: The first cloud session measures its host and saves the result

**Source:** the migration of 2026-11-06 to cloud sessions; issue 114 (the
  target host); `pod_hostprobe::probes`;
  `pod_hostprobe::probes`;
  `crates/podvm/src/doctor.rs:149`;
  A-V-10 of PodUtils/podcrates#39
**Category:** probe
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

From 2026-11-06 podvm is worked in cloud sessions, not on the
operator's Windows host. No record says what a cloud session's
host grants. An entry that assumes `/dev/kvm`, user namespaces
or a direct network on that host can be wrong without anyone
seeing it.

## Premise

`podvm probe` measures most mechanisms in disposable children:
the seccomp listener (`probes.rs:225`) and Landlock
(`probes.rs:231`) among them. `podvm doctor` reports the machine
legs (`doctor.rs:149`). The probe does not measure Syscall User
Dispatch (T-1652 adds it), and neither tool tests which
registries answer through the egress proxy. The main session
adds a setup script for cloud sessions, scripts/cloud-setup.sh.

## Approach

In the first cloud session, run the setup script, then
`podvm probe --json` and `podvm doctor`. Add measurements that
the probe lacks, in a tracked experiment that prints its
conditions: `/dev/kvm`, user namespaces, `chroot`, `/dev/ptmx`,
`/dev/fuse`, seccomp, the `prctl` of Syscall User Dispatch,
`openat2(RESOLVE_IN_ROOT)`, a manifest fetch from each registry
of DISTRO_ROWS_M5 through the proxy, and free blocks and inodes.
Save the output under `experiments/results/`. Compare the
result with issue 114 and name each difference.

## Decision

A measurement on the cloud host is a property of that host
(T-1203). Exit 2 means could not run, never denied. A host
property that changes an entry's premise is corrected in that
entry, in place.

## Prove

`sh experiments/424-cloud-host.sh` exits 0 or 1 in the first
cloud session after `sh scripts/cloud-setup.sh` exits 0, with
its output and `podvm probe --json` saved under
`experiments/results/`, and `./target/release/podvm-gate`
exits 0 with the result file cited here.

## Correction

**Amended 2026-11-09.** The ask A-V-10 of PodUtils/podcrates#39 (COORD-015,
a message from a peer session) wants a published JSON schema of `podvm probe
--json`. The ask does not fit the clauses of this entry. This entry saves
one document and publishes no schema. The saved `podvm probe --json`
document is the first sample that the schema must accept. The schema belongs
to podcrates podcrates@T-107 (the schema of `pod-hostprobe`). No podvm entry takes the
schema yet. When podcrates podcrates@T-107 releases it, podvm needs an entry that adds
a test that the output of `podvm probe --json` validates.

**Amended 2026-11-10, for podrunner.** T-1714 (issue 239) is the entry
that the amendment of 2026-11-09 asked for: it validates `podvm probe
--json` against the schema of podcrates@T-107, and it is blocked until
that schema has a release. The podvm driver of podrunner reads `probe
--json` (the podrunner design review, SPEC section 10.1).

## Done

**Done 2026-11-10.** `sh scripts/cloud-setup.sh` exits 0, and `sh
experiments/424-cloud-host.sh` exits 0 with [its output](../experiments/results/cloud-host-424.txt);
`podvm probe --json` is [saved beside it](../experiments/results/cloud-host-probe.json).
The cloud host (kernel 6.18.44, x86_64, uid 0, seccomp 0) has no
`/dev/kvm`, and has `/dev/fuse`, `/dev/ptmx` and `/dev/net/tun`. It
permits user namespaces, mount namespaces, chroot, Syscall User Dispatch
and `openat2(RESOLVE_IN_ROOT)`; `vm.memfd_noexec` is 0. Each registry of
`DISTRO_ROWS_M5`, Docker Hub and `mirror.gcr.io` answer `/v2/` through the
egress proxy (200 or 401); the request limit is separate: on 2026-11-09
and 2026-11-10 `public.ecr.aws` and Docker Hub answered image pulls with
HTTP 429, and `mirror.gcr.io` served them. Compared with issue 114 (the
errand host): the errand host denies chroot, user namespaces, mount and
ptrace and has no `/dev/fuse`; the cloud host permits each of them. So a
cloud session reaches the no-chroot rungs only through the seccomp fixture
of experiment 356 (`experiments/deny-chroot.c`), and an entry that needs
the errand host's denials proves them there. `podvm doctor` exits 0 with
the TCG profile.

# T-1689: The non-goal `remedy` follows the stance in the probe JSON

**Source:** issue 213; T-1306 (the non-goals); call C13
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`podvm probe --json` writes the fixed refusal remedy of each
non-goal on every stance. On an open row, "no TCP listener
here" and "ptrace denied here" contradict the row's own
`stance`. On the ceiling row, an open stance still says "size
guests under the ceiling". The stderr block prints `detail`
only and is correct. No check reads `remedy`.

## Premise

`refused`, `open` and `unestablished` copy `def.remedy`
(`crates/podvm-probe/src/nongoals.rs:125`,
`crates/podvm-probe/src/nongoals.rs:135`,
`crates/podvm-probe/src/nongoals.rs:145`), and so does
`assess_ceiling` (`crates/podvm-probe/src/nongoals.rs:193`,
`crates/podvm-probe/src/nongoals.rs:203`).
`crates/podvm-probe/src/report/document.rs:277` writes it for
every row. The test at `crates/podvm-probe/src/nongoals.rs:280`
reads `detail` only. T-1306 is done and stays done.

## Approach

The refused row keeps the remedy. The open row has an empty
`remedy` (call C13). The unestablished row names what settles
it. Rename the field of `DEFS` to `refusal_remedy`. Change
`assess_ceiling` the same way. Extend the unit test to the
open and the unestablished stances.

## Decision

A field that a caller reads as advice holds only what the
stance supports. The JSON key stays, so a reader keeps working.

## Prove

`cargo test -p podvm-probe` exits 0 with tests: an open row
has an empty remedy, an unestablished row's remedy names a
settling step, and a refused row's remedy is in its detail.

## Correction

**Filed 2026-11-09.** From issue 213 (an outside account). The triage of
2026-11-09 confirmed each claim at `2be31b2`. The fix is in podvm:
`pod-hostprobe` has no non-goals.

## Done

**Done 2026-11-09.** The field of `DEFS` is `refusal_remedy`. A refused
row keeps it as `remedy`, an open row has an empty `remedy` (call C13),
and an unestablished row names the step that settles it (`SETTLE`).
`assess_ceiling` follows the same rule. `cargo test -p podvm-probe` exits
0 with `the_remedy_follows_the_stance`. [The drive](../experiments/results/non-goal-remedy.txt):
`podvm probe --json` on the cloud host gives five open rows with an
empty remedy and the refused `kvm acceleration` row with "no KVM here;
the machine tier runs TCG".

# T-1714: `podvm probe --json` validates against the published schema of `pod-hostprobe`

**Source:** issue 239; A-V-10 of PodUtils/podcrates#39; T-1670 (its
  amendment of 2026-11-09); podcrates podcrates@T-107; ask 2.7 of
  COORD-021 (issue 227); the podrunner design review (the
  owner's design review of PodUtils/podcrates#47), SPEC section 10.1;
  `crates/podvm/src/main.rs:361`
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

A caller reads `podvm probe --json` to decide what a host
can run. No schema states its shape, so a change of a field
breaks the caller and nothing tells it.

## Premise

podcrates podcrates@T-107 owns the schema of `pod-hostprobe`. T-1670
saved the first sample
(`experiments/results/cloud-host-probe.json`) and said that
podvm needs this entry when podcrates@T-107 releases the schema. The
`--json` switch of `probe` is `crates/podvm/src/main.rs:361`.

## Approach

Take the podcrates release that carries the schema. Add a
test that validates the saved sample and a fresh `probe
--json` against it.

## Decision

The schema belongs to podcrates; podvm tests its output
against it and keeps no copy.

## Prove

`cargo test -p podvm-probe` exits 0 with a test that the report object of `probe --json` validates against the schema of the taken tag, and a test that the schema refuses a report of another version or rung

## Correction

**Filed 2026-11-10.** From issue 239, the amendment of T-1670 and the
podrunner design review (the ruling "podrunner", 2026-11-10): the podvm
driver of podrunner reads `probe --json`.

**2026-11-14.** The Blocker cleared: podcrates `v0.42.0` (`b6983fc`,
PodUtils/podcrates#145) carries podcrates@T-107, `report::schema()` and
`SCHEMA_ID` behind the feature `schema` of `pod-hostprobe`. The Blocker
was: "podcrates podcrates@T-107 has no release; clears when a podcrates
tag carries the schema of `pod-hostprobe`". The Prove named `cargo test
-p podvm`; the report is written in `podvm-probe`, so its test is there.
The schema states the report object of `pod-hostprobe`, which `probe
--json` holds under the key `hostprobe` since T-1746; the other keys of
`probe --json` are podvm's own and are not in the schema. The saved
sample of T-1670 predates the key, so the test validates a fresh probe.

## Done

**Done 2026-11-14.** `crates/podvm-probe/src/report/schema_tests.rs`:
`the_report_of_probe_json_validates_against_the_published_schema`
compiles `report::schema()` of the taken tag with `boon` (JSON Schema
2020-12, the validator of podcrates' own test) and validates the
`hostprobe` object of a fresh `probe --json` on the cloud host; and
`the_schema_refuses_a_report_of_another_version_or_rung` shows that the
check is not a pass of each object. `boon` and the feature `schema` are
dev-dependencies only: the binary links neither.


# T-1746: `run` reads the cached probe, and probes again only when its key changes

**Source:** PodUtils/podcrates#124 (the asks of podswarm to podvm, a message from a
  peer session; PodUtils/podcrates#107 is its tracker), ask V-2
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

Each `run` probes the host again (`crates/podvm/src/run/prepare.rs:112`
calls `podvm_probe::run()`). A podswarm worker that starts one attempt
for each task pays the probe each time.

## Premise

The probe cache of T-0111 exists, with the key of T-1333; only `probe
--cached` reads it. `podvm_image::probe_cache::resolve` gives the cached
result while its key matches the confinement of the machine, and measures
again where it does not.

## Approach

`run` takes its findings from `probe_cache::resolve`. A run says on
stderr, with `--debug`, whether the rung came from the cache or from a
probe now, as `probe --cached` does.

## Decision

The key of T-1333 decides when the cache is stale; this entry adds no
second rule.

## Prove

`sh experiments/446-run-probe-cache.sh` exits 0: with `PODVM_DEBUG`, a run with no cache says that the probe ran now and the next run says that it came from the cache; a cache of another build is not served; the median of 10 runs of `podvm run --rm IMAGE true` with a warm cache is lower than the median with no cache by at least 80 percent of the median of 10 runs of `podvm probe --json`

## Correction

**Blocked 2026-11-13 on podcrates.** `run` needs the structured
`pod_hostprobe::Findings` (`ptmx_usable`, `chroot_usable`, the banner,
the selection), and the cache holds only the JSON document of
`report::document`. `Findings` has no reader: its rows carry the
`&'static str` names of `probes::PROBES`, and no type derives a
deserializer. A second serializer in podvm would break the rule of
T-0111 (one writer, the key inside the document). COORD-069 asks podcrates
for a reader beside the writer in `pod-hostprobe`. podcrates answered
(PodUtils/podcrates#128, its call C241): podcrates@T-485 gives
`report::write(&Findings, &Selection)` and `report::read(document)` for
the crate's own types, in the next tag. podvm's side: `report::document`
embeds that object under one key beside podvm's own fields, and the cache
reads the key back with `report::read`. The condition that clears the
block: a tag of podcrates with podcrates@T-485.

**Amended 2026-11-13.** The effort is M, not S: the cache needs a reader of
its document, measured in `crates/podvm-image/src/probe_cache.rs` and
`pod-hostprobe` `v0.35.0`.

**Unblocked 2026-11-13.** podcrates `v0.36.0` (`408fb13`) carries
podcrates@T-485 (PodUtils/podcrates#129): `report::write` and
`report::read` are in `pod-hostprobe` behind the feature `report`. podvm
takes `v0.39.0` (`4c2d17c7`, COORD-080, issue 309), which keeps them. The
entry is open, at low priority (the owner's ruling "Complete release
first", 2026-11-13). The feature is off by default, so the work turns it
on where podvm writes and reads the document. COORD-069 (issue 291)
stays open until this entry is done.

**2026-11-13.** The Prove named a clause of experiment 170. Its clause 3
needs a docker daemon, which the cloud host does not have, so 170 exits 2
there whatever it adds; the clause is experiment 446. The Approach named
`--debug`: podvm has no such flag, so the line prints where
`PODVM_DEBUG` is set. The Decision added no second rule; one is needed:
the cache serves the rows and `self_exe` of the build that wrote it, so
`probe_cache::resolve` measures again where the document's `podvm` or
`self_exe` is not this process's. `probe --cached` and `run` share it.

## Done

**Done 2026-11-13.** `podvm-probe` turns on the feature `report` of
`pod-hostprobe`; `report::document` holds the object of `report::write`
under the key `hostprobe`; `Probe::findings` reads it back with
`report::read`; `prepare` takes the findings from
`probe_cache::resolve` and probes now only where the cache holds none.
Experiment 446 on the cloud host (the namespace rung): the medians of 10
runs are 140 ms with no cache and 57 ms with a warm cache, and `probe
--json` takes 76 ms: a warm cache saves 83 ms, more than 80 percent of
the probe (60 ms). The test
`the_findings_read_back_and_another_build_is_measured_over` holds the
reader and the rule of the build.



# T-1812: `probe --cached` reports `supervise` where `probe` reports `userland`

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), F-13 (MISC-01; verify item 11)
**Category:** probe
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

As uid 1000, `podvm probe` prints `userland` and `podvm probe --cached`
prints `supervise`, measured and then served from the cache; `--cached
--json` has `.rung = supervise`, and `--cached --strict` names
`supervise`. As root both print `namespace`. The cache is documented to
store "what `--json` prints, verbatim".

## Premise

`main` applies `crate::system::to_entered` before it renders the live
answer (`crates/podvm/src/main.rs:389`); the writer of the cache
(`crates/podvm-image/src/probe_cache.rs:97`, `measure`) takes the
selection alone.

## Approach

Apply the entered rung before the document is written to the cache, and
serve it as written. Add the cache key's version, so that a cache of an
older podvm is measured again.

## Decision

One document for the live and the cached answer.

## Prove

`cargo test -p podvm --bin podvm probe_cached_rung` exits 0, with a test that `probe --json` and `probe --cached --json` give one `.rung` on a selection above the entered rung

## Done

**Done 2026-11-14.** `cached_probe` (`main.rs`) answers from the
findings of the cache with `Selection::choose`, `to_entered` and
`report::document`, as the live `probe` does, and `--strict` takes
`Selection::exit_code`; the cache itself is unchanged, so `run`, which
reads its findings, is unchanged. The test `probe_cached_rung_is_the_live_rung`
writes a document of the cache and reads its findings back. By hand on
the fixture of the userland rung: beta.17 gave `probe` `userland` and
`probe --cached` `namespace`; this build gives `userland` for each.


# T-1835: `compare --strict` passes an empty report, an unparsable one, or one that lost whole sections

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), MISC-04
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`podvm compare empty.txt --reference empty.txt --strict` exits 0 ("0
same"). A file of two prose lines against a real reference exits 0 with
"61 only in the reference". A report with its `proxy` section removed
exits 0 with "4 only in the reference". A flipped invariant verdict
exits 1, as it must.

## Premise

`Outcome::code` (`crates/podvm-probe/src/inside/compare.rs:77`) counts
only the unexplained differences; nothing reads `only_reference`.
`parse_report` (`crates/podvm-probe/src/inside.rs:158`) drops a line that
does not parse.

## Approach

Refuse a report with no record or with a line that does not parse and is
not a comment. Under `--strict`, count an invariant section of the
reference that is absent here as unexplained.

## Decision

A strict gate fails when it measured nothing.

## Prove

`cargo test -p podvm-probe compare_strict` exits 0, with tests that an empty report, a prose file and a report with a missing invariant section each exit 1 under `--strict`

## Done

**Done 2026-11-15.** `compare` reads each file with
`inside::parse_report_checked`, which refuses a file with no record or
with a line that is neither a record, a `#` note nor blank (exit 1, the
line named); under `--strict`, an invariant section of the reference
with no record here fails the compare and the summary names it
(`Outcome::missing_invariant`). `cargo test -p podvm-probe
compare_strict` exits 0. By hand with a report of `probe-inside
mirror.gcr.io/library/alpine:3.20` (62 lines): an empty file and a file
of two lines of prose exit 1 as no report; the report with its 4 `proxy`
lines removed exits 1 under `--strict` with "invariant sections of the
reference with no record here: proxy"; the report against itself exits
0.

# T-1903: `podvm probe` takes userland on a host with user namespaces and KVM, and does not say why

**Source:** PodUtils/podvm#397 (the live run of 2026-11-15)
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

On a WSL2 host where unprivileged user namespaces and `/dev/kvm` work, `podvm probe` says "the probes select supervise, and run enters userland" ("namespaces: none").

## Premise

The namespace rung needs more than a user namespace (a mount namespace, a writable `/proc` view, the legs of `pod-hostprobe`); the probe names the result, not the leg that failed.

## Approach

Read the legs of the namespace rung on such a host, and make the summary of `probe` name the first leg that fails for each stronger rung.

## Decision

A probe that does not take a stronger rung says which leg stopped it.

## Prove

`podvm probe` on a host where the namespace rung is refused names the failed leg in its summary; a test of the summary holds it.

## Done

**Done (2026-11-15).** `podvm probe` names on stderr the leg that keeps each stronger rung from the run (`crates/podvm/src/system/rung_why.rs`), from the census rows: `unshare(CLONE_NEWNS)` for the namespace rung, with the answer of `unshare(CLONE_NEWUSER)`, and `chroot(2)` for the chroot rung. The cause on such a host: the namespace rung of pod-enter calls `unshare(CLONE_NEWNS)` in the namespace of the caller and never opens a user namespace, so an unprivileged caller gets `EPERM` where `unshare -Urm` works. The entry through a user namespace is T-1909 (COORD-132, PodUtils/podvm#406). Evidence (cloud host, kernel 6.18, uid 1000): `podvm probe` prints "the namespace rung needs unshare(CLONE_NEWNS) in the namespace of the caller, and it gives EPERM; a user namespace opens on this host (unshare(CLONE_NEWUSER) ok), and the namespace rung does not enter one yet" and "the chroot rung needs chroot(2), and it gives EPERM". The test `each_weaker_rung_names_the_leg_that_stopped_the_stronger_ones` holds the summary.


# T-1907: Under QEMU user mode, `probe` says `kvm: yes` for the device of the host

**Source:** PodUtils/podvm#401 (the live run of 2026-11-15)
**Category:** probe
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

In an arm64 container on an x86_64 host, `podvm-aarch64 probe` says `kvm: yes`: the x86_64 `/dev/kvm` cannot run an aarch64 guest.

## Premise

The leg opens `/dev/kvm`; under QEMU user mode the open passes through to the x86_64 kernel. The leg is in `pod-hostprobe` (podcrates).

## Approach

Ask podcrates for a leg that checks that the device runs a guest of the architecture of the build (an ioctl of that architecture, as `KVM_ARM_PREFERRED_TARGET` on arm64): COORD-133 (PodUtils/podvm#407), 2026-11-15. Take the tag that answers it.

## Decision

`kvm` is yes only for a device that runs a guest of the architecture of podvm.

## Prove

`podvm-aarch64 probe` in an arm64 container on an x86_64 host says `kvm: no` with the reason.

## Done

**Done (2026-11-15).** Run 43754060132 (`kvm.yml`, request 433, `6539446a`), job `arch`: an aarch64 podvm cross-built from the tree, under `qemu-aarch64` 10.2.1 (Debian's `qemu-user`) on an x86_64 runner with `/dev/kvm` (`crw-rw-rw-`): `probe --rows` gives `open(/dev/kvm, O_RDWR) FAIL errno=25 ENOTTY`, and `PODVM_MACHINE_ACCEL=kvm run --podvm-tier=machine` exits 125 with "PODVM_MACHINE_ACCEL=kvm asks for KVM, and /dev/kvm does not open or runs no guest of this arch (T-1907): podvm does not fall back to TCG". PASS.

**Partial (2026-11-15).** podcrates `v0.52.0` (podcrates@T-676, the answer to COORD-133) asks the device a question of the build's architecture after the open (`KVM_ARM_PREFERRED_TARGET` on arm64, `KVM_GET_SUPPORTED_CPUID` on x86_64), so the probe row `open(/dev/kvm, O_RDWR)` fails with "the device runs no guest of aarch64 (QEMU user mode?)". podvm's machine tier and Windows planner opened the device themselves; both now take the same leg (`machine::ssh::kvm_opens`), and `PODVM_MACHINE_ACCEL=kvm` names the reason. Remaining: the Prove, by the job `arch` of `kvm.yml` (an aarch64 podvm under `qemu-aarch64-static` on an x86_64 runner with `/dev/kvm`); the cloud host has no `/dev/kvm`.

**Partial (2026-11-15, CI).** Run 43740331439: the job `arch` stopped at its first step: the runner image has no package `qemu-user-static` ("has no installation candidate"). The job now takes `qemu-user` where that package is missing, and runs the `qemu-aarch64` that it finds. Remaining: the next run of the job.

# T-1909: The namespace rung does not enter through a user namespace

**Source:** PodUtils/podvm#397 (T-1903), COORD-132 (PodUtils/podvm#406)
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

On a host where an unprivileged user opens a user namespace (`unshare -Urm` works), podvm enters the userland rung: the namespace rung of pod-enter calls `unshare(CLONE_NEWNS)` in the namespace of the caller, which needs `CAP_SYS_ADMIN` there.

## Premise

A user namespace gives the caller the mount and chroot rights inside it. `unshare(CLONE_NEWUSER | CLONE_NEWNS)`, a one-line `uid_map` and `gid_map`, then the mount steps and the `chroot` of the rung, give the namespace rung to such a caller. The launch and the probe legs are in pod-enter and pod-hostprobe (podcrates).

## Approach

Take the tag of podcrates that answers COORD-132, select the rung where its new leg works, and run the namespace experiments as uid 1000 on such a host.

Or podvm opens the namespaces itself, before the rung is selected. Measured 2026-11-15 on the cloud host (kernel 6.18, uid 1000, podvm at fbc0e9c4): under `unshare -Ur`, `system info` gives `namespace`, and `run --rm alpine:3.20` enters it (`id` gives uid 0, `apk --version` runs), with "the namespace rung runs with no /proc: mounting proc on /proc failed with errno 1"; under `unshare -Urpf` the payload has `/proc` and `uid_map` reads `0 1000 1`; under `unshare -Urpfu` with `hostname unit1`, `uname -n` gives `unit1`. So a foreground `run` can call `unshare(CLONE_NEWUSER | CLONE_NEWPID | CLONE_NEWUTS)` at the start of `main` (one thread), write `setgroups` and the two maps, and fork the rest as pid 1. ⛔ First audit each pid that a record keeps: in the PID namespace `getpid` and `Child::id` give namespace pids, and `stop` or `kill` from outside would signal a host process of that number. `run -d`, `create` and `exec` stay outside until they enter the namespaces of the container (`setns`).

## Decision

The ownership view follows the map; the interposer view (T-0711) keeps each id that the map does not hold.

## Prove

As uid 1000 on a host where `unshare -Urm true` exits 0: `podvm system info --format '{{.EnteredRung}}'` prints `namespace`, and `podvm run --rm IMAGE sh -c 'cat /proc/self/uid_map'` shows the map of the user namespace.

## Done

**Done (2026-11-15).** podvm opens the namespaces itself (`crates/podvm/src/userns.rs`), so it does not wait on COORD-132. At the start of a foreground `run` of a caller that is not uid 0, while podvm has one thread, it tries the whole entry in a child that it throws away; where each step works, it calls `unshare(CLONE_NEWUSER | CLONE_NEWPID | CLONE_NEWUTS)`, writes `setgroups` and the maps (`0 UID 1`, `0 GID 1`), and forks pid 1 of the PID namespace (a reaper that forwards SIGHUP, SIGTERM, SIGUSR1 and SIGUSR2), which forks pid 2: the rest of podvm, in a mount namespace with a `/proc` of the PID namespace. The probe then selects the namespace rung, and `--hostname` names the UTS namespace (T-1908). `run -d`, `create`, `exec` and `--podvm-tier` keep the entry as before, because a record keeps pids. Where a step fails (a host that refuses user namespaces, or AppArmor's restriction of Ubuntu 24.04), the trial fails and the ladder decides as before. `PODVM_USERNS=0` turns it off (call C21). Evidence (kernel 6.18, uid 1000): `experiments/458-userns-entry.sh` PASS (`experiments/results/userns-entry.txt`): mode=namespace with the banner line, `uid_map` `0 1000 1`, `uname -n` `unit1`, exit 7, SIGTERM with a background child gives 143 and no process stays, `run -d` exits 0; the 25 integration test binaries pass as uid 1000 with the entry; under the `deny-unshare` fixture the run takes mode=userland. A first form killed pid 2 at the end of a run: the sweep of T-1708 read host pids from the host's `/proc`; the `/proc` of pid 2 fixed it.

**Done, amended (2026-11-15, later).** The QA pass of experiment 443 as uid 1000 (sections img, run, imgs, tools, life and sys) found defects of the entry, which the release `0.2.0-beta.22` carries: (1) the detached helper of T-1781 (the removal of a `--rm` tree and the fill of the spare) died with pid 1, and each run left a `.trash-*` tree; pid 1 now sends the code of pid 2 to the outer process on a pipe, so `run` returns at once, and stays for the helper; at each exit of pid 2 (`atexit`, so also the bound of `--timeout`), pid 2 sends SIGKILL to each other process of the namespace but the prefill (`userns::sweep`), so a daemon of the payload does not hold pid 1; pid 1 then replaces itself with `podvm --podvm-ns-reaper` (the same pid, with no word of the payload in its command line), which reaps for 600 s at most; (2) `mknod` is refused in the user namespace, so `/dev/null`, `/dev/zero`, `/dev/random` and `/dev/urandom` were the regular files of the fixups (`/dev/urandom` ended after 1 MiB); pid 2 binds the four nodes of the host onto the tree, and `podvm_extract::tree::remove` and `discard` take the binds off first (`unbind_devices`, with `UMOUNT_NOFOLLOW`); a first form gave the nodes as `-v` volumes, which moved each run to the loader family (T-1761); (3) the first extraction printed the ids that the map of the namespace does not hold; there, as outside it, they stay the caller's and the sidecar keeps them, so a quiet run says nothing; (4) `probe` and `system info` said `userland` where `run` enters `namespace`; they take the same entry now (T-1713); (5) a fill of a tree is named `.fill-PID-ID`, and the sweep of `podvm_extract::tree` removed a fill whose pid it could not see: a run in another PID namespace removed the fill of a live run, which then failed with "the container's own rootfs could not be made" or ran an incomplete tree; a fill now holds an `flock` on `.fill-PID-ID.lock` beside it (`podvm_extract::live::FillLock`), and the sweep keeps a fill whose lock is held (16 pairs of runs of python:3.12-slim at the same time, one in the namespace and one in the fixture: 0 failed, no tree left). The record of a run names host pids (pidfd and the `fdinfo` of the host's `/proc`), and `--hostname` left the refused list of experiment 443. The `git clone` rows fail on both rungs on the cloud host only: uid 1000 cannot read the CA bundle of its proxy.


# T-1911: In the user namespace of a run, `-u UID` is faked: the map holds the caller only

**Source:** experiment 443 as uid 1000 (2026-11-15, the row `run.user-deny.ns`), T-1909
**Category:** probe
**Milestone:** none
**Priority:** P3
**Effort:** M
**Status:** partial

## Problem

The user namespace of a foreground `run` (T-1909) maps only the caller (`0 UID 1`). `run -u 1000 IMAGE sh -c 'echo x > /root/qa'` cannot drop to uid 1000, so the interposer fakes the identity (T-0711), and the write succeeds where docker refuses it. As the userland rung did before T-1909.

## Premise

`newuidmap` and `newgidmap` (setuid root, the package uidmap) write a map with the range of `/etc/subuid` and `/etc/subgid` of the caller; rootless podman does the same. Measured on the cloud host: both are installed, and uid 1000 has `100000:65536`. With a range, `setuid(1000)` in the payload works, and the ids of the image (gid 42 of `/etc/shadow`) map too.

## Approach

Write the map from the outer process with `newuidmap PID 0 UID 1 1 START COUNT` (the child waits on a pipe), where the tools and a range exist; else keep the one-line map. ⛔ First solve the store: a file that a payload writes as uid 1000 lands on the host as uid `START+999`, and a run outside the namespace (`run -d`, `create`, `PODVM_USERNS=0`) as the caller cannot read or remove it. Either each verb that touches a tree enters the same namespace, or a tree of the namespace is removed only from inside it.

Measured 2026-11-15 on the cloud host as uid 1000 (range `100000:65536`), with `unshare --map-auto --map-root-user -U` (util-linux runs `newuidmap` from outside): the map is `0 1000 1` and `1 100000 65536`; under `setpriv --reuid 1000`, `echo x > /root/qa` gives "Permission denied"; a `chown 1000:42` in the namespace shows as `100999:100041` on the host; and uid 1000 outside still removes that file, as the removal of an entry needs only the write right on its directory. So the store rule narrows to a directory that a subordinate id owns and that holds entries (`open_dirs` cannot add the owner's bits to it). ⛔ The map is written from outside: a setuid `newuidmap` that runs inside the namespace does not take root, as root is not mapped there.

## Decision

A one-line map stays the default until the store rule holds for each verb.

## Prove

As uid 1000 on a host with `newuidmap` and a range of `/etc/subuid`: `run --rm -u 1000 public.ecr.aws/docker/library/alpine:3.20 sh -c 'echo x > /root/qa'` fails with `Permission denied`, `run --rm public.ecr.aws/docker/library/alpine:3.20 stat -c %g /etc/shadow` prints 42, and after `run -d` and `rm -f` of the same image no file of the store is owned by a subordinate id.

## Done

**Partial (2026-11-16).** Measured as uid 1000 on the cloud host with `uidmap` installed (`ubuntu:100000:65536`), `PODVM_USERNS_RANGE=1`, alpine:3.20. (1) T-1927 had undone the drop of `-u`: the foreground run nests a namespace of one line (`1000 0 1`), so the payload owned each file of root again, and `echo x > /root/qa` passed with the range too. Where podvm's own namespace holds the range up to the ids (`userns::owners::holds`), `run/user_ns.rs` now plans no nested namespace; the payload drops for real, and the write fails with "Permission denied" again (with no range it still passes, as before). (2) After a kept run of `-u 1000` that left `/var/tmp/d` (mode 700, host uid 100999), `cp` out of and into the tree failed with EACCES, and `exec` got "Permission denied". `cp` and `exec` of a container that is not running now run again in a user namespace with the range (`--podvm-range-again`, `userns/range.rs`, `again_in_range`), where the caller holds each subordinate id: both `cp` directions pass (the host copy belongs to uid 1000), and `exec k cat FILE` and `exec -u 1000 k cat FILE` read the files. A running container keeps the join of T-1775 and T-1910. `rm -f` leaves no file of a subordinate id (the removal ends some seconds after `rm` returns). Remaining: `start` of such a tree (its detached path has the map of one line: `ls` of the directory gives "Permission denied"), then the default. Tried and not kept: `start` again in the namespace with the range (as `cp` and `exec`) starts the payload, but pod-enter's entry there mounts no `/proc` ("mounting proc on /proc failed with errno 1": the namespace owns no PID namespace) and the payload cannot open `/dev/null`; a detached run needs the PID namespace of a foreground run, or the range in pod-enter's own user namespace (a map that pod-enter writes with `newuidmap`, a COORD issue). Tried again with podcrates `v0.68.0` (2026-11-17), where pod-enter makes a PID namespace for `start`: no `/proc` failure now, but the payload gets "sh: can't create /dev/null: Permission denied": pod-enter binds the host's devices only into a user namespace that it makes itself, so in the namespace with the range `/dev/null` stays the placeholder file of the tree. Not kept either; the range in pod-enter's own user namespace stays the way.

**Partial (2026-11-15).** `PODVM_USERNS_RANGE=1` (opt-in, `crates/podvm/src/userns/range.rs`): where `newuidmap`, `newgidmap` and a range of the caller in `/etc/subuid` and `/etc/subgid` exist, a helper forked before the `unshare` runs both tools on podvm from outside, and the map is `0 UID 1` and `1 START COUNT`; else the one line as before. The store rule: `podvm_extract::live::remove_any` removes a tree, and where that fails (a directory of a subordinate id that holds entries), the hook `RANGE_REMOVER` runs `podvm --podvm-range-rm DIR` in a fresh user namespace with the range, where the caller holds each subordinate id; only a tree under `trees/`. Inside the one-line namespace of a run the hook cannot map, and the trash waits for the next verb outside it. Measured as uid 1000 (alpine:3.20): the map is `0 1000 1` and `1 100000 65536`; `run --rm -u 1000 ... sh -c 'echo x > /root/qa'` fails with "Permission denied"; after a kept run that left `/srv/a/b/c` of uid 1000 (3 files of host uid 100999), `podvm rm` leaves 0 such files and no trash; a `--rm` run leaves none. The owners of the image (`crates/podvm/src/userns/owners.rs`): in a namespace with the range, each record of the sidecar `.meta.jsonl` whose owners differ from those on disk, and whose ids the map holds, gets them with `fchownat(AT_SYMLINK_NOFOLLOW)` in a directory opened with `openat2(RESOLVE_IN_ROOT | RESOLVE_NO_SYMLINKS)`, before the payload starts; `PODVM_USERNS_RANGE=1 run --rm alpine:3.20 stat -c '%u %g %a' /etc/shadow` prints `0 42 640` (the default run prints gid 0, as before). The 25 integration test binaries pass as uid 1000 with the removal hook. Remaining: the default, after `exec` and `cp` keep the store rule for a tree of a subordinate id.


# T-1912: `run -d` and `create` of an unprivileged caller do not use the user namespace of pod-enter

**Source:** podcrates `v0.52.0` (PodUtils/podcrates#176, podcrates@T-670 to podcrates@T-672, the answer to COORD-132), T-1909
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

podcrates `v0.52.0` enters the namespace rung through a user namespace with no PID namespace (`pod-hostprobe`'s leg `mount(tmpfs,/tmp) in unshare(NEWUSER+maps,NEWNS)`, and the selection reads `user+mount`). podvm's entry decision asks `chroot_usable`, which is false for such a caller outside a namespace, so `run -d` and `create` still enter the userland rung.

## Premise

Each caller of `podvm_probe::probes::chroot_usable` that decides between the rungs (lifecycle `entry.rs`, `prepare.rs`, `df_info.rs`, `exec`, `ladder.rs`) can take the leg as a way to the namespace rung. The foreground `run` keeps its own entry (T-1909).

## Approach

A helper "the namespace rung is reachable": `chroot_usable` or the leg passes; each deciding caller takes it. Then run experiments 443 and 458 as uid 1000.

## Decision

Not before the rung has the device nodes of the host: in the user namespace `mknod` is refused, and `/dev/urandom` of the fixups ends after 1 MiB, where the userland rung serves the device through the interposer.

## Prove

As uid 1000 on a host where `unshare -Urm true` exits 0: `podvm run -d public.ecr.aws/docker/library/alpine:3.20 sh -c 'id -u; head -c 2000000 /dev/urandom | wc -c; sleep 5'` logs `0` and `2000000`, and `inspect` names the namespace rung.

## Done

**Done (2026-11-15).** podvm takes podcrates `v0.53.0` (COORD-111, PodUtils/podcrates#185), whose entry through a user namespace binds the host's `/dev/null`, `/dev/zero`, `/dev/full`, `/dev/random`, `/dev/urandom` and `/dev/tty` (podcrates@T-688, the answer to COORD-135) and moves its root and its memfd clear of the passed descriptors (podcrates@T-691, COORD-138). `crate::ladder::ns_reachable` (`chroot` works, or the leg `mount(tmpfs,/tmp) in unshare(NEWUSER+maps,NEWNS)` passes) now decides in the ladder, in the gate of `start`, in the two deciders of `lifecycle/entry.rs`, and in the reports of `system info` and `probe`. The join of `exec` and `cp` (T-1910) takes only the namespaces that differ from the caller's (pod-enter's entry opens no PID and no UTS namespace, and a join of the host's PID namespace gave EPERM), and a refused join exits 125 in place of going on outside the run. Measured as uid 1000 on the cloud host: `run -d public.ecr.aws/docker/library/alpine:3.20 sh -c 'id -u; head -c 2000000 /dev/urandom | wc -c; sleep 5'` logs `0` and `2000000`, and `inspect` gives `"Rung":"namespace"`; `exec` gives uid 0 and sees a file that the payload wrote; `cp` copies it out; `stop` and `rm` leave no process; `exec` into a foreground run (T-1910) still sees its own pid 1.

**Blocked (2026-11-15), cleared by `v0.53.0`.** The device nodes on the rung: COORD-135 (PodUtils/podvm#414) asks pod-enter for the binds. Clears when a tag of podcrates carries them.


# T-1921: `chroot_usable` reads the leg in a user namespace as the caller's `chroot`, so `exec` enters into EPERM

**Source:** integration test tmpfs_exec (`docs/HANDOFF.md`, 2026-11-16), podcrates `v0.59.0` (podcrates@T-744); COORD-150 (#443)
**Category:** probe
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

As uid 1000 on a host that refuses `chroot` to the caller and opens a user namespace, `echo run | podvm run -i --name tx --tmpfs /tmp IMAGE tee /tmp/x` then `podvm exec tx cat /tmp/keep` exits 125 with "chroot into the rootfs failed with EPERM". The probe of `exec` gives `chroot=EPERM`, but the entry decision takes the chroot sequence. podcrates `v0.59.0` adds the row `chroot(/tmp) in unshare(NEWUSER+maps)` (podcrates@T-744), and `pod-hostprobe`'s `chroot_usable` takes each row whose name starts with `chroot(`. The leg passes, so `chroot_usable` is true. `rung_why.rs` finds the first such row and names the wrong verdict too.

## Premise

Each caller of `podvm_probe::probes::chroot_usable` asks about the caller's own `chroot(2)`, outside a namespace. The way to the namespace rung through a user namespace is `crate::ladder::ns_reachable` (T-1912), which reads the leg on its own.

## Approach

`podvm-probe` re-exports the module `probes` of `pod-hostprobe` with its own `chroot_usable`, which reads the row `chroot(/tmp)` only (`HOST_CHROOT`); each caller of podvm takes it with no change. `rung_why.rs` finds that row by name. COORD-150 asks podcrates for the same fix in `pod-hostprobe`; podvm removes its shadow when it takes that tag.

## Decision

A shadow in `podvm-probe` and not a change of each caller: the callers keep the one path `podvm_probe::probes::chroot_usable`, and the fix leaves in one place when podcrates fixes the crate.

## Prove

`cargo test -p podvm-probe --lib chroot_usable_reads_the_callers_own_row_only`, and the integration test `crates/podvm/tests/tmpfs_exec.rs` as uid 1000 on the cloud host, with the interposer objects built (`scripts/build-interpose.sh`).

## Done

**Done (2026-11-16).** `podvm-probe` shadows `chroot_usable` with a check of the row `chroot(/tmp)` only, and `rung_why.rs` reads that row by name. Measured as uid 1000 on the cloud host: `tmpfs_exec` failed with EPERM in the chroot sequence before the change, and passes after it; `exec` of the stopped run takes the loader family. The unit test holds the leg `Ok` with the caller's row `Denied` to false.

**Done, amended (2026-11-16).** podcrates answered COORD-150 with `v0.61.0` (podcrates@T-758): its `chroot_usable` reads the row `probes::CHROOT` alone. podvm takes the tag and drops its shadow; the unit test of `podvm-probe` now holds the crate's function to the same answer.

# T-1923: The namespace rung is taken through a user namespace where `chroot` is refused there

**Source:** `arm.yml` run 43819553294 (experiment 356, on `77a44998`)
**Category:** probe
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

Experiment 356 runs podvm under `unshare -Urm` with a seccomp filter that refuses `chroot` (`deny-chroot`), so that only the no-chroot families can run. On the hosted arm64 runner each run of its fixture exited 125 with "the namespace rung runs with no /proc" and "chroot into the rootfs failed with EPERM", in place of the loader family. `crate::ladder::ns_reachable` takes the namespace rung where the leg `mount(tmpfs,/tmp) in unshare(NEWUSER+maps,NEWNS)` passes, and the leg does not call `chroot`. The filter refuses `chroot` in each namespace, so the entry fails after the decision.

## Premise

podcrates `v0.59.0` gives the row `chroot(/tmp) in unshare(NEWUSER+maps)` (podcrates@T-744): the `chroot` of the entry through a user namespace. It is `Denied` under the filter, and `Ok` on a host with no root and user namespaces.

## Approach

`ns_reachable` takes the leg only where the row `chroot(/tmp) in unshare(NEWUSER+maps)` passes too; a table with no such row keeps the leg alone.

## Decision

The row of the crate, not a second probe of podvm.

## Prove

`cargo test -p podvm --bin podvm ns_reachable_needs_chroot_in_the_user_namespace`; experiment 356 in `arm.yml` passes.

## Done

**Done (2026-11-16).** `ns_reachable` needs the `chroot` row of the user namespace where the table has it; the unit test holds the fixture of 356 (each `chroot` refused, the leg `Ok`) to false. Measured: the unit test passes; `arm.yml` runs experiment 356 on the commit of the change.

# T-1962: The donor study: host probes and namespaces

**Source:** PodUtils/podvm#579, the donor study of 2026-11-17
**Category:** probe
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

`pod-hostprobe` asks which mechanism podvm can use. It does not ask which confinement primitives a job can nest inside the chosen rung. The probe selects by mechanism, not by namespace. The chroot rung shows the host's `/proc/kcore`. The namespace rung has no durable handle between runs.

## Premise

Re-verify each claim in `pod-hostprobe` first. Ask podcrates by COORD issue.

## Approach

1. TH-18: probe inward.
2. TH-04: probe anonymous exec memory.
3. TH-05: probe each namespace alone; mask host `/proc` entries.
4. TH-39, TH-10, TH-42 as the issue lists them.

## Decision

-

## Prove

On a rung that denies user namespaces, `probe` states that a nested Landlock works and a nested user namespace does not, with one line for each.
