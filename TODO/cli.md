Area: cli.

Record semantics: [task rules](RULES.md#5-entry-closure).


`crates/podvm`. `TOOL.md` section 6.8 and section 6.9.

[INDEX.md](INDEX.md) is the list and the counts. [PROGRESS.md](PROGRESS.md) is the work order.
[RULES.md](RULES.md) is how an entry closes. [reference-map.md](reference-map.md) is the corpus.

⛔ **An agent that knows docker must need zero new knowledge.** That is the
product requirement the whole specification exists for. Same verbs, same flags,
same exit codes; where a flag cannot be honoured it is accepted and reported in
the banner, and where a flag **requests isolation** it fails loudly with a
named reason.

# T-0801: The verb and flag parity table

**Source:** `TOOL.md` section 6.8
**Category:** cli
**Milestone:** none
**Priority:** P0
**Effort:** L
**Status:** done

## Problem

A tool that needs its user to learn its differences has not
replaced anything. Thousands of agents reach for `docker` because
it is the only container language they know.

## Premise

Read. `TOOL.md` section 6.8 is the table, in full, with a status per verb
and per flag. Four statuses: **Native** (real semantics),
**Degraded** (works, documented difference, stated in the banner),
**Stub** (accepted, no-op, listed in the banner), **None** (fails
with a named reason).

## Approach

Implement the table as data, not as a match arm per flag, so that
`podvm system info` can print it and a test can assert every row
is reachable. A flag with no row is a bug in the table, and the
parser says so rather than ignoring it.
⭐ The posture to copy is `dockless`'s, at
`references/ylang-ylang__dockless/tree/dockless/run.py:129-160`.
`guard_run` refuses `run --name` up front because the engine
beneath it cannot honour it, and warns hard when `--rm` names an
existing container, citing the incident that produced the guard.
⛔ dockless has **no licence statement of any kind**
([reference-map.md](reference-map.md)), so the posture is described
and adopted as a design and no line is copied.
⚠ Its degradation is worth noting from the other side: when it
cannot determine the target it prints that the guardrail itself is
degraded and continues. Naming the degradation of the guard is the
part to keep.

## Decision

A table plus a generated parser, over a hand-written one. The table
is what section 6.8 already is, and generating from it makes "the flag
exists and is unlisted" impossible.

## Prove

`./experiments/320-cli-contract.sh` clauses 1 to 3, whose first clause is `podvm system info --format '{{json .Parity}}' | jq -e 'length >= 60 and all(.status | IN("Native","Degraded","Stub","None"))'`

## Correction

**Amended, 2026-09-10, by the door sweep after T-0412.** ⛔ **One verb was
served by another verb's parser and nothing said so, in either direction.**
`create` is parsed by `run::parse`, which called `admit("run", …)` with the verb
written in rather than passed. Two consequences, and each is the class this
entry exists to make impossible:

- ⛔ `podvm create --no-steps` was **accepted and acted on** while
  `podvm system info` listed `--no-steps` under `run` and `exec` only. The
  table under-described the binary, which is "the flag exists and is unlisted"
  arriving through the door the entry did not enumerate.
- ⛔ `podvm create --no-such-flag` answered **`podvm run: unknown option`**
  and printed `usage: podvm run`, naming a verb the caller had not typed, and
  then sent them to "every flag this verb takes" -- a row set that does not
  exist.

`parity::rows_of` is the one place that says where a verb's rows live, so the
28 rows stay a single copy; `run::usage(verb)` builds the first line from the
verb the caller typed; and the `create` row and the refusal message both name
the borrowing out loud. ⚠ Threading the verb was not cosmetic: `admit` looks the
flag up, so naming the caller's verb without `rows_of` would have made `create`
refuse every flag it accepts. Six functions in `run.rs` took the verb --
`parse`, `prepare`, `acquire`, `extract_now` and `config_of` among them --
because each wrote `podvm run:` into a message `create` and `exec` can reach.

Prove: `a_verb_served_by_another_parser_resolves_that_verbs_rows_and_says_so`
asserts both halves, and driven: `podvm create --no-such-flag` now names
`create`, `podvm create --no-steps` is still accepted, and the table is 141
rows as before, because the fix added none.

## Done

**Done, 2026-09-09.** `crates/podvm/src/parity.rs` is the table,
`crates/podvm/src/system.rs` is `podvm system info`, and
`experiments/320-cli-contract.sh` drives all three of the contracts below
against the shipped binary.

| | |
| --- | --- |
| rows | 287 on 2026-11-11 (`awk '/^pub const TABLE/,/^\];/' crates/podvm/src/parity.rs \| grep -c '^\s*Row {'`). ⚠ 131 when this closed and 265 on 2026-10-01: the table grows with the flags, so a count here is a reading with its date |
| of which verbs | **69** |
| statuses used | Native, Degraded, Stub, None, and no fifth |
| rows with no reason | **0** |

⭐ **THE TABLE DECIDES, and that is the whole difference from a table that
merely describes.** Every argument beginning with `-` goes through
`parity::admit` before any match arm, so the two failures a hand-written parser
has are both structural rather than discouraged: a flag with no row cannot be
quietly accepted, because it never reaches an arm; and an arm for a flag with no
row is unreachable, because `admit` refused it first. A unit test walks the
table and asserts every row the table admits reaches an arm, ⚠ **by the exit
code rather than by a shape parsing**: `--pull` takes a closed set of values and
rejects anything a test could invent, so what the test asserts is that no shape
returns the fallback arm's own `EXIT_RUNTIME_ERROR`.

⚠ Written as universal on 2026-09-09 and measured as partial on 2026-09-21:
only `run`, `exec` (and `create` through `run`'s parser) admitted first;
`images`, `system`, `lifecycle`, `probe`, `version` and the `image` group
refused from hand-written arms. T-0808 threaded `parity::admit_all` through
every one of those parsers, so the sentence above holds since 2026-09-21 and
`experiments/325-parity-drive.sh` asserts it per row, starting from the flag.

⭐ **The refusal carries the row's own reason**, so a caller that reads the
table and then runs the verb gets the same sentence back from the same place:
`podvm run --name c1` exits 2 with "`--name` is in the parity table with status
None: a name identifies a container, and podvm has none until M4", and
`podvm ps` exits 125 with that verb's row. Clause 3 asserts the message a verb
prints is byte-for-byte the note the table published.

⛔ **`--format` grew exactly one function, `json`.** `{{json .Parity}}` is
docker's own spelling and it is T-0801's `Prove`, so refusing it would have made
the acceptance unwritable. It is not a general function call: a verb declares
which of its fields are already documents, `{{json .X}}` on a plain string
quotes and escapes it, and every other function is still refused by name. ⚠ The
alternative, guessing from the value's first byte, makes a string that happens
to begin with `[` come out unquoted, which is a wrong answer that parses.

⚠ **A `None` row is not a placeholder.** It is the sentence a caller gets
instead of a flag being ignored, so deleting one makes podvm quieter and less
honest rather than smaller. A test asserts every one of them carries a reason
longer than a milestone number, which caught eleven rows whose whole note was
"the lifecycle is M4".

**Partial, 2026-09-25.** Issue 60 reopens this entry on its own
invariant: a flag with no row is a bug in the table, and 46 `run`
flags (`--read-only`, `--mount`, `--env-file`, `--label`,
`--init` among them) plus 10 verbs (`manifest`, `service`,
`secret`, `trust`, `config`, `node`, `plugin`, `stack`,
`checkpoint`, `scan`) have no row at all. Measured on the lane
2026-09-25 (`experiments/results/triage-353.txt` clauses
`60-read-only`, `60-mount`, `60-env-file`, `60-manifest`,
`60-service`): each flag answers `unknown option` with no reason
at 125, each verb exits 1 on the neither-podvm-nor-docker arm
with the usage text claiming every other docker verb is named in
the table, which is not true while these are missing. Add a
`None` row with the fitting reason for each refused flag (the
peers already name the reasons: no mount, no namespace, no
cgroup), a `Native` row with a parser arm for any flag promoted
to real behavior (`--env-file`, `--label`, `--init`,
`--read-only`), and rows for the ten verbs on the existing
no-daemon and out-of-shape reasons. Correct the usage sentence
beside the rows. Keep the parser answering `unknown option` only
as the backstop, never as the documented answer. The `--device`
and `--log-driver` rows ride here too (issues 55 and 56, filed
apart because entries name them). Fix area is
`crates/podvm/src/parity.rs` with the `run` and `exec`
parsers for promoted flags. Risk if wrong is an operator
trusting an enumerable table that omits the docker surface they
script against. Prove is the curated flag list from the issue
driven row by row through the shipped binary with every refusal
naming its reason, beside the T-1325 check extension below.

**Done, 2026-09-25.** Every one of the issue's 46 flags and 10
verbs has a row, in `crates/podvm/src/parity.rs` (220 rows,
69 verbs). `--env-file` is Native with a parser arm and a reader
(`run.rs read_env_file`: `KEY=VALUE` lines loaded at the flag's
position, `#` comments and blanks ignored, one matching quote pair
stripped, 1 MiB ceiling, UTF-8, a line without `=` refused naming
its number). `--label`, `--attach` and `--expose` are Stub.
`--log-driver` is Stub for `json-file`, accepted and changing
nothing (one raw interleaved file either way), with any other
driver refused naming it: the shape T-0605 prescribes. The
remaining 41 flags are None with the fitting reason, and the 10
verbs are None on the existing no-daemon and out-of-shape
reasons. The usage sentence in `main.rs` stands unchanged: with
the rows added it is true again, and check 27 holds it.
Prove: `experiments/355-parity-curated.sh` exit 0 on the lane
(kernel 7.2.0-WSL2-STABLE, lane-built musl debug binary
0.1.0-beta.7, pinned alpine:3.20 digest `d9e853e8`):
clause-1 40/40 refused with status None at 125, clause-2 10/10
verbs refused at 125 with their notes, `--env-file` end to end
(`from-file` on payload stdout, a later `-e` winning, the `=`
form equal, a bad line and a missing file at 125), the stub
trio admitted at 0 with `--strict` refusing at 125 naming Stub,
`--log-driver=json-file` accepted in both forms with `syslog`
refused at 125 naming the value, `create` inheriting all of it.
Report in `experiments/results/parity-curated.txt`. In-suite:
`issue_60_curated_surface_stays_covered`,
`env_file_loads_and_stubs_parse`,
`log_driver_takes_json_file_and_nothing_else`, podvm 139
passed, 0 failed. Guards are check 27's curated arm and the
in-suite test beside it.

# T-0802: docker's exit codes, unaltered

**Source:** `TOOL.md` section 6.8
**Category:** cli
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

An automated caller reads the exit code first. A runtime that
returns its own codes breaks every script that branches on
docker's, and a runtime that improves a payload's code lies in the
field read first.

## Premise

Read. docker's convention: the payload's own status for `run`, 125
when the runtime itself could not run the command, 126 when the
command was found and could not be invoked, 127 when it was not
found.

## Approach

Return the payload's status verbatim from `run`, `start` in the
attached case, and `wait`. Use 125, 126 and 127 for the three
runtime-side cases and nothing else.
⛔ Never translate a payload's non-zero status into zero because a
fixup succeeded, and never into non-zero because a fixup warned.
That rule is what T-0409 depends on.

## Decision

Match docker rather than define a clearer scheme. Parity is the
product.

## Prove

`podvm run --rm public.ecr.aws/docker/library/alpine:3.20 sh -c 'exit 42'; test $? -eq 42 && podvm run --rm public.ecr.aws/docker/library/alpine:3.20 /nonexistent; test $? -eq 127`

## Done

**Done 2026-09-09, and the entry's own description of docker's convention was
incomplete.** `experiments/330-exit-codes.sh` runs the same case through both
binaries on one host and compares the numbers, which is the only way this can be
asserted: docker's contract is not written anywhere podvm can read, it is what
the binary does.

⭐ **THE DISCRIMINATOR IS NOT THE ONE THE ENTRY DESCRIBES, AND IT IS THE
FINDING.** docker exits **125** for anything its FLAG PARSER refuses and **1**
for anything the verb refuses afterwards. Measured on docker 29.3.1:

| case | docker | what podvm did before |
| --- | --- | --- |
| `run --badflag`, `--pull=bogus`, `--memory=notasize` | **125** | 2 |
| `images --format '{{.Nope}}'`, `run` with no image | **1** | 2 |
| `rmi no-such-image` | **1** | 125 |
| a verb neither tool has | **1** | 125 |
| the bare name, no arguments | **0**, help on stdout | 125 |
| the command found and not invocable | **126** | **127** |
| the command not found | 127 | 127 |
| the payload's own status, `128+N` for a signal | as docker | as docker |

Four of those eight were wrong. ⛔ The 126/127 one is the one that costs a
caller most: every `execve` failure was folded into "not found", so
`podvm run alpine /etc/passwd` said 127 where docker says 126, and a script
branching on 127 retries with another path. The split is on the **errno**:
`EPERM`, `ENOEXEC`, `EACCES`, `EISDIR` and `ETXTBSY` are 126; `ENOENT` and
anything about resolving the path are 127.

**Prune and rmi on a held image, decided 2026-10-01.** No source defect:
`store.delete` with `Held::Skip` returns `Ok` with the image in `skipped`,
which `prune` reports as exit 0 naming `skipped:`; `store.remove`
(`Held::Refuse`) errors `in use`, which `rmi` reports as 125. The script
(`experiments/160-store-gc.sh:138-139`), the source
(`crates/podvm/src/images/login_extract.rs:222-247`), and
`experiments/results/store-gc.txt` (one `skipped:` line) agree. Pinned by
`rmi_refuses_a_held_image` and `prune_skips_a_held_image` in
`crates/podvm/src/images.rs`.

⭐ **The codes now live in ONE file**, `crates/podvm-probe/src/exit.rs`, and
are served as data by `podvm system info --format '{{json .ExitCodes}}'`.
⛔ They had been written out in **three**: `podvm-image::error`,
`podvm::main` and `podvm-enter`, and two of the copies had already
diverged -- `main.rs` still carried the old usage code while `error.rs` carried
the corrected one, so two verbs of one binary disagreed about what a flag error
is. `scripts/common/exit-codes.sh` is how a shell script reads the table, and
six clauses across four experiments read it there instead of carrying a `2`.

⛔ **A fixup never moves the payload's code**, which is what
[T-0409](complete.md) depends on: clause 5 of the measurement runs the same
payload with and without the completion layer's source rewrite and asserts both
report the payload's own 7.

Prove, run 2026-09-09:

```
$ ./experiments/330-exit-codes.sh
  15 cases, podvm and docker agree on every one
  exit 0
```

# T-0803: Answer to `docker` and `podman` on PATH

**Source:** `TOOL.md` section 2.0, section 6.8
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

An agent runs `docker`. If podvm is only reachable as `podvm`,
it has replaced nothing on the machines it is for.

## Premise

Read. On the studied runtime `docker` on PATH is already a podman
alias with no daemon, so the name is available and currently
resolves to something that does not work.

## Approach

Multicall on `argv[0]`: `docker`, `podman` and `podvm` all enter
the same parser, and the banner names which. Install the two extra
names as symlinks, never as copies, so one binary is one artefact.
⚠ Check what is already on PATH before replacing it, and refuse to
overwrite a real docker client without an explicit flag. A machine
with a working daemon is a machine where podvm is the wrong tool.

## Decision

Symlinks over a wrapper script. A shell wrapper is a second
artefact, it breaks the memfd rung (T-1001), and it is the exact
thing `references/qaidvoid__onelf` records as skipping that rung.
⭐ **RULED by the operator on 2026-09-08, and no longer open:**
podvm **refuses** to take the `docker` name where a working
docker daemon is reachable, unless an explicit flag says otherwise,
and says why in one line. A machine with a working daemon is a
machine where podvm is the wrong tool.
The alternative considered and rejected is deferring to the real
daemon transparently by exec'ing it: it costs an exec on every
call and it hides which tool ran, which is the class of thing
`TOOL.md` section 4.1 exists to forbid.
⚠ The check is on a **reachable daemon**, not on a `docker` binary
being present: on the target runtime `docker` on PATH is a podman
alias with no daemon behind it, so a binary check would refuse on
exactly the machines podvm is for.

## Prove

`./experiments/320-cli-contract.sh` clauses 4 and 5: both names run the payload and say which tool ran, and the `docker` name is refused where a daemon answers unless `--force`

## Done

**Done, 2026-09-09.** `crates/podvm/src/names.rs`, the banner line in `run`
and `exec`, `podvm system install-names`, and clauses 4 and 5 of
`experiments/320-cli-contract.sh`.

⭐ **Multicall on `argv[0]`, and the banner names which name was used.** Taking
the name is the product requirement; taking it silently is what `TOOL.md`
section 4.1 forbids, so every `run` and `exec` reached as `docker` or `podman`
carries one extra line saying this is podvm and where the differences are
listed.

⛔ **The daemon check is on a REACHABLE DAEMON and it is a real request.** The
socket named by `$DOCKER_HOST`, or `/var/run/docker.sock`, is connected to and
asked `/_ping` under a two-second timeout, so a stale socket file with nothing
behind it reads as absent, which is the state the machines podvm is for are
actually in. ⚠ A `$DOCKER_HOST` that is not a `unix://` socket is a THIRD state:
podvm says the guard itself is degraded and continues, which is the half of
`dockless`'s posture worth keeping.

⭐ **The ruling is about the `docker` name only.** On this machine, where a
daemon answers, `podvm system install-names` installs `podman`, refuses
`docker` with the reason, and exits 125; `--force` installs it. ⚠ Clause 5 reads
which state the machine is in first and asserts the other half where it can, so
a machine with no daemon reports the refusal half as unmeasured rather than as a
pass.

⛔ **Symlinks, never copies and never a wrapper script**, and the clause asserts
it on the filesystem rather than on an exit code. A file that is already a link
to this binary is left alone; anything else needs `--force`.

# T-0804: The honesty rules, and one switch that makes every degradation fatal

**Source:** `TOOL.md` section 4.1, section 6.8; `paper_final.md` section 10.1
**Category:** cli
**Milestone:** none
**Priority:** P0
**Effort:** M
**Status:** done

## Problem

The audience is automated. An agent cannot notice that its
"container" was a `chroot` the way a human skimming a log might, so
the reporting is the product and not a disclaimer.

## Premise

Read, and both halves are in the corpus.
`references/RuriOSS__ruri/tree/src/include/ruri.h:237-248` is the
degradation macro that becomes fatal under one flag.
`references/multikernel__sandlock` is the counter-example: a
`--dry-run` that reports no changes while changing files, traced in
T-0606.

## Approach

Four rules, enforced in the output path rather than in review:

1. the banner prints on every `run` and `exec`, suppressible by
   config and never by default;
2. a request that **requires** isolation fails with a named reason
   instead of silently using the host;
3. `inspect` reports the true mode per container;
4. no output may imply namespaces, cgroups or devices exist when
   they do not.

⭐ Add `--strict`, which turns every Degraded and every Stub into a
refusal, so a caller that needs real semantics can demand them in
one flag rather than checking the banner.

## Decision

Suppressible by config, never by default. `ruri` allows
`--disable-warnings` to silence its degradation notices, which is
the `sandlock` failure mode with a flag in front of it; podvm's
equivalent is a config file a machine's operator sets once, and it
cannot be set from the command line of a single run.

## Prove

`! podvm run --rm --network=none public.ecr.aws/docker/library/alpine:3.20 true && ! podvm run --rm --memory=1g public.ecr.aws/docker/library/alpine:3.20 true && ! podvm run --strict --rm --memory=1g public.ecr.aws/docker/library/alpine:3.20 true`

## Done

**Done 2026-09-09.** All four rules, and the third and fourth needed a defect
fixed rather than a switch added.

1. **The banner prints on every `run` and `exec`**, suppressible by
   `<store>/config` with `banner = quiet` and never from the command line. ⭐ A
   file a machine's operator sets once is the Decision: `ruri`'s
   `--disable-warnings` is the `sandlock` failure with a flag in front of it.
   ⚠ Whether a machine has suppressed it is itself reported, by
   `podvm system info --format '{{.Banner}}'`, because a silent banner nobody
   can tell from an absent one is the same shape again. ⛔ A `--strict` refusal
   prints whether or not the banner is quiet: the switch silences a notice,
   never a refusal.
2. **A request that requires isolation fails with a named reason.** Every
   isolation flag -- `--network`, `--privileged`, `--cap-add`, `-m`, `--cpus`,
   `--hostname`, `-v`, `-p`, `-u` -- is a `None` row and
   [T-0801](#t-0801-the-verb-and-flag-parity-table-as-data)'s `parity::admit`
   refuses it up front with the row's own reason.
3. **`inspect` reports the true mode per container**, and the container record
   now carries `Complete.Fixups` and `Complete.Degraded` beside `Rung`. A
   `/dev/null` that is a regular file is part of the mode, and a report that
   left it out would say devices exist when they do not.
4. ⛔ **THE FOURTH RULE WAS BEING BROKEN, AND BY THE BANNER ITSELF.** The banner
   printed `mode=` **the rung the PROBE selected**, and `podvm_enter` performs
   a plain `chroot` on every machine. On this host, where the probe selects
   `namespace`, every `podvm run` printed

   ```
   podvm 0.1.0: mode=namespace (namespaces: as configured; mounts: full; ...)
   ```

   for a payload that had no namespace and no mounts at all. It now reads the
   rung the entry sequence IMPLEMENTS, from one constant
   (`podvm_enter::ENTERED_RUNG`), and prints the machine's own answer beside
   it rather than dropping it:

   ```
   podvm 0.1.0: mode=chroot (namespaces: uts-only; mounts: none; devices: real;
   ownership: real; network: host-shared; pids: host-shared)
   this mode does NOT provide: process, network, IPC or mount isolation
   ⚠ this machine would permit `namespace`; podvm's entry sequence is `chroot`
     and creates no namespace and mounts nothing
   ```

⭐ **`--strict` reads three inputs and names every reason at once**, rather than
the first one it finds:

- **the flags this invocation passed**, against the parity table's own status.
  A `Degraded` or `Stub` row is a difference this run actually incurs;
- **the rung podvm entered with**, against `Selection::STRICT_FLOOR` -- the same
  constant `podvm probe --strict` gates on, not a second spelling of it;
- **the completion layer's report**, one row per fixup marked degraded.

⛔ A `None` row was already fatal without `--strict`, and has been since T-0801
made the table binding, so the switch is about the two statuses that otherwise
let a run proceed.

⚠ **THE `Prove` ABOVE CANNOT PASS AND ITS SECOND CLAUSE IS THE REASON.** It
expects `podvm run --rm --memory=1g ... true` to exit **0** and the same run
under `--strict` to fail. T-0801 landed after this entry was written and made
every `None` flag a refusal in the parser, so `--memory=1g` exits 125 with or
without `--strict` -- the entry's own rule 2, enforced earlier than it expected.
The rewrite, run 2026-09-09:

```
$ podvm run --rm public.ecr.aws/docker/library/alpine:3.20 true; echo $?
  0, and the banner carries `mode=chroot` and the two lines above
$ podvm run --strict --rm public.ecr.aws/docker/library/alpine:3.20 true; echo $?
  podvm run: --strict, and this run is degraded in 2 way(s). podvm refuses
  rather than running and letting the payload discover them:
    - the selected rung is `chroot` and not `namespace`, so the payload shares
      this machine's process table, network, IPC and mount namespaces
    - rewrote etc/mtab (T-0405): ...
  125
$ podvm run --rm --memory=1g public.ecr.aws/docker/library/alpine:3.20 true; echo $?
  podvm run: -m, --memory is in the parity table with status None: resource
  limits need a cgroup this runtime does not grant
  125
```

# T-0805: Diagnostics that name the operation, the errno, the mechanism and the remedy

**Source:** `TOOL.md` section 6.9; `paper_final.md` section 10.8
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

In this environment the errno alone actively misleads. `EINVAL`
from `chown` reads as a bad argument and means an id that does not
exist. `fork/exec <path>: operation not permitted` reads as a
missing binary and is a denied `setgroups`.

## Premise

Read, and `TOOL.md` section 8 is a whole table of them. The target shape:

```
cannot restore ownership of etc/shadow (uid 0, gid 42): EINVAL
  gid 42 is not mapped in this user namespace (/proc/self/gid_map: 0 1000 1)
  extracting without ownership; intended metadata recorded in .meta.jsonl
```

## Approach

Every failure carries four parts: the operation, the errno, the
mechanism that produced it, and the remedy. The mechanism comes
from T-0102's discriminator and the maps from T-0105, so the
diagnostic quotes measured state rather than a guess.
Encode `TOOL.md` section 8's table as the mapping from a raw failure to
those four parts, so a new failure mode is a table row.

## Decision

Quote the actual `uid_map` contents rather than saying "an unmapped
id". The map is one read (T-0105) and it is what turns a confusing
errno into an explanation a reader can act on.

## Prove

`podvm pull public.ecr.aws/docker/library/alpine:3.20 && podvm extract public.ecr.aws/docker/library/alpine:3.20 2>&1 | grep -A2 'gid 42' | grep -q 'gid_map'`

## Done

**Done 2026-09-21.** `crates/podvm/src/diagnose.rs` carries TOOL.md
section 8 as data: 22 rows from observation to operation, errno, mechanism
and remedy. `ownership_note` renders the chown row with live measurements:
the first dropped entry from the sidecar, the map contents T-0105 reads,
and the sidecar path as the remedy. `report_dropped` prints the counts line
and the note. Both `extract` and `run` call it, so one extraction reads one
way on both verbs. The sidecar remembers the first dropped entry and
`Extracted` carries it (`src/sidecar.rs` and `src/lib.rs` of `pod-extract` (PodUtils/podcrates)).

The entry first named `pull` alone in Prove. Pull performs no extraction
(`crates/podvm-image/src/pull.rs:17`), so no pull output can name a dropped
id. Three routes were checked: a pull path that extracts somewhere unseen
(refuted by the pull handler, which prints the probe source and returns), a
predictive diagnostic inside pull (refuted, since pull cannot know dropped
ownership without extracting), and the pull-then-extract flow (adopted).

Run 2026-09-21 in a disposable container on host podman 6.1.2 with guest
maps `0 1000 1; 1 100000 65536`: pull exits 0, extract exits 0 over 517
entries with 1 dropped, and the note names etc/shadow (uid 0, gid 42) with
EINVAL beside the `/proc/self/gid_map` contents. The unit job exits 0: 71
passed in podvm with 5 new diagnose tests, 46 passed in
podvm-extract. Clippy passes with `-D warnings`. The spawn row in the table
stays unwired until T-0809.

# T-0806: Never prompt, never wait unbounded, and check space before every large write

**Source:** `TOOL.md` section 11.3; [RULES.md](RULES.md) section 8
**Category:** cli
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

A prompt with no terminal behind it is a hang, and a hang is total.
The three ways a long autonomous run dies are the three ways podvm
can make its caller die.

## Premise

Read, and one instance is in the corpus at file and line.
`references/89luca89__lilipod/tree/pkg/utils/utils.go:194-214`
checks `getsubids`, `newuidmap` and `newgidmap` with `exec.LookPath`
and returns an unrecoverable error when any is missing, and it runs
before **every** subcommand including `pull`. A dependency check
ahead of every syscall wall is the same class of problem as a
prompt: the tool refuses before it has done anything the user can
use.

## Approach

Three properties, tested rather than reviewed:

1. no code path reads stdin unless the user asked for it with `-i`;
2. every wait has an upper bound and a distinct outcome for reaching
   it (T-0602);
3. `statvfs` for blocks and inodes before every large write, with
   the destination named in the error (T-0203).

⚠ Where podvm needs a tool it does not have, it says so **at the
point of use** and not at startup, so every verb that does not need
it still works.

## Decision

Check at the point of use rather than up front. lilipod's shape
makes `pull` fail on a machine where `pull` would have worked, and
`pull` is the verb an agent reaches for first.

## Prove

`podvm run --rm public.ecr.aws/docker/library/alpine:3.20 true </dev/null && timeout 30 podvm pull public.ecr.aws/docker/library/alpine:3.20 </dev/null; test $? -ne 124`

## Done

**Done 2026-09-09.** All three properties, and each is asserted rather than
reviewed.

1. **No code path reads stdin.** podvm never opens or reads fd 0: the payload
   inherits the caller's, which is what makes `-i` a `Stub` rather than a
   feature. ⚠ `experiments/330-exit-codes.sh` runs every case with stdin
   redirected from `/dev/null` under a `timeout`, so a read that blocked would
   arrive as exit 124 rather than as a hang nobody attributed.
2. **Every wait has an upper bound and a distinct outcome for reaching it.**
   The launcher's are [T-0602](supervise.md)'s. M5 added one more and it is
   bounded too: the HTTPS reachability probe [T-0411](complete.md) makes before
   rewriting a package source gets two seconds to connect and two to read, and
   a host that does not answer keeps its `http://` and is named on the banner.
3. **`statvfs` for blocks and inodes before every large write.** [T-0203](image.md)
   covers the download and the extraction; M5 added the third caller, and it is
   the one a caller with NOTHING can reach: the `/dev` shims are three megabytes
   into a rootfs that may be on a small tmpfs, so `space::require` runs before
   the first of them with the destination named in the error.

⚠ **Where podvm needs a tool it does not have, it says so at the point of use.**
The keyring half of [T-0406](complete.md) is the instance: podvm cannot run
`pacman-key` from outside the chroot, so the fixup reports the state and names
the two commands, rather than refusing the run. That is the Decision's own
shape, against `lilipod`'s at
`references/89luca89__lilipod/tree/pkg/utils/utils.go:194-214`, which refuses
before every subcommand including `pull`.

Prove, run 2026-09-09:

```
$ podvm run --rm public.ecr.aws/docker/library/alpine:3.20 true </dev/null; echo $?
  0
$ timeout 30 podvm pull public.ecr.aws/docker/library/alpine:3.20 </dev/null; echo $?
  0, and never 124
```

# T-0807: `podvm images --format` refuses a template no verb can answer, before it looks at the store

**Source:** Found by driving the CLI cold against an empty store
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

A `--format` template was validated inside the loop over records.
An empty store means zero iterations, so an invalid template was
never seen: `podvm images --format '{{.Nope}}'` printed nothing
and exited 0, and a caller's typo read as an empty result set.

## Premise

⭐ **Measured by running it**, not by reading it, on 2026-09-08.
Three templates exited 0 against an empty store where all three are
refusals: an unknown field, an unterminated opening delimiter,
and docker's
`table` prefix, which rendered the word `table` beside the values.

## Approach

One walk over the template, shared by a `check` that has no record
in hand and a `render` that does, in
`crates/podvm/src/format.rs`. Both verbs call `check` before
the store is opened, against a named list of their own fields.
⛔ The field-name list and the field builder are two places holding
one value, so a test asserts they agree, which is the remedy
`docs/conventions/forbidden-patterns.md` names for exactly that row.

## Decision

Refuse `table` rather than rendering it as literal text. It selects
a column layout in docker and podvm does not have one, so printing
it is output shaped like something podvm does not do.

## Prove

`podvm images --format '{{.Nope}}' >/dev/null 2>&1; test $? -eq 2`

## Done

**Done 2026-09-08.** Driven against an empty store: the unknown field, the
unterminated brace, the `table` prefix and a pipeline all exit 2 and name what
is wrong; `{{.Digest}}` exits 0. `inspect` reports a bad template before it
reports a missing image, because the template is the caller's own input.

# T-0808: Drive every row of the parity table through the shipped binary

**Source:** T-0801; the door sweep of 2026-09-10
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** L
**Status:** done

## Problem

⛔ **The table is 141 rows and nothing asserts the binary agrees
with all of them.** `experiments/320-cli-contract.sh` drives a
handful: the JSON parses, one `None` flag refuses with its own
note, one unlisted flag is refused. The other rows are checked by
a unit test that walks the table and calls the PARSER, which is
the same code that reads the table, so the two cannot disagree
even when the binary is wrong.

## Premise

Found by hand on 2026-09-10 and not by any check: `podvm create
--no-steps` was accepted and acted on while `podvm system info`
listed `--no-steps` under `run` and `exec` only. ⚠ The unit test
could not have caught it, because it asks the table which flags a
verb has and then asks the parser about those; a flag the parser
accepts and the table does not mention is invisible from that
direction. ⛔ **The check has to run the BINARY and start from the
flag, not from the row.**

## Approach

One driver, over the table read out of the binary itself:
1. for every row with a flag, invoke the verb with it and assert
   the outcome its status predicts -- `None` refuses with the
   row's own note and the flag-error code, `Native` and
   `Degraded` do not refuse for the flag's own sake, `Stub` is
   accepted and named in the banner;
2. for every verb, invoke it with a flag no row names and assert
   the refusal names **the verb the caller typed**;
3. the direction the unit test cannot go: for every flag the
   parser has an arm for, assert a row exists. ⚠ This one needs
   the arms enumerated from the source rather than from the
   table, which is the only way the two lists can be compared at
   all.
⚠ Rows that need an image or a container are driven against a
store that has neither, so the assertion is about the FLAG's
refusal and not the verb's; a row whose flag cannot be reached
without state reports the third state rather than passing.

## Decision

Admit-first everywhere, in the same change as the driver. Every
dash-arg passes `parity::admit_all` before any match arm runs, so
an arm for a flag with no row is unreachable and a row with no arm
ends at `parity::no_arm`, which the driver reports as a mismatch.
Group subverbs carry the path the caller typed (`image prune`,
`system abi`) and resolve their rows through `parity::rows_of`.
Approach clause 3 closed structurally rather than by enumeration:
with the pre-pass there is no second list to compare.

## Prove

`./experiments/325-parity-drive.sh`, which exits 0 only when every
row of `podvm system info --format '{{json .Parity}}'` was driven
or reported as unreachable here, and prints the count of each.

## Done

**Done, 2026-09-21.** The driver is green against the shipped binary: 160
rows, 199 driven, 0 mismatches, 0 unreachable here
(`experiments/results/parity-drive.txt`, with the conditions at its head).
Both figures are historical readings; the table holds 265 rows
(`crates/podvm/src/parity.rs:119`).
The 153-row figure this record carried went stale at T-1302, which added
5 rows (the two tier rows, the two qemu-arg rows and `podvm`) without
re-driving, and T-1305 added 2 more (the two `--podvm-mem` rows) with the
re-drive above: 153 plus 5 plus 2 is the 160 the binary publishes.
`run -i` and `exec -i` both ran a self-pulled `alpine:3.20` payload with the
banner naming `-i`. Release build, `clippy --workspace --all-targets` with
`-D warnings`, and `cargo test -p podvm -p podvm-probe -p podvm-extract`
(73, 65 and 46 passed) were all green in the same run. The first drive found
4 mismatches and all 4 were the driver's: group subverbs probed as top-level
verbs, and `exec` driven with the run-only `--pull`. Both fixed in the
script; the second run was green, and a third stayed green after the
`admit_all` pre-pass landed.

**Re-driven 2026-09-22 under [T-0209](image.md), which added the 3 login
flag rows.** 164 rows, 202 driven, 0 mismatches, 2 unreachable here.
The committed 160-row reading had gone stale by one row meanwhile:
[T-1004](packaging.md) added `version --verbose` without re-driving
(160 plus 1 is the 161 the tree carried; plus 3 is the 164 the binary
publishes). The 2 unreachable are `run -i` and `exec -i`, and they
disagree with the sentence above: in the re-drive container both exit
0 and the banner names no `-i`, with and without a pty, so the driver
reports them unreachable here rather than passed. No line of the
`run`/`exec` parsers prints `-i` on a plain run; the TTY theory was
probed and refuted (0 mentions both ways). Whether the 2026-09-21
reading matched a lane-only line or an older banner is unestablished,
and neither the driver criterion nor the banner moves in the T-0209
change that exposed it.

# T-0809: The spawn that fails with the wrong reason, and the one field that fixes it

**Source:** `https://github.com/talaria0101/sandbox-insights`, its walls document, wall 2; T-0805 above
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

A payload inside podvm spawns a child, the spawn fails, and the
message says the operation is not permitted. ⛔ **That one string
covers at least two different refused calls**, and reading it as the
wrong one has cost whole architectures.

## Premise

⭐ **Measured, with the matrix committed, by somebody else.** Go's
`os/exec` issues `setgroups` in the child whenever a credential is
set, and one guard meant to suppress it does not take when its
enabling field is set with no mappings beside it. On a runtime that
denies `setgroups`, the spawn fails.
⚠ **The ambiguity is what makes it expensive.** A spawn that asks
for namespace clone flags AND a credential fails with the identical
string whichever call was refused: the clone runs first, the
`setgroups` runs in the child before the `execve`, and both surface
the same way. Reading it as "namespaces are denied" is the recorded
mistake.
⭐ The remedy is one field and it changes nothing else. Dropping the
credential entirely is not required and has other effects.

## Approach

podvm does not spawn this way: it is Rust and it declares its own
syscalls. ⛔ **The payload does, and podvm is what the payload's
operator reads.** So this is a diagnostic row and not a code change:
T-0805's four-part message gains an entry mapping the observed
string plus a denied `setgroups` leg to the mechanism and the
one-field remedy.
⚠ The discriminator is already probed: [probe.md](probe.md)
separates a refused clone from a refused `setgroups`, so podvm can
say which wall the payload met rather than repeating the payload's
own ambiguous string.

## Decision

Diagnose, never patch. ⛔ podvm does not rewrite a payload's
process attributes: that is a behaviour change inside somebody
else's program, which is a larger thing than
[complete.md](complete.md) T-0407's trust change, and it is not this
tool's to make. It names the wall and the remedy.

## Prove

`./experiments/151-spawn-ambiguity.sh` asserts podvm distinguishes a refused clone from a refused `setgroups` behind one identical payload message, and that the diagnostic names the field

## Done

**Done 2026-09-21.** The probe evidence names the wall per context
(`crates/podvm-probe/src/report.rs`): both legs with what this machine
measured, the payload shape that tells them apart, and
`Credential{NoSetGroups: true}` where setgroups met the denial. It prints
only where a leg is denied. Five unit tests pin the leg matrix, including
the unmeasured leg, which no spawn is attributed to.

No generic string matcher ships. The mapping keys on the probe legs, not
on substrings of payload output, so a matcher that guesses from text has
no caller. The table row from T-0805 stays as the static mapping.

Run 2026-09-21 in a disposable container (go 1.19.8, host podman 6.1.2):
`./experiments/151-spawn-ambiguity.sh` exits 0. Both payloads fail with
the byte-identical `fork/exec /bin/true: operation not permitted`, the
clone half attributed in the plain context and the setgroups half under
`unshare -Ur`, and the control payload exits 0 where setgroups is
allowed. `experiments/results/spawn-ambiguity.txt` carries the run.

# T-0810: `create` fails storing the memo: the container directory is never made

**Source:** T-0708's drive, host podman 6.1.2
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`podvm create --name e1 <image> ...` fails with `the ownership
memo could not be stored: No such file or directory`, so no
container with a record can be created and `inspect` has nothing
to read. `run --detach` shares the rename and fails the same way.

## Premise

Read at file and line. `crates/podvm/src/lifecycle.rs`
`create` renames the ephemeral memo into
`supervise::table::memo_path` beside the new record, and
`crates/podvm/src/run.rs` (detach) does the same. Neither
the rename's parent (`table::dir`, a pure join) nor
`supervise::create` (a table-JSON write only) makes the
directory. Foreground `run` never renames, which is why every
drive to date stayed green past it.

## Approach

Make the directory where the container is created, not where
the memo is renamed: `supervise::create` owns the container's
directories (`lock_path`, `log_path` and `control_path` all
live under `dir`), so it ensures `dir` after the table write.
Both rename call sites ride it with no change.

## Decision

A failed directory fails the creation loudly, beside the
rename's own failure. A container with a record and no memo
answers `stat` with the real uid and contradicts every
foreground run, which is the inconsistency T-0710 refuses.

## Prove

`podvm create --name e1 public.ecr.aws/docker/library/alpine:3.20 true && podvm rm e1`

## Done

**Done 2026-09-22.** `supervise::create` ensures `containers/<id>/` after the
table write (`crates/podvm-supervise/src/lib.rs`). Both memo renames ride it
with no change (`lifecycle.rs` `create`, `run.rs` detach). A directory the
kernel refuses fails the creation loudly. `creating_a_container_makes_its_directory`
creates a container, asserts the directory, and renames a file beside the
record. The entry Prove ran verbatim on host podman 6.1.2 with the shipped
binary (`create --name e1 ... true` then `rm e1`, both green). Row E1 of
the T-0708 drive covers the same create with a payload that writes the
memo: the `mknod` reads back through `inspect` at 1, and `rm -f` removes
the container.
A read-only audit over every other beside-the-record write site (log, lock,
control socket, memo opens in launcher and exec) names an independent
guarantee for each (the launcher's own directory creation, the lock's parent
creation, the memo helper's parent creation). No site writes beside a record
without a directory behind it.

# T-1319: `inspect` prints exactly one document for any reference

**Source:** issue 21, client beta testing 2026-09-22 (container
  inspect emits the record plus a spurious `[]`);
  `crates/podvm/src/images.rs` (the `find_one`, then
  `inspect_container`, then unconditional array print)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`podvm inspect <container>` prints two JSON documents: the
container record (printed inside `inspect_container`) and a
trailing `[]` (the empty image `records` vec, printed
unconditionally after). An automated caller parsing stdout
fails; `jq` cannot consume it. Image references print one
array correctly, and `--format` is unaffected, so only the
container path is broken.

## Premise

Measured by the reporter (`json.load` fails with "Extra
data", `raw_decode` finds two documents) and confirmed
against the loop above: the container branch prints inside
the call and returns `Some(0)`, leaving `records` empty for
the final print.

## Approach

Do not print the trailing image array where the reference
resolved to a container (or collect the rendered container
document into `records`, one path, one print). Out of
scope: changing either document's fields, touching image
inspect.

## Decision

Skip the second print on the container path: the smaller
diff, and the image path keeps its exact bytes.

## Prove

`podvm inspect <container>` piped to `python3 -c
'json.load(sys.stdin)'` and to `jq` both succeed with one
document; image inspect output is byte-identical before and
after. Close issue 21 with a comment showing the parse and
the single-print path as the guard that stops recurrence.

## Done

**Done 2026-09-23.** Skip the second print on the container path, as
decided. `inspect` (`crates/podvm/src/images.rs`) tracks whether any
reference resolved to a container; where the image `records` vec is empty
and a container document is already on stdout, the trailing array print is
skipped and the verb returns the container's code. The image path is byte
for byte what it was: the array prints wherever a record resolved,
including the empty store (`[]` with no container involved).

Driven on the shipped binary in the lane (`rust:1.98.1-bookworm` through
host podman 6.1.2), against `public.ecr.aws/docker/library/alpine:3.20`:

```
$ podvm create --name green1 ... true && podvm inspect green1
[{"Command":"true",...,"Name":"green1",...}]
$ podvm inspect green1 | python3 -c 'json.load(sys.stdin)' && echo PARSES
ONE-DOCUMENT len=1
$ podvm inspect public.ecr.aws/docker/library/alpine:3.20
[{"Architecture":"amd64",...}]      # one array, 14 keys, as before
```

Before the fix the same container printed the record plus a trailing
`[]`, and `json.load` failed with `Extra data: line 2 column 1`. The
guard that stops recurrence is the single-print path: there is now one
place that decides what `inspect` prints without `--format`, and a second
document cannot be added without passing it. `--format` is untouched: it
returns before the array either way.

The `jq` half ran beside it on a clean store in the same lane
(`rust:1.98.1-bookworm` through host podman 6.1.2):
`create --name jq1`, then `inspect jq1 | jq .` (JQ-CONTAINER-OK) and
`inspect <image> | jq .` (JQ-IMAGE-OK), one line each, both rc=0.
(The first attempt reused the T-1315 job's store after its byte flip,
so `create` failed there for the tampered-store reason; the rerun used a
fresh store.)

# T-1323: `cp` reaches an image's extracted rootfs, and copies directories with the same checks

**Source:** issues 14 and 16, client beta testing 2026-09-22 (`cp`
  gated on container records; no recursive copy);
  `crates/podvm/src/parity.rs` (the `cp` row: one file
  at a time, gated through the containment check)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

`cp` only addresses `container:path`, and `run` leaves no
record, so on a host where the payload cannot launch there
is no checked way to stage or retrieve files: the only
interface with containment checks is gated on a started
container, which restricted hosts cannot always provide.
Separately, `cp` copies one file at a time; a recursive
option with the same checks has no entry.

## Premise

Measured by the reporter: `cp alpine:latest:/etc/os-release`
refuses ("no such container"), `create`+`cp` works only
where `start` works, and raw store writes omit every
check. The containment machinery (`podvm-extract` safety
module) and the extract walker already exist to make both
halves safe.

## Approach

Accept `image:path` (the extracted rootfs in the store) as
well as `container:path`, reusing the containment-checked
path unchanged; add a recursive option over the same walker
with the same checks. Out of scope: a new stage/unstage
verb pair (rejected: `image:path` covers the workflow with
no new verb and no parity debt), documenting raw store
surgery (never an answer).

## Decision

Extend `cp`, do not add verbs. One verb, one checker, two
new addressings.

## Prove

`podvm cp image:/etc/os-release` on a host where
payloads cannot launch retrieves bytes, and `podvm cp` of
a directory tree round-trips with containment intact (a
hostile symlink inside the tree refuses rather than
escapes); `podvm cp --help` documents both. Close issues 14
and 16 (cp half) with comments showing the commands and the
shared containment check as the guard that stops recurrence.

## Done

**Done, 2026-09-23.** `cp` addresses `container:path` first and
`image:path` second (last-colon split, so a tag survives), extracting
on demand under the image hold; `-r` copies a tree over the same
`contain::within` gate in both directions. One deliberate deviation
from the Prove letter: a symlink inside the tree replicates verbatim,
it does not refuse. The first lane run proved the letter unworkable:
`cp -r alpine:3.20:/etc` refused on `/etc/mtab -> /proc/mounts`, and
every real rootfs carries such absolute links (TODO/extract.md
T-0305 settled this for extraction: absolute targets replicate, never
resolve). What refuses instead is a destination through a
pre-existing symlink, which would land where the link points.
Lane prove [`pb-w23c-prove`](../docs/history/lane-proofs-2026-09-23/pb-w23c-prove.txt), verdict `fail=0`:
`cp IMAGE:/etc/os-release` retrieves Alpine bytes with pull only (no
extract, no container); `cp -r IMAGE:/etc` lands 45 files plus 4
symlinks with `mtab` a link to `/proc/mounts` verbatim against the
rootfs; an imported absolute link replicates with its target intact
and stays a link; a planted destination link refuses naming the
symlink with nothing through it; the tree round-trips into a created
container byte-identical (`cmp` clean); a directory without `-r`
names `-r`; `--help` documents both addressings. Six unit tests by
exact name, `cargo clippy --workspace --all-targets -- -D warnings`
clean, `cargo test --workspace` exit 0. The guards that stop
recurrence are the shared `within` gate on every rootfs-side path,
the no-follow walk with verbatim link replication, and the
write-through-symlink refusal on every destination.

# T-1330: Bundled short flags parse as docker reads them

**Source:** issue 27, client beta testing 2026-09-22 (`ps -aq`,
  `run -it` refused); `crates/podvm/src/parity.rs`
  (the `-a`/`-q` and `-i`/`-t` rows, never mentioning
  bundling)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

Combined short flags are refused: `ps -aq` and `run -it`
answer `unknown option` with 125. Docker callers write
clusters constantly; the parity table lists the member flags
separately and never mentions bundling, so the refusal reads
as a missing flag rather than a missing spelling.

## Premise

Measured by the reporter on the beta.3 asset, both forms,
both 125. The parser has no cluster path (each flag is
matched whole).

## Approach

Parse a cluster of value-less short flags by docker's own
rule (a cluster containing a flag that takes a value
consumes the rest as its value; an unknown member refuses
naming the member). One parser change, every verb inherits
it. Out of scope: new flags, changing any refusal code.

## Decision

Docker's cluster rule exactly, not a subset. A subset would
move the surprise rather than remove it.

## Prove

`ps -aq`, `run -it` (and a cluster with an unknown member,
refusing by name) behave as docker's rule says across three
verbs; the existing flag tests still pass unchanged. Close
issue 27 (bundling third) with a comment showing the runs
and the single parser path as the guard that stops
recurrence.

## Done

**Done, 2026-09-23.** One rule, `parity::expand`, called at the top of
every flag-taking verb before admission: value-less shorts split
(`ps -aq`, `run -it`), a value-taking member consumes the rest
(`stop -t5`, `-eFOO=bar`, `-f{{.Id}}`), an unknown member refuses
naming the member. The value-taking shorts live once in
`CLUSTER_VALUES`, held to the table by unit test; verbs that stop at
the image (`run`, `exec`, in `CLUSTER_BOUNDARY`) expand only before
it, so `run IMG -la` still reaches the payload. The first lane run
caught the boundary missing (payload clusters refused) and the
opaque value rest (`-eFOO=bar` passed through); both fixed and
pinned by unit test. Prune's local `a`/`f` splitter is gone, replaced
by the shared rule, and check 26 expands the same way from the same
lists, with its own plant case. The work also found one dead arm:
`system info` accepted `-f` with no table row; the row carries both
spellings now. Lane prove [`pb-w30-prove`](../docs/history/lane-proofs-2026-09-23/pb-w30-prove.txt), verdict `fail=0`:
cluster units by exact name, the pre-existing flag tests unchanged,
clippy clean, `ps -aq`, `run -it`, `inspect -f` and `ls -la` through
a payload green, `-aZ` refusing 125 naming `-Z`. The guard is the
single expansion path plus check 26's mirror.

# T-1331: `--filter`, `restart` and `pull -a/-q` answer the docker idiom

**Source:** issue 27, client beta testing 2026-09-22 (three refused
  idioms with no entry);
  `crates/podvm/src/parity.rs` (the `None` rows for
  `--filter`, `restart`, `pull -a`)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

Three docker idioms break with 125 and have no work-order
entry: `--filter` on `images` and `ps` (every `docker ps
--filter name=x` one-liner), `restart` ("`stop` then `start`
is the same thing" is not the same verb), `pull
-a/--all-tags` and `pull -q`. The table is honest; the work
order does not know these are missing.

## Premise

Measured by the reporter on the beta.3 asset, each form
with its refusal text. Bundled flags are the sibling entry;
these are per-verb gaps, not parser gaps.

## Approach

Implement the three behind the table rows that already name
them: `--filter` as a caller-side predicate over listed
records (no new query language), `restart` as stop-then-
start in one verb reporting which half failed, `pull -a` as
all offered tags and `-q` as quiet output. Out of scope:
server-side filter syntax beyond `name=` and label
equality (named if dropped), restart policies (a different
gap).

## Decision

Caller-side filter, composite restart, per-tag pull. Each
reuses a path that already exists rather than forking one. A
tag with no manifest for this platform is named and skipped
(typed `NoPlatform`, not a string match); any other failing
tag stops the run, and an offer with nothing for this
platform at all is an error.

## Prove

`ps --filter name=x` selects, `restart` stops and starts
with both halves named, `pull -a` fetches every offered tag
and `-q` stays quiet; the three parity rows move status with
the behavior. Close issue 27 (idiom thirds) with a comment
showing the runs and the row-status moves as the guard that
stops recurrence.

## Done

**Done, 2026-09-23.** `--filter` is a caller-side predicate over listed
records (`name=` substring, `label=` equality, any other key refused
naming `name=` and `label=`), `restart` is stop-then-start in one verb
naming which half failed, `pull -a` pulls every offered tag of a bare
repository (a tag on it is refused) and `-q` sinks progress output. The
first lane run caught the real case the entry had not named:
`hello-world` offers windows-only `nanoserver` tags, so stop-on-first-
failure made `pull -a` unusable against any multi-OS repository. A tag
with no manifest for this platform is now named and skipped on a typed
`NoPlatform` variant (not a string match), any other failing tag still
stops the run, and an offer with nothing for this platform is an error
rather than an empty success. A single-tag pull of such a tag still
fails with the same text and code. Lane prove [`pb-w31-prove`](../docs/history/lane-proofs-2026-09-23/pb-w31-prove.txt),
verdict `fail=0`: filter units by exact name, suites, clippy clean,
`ps --filter` selects/empty/refused-key, `images --filter` selects,
`restart` composites with the stop half named, `pull -a` rc 0 with four
`nanoserver` skip lines and `hello-world:latest` plus `:linux` in the
store, explicit-tag `-a` refused, `-q` byte-quiet, the three parity rows
moved. The guard is the three moved table rows plus the prove script.

# T-1332: `podvm man` generates the manual from the binary, pager-aware

**Source:** operator order 2026-09-23 (drift-free human/AI manual;
  docs need not carry help text); `crates/podvm/src`
  (per-verb usage strings, the parity table as data)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

Help text lives in docs or not at all, so every new flag is
a chance for drift: the manual says what the binary said on
the day somebody copied it. Nothing generates a readable
manual from the binary for humans (`less`) and agents
(plain text) alike.

## Premise

Audited: no `man` surface exists in `crates/podvm/src`
(only shell completion in `complete.rs`); per-verb usage
strings and the parity table already carry the content a
generator needs.

## Approach

A `man` verb rendering every verb, flag and parity note
from the binary's own data (usage strings plus
`parity::TABLE`), plain text to stdout, shaped like
`wsl-toolkit man --no-pager`: version header, command list
with one-line summaries, a Global section, then a COMMAND
REFERENCE with one section per command carrying its
synopsis and flags. `podvm man [verb]` pages through
`$PAGER` (`less` where present), `--no-pager` (and non-tty
stdout) prints without paging. Progress and diagnostics
stay on stderr; stdout carries the answer alone. New verbs
and flags appear by construction, never by edit. The
generated output carries no emoji and no markers (the
binary-wide scrub is the sibling entry below). Out of
scope: troff/`man(1)` integration (no new dependency, no
roff to drift either), shell completions (unchanged),
`--json` (verbs that have structured answers keep it; the
manual is text).

## Decision

Generated plain text, pager-aware, no troff. The audience
is a tired reader and an agent: both read text, neither
needs typesetting.

## Prove

A script diffs `podvm man` output against every `--help`
output plus the parity rows and fails on any verb or flag
the manual lacks; `podvm man --no-pager` and piped `podvm
man` print identical bytes with no pager spawned. The
completeness script is the drift guard that stops
recurrence.

## Done

**Done, 2026-09-23.** New `man` verb in `crates/podvm/src/man.rs`:
version header, command list with one-line summaries (the first
sentence of each implemented verb row's note), a Global section whose
exit-code numbers render from the exit-code constants, then a command
reference with one section per implemented verb carrying that verb's
`--help` bytes captured by re-executing the binary, so a changed usage
string arrives with no edit here. The verb set is the table's own
implemented rows minus the argv0 aliases; group subverbs (`prune`,
`install-names`, `abi`) render through their group path, pinned
against `rows_of` by unit test. `man [verb]` renders one section;
`--no-pager` and a non-terminal stdout print the same bytes with no
pager spawned, and an unstartable pager falls back to stdout. Lane
prove [`pb-w32-prove`](../docs/history/lane-proofs-2026-09-23/pb-w32-prove.txt), verdict `fail=0`: seven man units by
exact name, suites, clippy, every implemented verb's `--help` exiting
0 with non-empty bytes contained in the manual, one-verb rendering,
byte-identity between `--no-pager` and piped runs, the refusal codes,
the moved `man` rows. One isolation gap is recorded, not hidden: the
bogus-PAGER run asserts identical bytes and rc 0, which the stdout
fallback also produces, so a follow-up prove should capture that run's
stderr separately and assert it empty to isolate non-spawning. The
prove caught two defects on the way: `restart --help` exited 125
because T-1331's verb lacked its `-h, --help` row (added here), and a
first pass of this change forgot the `--no-pager` row the same way
(caught in review before the green run). The manual carries the usage
strings verbatim, glyphs included: the ASCII scrub is the sibling
entry T-1336, whose guard covers this output. The guard is the
completeness script plus the pinned verb set.

**Partial, 2026-09-25.** Issue 50 reopens this entry on the
recorded isolation gap: the bogus-PAGER run asserts identical
bytes and rc 0, which the stdout fallback also produces, so the
test cannot tell non-spawning from a silent fallback. Capture the
bogus-PAGER run's stderr separately and assert it empty, keeping
the byte-identity assertion beside it. Fix area is the T-1332
prove script with `crates/podvm/src/man.rs`. Risk if wrong
is a regression that silently falls back to stdout and still
passes. Prove is the extended bogus-PAGER run with stderr
asserted empty.

**Done 2026-09-26.** No product change: the behavior was already
right, the isolation was what was missing. Lane drive
(`rust:1.98.1-bookworm` job, musl debug binary) with
`PAGER=/bogus-that-does-not-exist`: piped stdout exits 0 with
empty stderr (`PIPED-STDERR-EMPTY`, so no spawn was attempted)
beside byte-identical stdout to `--no-pager` (both 39,238 bytes,
`BYTES-IDENTICAL`); under a pty (`script`) the same bogus pager
is loud on the terminal (`podvm man:
/bogus-that-does-not-exist could not start (No such file or
directory (os error 2)), printing without a pager`,
`TTY-FALLBACK-LOUD`), which is what makes the empty stderr above
meaningful rather than vacuous. A regression that spawns where it
should not now fails the stderr assertion; a regression that
silences the fallback now fails the pty assertion.

# T-1336: CLI output is plain ASCII: no emoji, no markers, on every path

**Source:** operator order 2026-09-23 (no place for either in a CLI
  tool, let alone without a terminal or pty);
  `crates/podvm/src` (usage strings, error messages and
  parity notes carry marker glyphs on dozens of paths:
  `complete.rs:103`, `exec.rs:252`, `main.rs:372`,
  `names.rs:259`, `parity.rs:103`, `run.rs:26`, and the
  per-verb usage blocks behind every `--help`)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

The binary's user-facing text carries ⛔/⚠/⭐ glyphs on
dozens of paths: per-verb usage blocks (printed by every
`--help`), error messages (`complete`, `exec`, `probe`
cache, `install-names`), and parity notes (the
machine-readable contract). On a constrained host with no
terminal or pty these are unrenderable bytes in an
automated caller's stream, and they force every downstream
parser to handle codepoints that carry no meaning.

## Premise

Measured on this tree: non-ASCII bytes outside comments
across `crates/podvm/src` (usage blocks, emitted
errors, table notes); comments and doc-comments that are
never printed are not in scope and stay.

## Approach

Scrub every printed string to ASCII, replacing each glyph
with the word it stands for (`refused:`, `note:`), usage
blocks, errors, banner lines and parity notes alike; keep
source comments as they are. Add the guard that stops
recurrence: a test (or gate check) failing any non-ASCII
byte in `--help` output per verb, the `man` output, and the
error/catalog paths. Out of scope: docs and comments (the
markers check owns those), changing any message's meaning
(glyphs become words, nothing is reworded).

## Decision

Words, not glyphs, everywhere printed. The manual entry
(T-1332) inherits ASCII output from this one.

## Prove

`./experiments/352-ascii-output.sh` exits 0, with the
existing suite green; the guard runs in the gate so a new
glyph fails before it ships.

## Done

**Done 2026-09-23.** Words, not glyphs, as decided: every printed
string across `crates/podvm/src`, `crates/podvm-probe/src`,
`crates/podvm-image/src/tls.rs`, `src/abi.rs` of `pod-enter` (PodUtils/podcrates)
and `crates/podvm-complete/src` carries bytes `0x00`-`0x7F` only.
U+26D4 reads `refused:`, U+26A0 reads `note:`, U+2B50 is deleted;
source comments stay as they are. Twenty-two files, 84 insertions
and 82 deletions, nothing reworded.

`experiments/352-ascii-output.sh` drives the binary rather than
grepping the source: every implemented verb's `--help` (verbs read
out of the binary's own parity table, never listed), `man` under
both pagings with stdout and stderr separated, and a catalog of
five error paths. `experiments/results/ascii-output.txt` carries
the run: 69 `ASCII-OK` lines, zero failures, verdict `fail=0`.

Ten unit tests hold the constants the script drives (usage per
module, every parity note, the whole manual through the stub):
`cargo test -p podvm ascii`, 10 passed. Clippy `-D warnings`
clean in the same lane run. The gate guard is check 28 in
`crates/podvm-gate/src/main.rs` (any non-ASCII byte in a printed string
fails, comments exempt) with its plant case in `podvm-plant`:
a glyph planted in `run.rs` goes red naming `U+26D4` and T-1336.
The guard is the check plus the 352 experiment: a new glyph fails
the gate before it ships and fails the binary drive beside it.

# T-1337: Doctor, disk usage, and log tail for operators

**Source:** issue 38, beta.7 drive 2026-09-25 (no `doctor`, no `df`,
  `logs` without `--tail`); `references/carlbomsdata__winquick`
  `tree/src/main.rs:50` with `tree/src/facts.rs:125-180` (the shape
  to copy, not the content)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

Three operator gaps found while driving beta.7. `podvm
probe` prints the measurement half (legs, verdicts,
refusals) and never the setup half: a QEMU too old for
migration, firmware present or absent, helper tools
present, image installed, disk free against the ceiling.
No `system df` sums stored, extracted and reclaimable
bytes, which operators on small stores need beside the
prune path. `logs -f` replays from the start on every
reconnect, because there is no `--tail N` (and later no
`--since`).

## Premise

Read on the lane-built binary 2026-09-25
(`experiments/results/triage-353.txt` clauses
`38-system-help` and `38-logs-help`): `system --help`
lists no doctor and no df, `logs --help` lists only
`-f|--follow`, and `probe --json` already carries every
leg a doctor would consume. `TODO/supervise.md`
already documents that `logs` has no `--tail`.

## Approach

Three small parity additions, each with its row and its
Prove through the shipped binary, and no spec change
beyond one row each. `podvm doctor` (or `probe --fix`)
reuses the machine legs with the T-1301 checks and prints
one fix line per missing piece, with the third-state
exits (0 ok, 1 problem, 2 could not run). `podvm system
df` (with image du rows) reports stored, extracted and
reclaimable bytes with the container gate applied, and
never exits non-zero for accounting alone. `logs --tail
N` (with `--since` as a second step) reads the same
captured file T-0605 owns, bounded like `-f`, with
byte-identical default output unchanged.

## Decision

One entry, three verbs in order doctor, df, tail, each
shippable alone. The doctor copies the reference's shape
(check lines with fix lines) and none of its content.
Out of scope: a log stream split (T-0605), a network check needing
a registry, any new exit-code meaning.

## Prove

`podvm doctor` on a kvm-less host exits 1 naming the
missing legs with one fix line each; `podvm system df`
sums to the store's own accounting; `podvm logs --tail 5`
prints the last five lines and `podvm logs` prints all
lines byte-identical to before.

## Done

**Done, 2026-09-26.** Three verbs in commit order tail, df,
doctor. Tail landed first: `logs --tail N`
(`--tail=N`, both in any position) prints the last N lines of the
same captured file, byte-identical default untouched, and `-f
--tail N` prints the tail then follows from the chained offset
instead of replaying the file. `podvm-supervise::tail_offset`
(pure, unit-pinned) with `follow_from` chaining the read length
past the printed slice, so no line is lost or repeated. A
non-count is a flag error at 125 naming the value. Prove through
the shipped binary (`experiments/364-qol.sh` tail clauses, exit
0, `experiments/results/qol.txt`): last five of ten exact,
plain logs byte-identical to a wide tail, `--tail 0` empty at
exit 0, non-count refused, `-f --tail 3` following to the end.
Doctor and df stay open below.

**Df, 2026-09-26.** The second verb landed: `podvm system df`
prints one row per image (repository, tag with `<none>` where
none, twelve-digit IMAGE ID, stored blob bytes beside extracted
rootfs bytes), the image and container counts, stored and
extracted totals with each blob billed once, and Reclaimable:
what `image prune` would free. The number is prune's own gate
chain read-only (`prune_candidates(false)`, the `referencing`
record gate, the `in_use` hold check) with
`Store::reclaimable_bytes` standing in for `delete`, so on a
quiet store it is the exact string prune prints. Accounting
never fails the verb: `Store::blob_bytes` counts a missing blob
as 0, `space::dir_bytes` walks without following symlinks,
dedupes hardlinked inodes and counts an unreadable entry as 0.
Only a gate failure (unopenable store, unanswerable lock, failed
listing) exits non-zero. Parity: `df` Native row with its `-h`
row, the `system` note no longer claims df missing (check 27
would refuse the stale claim), `rows_of` maps `system df`, the
`podvm-gate` mirror maps it too, `man` routes its help.
Prove through the shipped binary (`experiments/364-qol.sh`
df clauses, exit 0, `experiments/results/qol.txt`): debian rows
26.9 MiB stored beside 74.6 MiB rootfs, totals present with the
image count identical before and after, a digest-pulled alpine
dangling (`<none>`) reclaimable at 3.7 MiB with prune freeing
the exact same string and df reading 0 B after, `--bogus` at
125, `--help` at 0. Unit-pinned: `reclaimable_bytes` counts an
unshared blob once and only where no survivor needs it,
`dir_bytes` counts once and never follows, df help and bad
flags need no store, short IDs are twelve hex digits.
Two findings ride with it. First, the drive refuted this
entry's own assumption that an unnamed `import` dangles: it
records `imported:latest` through `Reference::parse`'s central
default (`layout.rs:378-384`, T-1320 Done), so the IMPORT_USAGE
sentence claiming "no tag" was corrected in place; the honest
dangling path is a digest pull (`reference.rs`: a digest with
no tag keeps tag None). Second, lane notes: bootstrap's docker
daemon does not come up in job containers (364 needs no
docker, so the job proceeds past it with zig present), one
release build failed after printing Finished with no error
captured, and `build-interpose.sh` failed its musl half once
with no error captured; each went green on the identical tree
on retry, and all three are recorded here rather than
explained away. CI went red on the df commit itself (plant
27b MISS: the shipped note moved out from under its sed) and
green on the plant retarget one commit later; the MISS line is
what T-1202 exists for. Doctor stays open
below.

⚠ Citation correction, read at file and line 2026-09-26: the
doctor shape lives at `tree/src/main.rs` (help text naming
`winquick doctor`) and `tree/src/facts.rs:125-180` (`Status`,
`Check`, `Doctor` with the ordered fix list, builder with
ok/note/fail, health agreeing with the problem list) - there is
no `tree/src/bin/` directory. The tree was read at one pass
([history/2026-09-11-reference-sweep.md](../docs/history/2026-09-11-reference-sweep.md)),
so only the shape travels, none of the content.

**Doctor, 2026-09-26.** The first verb landed last:
`podvm doctor` prints every machine leg as a check line, one
fix line per failed required leg, and the profile. Required
legs fail with fixes (install QEMU where the emulator or its
accelerator list does not answer; the failing call named with
its errno for the fsize and space legs); kvm and tun missing
alone are notes (TCG without hardware acceleration,
user-mode networking); a required leg never measured is
unmeasured and exits 2 rather than guessing a fix. A final
check compares the disk free at the store (or `.` with no
store) against the RLIMIT_FSIZE ceiling read by syscall:
below it fails with room-freeing fixes, infinity notes, an
unreadable limit or room notes where the legs already refuse.
Exits 0, 1, 2 in that precedence. No firmware, helper-tool or
image-presence checks: the tier boots `-kernel` directly and
the runtime consumes no helpers, so each would check what
nothing reads. The leg names and the leg renderer are public
from `podvm-probe::machine` rather than copied, so the
wording cannot drift. Top-level parity row with its `-h` row.
Prove through the shipped binary (`experiments/364-qol.sh`
doctor clauses, exit 0, `experiments/results/qol.txt`): exit
0 with profile tcg on the kvm-less lane, the kvm line and the
ceiling line present with no fix; PATH stripped of the
emulator exits 1 naming the emulator leg with its install-QEMU
fix; `--bogus` at 125, `--help` at 0. Unit-pinned: health
agrees with the problem list (fail carries exactly one fix,
unmeasured outranks failed), help and bad flags need no probe.
With all three verbs driven and 364 fully green, issue 38
closes below.

# T-1414: The no-chroot path returns 127 and 126

**Source:** issue 75; `crates/podvm/src/lifecycle/entry.rs:186`;
  `crates/podvm-probe/src/exit.rs:86`;
  `experiments/330-exit-codes.sh`
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

A missing command returns 125 on the userland rung. The
contract reserves 125 for podvm failing and 127 for not
found. Scripts branching on 127 misread the result.

## Premise

`decide_entry` (`lifecycle.rs:1746`) returns 125
unconditionally, including the unresolvable-payload arms
(`lifecycle.rs:1774`, `lifecycle.rs:1831`). `EXIT_NOT_FOUND`
exists with its case row (`exit.rs:86`) and the chroot path
emits it from the child. Nothing in
`crates/podvm/src` references `EXIT_NOT_FOUND` or
`EXIT_CANNOT_INVOKE` (measured by grep: zero hits), so the
userland path cannot produce 127 or 126 by construction.
`330` covers not-found only through the absolute-path
chroot case.

## Approach

Map `ResolveKind::Absent` to 127 and found-but-not-invocable
to 126 in the `no_chroot` arms. Add the bare-name trio to
`330` per rung.

## Decision

The exit code follows the payload status on every rung. The
rung never changes the code's meaning.

## Prove

`cargo test -p podvm` exits 0.
`sh experiments/330-exit-codes.sh` exits 0 with the userland
missing, not-invocable, and podvm-failure clauses.

## Correction

**Open 2026-09-30 (history).** Filed from issue 75. Extends [T-0802](cli.md),
whose proof ran on the chroot rung only.

## Done

**Done 2026-09-30.** The `no_chroot` arms map `ResolveKind::Absent`
to 127 and found-but-not-invocable to 126 (the T-0802 errno split);
genuine podvm failures stay 125. `cargo test -p podvm` exits 0
with the remapped arms pinned. The trio is driven two ways: the new
330 section 7 runs the bare-name userland missing, not-invocable, and
podvm-failure clauses (shape-probed green in-lane), and 356 clauses
7, 10, and 13 witness 127, 126, and 125 through the same exit-code
table on the fixture ([the drive](../experiments/results/no-chroot-rung.txt)).
Amendment in writing: full `330` exits 2 SKIP in this lane (no engine;
the drive gates on one at its head), so the userland trio is proven
through 356 plus unit until an engine lane re-drives section 7 whole.
The 330 drive itself gained a portability fix beside this entry: its
`set -o pipefail` line killed dash outright, so the option is now
gated on the shell name and `sh 330-exit-codes.sh` reaches the engine
check ([the SKIP run](../experiments/results/exit-codes-2026-09-30.txt)).

# T-1415: Strict classifies degradations on the userland rung

**Source:** issue 76; `crates/podvm/src/complete.rs:122`;
  [T-0804](cli.md)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

`--strict` refuses every run where chroot is denied. The
five unavoidable dev-shim substitutions each count as a
degradation. The callers who want the gate most get
nothing.

## Premise

`strict_refusal` (`complete.rs:122`) pushes every
completion degradation and every step as a refusal reason
and refuses on the count. That is exactly what T-0804
specified before the userland rung existed. Its proof never
exercised `--strict` there.

## Approach

Classify degradations. Safety-relevant ones (no path
virtualization, host and libc mixing, escape) still refuse.
Substitution ones (dev shims) warn. Add `--strict=all`
preserving the current behavior. The refusal line names the
class.

## Decision

Kind decides, count no longer does. The current behavior
stays available under `--strict=all`.

## Prove

`cargo test -p podvm` exits 0. A tracked drive runs
`--strict public.ecr.aws/docker/library/alpine:3.20 echo hi`
clean on a chroot-denying fixture, still refuses a
path-virtualization defect, and `--strict=all` still refuses
the dev-shim run.

## Correction

**Open 2026-09-30 (history).** Filed from issue 76. Extends [T-0804](cli.md).

## Done

**Done 2026-09-30.** Strict refusal classifies by kind, not by count:
safety-relevant degradations (no path virtualization, host and libc
mixing, escape) still refuse, dev-shim substitutions warn, and
`--strict=all` preserves the old count-based behavior with the class
named on the refusal line. `cargo test -p podvm` exits 0 with
the class-aware tests. `sh experiments/356-no-chroot-rung.sh` exits 0
([the drive](../experiments/results/no-chroot-rung.txt)): `--strict`
runs the alpine payload clean on the fixture, refuses the
path-virtualization defect, and `--strict=all` refuses the dev-shim
run. This extends T-0804, whose proof never exercised `--strict` on
the userland rung.

# T-1416: Quiet runs and a verbose banner

**Source:** issues 77 and 79;
  `crates/podvm/src/complete.rs:57`;
  `crates/podvm/src/run/prepare.rs:213`; [T-0804](cli.md)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

Every successful run prints the rung narration to stderr
with no per-run opt-out. Harnesses running hundreds of
commands cannot silence it. Issue 79 asks for the stronger
shape: quiet by default, full narration under `-v`, `-q` as
force-off.

## Premise

`banner_quiet` reads only the store config, never argv or
env (`complete.rs:57`). `run` and `exec` parse no `-q`,
`--quiet`, `-v`, or `--verbose` flag (measured by grep:
zero hits). The never-quiet-by-default test pins the
current shape (`complete.rs:515`). T-0804 decided the banner
cannot be set from a single run's command line.

## Approach

Implement the issue 79 shape: empty stderr on success at
default level, banner under `-v`, `-q` forces off, refusals
and payload stderr untouched. Record the reversed T-0804
decision in writing.

## Decision

Ruled 2026-09-30 by the operator: the issue 79 shape.
Quiet by default, banner under `-v`, `-q` forces off. The
default output change touches every user; refusals and
payload stderr stay untouched, so no safety information is
lost. Issues 77 and 79 both close here. This reverses the
T-0804 command-line rule in writing.

## Prove

`cargo test -p podvm` exits 0. A tracked drive shows
empty stderr on success, the banner under `-v`, silence
under `-q`, and a refused run printing at default level.

## Correction

**Open 2026-09-30 (history).** Filed from issues 77 and 79. Read the current
source before implementation.

## Done

**Done 2026-09-30.** The operator-ruled issue-79 shape: runs are
quiet by default (empty stderr on success), the banner prints under
`-v`/`--verbose`, `-q`/`--quiet` forces it off, and refusals and
payload stderr are untouched. `--verbose` is long-only: `-v` stays
docker's `--volume` refusal, and reclaiming it would turn a volume
refusal into a banner switch, so the parity rows say so in writing.
`cargo test -p podvm` exits 0 with the rewritten banner tests.
`sh experiments/356-no-chroot-rung.sh` exits 0
([the drive](../experiments/results/no-chroot-rung.txt)): clause 14
shows empty payload stdout with no banner line on either stream at
default, the banner under `--verbose`, silence under `-q`, and a
refused run printing at default. Limit stated beside the proof:
extraction progress still rides stderr through the extract side, so
the clause asserts banner-silence per stream, not total silence.
This reverses the T-0804 command-line rule in writing; issues 77 and
79 both close here.

# T-1417: Inspect carries the OCI config

**Source:** issue 82; `crates/podvm/src/images/inspect.rs:145`;
  `src/spec.rs` of `pod-oci` (PodUtils/podcrates);
  `crates/podvm-image/src/store.rs:127`; [T-1319](cli.md)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

`inspect` returns only podvm's record. Image defaults
already applied at runtime, such as `Env`, are unreadable.
The docker `inspect` contract breaks for `Config.Env`,
`Cmd`, `Entrypoint`, `WorkingDir`, and `User`.

## Premise

`INSPECT_FIELDS` is a closed enum with no `Config` branch
(`images.rs:1728`). `inspect_fields` and `inspect_json`
have no `Config` arm (`images.rs:1789`, `images.rs:1824`).
The config is parsed as `RunConfig` (`oci.rs:141`), its
digest is stored on the record (`store.rs:127`), and the
blob is re-readable (`store.rs:479`).

## Approach

Read the config blob through the stored digest. Emit
`Config` with docker key names, null-safe where the image
omits a field. Keep the one-document contract from T-1319.

## Decision

`inspect` reports what the image declares and what podvm
applied, side by side. It never merges the two silently.

## Prove

`cargo test -p podvm` exits 0. A tracked drive matches
`.Config.Env` against the manifest and shows `Cmd`,
`Entrypoint`, `WorkingDir`, and `User` present.

## Correction

**Open 2026-09-30 (history).** Filed from issue 82. Read the current source
before implementation.

## Done

**Done 2026-09-30.** `inspect` reads the config blob through the
stored digest and emits a `Config` object with docker key names,
null-safe where the image omits a field, in both `--format` and the
JSON document, keeping the T-1319 one-document contract: declared and
applied side by side, never merged silently. `cargo test -p podvm`
exits 0 with the field-agreement and template tests.
`sh experiments/373-store-gates.sh` exits 0 with verdict
`STORE GATES HOLD` ([the drive](../experiments/results/store-gates.txt)):
clause 2 matches `.Config.Env` (PATH=) and `.Config.Cmd` (/bin/sh)
against the manifest and shows the `Config` object in the document.

# T-1630: The symlink-escape script is converted, and its deletion has a condition

**Source:** `refactor/recon-c.md:177`, the plan's row, read off disk
  2026-10-02
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

The experiment 85-completion-symlink-escape.sh (retired by T-1735) is converted
and its deletion is blocked on another task, so the tree
reads as an unfinished port with no owner and no
condition.

## Premise

The conversion is the two library tests the plan names in
the completion write module. The deletion condition is the
entry that writes those tests, named in the plan's row and
not yet a tracked entry, so it is recorded here rather than
cited as an id a reader cannot follow.

## Approach

Record the converted state and the deletion condition, and
name what clears it. Do not delete the script here: this
row is a record, and deleting it widens the unit past the
record.

## Decision

Record, not delete. The condition is another task's and a
deletion taken from under it loses its plant.

## Prove

`grep -n "85-completion-symlink-escape" TODO/cli.md` names
the converted tests and the work whose landing clears the
deletion.

## Correction

**Recorded 2026-10-02.** The experiment 85-completion-symlink-escape.sh (retired by T-1735)
stays on disk and is converted in the library. The script plants six
doors, five out of the rootfs and one legitimate internal link
(`85-completion-symlink-escape.sh:23`, `:105`): A `/etc/mtab` (`:25`), B
`/etc/resolv.conf` (`:26`), C `/etc/hosts` (`:27`), E `/etc/passwd`
(`:28`), D `/etc/ssl/certs` (`:29`), F `/etc/apt -> /var/aptreal` (`:30`).

The converted tests are the two in
`src/write.rs` of `pod-rootfs` (PodUtils/podcrates) `mod tests` that name the script's
clauses. `a_write_replaces_an_out_link_with_a_regular_file`
(`write.rs:752`) carries clauses A through E: the out-link is replaced by
a regular file inside the rootfs, so a later reader does not follow it
out, and the outside canary stays byte-identical (`write.rs:748`,
`:754`, `:766`). `a_write_follows_a_directory_link_that_stays_inside`
(`write.rs:779`) carries clause F, openSUSE's own shape: a directory link
that stays inside is followed and the drop-in lands at its target
(`write.rs:775`, `:781`, `:787`).

**The deletion condition, and what clears it.** The script is deletable
when the plan's library-test row lands. That row sits at
`refactor/recon-c.md:110`, and it reads
`Write the two library tests into src/write.rs of pod-rootfs (PodUtils/podcrates) mod
tests (:640)`. Its id is a plan id, not a tracked entry: no `TODO/*.md`
carries a heading for it. The only place that id appears in the tracked
tree is the two doc comments in `write.rs` (`:748`, `:775`) that name the
tests it owns. The plan's own row for this record is
`refactor/recon-c.md:177`, and it makes that row the clearing condition.

⛔ **The condition has not landed, and the tree disagrees with the plan's
wording.** The two tests are present and committed, so the library half of
the plan's row is done. The plan's conversion for this script is wider.
`refactor/01-audit/group-4.md:290` asks for a reachability
integration test in `crates/podvm/tests/`, to drive the four doors
through the shipped binary. The plan names a target file for it at
`group-4.md:294`. That file was never written: the directory git tracks
holds `curated_refusals.rs`, `detached_stdio.rs`, `qol.rs`, and
`store_gates.rs`, and no file of that name exists under `crates/`.
Two further library tests the plan names at `group-4.md:286` and `:288`,
`the_link_is_replaced_and_the_file_outside_is_untouched` and
`a_dangling_directory_link_is_refused_and_reported`, are also absent.

So the script is **partly converted**, and the deletion is blocked by
more than the plan's row states. The library doors are answered at
`write.rs`; the shipped-binary reach that the script's own header at
`:10-15` calls necessary is not, and that header's reason is that a
containment check right in the library and unreached by `podvm run` is
a check nobody has (`group-4.md:272-278`). Deleting the script now drops
the only drive of the reach path. The deletion clears when the plan's
library-test row and its reachability half land as tracked entries. Until
then this row is the record, and no entry deletes the script.

**Retired 2026-11-11.** T-1735 met the deletion condition: the reachability test `crates/podvm/tests/completion_reach.rs` drives the six doors through the shipped binary, and `pod-rootfs` at `v0.22.0` holds the dangling directory link (`a_write_through_a_symlinked_directory_is_refused`, `reach_tells_absent_from_a_link_inside_from_a_link_out`). The script is deleted.

## Done

**Done 2026-10-02.** Recorded the converted state of
the experiment 85-completion-symlink-escape.sh (retired by T-1735) and the condition that
clears its deletion. The six doors the script plants are carried by two
tests in `src/write.rs` of `pod-rootfs` (PodUtils/podcrates):
`a_write_replaces_an_out_link_with_a_regular_file` (`write.rs:752`) for
clauses A to E, and
`a_write_follows_a_directory_link_that_stays_inside` (`write.rs:779`)
for clause F. The premise that the plan's row at
`refactor/recon-c.md:110` is the whole conversion does not hold: the two
library tests named at `refactor/01-audit/group-4.md:286` and line 288 at `c36e0db672d2` are
absent, and the reachability integration test asked for at
`group-4.md:290` was never written. The record stands on that correction.
The script is only partly converted, and the deletion stays blocked
until the plan's library-test row and its reachability half land as
tracked entries. `grep -n "85-completion-symlink-escape" TODO/cli.md`
exits 0 and reports 6 matches.

# T-1649: A `#!` entrypoint needs an undocumented `--entrypoint`, and the refusal must name the way out

**Source:** issue 92; issue 114 section 3.3 and section 6 item 7;
  `crates/podvm/src/lifecycle/script.rs:105`;
  `src/memfd.rs` of `pod-enter` (PodUtils/podcrates);
  `crates/podvm/src/run/prepare.rs:240`;
  A-V-04 of PodUtils/podcrates#39
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

On a chroot-denied host this run exits 126:
`run node:alpine node -e 'console.log(1)'`. The entrypoint
`docker-entrypoint.sh` is a `#!` script, and its interpreter
resolves on the host root. The caller gave a command. The
way out, `--entrypoint node`, exists, and the refusal does
not name it, so a docker user cannot find it.

## Premise

`run.rs:1236` builds argv from the image Entrypoint and Cmd,
the caller's command and `--entrypoint`, by docker's rule.
The image script stays the program and the caller's words
become its arguments. `decide_entry` refuses a script at
`lifecycle.rs:1879`. The message (`memfd.rs:199`, T-1413)
names the path and the `#!` line but no remedy.

## Approach

Where the refused script is the image's own Entrypoint and
the caller gave a command, the refusal adds: retry with
`--entrypoint` followed by the caller's first word. For a
top-level `#!` payload, resolve the interpreter inside the
rootfs and run it on the loader family with the script path
as its argument. T-1646 does the same for a child.

## Decision

docker's argv rule stays: podvm does not drop the image
Entrypoint on its own. The refusal names the remedy. Where
the interpreter resolves inside the rootfs, the script runs
and no refusal prints.

## Prove

`cargo test -p podvm` exits 0 with a test: the script
refusal for an image Entrypoint plus a caller command
contains `--entrypoint` and the caller's first word.
`sh experiments/356-no-chroot-rung.sh` exits 0 with clause 13
changed: a `#!/bin/sh` script whose interpreter is in the
image runs at 0, and the node image run with a caller
command either reaches the payload or refuses naming
`--entrypoint`.

## Correction

**Filed 2026-11-06.** Quoted from issue 92 and its comments, measured on
release 0.1.0-beta.12 on the errand host:

```text
$ podvm run node:alpine node -e 'console.log(1)'
podvm run: ... the payload .../rootfs/usr/local/bin/docker-entrypoint.sh
is a #! script (#!/bin/sh), whose interpreter path resolves on the host
root without a chroot (TODO/enter.md T-1413)                  [exit 126]
$ podvm run --entrypoint node node:alpine -e 'console.log(1)'
... node is 149234536 bytes, over the 134217728-byte ceiling  [exit 125]
on a noexec mount, a #! script run from a memfd fd:
$ /state/home/noexec-test/t.sh
bash: /state/home/noexec-test/t.sh: /bin/sh: bad interpreter: Permission denied
$ /tmp/sandhome-memexec /state/home/noexec-test/t.sh
HI
```

**Amended 2026-11-09.** For T-1649, the ask A-V-04 of PodUtils/podcrates#39 (COORD-015,
a message from a peer session) wants a Node CI job to run from an OCI image
with no root. This entry is one step of that chain: the entrypoint of the
node image is a `#!` script (T-1661). Its clauses do not change. T-1646
holds the end-to-end clause.

## Done

**Done 2026-11-10.** For T-1649, the last clause of the partial record below is run. Experiment 356 exits 0 in full on 2026-11-10 ([the run](../experiments/results/no-chroot-rung.txt); T-1648 in `TODO/enter.md` records its inputs).

**Partial, 2026-11-10.** On the no-chroot path `decide_entry` follows
a `#!` payload through `script::follow` (`lifecycle/script.rs`): it reads
the `#!` line as the kernel does, resolves the interpreter inside the
rootfs (four levels), and runs the kernel's argv `interp [arg] script
args` on the family of the interpreter. An interpreter outside the image
refuses at 126, never on the host. Where the image Entrypoint is the
refused script and the caller gave a command, the refusal adds "retry
with `--entrypoint <first word>`". Both `exec` paths apply the
interposer after the decision and run the same argv. `cargo test
--workspace` exits 0 (989 passed) with
`an_entrypoint_script_refusal_names_the_entrypoint_flag`. [The drive](../experiments/results/script-entry-1649.txt):
`run node:22-alpine node -e 'console.log(1)'` prints 1 at 0 on the
chroot-denying fixture; an image without `/bin/sh` refuses at 126 with
the retry line. Clause 13 of experiment 356 holds it. Remaining: one full run of the committed `sh experiments/356-no-chroot-rung.sh` (its pulls from `public.ecr.aws` answered HTTP 429 on 2026-11-09 and 2026-11-10; a copy with `mirror.gcr.io` images passed each step of the clause).

# T-1650: `--strict` does not refuse the userland grandchild limit, while `--strict=all` refuses every run

**Source:** issue 93; issue 114 section 3.7 and section 6 item 8;
  `crates/podvm/src/complete/strict.rs:129`;
  `crates/podvm/src/complete/strict.rs:174`;
  `crates/podvm/src/lifecycle/entry.rs:451`; [T-1415](cli.md)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

No strictness level means "refuse a rung that cannot run
this command". `run --strict` of `sh -c '/bin/ls /'` warns,
then the shell fails `not found` at 127. `run --strict=all`
of `/bin/echo hi` refuses at 125 over five substitutions: the
namespace share and `dev/null`, `zero`, `random`, `urandom`.

## Premise

`classify_strict` (`complete.rs:251`) has two kinds, safety
and substitution. Where chroot is denied the userland rung
is a substitution (`complete.rs:296`), by T-1415's decision.
No reason names a capability gap of the selected family. The
exec gap appears only in the banner (`lifecycle.rs:1984`).

## Approach

Add a third kind, capability, for the gaps of the selected
family: a dynamic grandchild cannot exec (T-1646), a guest
object cannot load (T-1645), a relative path cannot resolve
(T-1643), a static payload cannot be reached (T-1648). A gap
leaves the list in the change that fixes it. `--strict`
refuses on a capability gap the payload will hit: a shell
given `-c` or a script runs children. Where podvm cannot
tell, it warns. The refusal names the gap and a rung that
runs the command, once T-1651 or T-1652 provides one.
`--strict=all` keeps its rule for substitutions.

## Decision

T-1415 stays: the userland rung alone is a warning where no
better rung exists. A gap that decides whether the command
can run at all is safety-relevant, and `--strict` refuses
it.

## Prove

`cargo test -p podvm` exits 0 with `classify_strict`
tests for the capability kind. `sh experiments/356-no-chroot-rung.sh`
exits 0 with a strict clause on the fixture: `run --strict`
of `sh -c '/bin/ls /'` exits 125 naming the exec gap, or 0
with the listing once T-1646 is done, and `run --strict` of
`/bin/echo hi` exits 0.

## Correction

**Filed 2026-11-06.** Quoted from issue 93, measured on release
0.1.0-beta.12 on the errand host with the exit codes read unpiped:

```text
$ podvm run --strict alpine:3.20 /bin/sh -c '/bin/ls /'
podvm run: --strict warning [substitution]: unchanged dev/random ...
sh: /bin/ls: not found
[exit 127]                       # strict did not refuse the run
$ podvm run --strict=all alpine:3.20 /bin/echo hi
... --strict=all refused ... [substitution] unchanged dev/zero ...
[exit 125]                       # refuses a run that would have worked
```

## Done

**Done 2026-11-10.** For T-1650, the last clause of the partial record below is run. Experiment 356 exits 0 in full on 2026-11-10 ([the run](../experiments/results/no-chroot-rung.txt); T-1648 in `TODO/enter.md` records its inputs).

**Partial, 2026-11-10.** `--strict` knows a third kind, a capability gap
of the loader family (`complete/capability.rs`). The list holds what is
true now, each gap with its entry: the routes T-1646 does not take
(`fexecve`, `execveat` with flags, `system()` and `popen()`, a foreign
child ELF; podvm cannot tell, so a warning), a static child on the host
root (T-1652; refused where a shell `-c` string or an entrypoint
argument names a static ELF of the image), and the raw `statx` of
libuv (T-1697; refused for npm, npx, yarn, pnpm, corepack and node given
a script). A test fails while a listed entry is `done`. `--strict=all`
keeps its rule. `cargo test --workspace` exits 0 with
`strict_refuses_a_capability_gap_that_the_payload_hits`. [The drive](../experiments/results/strict-capability-1650.txt):
issue 93's `--strict sh -c '/bin/ls /'` now runs at 0 (T-1646 routes the
grandchild), `--strict npm --version` refuses at 125 naming T-1697, and
`--strict=all /bin/echo hi` refuses at 125. Clause 24 of experiment 356
holds it. Remaining: one full run of the committed `sh experiments/356-no-chroot-rung.sh` (its pulls from `public.ecr.aws` answered HTTP 429 on 2026-11-09 and 2026-11-10; a copy with `mirror.gcr.io` images passed each step of the clause).

# T-1654: Profiles: strict TOML recipes with `extends` and `-s` overrides

**Source:** issue 166; #193; the owner's ruling of 2026-11-06
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

podvm has the docker and podman command line and a tier flag,
but no recipe layer. A caller cannot name a sandbox shape once
and run it again. sand-net ran 43 shapes from TOML recipes, and
each command line ran unchanged in every shape.

## Premise

The parity table is the only description of what `run`
accepts (`crates/podvm/src/parity.rs:209-277`). `--podvm-tier`
takes `machine` or `chroot` (`crates/podvm/src/tier.rs:22`).
Resource flags refuse by name (`parity.rs:341`, T-0804). The
identity calls are virtualised (T-0711, `parity.rs:338`). No
source reads a profile file, and no verb lists one.

## Approach

Add `podvm profile list` and `podvm profile show NAME`, and
`--profile NAME` on `run`. A profile is TOML with the tables
`fs`, `limits`, `net` and `env.set`. `extends` merges the
child's tables over the parent's. `-s key=value` sets one leaf
and repeats; the value parses as TOML and falls back to a
string. Built-in profiles live in the binary; user profiles
live under the XDG config directory. Map `fs.backend` to the
tier, `fs.user` to the identity of T-0711, `fs.wrap` to an
entrypoint prefix, and `limits.file_max` to `RLIMIT_FSIZE`.
`limits.memory` and `limits.cpus` keep the T-0804 refusal on a
rootfs tier and become guest RAM and vCPUs on the machine tier.

## Decision

An unknown key and a wrong type are errors that name the key:
"a misspelt key that is silently ignored leaves a cage looser
than the recipe says" (issue 166). A profile that names a
mechanism the probe reports absent refuses with the probe's
reason, at load time where the host is known, else at run
time. `--strict` keeps its meaning. On a host with no mount,
`/workspace` and `/state` are path-map entries (T-1653,
T-1652), not bind mounts.

## Prove

`cargo test -p podvm` exits 0 with tests: an unknown key and a
wrong type each refuse and name the key, a planted misspelt
key is refused, and `extends` plus `-s` give the expected
effective profile. `sh experiments/410-profiles.sh` exits 0:
`podvm profile show NAME -s k=v` prints the effective profile,
a profile that asks for an absent tier refuses with the
probe's reason, `sh -c 'pwd; ls /workspace'` answers the same
in every built-in profile that places `/workspace`, and
`limits.memory` refuses by name on a rootfs tier.

## Correction

**Filed 2026-11-06.** The facts below are quoted from issue 166, so the
entry stands if GitHub hides the issue. sand-net is the 0BSD tool that
issue 193 describes; its source is deleted.

```text
tables:   fs (backend, profile, read, write, execute, path_extra, disk_tmp,
          as_root, image, runtime, user, wrap, persist, rw_root),
          limits (file_max, tmp_size, shm_size, memory, cpus, pids),
          net (mode, ports, allow, auth, resolv, netns), env.set
defaults: backend bailey, net.mode broker, ports [443], tmp 256m, shm 250m,
          file_max 1g
-s 'net.ports=[80,443]'   -> a list (parsed as TOML)
-s net.mode=open          -> a string (TOML parse failed)
validation: broker and proxy-net need bailey; gvisor, bwrap, landrun take
          none, open, netns; microvm none and netns; ports 1 to 65535
F30: landrun has no mount namespace, so /workspace did not exist and
          `sand run landrun-none -- sh /workspace/probe.sh` failed
F24: a 0700 project made a rootless keep-id container fail to stat it
```

## Done

**Done 2026-11-11.** `cargo test -p podvm` exits 0 with the tests of
`crates/podvm/src/profile/tests.rs`: `an_unknown_key_refuses_and_names_the_key`,
`a_wrong_type_refuses_and_names_the_key`, `a_planted_misspelt_key_is_refused`
and `extends_and_s_give_the_effective_profile`, beside the cycle of
`extends`, each built-in profile, and `limits.memory` and `limits.cpus` on a
rootfs tier. `PODVM_410_MIRROR=1 sh experiments/410-profiles.sh` exits 0
(`experiments/results/profiles.txt`), each of its eight clauses:

| clause | reading |
| --- | --- |
| `profile list` | exit 0, the five built-in profiles |
| `profile show workspace -s fs.user=1000 -s limits.file_max="1g"` | `user = 1000`, `file_max = "1g"`, and the flags `--user=1000` and `--ulimit=fsize=1073741824` |
| an absent tier | `--profile incus`: exit 125, "no incusd answers"; `--profile machine` with `PODVM_MACHINE_ACCEL=kvm`: exit 125, "/dev/kvm does not open" |
| `sh -c 'pwd; ls /workspace'` | `/workspace` and `probe.txt` in workspace, readonly (on the chroot-denying fixture) and machine (a TCG guest) |
| `limits.memory` on a rootfs tier | exit 125, "`limits.memory` needs fs.backend = \"machine\"" |
| a misspelt key of a user profile | exit 125, "unknown key `fs.read_onyl`", before the pull |
| a user profile that extends `workspace` | its `env.set` reaches the command; `-s env.set.GREETING=...` overrides it |
| `-s` without `--profile` | exit 125 |

What podvm takes of issue 166, and what it leaves:

* The tables are podvm's own: `fs` (`backend`, `user`, `wrap`,
  `read_only`, `workspace`, `workdir`, `volumes`, `tmpfs`), `limits`
  (`file_max`, `memory`, `cpus`, `timeout`), `net` (`mode`) and
  `env.set`. A key of sand-net that has no mechanism in podvm is not
  accepted: it is an unknown key that names the keys of its table, which
  is the rule of the Decision. `net.mode` takes `host` only until the
  network entries T-1662 to T-1665 add the other modes.
* A profile becomes `run` flags that the one parser of `run` reads
  (`podvm profile show` prints them), so a profile can do nothing that a
  flag cannot, and a flag of the command line wins over the profile.
  `fs.backend` is `--podvm-tier`; `fs.workspace` is `-v HOST:/workspace`,
  a path-map entry of the loader family (T-1653) and a copy on the machine
  tier; `limits.file_max` is `--ulimit=fsize`; `limits.memory` and
  `limits.cpus` are `--podvm-mem` and the new `--podvm-cpus` on the
  machine tier and refuse on a rootfs tier.
* `--podvm-cpus` gives `-smp`; a guest of more than one CPU keeps ACPI,
  because with `acpi=off` the kernel saw one CPU (measured: `nproc` 1,
  then 2).
* User profiles are `$XDG_CONFIG_HOME/podvm/profiles/NAME.toml` (else
  `~/.config/podvm/profiles`); a user file takes the place of a built-in
  profile of the same name. A name takes letters, digits, `-`, `_` and
  `.` only.
* T-1728 came from clause 4: the machine tier gave the command a terminal,
  and `ls` coloured its output.

# T-1655: The sandbox hardening line: each `run` flag works on the tier or refuses by name

**Source:** issue 167; #193; the owner's ruling of 2026-11-06;
  A-V-07, G-12, G-06 of PodUtils/podcrates#39
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

sand-net ran podman and docker with one fixed hardening line.
podvm refuses most of that line with status None, so the same
command cannot run under podvm. Some of the refused flags have
an unprivileged equivalent that podvm does not use.

## Premise

The parity table refuses `--read-only`
(`crates/podvm/src/parity.rs:257`), `--tmpfs`
(`crates/podvm/src/parity.rs:259`), `--ulimit`
(`crates/podvm/src/parity.rs:274`), `--cap-add, --cap-drop`
(`crates/podvm/src/parity.rs:227`), `--security-opt`
(`crates/podvm/src/parity.rs:278`), `--runtime`
(`crates/podvm/src/parity.rs:277`) and `--userns`
(`crates/podvm/src/parity.rs:275`). `-u, --user` is Degraded
through the interposer identity
(`crates/podvm/src/parity.rs:225`). `setrlimit`,
`PR_SET_NO_NEW_PRIVS` and a seccomp filter need no privilege;
the probe already sets `PR_SET_NO_NEW_PRIVS` in a child
(`pod_hostprobe::probes`).

## Approach

Write an experiment that runs the sand-net line through
`podvm run` and records each flag as passed or refused by
name. Then implement the unprivileged equivalents. `--ulimit
fsize` calls `setrlimit` before the exec. `--read-only` makes
a write outside the named writable paths fail with `EROFS` in
the interposer and in the T-1652 path layer. `--cap-drop ALL`
and `--security-opt no-new-privileges` set
`PR_SET_NO_NEW_PRIVS`. `--tmpfs` with a size becomes a store
directory under a size check. `--memory`, `--cpus` and
`--pids-limit` keep the T-0804 refusal on a rootfs tier and
set guest resources on the machine tier. Each parity row moves
with its change.

## Decision

A flag works on the selected tier or refuses with a reason;
nothing in between. A flag that a path layer enforces is
Degraded, and its note says which layer, because the
interposer is not a boundary. `--userns keep-id` stays None:
there is no user namespace, and identity is virtualised, not
mapped. An image record carries its input digest, not a
timestamp (F33).

## Prove

`cargo test -p podvm` exits 0 with parity tests for each
changed row. `sh experiments/411-hardening-line.sh` exits 0 on
a chroot-denied fixture: every flag of the line passes or
refuses by name, `--read-only` gives `EROFS` for a write to `/`
while `/workspace` and `/tmp` stay writable, `--user 993:990`
reports 993 from `id -u` and writes the project, a write past
`--ulimit fsize` fails with `EFBIG`, and `--memory 128m`
refuses by name on a rootfs tier, `--ulimit nofile=64` shows in
`ulimit -n` in the payload, and `--timeout 2` on `sh -c 'sleep 100 &
wait'` ends the whole tree, with each child gone, within 60 s.

## Correction

**Filed 2026-11-06.** Quoted from issue 167. The host was Debian 13 in an
unprivileged Incus container, podman 5.4.2, docker 26.1.5, with egress
only through a loopback HTTP proxy:

```text
run --rm --cap-drop ALL --security-opt no-new-privileges --read-only
  --tmpfs /tmp:rw,nosuid,nodev,size=N --tmpfs /dev/shm:rw,nosuid,nodev,size=N
  --ulimit fsize=N -v PROJECT:/workspace -v STATE:/state
  -v RESOLV:/etc/resolv.conf:ro -w /workspace --network none|host|pasta|slirp4netns
  --http-proxy=false --userns keep-id[:uid=U,gid=G] --entrypoint CMD IMAGE ARGS
F13: rootless podman refused every cgroup limit ("rootless needs no limits +
     no cgrouppath when no permission is granted for cgroups")
F28: --user U:G without a mapping made the project read-only for 993, 1000, 0
F26: pasta printed --version and then failed "Couldn't set IPv4 route(s) in
     guest: Invalid argument" beside a linkdown route
F33: a cache-hit rebuild keeps Created, so a stale image looked current
probe matrix vs the bailey baseline: podman-none 307 same / 92 differ,
     docker-none 305 / 86, most in fs (54)
```

**Amended 2026-11-09.** The asks A-V-07, G-12 and G-06 of
PodUtils/podcrates#39 (COORD-015, a message from a peer session) add four
clauses. First, `--ulimit` takes `nofile`, `nproc`, `cpu`, `as`, `core` and
`fsize`, each set with `setrlimit` before the exec. A name that the host
denies refuses by name. Second, `--timeout SECONDS` (podman's spelling) ends
the whole process tree of the payload, with SIGTERM and then SIGKILL, and it
reaches a child that left the process group. Third, the run measures the
peak resident set and the CPU time of the tree from `wait4` and gives them
to the report of T-1695. Fourth, a run that ended by timeout or by `stop` is
gone within 60 s: its record and its store directory are removed (G-12). The
limits come from one source, the profile of T-1654, and the flags amend it.
The row of `--ulimit` in the parity table moves with the change.

**Amended 2026-11-10, for podrunner.** The podrunner design review (the
owner's design review of PodUtils/podcrates#47; the ruling "podrunner",
2026-11-10; SPEC section 10.4, its measurements M1 and M2) adds four clauses.
First, the mechanism of the `--timeout` clause and of the 60 s clause is
T-1708 (the launcher as a subreaper); this entry takes the flags. Second,
`--ulimit fsize` below 2 TiB kills a .NET payload that runs with the
default `DOTNET_EnableWriteXorExecute`: the runtime calls `ftruncate` on a
memfd with 2 TiB, then gets SIGXFSZ and exits 153 (M1: under 20 GiB, 1
GiB, 128 MiB and 1 MiB). The parity note of `--ulimit` and the profiles of
T-1654 say so, and the experiment adds a .NET case (T-1706). Third, the
read-only layer serves a `:ro` volume beside `:rw` volumes and `:rw`
volumes nested in a `:rw` volume, as the runner's job-container table
needs (`/__e` read-only beside `/__w` and `/__w/_temp`, T-1711); the Prove
adds that table. Fourth, the Premise cited parity lines that moved; they
are corrected in place.

**Amended 2026-11-11.** `Plan` of `pod-enter` has no field for a resource
limit or for `no_new_privs` at `v0.15.0`, and podvm cannot set either in
its own process before the fork (an `fsize` limit would bind podvm's own
writes, and `PR_SET_NO_NEW_PRIVS` cannot be undone). The child-side part
(`Plan::limits`, `Plan::no_new_privs`, set after the `chdir` and before
the exec in each entry, a refused call named by its step) is asked of
podcrates by COORD-027 (issue 245). The `--timeout` clause waits for
T-1708 (podcrates podcrates@T-162). The parts in podvm alone: experiment 411, the
parse and the parity rows of each flag, the `--read-only` layer of the
interposer with the `:ro` volumes, and `--tmpfs` as a store directory
under a size check.

## Done

**Done 2026-11-11.** Each flag of the line works on the rung that podvm
enters or refuses by name; the partial records below give each part:
`--timeout` (the wall bound), `--ulimit` and `--security-opt
no-new-privileges` (podcrates `v0.17.0`), `--cap-drop ALL`, the read-only
layer of `--read-only` and `:ro`, and `--tmpfs` (a directory of a
foreground run; a detached run refuses it by name). `--network`,
`--userns`, `--memory`, `--cpus` and `--pids-limit` refuse by name. The
Prove, on the cloud host, 2026-11-11: `cargo test --workspace` exits 0
with the parse and parity tests of each changed row, and the interposer
tests (75) with the read-only layer and the job-container table of the
runner (`/__e` read-only beside `/__w` and `/__w/_temp`).
`PODVM_411_MIRROR=1 sh experiments/411-hardening-line.sh` exits 0 on the
chroot-denied fixture ([the run](../experiments/results/hardening-line.txt)):
each flag alone works or refuses by name; the line gives `EROFS` for a
write to `/` while `/workspace` and `/tmp` take writes, `EFBIG` past
`--ulimit fsize`, and `NoNewPrivs: 1` in the payload; `--user 993:990`
prints 993 and writes the project; `--memory 128m` refuses by name;
`--ulimit nofile=64` shows in `ulimit -n`; `--timeout 2` ends `sh -c
'sleep N & wait'` in 2 s with no child left. Two clauses of the
amendments ride with their own entries: the .NET case with SIGXFSZ under
a small `fsize` is part of T-1706 (the help and the row of `--ulimit`
name it), and the container-hook table of the runner is T-1711. The
measures of the end (peak resident set, CPU) are in `.End` (T-1708) for
the report of T-1695.

**Partial, 2026-11-11.** `--timeout SECONDS` works (podman's spelling,
whole seconds over 0): a detached run sets the wall bound of its launcher
(`set_wall` of `pod-supervise` v0.16.0), and a foreground run arms a timer
thread that kills each descendant of podvm with SIGKILL at the bound
(`crates/podvm/src/run/wall.rs`); the end reads `wall`, and the parity row
is Native. Drive on the cloud host, 2026-11-11, the pinned alpine digest:
a foreground `run --rm --timeout 2` of `sh -c 'sleep N & sleep M & wait'`
ends at 2 s with 137 and leaves no process; the same under `run -d` reads
`exited wall 2001 0` in `inspect --format '{{.State}} {{.End.Reason}}
{{.End.RunMs}} {{.End.Residual}}'`; `--timeout 0` exits 125.
`cargo test -p podvm` exits 0 with `the_timeout_takes_whole_seconds_over_zero`.
`--ulimit` and `--security-opt no-new-privileges` work since podvm takes
podcrates `v0.17.0` (`4da8fb3`, its podcrates@T-176, PodUtils/podcrates#58):
`podvm_enter::ulimit` reads docker's `NAME=SOFT[:HARD]` (`-1` for no
limit), the run puts the limits and the bit in `Plan::limits` and
`Plan::no_new_privs`, and a detached run keeps them in its record
(`Detail::hardening`) for the launcher. Each other `--security-opt` is
refused by name. Drive on the cloud host, 2026-11-11, the pinned alpine
digest: `--ulimit nofile=64` reads 64 from `ulimit -n` in a foreground run
on the namespace rung and on the chroot-denied fixture (userland), and 77
in a detached run; with `--ulimit fsize=1024` and SIGXFSZ ignored, `dd` of
4096 bytes fails with "File too large" on both rungs; the payload of a
detached run with `--security-opt no-new-privileges` has `NoNewPrivs: 1`
and 0 without it (namespace rung; the fixture sets the bit for its own
filter); `--ulimit nofile=99999999:99999999` refuses with
`setrlimit(RLIMIT_NOFILE): EPERM`. `cargo test -p podvm -p podvm-enter`
exits 0 with `a_ulimit_reads_as_docker_writes_it` and
`the_limits_of_the_hardening_line_parse`; experiment 355 and
`curated_refusals` refuse 38 flags now. `--cap-drop ALL` sets the same
bit (row Degraded: podvm does not change the capability sets), a named
capability and `--cap-add` are refused by name (`cap_drop` in
`crates/podvm/src/run/parse/values.rs`). `--memory`, `--cpus` and
`--pids-limit` stay refused by name on a rootfs tier. The read-only layer
works: `PODVM_RO` (`ro:PREFIX`/`rw:PREFIX`, the longest prefix decides)
makes the interposer answer `EROFS` for a write-class call, an `open` that
writes and an `fopen` mode that writes under a read-only guest path
(`crates/podvm-interpose/src/map/readonly.rs`); `--read-only` gives
`ro:/` with `rw:/dev` and an `rw:` entry for each writable volume, and a
`-v HOST:GUEST:ro` volume gives `ro:GUEST` (`read_only_env` in
`crates/podvm/src/lifecycle/volume.rs`). Each entry with no interposer
table refuses both by name; the rows are Degraded, because a raw system
call passes. Interposer tests: 75 passed, with
`a_write_under_a_read_only_prefix_answers_erofs`; experiment 356 clauses
26 and 30 drive both. Remaining:
`--tmpfs`, the guest resources of the machine tier (T-1656), the report of
T-1695, and experiment 411.

# T-1659: Provider shapes and bare userlands as built-in profiles

**Source:** issue 172; #193; the owner's ruling of 2026-11-06
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

A caller cannot run a tool in the shape of its production
host: the provider's user, limits and variables. sand-net had
seven such shapes. podvm pulls each image, and has no profile
to put the shape around it.

## Premise

Profiles arrive with T-1654. The identity calls are virtualised
(T-0711), so `id -u` can answer 993 while the real owner of the
project stays the caller. Resource flags refuse by name on a
rootfs tier (`crates/podvm/src/parity.rs:228`, T-0804). The
Lambda and Vercel images are glibc, which the rootfs tier
declines today (T-1644).

## Approach

Ship built-in profiles: `lambda-al2023`, `cloudflare-sandbox`,
`vercel-sandbox`, `busybox-bare`, `distroless-static` and
`node-shell`, each with the user, environment and limits of
issue 172. Memory and pids limits run on the machine tier or
refuse by name. A read-only root and a sized `/tmp` use the
T-1655 path layer. Each profile prints what it does not
model. A profile that needs a missing tool, curl for example,
detects it and says so.

## Decision

A shape is a profile, not a tier. podvm never infers success
from a zero exit of a pipeline: `curl ... | sh` with no curl
exits 0 and installs nothing. `RLIMIT_NPROC` and
`RLIMIT_FSIZE` are partial stand-ins and the profile labels
them so. The Alpine and busybox shapes come first; the glibc
shapes follow T-1644.

## Prove

`cargo test -p podvm` exits 0 with a test that every built-in
profile loads and lists what it leaves out.
`sh experiments/415-provider-shapes.sh` exits 0: the
`lambda-al2023` profile prints 993 for `id -u`, prints
`AWS_LAMBDA_FUNCTION_NAME`, gives a read-only error for
`touch /x` and leaves the project writable; `busybox-bare`
runs `wget`; `distroless-static` runs a static binary and
refuses `sh -c true` by name; the memory limit refuses by name
on a rootfs tier.

## Correction

**Filed 2026-11-06.** Quoted from issue 172:

```text
lambda-al2023  public.ecr.aws/lambda/provided:al2023  user 993:990, memory 128m,
               /tmp 512m, pids 1024, read-only root, AWS_LAMBDA_FUNCTION_NAME,
               AWS_REGION=us-east-1, LAMBDA_TASK_ROOT=/var/task, TZ=:UTC
vercel-sandbox amazonlinux:2023, user 1000:1000, USER=LOGNAME=vercel-sandbox
busybox-bare   busybox:musl; wget prints "TLS certificate validation not
               implemented", and expired, self-signed and wrong-host
               certificates all exit 0
distroless     gcr.io/distroless/static-debian12; `sh -c true` fails to start,
               a static binary placed in the project runs
install by `curl -fsSL URL | sh`: node-shell, cloudflare-sandbox and
               busybox-bare had no curl, and the pipeline exited 0 with
               nothing installed
```

## Done

**Done 2026-11-11.** The last clause, `lambda-al2023`, runs. podcrates
`v0.24.0` applies a layer whose root entry has an empty name (`pod-extract`,
`safety::is_root_entry`, the answer to COORD-042, issue 261), and podvm
takes the tag (T-1679). The run then met the image's entrypoint,
`/lambda-entrypoint.sh`, which wants a handler name: that script is
Lambda's invocation protocol, which the profile leaves out. A new key
`fs.entrypoint` gives `run --entrypoint=VALUE`, and `lambda-al2023` sets it
to `""`, so the command runs as the program
(`crates/podvm/src/profile/model.rs`). `PODVM_415_MIRROR=1 sh
experiments/415-provider-shapes.sh` exits 0 on the cloud host: clause 1
prints 993 for `id -u` and `fn=podvm`, `touch /x` fails with "Read-only
file system", and the project is writable; clauses 2 to 7 hold.
`cargo test -p podvm` exits 0 with
`a_provider_shape_names_its_image_and_its_identity`, which now asserts the
cleared entrypoint.

**Partial 2026-11-11.** `cargo test -p podvm` exits 0 with
`each_built_in_profile_checks_and_gives_its_flags` (each built-in profile
loads and lists what it leaves out), `a_provider_shape_names_its_image_and_its_identity`
and `a_profile_with_an_image_takes_the_words_as_the_command`.
`PODVM_415_MIRROR=1 sh experiments/415-provider-shapes.sh` exits 0 on the
chroot-denying fixture (`experiments/results/provider-shapes.txt`):
`busybox-bare` has `wget`; `distroless-static` refuses `sh -c true` by
name (exit 127) and runs a static program of the project; `vercel-sandbox`
(the glibc amazonlinux:2023) prints 1000 and `USER=vercel-sandbox`;
`cloudflare-sandbox` answers `touch /x` with "Read-only file system" and
leaves the project writable; `-s limits.memory=128m` and `-s
'needs=["curl"]'` refuse by name (exit 125); each of the eleven built-in
profiles lists what it leaves out.

The parts of the build: a profile names its image (`fs.image`), and each
positional word of the command line is then the command; `--` ends
podvm's flags; `leaves_out` and `needs` are top-level keys; `needs`
refuses a run whose image lacks a program on its PATH, so `curl ... | sh`
is never trusted to an exit code. A static program in a volume now takes
the native-static rung (its guest path maps back through the volume).

Remaining clause: `lambda-al2023`. `pod-extract` refuses a layer of
`public.ecr.aws/lambda/provided:al2023` whose root directory entry has an
empty name; COORD-042 (issue 261) asks podcrates for the fix of that
frozen code. The clause runs again with the tag that has it.

# T-1660: `podvm notebook`: Jupyter and marimo cycles that keep one directory

**Source:** issue 174; #193; the owner's ruling of 2026-11-06
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

The owner names notebooks in the scope. podvm has no verb that
runs a notebook, and no model of a hosted session that wipes
its root on restart and keeps one directory.

## Premise

`run --rm` starts from the stored rootfs, so a fresh root for
each cycle costs nothing. A kept directory needs a volume,
which a host with no mount cannot give (T-1653); `cp` copies
in and out (T-1323, `crates/podvm/src/parity.rs:155`). The
supervisor holds one pidfd for the direct child
(`pod_supervise::launcher` of PodUtils/podcrates, T-0601) and
starts a session (`setsid` in the same launcher). A Jupyter kernel is a
grandchild, which T-1646 must reach. python:3.13-slim is
glibc (T-1644).

## Approach

Add `podvm notebook NB --persist DIR [--cycles N] [--kill-after
S] [--restart-kernel-after CELL]`. Each cycle is a new
container with a read-only root, a `/workspace` that holds
only the notebook, a new kernel and `DIR` at `/persist`. Print
each output line as `[N] text` and exit 1 when a cell raised.
`--kill-after` ends the cycle with 143 and kills the whole
process tree through the session. A `.py` notebook runs
through `marimo export html`. Use an Alpine Python image
until T-1644 admits glibc.

## Decision

The kept directory is the only state that crosses a cycle.
Two concurrent notebooks share the host network on a rootfs
tier, so each picks its own loopback ports; the banner says
the ports are not private. A failed `!` line does not raise,
so the verb does not report it as a failure.

## Prove

`cargo test -p podvm` exits 0 with tests for the argument
parse and the `[N]` line format.
`sh experiments/416-notebook-cycles.sh` exits 0: three cycles
keep only the `--persist` content, `--restart-kernel-after 1`
loses variables and keeps `/persist`, `--kill-after 5` on a
sleeping cell exits 143 and leaves no process, and a marimo
file exports to HTML.

## Correction

**Filed 2026-11-06.** Quoted from issue 174:

```text
jupyter cage:  jupyter nbconvert --to notebook --execute --stdout --log-level=ERROR NB
marimo cage:   marimo export html --no-include-code NB.py
sand notebook NB --persist DIR [--cycles N] [--lab LAB] [--kill-after S]
               [--restart-kernel-after CELL]... [--cage C]
NotebookCycles: only /persist survives a cycle and a kernel restart;
               variables do not survive the restart; /tmp does not survive
               the cycle; /workspace holds only the notebook
killing the client left the container running; leftover containers were
               found by their project volume and killed; exit 143
`curl -fsSL URL | sh` without curl: "curl: not found", exit 0, nothing installed
```

## Done

**Done 2026-11-11.** `cargo test -p podvm` exits 0 with
`the_arguments_parse_and_a_wrong_one_is_named`,
`each_output_is_a_line_of_its_cycle_and_an_error_raises` (the `[N]` line
format, and a failed `!` line that does not raise),
`a_restart_cuts_the_notebook_after_its_cell` and
`each_kernel_runs_with_a_read_only_root_and_only_persist_kept`. `sh
experiments/416-notebook-cycles.sh` exits 0 on the chroot-denying fixture
(`experiments/results/notebook-cycles.txt`): `--cycles 3
--restart-kernel-after 1` prints `[N] persist N tmp 1` and `[N] workspace
['c.ipynb']` for each cycle, `x kept False` after the restart with
`/persist` kept, and exits 1 for the raise; the control with no restart
keeps `x`; `--kill-after 5` on a sleeping cell exits 143 with no process
and no container tree left; `--kill-after 45` exits 143 after a kernel ran;
a marimo file exports to HTML that holds the output of its cell.

The verb: each cycle, and each part after a restart, is a new `podvm run
--rm` of the same binary with a read-only root, `--tmpfs /tmp`, a
`/workspace` with only the notebook and `--persist` at `/persist`; a
`.ipynb` runs through `jupyter nbconvert --execute --allow-errors
--stdout` of quay.io/jupyter/base-notebook, a `.py` through `marimo export
html` of ghcr.io/marimo-team/marimo. The images are glibc: the userland
rung runs them since T-1644, so no Alpine image is needed.

Two defects were found and fixed. The first exec of the loader family gave
the payload the host path of the loader's file as `argv[0]`, so the
`#!/opt/venv/bin/python3` of the marimo script lost its virtual
environment; the loader now gets `--argv0` with the invoked name, as the
interposer's exec route gives a child (`crates/podvm/src/lifecycle/argv0.rs`).
A run killed during the copy of its container tree left the partial tree;
a tree is now filled under `trees/.fill-PID-ID` and renamed when whole,
and each fill removes a partial tree whose process is gone.

# T-1661: Language runtime profiles: Node, Deno and Bun permissions, wasmtime and workerd

**Source:** issue 176; #193; the owner's ruling of 2026-11-06;
  A-V-05 of PodUtils/podcrates#39
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

For a language runtime with its own permission model, the
runtime is the boundary. podvm cannot start such a runtime
with its flags from one statement of the paths, and cannot say
which part the runtime enforces and which part podvm enforces.

## Premise

None of these runtimes needs a privilege. Today they hit filed
limits: the Debian images are glibc (T-1644), `node` in
node:alpine is over the read ceiling (T-1647), its entrypoint
is a `#!` script (T-1649), and `npm` spawns children (T-1646).
An `fs.wrap` prefix arrives with T-1654.

## Approach

Ship built-in profiles `node-permission`, `deno-permissions`,
`bun-runtime`, `wasmtime-wasi` and `workerd-test`. Generate the
runtime's flags (`--allow-fs-read`, `--allow-write`, `--dir`)
from the profile's `fs.read` and `fs.write`, so a profile
states each path once. Check the linkage of a host-style
binary before the rung is chosen. `profile show` prints the
runtime flags and podvm's path map side by side.

## Decision

The WASI profile needs no interposer and no tier: the module
sees only its preopened directories. Bun has no permission
model, and its profile says so. A runtime check never stands
in for podvm's own path map.

## Prove

`cargo test -p podvm` exits 0 with a test that one profile's
paths produce both the runtime flags and the path map.
`sh experiments/417-runtime-profiles.sh` exits 0 with the
fixtures of issue 176: `node-permission` prints `permission
on`, DENY for `/etc/hostname` and `/tmp`, ALLOW for
`/workspace`; `deno-permissions` prints `NotCapable` for the
outside read and for `Deno.env`; `wasmtime-wasi` prints `hello
from wasi`; `workerd-test` reports `typeof process =
undefined` and makes no connection: a `fetch` of a public URL from the
worker fails.

## Correction

**Filed 2026-11-06.** Quoted from issue 176:

```text
node-permission (Node 26): permission on; read /etc/hostname ERR_ACCESS_DENIED;
                 write /workspace/out.txt allowed; write /tmp/out.txt
                 ERR_ACCESS_DENIED
deno-permissions (Deno 2.9): NotCapable for reads outside /workspace, for
                 /tmp writes and for Deno.env (HOME)
bun-runtime (Bun 1.4): no permission model
wasmtime-wasi (wasmtime 50): "hello from wasi" with only /workspace
                 preopened and no sockets
workerd-test: typeof process = undefined; node:fs and node:child_process
                 cannot be imported; [ PASS ] main
F25: wasmtime and workerd publish no checksum; recorded by hash on first use
```

**Amended 2026-11-09.** The ask A-V-05 of PodUtils/podcrates#39 (COORD-015,
a message from a peer session) wants a workerd profile with no network.
`workerd-test` gets no outbound service in its generated workerd config, and
`net.ports = []` for the confine layer of T-1658. Where Landlock is absent,
the profile refuses by name, as T-1658 says. The Chromium half of the ask is
T-1694.

## Done

**Done 2026-11-12.** `wasmtime-wasi` is a built-in profile: `podvm run
--profile wasmtime-wasi -- MODULE` runs `wasmtime run --dir=/workspace`
of a module of `/workspace`. wasmtime publishes no checksum (F25), so
podvm pins its own measurement of the release v50.0.1: the SHA-256 of
`wasmtime-v50.0.1-x86_64-linux.tar.xz` and of the program in it. The
pinned programs of workerd and wasmtime share one table and one fetch
(`crates/podvm/src/profile/tools.rs`, a `.tar.gz` and a `.tar.xz`), and
each goes to `<store>/tools/` once. COORD-053 (issue 274) and COORD-054
(issue 275) ask podcrates to hold the pins in `pod-services`.

`cargo test -p podvm` exits 0 with
`each_pin_names_its_version_and_its_guest_directory`. `PODVM_417_MIRROR=1
sh experiments/417-runtime-profiles.sh` exits 0 with each of its six
clauses: clause 6 prints `hello from wasi`, and `/workspace` is the only
preopened directory of the module.

Since podcrates `v0.28.0` the URL, the version and the sums of both pins
are facts of `pod-services` (`cloudflare-workerd-npm`, `wasmtime-release`;
COORD-053 and COORD-054), and `crates/podvm/src/profile/tools.rs` reads
them there; `WAITING` of `crates/podvm/src/hosts_tests.rs` is empty.

**Partial 2026-11-11.** `cargo test -p podvm` exits 0 with
`one_profile_gives_the_runtime_flags_and_the_path_map` (the paths of
`node-permission` give both `node --permission --allow-fs-read=/workspace
--allow-fs-write=/workspace` and the `-v` of the path map),
`each_runtime_gets_its_flags_from_the_paths` and
`a_repeated_program_is_not_given_twice`. `PODVM_417_MIRROR=1 sh
experiments/417-runtime-profiles.sh` exits 0
(`experiments/results/runtime-profiles.txt`): on the chroot-denying
fixture, `node-permission` prints `permission on`, ERR_ACCESS_DENIED for a
read of `/etc/hostname` and a write of `/tmp/out.txt`, allows a write of
`/workspace/out.txt`, and `lstat` of `/workspace` answers a directory;
`deno-permissions` prints `NotCapable` for the read of `/etc/hostname`, the
write of `/tmp/out.txt` and `Deno.env`, and reads `/workspace`;
`bun-runtime` runs a script of `/workspace` in a TCG guest, and no emulator
stays; `profile show` prints the runtime's flags beside the run flags.

The parts of the build: the top-level key `runtime` (node, deno, bun,
wasmtime) and the keys `fs.read` and `fs.write`; the runtime and its flags
prefix the command, and a command that repeats the program (`node a.js`)
does not give it twice; a runtime profile clears the image's entrypoint
(`--entrypoint=`, which `run` now takes as docker does). Two defects were
found and fixed: the interposer answered `lstat("/workspace")` from the
rootfs, not from the volume (a mount point is never a link, so a call that
does not follow takes the volume's host path); and the guest's base archive
copied each hard link in full (busybox:musl took 500 MB).

Measured: on the userland rung, Bun read `/workspace/b.js` of the host's
own `/workspace`. Bun makes its own system calls, past the interposer, so
`bun-runtime` runs on the machine tier with 1 GiB.

**Partial 2026-11-12.** `workerd-test` is a built-in profile: `podvm run
--profile workerd-test` in a directory with `worker.js` runs `workerd test`
of that worker. workerd has no OCI image of its party, so podvm pins the
npm package `@cloudflare/workerd-linux-64` 1.20261029.1: the SHA-256 of
the archive and of `package/bin/workerd` (the registry's sha512 of the
archive agrees), fetched once into `<store>/tools/` and mounted read-only
at `/opt/podvm-workerd` (`crates/podvm/src/profile/workerd.rs`). The
config that podvm writes beside it has one worker, `globalOutbound =
"none"` and a network service that allows nothing, and the compatibility
date 2025-09-15, where `process` is undefined. The image is
`gcr.io/distroless/cc-debian12`, pinned by digest: glibc and a CA store
(workerd stops where it loads no CA certificate, also with no network).
The profile sets `confine = true` and `net.ports = []`: an empty list now
holds each TCP connect (`connect=` with no port), and no key leaves TCP
free. A profile that names its image now runs with no command; the
runtime workerd gives its own. `registry.npmjs.org` is in `WAITING` of
`crates/podvm/src/hosts_tests.rs` until `pod-services` lists it (COORD-053,
issue 274).

`cargo test -p podvm` exits 0 with `workerd_profile_holds_each_tcp_connect`
and `the_config_has_no_outbound_service`. `PODVM_417_MIRROR=1 sh
experiments/417-runtime-profiles.sh` exits 0 (clause 5, a fresh store):
the test of the worker passes, `typeof process = undefined`, `node:fs`
and `node:child_process` do not import, and a `fetch` to a host listener
fails with no connection.


# T-1694: Headless Chromium of Playwright runs as a proven payload with its flags in one profile

**Source:** A-V-05 of PodUtils/podcrates#39 (COORD-015); T-1661 (the
  runtime profiles); T-1655 (the hardening line);
  T-1658 (the confine layer)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

A browser test job needs headless Chromium, as Playwright
starts it. podvm has no profile that says how to start it on
a host that denies namespaces, and no proof that it runs.
Chromium starts many children, wants a shared-memory
directory, and is a large glibc binary. The table refuses a
`--tmpfs` and a `--shm-size`, so the usual answer to the
shared-memory need does not work.

## Premise

Chromium's own sandbox needs user namespaces, which the
target host denies. Playwright turns it off with
`--no-sandbox`. Then the boundary is the confine layer of
T-1658 and not the browser. The parity table refuses
`--tmpfs` (`crates/podvm/src/parity.rs:259`) and
`--shm-size` (`crates/podvm/src/parity.rs:266`). The chain
of entries for a large dynamic payload is T-1643 to T-1646,
T-1647 and T-1649. No run of Chromium under podvm was
measured when this entry was filed.

## Approach

Add the built-in profile `playwright-chromium` to the T-1654
profile table. It sets `--headless`, `--no-sandbox` and
`--disable-dev-shm-usage`, which moves the shared memory to
a store directory under a size check. It sets `fs.read` and
`fs.write` to the paths that the browser needs, and
`net.ports = []` unless the profile asks for a proxy. Pick
one image with Playwright's Chromium and record its digest
after the first run. Measure the first run, name each
failure by its filed entry, and refuse by name until the
chain works.

## Decision

`--no-sandbox` is explicit in the profile and in the banner.
The profile refuses where the T-1658 layer is absent: it
never runs Chromium unconfined. Chromium under podvm is not
a boundary against a hostile page. The profile says so.

## Prove

`cargo test -p podvm` exits 0 with tests
`chromium_profile_flags` and `chromium_profile_refuses_without_confine`.
`sh experiments/425-chromium.sh` exits 0 on a chroot-denied
fixture with the chain of entries done: headless Chromium
from the pinned image prints the DOM of a page in
`/workspace` with `--dump-dom`, writes a screenshot of more
than zero bytes, makes no network connection, and the run
leaves no child process.

## Correction

**Filed 2026-11-09.** From the ask A-V-05 of PodUtils/podcrates#39
(COORD-015), a message from a peer session, priority P2: "headless
Chromium of Playwright; a workerd profile with no network". The workerd
half is in T-1661. The ask is a new capability and nothing documented is
broken, so the priority is P2.

## Done

**Done 2026-11-12.** The built-in profile `playwright-chromium` runs
`chrome-headless-shell` of `mcr.microsoft.com/playwright:v1.65.0-noble`,
pinned by digest (`sha256:9beb5460...`), with `--headless`,
`--no-sandbox`, `--no-zygote`, `--single-process`,
`--disable-dev-shm-usage` and `--disable-gpu` in `fs.wrap`, so the
arguments after `--` go to the browser. It sets `confine = true` with
read of the font, CA and identity files, write of `/workspace` and
`/tmp`, and execute of `/ms-playwright`, through the new key `fs.exec`
(Landlock gives a library mapped executable only with the EXECUTE right).
`--no-sandbox` is in the profile and its `leaves_out` says that the
browser is no boundary against a hostile page.

The chain of the first run: the namespace rung mounts no `/proc`, and
Chromium finds its ICU data through `/proc/self/exe`, so the profile runs
on the loader family (named in `leaves_out`); there, Chromium aborted with
"free(): invalid pointer" because the interposer gave back libc's
`malloc` buffers to a program with its own allocator (T-1738, fixed); the
children of the default mode did not start (T-1739), so the profile used
`--single-process` until T-1739 was done; it now runs the default mode,
with a zygote and a GPU process.

`cargo test -p podvm` exits 0 with `chromium_profile_flags` and
`chromium_profile_refuses_without_confine`. `sh
experiments/425-chromium.sh` exits 0 on the chroot-denying fixture: the
DOM of a page of `/workspace` is printed (12 s), a screenshot of 3252
bytes is written, a page that loads an image from a host listener makes
no connection under `-s net.ports=[9]` (the control with no port rule
connects once), and no process of the browser stays. The host
`mcr.microsoft.com` waits in the reported list of
`crates/podvm/src/hosts_tests.rs` for `pod-services` (COORD-050).


# T-1695: One JSON report for each run, and real labels on records

**Source:** A-V-09, A-V-07 and G-06 of PodUtils/podcrates#39 (COORD-015);
  T-1655 (the peak resident set and the CPU time);
  T-1319 (one `inspect` document);
  `crates/podvm/src/parity.rs` line 278 at `9962ba77`
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** L
**Status:** done

## Problem

A caller that runs a job under podvm has no machine-readable
record of the run. The mechanism that podvm entered is a
banner on stderr. The caller cannot read the exit status,
the times and the peak memory from one place. `--label` also
does nothing: the container record carries no labels, so
`ps --filter label=` matches nothing, and a runner cannot
find its own jobs.

## Premise

The `--label` flag is accepted and dropped
(`crates/podvm/src/run/parse.rs` line 106 at `9962ba77`). The parity table
marks it Stub (`crates/podvm/src/parity.rs` line 278 at `9962ba77`). The help
of `ps` says that a label filter matches nothing
(`crates/podvm/src/lifecycle.rs` line 41 at `9962ba77`), and so does the help of
`images` (`crates/podvm/src/images.rs` line 106 at `9962ba77`). The only JSON
that podvm writes for a caller today is `probe --json`
(`crates/podvm/src/main.rs:361`). The foreground run knows
the exit code at its end (`crates/podvm/src/run/foreground.rs:204`)
and its start is `crates/podvm/src/run/foreground.rs:233`.
The record that `run` prepares is
`crates/podvm/src/lifecycle/entry.rs:24`.

## Approach

Keep each `--label KEY=VALUE` in the container record. Make
`ps --filter label=KEY[=VALUE]` and `inspect` use it. Take
the labels of an image from its OCI config for `images
--filter label=`. Add `run --report PATH`. The run writes
exactly one JSON document to PATH, with a rename, also when
it refuses before the start. The document holds: a schema
version, the podvm version, the image digest, the rung or
tier that the run entered and how it was measured, the
argument vector, the exit status or the signal, the start
and end times, the wall time, the CPU time and the peak
resident set of the tree (T-1655), whether a timeout ended
the run, the limits applied, the labels, and each
degradation. An unknown value is `null`.

## Decision

The report goes to a file, never to standard output: that
belongs to the payload. The report holds no value of an
environment variable and no secret. One run writes one
report, and a runner that wants one report for each job
gives each job its own path (G-06). The `--strict` rule that
refuses a Stub no longer applies to `--label`, because the
flag works. The parity row moves from Stub to Native.

## Prove

`cargo test -p podvm` exits 0 with tests
`label_reaches_ps_filter`, `label_reaches_inspect`,
`report_has_every_field` and `report_written_on_refusal`.
`sh experiments/426-run-report.sh` exits 0: a run with
`--label job=7 --report R.json` leaves one valid JSON file
that names the tier, exit 3 for `sh -c 'exit 3'`, a peak
resident set above zero, and the label; `ps -a --filter
label=job=7` lists the run.

## Correction

**Filed 2026-11-09.** From the asks A-V-09 (priority P1), A-V-07 and G-06
of PodUtils/podcrates#39 (COORD-015), a message from a peer session: "a
JSON report of each run, and real labels", with peak RSS and CPU in the
report, and one JSON report for each job. The priority is P1 because
`--label` is a flag that does nothing (the priority rules of
`TODO/INDEX.md`). The report half alone would be P2.

**Amended 2026-11-10, for podrunner.** The podrunner design review (the
owner's design review of PodUtils/podcrates#47; the ruling "podrunner",
2026-11-10; SPEC section 11.4 and the `wait` answer of its unit contract) reads
this report as the podvm half of its run report. Four clauses are new.
First, a detached run (`run -d`, `start`) writes its report too: the
launcher writes it with a rename when it reaps the payload, and `podvm
wait` returns after the rename. Second, the report names the entered rung
with the word of `.EnteredRung` (T-1713) and lists the volume table with
the mode of each pair. Third, the report gives the count of processes of
the tree that the end killed (T-1708, the subreaper), or `null` where
podvm cannot count. Fourth, an end by a signal names the signal (for
example `SIGXFSZ`) beside the number. Bytes in and out are not podvm's to
measure; the report does not carry them.

## Done

**Done 2026-11-11.** `run --report PATH` writes one JSON document of the
run with a rename, also when podvm refuses before the start
(`crates/podvm/src/run/report.rs`): the schema, the podvm version, the
image and its digest, the rung and how podvm knows it, argv, the
container, the exit code and the signal (128 + N), the refusal, the start,
the end and the wall time, the CPU time and the peak resident set of the
reaped children of the run (`getrusage(RUSAGE_CHILDREN)`), the timeout,
the limits, no_new_privs, the confine SPEC, the labels and the fixups. An
unknown value is null; no value of an environment variable is in it.
`--report` is a flag of `run` only: `create` refuses it by name. `images
--filter label=KEY[=VALUE]` reads the `config.Labels` of the image's OCI
config blob; by hand, `public.ecr.aws/lambda/provided:al2023` matches
`label=com.amazonaws.lambda.platform.kernel=k510ga` and not another value.
`cargo test -p podvm` exits 0 with `report_has_every_field`,
`report_written_on_refusal` and the image-label test.
`sh experiments/426-run-report.sh` exits 0 on the cloud host: a run with
`--label job=7 --report R.json` of `sh -c 'exit 3'` exits 3 and leaves one
valid document with rung `namespace`, exit_code 3, signal null,
max_rss_kib 26972 and the label; `ps -a --filter label=job=7` lists the
run and `label=job=8` does not; a refused run leaves its report (refused
125, exit_code null, no temporary file); the value of `-e SECRET=` is not
in the report.

**Partial 2026-11-11.** Labels are real for containers: each `--label
KEY[=VALUE]` of `run` and `create` joins the record (`Hardening.labels` of
`crates/podvm-supervise/src/table.rs`; the foreground record now stores
the options of its run, as `run -d` does), `ps --filter label=KEY` and
`label=KEY=VALUE` match them with docker's rule, and `inspect` prints
`.Labels` (`KEY=VALUE`, a comma between two). A label with no key is a
flag error (125). The parity row of `--label` moves from Stub to Native.
`cargo test -p podvm --test labels` exits 0 with `label_reaches_ps_filter`
and `label_reaches_inspect`, on an image that `podvm import` records (a
static program) and the record of a foreground run, with no registry. The
first form used `create` of a `#!` script: CI denies chroot, and there
`create` found no entry for it (gate run 43135351382); the test was
corrected on 2026-11-12.



# T-1696: `--secret` places a file with no argv and no env, and a read-once mode

**Source:** A-V-11 and G-03 of PodUtils/podcrates#39 (COORD-015);
  T-1653 (the volume path map); the ruling of 2026-11-09 on
  test accounts; `crates/podvm/src/run/parse.rs:108`
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

A job needs a credential. Today a caller passes it with `-e`,
which puts the value in the environment of the payload and
of each child, or in a file that a `-v` volume would place.
podvm has no flag that places one file, with a mode, for
the first process only.

## Premise

`--env-file` reads a file at parse time
(`crates/podvm/src/run/parse.rs:108`, used at
`crates/podvm/src/run/parse.rs:224`) and the values go into
the environment (`crates/podvm/src/run/prepare.rs:245`). The
userland path map is one pair for the root
(`src/userland.rs` of `pod-enter` (PodUtils/podcrates)); T-1653 adds pairs.
A foreground run removes its files at the end
(`crates/podvm/src/run/foreground.rs:204`). The parity table
has no `--secret` row. The interposer is not a boundary, so
a claim about "only the runner process" has a limit that the
banner must state.

## Approach

Add `run --secret src=PATH,target=GUEST,mode=0400[,once]`.
podvm reads `src` itself. The value never enters argv or the
environment. It stages the bytes under the run directory
(mode 0700 for the directory, the given mode for the file)
and adds one path-map pair from `target` (T-1653). Refuse a
`src` that group or other can read, and name its mode. With
`once`, the interposer serves the first `open` of `target`
from the first process of the payload (the runner) and
removes the staged file at once. A child, and any later
`open`, gets `ENOENT`. At the end of the run, also after a
failure, podvm removes the staged file. `inspect`, `ps`, the
banner, the logs and the report (T-1695) show the target and
the mode, never the value. The machine tier refuses
`--secret` by name until T-1656 carries a per-run file into
the guest.

## Decision

The credential reaches the runner process through a file and
nothing else (G-03). A payload that bypasses the libc
interposer with raw system calls can read the staged file if
it learns its host path. The host path is in no variable, and
the banner names this limit. podvm stores no credential
between runs. No test account is a CI secret now (the ruling
of 2026-11-09), so a live step that needs one waits.

## Prove

`cargo test -p podvm` exits 0 with tests
`secret_spec_parses`, `secret_refuses_group_readable_src`,
`secret_value_absent_from_argv_and_env` and
`secret_once_second_open_fails`.
`sh experiments/427-secret-flag.sh` exits 0 on a
chroot-denied fixture: the runner reads the target with mode
0400, `/proc/PID/cmdline` and `/proc/PID/environ` of the
payload hold no byte of the value, with `once` a second read
and a read from a child fail, and the staged file is gone
after the run, also after a payload that exits 1.

## Correction

**Filed 2026-11-09.** From the asks A-V-11 (priority P2) and G-03 of
PodUtils/podcrates#39 (COORD-015), a message from a peer session:
"--secret src=,target=,mode=0400 with no argv and no env", with a
read-once mode, and "the one-job runner credential reaches only the
runner process". The peer asks for P0 for G-03. The priority stays P2: no
behaviour of podvm is wrong today, and the flag is new.

**Amended 2026-11-10, for podrunner.** The podrunner design review (the
owner's design review of PodUtils/podcrates#47; the ruling "podrunner",
2026-11-10; SPEC section 8.2, its measurement M1 with runner v2.338.2) measured
the official runner: its credential is three files (`.runner`,
`.credentials`, `.credentials_rsaparams`) in its root; it keeps them for
its life and does not write them again; its Worker child reads `.runner`
from disk again. Four clauses are new. First, `--secret` is repeatable,
and several targets can be in one guest directory that the payload also
writes. Second, `once` breaks a payload whose child reads the file again;
the help of `once` says so, and the experiment adds a child that reads a
target a second time without `once`. Third, `--secret` works with `run -d`
and `start`: the staged files stay until the launcher reaps the payload,
and `rm` removes them also when the launcher is gone. Fourth, the staging
directory is mode 0700 whatever the umask (T-1715, the store modes), and
the tests use mode 0600 as well as 0400.

## Done

**Done 2026-11-11 by withdrawal: the owner dropped this entry (the ruling
"T-1696 dropped", the answer to Q5).** No code was built and the Prove did
not run. podvm has no `--secret` flag and no read-once mode. podrunner
stages the three credential files of the official runner itself, at mode
0600, with no argv and no environment, and removes them at `destroy` (G-03
as DR-6 restates it, PodUtils/podrunner#6). The clauses of other entries
that named `--secret` are corrected in place: T-1695 (the report),
T-1715 (the store modes) and T-1656 (the guest's tmpfs).

# T-1667: Doctor and installers: one check per scope capability, verified downloads, a space floor

**Source:** issue 188; #193; the owner's ruling of 2026-11-06
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** partial

## Problem

`podvm doctor` reports the rootfs and machine legs. It does
not cover the widened scope: Landlock, Syscall User Dispatch,
`openat2(RESOLVE_IN_ROOT)`, user-mode networking, labs in a
guest, proxy reach. Assets that the new scope downloads
(guest kernels, runtimes) have no stated verification rule.

## Premise

The doctor is `crates/podvm/src/doctor.rs:149`, and it reads
the machine assessment (`doctor.rs:190`). The probe already
has a Landlock leg (`pod_hostprobe::probes`).
Space is checked before large writes
(`pod_hostprobe::space`, T-0806). Release
verification uses cosign and the workflow identity (T-1328).

## Approach

Add one doctor group for each scope capability. Each check
prints `ok`, `MISSING` with a `fix:` line, or "cannot be fixed
on this host" with the reason. `--strict` exits 1 on a
MISSING. Each check proves the thing runs: a guest boots, a
payload enters. For each downloaded asset, verify a checksum
and a signature where upstream publishes them, else record a
first-use hash in the store and refuse a change. Fetch a
release through `gh`, and fall back to the public API when
the call fails, not when `gh auth status` fails.

## Decision

A present file is not a working one (T-1301). An auth check is
not a working credential: a suspended token passes `gh auth
status` and then gets 403 (F34). A checksum proves integrity,
not who built the file, and the doctor says which of the two
it has.

## Prove

`cargo test -p podvm` exits 0 with tests: a planted changed
asset is refused, a planted rejected `gh` token falls back to
the API, and a run below the space floor refuses and names the
path. `sh experiments/423-doctor-scope.sh` exits 0 on a
chroot-denied fixture: `podvm doctor` lists every scope
capability with one of the three answers, and `podvm doctor
--strict` exits 1 where one is MISSING.

## Correction

**Filed 2026-11-06.** Quoted from issue 188:

```text
F34: `gh auth status` exits 0 for a suspended token while every
     `gh release` call returns 403; the unauthenticated API allows 60
     requests an hour per address, and an install script hit the limit
F25: landrun, wasmtime and workerd publish no checksum, and
     `gh release view --json assets` gives no digest for them
F12: `apt-get install tayga` pulled apparmor; its pasta profile in enforce
     mode made `pasta --version` exit 139
F26: `pasta --version` succeeding does not mean pasta works
F31: a run failed with "no space left on device" at 188 MiB free; the
     disk guard warns below 1 GiB and refuses below 64 MiB
```

## Done

**Partial 2026-11-12.** The `gh` clause: the pin of wasmtime (T-1661) is
the first GitHub release that podvm fetches. `crates/podvm/src/profile/gh.rs`
runs `gh release download TAG -R REPO -p ASSET -O -` first, and takes the
public URL where that call fails (no `gh`, a rejected token, an answer over
the limit), with the reason on stderr; it asks no `gh auth status`. The pin's
SHA-256 checks the bytes of each path. `cargo test -p podvm` exits 0 with
`a_rejected_gh_token_falls_back_to_the_public_url` (a planted `gh` that
answers "HTTP 401: Bad credentials"), `gh_gives_the_bytes_where_its_call_works`
and `no_gh_falls_back_to_the_public_url`. Measured on the cloud host: its
`gh` exits 1 (the proxy refuses GraphQL), podvm names that and fetches the
public URL, and `wasmtime-wasi` runs.

**Partial 2026-11-11.** Done: `cargo test -p podvm` exits 0 with
`a_planted_changed_asset_is_refused` and
`a_run_below_the_space_floor_refuses_and_names_the_path`
(`crates/podvm/src/assets.rs`), and
`a_missing_scope_capability_fails_only_under_strict`.
`sh experiments/423-doctor-scope.sh` exits 0 on the chroot-denying
fixture (`experiments/results/doctor-scope.txt`): each of the ten scope
capabilities (`native-static`, `syscall-tier`, `openat2`, `landlock`,
`user-net`, `emulate`, `incus`, `proxy`, `space`, `assets`) has one
line with ok, MISSING or cannot; `scope.incus` is MISSING with its fix;
`doctor` exits 0 and `doctor --strict` exits 1; with
`--deny-dispatch`, the native-static rung and the syscall tier read
"cannot be fixed on this host". The native-static check proves the rung by
a run: podvm enters itself in a root of its own and prints its version.

Downloads: `crates/podvm/src/assets.rs` checks an asset against podvm's
pin, or records its SHA-256 at the first use in `<store>/assets/` and
refuses a change, naming the record to remove. `PODVM_MACHINE_KERNEL`, a
kernel of the caller with no pin, takes the first-use rule; the pinned
guest kernel keeps its pin. The fetch of the guest kernel checks a space
floor of 64 MiB first and names the path. `scope.assets` says what each
check proves: integrity, not who built the file.

Remaining clauses:
* "Labs in a guest": a check for it comes with the network entries
  T-1662 to T-1665.

# T-1690: The doctor disk-free row judges the soft `RLIMIT_FSIZE`

**Source:** issue 214; T-1301 leg 3; T-1305
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`podvm doctor` reads the hard `RLIMIT_FSIZE` in the row "disk
free against the file-size ceiling". The kernel raises SIGXFSZ
at the soft limit, and each other reader of the limit reads
the soft one. With a soft limit of 1 GB and a hard limit of
infinity, doctor prints a note and exits 0, and a machine-tier
run refuses on the same host.

## Premise

`crates/podvm/src/doctor.rs:317` keeps the hard limit. The
infinity test is at `crates/podvm/src/doctor.rs:329` and the
comparison at `crates/podvm/src/doctor.rs:343`. The soft
readers are `crates/podvm/src/tier.rs:121`,
`crates/podvm/src/machine/ssh/qemu.rs:260`,
`crates/podvm/src/windows/plan.rs:88`,
`crates/podvm-probe/src/nongoals.rs:190` and
`pod_hostprobe::space`. The function reads `prlimit` itself, so
a test cannot drive it.

## Approach

Keep the soft limit and use it for the infinity test and the
comparison. Print the hard limit as well when it differs. Move
the judgement into a pure function over the soft limit, the
hard limit and the free bytes.

## Decision

The enforced bound is the soft limit. Doctor and the machine
tier print the same number on one machine.

## Prove

`cargo test -p podvm` exits 0 with tests: a soft 1 GB and a
hard infinity give the 1 GB ceiling; the same limits with less
free space fail with a fix line; both infinite stay a note.

## Correction

**Filed 2026-11-09.** From issue 214 (an outside account). The triage of
2026-11-09 confirmed each claim at `2be31b2`.

## Done

**Done 2026-11-09.** `examine_ceiling` keeps the soft limit and judges
it; the comparison is the pure function `judge_ceiling`, which names the
hard limit when it differs. `cargo test -p podvm` exits 0 with
`the_ceiling_is_the_soft_limit` (a soft 1 GiB and a hard infinity give
the 1 GiB ceiling; less free space fails; equal limits print no hard
limit). [The drive](../experiments/results/doctor-soft-fsize.txt): with
`ulimit -S -f` at 2 GiB and the hard limit at infinity, `podvm doctor`
judges the 2 GiB ceiling, where it printed "no file-size ceiling"
before; at 64 GiB, above the free space, the row fails with its fix line.

# T-1706: The unmodified official `actions/runner` starts on the userland rung

**Source:** issue 231; the podrunner design review (the owner's design
  review of PodUtils/podcrates#47), SPEC sections 10.4, 10.5, WP-V1
  and milestone M-B, its measurements M1 and M2; A-V-04 of
  PodUtils/podcrates#39; T-1704 (`realpath` of
  `/proc/self/exe`); T-1705 (the doubled prefix); T-1707 (a
  first program relative to `-w`); T-1655 (`--ulimit`);
  `crates/podvm/src/tier.rs:121`
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

The official runner (linux-x64, glibc, .NET) does not start
under podvm on a chroot-denied host: exit 154, then 137 after
a workaround. It runs only under `--unsafe-host-paths` with
the runner at its real host path, which is not isolation. A
.NET payload also dies at start with SIGXFSZ (exit 153) under
each soft `RLIMIT_FSIZE` below 2 TiB, and podvm says only "the
payload died on signal 25".

## Premise

M2 at `36b15ec`, one host, runner v2.338.2, 3 runs for each
case. Blocker 1 is the `realpath` answer (T-1704), blocker 2
the doubled prefix (T-1705). A third layer after `--help` is
possible: no job ran. M1: the runtime calls `ftruncate` on a
memfd with 2199023255552 bytes; under a smaller limit that
is `EFBIG` and SIGXFSZ, in the listener, in `config.sh` and in
the Worker; with `DOTNET_EnableWriteXorExecute=0` it starts.
M2: a `prlimit` on the podvm process reaches the payload. M1:
the runner needs ICU or
`DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1`, `libssl.so.3`, and
a glibc loader with symbols up to `GLIBC_2.28`. podvm reads
the soft limit for guests (`crates/podvm/src/tier.rs:121`)
and not for a payload. No podvm record names .NET.

## Approach

Build a test image:
`public.ecr.aws/debian/debian:bookworm-slim`, the runner's
libraries, and the runner tarball checked against a pinned
SHA-256; build it where chroot is permitted and `podvm
import` it. Close T-1704, T-1705 and T-1707. Then drive, on
the chroot-denying fixture: `Runner.Listener --version`,
`config.sh --help`, and `run.sh` with the three credential
files of a synthetic blob pre-staged at mode 0600 against a
loopback stand-in that serves `POST /oauth/token`, `POST
/session` and `GET /message` (the shape of M1). File each new
blocker as an entry. When a payload dies on SIGXFSZ, print
the soft `RLIMIT_FSIZE`; for a payload with a `.dll` beside
it or a `DOTNET_` variable, also name
`DOTNET_EnableWriteXorExecute=0`.

## Decision

The runner is a payload, not a special case: podvm adds no
runner flag and ships no runner. `--unsafe-host-paths` is not
a pass. podvm does not raise or drop the caller's
`RLIMIT_FSIZE`; it names it. The stand-in server is test code.

## Prove

`sh experiments/428-actions-runner.sh` exits 0 on the
chroot-denying fixture with no `--unsafe-host-paths`:
`--version` prints the pinned version, `config.sh --help`
exits 0, `run.sh` with the pre-staged files reaches the long
poll of the stand-in and ends within 30 s of SIGTERM, and
under `prlimit --fsize=1073741824` a run with W^X on exits
153 with a message that names `RLIMIT_FSIZE`, while the same
run with `-e DOTNET_EnableWriteXorExecute=0` exits 0.

## Correction

**Filed 2026-11-10.** From issue 231 and the podrunner design review
(the ruling "podrunner", 2026-11-10), WP-V1: "Official runner starts on
the userland rung without `--unsafe-host-paths` (exit 154, then 137)".
The entry is a new proven payload, so it is P2; its defects T-1704,
T-1705 and T-1707 are P1 and are podvm's own.

## Done

**Done 2026-11-11.** `experiments/428-actions-runner.sh` builds the test
image on the capable rung (debian bookworm-slim, `ca-certificates`,
`libicu72`, `libssl3`, `libkrb5-3`, `zlib1g`, `liblttng-ust1`, and the
runner v2.338.2 in `/opt/runner`, checked against SHA-256 `baebf204…`,
with the three credential files of a synthetic blob at mode 0600), and
`PODVM_428_MIRROR=1 sh experiments/428-actions-runner.sh` exits 0 on the
chroot-denying fixture with no `--unsafe-host-paths`
([the run](../experiments/results/actions-runner.txt)):

* `-w /opt/runner ./bin/Runner.Listener --version` prints `2.338.2`;
* `./config.sh --help` exits 0;
* under `prlimit --fsize=1073741824` the same run exits 153, and podvm
  names the soft `RLIMIT_FSIZE` of 1073741824 bytes and
  `DOTNET_EnableWriteXorExecute=0`; with that variable it exits 0;
* `./run.sh` against `experiments/runner-standin.py` (the shape of
  podrunner's `podrunner-broker-fake`, PodUtils/podrunner#50) takes a
  token, makes a session, prints "Listening for Jobs" at the long poll,
  and ends 1 s after SIGTERM to podvm (143), with no `Runner.Listener`
  left.

The SIGXFSZ lines are `crates/podvm/src/run/exit_note.rs` (a foreground
run; a .NET payload is a `DOTNET_` variable or a `.dll` beside the
program). Found on the way: with a relative first program the interposer
was not loaded (fixed under T-1707); and the runtime asks `realpath("")`
and `access("")` at each start, which printed "podvm cannot resolve the
path": an empty path now passes to the kernel, which answers `ENOENT`
(`prepare_mode` in `crates/podvm-interpose/src/map/rewrite.rs`, the test
`an_empty_path_passes_unchanged`). The runner sent no `DELETE /session`
before it ended on SIGTERM; an ephemeral runner needs none.

# T-1711: The calls of the four container-hook commands run through podvm verbs on the userland rung

**Source:** issue 236; the podrunner design review (the owner's design
  review of PodUtils/podcrates#47), SPEC sections 7.2, 10.1, its
  unknown U19 and its hook contract; its R4:2.6 (the official
  docker hook); G-09 of PodUtils/podcrates#39; T-1653
  (volumes); T-1655 (`:ro`); T-1707; T-1709; T-1710;
  `crates/podvm/src/parity.rs:222`;
  `crates/podvm/src/exec/enter.rs:203`
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

A container job of the official runner calls its container
hook. podrunner's hook asks the host to start a job container,
run each step in it, run a container action, read logs and
exit codes, and remove the containers. On a podvm unit those
are podvm verbs. No test drives that sequence, so nobody knows
which call fails first.

## Premise

`run -d`, `--entrypoint`, `-e`, `-w`, `--name` and
`--add-host` are Native (`crates/podvm/src/parity.rs:210` to
`crates/podvm/src/parity.rs:235`); `-v` is Degraded and
refuses `:ro` (`crates/podvm/src/parity.rs:222`); `exec -e`
and `-w` are Native, `exec -d` is None
(`crates/podvm/src/parity.rs:291` to
`crates/podvm/src/parity.rs:297`); `logs` is one interleaved
file (`crates/podvm/src/parity.rs:153`); `network` and `-p`
are None. `exec` checks its workdir through the container's
volume table (`crates/podvm/src/exec/enter.rs:203`), not yet
driven. The official docker hook calls `network create`,
`port` and `exec -i`, so it cannot run over podvm (Inferred).
Each `exec` runs the probe and completion again; the cost is
not measured.

## Approach

Write an experiment that replays the podvm calls of the
hook for a job with `container:` and one container action:
a job container with `run -d --entrypoint tail` and the seven
mounts of the job-container table (`/__w`, `/__w/_temp`,
`/__w/_actions`, `/__w/_tool`, `/__e` read-only,
`/github/home`, `/github/workflow`) from directories under a
runner root; a step with `exec -e K=V -w /__w/repo J sh -e
/__w/_temp/step.sh`; a container action with `run --rm`, the
five mounts of the container-action table and `-w
/github/workspace`; `logs`; `wait`; `rm -f`. Use a job image
that differs from the runner's image. Record each call as
passed or refused by name, and file each defect. Measure the
time of 20 `exec` steps.

## Decision

podvm stays a command line. It adds no hook, no socket
server and no word of podrunner (the ruling "New projects",
2026-11-09). The hook and its control socket belong to
podrunner.

## Prove

`sh experiments/431-hook-verbs.sh` exits 0 on the
chroot-denying fixture: each call of the sequence exits as
the experiment records, a step sees its `-e` value and its
workdir, a write to `/__e` fails with `EROFS`, a write to
`/__w` lands in the runner root, the container action reads
`/github/workspace`, `rm -f` leaves no process, and the time
of 20 steps is printed.

## Correction

**Filed 2026-11-10.** From issue 236 and the podrunner design review
(the ruling "podrunner", 2026-11-10), section 7.2: the hook relays to a host
socket, and the host-side server calls podvm. A new use with nothing
wrong without it is P2.

## Done

**Done 2026-11-11.** `sh experiments/431-hook-verbs.sh` exits 0 on the
chroot-denying fixture (the userland rung, alpine 3.20 as the job image):
`run -d --name job` with the seven mounts of the job-container table,
`/__w/_temp` nested in `/__w`, exits 0; a step `exec -e K=V -w /__w/repo
job sh -e /__w/_temp/step.sh` prints `K=V` and `pwd=/__w/repo`, and its
write lands in the runner root; a write to `/__e` (`:ro`) fails with "Read-only
file system" and leaves no file; the container action `run --rm` with the
five mounts of the container-action table and `-w /github/workspace`
reads the checkout and exits 7; `logs job` holds the output of the
entrypoint; `wait` of a detached `sh -c 'exit 5'` prints 5; 20 `exec`
steps take 1812 ms (about 90 ms each, the probe and the completion of each
`exec` included); `rm -f job` exits 0 and leaves no process of the job.
No defect was found: T-1653, T-1655, T-1707, T-1709, T-1710, T-1725 and
T-1726 carry the sequence. The official docker hook also calls `network
create`, `port` and `exec -i`, which podvm does not serve; podrunner's own
hook makes only the calls above.


# T-1712: `--podvm-tier=chroot` does nothing: it runs the ladder

**Source:** issue 237; the podrunner design review (the owner's design
  review of PodUtils/podcrates#47), SPEC section 10.1, its measurement
  M2 (`01-probe-rung.log`); T-1302; call C2 of
  `docs/decisions.md`; `crates/podvm/src/tier.rs:90`;
  `crates/podvm/src/parity.rs:245`
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

The parity row says that `--podvm-tier=chroot` selects the
chroot tier. It runs the ladder. On a chroot-denied host it
enters the userland rung with exit 0; where the probe selects
the namespace rung it enters that. A caller that asks for one
mechanism gets another, and only the banner says so.

## Premise

`resolve` maps the value to `Tier::Chroot`
(`crates/podvm/src/tier.rs:90`). Each caller compares the tier
with `Tier::Machine` only
(`crates/podvm/src/run/prepare.rs:28`,
`crates/podvm/src/exec/command.rs:42`). T-1302 made `chroot`
the override of the argv0 default `podvm` = machine. Call C2
(2026-11-06) removed that default, so the value has no effect
now. M2 observed exit 0 and `mode=userland`.

## Approach

Take one of two forms and record it. (a) Remove the value
`chroot`; `--podvm-tier` takes `machine` only, and the ladder
is the default. (b) Make `chroot` enter the chroot sequence
and nothing else: where chroot is denied, refuse at 125 with
the T-1317 text; where the probe selects the namespace rung,
enter chroot without the namespace. The recommendation is
(a): a caller reads the entered rung from the report
(T-1695) and from `.EnteredRung` (T-1713), and needs no force.
Correct the parity row in the same change.

## Decision

A flag that names a tier selects it or refuses by name. No
compatibility is kept (the ruling "Compatibility",
2026-11-05).

## Prove

`cargo test -p podvm` exits 0 with a test of the chosen form.
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh`
exits 0 with a new clause: on the fixture,
`--podvm-tier=chroot public.ecr.aws/docker/library/alpine:3.20
true` is refused by name (form a: an unknown value; form b:
chroot denied).

## Correction

**Filed 2026-11-10.** From issue 237 and the podrunner design review
(the ruling "podrunner", 2026-11-10), M2 surprise 7: "`--podvm-tier=chroot`
on a chroot-denied host silently enters userland". The cause is wider:
the value has no effect on any host. A flag that does nothing is P1.

## Done

**Done 2026-11-11, form (a)** (call C14 of `docs/decisions.md`).
`--podvm-tier` takes `machine` only: `tier::want` knows no `chroot`, and
`Want::Chroot` and `Tier::Chroot` are gone (`crates/podvm/src/tier.rs`).
`tier::refusal` gives the one flag error of `run`, `create` and `exec`:
`--podvm-tier takes machine, not "chroot": with no flag the ladder selects
the rung, and podvm system info --format '{{.EnteredRung}}' names the rung
that a run enters`. The parity rows of `run` and `exec`, both help texts,
`docs/architecture.md` and call C2 say the same. The Prove, on the cloud
host, 2026-11-11: `cargo test -p podvm -p podvm-measure` exits 0 (306
tests), with `the_flag_spells_machine_and_nothing_else` and
`the_tier_flag_takes_machine_only_and_qemu_args_collect_whole` of `run`
and of `exec` (both spellings of `chroot` exit 125).
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh` exits 0 with
the new clause 27 ([the run](../experiments/results/no-chroot-rung.txt)):
on the chroot-denied fixture `run --rm --podvm-tier=chroot` of the pinned
alpine digest exits 125 with that message. `cargo clippy --workspace
--all-targets -- -D warnings` exits 0.

# T-1713: `.EnteredRung` reads `chroot` where the probe selects `supervise` and chroot is denied

**Source:** issue 238; the podrunner design review (the owner's design
  review of PodUtils/podcrates#47), SPEC section 10.1, its measurement
  M2 (`01-probe-rung.log`); T-0804 rule 4; T-1317;
  `crates/podvm/src/system/df_info.rs:307`;
  `crates/podvm/src/run/acquire.rs:23`
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

On a host where the probe selects `supervise` (the listener,
the argument channel and ptrace work) and chroot is denied,
`system info` gives `.EnteredRung` = `chroot`. `run` enters
the userland rung there. A caller that reads the field before
a run reads the wrong mechanism.

## Premise

`crates/podvm/src/system/df_info.rs:307` takes the word from
`entered_word` of `pod-enter`, which maps each selection but
`interpose` and `namespace` to `chroot` (`src/lib.rs` of
`pod-enter` v0.12.0). `run` adds its own userland decision
(`crates/podvm/src/run/acquire.rs:23`). The help of `system
info` says that the field reads `userland` where chroot is
denied. The tests cover the `interpose` and `namespace`
selections only. M2 observed `.Rung`=`supervise`,
`.EnteredRung`=`chroot` and the banner `mode=userland`. The
errand host denies ptrace, so it selects `interpose` and reads
correctly.

## Approach

Derive `.EnteredRung` from the same facts that `run` uses:
where the `chroot(/tmp)` finding is denied, the word is
`userland`, whatever the selection. If the rule belongs in
`pod-enter`, send it to podcrates as a COORD issue
(`AGENTS.md`, section 9, item 4); else keep it in
`df_info.rs`.

## Decision

The field names what `run` enters, never what the probe
selected (T-0804 rule 4).

## Prove

`cargo test -p podvm` exits 0 with a test that a `supervise`
selection with a denied chroot reads `userland`.

## Correction

**Filed 2026-11-10.** From issue 238 and the podrunner design review
(the ruling "podrunner", 2026-11-10), M2 question Q1: "`system info`
gives `.Rung`=`supervise`, `.EnteredRung`=`chroot`; the first `run`
banner says `mode=userland`". A wrong answer is P1.

## Done

**Done 2026-11-11.** The rule stays in podvm (`df_info.rs`), because the
decision that it mirrors is podvm's: `decide_entry` asks `chroot_usable`
before the selection, and `pod-enter`'s `entered_rung` knows only the
selection. `system::df_info::entered_rung(selection, chroot_usable)` gives
`userland` wherever chroot is denied, else `pod_enter::entered_rung`;
`collect` gives it `chroot_usable` of the same findings. The help of
`system info` says so. The premise was wider than written: under the
chroot-denied fixture of experiment 356 the cloud host selects
`namespace` (user namespaces work), and the field read `namespace` while
`run` entered the userland rung. The Prove, on the cloud host,
2026-11-11: `cargo test -p podvm` exits 0 with
`a_denied_chroot_reads_userland_whatever_the_selection` (`supervise`,
`namespace`, `chroot` and `interpose` with a denied chroot read
`userland`; `supervise` with a usable chroot reads `chroot`). Clause 27
of experiment 356 exits 0 ([the run](../experiments/results/no-chroot-rung.txt)):
`system info --format '{{.Rung}} {{.EnteredRung}}'` under the fixture
prints `namespace userland`.

# T-1726: `exec` into a container ignores its `:ro` volumes and `--read-only`

**Source:** PodUtils/podrunner#62 (COORD-008 of podrunner, a message from
  a peer session); measured on the cloud host, 2026-11-11
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

On the loader family, `podvm run --rm -v DIR:/e:ro IMAGE sh -c
'echo x > /e/r.txt'` fails the write (EROFS), but after `podvm
run -d --name c -v DIR:/e:ro IMAGE sleep 30`, `podvm exec c sh
-c 'echo x > /e/w.txt'` exits 0 and `DIR/w.txt` exists
(measured, 2026-11-11). podrunner mounts the externals of the
runner as `/__e:ro`, so one step can change the code of the
next.

## Premise

`run` sets the read-only table `PODVM_RO` of the interposer
(`lifecycle::volume::read_only_env`, T-1655). An `exec` enters
the container's rootfs with the container's table of volumes.

## Approach

Keep the read-only part of the run in the container record and
set `PODVM_RO` for each `exec` of the container as `run` does;
an entry that cannot serve it refuses by name, as `run` does.

## Decision

No entry of a container serves a write under a `:ro` volume or
under `--read-only`.

## Prove

`cargo test -p podvm` exits 0 with a test of the exec
environment of a container with a `:ro` volume, and
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh`
exits 0 with a clause where `exec` into a container with a
`:ro` volume fails a write there with EROFS and leaves no file
in the host directory.

## Correction

**Filed 2026-11-11.** From PodUtils/podrunner#62, reproduced the same day.

## Done

**Done 2026-11-11.** `cargo test -p podvm` exits 0 with
`an_exec_takes_the_read_only_table_of_its_container`, and
`PODVM_356_MIRROR=1 sh experiments/356-no-chroot-rung.sh` exits 0 ([the run](../experiments/results/no-chroot-rung.txt)): in clause 35 `exec` into a container with
`-v DIR:/e:ro` fails its write with exit 1, and DIR has no file.

**Partial, 2026-11-11.** The record of a detached run keeps `PODVM_RO` in its
environment; `exec` now takes it (`lifecycle::volume::container_read_only`,
called in `crates/podvm/src/exec/enter.rs`), drops a caller's own value, and
refuses by name on an entry with no read-only layer. `cargo test -p podvm`
exits 0 with `an_exec_takes_the_read_only_table_of_its_container`. On the
fixture, `exec` of a container with `-v DIR:/e:ro` answers "Read-only file
system" and DIR stays empty. Remaining: the run of experiment 356 with
clause 35.

# T-1745: A Rust call for `run` with a typed request that returns the record of `--report`

**Source:** PodUtils/podcrates#124 (the asks of podswarm to podvm, a message from a
  peer session; PodUtils/podcrates#107 is its tracker), ask V-1
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** L
**Status:** done

## Problem

A podswarm worker runs podvm as a program and parses its CLI. The flags
change at each beta (the ruling "Compatibility", 2026-11-05), so a worker
that parses them can break at each beta.

## Premise

`run --report PATH` writes one JSON record for each run (T-1695). The
steps of `run` are functions of the crate `podvm`, but no public call
takes a typed request and gives that record.

## Approach

A public module of the crate `podvm` with a request type (image, argv,
environment, mounts, limits, boundary) and a call that runs it and gives
the record of `--report`. The CLI of `run` becomes a caller of the same
call.

## Decision

The call is a library call of podvm, not a crate of podcrates: it holds
the ladder and the rungs of podvm. podswarm takes it by git tag.

## Prove

`cargo test -p podvm` exits 0, with a test that starts `true` through the
call and reads its record (exit code 0, the entered rung)

## Correction

**Amended 2026-11-13.** The call is the crate `podvm-api`, not a public
module of the crate `podvm`, and it spawns the podvm binary of the same
version (`docs/decisions.md`, call C20): a run in the caller's process
would make the caller the subreaper of the payload's tree, arm
`PR_SET_PDEATHSIG` on its thread and give the payload its streams. The
CLI of `run` stays the one path of a run; the crate holds the map of a
typed request to its argv, and podvm's parity test holds the map.

## Done

**Done 2026-11-13.** `crates/podvm-api` (one dependency, `serde_json`):
`Run` (image, command, environment, volumes, workdir, labels, ulimits,
timeout, tier, confine, pull) with `check`, `argv`, `job` and `run`; `Job`
(a private directory of mode 0700, the env file of mode 0600, the
`Command` that the caller can change) and `Report` (the record of layout
1; another version is refused). `cargo test -p podvm-api` passes (4
tests). `cargo test -p podvm` passes with
`a_run_through_the_api_gives_its_record` (`tests/api.rs`: an imported
one-file image runs through the call, the record names the rung, the
labels and exit code 0 and holds no value of the environment; a missing
image gives a record with `refused`) and
`each_flag_of_podvm_api_has_a_served_run_row`, which fails with
`--workdir` renamed in the crate.


# T-1747: `--memory`, `--cpus` and `--pids-limit` by cgroup2 where a subtree is delegated

**Source:** PodUtils/podcrates#124 (the asks of podswarm to podvm, a message from a
  peer session; PodUtils/podcrates#107 is its tracker), ask V-3
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** M
**Status:** done

## Problem

`run` refuses the cgroup flags by name. podswarm wants `memory.max` for
its boundaries `sandbox` and `vm`, so that a limit is enforced by the
kernel and not only polled by its keeper. The same limits serve G-12 of
PodUtils/podcrates#36 (podrunner).

## Premise

A user with no root can write a cgroup2 subtree only where systemd or the
host delegated it (`cgroup.subtree_control` and write access to the
directory). The probe of podvm does not report that today.

## Approach

The probe reports a delegated subtree. `run` makes one child cgroup for
each run there, writes `memory.max`, `cpu.max` and `pids.max`, and moves
the payload into it before the exec. With no delegation, each flag stays
refused by name. A run over its memory limit ends with a named reason
(`memory.events`, `oom_kill`).

## Decision

podvm writes no cgroup outside a delegated subtree, and a flag never
becomes a silent no-op.

## Prove

`cargo test -p podvm` exits 0, with a test that a run over `--memory` ends
with the named reason where a subtree is delegated, and a test that each
flag is refused by name where none is

## Correction

**Amended 2026-11-13.** The cloud host cannot prove this entry: its
`/sys/fs/cgroup` is cgroup v1, and its cgroup2 mount
(`/sys/fs/cgroup/unified`) has only the controller `hugetlb`. The Prove
needs a host with a delegated cgroup2 subtree with `memory`, `cpu` and
`pids`, for example a hosted runner of GitHub (cgroup2 with each
controller) in a workflow of this repository.

**Amended 2026-11-13, later.** A second lane: since T-1780 each guest of
the machine tier mounts cgroup2 at `/sys/fs/cgroup` with `cpuset`, `cpu`,
`io`, `memory`, `hugetlb` and `pids` (experiment 447, clause 1), so a
podvm in a guest on the cloud host can hold the Prove as root. The
report of a delegated subtree belongs to the probe of `pod-hostprobe`
(PodUtils/podcrates); ask it there first.

**Amended 2026-11-15.** The ask is COORD-125 (issue 390): a leg of
`pod-hostprobe` that reports a delegated cgroup2 subtree, its path and its
controllers, or the reason for none. podvm keeps the child cgroup of each
run and the read of `memory.events`.

## Done

**Done (2026-11-16).** Run 43767666467 (`kvm.yml`, request 434, `7a6c1edb`), job `cgroup`, experiment 459 in a delegated subtree on a runner with a swap file: clause 1 PASS (`run -d --memory 64m --cpus 0.5 --pids-limit 32` made the leaf `podvm-ci/podvm-eccc2908b1b8` beside the cgroup of the shell, with `memory.max` 67108864, `cpu.max` "50000 100000", `pids.max` 32 and 3 processes); clause 2 PASS (`run --rm --pids-limit 16` of 40 children exits 2 with the line of the failed fork); clause 3 PASS (`run --rm --memory 32m` of a shell that holds 200 MB exits 137 and names "oom_kill 1", now that `memory.swap.max` holds the value of `--memory`); no leaf stayed. PASS.

**Partial (2026-11-15).** podvm takes podcrates `v0.51.0`, whose `pod-hostprobe::cgroup` reports a delegated cgroup2 subtree (COORD-125). `crates/podvm/src/run/cgroup.rs`: early in `prepare` (before any pull), a native `run` or `run -d` with `--memory`, `--cpus` or `--pids-limit` finds the delegated directory, needs only the controllers of the flags given (systemd delegates memory and pids by default), and makes them reach a child of the directory or, where the directory holds a process (the scope of a shell), of its parent; else it refuses by name with exit 125 (no delegation, a cgroup v1 host, `create`, the guest tiers). Right before the payload or its launcher is forked, podvm makes the leaf `podvm-ID`, writes `memory.max`, `cpu.max` and `pids.max`, and moves itself in (`0` to `cgroup.procs`), so the run inherits it; at its exit (`atexit`, only in that process) it names an OOM kill from `memory.events`, moves back, and removes the leaf; each later use removes the empty `podvm-*` leaves of ended detached runs. On the cloud host (cgroup v1 hybrid) `run --memory 64m` refuses: "none: /sys/fs/cgroup is not cgroup2". The test `the_limits_become_the_files_of_the_leaf` holds the values. Remaining: the Prove on a host with a delegated subtree: experiment 459 in the job `cgroup` of `kvm.yml` (dispatch with `only=cgroup`), which makes the layout of a systemd user session on a runner.

**Partial (2026-11-15, later).** Run 43716914244 (`kvm.yml`, `only=cgroup`, request 415) on a runner with a delegated subtree: clause 1 PASS (`run -d --memory 64m --cpus 0.5 --pids-limit 32` made the leaf `podvm-ci/podvm-8b5218a140da` beside the cgroup of the shell, with `memory.max` 67108864, `cpu.max` "50000 100000", `pids.max` 32 and 2 processes in it); clauses 2 to 4 FAIL: a foreground run kept podvm itself in the leaf, so its threads counted against `pids.max` (podvm exited 125) and its memory against `memory.max`, and a leaf stayed. podvm now leaves the leaf right after the payload is spawned (`run::cgroup::leave`, from `take_hold`), so the leaf holds the payload and its children only. Experiment 459 prints the stderr of a failed clause and the processes of a leaf that stayed. Remaining: the rerun of the job.

**Partial (2026-11-15, rerun).** Run 43726504386 (`kvm.yml`, `only=cgroup`, request 418, beta.23): clause 1 PASS again; clause 3 PASS (`--memory 16m` with a payload over it: exit 137, "oom_kill 2" named); no leaf stayed. Clause 2 FAIL: a foreground `run --pids-limit 32` made its leaf under the cgroup of the shell (`podvm-ci/shell/podvm-*`) and the move into it gave EOPNOTSUPP. `pids` and `cpu` enable on a cgroup that holds a process, so the step "the directory, else its parent" took the directory of the shell. `limits` now skips a directory that holds a process: podvm's own cgroup, or one whose `cgroup.procs` names a process. Remaining: the rerun of the job.

**Partial (2026-11-15, second rerun).** Run 43740331439 (`kvm.yml`, request 432, `283e3492`): clause 1 PASS; clause 2 PASS now (`--pids-limit 16`: exit 2 with the line of the failed fork); clause 3 FAIL: `run --rm --memory 32m` of a shell that holds 200 MB exited 0 with nothing on stderr, where run 43726504386 gave 137 and named the OOM kill. The cause is not known yet: the change of the parent does not touch a `--memory` run (its leaf goes to the parent in both runs). Experiment 459 now makes the payload name its cgroup and its size on stderr, and prints `/proc/swaps` on a failure. Remaining: the next run of the job.

**Partial (2026-11-15, swap).** Run 43754060132 (request 433): clauses 1 and 2 PASS; clause 3 FAIL, now with the cause: the payload ran in the leaf (`0::/podvm-ci/podvm-2fb8e72367f2`) and held all 200 000 000 bytes under `memory.max` 32 MiB, as the runner has a swap file of 3 GiB (`/swapfile`) and podvm left `memory.swap.max` unbounded. Run 43726504386 passed it by chance of the runner. podvm now writes `memory.swap.max` with the value of `--memory`, as docker's default (`--memory-swap` twice `--memory`, so swap as large as the memory), where the kernel has the file. Remaining: the next run of the job.

# T-1752: A versioned JSON Schema for the record of `run --report`, published in each release

**Source:** PodUtils/podcrates#124 (the asks of podswarm to podvm, a message from a
  peer session; PodUtils/podcrates#107 is its tracker), ask V-8
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** M
**Status:** done

## Problem

podswarm reads the record of `run --report` and the JSON of `probe
--json`, and must refuse a version that it does not know. Neither record
names a schema version, and no schema is in the release.

## Premise

T-1714 holds the half of `probe --json`: it waits for the schema of
`pod-hostprobe` (podcrates@T-107). The record of `--report` is podvm's own
(T-1695).

## Approach

A schema file of the record of `--report` in the tree, a field
`schema_version` in each record, and the schema file as an asset of
`release.yml`. A test validates a record against the schema.

## Decision

The half of `probe --json` stays T-1714.

## Prove

`cargo test -p podvm` exits 0, with a test that each record of `run
--report` names its schema version and validates against the schema file
of the tree

## Done

**Done 2026-11-13.** The record of `run --report` named its layout
version already (`schema`, `SCHEMA` = 1, T-1695).
`crates/podvm/schemas/run-report.v1.schema.json` is the JSON Schema
(draft 2020-12) of version 1: each key required, no other key, the type
of each value, and `schema` the constant 1. `cargo test -p podvm` exits 0
with
`each_record_validates_against_the_schema_of_its_version`: the records of
a run, a refusal and a detached run have the keys of the schema and the
type of each property; with a key removed from the schema the test fails.
The x86_64 leg of `nightly.yml` stages the file as the asset
`podvm-run-report.v1.schema.json` with its `.sha256`, and the publish job
signs it with the binaries; the first release after this commit carries
it. The half of `probe --json` stays T-1714.


# T-1757: `-u` is dropped on the namespace rung when the image has an ENTRYPOINT: the payload runs as root

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the QA pass of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`run -u 1000 node:22-slim id -u` gives 0 on the namespace rung, with no message. With `--entrypoint id` it gives 1000.

## Premise

The path of the image's ENTRYPOINT (`docker-entrypoint.sh`) loses the user of `-u`.

## Approach

Apply `-u` to the argv that the ENTRYPOINT makes, on each rung; a test of the plan with an ENTRYPOINT and `-u`.

## Decision

The fix in the CLI is podvm's; a cause in a crate of podcrates goes there as a COORD issue.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `run.user-entrypoint.ns`, `run.user-node.ns`, and a test of the CLI holds the fix

## Done

**Done 2026-11-13.** `classify` of `crates/podvm/src/interpose.rs` follows
the `#!` line of the first program (`interpose/shebang.rs`), so `-u` reaches
a payload whose ENTRYPOINT is a script; where `-u` cannot reach the
payload, the run refuses at 125. The rerun of experiment 443 ([the run](../experiments/results/qa-surface.txt))
gives PASS for `run.user-entrypoint.ns` and `run.user-node.ns`.

# T-1760: `-t` gives no terminal on the namespace rung, and says nothing

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the QA pass of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

`script -qc 'podvm run --rm -t alpine tty'` prints `not a tty` with exit 0 on the namespace rung; with no terminal `-t` exits 1 with no message on both rungs.

## Premise

The namespace rung does not allocate a pty for `-t`.

## Approach

Allocate the pty on each rung that runs `-t`, and refuse by name where stdin is not a terminal, as docker does.

## Decision

The fix in the CLI is podvm's; a cause in a crate of podcrates goes there as a COORD issue.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `run.tty.ns`, `run.tty-term.ns`, `run.tty.ul`, and a test of the CLI holds the fix

## Done

**Done 2026-11-13.** podcrates `v0.38.0` (PodUtils/podcrates#136, its
podcrates@T-504) binds the host's `/dev/pts` on the `/dev/pts` of the image
in the namespace rung, where that directory exists, and podvm's completion
now makes `dev/pts` (mode 755, the row `dev-pts` of
`crates/podvm-complete/src/shm.rs`). On the namespace rung of the cloud
host, `script -qc "podvm run --rm -t alpine tty"` prints `/dev/pts/1`; the
test `dev_shm_is_made_once_and_never_through_a_link` holds the directory.

**Partial 2026-11-13; the rest waited for podcrates.** A foreground `run
-t` opens a pty of the host's `/dev/ptmx` (`crates/podvm/src/run/pty.rs`),
hands its slave to the payload as descriptors 0, 1 and 2, and relays the
master to podvm's stdout and podvm's stdin to the master. Where podvm's
stdin is a terminal, the pty takes its window size and that terminal is
raw for the run (the `Raw` guard of the machine tier). The relay drains
for at most 2 s after the payload exits. By hand on the cloud host, with
no terminal: `run --rm -t alpine sh -c 'test -t 1 && echo tty; test -t 0
&& echo in'` prints `tty` and `in` on both rungs, and under `script` the
userland rung's `tty` prints `/dev/pts/1`. The test
`a_pty_gives_a_terminal_to_its_slave` holds the pty.

Open: `run.tty-term.ns`. On the namespace rung `ttyname` finds no name:
the root has no /proc and no /dev/pts, which are mounts of `pod-enter`.
COORD-073 (issue 296) asks for both. The slave is not the controlling
terminal: `pod-enter` makes no session, so job control of an interactive
shell does not work; `exec -t` keeps the terminal of the caller.

# T-1761: `-v`, `--tmpfs` and `--read-only` are refused on the namespace rung with a message that names the chroot rung

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the QA pass of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

On the namespace rung, which can mount, `run -v HOST:GUEST`, `:ro`, `--tmpfs` and `--read-only` exit 125: "this host permits chroot(2), so podvm enters the chroot rung ... podvm cannot mount(2)". `--tmpfs` names its internal path of the store in the error.

## Premise

`crates/podvm/src/lifecycle/volume.rs` decides on chroot and not on the entered rung.

## Approach

Bind-mount volumes (read-only for `:ro`), mount the tmpfs and remount the root read-only in the namespace rung; decide the refusal on the entered rung.

## Decision

The fix in the CLI is podvm's; a cause in a crate of podcrates goes there as a COORD issue.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `run.vol.ns`, `run.vol-ro-read.ns`, `run.vol-ro-write.ns`, `run.vol-file.ns`, `run.vol-ls.ns`, `run.vol-wd.ns`, `run.vol-deb.ns`, `run.vol-ro-write-deb.ns`, `run.tmpfs.ns`, `run.tmpfs-tmp.ns`, `run.readonly.ns`, `run.readonly-tmpfs.ns`, `run.readonly-deb.ns`, `run.confine-vol.ns`, and a test of the CLI holds the fix

## Done

**Done 2026-11-13.** A run with `-v`, `--tmpfs` or `--read-only` asks
for the userland loader family also where chroot(2) works
(`decide_entry_prefer` in `crates/podvm/src/lifecycle/entry.rs`, called
from `crates/podvm/src/run/prepare.rs`): its path table serves each
volume and the read-only layer with no mount(2). A payload that the loader
family cannot take enters the chroot rung as before; podvm then prints the
reason of the loader family and the refusal of the volume, which no longer
says that chroot is denied. `PODVM_443_ONLY=run sh
experiments/443-qa-surface.sh` gives PASS for each of the 14 rows of the
Prove on the namespace rung; the full capture comes with the rerun of
experiment 443 before the release. `cargo test --release -p podvm --test
volumes_default_rung` passes: the test imports an image of the host's
`cat`, `tee` and glibc, and runs `-v`, `:ro`, `--tmpfs` and `--read-only`
on the default rung, with no network.

# T-1765: `-e NAME` with no value does not pass the host's variable, and `exec` does not take the environment of `run`

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the QA pass of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`FOO=x podvm run -e FOO alpine sh -c 'echo $FOO'` prints nothing; `exec` in a container of `run -d -e RV=v` does not see RV; on the namespace rung `exec` does not see the container's /tmp.

## Premise

The parse keeps `-e NAME` with no value as nothing; `exec` builds its environment from the image only and enters a new tmpfs.

## Approach

Pass `-e NAME` from the host's environment; give `exec` the environment of the record; enter the container's /tmp.

## Decision

The fix in the CLI is podvm's; a cause in a crate of podcrates goes there as a COORD issue.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `run.env-pass.ns`, `run.env-pass.ul`, `life.exec-env-run.ns`, `life.exec-env-run.ul`, `life.exec-sees-run-fs.ns`, and a test of the CLI holds the fix

## Done

**Done 2026-11-13.** The last clause came with podcrates `v0.37.0`
(PodUtils/podcrates#134, its podcrates@T-492): `exec` of a running container of the
namespace rung opens a pidfd of the payload and enters through
`spawn_joined`, which joins the payload's mount namespace
(`crates/podvm/src/exec/enter.rs`). A refused join is named by `pod-enter`,
which enters a namespace of its own. On the cloud host, `exec` in a
container of `run -d ... sh -c 'echo fromrun > /tmp/r; sleep 60'` prints
`fromrun`, and /proc is there.

**Partial 2026-11-13.** Done: `-e NAME`
and `--env=NAME` of `run` and `exec` take the caller's value, and an unset
NAME names nothing (`from_host` in `crates/podvm/src/run/parse/values.rs`;
the test `e_name_takes_the_value_of_the_caller`). `exec` of a container
takes the environment of its record without podvm's own variables
(`PODVM_*`, `LD_PRELOAD`), then its own `-e`
(`crates/podvm/src/exec/enter.rs`). On both rungs `FOO=hostval podvm run
-e FOO` prints `hostval`, and `exec -e XV=x` in a container of `run -d -e
RV=v` sees `RV=v XV=x`. `--env-file` keeps its rule of T-0801: a line
without `=` is refused.

Then open: `life.exec-sees-run-fs.ns`. The run's /tmp is a tmpfs in the
run's own mount namespace; COORD-074 (issue 297) asked podcrates for the
joined entry.

# T-1766: `--strict` and `probe` disagree with the rung that a run enters on the userland fixture

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the QA pass of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

On the chroot-denied fixture `run --strict` exits 0 with warnings (experiment 356, clause 6, wants 125), and `probe` names `namespace` (or `supervise`) while each run enters `userland`.

## Premise

The probe's selection and the entered rung of T-1713 are two answers; `--strict` reads the selection.

## Approach

Make `--strict` and the probe's rung line read the entered rung; keep the selection as its own field.

## Decision

The fix in the CLI is podvm's; a cause in a crate of podcrates goes there as a COORD issue.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `run.strict.ul`, `run.strict-all.ul`, `sys.probe.ul`, `sys.probe-strict.ul`, `sys.info-consistent`, and a test of the CLI holds the fix

## Correction

**2026-11-13.** Half of the Problem was a stale expectation. Experiment
356, clause 6 (T-1415) says that `--strict` warns on the userland rung
where chroot is denied, because that rung is the best of the machine, and
that `--strict=all` refuses. The row `run.strict.ul` of experiment 443
wanted 125 from `--strict`; it now wants exit 0 with a warning of the
substitution class, and the new row `run.strict-all.ul` wants 125 from
`--strict=all`. `sys.info-consistent` fails no more: the parity text of
`run` that it read was corrected before this entry.

## Done

**Done 2026-11-13.** `probe` names the rung that `run` enters
(`to_entered` in `crates/podvm/src/system/df_info.rs`, beside
`entered_rung` of T-1713): where the probes select `namespace` or
`supervise` and chroot is denied, stdout, `--json` and `--strict` read
`userland`, and stderr names the selection. On the chroot-denied fixture,
`probe` prints `userland` and `probe --strict` exits 1; on the namespace
rung of the cloud host, `probe` prints `namespace` and `probe --strict`
exits 0. `run --strict` on the fixture exits 0 with 4 warnings of the
substitution class, and `run --strict=all` exits 125. The test
`probe_names_the_entered_rung` holds the rung. The selection of the probes
stays in `pod-hostprobe`, and the namespace rung there does not ask
for chroot; podvm reads it through T-1713, so it asks podcrates for no
change. Experiment 443 runs the five rows again before the release.

# T-1769: An unknown verb or flag prints the whole usage

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the QA pass of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

`podvm frobnicate` and `run --foo` print more than 40 and 150 lines; each error ends with "podvm: invoked as ./podvm"; `profile list` breaks its columns on long names.

## Premise

The refusal prints the usage of the verb in full.

## Approach

Print one line that names the flag or the verb and the help command, and size the columns of `profile list` to the names.

## Decision

The fix in the CLI is podvm's; a cause in a crate of podcrates goes there as a COORD issue.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `sys.unknown-verb-out`, `sys.unknown-flag-short`, `sys.profile-cols`, and a test of the CLI holds the fix

## Done

**Done 2026-11-13.** Three parts:

- An unknown verb prints one line that names `podvm --help` (48 lines
  before), and an unknown flag prints its refusal, the synopsis of the
  verb and `podvm VERB --help` (163 lines for `run` before):
  `short_usage` in `crates/podvm/src/parity/lookup.rs`, used by `admit`
  and `refuse_member`. The line "podvm: invoked as ..." prints only where
  argv0 does not end in `podvm`, its use under another name (T-0803).
- `profile list` sizes its name column to the longest name.
- `names::start` (`crates/podvm/src/names.rs`) puts SIGPIPE back to its
  default at the start: `podvm profile list | head` panicked on the closed
  pipe, and now ends quietly with the status of SIGPIPE, as each Unix tool
  does.

The test `tests/short_refusals.rs` holds the short refusals. Experiment 443
runs the three rows again before the release.

# T-1772: `-w` with a directory that the image lacks is refused; docker creates it

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the rerun of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`podvm run --rm -w /newdir alpine pwd` exits 125: "the working directory
"/newdir" ... is not a directory in the image: docker creates it, and the
entry refuses it instead". docker prints `/newdir`.

## Premise

T-1412 chose to refuse a missing working directory, so that a payload
never starts in the wrong directory. Each run has a private rootfs, so
podvm can make the directory there before the check, as docker does.

## Approach

Before `check_workdir`, make each missing component of an absolute working
directory in the rootfs, following no link; leave a path in a volume to
the volume. A file or a link of the image on the way still refuses.

## Decision

The refusal of T-1412 stays for a path that escapes, loops, or is a file.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `run.workdir-new.ns` and `run.workdir-new.ul`, and a test of the CLI holds the fix

## Done

**Done 2026-11-13.** `make_workdir` in `crates/podvm/src/lifecycle/volume.rs`
runs before `check_workdir` in `crates/podvm/src/run/prepare.rs`: it makes
each missing component of an absolute working directory in the rootfs,
follows no link, refuses a `.` or `..` component before it makes anything,
and leaves a path in a volume to the volume. `run --rm -w /newdir alpine
pwd` prints `/newdir` on the namespace rung and on the chroot-denied
fixture. The test `a_missing_workdir_is_made_and_never_through_a_link`
holds it; its first form found that `/a/../b` made `a`, and the walk now
reads each component first.

# T-1775: `cp` into a running container of the namespace rung writes under the run's tmpfs

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the second rerun of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

After T-1765, `podvm cp FILE NAME:/tmp/F && podvm exec NAME cat /tmp/F`
fails on the namespace rung: `exec` joins the payload's mount namespace,
where /tmp is the run's tmpfs, and `cp` wrote into the /tmp of the rootfs
under it. The row `life.cp-in.ns` passed before T-1765.

## Premise

The namespace rung mounts its tmpfs on the host path of the rootfs's /tmp,
inside the payload's mount namespace, so a process that joins that
namespace sees the tmpfs at the same host path.

## Approach

`cp` joins the payload's mount namespace before the copy, as `exec` does;
the local path is made absolute first, because `setns` moves the working
directory.

## Decision

A refused join copies into the rootfs, as before T-1765.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the row `life.cp-in.ns`, and a test of the CLI holds the fix

## Done

**Done 2026-11-13.** `copy_named` in `crates/podvm/src/lifecycle/wait_rm_cp.rs`
calls `join_mounts` (`crates/podvm/src/exec/enter.rs`: a pidfd and
`setns(CLONE_NEWNS)`) for a running container of the namespace rung, after
it makes the local path absolute. On the cloud host, `cp cpin.txt
qa-c:/tmp/cpin.txt` (a relative local path) then `exec qa-c cat
/tmp/cpin.txt` prints `cpin`, and `cp qa-c:/tmp/r` copies the file that the
run wrote to its /tmp. `PODVM_443_ONLY=life sh
experiments/443-qa-surface.sh` on this binary gives PASS for each of its 76
rows, `life.cp-in.ns` and `life.exec-sees-run-fs.ns` among them.


# T-1783: Nine verbs print a usage error to stdout

**Source:** PodUtils/podvm#334 (a report of Nemo-010 on `v0.2.0-beta.14`)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`save`, `import`, `start`, `logs`, `stop`, `kill`, `restart`, `wait` and
`rm` printed their usage to stdout when an argument was missing, while
sixteen other verbs used stderr. `podvm save > image.tar` with no image
wrote 415 bytes of usage into the file, which `file` named ASCII text.

## Premise

The `--help` branch and the missing-argument branch of each verb were
two prints of the same text.

## Approach

A usage error goes to stderr with its exit code; `--help` stays on
stdout with exit 0.

## Decision

One rule for each verb, as the sixteen others already do.

## Prove

`cargo test -p podvm --test usage_stream` exits 0: each of the nine verbs with no argument leaves stdout empty and names the usage on stderr, and `--help` writes it to stdout with exit 0

## Done

**Done 2026-11-14.** `save_load.rs`, `lifecycle.rs`, `logs_stop.rs` and
`wait_rm_cp.rs` print the missing-argument usage with `eprint!`; `wait`
now splits its empty case from its `--help`. The test
`a_usage_error_leaves_stdout_empty_and_help_writes_it` passes; by hand,
each of the nine prints 0 bytes to stdout with no argument and the same
usage to stdout with `--help`.


# T-1801: `--report` after the image is read as podvm's, so a payload argument overwrites a host file

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), F-02 (CODE-02; verify item 4a)
**Category:** cli
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

`podvm run alpine:3.20 true --report $PWD/important.txt` exits 0 and
replaces `important.txt` with the JSON report; the payload also gets
`--report`. `podvm run --name --report alpine:3.20 cmd` writes a report
to a file named `alpine:3.20` and names the container `--report`. The
help of `run` says that parsing stops at the image name.

## Premise

`path_in` (`crates/podvm/src/run/report.rs:178`) scans each argument up to
`--`, the payload's included, before the parser, and does not know which
flags take a value. `run` calls it first
(`crates/podvm/src/run/foreground.rs:119`). `Report::write` renames over
the target.

## Approach

Delete the scan. The parser of `run` returns the report path in `Opts`,
also for a refusal after the parse (a `Result` that carries the path).
A refusal before the parser knows the path writes no report.

## Decision

One parser decides which words are podvm's. The QA patch 0002 stops the
scan at the image instead; that keeps two parsers, so this entry does
not take it as is.

## Prove

`cargo test -p podvm --bin podvm report_path` exits 0, with tests that `run IMG true --report F` and `run --name --report IMG cmd` write no file F and no file named after the image, and that `run --report F IMG true` writes F

## Done

**Done 2026-11-14.** `run/report.rs` has no scan before the parse now:
the parser of `run` notes the path when it reads `--report` or
`--report=` (`note_path`), and `run` takes it at the end (`take_path`).
The test `report_path_is_the_parsers` gives no path for
`IMG true --report F`, for `--name --report IMG cmd` and after `--`,
and the path for `--report F IMG` and for a refusal after the flag. By
hand with the built binary: the payload got `--report PATH` as its
argument and `important.txt` kept its text; `--name --report IMG true`
named the container and wrote no file; `--report F IMG true` wrote F;
`--report F --timeout abc IMG true` exited 125 and F held
`"refused": 125`. Experiment 426 passes its 4 clauses. Experiment 443
has the row `run.report-payload` on both rungs.


# T-1802: A run refused before `prepare` writes the `-e` values into its report

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), F-03 (CODE-03, RUNS-03 first half; verify item 4b)
**Category:** cli
**Milestone:** none
**Priority:** P0
**Effort:** S
**Status:** done

## Problem

`podvm run --report leak3.json --pull never -e API_TOKEN=s3cr3t nosuchimage:1 true`
exits 125 and the report holds
`"argv": ["--report", "leak3.json", "--pull", "never", "-e", "API_TOKEN=s3cr3t", ...]`.
The same podvm argv lands in the report of a flag error, an unpulled
image and a payload the entry cannot run. The schema says `argv` is the
command of the payload, and the help and `run/report.rs` say the report
holds no value of an environment variable.

## Premise

`Report::begin` (`crates/podvm/src/run/report.rs:60`) keeps the raw argv
of podvm; only `prepared` (`crates/podvm/src/run/report.rs:93`, called at
`crates/podvm/src/run/foreground.rs:145`) replaces it, and a refusal
never reaches it.

## Approach

`begin` keeps no argv. `argv` is the payload command from the parse when
the parse reached the image, and `null` before that. No field of the
report holds a value of `-e`, `--env` or `--env-file`.

## Decision

The schema already allows `null` (`podvm-run-report.v1.schema.json`).
Redaction as `NAME=<redacted>` (the QA patch) keeps podvm's flags in a
field that the schema defines as the payload's; this entry does not do
that.

## Prove

`cargo test -p podvm --bin podvm report_refusal` exits 0, with a test that a refused run with `-e K=canary-value` writes a report in which the bytes `canary-value` do not occur and `argv` is the payload command or `null`

## Done

**Done 2026-11-14.** `Report::begin` keeps no argv now. The parse of
`run` notes the image and the command of the payload (`note_parsed`),
and `run` puts them into the report of a run that a later step refused
(`Report::parsed`); `prepared` still gives them for each run that
prepared. A correction of the Premise: the schema
`podvm-run-report.v1.schema.json` declares `argv` an array with no
`null`, so a report written before the parse reached the image has
`"argv": []`, which keeps layout 1. The test
`report_refusal_holds_no_value_of_e` writes the report of a refused run
with `-e API_TOKEN=canary-value`, before and after the parse reached the
image: no `canary-value` in either, and `argv` is `["true"]` and `[]`.
By hand, the command of the report: exit 125, no `s3cr3t` in the file,
`argv` `["true"]`, the image named and `refused` 125. Experiment 426
passes; experiment 443 has the row `run.report-refused-env`.


# T-1811: `--podvm-net=V` and `--podvm-channel=V` are refused as "a bug in podvm", and so is `--podvm-tier` on `start`, `ps` and `rm`

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), F-12 (MACH-06; verify item 9)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`podvm run --podvm-tier=machine --podvm-net=none alpine:3.20 true` exits
125 with "\"--podvm-net=none\" is in the parity table and this parser has
no arm for it. That is a bug in podvm (TODO/cli.md T-0801)". The same for
`--podvm-channel=...`, and for `podvm start --podvm-tier=machine nosuch`,
`ps --podvm-tier=machine` and `rm --podvm-tier=machine`. The forms with a
space work, and `--podvm-mem=128M` works.

## Premise

`crates/podvm/src/run/parse.rs:140` and `:147` match `--podvm-net` and
`--podvm-channel` only as a separate word; the `=` arms exist for tier,
mem, cpus, qemu-arg and confine (`crates/podvm/src/run/parse.rs:396`).
The lifecycle verbs admit the parity row of `--podvm-tier` and have no
arm for it.

## Approach

Split `--flag=value` once, at admission, for each flag that takes a value,
so that no arm needs its own `=` form. Give `start`, `ps` and `rm` an arm
for `--podvm-tier` that serves the tiers of T-1722 and refuses the others
by name, as a usage error.

## Decision

The message "a bug in podvm" stays only for a row that no parser admits.
T-1853 adds the check that each row takes both forms.

## Prove

`cargo test -p podvm --bin podvm eq_form` exits 0, with a test that `--podvm-net=none` and `--podvm-channel=P:H:N` parse as their space forms, and that `start --podvm-tier=machine X` gives a named usage refusal and not the T-0801 text

## Done

**Done 2026-11-14.** `run/parse.rs` takes `--podvm-net=none`,
`--podvm-net=V` (refused by name) and `--podvm-channel=V` as the space
forms, and `incus::intercept` refuses another tier than `incus` on `ps`,
`start`, `stop`, `rm`, `logs` and `inspect` by name with 125
(`other_tier`). Tests: `eq_form_of_the_machine_flags_parses_as_the_space_form`
and `eq_form_of_another_tier_on_a_lifecycle_verb_is_refused_by_name`. By
hand: `run --podvm-tier=machine --podvm-net=none` goes past the flag;
`start`, `ps` and `rm` with `--podvm-tier=machine` print "takes
--podvm-tier=incus only" and no T-0801 text. The general split of each
`--flag=value` at admission is T-1853.


# T-1815: `run -d -t` and `create -t` accept `-t` and never allocate a terminal

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), F-16 (CODE-07; verify item 16)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`podvm run -d -t --name ttyd alpine:3.20 sh -c 'test -t 1 && echo TTY || echo NOTTY'`
exits 0, and `logs ttyd` prints `NOTTY`. No line says that `-t` was
dropped, and `inspect` records nothing. Where `/dev/ptmx` is unusable,
the same command is refused for a terminal that it would not use.

## Premise

The detached branch (`crates/podvm/src/run/foreground.rs:151`) never reads
`p.tty`; only the foreground branch does (`crates/podvm/src/run/foreground.rs:263`).
`create` takes the option set of `run`.

## Approach

Refuse `-t` with `-d` and on `create` by name, before the check of
`/dev/ptmx`, as `--tmpfs` is refused for a detached run.

## Decision

A detached terminal needs the launcher to hold the master; that is a
separate feature with its own entry if a user asks. An accepted flag
never does nothing (T-1421).

## Prove

`cargo test -p podvm --bin podvm detach_tty` exits 0, with tests that `run -d -t` and `create -t` exit 125 and name `-t`

## Done

**Done 2026-11-14.** `detached_tty` (`run/parse/values.rs`), called
by `parse` after each parse, refuses `-t` with `-d` and on `create` by
name with 125, before any check of `/dev/ptmx`. The test
`detach_tty_is_refused_by_name` refuses `run -d -t`, `run -dt` and
`create -t`, and keeps `run -t` a terminal. No experiment uses the pair.


# T-1816: A failed write to stdout aborts with SIGABRT, `images` exits 0 with its table lost, and `logs` exits 0 having written nothing

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), F-17 (CODE-09), CODE-08, MISC-07
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

`podvm version >/dev/full`, `probe`, `doctor`, `inspect IMG` and `ps` to
`/dev/full` exit 134 with "failed printing to stdout: No space left on
device" (measured again on `main` for `version`, 2026-11-14).
`podvm images >/dev/full` and `podvm logs NAME >/dev/full` exit 0 with no
byte written and no message (`images` measured on `main`). A refusal on a
full disk with an unwritable stderr also aborts.

## Premise

The print macros panic on a write error, and the release profile aborts
(`Cargo.toml`, `panic = "abort"`). `logs` discards the error
(`crates/podvm/src/lifecycle/logs_stop.rs:170`, `let _ = out.write_all`);
`images` discards the error of its writer.

## Approach

Each verb writes its answer through one checked writer at the verb
boundary. A write or flush error prints `podvm VERB: stdout: ERRNO` on
stderr (when stderr can take it) and exits 125. A failed write to stderr
does not change the exit code of the verb. T-1855 adds the scan for
`println!` in a verb and for a discarded write result.

## Decision

`panic = "abort"` stays: a panic is a defect, and a full disk is not a
panic.

## Prove

`cargo test -p podvm --test stdout_full` exits 0, with a test for `version`, `ps`, `images`, `inspect`, `probe` and `logs` with stdout on `/dev/full` (exit 125, the errno named, no SIGABRT)

## Done

**Done 2026-11-14.** `names/stdout.rs`: a panic hook, which `names::start`
installs, turns the panic of a print macro on stdout into exit 125 and
the line `podvm VERB: stdout: ERROR` before the abort runs; `exit` of
`main.rs` flushes stdout and gives 125 where the flush fails; `logs` and
`images` check their writes and their flush. A failed write is reported
once. The test `stdout_full_is_125_with_the_errno_named` runs `version`,
`ps -a`, `images` and `probe` with stdout on `/dev/full`: exit 125 and
one line with the errno each. By hand with the built binary, also
`inspect IMG`, `doctor`, `images --format` and `logs NAME`: 125 and one
line each; beta.15 aborted with 134 or exited 0. A deviation from the
Approach: a print macro that fails on stderr exits 125 with no line,
because the hook cannot return to the verb and stderr takes no line;
the code of the verb is lost there. T-1855 adds the scan of the source.


# T-1817: `run -t` never delivers end of input, so a payload that reads to the end blocks

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), F-18 (RUNS-27), RUNS-11 (disputed), MACH-13 item 5
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`timeout 5 podvm run -t alpine:3.20 cat </dev/null` ends at the outer
timeout (124), and so does `printf 'input\n' | podvm run -t alpine:3.20 cat`
after it prints `input`. A single `read` returns. The machine relay has
the same shape (not measured). RUNS-11 reports a NUL byte before the
first byte of a `-t` payload under `script -qec`, with a control that
shows no NUL for `script -qec "echo hi"`; the report refutes it as a
narration of `script`, and MACH-13 sees the same `^@` on the machine
console. The claim is disputed and not settled.

## Premise

The stdin thread of the relay (`crates/podvm/src/run/pty.rs`, the loop
after the master is cloned) drops its write clone on EOF, and the read
thread keeps another clone of the same master, so the slave never reads
0. The machine relay drops `w` and keeps `conn`
(`crates/podvm/src/machine/guest/tty.rs`).

## Approach

On EOF of stdin, write the VEOF character of the terminal (read from
termios, `0x04` by default) to the master, as the QA patch 0006 does; do
the same in the machine relay. Add a byte-exact test of the output of
`-t` with stdin from `/dev/null`, which settles the NUL claim.

## Decision

A pty has no half-close; VEOF is the terminal's own end of input.

## Prove

`cargo test -p podvm --test pty_eof` exits 0, with a test that `run -t IMG cat` ends with stdin from `/dev/null` and from a closed pipe, and that the output of `run -t IMG echo hi` is exactly `hi` and CR LF

## Correction

**Amended, 2026-11-14.** The test target `pty_eof` of the Prove was not
made: the test is the unit test `veof_ends_the_input_of_the_slave` of
`run/pty.rs`, on a real pty.

## Done

**Done 2026-11-14.** At the end of podvm's stdin, the relay of `-t`
(`run/pty.rs`) writes VEOF to the master, and one more after a partial
line; the relay of the machine tier (`machine/guest/tty.rs`) does the
same on its socket. The test `veof_ends_the_input_of_the_slave` checks
both cases on a real pty. By hand on the namespace rung: `run -t cat
</dev/null` ends at once with exit 0 (beta.17: the outer timeout, 124),
`printf 'input\n' | run -t cat` prints the line and ends, and a partial
line ends after `cat` prints it. The NUL claim: `run -t cat </dev/null`
writes 0 bytes, and `run -t echo hi` writes exactly `h i \r \n`; no NUL
on this rung. On the machine tier, `-t` with piped input ends in 3 s
(beta.17: 400 s, the outer timeout); the line itself does not come
back, which beta.17 did not do either: T-1885.


# T-1824: The machine notice ignores `-q`, and the banner is said to print "on every run" where runs are quiet by default

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), RUNS-14, MISC-10, benchmark P1 (`benchmarks/compare.md`, part C)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`podvm run --podvm-tier=machine -q alpine:3.20 echo qtest` still prints
"podvm run: machine tier: a guest under KVM ..." on stderr; `-q` is
documented as forcing silence. `system info --format '{{.Banner}}'` says
"printed on every run and exec", and the parity note of `run` says "The
banner names what the selected rung does not provide, on every run",
while a default run prints 0 bytes on stderr and the help of `-q` says
"Runs are already quiet by default". The benchmark report asks for one
line on the default path that names the rung and that it gives no
isolation.

## Premise

`crates/podvm/src/machine/guest.rs:347` prints the notice with no test of
the banner switch. The texts are `crates/podvm/src/system.rs:114` and
`crates/podvm/src/parity.rs:121`.

## Approach

Gate the machine notice on the same switch as the entry banner. Make the
two texts say "with `--verbose`". Write the question of a default line
for the owner in `docs/decisions.md`, with the recommendation: one line
on stderr only where the entered rung gives no isolation and stdout or
stderr is a terminal.

## Decision

Quiet by default stays until the owner answers; the texts must say what
the binary does now.

## Prove

`cargo test -p podvm --bin podvm banner_quiet` exits 0, with a test that the machine notice is not printed under `-q`, and `podvm system info --format '{{.Banner}}'` names `--verbose`

## Done

**Done 2026-11-15.** `machine/guest.rs` prints the notice of the machine
tier through `notice`, which returns nothing where the banner switch is
off (`complete::banner_loud`: `-q`, the store config, or no `--verbose`).
The texts of `system info` (`.Banner`), of the parity row of `run`, of
the help of `run` and of `exec.rs` say "with `--verbose`". The question
of a default line is Q8 of `docs/decisions.md`. `cargo test -p podvm
--bin podvm banner_quiet` exits 0, and `podvm system info --format
'{{.Banner}}'` prints "printed on a run and an exec with --verbose; -q
forces it off".


# T-1825: A `--report` that cannot be written exits 0, and the report of a detached run is never finished

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), RUNS-15, RUNS-16
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

`podvm run --report /no/such/dir/rep.json alpine:3.20 true` prints "the
report could not be written" and exits 0, so a caller that reads the exit
code believes it has a report. `podvm run -d --name rptto --timeout 1 --report r.json alpine:3.20 sleep 30`
ends `Exited (137)`, and the report keeps `exit_code: null`,
`signal: null`, `timed_out: false` and `ended` equal to `started`.

## Premise

`crates/podvm/src/run/foreground.rs:126` prints the write error as a
notice. The report is written once, before the launcher returns; the
launcher of a detached run never rewrites it.

## Approach

Check that the directory of the report path is writable before the run,
and refuse with 125 by name if not; a write that fails after the run
exits 125 with the payload code named on stderr. The launcher of a
detached run rewrites the report at the end of the payload with the
fields of the end, by the same atomic rename.

## Decision

The report is one document of the run (T-1695), so a detached run gets
its end; a start record is not a report.

## Prove

`cargo test -p podvm --test report_paths` exits 0, with a test that an unwritable report directory exits 125 before the payload runs, and a test that a detached run with `--timeout 1` leaves a report with `timed_out: true` and `signal: 9` after `wait`

## Correction

**2026-11-15.** The Prove names one test target, `report_paths`, for
both clauses. The clause of the detached run needs the launcher of
`podvm-supervise` to write the report, so its test comes with that
change.

## Done

**Done 2026-11-15.** The record of a detached run keeps the absolute
path of its report (`Hardening::report`). `pod_supervise` drops
`Payload::keep` after it wrote the end into the record, so a guard in it
(`launcher.rs`, `Finish`) writes the end into the report:
`podvm_supervise::report::finish` sets `exit_code`, `signal`,
`timed_out`, `ended` and the times, by an atomic rename. `run -d` calls
it again after its own write of the start, so a payload that ended first
gets its end too. `cargo test -p podvm --test report_paths` exits 0: the
unwritable directory (125, before the run), and `run -d --timeout 1`,
after `wait`, gives `timed_out: true`, `signal: 9` and `exit_code: 137`.
By hand on the cloud host: five `run -d ... sh -c 'exit N'` reports hold
N and an end time.

**Partial 2026-11-15.** The foreground half: `--report PATH` is refused
with 125 at the parse when the directory of a new PATH takes no file, or
an existing PATH does not open for append (`run/parse/values.rs`,
`report_path`), before an image is fetched and before the payload runs;
a write that fails after the run exits 125 and names the code of the
payload (`run/foreground.rs`). `cargo test -p podvm --test report_paths`
exits 0 (`/nonexistent/podvm/r.json`: 125, the image never reached). By
hand: `run --report /no/such/dir/rep.json alpine:3.20 sh -c 'echo ran'`
exits 125 and prints no `ran`; a report into a writable directory or an
existing file is written. The clause that remains: the launcher of a
detached run rewrites the report at the end of the payload, with
`timed_out` and `signal`.

# T-1827: A repeated `--ulimit` that raises a hard limit, `--ulimit nofile=-1` and a hard value over podvm's own all fail with a bare EPERM

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), RUNS-06, RUNS-07, RUNS-26
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`--ulimit nofile=64 --ulimit nofile=128` exits 125 with
"setrlimit(RLIMIT_NOFILE): EPERM (1)"; the reverse order prints 64, and
podman takes the last value (128). `--ulimit nofile=-1` gives the same
EPERM as uid 1000 and as root on the namespace rung, although the help
says `-1` means no limit. `--ulimit nofile=64:1048577` over a hard limit
of 1048576 gives the bare EPERM; the parity note says such a value "is
refused by name".

## Premise

`pod-enter` applies each limit in order in the child (podcrates
v0.42.0, `src/launch.rs`), so the first call lowers the hard limit and
the second cannot raise it. `RLIM_INFINITY` over a finite hard limit
needs `CAP_SYS_RESOURCE`. podvm parses the values in
`crates/podvm/src/run/parse/values.rs:19` and checks no ceiling.

## Approach

In podvm: keep one value for each resource, the last one, before the
plan reaches `pod-enter`. Read the hard limit of podvm (`prlimit`) and
refuse a hard value over it by name with both numbers. Take `-1` as "the
hard limit that podvm has" when podvm cannot raise it, and say so on
stderr under `--verbose`. podvm asks podcrates to refuse a list with two
entries for one resource (proposed COORD issue).

## Decision

Docker and podman keep the last value for a resource. `-1` that cannot
be infinite takes the inherited hard limit, which is the largest value
the payload can have.

## Prove

`cargo test -p podvm --bin podvm ulimit_plan` exits 0, with tests that two `nofile` values leave the last, that `-1` resolves to the inherited hard limit, and that a hard value over it is refused with both numbers

## Done

**Done 2026-11-14.** `run/parse/values.rs` (`ulimit_plan`, `ulimits_of`)
keeps the last value of each resource before the plan (podcrates
`v0.45.0` refuses two limits of one resource), gives `-1` the hard limit
that podvm has where it is finite, and refuses a hard value over it as a
flag error with both numbers. `cargo test -p podvm --bin podvm
ulimit_plan` exits 0. By hand with `ulimit -Hn` 20000:
`--ulimit nofile=64 --ulimit nofile=128` gives 128 and the reverse 64;
`nofile=-1` gives 20000; `nofile=64:99999999` exits 125 with "the hard
value 99999999 is over the hard limit 20000 of RLIMIT_NOFILE".

# T-1828: `--add-host` takes a non-address, `-e =V` is dropped, `--env-file` removes quotes, and a repeated `--label` key keeps both values

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), RUNS-08, RUNS-09, RUNS-10, RUNS-19, `findings/code.md` "Not covered" (an `--add-host` with a newline)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`podvm run --add-host foo:notanip alpine:3.20 cat /etc/hosts` writes
`notanip foo`; podman refuses "invalid IP address in add-host". An
`--add-host` value with a newline or a space makes the whole hosts fixup
fail, so the name of the container is missing from `/etc/hosts`, exit 0,
visible only under `--verbose`. `-e =V` exits 0 and sets nothing; docker
and podman refuse it. `--env-file` with `B="two words"` gives
`B=[two words]`; docker and podman keep the quotes. `--label a=b --label a=c`
keeps both; podman keeps `a=c`.

## Premise

`add_host` (`crates/podvm/src/complete/strict.rs` then; `complete/add_host.rs` now) checks only for a
`:`. `from_host` (`crates/podvm/src/run/parse/values.rs:113`) takes each
string with a `=`, an empty name too. The quote removal is at
`crates/podvm/src/run/parse/values.rs:171`. `label`
(`crates/podvm/src/run/parse/values.rs:196`) keeps each value.

## Approach

Parse the address of `--add-host` as IPv4 or IPv6 and the name as a host
name; refuse anything else with exit 125, naming the value. Refuse an
empty name in `-e` as `--env-file` already does. Keep the value of an
`--env-file` line byte for byte after the trim of the line. Keep the last
value of a repeated label key in the record and the report.

## Decision

docker's behaviour for each of the four, measured by the QA on docker
26.1.5 and podman 5.4.2. The help of `--env-file` names the rule.

## Prove

`cargo test -p podvm --bin podvm run_values` exits 0, with tests for `foo:notanip` and a value with a newline (refused, value named), `-e =V` (refused), `B="two words"` (quotes kept) and `a=b a=c` (one label, `a=c`)

## Done

**Done 2026-11-15.** `--add-host` takes a host name and an IPv4 or IPv6
address (or `host-gateway`), and refuses anything else with exit 125
naming the value (`complete/strict.rs`, `check_add_host`); `-e` and
`--env` refuse an empty name (`env_arg`, `run` and `exec`); `--env-file`
keeps the value of a line byte for byte, quotes too; a repeated
`--label` key keeps its last value (`push_label`). The help of `run` and
the parity note of `--env-file` name the rule. `cargo test -p podvm
--bin podvm run_values` exits 0; the test of `--env-file` of T-0801 now
expects the quotes. By hand with `mirror.gcr.io/library/alpine:3.20`:
`--add-host foo:notanip` and `-e =V` exit 125 with the value named;
`--add-host foo:10.1.2.3` writes `10.1.2.3 foo` into `/etc/hosts`.

# T-1829: Refusals of `run` lead with the wrong cause or name the wrong spelling

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), RUNS-21, RUNS-22, RUNS-24, RUNS-25, MACH-13 item 1, `findings/life.md` coverage row `logs --tail -1`, verify V-1 (the `,` separator)
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

`podvm run` with no image prints the whole help, 11 521 bytes, to stderr
with exit 1; an unknown flag prints one line with exit 125. `podvm run --env`
says "-e needs a value"; docker says `--env`. A missing `--entrypoint`
program on the userland rung leads with "chroot(2) is denied on this
machine" before "names no file in the image". `--security-opt
no-new-privileges=bogus` is refused as if the option had no backend.
`--podvm-confine read=/definitely-not-in-image` names the host path of
the tree. `logs --tail -1` says "unknown option \"-1\"". A confine spec
with `,` in place of `;` is refused as "the path \"exec=/bin\" of read is
not absolute", which does not name the separator.

## Premise

`crates/podvm/src/run/parse.rs:431` prints the flag as the parser stored
it. The texts of the entrypoint, security-opt and confine refusals are in
`crates/podvm/src/run/parse/values.rs` and `crates/podvm/src/lifecycle/confine.rs`.

## Approach

With no image: print the usage line and "an image is needed", exit 1.
Name the spelling that the caller used. Put the image check before the
chroot sentence. For `no-new-privileges=X`, say that X is not `true` or
`false`. Name the guest path in a confine refusal. Name `;` as the
separator when a rule value holds `=`. Say that `--tail` takes a count of
zero or more.

## Decision

Text only; no exit code changes. The exit 1 for no image
matches docker 26.1.5.

## Prove

`cargo test -p podvm --bin podvm refusal_text` exits 0, with a test for each of the seven texts above

## Correction

**Corrected 2026-11-15.** Six of the seven texts come from the binary,
so their test is the integration test `tests/refusal_text.rs`; the text
of the entrypoint on the userland rung needs chroot denied, so its test
is the unit test `lifecycle::tests::script::refusal_text_leads_with_the_missing_program`.

## Done

**Done 2026-11-15.** `run` with no image prints the usage line and "an
image is needed" with exit 1 (`run/parse.rs`, `no_image`). A flag with no
value names the spelling of the caller (`--env needs a value`). A missing
program of the userland rung leads its refusal, before the chroot
sentence (`lifecycle/script.rs`). `no-new-privileges=X` says that X is not
true or false. A confine rule with `=` in its value names `;` as the
separator (`podvm-enter/src/confine.rs`), and a path that the image lacks
is named by its guest path (T-1888). `logs --tail -1` says that `--tail`
takes a non-negative count: `parity::expand` and `admit_all` take the word
after a valued long flag as its value. `cargo test -p podvm --bin podvm
refusal_text` and `cargo test -p podvm --test refusal_text` exit 0.


# T-1830: A `--tmpfs` run leaves its map in the record, so a later `exec` cannot see the path

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), RUNS-05 (tester-reported, not re-run by the report owner)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`podvm run --name tmptmp --tmpfs /tmp alpine:3.20 sh -c 'echo hi > /tmp/x'`
works, and then `podvm exec tmptmp ls -la /tmp` prints
"ls: /tmp: No such file or directory". The same for `--tmpfs /etc`. The
record keeps `PODVM_MAPS=...,/tmp:/.../tmpfs/25983/0`, and the run removed
that directory at its end. A control with `--tmpfs /mnt/x` keeps `/etc`
and `/tmp`.

## Premise

`add_tmpfs` (`crates/podvm/src/run/acquire.rs:172`) adds the map to the
environment that the record keeps, and `exec` into a container reuses
that environment.

## Approach

Keep the tmpfs maps out of the environment in the record, or rebuild the
maps of an `exec` from the record without the tmpfs entries.

## Decision

A tmpfs ends with its run, as the help says; `exec` after it sees the
tree of the image.

## Prove

`cargo test -p podvm --test tmpfs_exec` exits 0, with a test that `exec` after a named `run --tmpfs /tmp` lists `/tmp` of the tree

## Done

**Done 2026-11-14.** `exec` drops from the volumes of the record each
`--tmpfs` directory of the store that no longer exists (it ended with
its run) and each device pair of T-1774, which `maps` adds again
(`exec/enter.rs`, `lifecycle/volume.rs` `is_device_pair`). During a
foreground run, which `exec` reaches since T-1786, the directory exists
and `exec` sees it. `cargo test -p podvm --test tmpfs_exec` exits 0:
after `run --name tx --tmpfs /tmp IMG tee /tmp/x`, `exec tx cat
/tmp/keep` prints the file of the tree and `exec tx cat /tmp/x` exits 1;
without the change the first `exec` refused at 125. By hand on the
namespace rung and on the userland fixture with `alpine:3.20`, `exec tt
ls -ld /tmp` lists the directory of the tree and `/dev/zero` reads.

# T-1831: The help of `run` and `create` promises what the verbs do not do

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), LIFE-06, LIFE-07, RUNS-18, RUNS-23, LIFE-09
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`podvm create -d` prints an id and starts nothing; `create --help` lists
`-d` ("start the container") and `--report`, and `create --report` is
refused. The help of `--strict` says it refuses where "the selected
rung" is Degraded and that an "escape ... still refuse[s]"; plain
`--strict` on the userland rung exits 0 with seven warnings, one of them
the raw-syscall escape. The help of `-v` says "refused on the namespace
rung until TODO/cli.md T-1761"; T-1761 is done and `-v` works there.
`--label` (Native, filterable) is absent from `run --help` and
`create --help`, and `rm -v` (Stub) from `rm --help`.

## Premise

`create` takes the option set and the help of `run`
(`crates/podvm/src/lifecycle.rs:98`). The texts are in
`crates/podvm/src/run.rs:54` (the `-v` text), `crates/podvm/src/run.rs:99`
(the `--strict` text) and the usage of `rm` at
`crates/podvm/src/lifecycle/wait_rm_cp.rs:124`. The escape gap is
`Capability { hit: false }` in `crates/podvm/src/complete/strict.rs`.

## Approach

Refuse `-d` on `create` by name, and leave `-d` and `--report` out of
`create --help`. Rewrite the `--strict` sentence: plain `--strict`
refuses the safety classes it can attribute and warns on a gap it cannot
attribute; `--strict=all` refuses each. Remove the T-1761 clause. Add
`--label` and `rm -v`.

## Decision

The behaviour of `--strict` stays (the QA calls it defensible); the text
changes. T-1857 adds the check for an "until T-NNNN" whose entry is done.

## Prove

`cargo test -p podvm --bin podvm help_text` exits 0, with tests that `create -d` exits 125, that `create --help` has no `--report`, that `run --help` has `--label` and no "until TODO/cli.md T-1761", and that `rm --help` has `-v`

## Done

**Done 2026-11-15.** `create -d` is refused with 125 by name
(`create_detach`); `create --help` leaves out `-d` and `--report`
(`usage`, `without_blocks`); the text of `--strict` says that plain
`--strict` refuses the safety classes podvm can attribute and warns on
each other difference, the raw-syscall escape too, and that
`--strict=all` refuses each; the `-v` text drops the clause of T-1761;
`run --help` has `--label`; `rm --help` has `-v, --volumes`
(`RM_USAGE`). `cargo test -p podvm --bin podvm help_text` exits 0.

# T-1832: The manual, `--help` and `docs/limits.md` disagree with the binary in six places

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), MACH-11, MACH-12, IMG-18, IMG-20, MISC-08, MISC-09
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

`exec` and `create` with `--podvm-tier=machine` say "running an OCI image
as a guest arrives with T-1656"; T-1656 is done, and `exec --help` does
not say the tier is refused. The limits page said "A dynamic
program and a detached run refuse by name" on the syscall and emulate
tiers; a dynamic program runs there. `extract --help` says a reference
that names more than one image is refused; the host platform is taken
silently. `pull --help` calls `--insecure-registry` "docker's flag";
`docker pull` has none (it is a daemon option). The manual heads
sections `podvm prune`, `podvm df`, `podvm abi`, `podvm install-names`,
and each of them is refused at the top level. `podvm --help` omits
`login` and `logout`.

## Premise

The machine text is `crates/podvm/src/tier.rs:178`. The pull text is
`crates/podvm/src/images.rs:39` and the parity notes at
`crates/podvm/src/parity.rs:234` and `:342`. The host preference is
deliberate (`crates/podvm-image/src/store/records.rs`). The top-level
list is in `crates/podvm/src/main.rs`.

## Approach

Correct each text to the behaviour: "the machine tier serves `run`; a
re-entry into a guest is T-1750"; scope the limits sentence to a
detached run; say that `extract` prefers the host platform and print the
chosen platform on stderr; say "docker's daemon option of the same
name"; head the manual sections by the verb that serves them; add
`login` and `logout` to the top-level help.

## Decision

Text only. The behaviour of each verb stays.

## Prove

`cargo test -p podvm --bin podvm doc_text` exits 0, with tests on the six texts, and `podvm --help` lists `login` and `logout`

## Done

**Done 2026-11-15.** The machine refusal of `exec` and `create` says that
the tier serves `run` (`tier.rs`, also T-1857), and `exec --help` says
that the tier is refused and that a re-entry into a guest is T-1750.
`docs/limits.md` scopes the refusal of the syscall and emulate tiers to a
detached run. `extract --help` says that the host platform is taken, and
`extract` names the platforms and the one it took on stderr.
`--insecure-registry` is "docker's daemon option of the same name" in the
help of `pull` and in the parity rows. The manual heads each section by
the argv that serves it (`=== podvm image prune`, `=== podvm system df`).
`podvm --help` lists `login`, `logout` and `container`. `cargo test -p
podvm --bin podvm doc_text` exits 0.


# T-1833: Twelve commands refuse `-h` and `--help`, and `system df --help` prints the usage of `system info`

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), MISC-05
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`podvm lab plan --help` exits 125 with "flag --help: the flag needs a
value"; `lab up`, `down`, `check`, `try` and `matrix` refuse before any
flag; `profile list --help` prints the usage to stderr with exit 125;
`profile show --help` says "unknown option"; `prune`, `install-names`,
`abi` and `df` with `--help` exit 125 (measured for `prune` on `main`).
The manual says "Every verb prints its own usage with -h or --help and
exits 0", and the parity row of `prune -h, --help` says Native.
`system df --help` and `system abi --help` exit 0 with the usage of
`system info`.

## Premise

The lab parser reads `-h` as a knob; the guest verbs of `lab` refuse
first; the profile verbs have no help arm; the four top-level names go
to the parity lookup, which refuses them with the advice to use the
group.

## Approach

Take `-h` and `--help` first in each of those verbs. For the four
top-level names, print the usage of the group verb with exit 0. Give
`system df` and `system abi` their own usage.

## Decision

The rule of the manual holds for each name that the manual or the
parity table lists. T-1853 adds the check over the parity table.

## Prove

`cargo test -p podvm --test help_every_verb` exits 0, with a test that each verb and sub-verb of the parity table answers `-h` and `--help` with exit 0 and its own usage on stdout

## Done

**Done 2026-11-15.** Each verb of `lab` answers `-h` and `--help` before
its flags and before the refusal of a guest verb (`lab::verb_usage`);
`profile list` and `profile show` answer them; `podvm prune`,
`install-names`, `abi` and `df` with `-h` or `--help` print the usage of
the group verb that serves them with exit 0 (`man::alias_help`); `system
df` and `system abi` print their own usage (`DF_USAGE`, `ABI_USAGE`),
not the one of `system info`. `cargo test -p podvm --test
help_every_verb` exits 0: each of the 45 verbs of the parity table
(from `system info --format '{{json .Parity}}'`, without the names
`docker` and `podman`) gives exit 0 and a usage on stdout for `-h` and
for `--help`. By hand: `lab plan`, `lab up`, `lab matrix`, `lab
scenarios`, `profile list`, `profile show`, `prune`, `abi`, `df` and
`install-names` with `--help` give exit 0 and their usage.

# T-1834: `system info --format '{{json .X}}'` prints counts and `StrictOk` as strings

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), MISC-06 (verify item 17)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`podvm system info --format '{{json .StrictOk}}'` prints `"false"`, which
`jq` reads as true. `.Verbs`, `.ParityRows` and `.VerbsNone` print `"80"`,
`"302"` and `"33"`.

## Premise

The fields are built as strings (`crates/podvm/src/system/df_info.rs:382`,
`("StrictOk", strict_ok.to_string())`), and the `json` function quotes
each string.

## Approach

Give the template values a type: a boolean for `StrictOk`, numbers for
the counts. The text template prints them as now.

## Decision

The JSON form exists so that a program can branch on it.

## Prove

`cargo test -p podvm --bin podvm info_json_types` exits 0, with tests that `{{json .StrictOk}}` is `true` or `false` and `{{json .Verbs}}` is a number

## Done

**Done 2026-11-15.** `system info` names `StrictOk`, `ParityRows` and
the `Verbs` counts as JSON documents beside `Parity` and `ExitCodes`
(`system.rs`, `DOCUMENTS`), so `{{json .StrictOk}}` prints `true` or
`false` and `{{json .Verbs}}` a number; the text template prints them as
before. `cargo test -p podvm --bin podvm info_json_types` exits 0. By
hand: `{{json .StrictOk}} {{json .Verbs}} {{json .VerbsNone}} {{json
.Version}}` prints `true 80 33 "0.2.0-beta.18"`.

# T-1837: The refusals of `lab`, `windows fetch`, `remote ssh` and `probe-inside` do not name the value, the unit or the verb

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), MISC-11 items 1 to 11
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

`lab plan --loss 101` says "loss 10100 is out of its range 1 to 2000"
(hundredths of a percent, no unit). `--tcp 70000` does not name the
value. `--delay -5` and `--loss -1` say "not a whole number" and "not a
percent". `--cgnat` says "cgnat needs nat44", and the shipped scenario
`cgnat` is `--nat both --cgnat`. `--nat both extra` calls the stray word
a flag with no value. `doctor store_exec` names `$PODVM_STORE` but not the
action. `windows fetch --url file:///etc/hostname` prints ": : connection
failed: the URL is not http:// or https://". A download clamped by
`RLIMIT_FSIZE` names the ceiling and not its source. `probe-inside`
refusals start with `podvm run:`. `remote ssh forward tcp onlyhost` and
`connect --ready-wait 0` print the bare usage of a helper.

## Premise

The knob errors are in `pod-netlab` (`src/knobs/check.rs`,
`KnobError::OutOfRange`); podcrates `v0.45.0` gives each one the value
as written, its unit and its range, a text for a negative number, and a
`--cgnat` text that agrees with the scenario (podcrates@T-586, COORD-101,
issue 347); the fetch text is in
`crates/podvm-windows/src`; the helper usages come from the podvm-ssh
helpers; `probe-inside` calls the parser of `run` with the verb `run`.

## Approach

podvm asks podcrates to name the value and the unit in each knob error,
to say "must be positive" for a negative number, and to accept `both`
with `cgnat` or say so (proposed COORD issue). In podvm: one text for a
scheme refusal, the source of a clamped ceiling, the outer verb in
`probe-inside`, the flag and the value in each helper refusal, and
"set `$PODVM_STORE` to a directory on a file system that permits exec".

## Decision

Text only.

## Prove

`cargo test -p podvm --bin podvm misc_refusal_text` exits 0, with a test for each podvm-owned text above; the knob texts are proved in podcrates

## Done

**Done 2026-11-15.** The knob texts of `lab plan` come from podcrates
(podcrates@T-586, taken with `v0.45.0` and later): `--loss 101` reads
"loss 101% is out of its range 0.01% to 20%", a negative value says that
it is negative, and `--cgnat` names `--nat nat44 or --nat both`. In podvm:
`windows fetch --url file:///...` is one refusal at the parse (125), not
five failed tries; a file over the ceiling names RLIMIT_FSIZE and `ulimit
-f` (`room_for`, already); `probe-inside` names its run after a refusal
that starts with `podvm run:`; the `tcp` target of the proxy helper and
`--ready-wait` of the operator name the cause before the usage. The text
of `doctor store_exec` is in podcrates (`pod-hostprobe`), which names the
cause of the denial. `cargo test -p podvm --bin podvm misc_refusal_text`
exits 0, and `cargo test -p podvm-ssh` exits 0 with a test for each
helper text.


# T-1838: The help of `windows` and `machine ssh` does not align its columns, and 21 sentences start with a lowercase `note:`

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), `findings/misc.md`, "Small prose defects"
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

In `podvm windows --help`, descriptions start at column 33, 32 and 31 in
one block; in `podvm machine ssh --help`, at 28 and 26. 21 help sentences
start with `note:` after a full stop.

## Premise

The usage texts are literal strings with spaces for alignment.

## Approach

Align each description column of the two texts. Keep `note:` as the
fixed marker, and start it on a new line, so that it does not follow a
full stop in a sentence.

## Decision

The marker stays lowercase: it is the marker of each help text.

## Prove

`cargo test -p podvm --bin podvm help_columns` exits 0, with a test that each option line of a usage text starts its description at one column for each block

## Done

**Done 2026-11-15.** The `--podvm-*` lines of `windows --help` and
`machine ssh --help` start their descriptions one column right, at the
column of their block (the rename from podbox had made the flags one
letter shorter). In the help of `run`, each `note:` that followed a full
stop starts its own line (five texts; no other usage text has one in a
line). `cargo test -p podvm --bin podvm help_columns` exits 0: a test
reads each block of the two texts and finds one column.


# T-1845: `profile show` writes a part of the document to stdout and then fails

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), IMG-12
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

With a profile `mine.toml` (`extends = "default"`, `limits.cpus = 3`),
`podvm profile show mine` prints the table to stdout and then exits 125
with "`limits.cpus` needs fs.backend = \"machine\"". The stdout document
has no `# run flags:` section and looks complete to a parser.

## Premise

`show` (`crates/podvm/src/profile.rs:192`) prints the table before it
builds the run arguments.

## Approach

Build the run arguments first, then print the whole document, or print
nothing on stdout on a refusal.

## Decision

A refused verb writes no answer to stdout.

## Prove

`cargo test -p podvm --bin podvm profile_show_refusal` exits 0, with a test that a profile that cannot become a run writes 0 bytes to stdout

## Done

**Done 2026-11-15.** `profile show` builds the whole document first
(`profile.rs`, `document`), the run flags included, and prints it only
when it is whole; a refusal goes to stderr with 125 and writes nothing on
stdout. `cargo test -p podvm --bin podvm profile_show_refusal` exits 0:
`default` with `limits.cpus=3` gives no document, and `default` gives the
document with its `# run flags:` line.


# T-1850: `exec` takes 1.6 times the time of `docker exec`, and `pull` 1.4 times the time of `podman pull`

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), benchmark P4 and P5 (`benchmarks/compare.md`, part A, rows A1 and A6)
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** M
**Status:** done

## Problem

On one host (estimate: one network moment, medians of 5 and 3 runs),
`podvm exec alpine:3.20 true` took 0.1428 s and `docker exec` 0.0897 s,
while `podvm run` took 0.0673 s. `podvm pull alpine:3.20` took 4.869 s and
`podman pull` 3.488 s for the same blob of 3 630 933 bytes through the
same proxy. `exec` creates no record and no tree, so the time is in the
entry path.

## Premise

Not measured in this repository. The setup of a pull (token, manifest,
TLS) and the entry of `exec` (`crates/podvm/src/exec/command.rs`) are the
two candidates the report names.

## Approach

Add a timed row for `exec IMG true` and for a pull of one small image to
the performance budget, with the phases of each (resolve, enter, exec;
token, manifest, blob). Find the largest phase and record it.

## Decision

A measurement first; a change only where a phase is measured.

## Prove

`sh experiments/443-qa-surface.sh` exits 0, with a perf row for `exec IMG true` and one for `pull` that print the time of each phase

## Done

**Done 2026-11-15, later.** `PODVM_PULL_PHASES=1` makes `pull` print the
time of each phase (`podvm-image/src/pull/run.rs`): the client, the
manifest with its token (the 401 and the token request are inside the
first manifest request of `pod-oci`, so the two are one phase), and the
blobs. The row `perf.pull` of experiment 443 prints them: on the cloud host
`ms=2337 phases=[client 5 ms, manifest and token 964 ms, blobs 1300 ms (2
blob(s), one client and token each)]` for busybox:musl; python:3.12-slim
gave 6, 1065 and 2903 ms. The largest phases are the manifest with its
token and the blobs; each blob worker builds its own client, so each blob
starts with its own token request (an estimate of the cause, not timed
alone). The share of one token across the workers needs `pod-oci`
(COORD-129, issue 395). With the `exec` phase of the Partial below, each
clause of the Prove holds.

**Partial 2026-11-15.** The largest phase of `exec` was the probe: `exec`
ran the full probe on each call, where `run` reads the cache of T-1746.
Both paths of `exec` now take the cached findings
(`run::probe_cache::findings`). On the cloud host, a median of 5 `exec
alpine:3.20 true` went from 77 ms to 14 ms (a median `run --rm` is 27
ms), and `exec` still runs on the userland fixture. Experiment 443 has
the INFO rows `perf.exec` (ns and ul) and `perf.pull` (the wall time of a
pull of busybox:musl into a fresh store, 2.5 s here). The clause that
remains: the phases of a pull (token, manifest, blob), which podvm does
not time yet.

**Done, amended (2026-11-15, night).** podcrates `v0.52.0` gives `Client::tokens` and `Client::with_tokens` (COORD-129, issue 395): each blob worker of a pull now starts with the tokens of the client that got the manifest, so a blob asks the registry for no token of its own (`podvm-image/src/pull/run.rs`). Three pulls of `mirror.gcr.io/library/python:3.12-slim` into a fresh store on the cloud host: blobs 2743, 2599 and 2360 ms (median 2599 ms, against the 2903 ms measured before; an estimate, with the noise of the network), manifest and token 939 to 1180 ms.


# T-1913: The store of a verb has no flag, only `PODVM_STORE`

**Source:** PodUtils/podrunner#168 (COORD-056 of podrunner, ask 1)
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

`podvm load` and `podvm images` take the store from `PODVM_STORE` only, else the XDG data directory, so `podvm images` does not show what a user loaded into another store unless the variable is set.

## Premise

Each verb opens the store through `podvm_image::Store`, which reads `PODVM_STORE` first.

## Approach

A global `--store DIR`, before the verb or right after it, that sets `PODVM_STORE` for the process before any verb parses its arguments; a word further on belongs to the verb or the payload.

## Decision

The flag and the variable have one meaning; the flag wins, as it sets the variable.

## Prove

`cargo test -p podvm --bin podvm store` exits 0, with the test `the_store_flag_stands_before_or_right_after_the_verb`

## Done

**Done (2026-11-15).** `crates/podvm/src/names/store.rs` (`take`, first in `main`). On the cloud host: `podvm --store S pull alpine:3.20`, then `podvm images --store S` lists it, `podvm images` with no flag does not, `PODVM_STORE=S podvm images` does; `podvm --store=S run --rm alpine:3.20 echo --store kept` prints `--store kept`; `podvm images --store` with no value exits 125 by name. The test passes.


# T-1917: The help and the manual on the shared crate of PodUtils

**Source:** PodUtils/podnet#584 (COORD-090 of podnet), COORD-141 (PodUtils/podvm#425)
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** M
**Status:** partial

## Problem

The owner ruled on 2026-11-15 that each command line of PodUtils shares one crate for its help and its manual (PodUtils/podnet#584). podvm's `man.rs` (T-1332) has its own text writer and pager, and no man(7) or JSON writer.

## Premise

podvm's manual has no hand-kept text: the parity table gives the verbs and the summaries, and each verb's `--help` gives its section. These stay podvm's facts; only the writers move.

## Approach

Fill the crate's model from the parity table and the usage strings (no clap), with the status of each flag against docker and podman, the names by argv0, docker's exit codes and the `PODVM_*` variables in groups. Move `podvm man` onto the crate's text writer and pager, and add its man(7) and JSON writers.

## Decision

podvm keeps the parity table as the one source of the flags; the crate renders it and adds no second list.

## Prove

`podvm man --no-pager run` prints the bytes of `podvm run --help`, the release carries a `podvm.1` that `mandoc -Tlint` passes with no warning, and `podvm man --json` lists each implemented verb of the parity table.

## Done

**Partial (2026-11-16, the release).** `v0.2.0-beta.29` (release run 43913242822) carries `podvm.1` and `podvm-llms.txt`, each with its `.sha256` and `.sigstore`; the x86_64 leg linted the page with `mandoc -Tlint -Wwarning`. Remaining: the text through `pod-manual`'s writer and pager, the facts and their checks, and the rule of no work ids in the text.

**Partial (2026-11-16, the assets).** The x86_64 leg of `nightly.yml` writes `podvm.1` (`podvm man --roff`) and `podvm-llms.txt` (`podvm man --llms`) from the release binary, with `SOURCE_DATE_EPOCH` of the commit, and fails when `mandoc -Tlint -Wwarning` names a line or the text is empty. The publish job signs, attests and uploads them with the binaries, and `release.yml` refuses a release without each of the two files, its `.sha256` and its `.sigstore`. Measured on the cloud host with the build of `20e8d09a`: 3129 lines of roff, `mandoc -Tlint -Wwarning` gives no line (exit 0), and `--llms` gives 2904 lines. Remaining: a release that carries them (the Prove), the text through `pod-manual`'s writer and pager, the facts and their checks, and the rule of no work ids in the text.

**Partial (2026-11-16).** `podvm man --llms` writes the whole model as one `llms.txt` (`pod_manual::llms::full`, 2904 lines), with its row of the parity table; the date of the model comes from `SOURCE_DATE_EPOCH` where it is set (a reproducible build: `SOURCE_DATE_EPOCH=0` gives `1970-01-01`), else today. So a release can write `podvm.1` and `llms.txt` from its own binary, and no copy in the tree can drift. Remaining: those two files as assets of `release.yml` (with `mandoc -Tlint`), the text through `pod-manual`'s writer and pager, the facts and their checks, and the rule of no work ids in the text.

**Partial (2026-11-15).** podvm takes `pod-manual` of podcrates `v0.54.0` (COORD-113, podcrates@T-700 part 1). `crates/podvm/src/man/model.rs` fills its model from podvm's own sources and no second text: each verb that the parity table implements is a command, with its flags (one flag for `-a, --all`, one each for a row of several long flags; a refused row is `FlagKind::Refused`), its status against docker (`Native` is `implemented`, `None` is `refused`), the bytes of its `--help` as its usage, and the names `docker` and `podman` by argv0. `podvm man --json` and `podvm man --roff [VERB]` are its writers: 48 commands; `mandoc -Tlint -Wwarning` and `groff -man -ww -z` of the page give no line. A unit test runs `pod_manual::check::model` on the model with no finding (it found the verb `container prune` as one word of a path). The plain text of `podvm man` stays podvm's own. Remaining: the text through `pod-manual`'s writer and pager; the facts (the `PODVM_*` variables in groups, the exit codes) and the checks of the variables and the exit codes against the code; the rule of no work ids in the text (each usage string and note cites an entry); a `podvm.1` and `llms.txt` with the release, with a drift check.

**Blocked (2026-11-15), cleared by `v0.54.0`.** 
**Blocked (2026-11-15).** podcrates has not named the crate and its tag (PodUtils/podnet#584). Clears when a tag of podcrates carries the crate.

**Blocked, amended (2026-11-15, later).** podcrates takes it as `pod-manual` (COORD-108, PodUtils/podcrates#181, podcrates@T-700), in the first tag after `pod-lab` part 1. The ruling "Discipline" of the owner (2026-11-15, as podcrates relays it) adds to the Prove: `podvm man --json`; a test in CI fails when the man page or `llms.txt` differs from the model, or when the code reads a `PODVM_*` variable or gives an exit code that has no fact in the table. Clears when that tag is out.

# T-1918: podvm has no installer: the user downloads and checks the binary by hand

**Source:** PodUtils/podcrates#182 (COORD-109 of podcrates, the ruling "Installer"), COORD-142 (PodUtils/podvm#427)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** partial

## Problem

The owner ruled on 2026-11-15 that each project publishes one POSIX installer that is hermetic, idempotent and fast, and that runs on each host class of `pod-lab`. podvm has none: the README gives five lines of `curl`, `sha256sum` and `install`.

## Premise

podcrates gives the shared core (podcrates@T-702): the detection of the OS and the architecture, the download, the check against the sums of the release, the atomic move, the PATH line, `--prefix`, `--version`, `--dry-run` and an offline install.

## Approach

Take the core at its tag; podvm keeps its name, its assets and its setup steps (the optional `docker` and `podman` links, then `podvm version --verbose` and `podvm probe`). Publish the installer as an asset of each release, and run it in the release gate.

## Decision

The installer asks before a link would shadow a `docker` or a `podman` on `PATH`.

## Prove

On a host with no root, `sh install.sh` puts podvm under its prefix in seconds, a second run changes nothing and says so, and `dash`, `busybox sh` and `bash --posix` each run it.

## Done

**Partial (2026-11-16).** podcrates `v0.58.0` (COORD-116, PodUtils/podcrates#200) carries the installer core (podcrates@T-702 part 1). `scripts/install/podvm.sh` is podvm's installer on it: `inst_files` maps each triple to podvm's asset (`podvm-x86_64`, `podvm-armv7`, ...), `inst_setup` prints the link for the names `docker` and `podman` and makes none, and `inst_selfcheck` runs `podvm version --verbose` and `podvm probe` with no network. `scripts/install/core.sh` and `pins.py` are those of the tag, and the publish job of `nightly.yml` checks their SHA-256, stamps `podvm-install.sh` with the pins of the release, and makes `podvm-VERSION.tar.gz` and `podvm-SHA256SUMS`. Measured on the cloud host as uid 1000, with a release stamped from the build of the workspace: `--offline podvm-0.2.0-beta.24.tar.gz` installs under `dash`, `bash --posix` and `busybox sh`, and the self-check says the probe selects `namespace`; a second run says "already installed" and downloads nothing; a bundle whose binary has one byte more is refused by its pin, and the prefix stays empty; `pins.py check` of the tree copy passes; `shellcheck --shell=sh` of the stamped file gives one note of the core (a hook that podvm redefines). Remaining: a release that carries it, and its run in the lab profiles.

**Blocked (2026-11-16), cleared by `v0.58.0`.** 
**Blocked (2026-11-15).** The core of podcrates@T-702. Clears when a tag of podcrates carries it.

# T-1919: The state of an issue is written by hand

**Source:** PodUtils/podcrates#183 (COORD-110 of podcrates, the ruling "Issues"), COORD-143 (PodUtils/podvm#428)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

The owner ruled on 2026-11-15 that the state of each issue comes from the record by a tool. In podvm a session writes the rows of `TODO/issues.md`, the lines of `TODO/github.txt` and the `state:` lines of a body by hand, so an issue can stay open after its entries are done.

## Premise

podcrates gives a reusable workflow and the check in `pod-todo` (podcrates@T-703): an issue is open exactly when one of its entries is not done, and a block of its body holds the state.

## Approach

Take the workflow at its tag, with a kind for each issue (`report`, `coord`, `request`; a request has no entry), and move the map of `TODO/issues.md` into the form that `pod-todo` reads.

## Decision

A dry run prints each change before the workflow first closes or reopens an issue.

## Prove

`./target/x86_64-unknown-linux-musl/release/podvm-gate` fails on a row of `TODO/issues.md` whose state differs from GitHub, and after a push that marks the last entry of an open issue done, the workflow closes the issue with the tag in its comment.

## Done

**Done (2026-11-16).** Both clauses of Prove hold. `podvm-gate` fails on a row whose state differs from GitHub: with #440 set open in `TODO/github.txt` (GitHub's states, which `scripts/check-github-numbers.py` holds to GitHub) and closed in its row, it exits 1 with "issue #440 is open on GitHub, and its row says closed". After a push that marks the entries of an open issue done, `issue-sync.yml` closes the issue with the status block that names the tag: #443 (T-1921, podcrates `v0.61.0`) and #395 (T-1850, `v0.2.0-beta.22`) on 2026-11-16, and #440 after its row was fixed. The sync runs on each push to `main` (runs 43816598500 onward) and each day.

**Blocked (2026-11-15).** The workflow and the check of podcrates@T-703. Clears when a tag of podcrates carries them.

**Blocked, amended (2026-11-16).** podvm takes podcrates `v0.55.0` (COORD-115, PodUtils/podcrates#194), whose `pod-todo` holds the rules of the issues and whose `issue-sync.yml` writes them. With `issue_rules = "on"`, `podvm-todo check` gave 52 problems: 5 open reports whose entries are done (podvm closed #272, #353 and #401; #149 and #162 are references), and 47 references "(report) names no entry". A reference holds no work, and no kind of `pod-todo` fits it. COORD-145 (PodUtils/podvm#435) asks for a kind `reference`. Clears when a tag carries it; then the rules go on, and `issue-sync.yml` is called with a dry run first.

**Partial (2026-11-16).** podcrates answered COORD-145 with the kind `reference` in `v0.56.0` (podcrates@T-723). podvm takes `v0.56.0` and sets `issue_rules = "on"` in `TODO/todo.toml`: `podvm-todo check` now ties the state of each issue to its entries, and gives no problem. The 47 references take their kind from the title `[Reference]`; the three messages of another writer (#331, #335, #337, titled `COORD-NNN of NAME:`) are in a table with a column `Kind` (`coord`). `.github/workflows/issue-sync.yml` calls the reusable workflow of podcrates at the commit of `v0.56.0`, with a dry run by default, on dispatch only. Remaining: the dry run, read; then `push` to `main` and the daily schedule.

**Partial (2026-11-16, the dry run).** The first dry run (run 43770947835, request 436) stopped in the step `pod-todo` of the reusable workflow: it checks out podcrates under podvm's workspace, so cargo takes podvm's `.cargo/config.toml` (target `x86_64-unknown-linux-musl`, not installed on the runner). No issue changed. COORD-146 (PodUtils/podvm#437) asks podcrates to build `pod-todo` outside the consumer's workspace. Remaining: the tag that carries it, the dry run, then `push` and the schedule.

**Partial (2026-11-16, the dry run read).** With podcrates `v0.57.0` (COORD-146 answered: the build of `pod-todo` runs from `$RUNNER_TEMP`), the dry run passed (run 43775969796, request 438): it would write the status block into 291 issues, close #395 (COORD-129, open on GitHub and closed in the record, as `v0.52.0` answered it), and leave 30 issues from outside the team unchanged until they carry the label `confirmed`. `issue-sync.yml` now runs on each push to `main` and each day at 06:17 UTC; a dispatch is a dry run by default. The 30 issues from outside the team (#86 to #94, #109, #114, #213 to #217, #329, #331 to #337, #352, #354, #356 to #358, #364, with #330 and #353 before) carry the label `confirmed`: each is closed, and each maps to entries that podvm verified and did, or to work of podcrates that podvm took. A push of podvm's account starts no run (Actions is disabled for that account), so the first real run is the schedule at 06:17 UTC. Remaining: that run.

**Partial (2026-11-16, the first runs).** Pushes start runs again. The first real sync (run 43816598500, on `b589bb94`) passed: it closed #395 as the record has it, with the status block that names T-1850 and its tag, and it kept #443 (COORD-150, its entry done) open, as a message waits for its answer. It reopened #441, because it read the record of a commit before the close, and #440, whose row was stale (fixed in `6980523c`); the next sync closes both. Remaining: the check of Prove that `podvm-gate` fails on a row whose state differs from GitHub.


# T-1925: The installer takes the core of podcrates `v0.61.0`: `core-net.sh` and the signature of `SHA256SUMS`

**Source:** podcrates `v0.61.0` (PodUtils/podcrates#209, podcrates@T-717, podcrates@T-718), T-1918
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

`scripts/install/core.sh` is the core of podcrates `v0.58.0`. In `v0.61.0` the core is two files (`core.sh` and `core-net.sh`), `pins.py` puts both inline, and the core checks the keyless cosign signature of `SHA256SUMS` where `cosign` is on the host and the run is not offline; a failed check is a refusal. podvm's release (the publish job of `nightly.yml`) does not sign `SHA256SUMS`, so a host with `cosign` would refuse podvm's installer of the new core until it does. The core also runs on macOS and Windows (Git Bash) now.

## Premise

podcrates' `release.yml` signs `SHA256SUMS` on the tag (`SHA256SUMS.sigstore.json`), and its first run on `v0.61.0` verified the signature, and refused a changed copy and another identity.

## Approach

1. Copy `core.sh`, `core-net.sh` and `pins.py` of the tag, and the changes of `podvm.sh` that the new core needs.
2. The publish job of `nightly.yml` (the signer of each podvm release) signs `SHA256SUMS` as podcrates does, before the installer is published.
3. Run the installer in the labs and with `cosign` on a hosted runner.

## Decision

A separate entry: the take of the crates of `v0.61.0` changes no release asset, and this change must land with the signing step of `release.yml`.

## Prove

On a hosted runner with `cosign`, `podvm-install.sh` of a release installs and checks the signature; a changed `SHA256SUMS` is refused; a second run changes nothing.

## Done

**Done (2026-11-16).** `v0.2.0-beta.29` (release run 43913242822, the publish job of nightly run 43915911525 on the tag) carries `SHA256SUMS` and `SHA256SUMS.sigstore.json`. The steps of the publish job passed on a hosted runner with cosign: the offline install and a second run that says "already installed"; `cosign sign-blob` of `SHA256SUMS` for `nightly.yml@refs/tags/v0.2.0-beta.29`; its check, which also fails on a changed copy and on the identity of `refs/heads/main`; and the published installer from the files of the release (`PODVM_MIRROR=file://`), which says "the signature of SHA256SUMS: ok (cosign, ...)", then "already installed" on a second run. `release.yml` found each asset.

**Partial (2026-11-16).** podvm takes podcrates `v0.63.0` (the tag of the crates of `Cargo.toml`); its core differs from `v0.61.0` in one case: a copy with no pins says that it is not compared with `SHA256SUMS`. `scripts/install/core.sh`, `core-net.sh` and `pins.py` are those of `v0.63.0`. `scripts/install/podvm.sh` includes both files of the core and sets `INST_WORKFLOW=nightly.yml`. The core then fetches the release assets `SHA256SUMS` and `SHA256SUMS.sigstore.json`, and runs `cosign verify-blob --bundle` with the identity `https://github.com/PodUtils/podvm/.github/workflows/nightly.yml@refs/tags/vVERSION` (as `crates/podvm-release/src/identity.rs`) and the issuer `https://token.actions.githubusercontent.com`. The publish job of `nightly.yml` checks the SHA-256 of the three core files, stamps the installer from the sums of the binaries, and writes `SHA256SUMS` of the binaries, the installer and the bundle (in place of `podvm-SHA256SUMS`). It installs from the bundle twice, refuses a run whose ref is not `refs/tags/vVERSION`, and signs `SHA256SUMS` with `cosign sign-blob --bundle SHA256SUMS.sigstore.json`. The check of the signature must pass, and must fail on a changed copy and on the identity of `main`. Then the published installer installs from `dist` (`PODVM_MIRROR=file://`) with `cosign`, and a second run changes nothing. The release attaches both files, and `release.yml` checks them with the installer, the bundle and their `.sigstore`.

Measured on the cloud host as uid 1000, with an installer stamped from the build of the workspace (0.2.0-beta.28): `--offline` installs under `dash`, `bash --posix` and `busybox sh` 1.36.1, and a second run says "already installed". Through `PODVM_MIRROR=file://` with no `cosign`, one line says that the signature is not checked. With a stand-in `cosign`, the installer gives the identity and the issuer above. It refuses a missing `SHA256SUMS.sigstore.json`, a failed `cosign`, a `SHA256SUMS` whose pin of `podvm-x86_64` differs, and a changed installer; each refusal leaves the prefix empty. `shellcheck --shell=sh` 0.11.0 gives no note for the tree copy. For the stamped file it gives three notes SC2329 (info) of the core: the hooks that a project redefines (the core disables SC2317 only). Remaining: the hosted run of Prove, a release whose publish job signs `SHA256SUMS`, then its installer with `cosign`. A real `cosign` cannot run on this host.

# T-1931: A run connects to the hosts of the package sources of the image, and no document names it

**Source:** PodUtils/podrunner#202 (COORD-076 of podrunner, podrunner@T-329), COORD-170 (PodUtils/podvm#463), COORD-171 (PodUtils/podvm#464)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

podrunner's fifth QA saw `podvm run` of an Ubuntu 24.04 image open connections to `help.ubuntu.com:443`, `archive.ubuntu.com:443` and `security.ubuntu.com:443` before the payload ran. A user who holds the egress of a unit reads them as "the unit calls home".

## Premise

The connections are the probe of the source fixup (T-0411): a bounded `HEAD https://HOST/` to each host of an `http://` URL in the package sources of the image, through the proxy of the environment. `--no-source-fixup` turns it off. `help.ubuntu.com` is in a comment of `ubuntu.sources`, and `pod_rootfs::sources::rewrite` reads comments too.

## Approach

Name the probe, its hosts, its bounds and its cache in the parity notes of `--no-source-fixup`; tell podrunner to pass the flag; ask podcrates to read a URL only from a line that the package manager reads.

## Decision

The fixup stays on by default: a host that drops tcp/80 stalls apt for minutes, and the probe costs at most 4 s for each host, once a day.

## Prove

`podvm run --rm --no-source-fixup mirror.gcr.io/library/ubuntu:24.04 true` with `HTTPS_PROXY` at a listener sends no request; with no flag it sends one `CONNECT` to `archive.ubuntu.com:443` and one to `security.ubuntu.com:443`, and none to `help.ubuntu.com`.

## Done

**Done (2026-11-17).** podcrates `v0.68.0` (PodUtils/podcrates#219, podcrates@T-846) answers COORD-170: `pod-rootfs` takes a URL only from a line that the package manager reads (no comment; in a `*.sources` file only `URIs`). Measured as uid 1000 on the cloud host with `ubuntu:24.04` and a listener as `HTTPS_PROXY`: with no flag the run sends `CONNECT archive.ubuntu.com:443` and `CONNECT security.ubuntu.com:443`, and none to `help.ubuntu.com`; with `--no-source-fixup` it sends none. podrunner passes the flag since its `2d481dd` (PodUtils/podrunner#202 closed).

**Partial (2026-11-17).** Measured as uid 1000 on the cloud host (a listener on `127.0.0.1:18777` that answers 403, `ubuntu:24.04`): with no flag the run sends `CONNECT help.ubuntu.com:443`, `CONNECT archive.ubuntu.com:443` and `CONNECT security.ubuntu.com:443`; with `--no-source-fixup` it sends none, and `run -d`, `create` and `exec` take the flag. `help.ubuntu.com` is line 1 of `etc/apt/sources.list.d/ubuntu.sources`, a comment. The parity notes of `--no-source-fixup` (`run`, `exec`) now name the request, its hosts, its bounds and its cache. COORD-171 (#464) tells podrunner to pass the flag; COORD-170 (#463) asks podcrates to skip comments. Remaining: the tag of podcrates that skips them, and the Prove with no `help.ubuntu.com`.

# T-1937: The external QA of `0.2.0-beta.31`: the argument checks of the CLI

**Source:** the external QA of `0.2.0-beta.31` (2026-11-17), issues 478, 479, 482, 483, 484, 485, 486, 487, 488, 489, 491, verified by a reproduction on `main`
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

Each verified report is a promise of the help or of docker that the parser does not keep:
- 484: `--name` takes an empty name, a `/`, a space and 300 characters, and `ps` and `inspect` show them.
- 486: `logs NAME extra` and `version extra` ignore the extra word (also `lab profiles extra`, a comment of 486).
- 487: `image bogus` says "not implemented yet" with exit 125, where `container bogus` says "not a subverb" with exit 1.
- 489: `exec --platform P NAME` of a container is accepted and ignored.
- 491: `doctor` ends with a bare `profile: refused`, with no reason.
- 478: `man --llms` with `--json` or `--roff` (and a page name) ignores the other flag.
- 479: `doctor --strict=0` and `probe --strict=0` report an internal bug of podvm; `--strict=false` works.
- 482: `run --profile NAME IMAGE CMD`, where the profile names an image, takes IMAGE as the command, and no help says so.
- 483: `podvm man` lists `abi`, `df`, `prune` and `install-names` as top-level commands; they refuse and name `system`.
- 485: `logout` help says a second logout is still logged out; it exits 1 (the decision of T-0209).
- 488: `--` ends the options of `run`, and `exec`, `pull` and `images` refuse it.

## Premise

Each item is reproduced as uid 1000 on the cloud host with the binary of `main` at `cf61cdc`.

## Approach

One small change for each item, with a test of its refusal. 485 keeps exit 1 (T-0209) and says so in the help.

## Decision

-

## Prove

`cargo test --release -p podvm --bin podvm` passes, with a test of each refusal, and the repro of each issue, as uid 1000, gives the refusal or the result of the fix.

## Done

**Done (2026-11-24).** Measured 2026-11-24 on 0.2.0-beta.40 as uid 1000: #478, #479, #482, #483, #484, #485, #487, #488, #489 and #491 are fixed (the commits are in `TODO/issues.md`, the notes in CHANGELOG 0.2.0-beta.32), and their issues are closed. #486 (`lab profiles extra` and `lab scenarios extra` exit 0 and ignore the extra word) moved to [T-2088](cli.md).

**Partial (2026-11-17).** 484: `--name` and `--name=` take docker's rule (`opts::name_arg`, `[a-zA-Z0-9][a-zA-Z0-9_.-]*`, at most 64 bytes), with a test. 486: `logs` refuses a second container and `version` an operand, with exit 1. 487: `image bogus` says "is not a subverb of the group" with the usage (which names `extract` now) and exit 1. 489: `exec --platform` of a container refuses a platform that is not the one of its image (`command::container_platform`), exit 125. 491: `doctor` prints `machine profile: refused (the machine tier needs: LEGS)`; experiment 364 reads the new label. Measured as uid 1000 with the binary of this commit. 485: the `logout` help says that a logout of a server with none stored exits 1 (T-0209 keeps the code). 478: `man --llms` with `--json`, `--roff` or a page is refused by name, exit 1. 488: `--` ends the options of each verb (`parity::expand`): `pull -- IMG`, `images -q -- NAME` and `exec -- NAME CMD` work; `run` and `create` keep their own reading, and a `--` of the payload after the image stays (a test pins each). 482: under a profile that names an image, a first operand that reads as an image reference is refused by name and names `-s fs.image=` (a test pins it). Remaining: `lab profiles extra`.

# T-1943: The external QA of `0.2.0-beta.31`, second batch: panics, flags and messages

**Source:** the external QA of `0.2.0-beta.31` (2026-11-17), issues 492, 493, 494, 497, 499, 500, 504, 505, 506, 507, 508, verified by T-1942
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

- 505: each argument that is not UTF-8 panics podvm (`env::args`), exit 134.
- 492: `--tls-verify=false` is split into `--tls-verify` and the word `false`, which becomes the image; TLS verification cannot be turned off.
- 506: `notebook --kill-after` with a large value panics (an overflow of `Instant`), after the child run started.
- 508: `--podvm-tier=syscall -u UID` says that the identity is faked, and the payload runs as the caller (the tier takes the interposer out).
- 507: the warning of an image `USER` says "podvm runs as uid 0" on the userland rung, where the payload runs as the caller.
- 493, 494: `notebook --cycles` clamps above 1000, and `--restart-kernel-after` takes a cell past the end, with no word.
- 497: `run --help` omits working flags (`--profile`, `--hostname`, `--cpus`, `--pids-limit`, `--network`, and more); `exec --help` omits `-i`.
- 499: the help of `--podvm-confine` omits the paths that it always grants and that TCP is free with no `connect=`.
- 500: `login` with no `--password-stdin`, no user or an empty password prints only the usage.
- 504: `compare --strict` takes an empty reason as an explanation.

## Premise

Each item is reproduced as uid 1000 on the cloud host with the binary of `main` (T-1942), except that 508 and 507 were measured on the userland rung under `scripts/dev/nouserns.c`.

## Approach

505: `main` reads `args_os` and refuses an argument that is not UTF-8 by name, exit 125. 492: `expand` passes `--tls-verify=VALUE` through, as `--strict=VALUE`. 506: a bound on `--kill-after` and checked sums. 508: the tier refuses `-u` by name. The others: a refusal or a sentence of the help each, and a test that each flag row of `run` and `exec` is in its help (497).

## Decision

-

## Prove

`cargo test --release -p podvm --bin podvm` passes, with a test of each refusal, and the repro of each issue, as uid 1000, gives the refusal or the result of the fix.

## Done

**Done (2026-11-24).** Measured 2026-11-24 on 0.2.0-beta.40 as uid 1000: #492, #493, #494, #497, #499, #500, #504, #505, #506, #507 and #508 are fixed (the commits are in `TODO/issues.md`, the notes in CHANGELOG 0.2.0-beta.32), and their issues are closed. No item of this entry remains.

**Partial (2026-11-17).** 505: `names::argv` reads `args_os` and refuses an argument that is not UTF-8 by name with exit 125 (before: a panic, exit 134); the names of the process are read lossily. 492: `expand` passes `--tls-verify=VALUE` whole to the parser. 479: `--strict=VALUE` passes through only for `run`, `create` and `exec`; `doctor --strict=0` gets the refusal of the table. A unit test pins 492 and 479. Measured as uid 1000. 493 and 506: `notebook` takes `--cycles` from 1 to 1000 and `--kill-after` from 1 to a year, and refuses a value out of the bound by name; the sums are checked. 494: a `--restart-kernel-after` of the last cell or past it is refused, and the help says that CELL counts each cell. 497: each help names each flag row of its verb, and T-1946 checks it. 508: `--podvm-tier=syscall` refuses `-u` by name (the tier fakes no identity). 507: the warning of an image `USER` names the uid that podvm runs as (`interpose::own_uid`), not uid 0. 500: `login` names what is missing: the value of a flag, `-u USER`, `--password-stdin`, or a password that is empty or does not read. 504: `compare --strict` takes an empty or a missing reason as no explanation (a test pins both). 499: the help of `--podvm-confine` names the paths that each run may read and write, and that TCP is not confined with no `connect=`. Remaining: none of T-1943's podvm items.

# T-1944: The external QA of `0.2.0-beta.31`, second batch: `cp`, `import` and the helpers of credentials

**Source:** the external QA of `0.2.0-beta.31` (2026-11-17), issues 495, 496, 498, 502, 503, verified by T-1942
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

- 495: on the userland rung, `ln -s /tmp/f` in a container stores the host path of the tree, and `cp` out copies it; in the container `readlink` gives `/tmp/`.
- 498: `cp` into an image changes only the extracted cache, which `extract --force` and `save` lose.
- 503: `import` records an image with a hard link to an absolute path (or through `..`), which `extract` then refuses.
- 496: a helper of credentials gets `https://HOST/v2/`, not the server key of the configuration (pod-oci, podcrates).
- 502: `remote ssh forward exec` exits 0 whatever the exit code of the command (`podvm-ssh` `Exec` drops the status).

## Premise

Verified by T-1942 (the reproduction of each, or the code for 502).

## Approach

495: `cp` out takes the prefix of the tree off an absolute target, and the reverse map of `readlink` is fixed. 498: `cp` into an image is refused by name. 503: `import` refuses a hard link target that is absolute or holds `..`. 496: a COORD issue to podcrates. 502: `Exec` waits for the child and gives its code.

## Decision

-

## Prove

`cargo test --release -p podvm --bin podvm` passes, with a test of each refusal, and the repro of each issue, as uid 1000, gives the refusal or the result of the fix.

## Done

**Done (2026-11-24).** #496 (podcrates `v0.70.0`, `helper_server`, taken in `6b5a6cb8`; measured 2026-11-24 on 0.2.0-beta.40 as uid 1000: a helper of credentials gets the `ServerURL` `127.0.0.1:1`), #498 and #503 are fixed, and their issues are closed. #495 and #502 moved to [T-2088](cli.md).

**Partial (2026-11-17).** 498 (with T-1952): `cp` into an image is refused by name. 503 (with T-1952): `import` refuses a hard link whose target is absolute, holds `..`, or comes later. COORD-181 (#568) asks podcrates for 496 (the `ServerURL` of a helper of credentials). Remaining: 495 and 502.

# T-1945: `exec --env-file`, and a `-v` source that is opened with no symbolic link (PodUtils/podrunner#277)

**Source:** PodUtils/podrunner#277 (COORD-081 of podrunner, podrunner@T-403 and @T-405)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

podrunner gives the secrets of a job as `-e K=V`, which `/proc/PID/cmdline` of `podvm` shows to each user of the host. And a job can make a symbolic link in the source of a `-v` after podrunner checked it and before podvm opens it, so that the container gets a writable path of the host.

## Premise

`run`, `run -d` and `create` take `--env-file` since T-1828; `exec` did not.

## Approach

`exec --env-file PATH`. For the source of a volume: an option with which podvm opens the source with `openat2(RESOLVE_NO_SYMLINKS)` and binds the descriptor, not the path; the bind of the namespace rung is pod-enter's (a COORD issue to podcrates), and the userland rung maps paths, which cannot hold a descriptor.

## Decision

-

## Prove

As uid 1000: `exec --env-file F NAME sh -c 'echo $SECRET'` prints the value, and no `/proc/*/cmdline` holds it while the exec runs. With `--podvm-volume-base BASE`, a `-v BASE/LINK:/w` whose `LINK` is a symbolic link out of `BASE` exits 125 on `run` and `run -d` and names the volume, and a `-v BASE/DIR:/w` binds and takes a write.

## Done

**Done (2026-11-18).** `--podvm-volume-base DIR` of `run`, `run -d` and `create`: each `-v` source under DIR opens under it with `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`, and the bind takes that descriptor. `run -d` and `create` pass DIR in `PODVM_BINDS_BASE` of the record, and the launcher sets `VolumeBind::host_base` of pod-enter (podcrates `v0.70.0`, podcrates@T-876); a foreground run opens the source itself and binds `/proc/self/fd/N` (`userns::host::beneath`). The parser compares the spelled source with its canonical path, so a link under DIR that the parse would follow is a refusal before any bind (`run/parse/volume_base.rs`). The userland rung and each guest tier refuse the flag by name. Measured as uid 1000 on the cloud host with `python:3.13-slim`: `-v BASE/link:/w` (a link to a directory out of BASE) exits 125 with "its host path holds a symbolic link under its base" on `run` and on `run -d`, also as `--podvm-volume-base=BASE`; `-v BASE/work:/w` binds, and a write of the payload of `run -d` lands in `BASE/work`.

**Partial (2026-11-17).** `exec --env-file PATH` (and `--env-file=`), with the reader of `run` and a parity row. With alpine 3.20 of mirror.gcr.io, as uid 1000: the payload of `exec --env-file` prints the value, and no `/proc/*/cmdline` of the host holds it while the exec runs. Remaining: the source of a volume with no symbolic link.

# T-1951: The external QA of `0.2.0-beta.31`, third batch: the output contract

**Source:** the external QA of `0.2.0-beta.31` (2026-11-17), issues 513, 514, 517, 518, 519, 520, 521, 522, 524, 530, 531, 532, 533, 541, 542, 543, 545, 546, 556, 559, verified by T-1950
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

- Exit codes: seven verbs answer a missing argument with 125, not 1 (519); `system abi` exits 2, a code of no table (521); bare `profile`, `lab` and `windows` exit 125, where the table says 0 (532); `--report` that cannot be written runs the payload, then exits 125 (513).
- Output: `inspect A B` prints two JSON arrays (514); `save -o FILE` prints the file name on stdout (522); `man --roff` and `--llms` print each usage line twice (518); `inspect` names fields that `--format` does not take (524); `uname -v` of the syscall tier holds a task reference (530); `version --verbose` shows a build error as the commit (531).
- Texts that are not true: `podvm --help` omits `port` and `logout` (520); `run --help` says that a `-w` in a volume is refused (533); the machine tier's `exec` and `--device` texts (541, 542); `docs/limits.md` on arm64 guests (543); the probe, the banner, README, architecture, SECURITY and the limits page on the namespaces of the namespace rung (546), and on `-v` as a bind mount there (556); a sealed bundle with no `argv` (517, by design and in no document); `login` says "Login Succeeded" with no contact to the registry (559); `run -t` with no `-i` relays stdin (545).

## Premise

Each item was reproduced as uid 1000 on the cloud host with the binary of `main`, or traced in the code where it needs a guest (T-1950).

## Approach

One change for each item; `tests/cli_contract.rs` (T-1946) checks the help of each verb and the top help.

## Decision

-

## Prove

`cargo test --release -p podvm --bin podvm` and `cargo test --release -p podvm --test cli_contract` pass, and the repro of each issue, as uid 1000, gives the refusal or the result of the fix.

## Done

**Done (2026-11-24).** Measured 2026-11-24 on 0.2.0-beta.40 as uid 1000: #513, #514, #518, #520, #521, #522, #530, #532, #533, #541, #542, #543, #545, #556 and #559 are fixed (the commits are in `TODO/issues.md`, the notes in CHANGELOG 0.2.0-beta.32), and their issues are closed. #517, #519, #524, #531 and #546 moved to [T-2088](cli.md).

**Partial (2026-11-17).** 520: `podvm --help` names `port` and `logout` (the text moved to `usage_text.rs`), and the contract test checks that it names each one-word verb, with the names that redirect to `system` as named exceptions. 522: `save -o` writes no line on stdout, and its line on stderr names the file. 521: an unreadable file of `system abi` exits 125, a code of the table. 483 (T-1937): `podvm man` lists the redirects as `system df`, `image prune` and the others. 514 and 523 (T-1952): `inspect` prints one JSON array of each reference in the order given, a container before an image; measured with two containers, a container and an image, a name of both, and a missing name. 532: a bare `profile`, `lab` and `windows` print the help and exit 0. 530: `uname -v` of the syscall tier is `#1 SMP podvm syscall tier`. 533: `run --help` says that `-v` is a bind mount on the namespace rung and that a `-w` in a volume resolves there. 542: the refusal of the machine tier names only the flag given. 513: `--report` refuses an empty path, and its preflight probes the directory where the writer makes its temp file (`/dev/full` is refused before the payload). 519: `probe-inside`, `compare`, `port` and `profile show` answer a missing argument with exit 1 and a line that names it; `login` keeps 125 for a missing flag (a refusal of the flag parser, in the words of the table) and names the flag. 559: `login` keeps docker's `Login Succeeded` on stdout (scripts read it), and says on stderr that the registry checks the login at the next pull. 541 and 543: the `exec` help and `docs/limits.md` say that `exec NAME` enters a running guest, and that an arm64 guest runs. 556: `docs/limits.md` says that `-v` is a bind mount on the namespace rung. 518: the model of the manual takes the arguments of the usage line only (`man/model.rs` `split_usage`), and the rest of the help is a note; `man --roff` has 48 `Usage:` lines with none doubled, and `mandoc -Tlint -Wwarning` passes. 545: `run -t` with no `-i` gives the payload no stdin, as docker's; under `script` the output is `hi\r\n` with no `^@`, and `-i -t` still takes input. Remaining: 517, 519 (`notebook`, `machine ssh`), 524, 531, 546.

# T-1970: The outside pass of 2026-11-18: three P1 and seven P2 faults on the path of a unit

**Source:** the outside pass of the gate of a beta (T-1949, `docs/maintenance.md`), 2026-11-18: an agent with the binary of `6b5a6cb` and `podvm man` only, as uid 1000 on the cloud host
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** partial

## Problem

The pass found 3 P1, 7 P2 and 7 P3 faults (OQ-01 to OQ-17). P1: a foreground `-v HOST:/tmp[/X]` is hidden by the tmpfs on `/tmp` (OQ-01); Ctrl-C in `run -t` does nothing, because the pty is no controlling terminal (OQ-02, in `0.2.0-beta.31` too); each refused `run -d --name DUP` leaves its tree of about 110 MiB (OQ-03). P2: `run -t` with no `-i` hangs on a read (OQ-04); `exec` starts in `/`, not in the `-w` of the container (OQ-05); `inspect` of a container has no `.Config.*` (OQ-06); exit codes outside the table (OQ-07); `pull -q` hides an error (OQ-08); `run -d --rm` keeps the record (OQ-09, #474); each `rm` starts an unbounded copy of a spare tree (OQ-10). P3: OQ-11 to OQ-17.

## Premise

Each finding was reproduced twice by the pass. OQ-02 reproduces with the released `0.2.0-beta.31`.

## Approach

The P1 faults and OQ-05 first, as each meets a unit of podrunner; then the P2 faults; the P3 faults with the quality entries of their class.

## Decision

-

## Prove

As uid 1000: `run --rm -v H:/tmp IMAGE sh -c 'echo hi > /tmp/tw'` leaves `H/tw`; `(sleep 6; printf '\003'; sleep 20) | script -qec 'podvm run --rm -it IMAGE sleep 100' /dev/null` ends with 130; a refused `run -d --name DUP` adds no tree; `exec` of a container of `-w /usr/lib` prints `/usr/lib` for `pwd`.

## Done

**Partial (2026-11-18).** OQ-01: in the user namespace of a foreground run, each volume at or under `/tmp` goes to pod-enter's binds (`PODVM_BINDS` of the plan), which come after its tmpfs; podvm binds the others as before (`run::binds::split_tmp`). OQ-02: the relay of `-it` takes the `VINTR` byte of the pty out of the input where the pty has `ISIG`, and raises SIGINT on podvm, whose handler forwards it to the payload (`run::pty::send`); a controlling terminal needs a session in pod-enter (a COORD issue). OQ-03: a refused `run -d` drops its tree as `create` does (`run::drop_unrecorded`). OQ-05: `exec` takes the working directory of the record where `-w` names none. Measured as uid 1000 with `docker.io/library/python:3.13-slim`: `-v H:/tmp` lists and writes `H`, `-v H:/tmp/x:ro` reads and refuses a write, `-v H/one.txt:/tmp/one.txt` reads the file, `-v H:/data` still works; `-it` and a `^C` after 6 s end with 130; a refused `run -d --name dup` adds no directory under `trees/`; `exec` gives `/usr/lib` for a container of `-w /usr/lib`, `/etc` with `-w /etc`, and `/` for a container with no `-w`. Remaining: OQ-04, OQ-06 to OQ-17.

# T-1971: `--env-file` cannot hold a value with a newline

**Source:** PodUtils/podrunner#300 (COORD-090 of podrunner, asks 1 and 2, podrunner@T-405)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`--env-file` reads `KEY=VALUE` lines, so a multi-line secret of a job (a key, a certificate) cannot go in it, and podrunner passes it as `-e`, which each user of the host reads in `/proc/PID/cmdline`. podrunner also asks whether `--env-file` holds on the machine and incus tiers.

## Premise

`env -0` writes `KEY=VALUE` entries with a NUL after each; a NUL cannot be part of a value of the environment.

## Approach

`--env-file0 PATH` of `run`, `run -d`, `create` and `exec` (also `--env-file0=PATH`): the checks of `--env-file` (a file, readable, at most 1 MiB), entries split at each NUL, each `KEY=VALUE` with a name and kept byte for byte; the order rule of `--env-file`.

## Decision

-

## Prove

As uid 1000: `printf 'KEY=line1\nline2\0' > F; podvm run --rm --env-file0 F IMAGE python3 -c 'import os; print(repr(os.environ["KEY"]))'` prints `'line1\nline2'`, and no `/proc/*/cmdline` of the host holds the value while a unit and a machine guest run.

## Done

**Done (2026-11-18).** `run::parse::values::read_env_file0`, wired in `run` and `exec`, with parity rows and the help. Measured as uid 1000 on the cloud host with `docker.io/library/python:3.13-slim`: a value `line1\nline2-SEKRET` and a value `two words` reach the payload of a foreground run as written; `run -d --env-file0=F` and `exec --env-file0 F` give the same value; an entry with no `=` exits 125 with one line. While a unit ran, no `/proc/*/cmdline` of the host held the value. The machine tier under TCG (`docker.io/library/alpine:3.20`): the guest prints both lines, and no process of the host, QEMU included, holds the value in its argv (the values go into the init script of the initrd; the keeper of `run -d` takes the path of the file, not the values). The incus tier: by reading of the code, the values of `run -d` go into `/.podvm/cmd` of the instance (a file of root, mode 0600) and those of a foreground run and of `exec` into the request of the incus API; podvm starts no `incus` program.

# T-1974: The outside pass of 2026-11-18, the second: a P0 and four P1 faults of the new surface

**Source:** the outside pass of the gate of `0.2.0-beta.33` (T-1949, `docs/maintenance.md`), 2026-11-18: an agent with the binary of `9f714c7`, `podvm man` and a filter that denies user namespaces, as uid 1000 on the cloud host
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** partial

## Problem

The pass found 1 P0, 4 P1, 7 P2 and 5 P3 faults (OR-01 to OR-17). P0 (OR-01): on the syscall tier, `exec` followed a symbolic link that replaced a volume source after the start, and read and wrote out of `--podvm-volume-base`. P1: Ctrl-C of `run -it` ended podvm and not the payload (OR-02); Ctrl-C of `run -t` did nothing (OR-03); `exec` with no `-w` refused where the `-w` of the unit is in a volume (OR-04); a foreground syscall-tier `-v H:/tmp` was refused as a second volume at `/tmp` (OR-05). P2: an absolute link in a volume of the syscall tier did not resolve, so `python -m venv` there broke (OR-06, T-1975); `getsockname` names `/proc/self/fd/N/...` (OR-07); a source out of the base is accepted with no word (OR-08); exit 127 for a refused volume, a file as a volume source, a directory as the command and an environment over the kernel's limit on the syscall tier (OR-09 to OR-12). P3: OR-13 to OR-17.

## Premise

Each finding was reproduced twice by the pass.

## Approach

The P0 and the P1 faults, and OR-06 (a venv in a work directory of a unit), before the beta; the others with the quality entries of their class.

## Decision

-

## Prove

As uid 1000: the repro of OR-01 to OR-06 of the pass gives the result of the fix (`scratchpad` scripts `or05.sh`, `or06.sh` and the commands of the Done record).

## Done

**Partial (2026-11-18).** OR-01: the handler of `exec` on the syscall tier takes `PODVM_VOLUME_BASE` of the record (`lifecycle::syscall::exec_entry`), so a link that replaced a source is a refusal; a refused volume of the handler exits 125, not 127 (also OR-09). Measured: after `mv B/u B/u.old; ln -s OUT B/u`, `exec` exits 125 with "its host path holds a symbolic link under its base", reads nothing out of B and writes nothing there. OR-02: the relay of `-it` sends SIGINT to the payload (`run::pty::set_payload`); OR-03: with no `-i` the terminal stays cooked, and the relay threads block the signals that the waiter forwards. Measured with a payload whose handler logs SIGINT and exits 5: `-it` and `-t` each give the log line and exit 5. OR-04: `exec` passes the binds of the record to the plan of the joined entry and to its own check (`exec::in_bind`): `exec` of a unit of `-w /vol/sub -v V:/vol` prints `/vol/sub`, and a write lands in V. OR-05: a volume or `--tmpfs` at `/tmp` replaces the `/tmp` of the syscall tier (`run::binds::tmp_is_given`): `-v H:/tmp` writes into H. OR-06: T-1975. Remaining: OR-07, OR-08, OR-10 to OR-17. A fault of the OR-04 fix (found by the uid-1000 tests of the beta.34 gate, `tests/tmpfs_exec.rs`): the binds that `exec` gives the entry held the `--tmpfs` directory of an ended foreground run, and `exec` refused it with 125; the binds now leave out a `--tmpfs` directory that is gone, as the table of T-1830 does (`exec/enter.rs`). The 29 uid-1000 test binaries pass.

# T-1980: The faults of the third outside pass (2026-11-18, the gate of beta.34)

**Source:** the outside pass of the gate of `0.2.0-beta.34` (TODO/gate.md T-1949): an agent with the binary and `podvm man` only, as uid 1000
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

The pass found 0 P0, 2 P1, 5 P2 and 10 P3 faults (OS-01 to OS-17). The P1 faults are T-1978 (OS-01, `posix_spawn` of a link in a volume on the syscall tier) and T-1979 (OS-02, a link in a volume on the loader family), both done. Open: OS-03 (P2, on the loader family with alpine, `ls -l` of a link that the payload makes in the tree shows `-> /`), OS-04 (P2, a file as `-v` source on the syscall tier: "native-static rung: the volume ...: errno 20"), OS-05 (P2, a refused run of the syscall tier leaves `tmpfs/PID/0` in the store), OS-06 (P2, `--podvm-volume-base` takes a source with `..` that leaves the base, and a source out of the base), OS-07 (P2, a command that is not runnable: 125 or 127 where the manual gives 126, and "names no file in the image" for a directory), OS-08 (P3, SIGQUIT to `run --rm` leaves a Dead record), OS-09 (P3, the removal of the tree goes on after podvm exits), OS-10 (P3, `-t` sets no `TERM`), OS-11 (P3, the line of a forwarded signal), OS-12 (P3, the last line of `-it` Ctrl-C on the syscall tier has no CR), OS-13 (P3, `--env-file` names are not checked), OS-14 (P3, the message of `--podvm-tier` names `runsc`), OS-15 (P3, the report: the syscall tier, `oom_kills` and `degradations` are not in the manual), OS-16 (P3, `cp` through a guest-absolute link), OS-17 (P3, `exec` into an exited container runs the command).

## Premise

Each finding was reproduced twice by the pass, on the binary of `65287500` (before T-1976 was done).

## Approach

Work each by its class with T-1947 (fail safe at the edges) and T-1948 (messages from the entered mechanism); OS-04 and OS-07 with OR-10 and OR-11 of T-1974, OS-06 with OR-08, OS-14 with OR-14.

## Decision

-

## Prove

As uid 1000 under `scripts/dev/nouserns.c`: `podvm run --rm --podvm-tier=syscall -v FILE:/f quay.io/almalinuxorg/almalinux:9 true` refuses with 125 and a line that names the cause (OS-04), and leaves no `tmpfs/PID` directory in the store (OS-05); `podvm run --rm --podvm-volume-base B -v B/../x:/v quay.io/almalinuxorg/almalinux:9 true` refuses with 125 (OS-06); `podvm run --rm --podvm-tier=syscall quay.io/almalinuxorg/almalinux:9 /etc` exits 126 (OS-07); each other finding has a test of its class.

# T-1989: The faults of the fourth outside pass (2026-11-18, the gate of beta.35)

**Source:** the outside pass of the gate of `0.2.0-beta.35` (TODO/gate.md T-1949): an agent with the binary and `podvm man` only, as uid 1000
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

The pass found 0 P0, 6 P1, 3 P2 and 7 P3 faults (OT-01 to OT-16). The P1 faults are T-1983 to T-1988. Open here: OT-07 (P2, on the userland rung `..` after a link folds by its text), OT-08 (P2, on the syscall tier `..` at the root of a volume stays in the volume; OR-15 of T-1974), OT-09 (P2, `system abi`, `load`, `import`, `profile show` and `cp` of a directory without `-r` exit 125 where the manual gives 1 or 2), and OT-10 to OT-16 (P3: a refused start of the tier leaves `tmpfs/PID/0`, as OS-05 of T-1980; a banner of the namespace rung that says two things; and five more in the report of the pass).

## Premise

Each finding was reproduced twice by the pass, on the binary of `87f31bf`.

## Approach

Work each by its class with T-1980 and T-1974.

## Decision

-

## Prove

As uid 1000: `podvm system abi --bogus` exits 2 (OT-09); each other finding has a test of its class.

# T-1992: Each release publishes its binaries on ghcr.io

**Source:** the owner's ruling "ghcr.io" (2026-11-19, `docs/decisions.md`), relayed by PodUtils/podrunner#312
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

podvm publishes its release binaries as GitHub release assets only. podrunner pins each project by an OCI digest beside the SHA-256 of the asset, from one registry (`images/podvm.toml`), and its images are on ghcr.io.

## Premise

`release.yml` builds, signs and publishes the static binaries for x86_64, aarch64 and the other targets of the release, and has `GITHUB_TOKEN`.

## Approach

The form of call C789 of podcrates (PodUtils/podcrates#236, COORD-142), which podnet took first: `.github/workflows/image.yml` builds one image `FROM scratch` for each arch of the matrix of `nightly.yml` (`images/podvm/Dockerfile`: the static binary at `/usr/local/bin/podvm`, `ENTRYPOINT`, the labels `org.opencontainers.image.source`, `.version` and `.revision`) from the assets of the release (no second build; each asset is checked against its `.sha256`), pushes them with the tags `TAG-ARCH`, and joins them in one index `ghcr.io/podutils/podvm:TAG` (and `beta` or `latest`). After the push it pulls each platform of the index by digest and compares the binary with its asset (SHA-256), attests the provenance of the index in the registry (`actions/attest`, `push-to-registry`), checks it with `gh attestation verify`, and adds the digests to the release notes beside the SHA-256 of each binary. `release.yml` calls it with `push: true` after its check of the assets; a dispatch with `push: false` is the dry run. The GitHub assets stay.

## Decision

-

## Prove

`podvm pull ghcr.io/podutils/podvm:0.2.0-beta.N` and `docker pull --platform linux/arm64` of it give the binary of the asset of that release, byte for byte (`sha256sum` equal), and the notes name the digest.

## Done

**Done (2026-11-19).** `image.yml`, `images/podvm/Dockerfile` and the job `image` of `release.yml` are on `main`. The dry run passed (run 44446041112). Two faults of the push were fixed: the check after the push takes the manifest of each platform from the index (`7d895ec`), and the login to ghcr.io stays for `actions/attest` (`930b37e`). Run 44452103325 pushed `ghcr.io/podutils/podvm:v0.2.0-beta.35` (index `sha256:e11a9a7054c4077c24add7ab9f078fd57b82977965a62c42d02c45894fd2cc3a`), compared each of the 7 platforms with its asset, attested the index in the registry, checked the attestation, and wrote the digests into the notes. From the cloud host with no login: the binary of the layer of `linux/amd64` and of `linux/arm64` is the asset byte for byte (SHA-256 `b39df0b1...` and `41377e93...`), and `podvm pull ghcr.io/podutils/podvm:v0.2.0-beta.35` gives that digest. The job of `release.yml` runs first at the next beta, under its gate.

# T-2001: The faults of the fifth outside pass (2026-11-19, the gate of beta.36)

**Source:** the outside pass of the gate of `0.2.0-beta.36` (TODO/gate.md T-1949): an agent with the binary and `podvm man` only, as uid 1000
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

The pass found 0 P0, 1 P1 (OP-04, T-2000, fixed) and these: OP-01 (P2) `PODVM_USERNS=0` gives the userland rung only with a volume, else the namespace rung, and the banner says "namespaces: user+mount" beside `clone(NEWNS)=EPERM`; OP-02 (P2) the syscall tier refuses a `-v` of one file with "errno 20" and no line that names the limit; OP-03 (P2) `npm -v` and `npx` of `node:22-slim` end with SIGSEGV (139) on the syscall tier, while `node` runs; OP-05 (P3) a SIGUSR1 or SIGHUP to a foreground `podvm run --rm` ends podvm and leaves a record `Dead`; OP-06 (P3) `inspect` of a container has no `.State.ExitCode`; OP-07 (P3) the cap of `/tmp` of the syscall tier is not stated.

## Premise

Each finding was reproduced twice by the pass, on the binary of `23562a2`.

## Approach

Work each by its class with T-1989: OP-02 and OP-03 with the volumes and the exec of the syscall tier (T-1652), OP-05 with the signals of the waiter (T-1970), OP-01 with the ladder.

## Decision

-

## Prove

As uid 1000: `podvm run --rm --podvm-tier=syscall -v FILE:/f IMAGE cat /f` prints the file or one line that names the limit (OP-02); `podvm run --rm --podvm-tier=syscall node:22-slim npm -v` prints the version (OP-03); each other finding has a test of its class.

## Done

**Done (2026-11-20).** Each finding of the fifth outside pass is fixed: OP-01, OP-02, OP-03, OP-05, OP-06, OP-07 and the P3 finding, as the paragraph below names each with its test; OP-04 was T-2000.

Partial (2026-11-19): OP-02 is fixed. On the syscall tier a `-v` of one file is a volume of one file (`vols.rs` opens it with no `O_DIRECTORY` and marks it `file`): its path reopens the file through `/proc/self/fd/N` (`path::in_file_volume`), a call on its parent reaches the host file in its host directory (`path::parent`), a path under it is ENOTDIR, and an unlink or a rename of it is EBUSY, as of a bound file. The dispatch test `a_volume_of_one_file_is_the_file` passes. As uid 1000 on the tier with `alpine:3.20`: `cat`, `stat`, `chmod`, `touch` and `>>` of the file work, `rm` and `mv` give "Resource busy", `:ro` refuses a write, and a CA bundle of 121 certificates reads through `-v ca.crt:/ca.crt:ro`. OP-03 is fixed too: the entry of the tier laid the strings of argv and of the environment in falling order, and libuv, which takes `argv[0]` to the end of the last argument as the room of the process title, wrote past the stack at `process.title = "npm"`; they now lie in one block in rising order, as the kernel lays them (`enter::stack`). The dispatch test `the_strings_of_argv_lie_in_rising_order_below_the_environment` fails without the change and passes with it; on the tier, `npm -v` and `npx --version` of `node:22-slim` print 10.9.9 (before: SIGSEGV, 139). OP-06 is fixed (2026-11-20): `inspect` of a container gives docker's `.State.Status`, `.State.Running`, `.State.ExitCode`, `.State.Pid`, `.State.StartedAt` and `.State.FinishedAt` through `--format`, and its JSON gives `State` as docker's object with typed values (`ExitCode` is null, not 0, before an end); `--format '{{.State}}'` stays the word. An instance of the incus tier gives the first three too. The unit test `the_declared_container_fields_are_the_built_ones` holds the names, the render and the JSON. OP-07 is fixed: the help of `--podvm-tier` states the cap of `/tmp` of the syscall tier (256 MiB, set by `--tmpfs /tmp:size=N`, ENOSPC past it), and the unit test `the_help_states_the_default_cap_of_tmp` reads it from `TMP_CAP`. OP-05, measured on `163c231` as uid 1000: a SIGHUP is forwarded (exit 129, no record stays); a SIGUSR1 ends podvm with 138 and leaves the payload and a record `Dead`, as pod-enter's `wait_forwarding` forwards SIGHUP, SIGINT and SIGTERM only; COORD-211 (#608) asks podcrates for the set of call C769 of podcrates, and the parity row of `run` names the limit until then. OP-01 is fixed (2026-11-20): `podvm man` has an ENVIRONMENT section, and the facts of `man --json`, `--roff` and `--llms`, from one table (`crates/podvm/src/environ.rs`) of each variable that a user sets, with its values and its effect. The table says what `PODVM_USERNS=0` does: a foreground run with a volume gets the userland loader family, and a run with no volume gets the namespace rung of pod-enter. The other `PODVM_*` names are sorted as internal or as dev in the same module. The unit test `each_variable_of_the_sources_has_one_home_and_a_user_variable_is_in_the_manual` scans the sources of each crate, so a new name with no home fails it. The `probes` line of the banner now reads `probes (host, outside any user namespace):`, so `mode=namespace` beside `clone(NEWNS)=EPERM` is no contradiction (`podvm_probe::report::PROBES_LABEL`, unit test `the_probe_rows_say_that_they_are_of_the_host_outside_a_user_namespace`). The P3 finding is fixed (2026-11-20): the run makes the mount point of each volume in its own tree, as docker makes it in the layer of a container (`lifecycle::volume::make_mount_points`: a directory, or an empty file for a volume of one file; no link of the image is followed; `/dev`, `/proc` and `/sys` stay the completion's), so a listing of `/` names a volume whose guest path the image lacks. The unit test `the_mount_point_of_a_volume_is_made_in_the_tree` holds the table. As uid 1000 with `alpine:3.20` and `-v DIR:/data`: `ls /` names `data` on the syscall tier and on the loader family of the userland rung (`PODVM_USERNS=0`). OP-05 is fixed (2026-11-20): podcrates `v0.79.0` (podcrates@T-995, the answer to COORD-211) forwards SIGHUP, SIGINT, SIGQUIT, SIGUSR1, SIGUSR2, SIGTERM, SIGALRM and SIGPWR in `wait_forwarding` (`pod_enter::WAIT_FORWARDED`). podvm's part: the outer process of the user namespace relays SIGALRM and SIGPWR too (`userns::sigrelay`), and each thread of a foreground run (the pump of `tee`, the bound of `wall`, the relay of the pty, the start of the hold) blocks the whole set, as a thread that left one unblocked took it with the default action and ended podvm with the payload still running (measured: pid 2 had threads, and SIGUSR1 ended podvm with 138 and left the payload under the reaper). The unit test `the_outer_process_relays_each_signal_that_the_waiter_forwards` holds the set of pod-enter, the relay and the parity row of `run` to one table. As uid 1000 on the cloud host, `run --rm alpine:3.20 sh -c 'sleep 20'`: SIGUSR1, SIGALRM and SIGPWR to podvm print "forwarded SIG... to the payload", podvm exits 138, 142 and 158, and no record and no process stays; SIGHUP gives 129 as before.

# T-2043: `exec -t` gives the command no terminal and does not refuse

**Source:** the sixth outside pass (T-2044), OQ-04 (P1)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

`exec -t NAME tty` (also `-it`, also from a real pty) prints `not a tty` and exits 1, where `run -t` gets `/dev/pts/0` on the same host; the manual says that `-t` is refused by name only where `/dev/ptmx` is unusable.

## Premise

`run -t` opens a pty for the payload (`run::pty`); `exec` takes `-t` and opens none.

## Approach

`exec -t` opens a pty as `run -t` does, on each rung where `run -t` does, and refuses by name elsewhere.

## Decision

-

## Prove

As uid 1000: `exec -t NAME tty` prints a `/dev/pts/N` path and exits 0 on the namespace rung and the loader family; `exec -it` of a shell takes input; a table test holds the refusal where no pty opens.

## Done

**Done (2026-11-20).** `exec -t` opens a pty of the host's `/dev/ptmx` as `run -t` does, and gives its slave to the command as descriptors 0, 1 and 2 on each entry of `exec`: the chroot, the namespace rung, the join of a running container of the namespace rung, and the userland families (`crates/podvm/src/exec/tty.rs`, with the relay of `crates/podvm/src/run/pty.rs`). The pty takes stdin only with `-i`, as in `run`, and its interrupt key goes to the command. A running container of the namespace rung mounts a devpts of its own on `/dev/pts`, so the join takes a pty of that devpts (`/proc/PID/root/dev/pts/ptmx`, `run::pty::Pty::open_in`), and `tty` names it in the container; where that does not open, a pty of the host still gives a terminal (measured: with a pty of the host, `tty` said `not a tty` and `test -t 1` passed). `exec -t` refuses by name, with 125, where `/dev/ptmx` is unusable (T-0503), and into a guest of the machine tier or an instance of the incus tier, whose exec channel carries no terminal. A container with the init of the image keeps the pty of its init (T-2002). The unit test `exec_t_gives_a_pty_or_refuses_by_name` holds the rule for each target. As uid 1000 with `alpine:3.20`, before (`d9b152f`) and after: `exec -t tt tty` of a container of the namespace rung, in a pty and with stdin `/dev/null`, gave `not a tty` and 1, and now gives `/dev/pts/1` and `/dev/pts/0` and 0; `exec -it tt sh` takes input (`echo hi-from-input` prints, `exit 3` gives 3); on the loader family (the fixture of experiment 356, `deny-chroot` with `/dev/ptmx` usable) `exec -t cl tty` and `exec -t IMAGE tty` gave `not a tty` and 1, and now give `/dev/pts/0` and 0; with `/dev/pts` hidden, `exec -t` refuses with the line of T-0503 and 125. On the chroot rung `test -t 0` passes and `tty` finds no name, as `run -t` on that rung (no `/proc`, T-1758).

# T-2044: The faults of the sixth outside pass (2026-11-20, the gate of the next beta)

**Source:** the sixth outside pass (2026-11-20, the gate of the next beta; [the report](../docs/history/outside-pass-2026-11-20.md)): an agent with the binary of `2fc28af` and `podvm man` only, as uid 1000
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** partial

## Problem

The pass found 1 P0 (OQ-01: T-2040), 3 P1 (OQ-02: T-2041, OQ-03: T-2042, OQ-04: T-2043) and these: OQ-05 (P2) `run -it`: a Ctrl-C does not stop the child of the payload; OQ-06 (P2) `inspect --format` gives lists and maps as flat strings, and the JSON of a container gives `Labels`, `ExitCode` and `Pid` as strings and `Config` as null; OQ-07 (P2) the banner of the namespace rung says no /proc and no PID isolation beside the PID namespace and the /proc that it opens, and `.Exec.Shares` says `filesystem` only; OQ-08 (P2) on the namespace rung, podvm's process is PID 1 of the payload with its argv (the values of `-e`), the environment shows host paths, and the payload inherits descriptors of the lock and the memo; OQ-09 (P2) docker differences that the manual does not name (`TERM` of `-t`, `HOSTNAME`, `exec` into an exited container, `logs --tail all`, `start -a`, `cp CONTAINER:path -`, the text of `-v ...:z` on the namespace rung); OQ-10 (P2) exit codes: E2BIG at `execve` gives 127, and refusals after the parse of `rmi`, `load`, `import` and `cp` give 125 where the manual says 1; OQ-12 (P3) `store/staging/memo-*.tmp` stays after a SIGKILL of podvm; OQ-13 (P3) texts of the manual and of messages.

## Premise

Each finding was reproduced at least twice by the pass, on the binary of `2fc28af` (`docs/history/outside-pass-2026-11-20.md`).

## Approach

Work each by its class: OQ-06 with the fields of `inspect` (T-2001 OP-06), OQ-07 and OQ-13 with the help and the banner, OQ-08 with the entry of the namespace rung, OQ-10 with the table of exit codes, OQ-05 with the pty relay (T-1974).

## Decision

-

## Prove

`sh scripts/dev/checks.sh` passes with a test of the class of each fixed finding; each other finding is a line in "Known gaps" of the release.

## Done

**Partial (2026-11-23).** The state of OQ-05 to OQ-13, with the merge of the branch `wip/oq-run` on `main`:

| Finding | Entry | State |
| --- | --- | --- |
| OQ-05, Ctrl-C of `run -it` | [T-2052](cli.md) | done: the interrupt and quit keys go to each process of the payload's tree in its process group |
| OQ-06, `inspect --format` | [T-2060](cli.md) | done |
| OQ-07, the banner and `.Exec.Shares` | [T-2053](cli.md) | done: one table `podvm_probe::isolation` |
| OQ-08, podvm's argv, environment and descriptors in the payload | [T-2054](enter.md) | partial: titles and an empty environment, no lock and no host path of the loader family where a PID namespace holds the tree; the rest is in T-2054, with an ask to podcrates |
| OQ-09, docker differences | [T-2061](cli.md) | done |
| OQ-10, exit codes | [T-2062](cli.md) | done |
| OQ-11, `--podvm-volume-base` on the loader family | [T-2055](enter.md) | done: the manual held the rule; a test holds it |
| OQ-12, `memo-*.tmp` after a SIGKILL | [T-2063](cli.md) | done |
| OQ-13, texts | [T-2064](cli.md) | done |

Left: the remainder of OQ-08 is a line in "Known gaps" of the next release, then this entry is done.

# T-2046: `logs` loads the whole log into memory and has no bound of bytes

**Source:** PodUtils/podrunner#333 (COORD-115 of podrunner)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

podrunner reads the log of a unit at the end of each job with `podvm logs CONTAINER` and `podvm logs --podvm-tier=incus NAME`, and a job sets the size of that log. `logs` reads the whole log into memory before it writes (`pod_supervise::logs` gives one `Vec`; `--tail` and `-f --tail` read the whole log too), so a log of 200 MB costs 200 MB of the host's memory before podrunner can stop the read. `logs` has no flag that stops after N bytes.

## Premise

The log of a container is one file (`$store/containers/ID/log`); a read with a seek and one buffer gives each byte in order. The API of incus gives a file whole (`GET /1.0/instances/NAME/files?path=`), and `pod-incus` reads that answer whole, 64 MiB at most (`Settings::max_body`).

## Approach

`logs` streams the log from its start to stdout through one buffer of 64 KiB, finds the start of `--tail N` from the end in pieces of the same size, and takes `--limit-bytes N`: stop after the first N bytes and exit 0. One parser of a whole number with a range reads `--tail` and `--limit-bytes`. The incus path takes `--limit-bytes` and `-f` with it; its memory bound is the one of `pod-incus`, and the help names it. Ask podcrates for the stream in `pod-supervise` and for a ranged read of a file in `pod-incus`.

## Decision

`--limit-bytes` composes with `--tail` (the first N bytes of the tail) and with `-f` (the follow ends at N bytes, or at the end of the container). `--since`, `--until`, `-t` and `--details` have no row in the parity table, so `logs` refuses them by name with 125. N is from 1 to 9223372036854775807 (the largest `off_t`); 0, a sign, a letter and a larger N are refused by name with 125.

## Prove

As uid 1000: a container whose log is 200 MB; the peak RSS of `podvm logs` under `/usr/bin/time -v` before and after; `--limit-bytes 1000` gives exactly 1000 bytes and exit 0. A table-driven test of the parse and of the combinations. Clause 25 of experiment 434 on the incus tier.

## Done

**Done (2026-11-20).** `logs` streams the log from its start to stdout through one buffer of 64 KiB (the module `stream` of `podvm-supervise`, now `pod_supervise::stream` of podcrates `v0.81.0`: `copy_range`, and `tail_start`, which finds the start of the last N lines from the end in pieces of the same size), on each path: a container, `-f` (the first read, then `pod_supervise::follow_from` from its end), `--tail` and `-f --tail`, and the serial log of a detached machine, which ignored `--tail` before. `--limit-bytes N` stops after the first N bytes with exit 0; with `--tail` it gives the first N bytes of the tail, and with `-f` the follow ends at N bytes (a writer that refuses each write past the bound ends `follow_from` at once) or at the end of the container. `--since`, `--until`, `-t` and `--details` have no row in the parity table and are refused by name with 125. One parser of a whole number with a range (`lifecycle::logs::whole`) reads `--tail` (0 to 2^64-1) and `--limit-bytes` (1 to 9223372036854775807); 0, a sign, a letter, an empty value and a larger N are refused by name with 125. On `--podvm-tier=incus` the verb takes the same parser, `-f` and `--limit-bytes` (`crates/podvm/src/incus/logs.rs`) and refuses `--tail` as before; the API of incus gives the file whole and `pod-incus` reads it whole, 64 MiB at most, so podvm holds the log in memory and refuses a larger one, and the help, the parity row, `docs/limits.md` and `docs/incus.md` say so. A write to a closed stdout ends with SIGPIPE (T-1769), and where SIGPIPE is ignored with exit 125 and one line (T-1816), never a panic. The unit tests `the_values_of_logs_are_parsed_by_one_rule`, `limit_bytes_composes_with_each_flag_of_logs`, `the_copy_of_a_log_is_the_slice_of_the_whole_read` and `the_help_states_the_bounds` hold the parse, the combinations and the copy; `the_tail_from_the_end_is_the_tail_of_the_whole_read` holds `tail_start` to `pod_supervise::tail_offset` for each text, count and size of piece; `the_log_of_an_instance_stops_at_the_limit` holds the incus path against a stand-in of the API. As uid 1000 with `mirror.gcr.io/library/alpine:3.20`, a container whose log is 200,000,000 bytes: `podvm logs` of `bf0608c` has a peak RSS of 200,160 KiB, and now 4,952 KiB with the same 200,000,000 bytes on stdout; `--tail 1` of the same log (no line end) 4,952 KiB; `--limit-bytes 1000` gives 1000 bytes and exit 0; `-f --limit-bytes 20` of a container that runs 30 s more ends after 0.3 s with 20 bytes and 0. Clause 25 of experiment 434 (`PODVM_434_MIRROR=1`) passes: `--limit-bytes 4` of a running instance gives `star` and 0, `-f --limit-bytes 3` ends with `sta` and 0, and `--limit-bytes 0` is refused by name with 125. The ask to podcrates: the stream of `stream.rs` in `pod-supervise` (`logs`, `follow_tail` and `append_from` read whole), and a ranged or streamed read of a file in `pod-incus`; the lead writes it as a COORD issue.

Amended (2026-11-21, [T-2066](deps.md), podcrates `v0.81.0`, the answer to #631). podvm takes `pod_supervise::stream` (`open`, `open_log`, `tail_start`, `copy_range`, `Capped`) and `follow_from_limit`, and its own module `stream` and its tests are gone: the tests of the stream are the crate's. `pod_supervise::logs` and `follow_tail` now open the log with `O_NOFOLLOW` too, so each read of a log of podvm refuses a symbolic link at the place of the log (podvm's `logs` did so before). `logs -f --limit-bytes N` is `podvm_supervise::follow_from_limit`. On `--podvm-tier=incus` each read is `Client::get_file_to(name, LOG, Some(ByteRange::from(seen)), left, out)`: the bytes from the offset that podvm wrote last, in pieces of 64 KiB, and no bound of 64 MiB; the help, the parity row, `docs/limits.md` and `docs/incus.md` say so. The unit test `the_log_of_an_instance_stops_at_the_limit` has rows for a log of 200,000 bytes through a client whose `max_body` is 4,096 (whole, to 70,000 bytes, and with `-f` to 65,537 bytes), and `the_help_states_the_bounds` holds that the help names no bound of MiB. Clause 25 of experiment 434 on route A is not run again for this take.

# T-2060: The sixth outside pass, OQ-06: `inspect --format` gives lists and maps as flat strings

**Source:** the sixth outside pass (T-2044), OQ-06 (P2)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`inspect --format '{{json .Config.Env}}'` gives one string of the variables, space-joined; `{{.Config.Cmd}}` gives `python3`, where docker gives `[python3]`; and `{{json .Labels}}` of a container gives `"team=ci"`. The JSON of a container gives `Labels`, `ExitCode`, `Pid` and the counts of `End` as strings, and has no `Config`. docker gives lists, maps and numbers, and the manual says that `--format` reads the same fields as `inspect`.

## Premise

The engine of `--format` (`crates/podvm/src/format.rs`) took each field as text. An image built its fields as flat text (`config_flat`) and its document as JSON by a second path. A container built its fields as text, and the document wrapped each of them as a JSON string (`json_of`).

## Approach

One model of the fields, typed as docker types them. The engine takes a JSON value for each field: `{{json .X}}` prints it as JSON, and `{{.X}}` prints it as Go does (`[a b]` for a list, `map[k:v]` for a map in key order, a dash for null). An image reads its fields from the one document that plain `inspect` prints (`inspect_doc`). A container types its text fields in one place (`lifecycle/typed.rs`), and plain `inspect` and `--format` both read that. A container has docker's `Config` (`Env`, `Cmd`, `Image`, `WorkingDir`, `Labels`); `Env` leaves out the `PODVM_*` variables and `LD_PRELOAD` of the entry, as `exec` does.

## Decision

-

## Prove

Two table tests hold each field type against docker's: `the_fields_of_an_image_have_dockers_types` and `the_fields_of_a_container_have_dockers_types`. As uid 1000: `inspect --format '{{json .Config.Env}}|{{.Config.Cmd}}'` of an image gives `["PATH=..."]|[/bin/sh]`; a container made with `--label team=ci` gives `{"team":"ci"}` for `{{json .Labels}}`, and a number for `{{json .Pid}}`.

## Done

**Done (2026-11-21).** The engine takes typed values (`format::render_typed`, `Typed`, `go_text`), and `render` and `render_with_documents` keep their text interface through it. `inspect` of an image builds one document, and `--format` reads its parts by their dotted names (`images/inspect.rs`: `inspect_doc`, `inspect_fields`); `config_flat` is gone. `inspect` of a container types its fields in `lifecycle/typed.rs`: `Pid`, `LauncherPid`, `ExitCode` and `End.*` are numbers (null where the record has none), `State.Running` is a boolean, `Labels` and `Config.Labels` are maps, `Config.Env` and `Config.Cmd` are lists, and `State` is docker's object in JSON and the word in text. `Config` and `Config.*` are new fields of both kinds. A dash in text is a null in JSON. Before (the binary of `3fa36a6`): `{{json .Labels}}` gave `"team=ci"`, `{{json .Pid}}` gave `"27332"`. After: `{"team":"ci"}`, `16763`, `{{.Labels}}` gives `map[team:ci]`, `{{.Config.Cmd}}` gives `[sleep 300]`, and an image of Alpine 3.20 gives `["PATH=..."]|[/bin/sh]`. The two table tests hold the types.

# T-2061: The sixth outside pass, OQ-09: docker differences of `-t`, `exec`, `logs`, `-v` and the manual

**Source:** the sixth outside pass (T-2044), OQ-09 (P2)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

Six differences from docker: `run -t` sets no `TERM` (docker: `xterm`); `HOSTNAME` is not set to the short id of the container, and the host name is the host's; `exec` into an exited container runs, and docker refuses it; `logs --tail all` is refused; `-v H:G:rw,z` is refused with a text that is false on the namespace rung; `start -a` and `cp NAME:PATH -` are refused as unknown.

## Premise

Each is a difference that the manual does not name. The refusal of `z` is checked at the parse, where the rung is not known.

## Approach

Fix the clear ones: `TERM=xterm` for `-t` where neither the image nor `-e` sets `TERM`; `exec` refuses a container that exited or died with `container NAME is not running (STATE)` and exit 1, the code of a refusal after the parse in `podvm-probe::exit`; `--tail all` is the whole log; `z` and `Z` are accepted and do nothing (podvm labels nothing for SELinux, and docker does nothing where SELinux is off). Name the rest in the parity table as known differences: `start -a`, `cp NAME:PATH -`, and `HOSTNAME`.

## Decision

-

## Prove

A table test for each rule: `a_tty_gets_term_xterm_unless_the_caller_set_one`, `exec_into_a_container_that_ended_is_refused`, `tail_all_is_the_whole_log`, and the table of `a_volume_spelling_podvm_cannot_serve_is_refused`. As uid 1000: `run -t` prints `TERM=xterm`; `exec` into an exited container exits 1 with one line.

## Done

**Done (2026-11-21).** `TERM=xterm` (`run/home.rs`: `tty_term`), the refusal of `exec` into an exited or dead container (`exec/ns.rs`: `not_running`, exit 1; a created container keeps the entry of T-0505), `--tail all` (`lifecycle/logs.rs`), and `:z`, `:Z` as no-ops (`lifecycle/volume.rs`). Left, and named as known differences in the parity table (so in `podvm man`): `start -a` and `cp NAME:PATH -`, which need an attach (the follow of `logs`, `wait` and the signals) and a tar writer that are not small; and the host name of a run with no `--hostname`, which is the host's, with no `HOSTNAME`, because a foreground run has no container id at the entry. Before (`3fa36a6`): `exec c1 echo into-exited` gave `into-exited` and 0; `--tail all` gave 125; `-v ...:rw,z` gave 125. After: 1 with the line "podvm exec: container c1 is not running (exited)" and the way to start it; the whole log; 0.

# T-2062: The sixth outside pass, OQ-10: exit codes are not the documented ones

**Source:** the sixth outside pass (T-2044), OQ-10 (P2)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

An E2BIG of `execve` (a 1 MB value of `--env-file`) exits 127, and the manual says that 126 is the command found and not invocable. The manual says that a refusal after the flags parsed exits 1, and `rmi` of a referenced image, `load` of a tampered or unreadable archive, `import` of a `.gz` and `cp CONTAINER:/../../etc/shadow` exit 125.

## Premise

`pod-enter` reads E2BIG as "not found" (`invocable_but_refused` names five errnos). Each refusal that exits 125 set `EXIT_RUNTIME_ERROR` by hand, beside the table of `podvm-probe::exit`.

## Approach

One table: `podvm-probe/src/exit.rs` (`CASES`, and the code of the manual's `GLOBAL`). `cannot-invoke` holds E2BIG, and `cli-error` names the refusals above. The code follows the table: `podvm_enter::ExitCode` gives 126 for an E2BIG; `rmi`, `cp` and the archive verbs give 1. A failure of the store stays 125. The ask for podcrates (#639, COORD-225): `invocable_but_refused` takes E2BIG, and podvm then drops its read of the text.

## Decision

-

## Prove

`errors_map_to_dockers_codes` (E2BIG gives 126) and `a_refused_archive_exits_with_the_code_of_the_table`. As uid 1000: `run --env-file` with a value of 1 MB exits 126; `rmi` of a referenced image, `load -i /etc/shadow`, `import a.gz` and `cp c:/../../etc/shadow x` exit 1.

## Done

**Done (2026-11-21).** The refusals after the parse exit 1: `rmi` of an image that a container uses, the gate of `cp` (`the copy is refused`, and a link in the path), and `load` and `import` through one function (`images/save_load.rs`: `archive_exit`, a refused archive or a file of the caller that does not read gives 1, a failure of the store 125). `podvm_enter::ExitCode::exit_code` gives 126 where `pod-enter` reports "not found" with E2BIG in its text (a stand-in until podcrates takes E2BIG; the lead writes the COORD issue). `CASES` names both. As uid 1000 with `alpine:3.20`, before (`3fa36a6`) and after: `run --env-file` with a value of 1 MB gave 127, and now gives 126; `rmi` of an image that a container uses, `load -i /etc/shadow`, `import` of a `.gz` and `cp c:/../../etc/shadow x` gave 125, and now give 1.

Amended (2026-11-23, [T-2066](deps.md)): podcrates `v0.82.0` (`pod-enter`, the answer to #639) gives `Error::CannotInvoke` for an E2BIG of `execve`, and for `EINVAL` and `ELIBBAD`, and keeps a refused errno across the search of `PATH`. `podvm_enter::ExitCode::exit_code` no longer reads the text `E2BIG (7)`, and `errors_map_to_dockers_codes` no longer holds that text; the row `cannot-invoke` of `CASES` names each errno of `invocable_but_refused` of the crate. On the cloud host, the build of the take with a value of 200,000 bytes in `--env-file` and an image whose `/bin/prog` is a script: `run p:1 prog` and `run p:1 /bin/prog` exit 126 with "execve of every candidate path failed with E2BIG (7)", as root and as uid 1000 (the namespace rung, with and without `PODVM_USERNS=0`). The test of each rung is podcrates' `exec_refused.rs`: the loader family does not take a script payload, so a podvm test of the run would prove a different rung on each host.

# T-2063: The sixth outside pass, OQ-12: a SIGKILL of podvm leaves `staging/memo-*.tmp`

**Source:** the sixth outside pass (T-2044), OQ-12 (P3)
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

A SIGKILL of podvm leaves `store/staging/memo-<pid>-<n>.tmp`, and `rm -f` and `container prune` do not remove it.

## Premise

The memo of a foreground run is a file of `staging/` that the run removes when it ends. The sweep of `staging/` took only `*.partial`. A pid is not a proof of an owner across PID namespaces, so the pid in the name cannot decide.

## Approach

A run takes an exclusive `flock` on its memo when it makes the name (`interpose::ephemeral_memo_path`), close-on-exec, until the process ends, however it ends. `Store::sweep_staging` removes a `memo-*.tmp` that it can lock, and that is 10 s old (a file between its creation and its lock is young); `container prune` removes one that no lock holds at once.

## Decision

-

## Prove

`the_sweep_removes_the_memo_of_a_dead_run_and_only_that`: one table of lock, age and name. As uid 1000: `kill -9` of a foreground `run`, then `container prune`: no `memo-*.tmp` stays.

## Done

**Done (2026-11-21).** `Store::sweep_staging` and `sweep_staging_aged` (`podvm-image/src/store/blobs.rs`) take `memo-*.tmp` by the lock, and each `podvm` command that opens the store sweeps with an age of 10 s; `container prune` sweeps with none. A live run holds the lock (`interpose::hold_memo`), a payload does not (close-on-exec). The table test holds the rule for a locked file, a young file, a name that is not a memo, and a `.partial`.

# T-2064: The sixth outside pass, OQ-13: texts of the manual and of messages

**Source:** the sixth outside pass (T-2044), OQ-13 (P3)
**Category:** cli
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

The section of `container prune` in the manual says that it is no command; it is one. The manual says that `--read-only` is refused outside the loader family, and it works on the namespace rung. `--timeout 99999999999999999999` says "takes whole seconds over 0". `--report tmp/nodir/r.json` says that the directory "takes no file of the caller". The lines for a forwarded signal and for a signal exit of the payload have no `podvm` prefix. `--env-file` with bytes that are not UTF-8 names no file and no line. (The version of the candidate was a build before the bump, and is not a fault.)

## Premise

Each text was written for the first case that the author saw. `pod-enter` writes the two lines of the wait to a writer that the caller gives, with no name of the program.

## Approach

Each text says what is true. A table test holds the refusals of a value (`a_refused_value_says_what_is_wrong_with_it`). A writer that starts each line with `podvm: ` wraps the writer of each wait (`podvm_enter::wait_forwarding`).

## Decision

-

## Prove

`a_refused_value_says_what_is_wrong_with_it`, `the_lines_of_the_wait_start_with_the_name`; `podvm man container prune` shows the usage; the help and the parity row of `--read-only` name the namespace rung.

## Done

**Done (2026-11-21).** `--timeout` names the bound (`run/parse/bound.rs`: from 1 to 18446744073709551 seconds, and a number of digits over it is too large); the directory of `--report` that is not there says `does not exist`; `--env-file` names the file, the line and the byte; the lines `forwarded SIGINT to the payload (pid N)` and `the payload died on SIGINT: exit 130` start with `podvm: `; the section of `container prune` shows its usage (`man.rs`); `--read-only` is documented for the namespace rung.

# T-2052: `run -it`: the interrupt key does not stop the foreground child of the payload

**Source:** the sixth outside pass ([T-2044](cli.md)), OQ-05 (P2)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

In a pty, `run --rm -it IMAGE sh -c 'echo ready; sleep 6'` and a `^C` after 3 s: the run ends after the `sleep` (about 6 s), and a trap of `sh` runs only then. With `-t` and no `-i` the same key ends the run at once. The manual: "a SIGINT or SIGQUIT of the terminal reaches the payload once".

## Premise

The slave of the pty is not the controlling terminal of the payload (pod-enter makes no session; COORD-188 asks for one), so the line discipline signals no process. With `-i` the terminal of the caller is raw, and podvm takes the `VINTR` byte out of the input and sends SIGINT to the pid of the payload alone (`run/pty.rs`, T-1970). A shell runs its trap after its foreground command ends, and `sleep` got no signal. With no `-i` the terminal of the caller stays cooked: its key goes to the whole process group of podvm, which holds the payload and its children.

## Approach

Send the signal of a key as a terminal does: to each process of the tree of the payload that is in the process group of the payload. Read the tree in the procfs whose pids are the pids of the payload.

## Decision

2026-11-21, the session's call. The keys are `VINTR` (SIGINT) and `VQUIT` (SIGQUIT), read from the pty where its `ISIG` is on; `VSUSP` stays a byte of the input, as no job control can continue a stopped payload. A process of podvm in the tree (the relay of `exec` that stays in a PID namespace) gets no signal: it forwards one, and the payload would take it twice. A process that left the group (`setsid`) gets none, as from a terminal. Where no procfs gives the tree, the payload alone gets the signal, as before.

## Prove

In a pty, as root and as uid 1000 with a subordinate range, on the namespace rung: `run --rm -it` and `exec -it` into a `run -d` container of `sh -c 'trap "echo got-INT; exit 9" INT; echo ready; sleep 6'`, `^C` after 3 s: the trap runs at once and the exit is 9. A table-driven test of the targets of a key.

## Done

**Done (2026-11-21).** `crates/podvm/src/run/pty/keys.rs` (new) gives the targets of a key: the payload and its descendants (`pod_supervise::launcher::tree::descendants`) in the process group of the payload, less each process whose program is podvm's (`/proc/PID/exe` against `/proc/self/exe`), else the payload alone; `run/pty.rs` reads `VINTR` and `VQUIT` of the pty (`keys_of`) and sends their signals to those targets. The procfs is `/proc`, and for `exec` that joined the PID namespace of a run (T-1910) the `/proc` of the run through the root of its payload (`exec::tty::proc_of`, `userns::joined_root`, `/proc/PID/root/proc` with the host pid of the join): there the `/proc` of podvm names the pids of the host, and the pid of the payload is a pid of the run. Measured on the cloud host with `mirror.gcr.io/library/alpine:3.20` and a `^C` (0x03) 3 s after the start, in a pty (`script` of Python): before (the binary of `main` at `3fa36a6`), `run -it` gave `got-INT` after 6.2 s (uid 1000) and 6.0 s (root), and `exec -it` into a `run -d` container after 5.2 s and 5.0 s; after, 3.4 s, 3.1 s, 2.0 s and 2.0 s, each with exit 9, and with no trap `the payload died on SIGINT: exit 130` after 3.2 s; `-t` with no `-i` gave the trap at once before and after. The unit tests `a_key_signals_the_group_of_the_payload_in_its_tree` (six shapes of a tree), `the_group_is_read_after_the_last_parenthesis`, `the_tree_of_a_live_process_names_its_child` and `the_keys_of_a_fresh_pty_are_intr_and_quit` hold the rule. The parity row of `run -t` names the keys, and no longer says that `tty` finds no name on the namespace rung: the rung mounts a `/proc`, and `tty` prints `/dev/pts/N` as root and as uid 1000.

At the merge of `wip/oq-run` (2026-11-23, call C29), the cuts of its size commit (2026-11-21, call C24) in the key relay: `keys_of` gives a fixed pair, where byte 0 is no key, and `tree_of` adds the payload once after its descendants. Both stay: the code is simpler and the behaviour is the same. The cut also put the choice of the second procfs on `userns::joined` in place of a test of `self` in each procfs (`own_proc`). Measured at the merge as uid 1000, with `exec -it` into a `run -d` container: both forms gave the trap after 6.1 s, not 3 s. The process of the join is a member of the PID namespace of the run but not of its mount namespace: `self` of the `/proc` of the run is `ENOENT` for it, and that `/proc` is mounted only in the mount namespace of the run, so the path of the root in the store shows an empty directory and the key went to the payload alone. The fix: the join keeps the host pid of the payload of the run (`join::JOIN_PID`), and the key reads `/proc/PID/root/proc`. After, on the cloud host with `mirror.gcr.io/library/alpine:3.20` and a `^C` 3 s after the start, twice each: `run -it` and `exec -it`, as root and as uid 1000, each gave `got-INT` after 3.0 s with exit 9.

# T-2053: The banner, `--strict` and `inspect` of the namespace rung deny the PID namespace and the `/proc` that the run has

**Source:** the sixth outside pass ([T-2044](cli.md)), OQ-07 (P2); PodUtils/podvm#557 (the mtab of `--strict`, T-1953)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** done

## Problem

On the namespace rung, `run --verbose` says "this mode does NOT provide: user, pid or network isolation, or a /proc filesystem" and "no /proc is mounted in the payload root", and then "podvm opened a user, a PID and a UTS namespace for this run ... and the payload has its own /proc". `--strict` warns "/proc is not mounted in the root" (the mtab fixup). In the payload, `mount` lists `proc on /proc` and `ps` shows a private PID table. `inspect --format '{{.Exec.Mode}} | {{.Exec.Shares}}'` of a container gives `fresh-chroot | filesystem`, and `exec c1 ps` lists the processes of the container.

## Premise

Each text came from the name of the rung (`Rung::must_never_claim`, `Provides::of` of `pod-hostprobe`, the `/proc` sentence of `podvm_probe::report`, the constants `EXEC_MODE` and `EXEC_SHARES`), not from the run. What a run of the rung opens depends on the caller and the verb: podvm's user, PID and UTS namespace for a foreground run of uid 1000 (T-1909), pod-enter's user namespace and the PID namespace of `Plan::pid_namespace` for `run -d` (T-1930), a mount namespace only for uid 0; pod-enter mounts a `/proc` where the image has the directory. podvm never set `entry_mounts_proc` of the completion (`pod-rootfs`, #557).

## Approach

One table of the parts of an entry (user, pid, uts, net, mount, proc), and the facts of one entry, from which the banner, `--strict`, `inspect`, the line of `exec` and the manual read.

## Decision

2026-11-21, the session's call. The table is `podvm_probe::isolation` (`ROWS`, `Opened`, `of_entry`). The "does NOT provide" words of the namespace rung stay those of `pod_hostprobe::select::Rung::must_never_claim_with` (rule 2a): the table gives it the parts. The record keeps the words of the parts (`Hardening::opened`, optional, so an older record parses); a record of the rung with none reads as its mount namespace. `.Exec.Mode` of a container of the namespace rung is `join`, and `.Exec.Shares` is `filesystem` and each namespace that `exec` into the running container joins (the UTS namespace only with the user namespace: podvm's join of T-1910 takes it, pod-enter's does not). The other rungs keep `fresh-chroot | filesystem`.

## Prove

As root and as uid 1000 with `public.ecr.aws/docker/library/alpine:3.20`: `run --verbose` names the parts that the run opens and denies none of them; `run --strict` gives no mtab warning; `inspect` of a `run -d` container gives `join` and its namespaces; `podvm man` has the table. A table-driven test of each shape of an entry.

## Done

**Done (2026-11-21).** `crates/podvm-probe/src/isolation.rs` (new) holds the table and `of_entry` (the facts: podvm's user namespace, uid 0 of the host, `run -d` or `create`, `--hostname`, a `/proc` directory in the image); `podvm_probe::report::entry_banner_of` takes the `namespaces:` and `pids:` cells, the "does NOT provide" line, the `/proc` sentence, and a line where the `/proc` of the payload lists the processes of the host, from it; `run/acquire.rs` gives the facts of the run, sets `entry_mounts_proc` of the completion, and keeps the words in the record; `lifecycle/copy_tree.rs` gives `.Exec.Mode` and `.Exec.Shares`; `exec` into a running container of the rung says that it joins (`exec::joined_line`); `podvm man` has an ISOLATION section from the table; the parity row of `exec` no longer says "never an entry into a running container's namespaces". Measured on the cloud host with `mirror.gcr.io/library/alpine:3.20`. Before (`main` at `3fa36a6`), as uid 1000 and as root: `namespaces: mount-only`, `pids: host-shared`, "does NOT provide: user, pid or network isolation, or a /proc filesystem", "no /proc is mounted", the mtab warning of `--strict`, and `fresh-chroot | filesystem`. After, as uid 1000: `namespaces: user+pid+uts+mount`, `pids: private`, "does NOT provide: network isolation", no `/proc` sentence, no warning of `--strict`, and `join | filesystem+user+pid+mount` for a `run -d` container; as root: `namespaces: mount-only`, `pids: host-shared`, "does NOT provide: network isolation, or user isolation, or pid isolation", a line that the `/proc` of the payload lists the processes of the host, no mtab warning, and `join | filesystem+pid+mount`. The unit tests `each_statement_of_an_entry_reads_its_facts` (eight shapes: each cell, the words, the record, `.Exec.*` and the line), `the_manual_names_each_row_and_each_shape`, `an_unknown_word_of_a_record_is_left_out` and `the_namespace_banner_reads_what_the_entry_opened` hold the rule. Not changed: the reason of the built-in profile of Chromium (`profile/builtin.rs`) still says that the namespace rung mounts no `/proc`; the profile keeps the loader family for its other reasons.

The cuts of the size commit of `wip/oq-run` in the table (reviewed at the merge, 2026-11-23). Kept: the banner has no line "this run opens: ..." that repeats the `namespaces:` cell in long words; its one added line says that the `/proc` of the payload lists the processes of the host where the run has no PID namespace (`Opened::line`). Reverted: the rows as a `match` and the shapes as a function (the cut was for the relocations only; a static table `ROWS` and `SHAPES` is the table that the manual reads), the words of the record as one string (`Hardening::opened` stays a list of words, as JSON gives a list), and the shorter texts of the manual, of `exec`, and of the parity rows of `exec` and `run -t`.

# T-2070: The faults of the seventh outside pass (2026-11-23, the gate of the next beta)

**Source:** the seventh outside pass (2026-11-23, the gate of the next beta): an agent with the release binary of `4887fb72` and `podvm man` only, as uid 1000 through `setpriv`, with a store root that root owns with mode 777, for about one hour
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** partial

## Problem

The pass found 18 faults, OR-B38-01 to OR-B38-18. The tester gave OR-B38-01 P1, OR-B38-02 to OR-B38-10 P2 and OR-B38-11 to OR-B38-18 P3; the lead made OR-B38-06 P1 and OR-B38-09 P3. The rule of the gate: no P0 or P1 finding stays open.

## Premise

Each finding was reproduced at least twice by the pass, on the binary of `4887fb72` (`--version` 0.2.0-beta.37).

## Approach

Fix the two P1 findings and the P2 findings that the lead marked "fix"; give OR-B38-05 to T-2069; keep each other finding in the table below, as a line of "Known gaps" of the release.

## Decision

-

## Prove

`sh scripts/dev/checks.sh` passes with a test of the class of each fixed finding; the reproducer of each fixed finding, run again on the new release binary as uid 1000, gives the docker result; each other finding is a line in "Known gaps" of the release.

## Done

**Partial (2026-11-23).** The state of each finding:

| Finding | Sev | Triage | Entry | State |
| --- | --- | --- | --- | --- |
| OR-B38-01, `exec -it CTR sh` takes no input after the first chunk | P1 | fix | [T-2071](cli.md) | done |
| OR-B38-02, a foreground `run` reports the owner of the store for a name collision or a corrupt `containers.json` | P2 | fix | [T-2073](cli.md) | done |
| OR-B38-03, each `exec` rewrites `/etc/hosts` and drops `--add-host`; `run -d --hostname hx` maps 127.0.0.1 to the name of the container | P2 | fix | [T-2075](cli.md) | done |
| OR-B38-04, `run -d --rm` keeps the container after its end | P2 | fix | [T-2074](cli.md) | done |
| OR-B38-05, a killed `pull` or `load` leaves a `.partial` in `store/staging` that no verb removes or counts | P2 | T-2069 | [T-2069](image.md) | done: the next command after 10 s removes it (T-2069) |
| OR-B38-06, `-v DIR:/etc` and `-v DIR:/dev` write podvm's fixups into DIR | P1 (lead) | fix | [T-2072](cli.md) | done |
| OR-B38-07, `cp -r` onto an existing copy of a tree with symbolic links fails half way ("is a symlink; writing through one is refused") | P2 | known gap | - | open |
| OR-B38-08, `login` prints "Login Succeeded" for a registry that it did not contact | P2 | known gap | - | open |
| OR-B38-09, `run -h hx IMAGE CMD` prints the usage (docker's `-h` is `--hostname`) | P3 | by design: the manual says that each verb takes `-h` as help | - | open |
| OR-B38-10, `probe` says mount only, no user or PID isolation and no `/proc`, where `run` enters user+pid+uts+mount with `/proc` | P2 | known gap | - | open |
| OR-B38-11, `--env-file`: a line with no `=` refuses the run (docker passes the host value); a UTF-8 BOM on line 1 stays | P3 | known gap | - | open |
| OR-B38-12, `run -d IMAGE /nonexistent` exits 125, a foreground run 127 (docker: 127) | P3 | known gap | - | open |
| OR-B38-13, `run` with no `-i` passes stdin; `run -it` with a stdin that is no terminal is accepted with no line (docker refuses) | P3 | known gap | - | open |
| OR-B38-14, `cp` prints paths of the store; `cp FILE CTR:/missing/dir/file` makes the parents (docker: an error) | P3 | known gap | - | open |
| OR-B38-15, gaps of docker's `--format`, `--filter` and flags, each refused with a true line | P3 | documented | - | open |
| OR-B38-16, `--tmpfs P:mode=1777` drops the mode with no line | P3 | known gap | - | open |
| OR-B38-17, `run -it sh` and `exec -it CTR sh`: "can't access tty; job control turned off" (the pty is no controlling terminal) | P3 | known gap | - | open |
| OR-B38-18, texts: 14 spaces in the note of an exec into a created container; `/proc/mounts` of `--read-only`; `--cap-drop ALL`; `pull http://x/y` names registry-1.docker.io; `system df --verbose`; `podvm help run`; `system df` after a refused extraction; the text of a refused hard link; `ps -a` on a store with no write; `rmi -f`; `pull -a` and schema 1; `HOSTNAME` and `hostname` with no `--hostname` | P3 | known gap | - | open |

Proof of the fixes (2026-11-23, on `160c5567`): `sh scripts/dev/checks.sh` passes; `cargo check --target aarch64-unknown-linux-musl --workspace` passes; `cargo test --workspace --no-fail-fast` gives 1136 passed, 0 failed, 1 ignored. The release binary is 9 050 328 bytes (the ceiling is 9 500 000). The reproducers of the pass, as uid 1000 through `setpriv` with a store root of root with mode 777 under `/var/tmp`, on the binary of the pass and on the new one: OR-B38-01, `exec -it ex1 sh` through a pty: no input after the prompt, now `echo one` to `exit 0` run and exit 0, and `sh -c 'set -m; cat'` gives each line; OR-B38-02, the second `run --name dup`: the line of the owner, now "the name \"dup\" is already used", and a cut `containers.json` gives "does not parse ... move it aside"; OR-B38-03: the `/etc/hosts` after an `exec` had no `1.1.1.1 a` and no `127.0.0.1 hx`, now both; OR-B38-04: `ps -a` listed the container of `run --rm -d` after its end, now not; OR-B38-06: `-v DIR:/etc` left six entries of podvm in DIR and `-v DIR:/dev` ten, now DIR holds its own file alone. Left: the findings of the table that are open.

# T-2071: `exec -it CTR sh`: a job-control shell takes the caller's terminal, and the relay gets no more input

**Source:** the seventh outside pass (T-2070), OR-B38-01 (P1)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

As uid 1000, `exec -it CTR sh` into a `run -d` container shows the prompt and then takes no input: `echo one` is not echoed and does not run. `exec -it CTR sh -i` is the same; `exec -it CTR sh -c 'set -m; cat'` gives the first line and no second; `exec -it CTR sh +m`, `exec -it CTR cat` and `run -it IMAGE sh` work.

## Premise

-

## Approach

Find what stops the relay of the pty when a job-control shell starts, and give the payload no way to the caller's terminal.

## Decision

-

## Prove

A test that types three lines into a job-control shell (`sh`, `sh -i`, `sh -c 'set -m; cat'`) through the pty of `exec -it` and `run -it`, from a terminal of its own, and gets each answer in order, as root and as uid 1000; the reproducer of the pass on the new release binary.

## Done

**Done (2026-11-23).** The cause: the payload of `exec` was a member of podvm's session, whose controlling terminal is the caller's terminal, and a `run -d` of a user namespace binds the host's `/dev/tty` in its tree (the device binds of T-1909). A job-control shell opened `/dev/tty`, which is the controlling terminal of its session, so the caller's terminal, and made its own process group the foreground group of that terminal (measured on the binary of the pass: the shell's `tpgid` is its own pgrp, its session is podvm's, its descriptor 10 is `/dev/tty`). podvm's relay thread, then in a background group, got no more input from the caller's terminal. The fault is not a regression of T-2052: the release binary of `v0.2.0-beta.37` has it too (measured). The fix (`crates/podvm/src/run/pty.rs`, `leave_the_session`): where podvm relays stdin to the pty (`-i` with `-t`), the process that spawns the payload leaves the caller's session (`setsid`) before the spawn, so the payload has no controlling terminal: `/dev/tty` gives `ENXIO` and the shell turns job control off, as on each other rung ("can't access tty; job control turned off", OR-B38-17). The caller's terminal is raw then and sends no signal, so the move changes no signal; with `-t` and no `-i` the session stays, as the interrupt key of the cooked terminal must reach podvm's group (OR-03). `setsid` fails where podvm leads its process group (a foreground run of root from a shell); no rung of such a run binds `/dev/tty`. The integration test `a_job_control_shell_on_the_pty_of_exec_it_and_run_it_gets_each_line` (`crates/podvm/tests/tty_job_control.rs`, an image of the host's `dash`, `cat` and `sleep`) is the caller's terminal: it opens a pty, makes it the controlling terminal of podvm's session, and types three lines into `exec -it CTR sh -i`, `exec -it CTR sh`, `exec -it CTR sh -c 'set -m; cat'`, `run --rm -it IMAGE sh -i` and `run --rm -it IMAGE sh -c 'set -m; cat'`; each answer must come in order, and the exit code of the end. It passes as root and as uid 1000; as uid 1000 with the `setsid` taken out it fails on each `exec` case after the first line. As uid 1000 with `alpine:3.20`, `exec -it ex1 sh` through a pty: the binary of the pass and of `v0.2.0-beta.37` show the prompt and take no input; the new binary runs `echo one`, `echo two`, `echo three` and `exit 0`, and a Ctrl-C ends `sleep 30` and the shell goes on.

# T-2072: `-v DIR:/etc` and `-v DIR:/dev` write podvm's fixups into DIR

**Source:** the seventh outside pass (T-2070), OR-B38-06 (P2; P1 by the lead)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** S
**Status:** done

## Problem

As uid 1000, `run --rm -v $PWD/vol:/etc alpine true` leaves `group`, `hosts`, `mtab`, `passwd`, `resolv.conf` and `ssl/certs/ca-certificates.crt` in `vol`, made by podvm (216 KiB). `-v DIR:/dev` leaves `null`, `zero`, `random` and `urandom` as regular files (1 MiB each for three), the links `stdin`, `stdout`, `stderr` and `fd`, and `pts/` and `shm/`. Docker writes no such file into a bound host directory.

## Premise

-

## Approach

podvm never writes a file of its own through a volume into a host directory. Find each rung where a fixup goes through a volume, and keep the fixups in the tree there.

## Decision

The payload sees the directory of the volume as the caller left it: a volume over `/etc` hides podvm's `hosts`, `hostname` and `resolv.conf`, and a volume over `/dev` hides the device nodes. Docker binds its own `hosts`, `hostname` and `resolv.conf` over a volume of `/etc`; podvm has no mount on each rung (the userland families, the syscall tier), so it writes none of them and names the difference. podvm does not refuse `-v DIR:/dev`: docker takes it. `--verbose` names each fixup that a volume covers in one line.

## Prove

A table-driven test over the fixup targets and a volume over `/etc`, `/etc/ssl`, `/etc/ssl/certs`, `/dev` and `/dev/pts`: the host directory gets no file that podvm wrote, as root and as uid 1000; the reproducer of the pass on the new release binary.

## Done

**Done (2026-11-23).** The cause: on the namespace rung of a foreground run in podvm's own user namespace (a caller with no root), `run::acquire::hostname_env` bound the volumes into the tree (`userns::bind_volumes`) before `complete::prepare` and before `/etc/hostname`, so each write of the completion went through the bind into the host directory. A detached run (pod-enter binds at the launch), an `exec` (measured: no file), the userland families (a path table, no bind) and the rungs of root (pod-enter binds at the entry) wrote into the tree. The fix: the bind is `run::binds::bind`, called after the completion (`crates/podvm/src/run/prepare.rs`); the volumes at or under `/tmp` stay pod-enter's (OQ-01). A working directory in a volume is the volume's, as before (`make_workdir`). `run::binds::covered` gives one line of the banner for each volume that covers a fixup ("podvm: -v DIR:/etc covers etc/resolv.conf, etc/hosts ... of the tree: the payload sees the directory of the volume there, and podvm writes no file into it"); the unit test `a_volume_names_each_fixup_that_it_covers` holds the rule for each target and volume. The integration test `a_volume_over_a_fixup_target_gets_no_file_of_podvm` (`crates/podvm/tests/volume_fixups.rs`) seeds a directory with one file for each case (`/etc`, `/etc` with `--hostname` and `--add-host`, `/etc/ssl`, `/etc/ssl/certs`, `/dev`, `/dev/pts`) and checks that it holds that file alone after the run. It passes as root and as uid 1000; as uid 1000 with the bind before the completion it fails on five of the six cases. As uid 1000 with `alpine:3.20`: on the binary of the pass, `-v DIR:/etc` left six files and `-v DIR:/dev` eleven entries in DIR; on the new binary DIR holds its own file alone, and the payload runs (exit 0 for `/dev` and `/etc/ssl`; with `/etc`, `cat /etc/hostname` finds no file).

# T-2073: A foreground `run` reports the owner of the store for each failure of its record

**Source:** the seventh outside pass (T-2070), OR-B38-02 (P2)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

As uid 1000 with a store root that root owns with mode 777, `run --name dup IMAGE true` twice gives "the run could not be recorded: STORE is owned by uid 65534, and this process runs as uid 0; use a store of your own ($PODVM_STORE)" and 125. `run --rm --name dup` while `dup` runs, and a foreground run with an empty or cut `containers.json`, give the same line. `ps -a`, `run -d` and `create` give the true lines. The refusal of a `containers.json` that does not parse has no remedy, where the one of `store.json` says "move it aside".

## Premise

-

## Approach

Each failure gives one true line (`AGENTS.md`, 2b): the line of the owner only where the owner is the cause.

## Decision

-

## Prove

`cargo test -p podvm --test record_failure`: a table test over the causes (a name in use, a table that is empty, cut or not JSON) with a store root of another uid and mode 777: each gives its cause, with 125, and never the line of the owner; the reproducer of the pass on the new release binary.

## Done

**Done (2026-11-23).** The cause: `run::foreground::record::owner_refusal` (T-1851) gave its line for each failure of `podvm_supervise::create` where `containers.json` or the store root belonged to another uid, and did not look at the failure. Under a store root of root with mode 777, the user namespace of a foreground run shows root as uid 65534 and podvm as uid 0, so each failure named the owner. The fix: the line of the owner comes only where that file denies this process the access that the record needs (`access(2)`: read of `containers.json`, write and search of the root, as the table is replaced by a rename there); else the caller prints the failure of the record. `podvm_supervise::Error` from a failure of `pod-supervise` adds the remedy to the refusal of a table that does not parse: "; move it aside to start an empty table (podvm then knows no container of it)" (`TABLE_REMEDY`). The unit test `the_owner_is_the_cause_only_where_it_denies_the_access` holds the rule for each uid and each refused access; `a_table_that_does_not_parse_names_the_remedy` (`podvm-supervise`) holds the remedy for an empty, a cut, a non-JSON table and one with no version; the integration test `a_failed_record_of_a_foreground_run_names_its_cause` (`crates/podvm/tests/record_failure.rs`) gives the store root to uid 65534 with mode 777 as root and checks the line of a name in use and of three bad tables (125, one line, no owner). As root with a store root of uid 65534 and mode 777, the binary of the pass gives the line of the owner for a name in use, and the new binary "--name \"dupy\": the run could not be recorded: the name \"dupy\" is already used by a container".

# T-2074: `run -d --rm` keeps the container after its end

**Source:** the seventh outside pass (T-2070), OR-B38-04 (P2)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`id=$(podvm run --rm -d IMAGE sh -c 'sleep 2; echo bye'); sleep 4; podvm ps -a` lists the container `Exited (0)`; `logs` works; the run exits 0 with no line. Docker removes the container at its end.

## Premise

`pod-supervise` removes the record and the directory of a container after the end of its run where the record has `rm` (`pod_supervise::auto_remove`, PodUtils/podvm#568); podvm did not set it.

## Approach

Set `rm` of the record for `--rm` of `run -d` and `create`, and put the tree of the container aside after the removal.

## Decision

-

## Prove

`cargo test -p podvm --test detached_rm`: `run -d --rm` (with and without `wait`) and `create --rm` with `start` leave no record and no tree of the container after the end, as root and as uid 1000; the reproducer of the pass on the new release binary.

## Done

**Done (2026-11-23).** The cause: `run -d` (`crates/podvm/src/run/foreground.rs`) and `create` (`crates/podvm/src/lifecycle.rs`) parsed `--rm` and did not give it to the record. The fix: both call `lifecycle::rm_at_end`, which sets `rm` of the record (`podvm_supervise::auto_remove`, over `pod_supervise::auto_remove`) before the start. The launcher of `pod-supervise` removes the record and the directory of the container after it writes the end and answers each waiter (so `wait` gets the code), and podvm's part of the launcher puts the tree of the container aside when it drops what it kept (`launcher::Discard`, last of `Kept`), as `podvm rm` does. The parity row of `--rm` says so. The integration test `rm_of_a_detached_run_removes_the_container_at_its_end` (`crates/podvm/tests/detached_rm.rs`) holds the rule for `run -d --rm`, `run -d --rm` with `wait` (the code 3) and `create --rm` with `start`: `ps -a` does not list the container after its end, and the store holds no more trees than before. It passes as root and as uid 1000. As uid 1000 with `alpine:3.20`: on the binary of the pass the container stays `Exited (0)`; on the new binary `ps -a` does not list it 4 s after, its directory and its tree are gone, `wait` of `run -d --rm --name rmw ... exit 3` prints 3, and `create --rm` with `start` leaves no record.

# T-2075: Each `exec` rewrites `/etc/hosts` and drops the lines of `--add-host` and `--hostname`

**Source:** the seventh outside pass (T-2070), OR-B38-03 (P2)
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`run -d --name ah --add-host a:1.1.1.1 IMAGE sh -c 'cat /etc/hosts; sleep 3; cat /etc/hosts; ...'` with an `exec ah true` in between: the first `cat` shows `1.1.1.1 a`, the second only the lines of localhost and `127.0.0.1 localhost ah`. `exec ah cat /etc/hosts` never shows it. With `run -d --hostname hx`, `exec` shows 127.0.0.1 mapped to the name of the container and not to `hx`.

## Premise

The completion of `exec` rewrites `/etc/hosts` from its own inputs: the name that the caller typed and the `--add-host` of `exec`.

## Approach

`exec` writes the content of the run: the record keeps the parts of `/etc/hosts` of the run.

## Decision

-

## Prove

`cargo test -p podvm --test exec_hosts`: three `exec` into a `run -d --hostname hx --add-host a.example:192.0.2.1` container each read both lines, as root and as uid 1000; a table test of the parts that `exec` gives the completion; the reproducer of the pass on the new release binary.

## Done

**Done (2026-11-23).** The cause: `exec::enter` gave the completion the name that the caller typed (`ask.container_name = target`) and the `--add-host` of `exec`, and `pod_rootfs` rewrites `/etc/hosts` from them, so the lines of the run went, for each process of the container. The run writes them right: `run -d --hostname hx` has `127.0.0.1 hx`; the pass saw the file after an `exec`. The fix: the record of a run keeps the parts of its `/etc/hosts` (`Hardening::hosts`: the name on the line of 127.0.0.1 and each `NAME:ADDRESS` of `--add-host` and `--hostname`), and `exec::ns::hosts_of` gives them to the completion of `exec`, with the `--add-host` of `exec` after them; a record of an older podvm keeps the old rule. The unit test `exec_writes_the_hosts_of_the_run` holds the rule for a record with and without the parts and an `exec` with and without `--add-host`; the integration test `exec_keeps_the_hosts_of_the_run` (`crates/podvm/tests/exec_hosts.rs`) passes as root and as uid 1000. As uid 1000 with `alpine:3.20`, `run -d --hostname hx --add-host a:1.1.1.1` and an `exec ah2 true`: the payload's second `cat` and `exec ah2 cat /etc/hosts` show `1.1.1.1 a` and `127.0.0.1 hx` (before: neither).

# T-2079: The faults of the eighth outside pass (2026-11-24, the gate of 0.2.0-beta.39)

**Source:** the eighth outside pass (2026-11-24, the gate of 0.2.0-beta.39): an agent with the release binary of `b6a198f3` (`--version` 0.2.0-beta.38) and `podvm man` only; the native surface as uid 1000, the incus tier as root against a private incusd 6.0.0 that the tester started (a pool `dir`; a second incusd with a pool of 40 MiB for the full pool)
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** partial

## Problem

The pass found 19 findings, OR-B39-01 to OR-B39-19: four P1 (three of the
incus tier, one of the native `cp -r`), twelve P2 and three P3. podrunner
uses the incus tier, so its P1 findings come first. The rule of the gate: no
P0 or P1 finding stays open (`docs/maintenance.md`, "Releases", step 4).

## Premise

Each P1 finding of the incus tier was reproduced on the release binary of
beta.38 against a private incusd 6.0.0 on the cloud host (2026-11-24):
OR-B39-01 (`sh -c 'echo before; exit 127'`: "incus refused the call (400,
error_code 400): Command not found", exit 125, no `before`), OR-B39-02 (six
parallel first runs of `busybox:1.36`: one passed, five "Image with same
fingerprint already exists", exit 125), OR-B39-10 (`ps --podvm-tier incus`
on a socket that takes the connection and never answers: still waiting at
45 s). Each fault is old: the code of beta.38 has it since the entry that
made the path (T-1718, T-1722).

## Approach

Fix the three P1 findings of the incus tier and the P2 findings of the
incus tier that are small and safe in the same change (T-2080, T-2081);
give OR-B39-04 (native `cp -r`) to a second agent after this one; give
OR-B39-13 to T-2078; keep each other finding in the table below, as a line
of "Known gaps" of the release.

## Decision

-

## Prove

`sh scripts/dev/checks.sh` passes with a test of the class of each fixed
finding; the reproducer of each fixed P1 finding on a real incusd gives the
docker result on the new release binary and the fault on beta.38;
experiment 434 with clauses 28 to 30; each other finding is a line in
"Known gaps" of the release.

## Done

**Partial (2026-11-24).** The state of each finding:

| Finding | Sev | Triage | Entry | State |
| --- | --- | --- | --- | --- |
| OR-B39-01, incus: a command that ends with 126 or 127 gives "incus refused the call (400): Command not found / Command not executable", exit 125, and its output is lost (`run`, `exec`) | P1 | fix | [T-2080](machine.md) | done |
| OR-B39-02, incus: parallel first runs of one image race on the import ("Image with same fingerprint already exists", a 409 of the alias) | P1 | fix | [T-2081](machine.md) | done |
| OR-B39-03, incus: SIGINT, SIGTERM or SIGHUP to a foreground `run --rm` leaves the instance running | P1 | fix | [T-2080](machine.md) | done |
| OR-B39-04, native: `cp -r SRC EXISTING_DIR` merges the contents of SRC into DEST (docker: DEST/basename(SRC)) | P1 | fix, a second agent | - | done: docker's rule of the destination is one function (`lifecycle/cp_rule.rs`), for each direction and an image as the source; `cp_tree.rs` pins it |
| OR-B39-05, incus: `exec` drops stdin and lacks the environment of the run | P2 | fix if cheap | [T-2080](machine.md) | done |
| OR-B39-06, incus: a foreground `run` gives its output at the end, stderr after stdout | P2 | gap | [T-2080](machine.md) | done |
| OR-B39-07, incus: `run -d --rm` keeps the stopped instance; a foreground `--name` is ignored | P2 | fix if cheap | [T-2080](machine.md) | done (a foreground run still keeps no record: the instance goes at its end, as under `--rm`) |
| OR-B39-08, incus: the import expands hard links (busybox: 2.1 MiB stored, 420 MiB for each instance) | P2 | gap | - | open |
| OR-B39-09, incus: `run -w /nonexistent` runs in `/` with exit 0 | P2 | fix if cheap | [T-2080](machine.md) | done for `run` (docker makes the directory); `exec -w` to a missing directory still runs in `/` (incusd) |
| OR-B39-10, incus: a daemon that takes the connection and never answers holds podvm with no bound | P2 | fix if cheap | [T-2080](machine.md) | done for the first call of each verb (30 s); a bound of each call is asked of podcrates |
| OR-B39-11, incus: `rm -rf /.podvm` in the payload deletes its log; `logs` gives a raw 404 | P2 | gap | - | open |
| OR-B39-12, incus: after a failed create, "the instance stays" although the delete answered 404; `incus tier;` | P2 | fix | [T-2080](machine.md) | done |
| OR-B39-13, incus: no check of the pool for an image that incus has; the line of the check names a host path | P2 | T-2078 | [T-2078](machine.md) | done: the import records the need of the tree in the store, and a run of an image that incus has checks the pool with it (T-2078); the path of the pool stays in the line |
| OR-B39-14, native: `cp HOST ctr:/nonexistdir/f` makes the directory | P2 | known (OR-B38-14) | - | done: a missing parent of DEST refuses the copy (exit 1) in each direction, as docker; the same rule as OR-B39-04 |
| OR-B39-15, native: the edits of a payload to `/etc/hosts` are lost at the next `exec` | P2 | gap | - | open |
| OR-B39-16, incus: `--platform linux/arm64` on amd64 gives "Command not executable" with no word of the architecture | P2 | with OR-B39-01 | [T-2080](machine.md) | done for a foreground `run`; `run -d` writes the error of the start to the log |
| OR-B39-17, incus: `--podvm-init=image` of an image with no init: a raw error of forkstart, the instance stays RUNNING | P3 | gap | - | open |
| OR-B39-18, `container ls --podvm-tier incus` gives "a bug in podvm (T-0801)" | P3 | fix | [T-2080](machine.md) | done |
| OR-B39-19, many small items (the list of the findings file) | P3 | gap | - | open |

Proof of the fixes (2026-11-24, the tree of the commit of T-2080 and
T-2081): `sh scripts/dev/checks.sh` passes; `cargo check --target
aarch64-unknown-linux-musl --workspace` passes; `cargo test --workspace
--no-fail-fast` gives 1154 passed, 0 failed, 1 ignored (96 test binaries).
The release `podvm` is 9 148 632 bytes (the ceiling is 9 500 000).
Experiment 434 (`PODVM_434_MIRROR=1`): "THE INCUS TIER RUNS", each clause,
with the new clauses 28 (OR-B39-01), 29 (OR-B39-02) and 30 (OR-B39-03).
The reproducers of the pass on a private incusd: T-2080 and T-2081.
PodUtils/podrunner#347 ask 2, which came during this work, is T-2082.

Proof of OR-B39-04 and OR-B39-14 (2026-11-24): the cause was `copy_one`
of `crates/podvm/src/lifecycle/wait_rm_cp.rs`, which gave each directory
source to `copy_tree` with DEST as the top (so the contents of SRC always
merged into DEST) and made each missing parent of a file
(`create_dir_all`). docker's rule of the destination is now one function,
`target` of `crates/podvm/src/lifecycle/cp_rule.rs`, with one table-driven
test of each row; `copy_one` uses it in each direction and for an image as
the source. `crates/podvm/tests/cp_tree.rs` pins the three cases of the
pass, `SRC/.`, an image source and five refusals; it passes as root and as
uid 1000. `sh scripts/dev/checks.sh` passes; `cargo check --target
aarch64-unknown-linux-musl --workspace` passes; `cargo test --workspace
--no-fail-fast` gives 1156 passed, 0 failed, 1 ignored (96 test
binaries). The release `podvm` is 9 148 632 bytes. The reproducers of the
pass as uid 1000 with a new store under `/var/tmp`: beta.38 gives
`/opt/exist/{a,lnk,sub,old}`, `a lnk sub` in `/opt`, `dst/{inner,y}` and a
new `/nonexistdir/f` with exit 0; the new binary gives
`/opt/exist/{dir,old}`, `/opt/dir`, `dst/srcd/{inner,y}`, and "the parent
directory of /nonexistdir/f does not exist" with exit 1.
Left: the findings of the table that are open.

# T-2087: The faults of the ninth outside pass (2026-11-24, the gate of 0.2.0-beta.40)

**Source:** the ninth outside pass (2026-11-24, the gate of 0.2.0-beta.40): an agent with the release binary of `bfb3822` (`--version` 0.2.0-beta.39) and `podvm man` only; the native surface as uid 1000, the incus tier as root against a private incusd 6.0.0 that the tester started ([the report](../docs/history/outside-pass-2026-11-24.md))
**Category:** cli
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** partial

## Problem

The pass found 28 findings, OR-B40-01 to OR-B40-28: no P0, three P1, 15
P2 and ten P3. The rule of the gate: no P0 or P1 finding stays open
(`docs/maintenance.md`, "Releases", step 4). The changes of the beta held:
systemd of `--podvm-init=image` on the incus tier, the usage of an ended
instance, the import of a large image, the joins and the signals.

## Premise

Each P1 finding was reproduced on a release binary of the tree before its
fix, as uid 1000: OR-B40-01 (`rmi local/alp:v2` with a running container
of the image: exit 125), OR-B40-02 (`inspect` of a created container:
`created join filesystem+user+pid+mount`), OR-B40-03 (`PODVM_MODE=tmpfs`
with a fresh store: "dev/random: the file could not be copied", exit 125).

## Approach

Fix the three P1 findings; keep each other finding in the table below, as
a line of "Known gaps" of the release.

## Decision

-

## Prove

`sh scripts/dev/checks.sh` passes with a test of the class of each fixed
finding; the reproducer of each P1 finding on the new release binary as
uid 1000 gives the docker result; each other finding is a line in "Known
gaps" of the release.

## Done

**Partial (2026-11-24).** The state of each finding:

| Finding | Sev | Triage | Entry | State |
| --- | --- | --- | --- | --- |
| OR-B40-01, `rmi` of a second tag of an image that a running container uses is refused with exit 125 | P1 | fix | [T-2087](cli.md) | done: the name goes as docker untags it, where another record keeps the image; the last name is refused with exit 1, the `cli-error` of the exit table |
| OR-B40-02, `inspect` gives `.Exec.Mode join` and pid sharing for a created container, where `exec` is a fresh chroot | P1 | fix | [T-2087](cli.md) | done: `.Exec.Mode` and `.Exec.Shares` say `join` only for a running run (`lifecycle::exec_opened`) |
| OR-B40-03, `PODVM_MODE=tmpfs`, `rundir` and `cache` fail for uid 1000 with a copy error of `dev/random` | P1 | fix | [T-2087](cli.md) | done: the binds of the devices go off the source tree before the copy and are made again on the staged root |
| OR-B40-04, incus: `wait` and `kill` have no path of the tier | P2 | gap | - | open |
| OR-B40-05, a foreground run on the namespace rung does not reap orphans until it ends | P2 | gap | - | open |
| OR-B40-06, `--init` and `top` are refused with a reason that is false on the namespace rung | P2 | fix | [T-2087](cli.md) | done (2026-11-26): `--init` holds where the run has a PID namespace (the namespace rung), whose PID 1 is podvm's init; a rung with no PID namespace refuses it by name with 125 (`initns::stays::check_init`, the table test `init_holds_where_the_run_has_a_pid_namespace`), each guest tier refuses it, and `--init` with `--podvm-init=image` is a flag error. The note of `top` names each rung truly and names `podvm exec NAME ps`. As uid 1000 the binary before the fix (`main`, 0.2.0-beta.41) refused `run --rm --init alpine:3.20 true` with 125 and "no PID namespace"; the fix ran it with `podvm-init` as PID 1 (exit 0), in the foreground and in `run -d`, and refused it on the userland rung (`scripts/dev/nouserns.c`) with 125 and the rung named. A foreground run reaps an orphan only at its end: OR-B40-05 stays open |
| OR-B40-07, `doctor` and `probe` say the machine tier is refused where `run` fetches its own QEMU | P2 | gap | - | open |
| OR-B40-08, `login` checks no password (known: OR-B38-08) | P2 | known | - | open |
| OR-B40-09, a payload argument that is not UTF-8 is refused | P2 | gap | - | open |
| OR-B40-10, a name conflict exits 1 from `create` and 125 from `run` | P2 | fix | [T-2100](cli.md) | done: OR-B41-10 of T-2100; each verb that makes a record exits 1, the cli-error of the published table |
| OR-B40-11, `run --rm` keeps the extracted tree of the image (the store keeps trees by design) | P2 | gap | - | open |
| OR-B40-12, `--env-file` refuses a bare NAME (known: OR-B38) | P2 | known | - | open |
| OR-B40-13, `run -it` and `exec -it`: the pty is not the controlling terminal (known: OR-B38-17) | P2 | known | - | open |
| OR-B40-14, `--network host` is refused, though the payload shares the host network | P2 | gap | - | open |
| OR-B40-15, `--format '{{json .}}'`, `table` and `index` are refused | P2 | gap | - | open |
| OR-B40-16, `cp` refuses a container path with `..` or an absolute link that docker resolves in the root | P2 | fix | [T-2087](cli.md) | done (2026-11-26): `lifecycle::cp_root::in_root` resolves the container path in the root of the container with the walk of the payload (`resolve_dir_in` of `pod-enter`), in both directions: a link with an absolute target starts again at that root, and a `..` that stays in it works. A `..` or a relative link above the root, a loop and a link to a path that the container does not have are refused by name; no host path is read or written. The table test `a_container_path_resolves_in_the_root_of_the_container`; as uid 1000 the binary before the fix (`main`, 0.2.0-beta.41) refused `cp f c:/var/run/f`, `cp c:/root/hostroot/etc/hostname out` and `cp c:/etc/../etc/hostname out` (exit 1), and the fix copied each one into the root of the container (exit 0) |
| OR-B40-17, `run -u` does not drop privileges and says nothing (`exec -u` says it) | P2 | fix | [T-2087](cli.md) | done (2026-11-26): `run -u` and `create -u` say one line where the run keeps the caller's access (`run::user_ns::kept_line`, the table test `a_user_that_keeps_the_callers_access_is_named`): on the namespace rung, a user namespace that maps one id; on a rung with no interposer, the caller's uid. The interposer of the userland rung says its own line of T-0711 already, and a foreground run with a subordinate range drops for real (T-1911) and says nothing. As uid 1000, `run --rm -u 1000:1000 alpine:3.20 sh -c 'touch /x'`: the binary before the fix (`main`, 0.2.0-beta.41) said nothing; the fix says the line. The drop itself needs a subordinate range |
| OR-B40-18, incus: a created instance reads `Stopped`, and `inspect --verbose` gives a false reason for the dashes | P2 | gap | - | open |
| OR-B40-19 to OR-B40-28, ten P3 items (the list of the findings file) | P3 | gap | - | open |

Proof of the P1 fixes (2026-11-24): OR-B40-01: `Store::delete` lets a
name go where another record keeps the same image, and refuses the last
name with `Error::Usage` (exit 1); the test
`an_image_a_container_holds_is_refused_by_rmi_and_skipped_by_prune` has the
second tag, and `rmi_refuses_a_held_image` pins exit 1. OR-B40-02:
`lifecycle::exec_opened`; the table test
`the_exec_fields_say_what_exec_does_in_each_state` (created, running,
exited, another rung). OR-B40-03: `podvm_extract::live::unbind_devices_at`
(the four devices and `dev/ptmx`) before the copy of the rundir, cache and
tmpfs rungs, `userns::bind_devices` on the staged root after it, and the
binds off before the removal of a rundir tree. The reproducers as uid 1000
on the release binary of the fix: `rmi local/alp:v2` gives "Untagged:
local/alp:v2" and exit 0, `rmi` of the last name exit 1; `inspect` gives
`created fresh-chroot filesystem` and `running join
filesystem+user+pid+mount`; `PODVM_MODE=tmpfs`, `rundir` and `cache` (twice)
run with `/dev/urandom` and a character device at `/dev/null`, exit 0, and
no tree of `runs/` stays. The test of OR-B40-01 found a second fault, as old
as the locks: `Store::delete` removed the payload, fill and extraction
lock files of each removed name, and they are keyed by the image, so the
removal of one of two names of an image removed the lock that a container
held, and the image then read as not in use. The files now stay where a
surviving record names the same image. `cargo test --workspace
--no-fail-fast`: 97 test binaries, each passed.


# T-2088: The faults of the external QA of `0.2.0-beta.31` that stay open on `0.2.0-beta.40`

**Source:** the external QA of `0.2.0-beta.31` (2026-11-17), issues 486, 495, 502, 510, 517, 519 and 524 (531 moved to [T-2091](cli.md) and 546 to [T-2092](cli.md) on 2026-11-25), measured again on 2026-11-24 as uid 1000 with the release binary of `0.2.0-beta.40`, or read in the code where it is noted
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** open

## Problem

- 486: `lab profiles extra` and `lab scenarios extra` exit 0 and ignore the extra word, as `lab profiles --bogus` does.
- 495: on the userland rung, `cp` out of a container copies a symbolic link that holds the host path of the tree. Under `scripts/dev/nouserns.c`: `ln -s /tmp/f /ln2` in the container, then `readlink` of the copy gives the path of the tree followed by `/tmp/f`. In the container, `readlink` gives `/tmp/f`.
- 502: `remote ssh forward exec PROG` exits 0 whatever the status of PROG. In the code, `run_inner` of `crates/podvm-ssh/src/bin/proxy.rs` returns `Ok` after `pump`, and nothing waits for the child.
- 510: the image form `exec IMAGE CMD` says in its banner and its help that it shares "not the process table, not /proc, not signals". It runs in the process table of the caller, and `kill -0` of the payload of a container gives 0. The container form is fixed (T-2053).
- 517: a sealed bundle with an empty, a missing or a null `argv` runs the command of the image (a shell with no input) and exits 0. No document says so.
- 519: `notebook` and `machine ssh` exit 125 for a missing argument, where the exit table gives 1 for a missing argument. `profile show` with no name prints the usage of `profile list`.
- 524: the `.Digest` that `inspect` prints is no reference for `inspect`, `rmi`, `tag` or `cp` ("no such image"). `.ManifestDigest` and `.PulledAt` are in the JSON, and `--format` refuses them.

## Premise

Each item is measured on 2026-11-24 as uid 1000 with `/var/tmp/podvm-current/podvm` (0.2.0-beta.40), except 502, which is read in the code (the helper `proxy` is not beside the binary there).

## Approach

486: refuse an operand of each list verb of `lab`. 495: take the prefix of the tree off the target of a symbolic link on a copy out. 502: wait for the child and give its code (128 plus the signal). 510: write the banner and the help of the image form from the entered rung. 517: refuse an empty `argv` by name, or say in `docs/limits.md` that it keeps the command of the image. 519: exit 1 for the two verbs, and give `profile show` its own usage. 524: take a bare digest as a reference, and add the two fields to `--format`.

## Decision

-

## Prove

`cargo test --release -p podvm --bin podvm` passes with a test of each item, and the repro of each issue, as uid 1000, gives the refusal or the result of the fix.

# T-2091: `podvm-dev build` takes the error of a failed `build-state.py commit` as the commit

**Source:** PodUtils/podvm#531 (the external QA of `0.2.0-beta.31`), read in the code on 2026-11-24
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`build_commit` of `crates/podvm-gate/src/dev/steps.rs` takes the text of a failed `scripts/build-state.py commit` (the build record is not built) as the commit, so `version --verbose` of a source build shows a build error as `commit:`.

## Premise

The issue is part of the external QA of `0.2.0-beta.31`, kept open on `0.2.0-beta.40` in [T-2088](cli.md).

## Approach

Take the commit only where the script exits 0 and prints a commit; else pass no `PODVM_BUILD_COMMIT`, so that `crates/podvm/build.rs` reads git, and give one line.

## Decision

-

## Prove

`cargo test -p podvm-gate steps::tests` passes, and with `podvm-buildstate` not built, `podvm-dev build` then `podvm version --verbose` gives a commit of git.

## Done

**Done (2026-11-25).** `build_commit` of `crates/podvm-gate/src/dev/steps.rs` takes the
  first line of `scripts/build-state.py commit` only where it exits 0 and
  the line is a hex commit of 7 to 64 digits (with `-dirty` or not) or
  `unknown` (`commit_of`); else the build gets no `PODVM_BUILD_COMMIT`, one
  line says so, and `crates/podvm/build.rs` reads the commit from git. Before,
  the background build also set the variable to an empty string where the
  script failed. Table test `steps::tests::only_a_commit_word_with_exit_0_is_the_commit`
  (14 cases). Repro on 2026-11-25 with `podvm-buildstate` not built (the
  shim exits 2): `podvm-dev build` gives the line, and `podvm version
  --verbose` gives `commit: fe3f2bf19a5061c224b997bceeb5da4524195ade-dirty`.

# T-2092: The documents, the probe field and the refusal of `--pid` say that the namespace rung opens no user, PID or UTS namespace

**Source:** PodUtils/podvm#546 (the external QA of `0.2.0-beta.31`), measured on 2026-11-24 as uid 1000 on `0.2.0-beta.40`
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

`README.md`, `docs/architecture.md`, `SECURITY.md` and `docs/limits.md` say that the namespace rung makes no user, PID or UTS namespace. The probe field `rung_must_never_claim` says it too, and the refusal of `--pid` says that there is no PID, IPC, UTS or user namespace to select. The run opens a user, a PID and a UTS namespace (the banner says so).

## Premise

The issue is part of the external QA of `0.2.0-beta.31`, kept open on `0.2.0-beta.40` in [T-2088](cli.md).

## Approach

Write each text from what an entry opens (`podvm_probe::isolation::of_entry`, T-2053): the rung makes no network namespace in any entry, and the other parts depend on the caller and the verb.

## Decision

-

## Prove

As uid 1000, `podvm probe --json` gives the words of the fix, the refusal of the flag `--pid` gives its words with exit 125, and `run --verbose` names the same namespaces.

## Done

**Done (2026-11-25).** `README.md`, `docs/architecture.md`, `SECURITY.md` and
  `docs/limits.md` say that the user, PID and UTS namespaces of the
  namespace rung depend on the caller and the verb, that the banner names
  them, and that the rung makes no network namespace; `docs/limits.md` also
  gives the PID namespace of `run -d` and `create` (T-1930). The probe field
  `rung_must_never_claim` of the namespace rung is "network isolation; user,
  pid or /proc isolation where the entry does not open it (the banner of the
  run names each part; `podvm man`, ISOLATION)" (`report::never_claim`).
  The refusal of `--pid, --ipc, --uts, --userns` says that a foreground run
  of a caller that is not root opens its own user, PID and UTS namespace,
  and that each run shares the network and IPC namespaces of the host; the
  note of `--domainname` no longer says that the runtime grants no UTS
  namespace. Repro on 2026-11-25 as uid 1000 with the release `podvm`: the
  probe field and the refusal (exit 125) give these words, and `run
  --verbose` gives `namespaces: user+pid+uts+mount` and `pids: private`.

# T-2100: The faults of the tenth outside pass (2026-11-26, the gate of the beta after 0.2.0-beta.40)

**Source:** the tenth outside pass (2026-11-26): an agent with the release binary of `7c2b161` (`--version` 0.2.0-beta.40) and `podvm man` only; the native surface as uid 1000, the incus tier as root against a private incusd 6.0.0, the machine tier under TCG ([the report](../docs/history/outside-pass-2026-11-26.md))
**Category:** cli
**Milestone:** none
**Priority:** P2
**Effort:** M
**Status:** partial

## Problem

The pass found 17 findings, OR-B41-01 to OR-B41-17: no P0, no P1, eight
P2 and nine P3. The rule of the gate: no P0 or P1 finding stays open
(`docs/maintenance.md`, "Releases", step 4), so the beta can go out with
each finding as a line of "Known gaps". Four findings are faults of the
ninth pass that stay open (OR-B40-07, OR-B40-08, OR-B40-10, OR-B40-16),
and one is the class of #473 (T-1939). The changes of the candidate held:
`update` on the incus tier, `load` of an archive that names one layer more
than once, and the machine tier under TCG.

## Premise

The lead did not reproduce the findings before the record; each has the
command and the count of the tester. The pass is not blind by a tool: the
tester ran as root on the host of the checkout (the report says what it
saw).

## Approach

Keep each finding in the table below, as a line of "Known gaps" of the
beta. Fix the P2 findings by their class after the beta, each with a test
of the class.

## Decision

-

## Prove

`sh scripts/dev/checks.sh` passes with a test of the class of each fixed
finding; the reproducer of each fixed finding on a release binary as uid
1000 gives the documented result.

## Done

The state of each finding (2026-11-26):

| Finding | Sev | Triage | Entry | State |
| --- | --- | --- | --- | --- |
| OR-B41-01, `doctor` refuses the machine tier where podvm fetched its QEMU | P2 | known | [T-2087](cli.md), OR-B40-07 | open: the legs of `pod-hostprobe` run `qemu-system-x86_64` from PATH only; COORD-256 (#675) asks podcrates for a path of the emulator |
| OR-B41-02, the fetched QEMU and the records of a detached guest go under `$HOME`, not `PODVM_STORE` | P2 | fix | [T-2100](cli.md) | partial: `podvm_windows::tools::root_of` puts the copy of QEMU in `PODVM_STORE/tools/qemu-VERSION` where `PODVM_STORE` is set (`PODVM_TOOLS_DIR` first); the table test `the_copy_of_qemu_goes_where_the_variables_say`; as uid 1000 with a home of mode 555 and a private store, `run --rm --podvm-tier=machine` fetched QEMU into the store and printed `hi`, exit 0. Open: the machine directory of a detached Linux guest stays under the Windows catalog (`PODVM_MACHINES_DIR`) |
| OR-B41-03, `system abi` exits 125 for a file that cannot be read; the manual says 2 | P2 | fix | [T-2100](cli.md) | done: the usage says 125, the code of the published table for a file that podvm cannot read (#521) |
| OR-B41-04, `cp` refuses a path through an absolute link in the container | P2 | fix | [T-2087](cli.md), OR-B40-16 | done (2026-11-26): OR-B40-16 of T-2087; `cp f c:/var/run/f` of stock alpine lands in `/run/f` of the container, and `cp c:/tmp/abs/x out` reads the file of the container |
| OR-B41-05, `--format json` prints `json` and exits 0 | P2 | fix | [T-2100](cli.md) | done: `format::walk` refuses `json` by name, as `table`; a row of the test of the image fields |
| OR-B41-06, a foreground `run --rm` that SIGPIPE ends keeps a `Created` record | P2 | fix | [T-2100](cli.md) | done: from the start of the record to its end, podvm catches SIGPIPE with a handler that does nothing (`names::stdout::catch_pipe`), so a write of podvm to a closed pipe gives EPIPE; `execve` sets the signal back to its default, and the payload gets SIGPIPE as with no podvm. The cause: the payload died of SIGPIPE, the pump of stderr closed its pipe, and the note of the exit ended podvm. As uid 1000, `run --rm alpine:3.20 sh -c 'yes x >&2' 2>&1 \| head -1`: the binary of `d0753a0` kept a record in 3 of 3 runs (`Dead`), the fix in 0 of 3, exit 141 each time; the test `a_run_rm_whose_reader_left_removes_its_record` (`tests/rm_closed_pipe.rs`, a stream on stdout and one on stderr) fails without the handler |
| OR-B41-07, incus: a `--rm` run that the OOM killer ends can keep its instance | P2 | fix | [T-2100](cli.md) | done: `incus::verbs::discard` takes a new stop and delete after a 400 of the delete, up to 4 tries, 1 s apart, and names the error of the last stop; a stand-in row (`flip1`); live as root against a private incusd 6.0.0 on the cloud host: the binary of `7c2b161` kept the instance in 1 of 4 runs ("Instance is running"), the fix in 0 of 8 |
| OR-B41-08, `--device HOST:GUEST` with another guest path is visible only in part | P2 | fix | [T-1939](enter.md) | done (2026-11-26): a device that reads goes as a volume of one file (`run::devices::fold`): a bind on the namespace rung, the path table on the userland rung; T-1939 has the proof |
| OR-B41-09, a repeated layer counts in `.Size` each time; a manifest with more layers than `diff_ids` loads | P3 | gap | - | open: `pod_oci::spec::Config` has no `rootfs.diff_ids`; COORD-258 (#677) asks podcrates for the field and a check of the count |
| OR-B41-10, a name conflict exits 1 from `create` and 125 from `run` | P3 | fix | [T-2100](cli.md) | done: by the published table a name in use is a refusal after the parse, cli-error (1), and the table names it now; `create`, `run -d` and a foreground `run` take one rule, `lifecycle::code_of`. As uid 1000 the binary of `d0753a0` gave 1, 125 and 125, the fix 1, 1 and 1; rows of `a_failed_record_of_a_foreground_run_names_its_cause` (`tests/record_failure.rs`). The incus tier stays 125 for its own check of a name (T-2080) |
| OR-B41-11, no bound of the length of a reference | P3 | gap | - | open: the parse is `Reference::parse` of `pod-oci` (podcrates `v0.85.0`), so the bound goes there: COORD-259 (#678) to podcrates asks for docker's limits (a name of at most 255 characters, a tag of at most 128) |
| OR-B41-12, `login` checks no password | P3 | known | [T-2087](cli.md), OR-B40-08 | open |
| OR-B41-13, a missing argument exits 1 or 125 by verb | P3 | fix | [T-2100](cli.md) | done: by the published table a missing argument is a cli-error (1) and a missing required flag a flag-error (125); `notebook` with no NB was the one argument that gave 125, now 1 (rows of `the_arguments_parse_and_a_wrong_one_is_named`). `login` (no `-u`), `machine ssh` (no `--kernel`) and `update` (no `--podvm-tier=incus`) miss a flag, and `windows run` with no disk is a runtime-error: each 125 is the table's |
| OR-B41-14, `system df` counts an extraction that failed | P3 | fix | [T-2100](cli.md) | done: `system df` takes `podvm_extract::is_extracted`, the check of `inspect .Extracted`; with an empty tree in place of an extraction, the binary of `7c2b161` gave `Images: 1 (1 extracted)` and the fix `Images: 1 (0 extracted)` with `.Extracted` false |
| OR-B41-15, the machine tier with too little memory names no minimum | P3 | gap | - | open |
| OR-B41-16, `update --memory` of a value too large says "at least 6m" | P3 | fix | [T-2100](cli.md) | done: an overflow says "larger than 16 EiB"; a row of the update test |
| OR-B41-17, words and looks (the list of the report) | P3 | fix | [T-2100](cli.md) | partial: done: the note of `exec` on a created container and two more messages had the indentation of a continuation line in them, and the gate counts the pattern now (`joined_literal`, plant 27g); the usage of `exec` says `<container\|image>`, and `exec nosuch` says "no such container or image" (`the_words_of_exec_name_a_container_in_one_spacing`); `load` of stdin names "the archive on stdin", not the spool (a row of `refusal_text_names_the_cause`); the refusal of `cp -` names both directions; `save` to a full stdout gives one line (a row of `stdout_full_is_125_with_the_errno_named`); the usages of `stop`, `ps` and `rm` name the flags that the incus tier refuses (`a_usage_with_the_incus_tier_names_the_flags_it_refuses`); a file on the path of the store root gives "PATH cannot be the store root: FILE is a file" (`a_file_on_the_path_of_the_store_root_is_named`). Open: `exec -w /nonexistent` says "docker creates it" from `pod-enter` (`refuse_workdir`, podcrates `v0.85.0`), COORD-259 (#678); `Up` with no time, `(linux guest)`, `verify` and `images` names, the line of each concurrent pull, `images` on a read-only store, the fixture path of `probe` and `doctor`, and `rmi -f` of an image of a stopped container each need a feature or a decision |
