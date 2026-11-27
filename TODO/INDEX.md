# TODO

Every entry, one line each, sorted by id. The entry itself lives in the
`TODO/<category>.md` the row links to, and it closes there with its own
acceptance command, actually run, with the output recorded underneath.

**What to work on next is not here.** [PROGRESS.md](PROGRESS.md) carries the
work order and is the only place that has one. This file carries the list, definitions, and derived counts.

[RULES.md](RULES.md) is how this repository is worked on.
[ROADMAP.md](ROADMAP.md) says where the milestones are: each is an entry.
[reference-map.md](reference-map.md) is the corpus and its licence
determinations.
[issues.md](issues.md) maps each GitHub issue of the record to its
entries, and `TODO/github.txt` lists the numbers it holds.

`crates/podvm-gate/src/main.rs` checks this file against the entries: a status that
disagrees, a row with no entry, an entry with no row, a count that does not add
up, a missing field, a `Prove` that is not a command, a reference that does not
resolve, a dead link, and a cited path or line that does not exist.

```sh
./target/x86_64-unknown-linux-musl/release/podvm-gate
```

## Priority

- **P0** breaks correctness, loses data, or takes the process down.
- **P1** a documented capability does not work, or a flag does nothing.
- **P2** worth doing, nothing is wrong without it.
- **P3** worth recording so it is not rediscovered.

## Effort

Effort is an estimate of scope: S is small, M is medium, L is large,
and XL requires a scope review. It is not a measured duration.

## Status

Status values are **open**, **partial**, **blocked**, and **done**.
See [entry closure](RULES.md#5-entry-closure) for their use.

## Categories

| category | crate | specification |
| --- | --- | --- |
| [probe](probe.md) | `crates/podvm-probe` | `TOOL.md` section 6.1 |
| [image](image.md) | `crates/podvm-image` | `TOOL.md` section 6.2 |
| [extract](extract.md) | `crates/podvm-extract` | `TOOL.md` section 6.3 |
| [complete](complete.md) | `crates/podvm-complete` | `TOOL.md` section 6.4 |
| [enter](enter.md) | `crates/podvm-enter` | `TOOL.md` section 6.5 |
| [supervise](supervise.md) | `crates/podvm-supervise` | `TOOL.md` section 6.6 |
| [interpose](interpose.md) | `crates/podvm-interpose` | `TOOL.md` section 6.7 |
| [cli](cli.md) | `crates/podvm` | `TOOL.md` section 6.8, section 6.9 |
| [deps](deps.md) | the whole tree | `TOOL.md` section 3.5 |
| [packaging](packaging.md) | the artefact | `TOOL.md` section 3.4, section 5 M7 |
| [milestones](milestones.md) | the gates | `TOOL.md` section 5 |
| [machine](machine.md) | the machine tier | `https://github.com/talaria0101/vm-research`, its podvm-spec document |
| [gate](gate.md) | `scripts/` | `docs/methodology/gate.md` |
| [podssh](podssh.md) | proposed SSH crate | the SSH work in pull requests 66 and 67 |
| [net](net.md) | proposed: podcrates `pod-netlab`, `pod-pcap`, `pod-egress` (the roads), `pod-broker`, `pod-netstack` | issues 178, 180, 182 and 184 of the widened scope, index issue 193 |

## Entries

| ID | Priority | Category | Status | Item |
| --- | --- | --- | --- | --- |
| [T-0101](probe.md) | P0 | M | none | probe | done | The probe set, one disposable child per probe, errno not boolean |
| [T-0102](probe.md) | P0 | S | none | probe | done | Separate a filtered syscall from an executed one with a bogus argument |
| [T-0103](probe.md) | P1 | S | none | probe | done | Probe creation and attachment separately |
| [T-0104](probe.md) | P0 | S | none | probe | done | Probe the write allowlist by writing, for blocks and inodes |
| [T-0105](probe.md) | P1 | S | none | probe | done | Read the ID maps directly rather than inferring them |
| [T-0106](probe.md) | P2 | S | none | probe | done | The `mknod` pair, because one of them tests nothing |
| [T-0107](probe.md) | P0 | M | none | probe | done | Mode selection, and one switch that turns every degradation into a refusal |
| [T-0108](probe.md) | P0 | S | none | probe | done | The mode banner |
| [T-0109](probe.md) | P0 | S | none | probe | done | A verdict is the operation's, and "could not run" never reads as "denied" |
| [T-0110](probe.md) | P1 | S | none | probe | done | `podvm probe` exit-code and channel contract |
| [T-0111](probe.md) | P2 | S | none | probe | done | Cache the probe result, and key it on what actually decides it |
| [T-0112](probe.md) | P1 | M | none | probe | done | A virtual machine answers the two questions this host's kernel cannot |
| [T-0113](probe.md) | P1 | S | none | probe | done | Two probes in one process took each other's scratch name |
| [T-0201](image.md) | P0 | M | none | image | done | Registry client, HTTPS only, with no plain-HTTP fallback |
| [T-0202](image.md) | P0 | M | none | image | done | A content-addressed store, and digest parity with docker |
| [T-0203](image.md) | P0 | S | none | image | done | Check `statvfs` for blocks and inodes, and name the destination |
| [T-0204](image.md) | P1 | M | none | image | done | `images`, `rmi`, `tag`, and a store GC that cannot delete a running container's rootfs |
| [T-0205](image.md) | P3 | S | none | image | done | Re-test podman with `vfs` and `ignore_chown_errors` before repeating "no path exists" |
| [T-0206](image.md) | P3 | L | none | image | done | A registry fixture, so the acceptance stops depending on somebody else's quota |
| [T-0207](image.md) | P2 | L | none | image | done | Fetch layers with bounded concurrency, and measure what it buys |
| [T-0208](image.md) | P2 | L | none | image | done | `--platform`, and a store that can hold two variants of one tag |
| [T-0209](image.md) | P2 | L | none | image | done | Registry authentication, without a credential ever entering this tree |
| [T-0210](image.md) | P1 | L | none | image | done | The store's concurrency contract, written down and driven |
| [T-0211](image.md) | P1 | S | none | image | done | An image lock outlives its holder whenever anything forks |
| [T-0212](image.md) | P0 | M | none | image | done | The platform is decided at run time, and the store holds more than one |
| [T-0213](image.md) | P0 | M | none | image | done | A registry with no certificate, or one nothing trusts, and the refusal kept |
| [T-0214](image.md) | P2 | S | none | image | done | A blob body cut off mid-stream is not retried, and the bounded retry is around the wrong thing |
| [T-0215](image.md) | P0 | M | none | image | done | Four lock tests fail in two runs of five, and the gate has never said so |
| [T-0301](extract.md) | P0 | M | none | extract | done | Extract in-process, at entry level, never through system `tar` |
| [T-0302](extract.md) | P0 | M | none | extract | done | Ownership-neutral extraction plus the sidecar |
| [T-0303](extract.md) | P0 | S | none | extract | done | Whiteouts are matched on the basename, never with a path glob |
| [T-0304](extract.md) | P0 | M | none | extract | done | Refuse an entry that resolves outside the destination, including through a symlink from the same layer |
| [T-0305](extract.md) | P1 | S | none | extract | done | An absolute symlink target is rootfs-relative, not a refusal |
| [T-0306](extract.md) | P1 | S | none | extract | done | Re-permission between layers, or the second layer fails |
| [T-0307](extract.md) | P1 | S | none | extract | done | Hard links, symlinks and the layer order |
| [T-0401](complete.md) | P0 | S | none | complete | done | Device shims as regular files |
| [T-0402](complete.md) | P0 | S | none | complete | done | Always install the host's `/etc/resolv.conf` |
| [T-0403](complete.md) | P2 | S | none | complete | done | `/etc/hosts` |
| [T-0404](complete.md) | P0 | M | none | complete | done | Synthesize `/etc/passwd` and `/etc/group`, and let the payload's writes persist |
| [T-0405](complete.md) | P1 | S | none | complete | done | `/etc/mtab` is a symlink, and writing through it escapes the rootfs |
| [T-0406](complete.md) | P1 | S | none | complete | done | pacman: `DownloadUser` and the keyring |
| [T-0407](complete.md) | P1 | S | none | complete | done | apt: the sandbox user, https sources and the CA bundle |
| [T-0408](complete.md) | P2 | S | none | complete | done | zypper: fix the RIS index, not `repos.d` |
| [T-0409](complete.md) | P2 | S | none | complete | done | Ownership failures from `dpkg`, `rpm` and `xbps` are warnings |
| [T-0410](complete.md) | P0 | S | none | complete | done | Supply `/etc/nsswitch.conf`, or the supplied `/etc/passwd` is a no-op |
| [T-0411](complete.md) | P1 | M | none | complete | done | A payload whose package sources are `http://`, on a runtime where tcp/80 hangs |
| [T-0412](complete.md) | P1 | M | none | complete | done | A fixup that has to run INSIDE the rootfs, and podvm runs it from outside |
| [T-0413](complete.md) | P1 | M | none | complete | done | No `/proc` inside a chroot, and the shell feature that quietly stops working |
| [T-0414](complete.md) | P1 | M | none | complete | done | Two walls only one instance of the class has shown, and podvm has probed neither |
| [T-0415](complete.md) | P1 | S | none | complete | done | A device stand-in is checked by type, because an absent one becomes a growing file |
| [T-0501](enter.md) | P0 | M | none | enter | done | Open every descriptor before the root changes |
| [T-0502](enter.md) | P0 | S | none | enter | done | Resolve the program inside the new root, in the process that changed it |
| [T-0503](enter.md) | P1 | S | none | enter | done | Probe `/dev/ptmx`, and refuse `-t` by name where it is absent |
| [T-0504](enter.md) | P1 | S | none | enter | done | Refuse a rootfs path that is a symlink |
| [T-0505](enter.md) | P1 | S | none | enter | done | `exec` is a fresh chroot, and `inspect` says so |
| [T-0506](enter.md) | P0 | M | none | enter | done | A foreign-architecture container, and never a rung measured by the emulator |
| [T-0601](supervise.md) | P0 | M | none | supervise | done | One pidfd per direct child, `waitid` for status |
| [T-0602](supervise.md) | P0 | M | none | supervise | done | Never decide "running" by sleeping and looking |
| [T-0603](supervise.md) | P1 | S | none | supervise | done | `PR_SET_PDEATHSIG` fires on the creating thread's exit |
| [T-0604](supervise.md) | P1 | M | none | supervise | done | Running state is launcher state |
| [T-0605](supervise.md) | P2 | S | none | supervise | done | Capture logs at spawn, from the descriptors opened in step 2 |
| [T-0606](supervise.md) | P0 | L | none | supervise | done | The notification tier: probe three legs, refuse the tier, never fall back per call |
| [T-0607](supervise.md) | P0 | M | none | supervise | done | The lifecycle, twenty times, twenty passes |
| [T-0608](supervise.md) | P1 | M | none | supervise | done | A detached container that reads `exited` with no launcher, seen twice and not reproduced |
| [T-0701](interpose.md) | P0 | M | none | interpose | done | The cdylib build constraints |
| [T-0702](interpose.md) | P0 | M | none | interpose | done | One object per libc, and it must live inside the rootfs |
| [T-0703](interpose.md) | P0 | L | none | interpose | done | Path virtualization: the entry-point set and `*at` resolution |
| [T-0704](interpose.md) | P0 | L | none | interpose | done | Ownership virtualization: the half a path interposer does not have |
| [T-0705](interpose.md) | P1 | M | none | interpose | done | Reverse mapping, so the payload reads back what it wrote |
| [T-0706](interpose.md) | P0 | S | none | interpose | done | Classify the payload and decline with a named reason |
| [T-0707](interpose.md) | P1 | S | none | interpose | done | The paths that must not be rewritten |
| [T-0708](interpose.md) | P2 | M | none | interpose | done | Intercept the operations the runtime cannot provide |
| [T-0709](interpose.md) | P0 | M | none | interpose | done | Select the interposer by `DT_NEEDED`, and refuse on the version predicate |
| [T-0710](interpose.md) | P1 | L | none | interpose | done | The ownership memo lives where the payload can edit it, and answers by linear scan |
| [T-0711](interpose.md) | P1 | L | none | interpose | done | The identity calls, and podvm's honesty rules point the other way from fakeroot's |
| [T-0712](interpose.md) | P2 | S | none | interpose | done | A reach matrix holds its arguments constant, or it measures two things |
| [T-0801](cli.md) | P0 | L | none | cli | done | The verb and flag parity table |
| [T-0802](cli.md) | P0 | S | none | cli | done | docker's exit codes, unaltered |
| [T-0803](cli.md) | P1 | S | none | cli | done | Answer to `docker` and `podman` on PATH |
| [T-0804](cli.md) | P0 | M | none | cli | done | The honesty rules, and one switch that makes every degradation fatal |
| [T-0805](cli.md) | P1 | M | none | cli | done | Diagnostics that name the operation, the errno, the mechanism and the remedy |
| [T-0806](cli.md) | P0 | S | none | cli | done | Never prompt, never wait unbounded, and check space before every large write |
| [T-0807](cli.md) | P2 | S | none | cli | done | `podvm images --format` refuses a template no verb can answer, before it looks at the store |
| [T-0808](cli.md) | P1 | L | none | cli | done | Drive every row of the parity table through the shipped binary |
| [T-0809](cli.md) | P1 | M | none | cli | done | The spawn that fails with the wrong reason, and the one field that fixes it |
| [T-0810](cli.md) | P1 | S | none | cli | done | `create` fails storing the memo: the container directory is never made |
| [T-0901](deps.md) | P1 | M | none | deps | done | Sweep: syscalls |
| [T-0902](deps.md) | P2 | S | none | deps | done | Sweep: seccomp BPF |
| [T-0903](deps.md) | P3 | S | none | deps | done | Sweep: Landlock |
| [T-0904](deps.md) | P1 | M | none | deps | done | Sweep: OCI registry client and types |
| [T-0905](deps.md) | P0 | S | none | deps | done | Sweep: TLS |
| [T-0906](deps.md) | P1 | S | none | deps | done | Sweep: HTTP |
| [T-0907](deps.md) | P0 | M | none | deps | done | Sweep: tar, gzip, zstd |
| [T-0908](deps.md) | P1 | M | none | deps | done | Sweep: digests, JSON, argument parsing, ELF |
| [T-0909](deps.md) | P1 | M | none | deps | done | Vendor the memfd and userland-exec rungs, and fix the fork's regression here |
| [T-0910](deps.md) | P0 | S | none | deps | done | The `cargo bloat` baseline, committed, and checked at the gate |
| [T-0911](deps.md) | P0 | M | none | deps | done | The syscall table and the kernel structs come from a crate, per architecture |
| [T-0912](deps.md) | P2 | M | none | deps | done | The powerpc gate is the crate's and it is stale, so podvm can clear it |
| [T-1001](packaging.md) | P0 | S | none | packaging | done | A single static binary with no `PT_INTERP` |
| [T-1002](packaging.md) | P1 | M | none | packaging | done | Embed the interposer as bytes and place it inside the rootfs |
| [T-1003](packaging.md) | P2 | L | none | packaging | done | The launch ladder, and a single file with an embedded rootfs |
| [T-1004](packaging.md) | P3 | M | none | packaging | done | A reproducible build, and the artefact's own inputs recorded |
| [T-1005](packaging.md) | P1 | S | none | packaging | done | A session reaches the code in one command, and the build runs behind the reading |
| [T-1100](milestones.md) | P0 | L | none | milestones | done | M-1 the corpus, the work index and the skeleton |
| [T-1101](milestones.md) | P0 | M | none | milestones | done | M0 the probe, and nothing else |
| [T-1102](milestones.md) | P1 | M | none | milestones | done | M1 image acquisition |
| [T-1103](milestones.md) | P0 | L | none | milestones | done | M2 extraction that survives the ownership wall |
| [T-1104](milestones.md) | P0 | M | none | milestones | done | M3 `run` on the chroot rung |
| [T-1105](milestones.md) | P0 | L | none | milestones | done | M4 the lifecycle, twenty times |
| [T-1106](milestones.md) | P1 | L | none | milestones | done | M5 environment completion, ten distributions |
| [T-1107](milestones.md) | P1 | L | none | milestones | done | M6 the interposer |
| [T-1108](milestones.md) | P2 | M | none | milestones | done | M7 packaging |
| [T-1109](milestones.md) | P1 | M | none | milestones | done | The negative tests, which are tests |
| [T-1110](milestones.md) | P0 | L | none | milestones | done | M6's acceptance: a payload the interposer is the only reason works |
| [T-1111](milestones.md) | P1 | L | none | milestones | done | M8 the nix acceptance: a real payload the chroot tier is exactly the answer for |
| [T-1112](milestones.md) | P1 | L | none | milestones | done | A disposable guest that is not Linux |
| [T-1201](gate.md) | P0 | S | none | gate | done | The gate reaches every file this project wrote |
| [T-1202](gate.md) | P0 | M | none | gate | done | Every check is planted against, and a plant that stops reaching its subject says so |
| [T-1203](gate.md) | P1 | M | none | gate | done | A measurement taken on one host is a property of that host |
| [T-1204](gate.md) | P1 | S | none | gate | done | Check 17 holds the newest committed reading under the ceiling, not only the baseline |
| [T-1205](gate.md) | P1 | S | none | gate | done | The gate holds experiment numbers unique, because four Proves already collide |
| [T-1206](gate.md) | P0 | M | none | gate | done | CI installs the toolchain the build config names, and nine commits proved nobody was holding it |
| [T-1207](gate.md) | P1 | L | none | gate | done | The one crate that runs inside other people's processes is the one the gate does not check |
| [T-1208](gate.md) | P1 | M | none | gate | done | A closed entry carries its recorded run, and the gate can see it |
| [T-1209](gate.md) | P1 | M | none | gate | done | Thirty-nine `Prove` lines pull from the one registry the acceptance may not use |
| [T-1210](gate.md) | P1 | M | none | gate | done | Convert the interpose engine scripts to `experiments/lib/engine.sh` |
| [T-1211](gate.md) | P1 | M | none | gate | done | Convert the distribution and probe engine scripts to `experiments/lib/engine.sh` |
| [T-1212](gate.md) | P1 | M | none | gate | done | Convert the image, registry and CLI engine scripts to `experiments/lib/engine.sh` |
| [T-1213](gate.md) | P1 | M | none | gate | done | Convert the target-image pair and its probe consumer to `experiments/lib/engine.sh` |
| [T-1301](machine.md) | P0 | M | none | machine | done | The machine tier is probed leg by leg, and a present file is not a working one |
| [T-1302](machine.md) | P1 | M | none | machine | done | One binary, one parity table, and a VM-only flag that cannot collide |
| [T-1303](machine.md) | P1 | L | none | machine | done | The image is a rootfs directory, and an initramfs with no console is a silent machine |
| [T-1304](machine.md) | P1 | L | none | machine | done | The exec protocol is a serial pair with a nonce, and every wait is bounded |
| [T-1305](machine.md) | P2 | L | none | machine | done | The fleet and the fork, and the file-size ceiling that bounds both |
| [T-1306](machine.md) | P1 | M | none | machine | done | The non-goals are refusals the code makes, not notes in a document |
| [T-1307](machine.md) | P2 | S | none | machine | done | The five Rust VM tools, ruled one by one, so nobody surveys them again |
| [T-1308](machine.md) | P1 | M | none | machine | done | One TCG number is a claim about one benchmark, and the range is 3x to 21x |
| [T-1309](interpose.md) | P1 | M | none | interpose | done | libdnf repodata downloads fail under the preloaded interposer |
| [T-1310](image.md) | P1 | S | none | image | done | The store suite exhausts the sixteen fork-shed slots, and the victim varies |
| [T-1311](interpose.md) | P1 | S | none | interpose | done | The interposed `fchmodat` drops the `flags` argument |
| [T-1312](interpose.md) | P1 | S | none | interpose | done | The glibc interposer needs newer symbols than the payload provides |
| [T-1313](machine.md) | P1 | M | none | machine | done | Every experiment fetch carries its own ceiling, and a stalled origin proves it |
| [T-1314](packaging.md) | P1 | L | none | packaging | done | Nightly releases: one tag builds and tests every supported arch |
| [T-1315](extract.md) | P1 | S | none | extract | done | Extract re-verifies the blob digest against the manifest |
| [T-1316](deps.md) | P2 | S | none | deps | done | The committed lock matches the manifests, so a clean build leaves a clean tree |
| [T-1317](enter.md) | P1 | M | none | enter | done | A chroot-denied host gets an up-front refusal naming chroot, not a 125 after the work |
| [T-1318](supervise.md) | P3 | S | none | supervise | done | `logs -f` follows a container's log, bounded like the rest |
| [T-1319](cli.md) | P1 | S | none | cli | done | `inspect` prints exactly one document for any reference |
| [T-1320](image.md) | P2 | M | none | image | done | Images move without a registry: save, load, import |
| [T-1321](image.md) | P2 | M | none | image | done | A store-health verb and a pull-provenance record |
| [T-1322](image.md) | P1 | M | none | image | done | A container record gates every removal of the image it references |
| [T-1323](cli.md) | P2 | M | none | cli | done | `cp` reaches an image's extracted rootfs, and copies directories with the same checks |
| [T-1324](complete.md) | P2 | S | none | complete | done | The README tells the truth: build order, rung map, status, auth scope |
| [T-1325](gate.md) | P1 | M | none | gate | done | The gate checks what entries claim: reachable flags and true parity notes |
| [T-1326](gate.md) | P1 | S | none | gate | done | `check-markers.sh` builds its marker bytes portably across `/bin/sh` |
| [T-1327](interpose.md) | P2 | L | none | interpose | done | The interposer builds per architecture, and the T-0704 citations point at it |
| [T-1328](packaging.md) | P2 | M | none | packaging | done | The nightly signs its artefacts, with provenance a downloader can check |
| [T-1329](packaging.md) | P2 | M | none | packaging | done | The per-arch smoke pulls and extracts, not just versions |
| [T-1330](cli.md) | P2 | S | none | cli | done | Bundled short flags parse as docker reads them |
| [T-1331](cli.md) | P2 | M | none | cli | done | `--filter`, `restart` and `pull -a/-q` answer the docker idiom |
| [T-1332](cli.md) | P1 | M | none | cli | done | `podvm man` generates the manual from the binary, pager-aware |
| [T-1333](probe.md) | P1 | S | none | probe | done | The probe cache key carries the capability set |
| [T-1334](packaging.md) | P1 | S | none | packaging | done | The release carries its build commit's gate state and its reproducibility boundary |
| [T-1335](supervise.md) | P3 | S | none | supervise | done | The launcher forwards its own shutdown signals to the payload |
| [T-1336](cli.md) | P1 | M | none | cli | done | CLI output is plain ASCII: no emoji, no markers, on every path |
| [T-1337](cli.md) | P2 | M | none | cli | done | Doctor, disk usage, and log tail for operators |
| [T-1338](gate.md) | P2 | L | none | gate | done | A performance harness with baselines and a regression gate |
| [T-1339](enter.md) | P1 | L | none | enter | done | Enter the namespace rung where the probe permits it |
| [T-1340](gate.md) | P3 | M | none | gate | done | Isolate why early payload runs cost seconds and late ones do not |
| [T-1341](gate.md) | P3 | S | none | gate | done | Budget the warm run cost beside the cold one |
| [T-1342](image.md) | P2 | M | none | image | done | A pull refuses past the ceiling instead of dying past it |
| [T-1343](gate.md) | P1 | M | none | gate | done | The Windows lane uses toolkit 6 job inputs and checks every retained job |
| [T-1344](gate.md) | P1 | S | none | gate | done | Experiment jobs find the checkout after the Windows input change |
| [T-1345](gate.md) | P1 | S | none | gate | done | The Windows job wrapper runs a Bash caller with Bash |
| [T-1346](gate.md) | P1 | S | none | gate | done | The plant script finds the checkout from a Windows job input |
| [T-1347](gate.md) | P1 | S | none | gate | done | Check parity notes against the milestone entries |
| [T-1348](gate.md) | P1 | L | none | gate | done | Check current documents against source declarations |
| [T-1349](gate.md) | P1 | M | none | gate | done | Verify build freshness from input and output bytes |
| [T-1350](gate.md) | P1 | M | none | gate | partial | Make Windows proofs use explicit inputs and return build artifacts |
| [T-1351](gate.md) | P1 | S | none | gate | done | Keep the failed check's complete diagnostic |
| [T-1401](podssh.md) | P1 | L | none | podssh | done | Prove the SSH transport and server in the current tree |

| [T-1402](podssh.md) | P1 | L | none | podssh | done | Provide an interactive session without a pty |
| [T-1403](podssh.md) | P1 | L | none | podssh | done | Prove concurrent sessions on one relay connection |
| [T-1404](podssh.md) | P2 | L | none | podssh | done | Add the remote and machine SSH verbs after the transport holds |
| [T-1405](podssh.md) | P1 | M | none | podssh | done | Ship and smoke the SSH helper archive |
| [T-1406](podssh.md) | P1 | L | none | podssh | done | Bound SSH DNS and writes and prove node reconnect |
| [T-1407](enter.md) | P0 | L | none | enter | done | Anchor userland paths inside the rootfs or refuse |
| [T-1408](enter.md) | P1 | S | none | enter | done | Resolve the loader symlink inside the rootfs |
| [T-1409](enter.md) | P1 | M | none | enter | done | Answer the payload-visible exe on the loader family |
| [T-1410](enter.md) | P1 | L | none | enter | done | Exec through the no-chroot family |
| [T-1411](enter.md) | P1 | L | none | enter | done | A detached launcher for the no-chroot family |
| [T-1412](enter.md) | P1 | S | none | enter | done | A missing workdir refuses naming the path |
| [T-1413](enter.md) | P2 | S | none | enter | done | A script refusal names the file and its shebang |
| [T-1414](cli.md) | P1 | S | none | cli | done | The no-chroot path returns 127 and 126 |
| [T-1415](cli.md) | P1 | M | none | cli | done | Strict classifies degradations on the userland rung |
| [T-1416](cli.md) | P2 | S | none | cli | done | Quiet runs and a verbose banner |
| [T-1417](cli.md) | P2 | M | none | cli | done | Inspect carries the OCI config |
| [T-1418](image.md) | P1 | M | none | image | done | A noexec store refuses before the pull |
| [T-1419](image.md) | P1 | M | none | image | done | Pull-missing verifies the blobs it trusts |
| [T-1420](image.md) | P2 | S | none | image | done | Transport errors name the host once |
| [T-1421](supervise.md) | P1 | M | none | supervise | done | A foreground run leaves a container record |
| [T-1422](gate.md) | P2 | S | none | gate | done | Plant the kept-session check where the OS execs batch files |
| [T-1423](packaging.md) | P1 | S | none | packaging | done | The release tag names the workspace version |
| [T-1559](packaging.md) | P2 | M | none | packaging | done | Port `scripts/build-state.py` to `podvm-buildstate` |
| [T-1560](packaging.md) | P2 | S | none | packaging | done | Port `scripts/release-licenses.py` to `podvm-release-licenses` |
| [T-1561](packaging.md) | P2 | M | none | packaging | done | Port `scripts/verify-release.sh` to `podvm-verify` |
| [T-1562](deps.md) | P0 | M | none | deps | done | Port `experiments/110-bloat-delta.sh` to `podvm-size` |
| [T-1563](packaging.md) | P1 | M | none | packaging | done | Port `experiments/120-reproducible-build.sh` and `157` to `podvm-prove-t0211` |
| [T-1564](complete.md) | P2 | M | none | complete | done | Port `scripts/document-state.py` to `document-state` |
| [T-1565](packaging.md) | P2 | S | none | packaging | done | Port `scripts/release-notes.sh` to `release-notes` |
| [T-1566](packaging.md) | P2 | M | none | packaging | done | Port `experiments/395-reconcile-repository.py` to `podvm-reconcile` |
| [T-1567](packaging.md) | P2 | M | none | packaging | done | Port `experiments/399-publication.py` to `podvm-publish` |
| [T-1568](machine.md) | P2 | M | none | machine | done | Port `experiments/145-podvm-parity.sh` to `podvm-parity` |
| [T-1569](machine.md) | P2 | S | none | machine | done | Port `experiments/154-tcg-workload-spread.sh` to `podvm-workload` |
| [T-1601](interpose.md) | P0 | S | none | interpose | done | Unblock the static build under Rust 1.99 `invalid_runtime_symbol_definitions` |
| [T-1602](image.md) | P0 | M | none | image | done | The registry fixture fields are read by their consumer tests |
| [T-1603](gate.md) | P0 | M | none | gate | done | Remove `check-no-secrets` and add a pinned trufflehog scan workflow |
| [T-1604](enter.md) | P1 | M | none | enter | done | Fix the detached-stdio serial mutex and the 1-of-3 detached start failure |
| [T-1605](packaging.md) | P1 | S | none | packaging | done | Push a version tag and prove the nightly release-notes wiring live |
| [T-1606](machine.md) | P2 | L | none | machine | done | Repoint all 47 `references/` citations before deleting the corpus |
| [T-1607](gate.md) | P1 | M | none | gate | done | File the 21 unbatched plan tasks as a later queue |
| [T-1608](gate.md) | P2 | S | none | gate | done | Reword the Batch 3 Done paragraphs so one-home passes |
| [T-1609](gate.md) | P0 | M | none | gate | done | Build the KVM watchdog that makes an unattended guest run safe |
| [T-1610](gate.md) | P0 | M | none | gate | done | Repair the base's stale podman state and settle which qemu the proof uses |
| [T-1611](gate.md) | P1 | S | none | gate | done | Repoint the code maps and limits page after the Batch 3 port |
| [T-1612](gate.md) | P1 | S | none | gate | done | Give the unowned work in the record an owner |
| [T-1613](gate.md) | P1 | S | none | gate | done | The 21 unbatched plan rows are in the record, and two were already tracked |
| [T-1619](packaging.md) | P0 | M | none | packaging | done | The `10`/`20`/`130` three-way decision belongs to the wave that deletes `10` |
| [T-1620](image.md) | P1 | S | none | image | done | The lock-race clause decision belongs to the store work it cites |
| [T-1621](gate.md) | P2 | S | none | gate | done | The 11 Batch 3 rows were already tracked and are done |
| [T-1622](gate.md) | P0 | S | none | gate | done | Reconcile the ledger's 121 rows against the seven entries and record the three VC changes |
| [T-1623](milestones.md) | P3 | S | none | milestones | done | Record the 29 KEEP-SHELL rows as the retained set, re-derived from the ledger |
| [T-1624](deps.md) | P2 | S | none | deps | done | Record the 265-row parity figure and the four stale values the tree carries |
| [T-1625](image.md) | P1 | S | none | image | done | Record that `70-whiteout-contract.sh` check B needs a fixture, and where it goes |
| [T-1626](machine.md) | P1 | S | none | machine | done | Record that `149` clause 6 has no Rust arm and stays shell pending the guest fixture |
| [T-1627](gate.md) | P1 | S | none | gate | done | Record `80-interposer-abi.sh` check B and `170` clause 3 as staying shell |
| [T-1628](interpose.md) | P1 | S | none | interpose | done | Record `162-tar-symlink-modes.sh` as retained, DELETE refuted by VC-2 |
| [T-1629](probe.md) | P2 | S | none | probe | done | Record `95-podman-vfs-ignorechown.sh` as KEEP-SHELL, a deployment proof under VC-3 and VC-6 |
| [T-1630](cli.md) | P1 | S | none | cli | done | The symlink-escape script is converted, and its deletion has a condition |
| [T-1631](milestones.md) | P2 | S | none | milestones | done | Record the `10`/`20`/`130` irreducible dependency as open, beside its decision entry |
| [T-1632](gate.md) | P1 | S | none | gate | done | Record the 149-tree boundary against the 121 top-level figure |
| [T-1633](deps.md) | P2 | S | none | deps | done | Record the 13-versus-3 `plant.sh` `Prove` discrepancy and which count is right |
| [T-1634](image.md) | P1 | S | none | image | done | Record that `serve_once` is test-private and no fixture existed before T-1602 |
| [T-1635](gate.md) | P1 | S | none | gate | done | Record the `py_compile` glob at `gate.yml:186` as needing a change |
| [T-1636](gate.md) | P1 | S | none | gate | done | Record the second workflow, `nightly.yml`, as a consumer of four ported subjects |
| [T-1637](milestones.md) | P2 | S | none | milestones | done | Record the eleven converged-on-Rust counts and the 29 that stay in shell |
| [T-1638](gate.md) | P0 | S | none | gate | done | The record gate proves itself with the tool its own port deletes |
| [T-1639](interpose.md) | P1 | S | none | interpose | done | Record the six interpose members and their excluded-crate proofs as separate jobs |
| [T-1640](interpose.md) | P1 | S | none | interpose | done | Record that the interpose export check needs no `rlib` and no new test |

| [T-1641](gate.md) | P0 | M | none | windows | done | The provisioner writes into the vendor's backing image, and a timed-out run orphans its emulator |
| [T-1642](deps.md) | P1 | S | none | deps | done | Refuse a Linux-target cargo build launched on a Windows host |
| [T-1643](interpose.md) | P1 | M | none | interpose | done | A relative path resolves against the host cwd and gets the rootfs prefix twice |
| [T-1644](interpose.md) | P1 | M | none | interpose | done | Admit a glibc payload against its whole NEEDED closure, read inside the rootfs |
| [T-1645](interpose.md) | P1 | M | none | interpose | done | Intercept `dlopen` and `dlmopen` |
| [T-1646](interpose.md) | P1 | L | none | interpose | done | A dynamic grandchild re-enters through the host loader |
| [T-1647](enter.md) | P1 | M | none | enter | done | Stream a payload into a memfd: the 128 MiB read ceiling refuses large runtimes |
| [T-1648](enter.md) | P0 | S | none | enter | done | A static Go payload runs into a GOROOT error instead of a named refusal |
| [T-1649](cli.md) | P2 | S | none | cli | done | A `#!` entrypoint needs an undocumented `--entrypoint`, and the refusal must name the way out |
| [T-1650](cli.md) | P1 | M | none | cli | done | `--strict` does not refuse the userland grandchild limit, while `--strict=all` refuses every run |
| [T-1651](enter.md) | P2 | L | none | enter | done | An emulate tier on a prefix-preserving, fail-closed qemu-user for static payloads |
| [T-1652](enter.md) | P2 | L | none | enter | done | A native-static rung: userland execve, Syscall User Dispatch and openat2(RESOLVE_IN_ROOT) |
| [T-1653](enter.md) | P2 | L | none | enter | done | `-v` volumes on a host with no /dev/fuse and no mount |
| [T-1654](cli.md) | P2 | L | none | cli | done | Profiles: strict TOML recipes with `extends` and `-s` overrides |
| [T-1655](cli.md) | P2 | L | none | cli | done | The sandbox hardening line: each `run` flag works on the tier or refuses by name |
| [T-1656](machine.md) | P2 | L | none | machine | done | MicroVMs: a fast disposable guest from an OCI image, on KVM or TCG |
| [T-1657](enter.md) | P2 | L | none | enter | done | A syscall-interpreting tier: the gVisor capability without user namespaces |
| [T-1658](enter.md) | P2 | L | none | enter | done | A confine layer: Landlock, seccomp and no_new_privs on every rootfs tier |
| [T-1659](cli.md) | P2 | M | none | cli | done | Provider shapes and bare userlands as built-in profiles |
| [T-1660](cli.md) | P2 | L | none | cli | done | `podvm notebook`: Jupyter and marimo cycles that keep one directory |
| [T-1661](cli.md) | P2 | M | none | cli | done | Language runtime profiles: Node, Deno and Bun permissions, wasmtime and workerd |
| [T-1662](net.md) | P2 | L | none | net | partial | Network labs: NAT, IPv6, policy and impairment labs inside one guest |
| [T-1663](net.md) | P2 | L | none | net | blocked | Tunnels and live uplinks: WireGuard kill switch, WARP uplink, proxy-forced programs |
| [T-1664](net.md) | P2 | L | none | net | blocked | Observability: capture, counters and connection logs without `CAP_NET_RAW` |
| [T-1665](net.md) | P2 | L | none | net | done | An egress broker, and the guest's view of a loopback-only proxy |
| [T-1666](probe.md) | P2 | L | none | probe | done | A security probe inside the payload, and a compare against a reference report |
| [T-1667](cli.md) | P2 | L | none | cli | partial | Doctor and installers: one check per scope capability, verified downloads, a space floor |
| [T-1668](gate.md) | P2 | M | none | gate | done | A findings page for the scope: observed or inferred, with host conditions per row |
| [T-1669](deps.md) | P1 | M | none | deps | done | Pin the stable Rust of the day and edition 2024, before the first `pod-*` crate |
| [T-1670](probe.md) | P1 | M | none | probe | done | The first cloud session measures its host and saves the result |
| [T-1671](packaging.md) | P2 | M | none | packaging | done | Release the first podvm beta with `## Breaking` notes, from a dispatched `release.yml` |
| [T-1672](gate.md) | P2 | L | none | gate | done | Move the record to the format of `pod-todo`, with a `podvm-todo` wrapper and a todo.toml |
| [T-1673](gate.md) | P2 | M | none | gate | done | File the 29 plan rows that T-1607 owns, read from the archive branch |
| [T-1674](gate.md) | P1 | L | none | gate | done | Split every Rust source file to 500 lines, then hold the limit in the gate |
| [T-1675](deps.md) | P2 | M | none | deps | done | Take `pod-term` for the SSH line discipline |
| [T-1676](deps.md) | P2 | M | none | deps | done | Take `pod-tls` for both TLS stacks, with its synchronous face |
| [T-1677](deps.md) | P2 | L | none | deps | done | Take `pod-proxy` with feature `blocking`: HTTPS_PROXY egress for pulls and the SSH dial |
| [T-1678](deps.md) | P2 | L | none | deps | done | Take `pod-relay` with feature `blocking` for the relay, keeping `reverse-v1` |
| [T-1679](deps.md) | P2 | M | none | deps | done | Take `pod-services` for the third-party hosts compiled into podvm |
| [T-1680](deps.md) | P2 | L | none | deps | done | Take `pod-todo` for the record half of `podvm-gate` |
| [T-1681](deps.md) | P2 | L | none | deps | done | Hand the seven generic crates to podcrates and freeze their files |
| [T-1682](machine.md) | P2 | M | none | machine | done | A `kvm.yml` workflow that runs the machine tier's Linux guest on a hosted runner |
| [T-1683](gate.md) | P2 | M | none | gate | done | Retire the PowerShell twins of `scripts/common/` and `check-twins.sh` |
| [T-1684](gate.md) | P2 | L | none | gate | done | Fuzz the parsers that read untrusted input, and watch `vendor/` upstream |
| [T-1685](gate.md) | P2 | M | none | gate | done | A maintenance page and a development page in the sibling form |
| [T-1686](complete.md) | P2 | S | none | complete | done | The device tests of a stand-in fail where `mknod` is permitted |
| [T-1687](deps.md) | P2 | S | none | deps | done | Fetch the Alpine netboot kernel and modloop from the 3.22.6 archive |
| [T-1688](enter.md) | P2 | M | none | enter | done | The memfd rung measures whether a memfd executes, names `vm.memfd_noexec`, and refuses before staging |
| [T-1689](probe.md) | P2 | S | none | probe | done | The non-goal `remedy` follows the stance in the probe JSON |
| [T-1690](cli.md) | P2 | S | none | cli | done | The doctor disk-free row judges the soft `RLIMIT_FSIZE` |
| [T-1691](supervise.md) | P1 | S | none | supervise | done | `stop -t 0` is a stop: no grace sends SIGKILL at once, and a failed bound is not a gone launcher |
| [T-1692](milestones.md) | P2 | S | none | windows | done | The `windows --podvm-mem` row says what the verb does about `RLIMIT_FSIZE` |
| [T-1693](packaging.md) | P2 | S | none | packaging | done | README and SECURITY name the last podbox tag that the verifier uses |
| [T-1694](cli.md) | P2 | L | none | cli | done | Headless Chromium of Playwright runs as a proven payload with its flags in one profile |
| [T-1695](cli.md) | P1 | L | none | cli | done | One JSON report for each run, and real labels on records |
| [T-1696](cli.md) | P2 | M | none | cli | done | `--secret` places a file with no argv and no env, and a read-once mode |
| [T-1697](interpose.md) | P1 | L | none | interpose | done | A raw `statx` of libuv misses the path table, so npm cannot find its modules |
| [T-1698](extract.md) | P0 | S | none | extract | done | A whiteout of `..` deletes outside the destination |
| [T-1699](interpose.md) | P2 | S | none | interpose | done | `mkostemp` of a mapped template answers ERANGE, and npm prints a line for each cache entry |
| [T-1700](enter.md) | P1 | M | none | enter | done | A run on the userland rung sometimes exits 0 with no output |
| [T-1701](interpose.md) | P2 | S | none | interpose | done | `mkstemp64`, `mkostemp64`, `mkstemps` and `mkostemps` pass the path table |
| [T-1702](machine.md) | P2 | M | none | machine | done | The TCG guest of `podvm-workload` prints no console byte in 600 s on the cloud host |
| [T-1703](interpose.md) | P2 | L | none | interpose | done | The exec routes that T-1646 does not take still enter through the host |
| [T-1704](interpose.md) | P1 | S | none | interpose | done | `realpath` of `/proc/self/exe` names the image's loader, while `readlink` names the program |
| [T-1705](interpose.md) | P1 | M | none | interpose | done | A host path that a loaded object reports gets the rootfs prefix a second time |
| [T-1706](cli.md) | P2 | M | none | cli | done | The unmodified official `actions/runner` starts on the userland rung |
| [T-1707](enter.md) | P1 | S | none | enter | done | A relative first program resolves against the image root, not against `-w` |
| [T-1708](supervise.md) | P1 | M | none | supervise | done | `stop`, `kill` and `rm -f` end the whole tree: the launcher is a subreaper |
| [T-1709](image.md) | P1 | L | none | image | done | Each container gets its own rootfs: a write in one container is not read by another |
| [T-1710](interpose.md) | P1 | M | none | interpose | done | A Unix socket path in `connect` and `bind` passes the path table |
| [T-1711](cli.md) | P2 | L | none | cli | done | The calls of the four container-hook commands run through podvm verbs on the userland rung |
| [T-1712](cli.md) | P1 | S | none | cli | done | `--podvm-tier=chroot` does nothing: it runs the ladder |
| [T-1713](cli.md) | P1 | S | none | cli | done | `.EnteredRung` reads `chroot` where the probe selects `supervise` and chroot is denied |
| [T-1714](probe.md) | P2 | S | none | probe | done | `podvm probe --json` validates against the published schema of `pod-hostprobe` |
| [T-1715](supervise.md) | P2 | S | none | supervise | done | The container table keeps `-e` values in a file that other users can read |
| [T-1716](enter.md) | P2 | S | none | enter | done | Take the `pod-enter` change of podcrates podcrates@T-155: `Maps::new` takes `&Names` and names the volume of a missing `-w` |
| [T-1717](machine.md) | P2 | L | none | machine | done | Study incus without root and without heavy overhead, with errand-box, before an incus back end |
| [T-1718](machine.md) | P2 | L | none | machine | done | An incus back end: podvm drives incus over its REST API |
| [T-1719](enter.md) | P2 | S | none | enter | done | The loader's library directories leave out `/usr/local/lib` and the image's `ld.so.conf` |
| [T-1720](supervise.md) | P1 | S | none | supervise | done | `wait` gives no exit code for a payload that ends at once |
| [T-1721](interpose.md) | P2 | L | none | interpose | done | An interposer for aarch64: the userland rung runs no arm64 payload |
| [T-1722](machine.md) | P3 | L | none | machine | done | The incus back end: the verbs of an instance that stays, volumes and the ladder |
| [T-1723](machine.md) | P3 | L | none | machine | done | The microVM of run: the microvm machine, a shared /workspace and an interactive stdin |
| [T-1724](enter.md) | P2 | M | none | enter | done | A static child of the loader family, and a volume, on the native-static rung |
| [T-1725](interpose.md) | P1 | M | none | interpose | done | An absolute symlink in the rootfs resolves on the host for each path call |
| [T-1726](cli.md) | P1 | S | none | cli | done | `exec` into a container ignores its `:ro` volumes and `--read-only` |
| [T-1727](machine.md) | P2 | S | none | machine | done | Take podcrates v0.21.0: `pod-incus` replaces the incus client of T-1718 |
| [T-1728](machine.md) | P2 | S | none | machine | done | Without `-t`, a command of the machine tier writes to a terminal |
| [T-1729](enter.md) | P3 | M | none | enter | done | An optional `runsc` backend where a host grants user namespaces |
| [T-1730](enter.md) | P1 | S | none | enter | done | A detached thread of a static payload ends with SIGSEGV on the native-static rung |
| [T-1731](enter.md) | P2 | L | none | enter | done | A dynamic program on the native-static rung: its loader under the handler |
| [T-1732](enter.md) | P1 | S | none | enter | done | Take podcrates v0.22.0: a detached payload ends with its launcher |
| [T-1733](podssh.md) | P3 | S | none | podssh | done | Retire the experiment 388-interactive-shell.sh for the tests of `session_interactive.rs` |
| [T-1734](extract.md) | P3 | S | none | extract | done | Retire the experiment 220-extract-path-safety.sh, and point its eight citations at the tests |
| [T-1735](complete.md) | P2 | M | none | complete | done | The dangling-directory-link test and the reachability test, then retire `85-completion-symlink-escape.sh` |
| [T-1736](machine.md) | P3 | S | none | machine | done | A test of the refusal of `windows --dos` with no DOS base image |
| [T-1737](packaging.md) | P3 | M | none | packaging | done | Decide where the SSH packaging scripts go, and port scripts/package-ssh.sh into `podvm-release` |
| [T-1738](interpose.md) | P1 | S | none | interpose | done | A buffer of the interposer reaches the free of a program that has its own allocator |
| [T-1739](interpose.md) | P2 | M | none | interpose | done | Chromium's child processes do not start on the loader family |
| [T-1740](enter.md) | P3 | M | none | enter | partial | Raw system calls of a dynamic program on the loader family: measure the syscall tier, then choose the default |
| [T-1741](image.md) | P1 | S | none | image | done | A plain tar layer whose end padding is past the last read is refused as a digest mismatch |
| [T-1742](enter.md) | P2 | S | none | enter | done | The loader family does not find the glibc of a Debian image on arm64 |
| [T-1743](interpose.md) | P1 | S | none | interpose | done | GNU tar with an absolute `-C` opens the directory on the host: the fortified opens skip the table |
| [T-1744](interpose.md) | P2 | S | none | interpose | done | The open flags of the interposer have their x86_64 values on aarch64 |
| [T-1745](cli.md) | P3 | L | none | cli | done | A Rust call for `run` with a typed request that returns the record of `--report` |
| [T-1746](probe.md) | P2 | M | none | probe | done | `run` reads the cached probe, and probes again only when its key changes |
| [T-1747](cli.md) | P3 | M | none | cli | done | `--memory`, `--cpus` and `--pids-limit` by cgroup2 where a subtree is delegated |
| [T-1748](supervise.md) | P2 | S | none | supervise | done | A foreground payload dies when its `podvm run` is killed with SIGKILL |
| [T-1749](enter.md) | P3 | L | none | enter | partial | A read-only image as the lower layer of the root, with a tmpfs upper layer, and no copy |
| [T-1750](machine.md) | P3 | L | none | machine | done | `exec` into a running guest of the machine tier |
| [T-1751](machine.md) | P1 | L | none | machine | done | The guest plan of shield `guest`: no host file in the guest, no network card, one serial channel |
| [T-1752](cli.md) | P3 | M | none | cli | done | A versioned JSON Schema for the record of `run --report`, published in each release |
| [T-1753](interpose.md) | P1 | M | none | interpose | done | The userland rung reads the host's /etc/hosts, /etc/passwd and /etc/group through libc's internal opens |
| [T-1754](interpose.md) | P1 | S | none | interpose | done | `scandir64` and other 64-bit directory calls skip the path table: node lists the host's /etc, npm install fails |
| [T-1755](interpose.md) | P1 | S | none | interpose | done | `lutimes` skips the path table: dpkg fails on each package, so apt-get install fails |
| [T-1756](interpose.md) | P2 | S | none | interpose | done | A trigger of apk runs `/bin/busybox` by an absolute path that is not anchored |
| [T-1757](cli.md) | P1 | S | none | cli | done | `-u` is dropped on the namespace rung when the image has an ENTRYPOINT: the payload runs as root |
| [T-1758](enter.md) | P1 | M | none | enter | done | The namespace rung has no /proc in the root: go cannot find GOROOT, /proc/self/status is missing |
| [T-1759](enter.md) | P1 | M | none | enter | done | No /dev/shm on either rung: python multiprocessing fails |
| [T-1760](cli.md) | P2 | M | none | cli | done | `-t` gives no terminal on the namespace rung, and says nothing |
| [T-1761](cli.md) | P1 | M | none | cli | done | `-v`, `--tmpfs` and `--read-only` are refused on the namespace rung with a message that names the chroot rung |
| [T-1762](image.md) | P1 | M | none | image | done | The source fixup probes four HTTPS hosts on each run: about 2 s of each ubuntu run, uncached |
| [T-1763](image.md) | P2 | S | none | image | done | The `openssl rehash` step prints 108 lines on each python run and costs about 0.5 s |
| [T-1764](enter.md) | P2 | S | none | enter | done | `--podvm-confine` without /etc in the read list breaks the loader on the namespace rung |
| [T-1765](cli.md) | P1 | S | none | cli | done | `-e NAME` with no value does not pass the host's variable, and `exec` does not take the environment of `run` |
| [T-1766](cli.md) | P2 | S | none | cli | done | `--strict` and `probe` disagree with the rung that a run enters on the userland fixture |
| [T-1767](image.md) | P2 | S | none | image | done | Quiet runs print notes: a false chown EINVAL on extraction, the probe line of `pull -q`, the base archive of the machine tier |
| [T-1768](image.md) | P2 | S | none | image | done | `cp -r` of a container path copies each hard link as a file |
| [T-1769](cli.md) | P3 | S | none | cli | done | An unknown verb or flag prints the whole usage |
| [T-1770](net.md) | P3 | S | none | net | done | `podvm lab`: a knob line with an empty value, and the words of the errors of the knobs |
| [T-1771](image.md) | P2 | S | none | image | done | node does not trust the host's CA that podvm appends, so npm fails behind an intercepting proxy |
| [T-1772](cli.md) | P2 | S | none | cli | done | `-w` with a directory that the image lacks is refused; docker creates it |
| [T-1773](image.md) | P2 | S | none | image | done | A step of the completion fails on each run of the userland rung |
| [T-1774](image.md) | P3 | S | none | image | done | busybox `head -c` of `/dev/urandom` stops at 1 MiB on the userland rung |
| [T-1775](cli.md) | P1 | S | none | cli | done | `cp` into a running container of the namespace rung writes under the run's tmpfs |
| [T-1776](machine.md) | P1 | S | none | machine | done | Each file of a guest of the machine tier belongs to root |
| [T-1777](machine.md) | P1 | S | none | machine | done | The incus tier takes no stdin for an instance that stays |
| [T-1778](machine.md) | P1 | S | none | machine | done | The channel of a guest gets no request through a loopback proxy variable |
| [T-1779](machine.md) | P1 | S | none | machine | done | The incus tier takes `--podvm-net none` and `--podvm-channel` |
| [T-1780](machine.md) | P1 | M | none | machine | done | A guest of the machine tier runs a container engine |
| [T-1781](image.md) | P2 | M | none | image | done | A spare tree of each image, and the removal of a `--rm` tree after podvm's exit |
| [T-1782](image.md) | P1 | S | none | image | done | `load` spools stdin to a predictable name of `$TMPDIR` |
| [T-1783](cli.md) | P2 | S | none | cli | done | Nine verbs print a usage error to stdout |
| [T-1784](interpose.md) | P2 | S | none | interpose | done | The honest refusal of an identity call says that the payload runs as uid 0 |
| [T-1785](interpose.md) | P3 | S | none | interpose | done | The notice of `--user` repeats in each process of a run |
| [T-1786](supervise.md) | P1 | M | none | supervise | done | A foreground run is recorded `Created` while its payload runs |
| [T-1787](image.md) | P2 | S | none | image | done | `system df` misses the trees and the extractions that no image names, and no verb removes the latter |
| [T-1800](supervise.md) | P0 | S | none | supervise | done | `ps` aborts with SIGABRT when a command has a multibyte character at byte 17 |
| [T-1801](cli.md) | P0 | S | none | cli | done | `--report` after the image is read as podvm's, so a payload argument overwrites a host file |
| [T-1802](cli.md) | P0 | S | none | cli | done | A run refused before `prepare` writes the `-e` values into its report |
| [T-1803](machine.md) | P1 | M | none | machine | done | The `--report` of a machine-tier run reads as a refusal with no rung, image or exit code |
| [T-1804](enter.md) | P0 | S | none | enter | done | `--read-only` with `-v HOST:/GUEST/:ro` leaves the volume writable |
| [T-1805](supervise.md) | P1 | M | none | supervise | done | A name that resolves to nothing exits 125 on the lifecycle verbs, where docker and podvm's own table say 1 |
| [T-1806](supervise.md) | P1 | S | none | supervise | done | A foreground run records only stdout, so `logs` loses each stderr line |
| [T-1807](supervise.md) | P1 | S | none | supervise | done | A restarted container shows the exit code and end of its previous run while it runs |
| [T-1808](machine.md) | P0 | M | none | machine | done | The machine tier accepts `--podvm-confine` and eight hardening flags and drops them |
| [T-1809](machine.md) | P1 | S | none | machine | done | `--timeout` on the machine tier kills 10 s late and exits 125 in place of 137 |
| [T-1810](machine.md) | P0 | S | none | machine | done | A sealed bundle member that leaves the guest root is skipped and the run exits 0 |
| [T-1811](cli.md) | P1 | S | none | cli | done | `--podvm-net=V` and `--podvm-channel=V` are refused as "a bug in podvm", and so is `--podvm-tier` on `start`, `ps` and `rm` |
| [T-1812](probe.md) | P1 | S | none | probe | done | `probe --cached` reports `supervise` where `probe` reports `userland` |
| [T-1813](extract.md) | P1 | S | none | extract | done | Extraction drops the setuid and setgid bits of a file whose mode already has owner write |
| [T-1814](image.md) | P2 | S | none | image | done | `system df` prints the manifest digest in its IMAGE ID column |
| [T-1815](cli.md) | P1 | S | none | cli | done | `run -d -t` and `create -t` accept `-t` and never allocate a terminal |
| [T-1816](cli.md) | P1 | M | none | cli | done | A failed write to stdout aborts with SIGABRT, `images` exits 0 with its table lost, and `logs` exits 0 having written nothing |
| [T-1817](cli.md) | P1 | S | none | cli | done | `run -t` never delivers end of input, so a payload that reads to the end blocks |
| [T-1818](complete.md) | P0 | S | none | complete | done | `--no-host-cas` is ignored on an image that ships no CA bundle |
| [T-1819](enter.md) | P0 | M | none | enter | done | An `exec=` rule of `--podvm-confine` does not deny a program on the loader family |
| [T-1820](net.md) | P1 | M | none | net | done | An allowed host through `--network broker` answers 502 "the far end failed" |
| [T-1821](podssh.md) | P1 | S | none | podssh | done | `remote ssh serve` runs any program named `node` on PATH |
| [T-1822](supervise.md) | P2 | S | none | supervise | done | `cp` of a file onto a directory fails and blames the source, `cp -r` drops directory modes, and the containment refusals name the wrong operation |
| [T-1823](machine.md) | P2 | M | none | machine | done | An early end of the guest init reads "the guest gave no exit marker" with the socket of QEMU |
| [T-1824](cli.md) | P2 | S | none | cli | done | The machine notice ignores `-q`, and the banner is said to print "on every run" where runs are quiet by default |
| [T-1825](cli.md) | P2 | M | none | cli | done | A `--report` that cannot be written exits 0, and the report of a detached run is never finished |
| [T-1826](complete.md) | P2 | S | none | complete | done | `--no-steps` leaves no line and no report row that says what the caller gave up |
| [T-1827](cli.md) | P2 | S | none | cli | done | A repeated `--ulimit` that raises a hard limit, `--ulimit nofile=-1` and a hard value over podvm's own all fail with a bare EPERM |
| [T-1828](cli.md) | P2 | S | none | cli | done | `--add-host` takes a non-address, `-e =V` is dropped, `--env-file` removes quotes, and a repeated `--label` key keeps both values |
| [T-1829](cli.md) | P3 | S | none | cli | done | Refusals of `run` lead with the wrong cause or name the wrong spelling |
| [T-1830](cli.md) | P2 | S | none | cli | done | A `--tmpfs` run leaves its map in the record, so a later `exec` cannot see the path |
| [T-1831](cli.md) | P2 | S | none | cli | done | The help of `run` and `create` promises what the verbs do not do |
| [T-1832](cli.md) | P3 | S | none | cli | done | The manual, `--help` and `docs/limits.md` disagree with the binary in six places |
| [T-1833](cli.md) | P2 | S | none | cli | done | Twelve commands refuse `-h` and `--help`, and `system df --help` prints the usage of `system info` |
| [T-1834](cli.md) | P2 | S | none | cli | done | `system info --format '{{json .X}}'` prints counts and `StrictOk` as strings |
| [T-1835](probe.md) | P2 | S | none | probe | done | `compare --strict` passes an empty report, an unparsable one, or one that lost whole sections |
| [T-1836](image.md) | P3 | S | none | image | done | Six image messages name the wrong cause or repeat themselves |
| [T-1837](cli.md) | P3 | S | none | cli | done | The refusals of `lab`, `windows fetch`, `remote ssh` and `probe-inside` do not name the value, the unit or the verb |
| [T-1838](cli.md) | P3 | S | none | cli | done | The help of `windows` and `machine ssh` does not align its columns, and 21 sentences start with a lowercase `note:` |
| [T-1839](image.md) | P2 | S | none | image | done | `load` takes an index with `schemaVersion` 9 and names an image with no ref-name annotation `sha256` |
| [T-1840](image.md) | P2 | S | none | image | done | The `images` table cannot tell two platform records apart, `images REPO` shows only `latest`, and `inspect` has no `.Config.Labels` |
| [T-1841](image.md) | P2 | S | none | image | done | `save` advises a `--platform` it does not have, and `pull -a` records the endpoint host, which makes a duplicate record |
| [T-1842](image.md) | P2 | S | none | image | done | `save` and `import` exit 125 for an input error that their error type maps to 1, and a bad insecure-registry host is taken |
| [T-1843](image.md) | P2 | S | none | image | done | `extract --force` removes the extracted root before it takes the lock of the store |
| [T-1844](interpose.md) | P2 | S | none | interpose | done | `interpose::place` writes the object through one fixed temporary name in the root |
| [T-1845](cli.md) | P3 | S | none | cli | done | `profile show` writes a part of the document to stdout and then fails |
| [T-1846](machine.md) | P2 | M | none | machine | done | A failed incus run leaves its instance, `ps` omits instances, `rm` calls an ERROR instance running, and `system info` takes `tier = bogus` |
| [T-1847](net.md) | P3 | M | none | net | done | The scenarios and knobs of `lab` differ from the surface that sand-net documents |
| [T-1848](supervise.md) | P2 | M | none | supervise | done | No verb removes the exited containers in bulk, and `container` names verbs "listed above" that it does not list |
| [T-1849](machine.md) | P3 | S | none | machine | done | `run` does not say that the machine tier needs `qemu-system`, and the guest sees less memory than `--podvm-mem` asks |
| [T-1850](cli.md) | P3 | M | none | cli | done | `exec` takes 1.6 times the time of `docker exec`, and `pull` 1.4 times the time of `podman pull` |
| [T-1851](supervise.md) | P3 | S | none | supervise | done | A store that root and a user share refuses the next run of the user with a bare `Permission denied` |
| [T-1852](gate.md) | P1 | M | none | gate | done | The QA pass of experiment 443 has no row for the 22 defects that the external gate found |
| [T-1853](gate.md) | P2 | S | none | gate | done | No check proves that each parity row with a value takes `--flag=value`, and that each verb takes `-h` and `--help` |
| [T-1854](gate.md) | P1 | S | none | gate | done | No test proves that each option of `run` is honoured or refused on each tier |
| [T-1855](gate.md) | P2 | S | none | gate | done | The gate does not scan for the source patterns behind five defects |
| [T-1856](gate.md) | P2 | S | none | gate | done | No test reads the `--report` of each path of a run against its schema and for a secret |
| [T-1857](gate.md) | P3 | S | none | gate | done | A help text or a document can say "until T-NNNN" after that entry is done |
| [T-1858](supervise.md) | P3 | S | none | supervise | done | The `ps` table pads by characters, so a wide character puts its row out of line |
| [T-1859](gate.md) | P2 | S | none | gate | done | Tests leave their stores under `/tmp`, and an interrupted run leaves 1.3 GB each |
| [T-1860](gate.md) | P2 | S | none | gate | done | The tests of `podvm-interpose` fail when they run in parallel, and CI does not run them |
| [T-1861](machine.md) | P1 | M | none | machine | partial | `podvm run windows:NAME` has no image name, no catalog and no `pull`, `images` or `rmi` |
| [T-1862](machine.md) | P1 | M | none | machine | done | The download has no resume, no free-disk check and no `RLIMIT_FSIZE` check |
| [T-1863](machine.md) | P1 | M | none | machine | done | `windows:validationos` has no fetch by range, and the extent of the VHDX is a fixed sector |
| [T-1864](machine.md) | P1 | S | none | machine | done | The agent of a Windows guest runs the command with `call`, so a payload that ends in `exit` times out |
| [T-1865](machine.md) | P1 | M | none | machine | done | The plan of a Windows guest has no NIC, no CD-ROM, no USB media, no VNC socket and no serial channel |
| [T-1866](machine.md) | P1 | M | none | machine | done | There is no generator of the unattended answer file or of the USB FAT media |
| [T-1867](machine.md) | P1 | L | none | machine | done | `podvm pull windows:11` cannot install a full Windows guest |
| [T-1868](machine.md) | P2 | M | none | machine | done | A full Windows guest has no `exec`, `cp`, graceful `stop` or readiness probe |
| [T-1869](machine.md) | P2 | M | none | machine | partial | A Windows guest has no `-p`, no RDP check, no VNC, no screenshot and no `creds` |
| [T-1870](machine.md) | P2 | L | none | machine | partial | A Windows guest has no lifecycle: no `run -d`, `ps`, `stop`, `start`, `rm`, `logs` or `--name` |
| [T-1871](machine.md) | P1 | M | none | machine | done | `podvm windows doctor` is thin |
| [T-1872](machine.md) | P2 | M | none | machine | open | A Windows guest has no `-v`: no way to bring a folder in or out |
| [T-1873](machine.md) | P2 | L | none | machine | partial | `windows setup` sleeps the whole wait with no readiness check |
| [T-1874](machine.md) | P2 | L | none | machine | open | Each Windows run is a full boot: there is no warm start |
| [T-1875](machine.md) | P2 | L | none | machine | done | podvm needs `qemu-system-x86_64`, `qemu-img` and OVMF on the host |
| [T-1876](machine.md) | P2 | S | none | machine | done | Nothing checks the catalog pins for drift |
| [T-1877](machine.md) | P2 | M | none | machine | open | A Windows desktop has no Hyper-V enlightenments, and no one measured its boot time |
| [T-1878](machine.md) | P3 | L | none | machine | done | `podvm run -it windows:11 -- powershell`, streamed logs, a web viewer and Windows ARM64 are not planned in detail |
| [T-1879](machine.md) | P2 | S | none | machine | done | The messages of `dos.rs` and `doctor.rs` and the premise of T-1112 still say that podvm fetches no image |
| [T-1880](interpose.md) | P1 | M | none | interpose | done | The glibc object needs `libdl.so.2`, so the loader family refuses a glibc 2.34 image that ships none |
| [T-1881](extract.md) | P0 | M | none | extract | done | pod-extract masks the mode of a directory, keeps the first layer's mode, and its widening can chmod a directory outside the tree through a symlink |
| [T-1882](extract.md) | P2 | S | none | extract | done | pod-extract counts the `./` archive root as an entry it could not make |
| [T-1883](image.md) | P2 | S | none | image | done | `create` and `run` do not report the entries that the extraction could not make |
| [T-1884](machine.md) | P1 | S | none | machine | done | A machine run that podvm ends on SIGTERM leaves its emulator running |
| [T-1885](machine.md) | P2 | S | none | machine | done | Piped input to a machine run with `-t` is lost before the guest terminal opens |
| [T-1886](image.md) | P1 | S | none | image | done | Concurrent extractions of one image write one tree, and the first to fail removes it under the others |
| [T-1887](enter.md) | P1 | S | none | enter | done | `--podvm-confine` on the userland rung resolves an absolute link of the image against the host root |
| [T-1888](enter.md) | P2 | S | none | enter | done | A required confine path whose link leaves the tree still grants a host inode |
| [T-1889](machine.md) | P1 | S | none | machine | done | The machine tier boots an image of another architecture and ends with "no exit marker" |
| [T-1890](machine.md) | P2 | L | none | machine | done | The machine tier has no guest of another architecture |
| [T-1891](extract.md) | P1 | S | none | extract | done | A tree with a directory of mode 0555 cannot be removed by a user who is not root |
| [T-1892](enter.md) | P3 | S | none | enter | done | Two exec tables reach the payload: podvm's guest paths and the crate's host paths |
| [T-1893](machine.md) | P1 | S | none | machine | done | The provision of a Windows guest leaves the emulator running when the typing fails |
| [T-1894](machine.md) | P1 | S | none | machine | done | The provision of a Windows guest never types: the screen-settle readiness of podcrates v0.45.0 fails |
| [T-1895](image.md) | P1 | S | none | image | done | A reference by an index digest finds a tag record and a digest record of one manifest, and `save` calls them two images |
| [T-1896](enter.md) | P1 | S | none | enter | done | `run -d` of a static image where chroot is denied refuses: the native-static rung has no detached driver |
| [T-1897](machine.md) | P1 | M | none | machine | done | The incus tier and the native tiers hold no limit of memory, CPU or processes for a detached run |
| [T-1898](machine.md) | P3 | L | none | machine | open | A Windows guest has no terminal and no streamed output: `run -it windows:11 -- powershell` and `logs -f` of the console |
| [T-1899](machine.md) | P3 | L | none | machine | done | A Windows guest has no viewer of its screen in a browser |
| [T-1900](machine.md) | P3 | L | none | machine | open | A Windows guest follows the host architecture, so an arm64 host has no Windows guest |
| [T-1901](enter.md) | P1 | M | none | enter | done | A `:ro` volume is refused on the native-static rung, so a static job container of the runner fails where chroot is denied |
| [T-1902](enter.md) | P1 | M | none | enter | done | `--podvm-confine` stops name resolution in the payload, while TCP to an address works |
| [T-1903](probe.md) | P2 | S | none | probe | done | `podvm probe` takes userland on a host with user namespaces and KVM, and does not say why |
| [T-1904](image.md) | P3 | S | none | image | done | `podvm load` names a local image `docker.io/library/runner` |
| [T-1905](packaging.md) | P2 | S | none | packaging | done | The release has no attestation in GitHub's store |
| [T-1906](machine.md) | P1 | S | none | machine | done | The store keeps `machine/run-PID-*` after each guest of the machine tier |
| [T-1907](probe.md) | P3 | S | none | probe | done | Under QEMU user mode, `probe` says `kvm: yes` for the device of the host |
| [T-1908](enter.md) | P1 | M | none | enter | done | A payload has the host name of the host: no `--hostname` |
| [T-1909](probe.md) | P2 | M | none | probe | done | The namespace rung does not enter through a user namespace |
| [T-1910](enter.md) | P2 | M | none | enter | done | `exec` and `cp` do not join a run that podvm put in a user namespace |
| [T-1911](probe.md) | P3 | M | none | probe | partial | In the user namespace of a run, `-u UID` is faked: the map holds the caller only |
| [T-1912](probe.md) | P2 | S | none | probe | done | `run -d` and `create` of an unprivileged caller do not use the user namespace of pod-enter |
| [T-1913](cli.md) | P3 | S | none | cli | done | The store of a verb has no flag, only `PODVM_STORE` |
| [T-1914](enter.md) | P1 | S | none | enter | done | `run -d` refuses "a passed descriptor goes onto 17" when the parent holds descriptors 3 to 15 |
| [T-1915](interpose.md) | P1 | S | none | interpose | done | On the userland rung, `tar -x` fails when a symbolic link comes before its target |
| [T-1916](net.md) | P2 | L | none | net | partial | Lab profiles: each hostile host class as a profile that a project brings up |
| [T-1917](cli.md) | P3 | M | none | cli | partial | The help and the manual on the shared crate of PodUtils |
| [T-1918](cli.md) | P2 | S | none | cli | partial | podvm has no installer: the user downloads and checks the binary by hand |
| [T-1919](cli.md) | P2 | M | none | cli | done | The state of an issue is written by hand |
| [T-1920](enter.md) | P1 | M | none | enter | done | A volume moves a run off the namespace rung, and a raw syscall there sees host paths |
| [T-1921](probe.md) | P1 | S | none | probe | done | `chroot_usable` reads the leg in a user namespace as the caller's `chroot`, so `exec` enters into EPERM |
| [T-1922](machine.md) | P1 | M | none | machine | done | The machine tier ends before the exit code for a user with no root that `sudo -u` makes |
| [T-1923](probe.md) | P1 | S | none | probe | done | The namespace rung is taken through a user namespace where `chroot` is refused there |
| [T-1924](net.md) | P2 | S | none | net | done | `podvm lab run --way-out` for a live lane in a network class |
| [T-1925](cli.md) | P2 | M | none | cli | done | The installer takes the core of podcrates `v0.61.0`: `core-net.sh` and the signature of `SHA256SUMS` |
| [T-1926](enter.md) | P1 | S | none | enter | done | `run -d --hostname` refuses on the namespace rung that pod-enter enters |
| [T-1927](enter.md) | P1 | M | none | enter | done | On the namespace rung of a user with no root, the payload is uid 0 of a one-uid namespace, not the user of the image |
| [T-1928](interpose.md) | P0 | S | none | interpose | done | A payload that closes descriptor 17 and opens a file there gets the ownership memo read and written on that file |
| [T-1929](enter.md) | P3 | S | none | enter | blocked | `PODVM_MODE=rundir` and `tmpfs` refuse each tree of root: the device nodes of the fixups |
| [T-1930](enter.md) | P1 | M | none | enter | done | `run -d` on the namespace rung has no PID namespace: a job sees and signals the processes of the host |
| [T-1931](cli.md) | P2 | S | none | cli | done | A run connects to the hosts of the package sources of the image, and no document names it |
| [T-1932](machine.md) | P2 | S | none | machine | done | A foreground `run windows:NAME` writes its overlay into the tmpfs of the runtime directory |
| [T-1933](machine.md) | P1 | S | none | machine | done | The first `run windows:NAME` of a fresh golden image can miss its agent on 2 CPUs |
| [T-1934](enter.md) | P2 | S | none | enter | done | A unit on the namespace rung has no usable pty: no `/dev/ptmx`, and the host's `/dev/pts` |
| [T-1935](enter.md) | P1 | M | none | enter | partial | `run -d` on the userland rung: a raw path call of a dynamic program reaches the host |
| [T-1936](gate.md) | P1 | S | none | gate | done | A gate of the tools of a CI job: a raw syscall, a spawn, a pty, `uv` and `ruff` in a unit |
| [T-1937](cli.md) | P2 | M | none | cli | done | The external QA of `0.2.0-beta.31`: the argument checks of the CLI |
| [T-1938](supervise.md) | P1 | S | none | supervise | partial | The external QA of `0.2.0-beta.31`: two runs at once lose the table of containers or the exec probe |
| [T-1939](enter.md) | P2 | M | none | enter | done | The external QA of `0.2.0-beta.31`: `--device` and `run -d --rm` |
| [T-1940](image.md) | P2 | S | none | image | partial | The external QA of `0.2.0-beta.31`: `images --filter`, `load`, `verify` and `--add-host` |
| [T-1941](machine.md) | P3 | S | none | machine | open | The built-in `machine` profile asks for 1 GiB, which a file-size ceiling under 1 GiB refuses |
| [T-1942](gate.md) | P1 | M | none | gate | done | The external QA of `0.2.0-beta.31`: verify the second batch of reports |
| [T-1943](cli.md) | P1 | M | none | cli | done | The external QA of `0.2.0-beta.31`, second batch: panics, flags and messages |
| [T-1944](cli.md) | P2 | M | none | cli | done | The external QA of `0.2.0-beta.31`, second batch: `cp`, `import` and the helpers of credentials |
| [T-1945](cli.md) | P1 | M | none | cli | done | `exec --env-file`, and a `-v` source that is opened with no symbolic link (PodUtils/podrunner#277) |
| [T-1946](gate.md) | P1 | M | none | gate | done | The contract of the CLI: one table-driven harness of each verb |
| [T-1947](gate.md) | P1 | M | none | gate | partial | Fail safe at the edges: a panic hook, checked numbers, and the writes of the store |
| [T-1948](gate.md) | P2 | M | none | gate | done | Each banner and each inspect field says what the entered mechanism did |
| [T-1949](gate.md) | P1 | S | none | gate | done | The gate of a beta: workloads, the harness, an outside pass, and the known gaps |
| [T-1950](gate.md) | P1 | M | none | gate | done | The external QA of `0.2.0-beta.31`: verify the third batch of reports |
| [T-1951](cli.md) | P1 | M | none | cli | done | The external QA of `0.2.0-beta.31`, third batch: the output contract |
| [T-1952](image.md) | P2 | M | none | image | partial | The external QA of `0.2.0-beta.31`, third batch: images, `import`, `cp` and the store |
| [T-1953](enter.md) | P1 | M | none | enter | done | The external QA of `0.2.0-beta.31`, third batch: signals, rungs and runs |
| [T-1954](machine.md) | P2 | M | none | machine | open | The external QA of `0.2.0-beta.31`, third batch: the machine tier |
| [T-1955](deps.md) | P2 | M | none | deps | partial | Remove the copies of shared code that podcrates names (COORD-131 of podcrates) |
| [T-1956](enter.md) | P2 | L | none | enter | open | The donor study: syscall tiers |
| [T-1957](enter.md) | P2 | M | none | enter | open | The donor study: limits and cgroup kill |
| [T-1958](supervise.md) | P2 | L | none | supervise | open | The donor study: confinement and policy |
| [T-1959](machine.md) | P2 | L | none | machine | open | The donor study: the machine tier |
| [T-1960](image.md) | P2 | L | none | image | open | The donor study: images and store |
| [T-1961](net.md) | P3 | M | none | net | open | The donor study: the network gateway |
| [T-1962](probe.md) | P2 | M | none | probe | open | The donor study: host probes and namespaces |
| [T-1963](complete.md) | P3 | L | none | complete | open | The donor study: research backlog |
| [T-1964](machine.md) | P1 | M | none | machine | done | The incus tier cannot size the root disk of an instance |
| [T-1965](machine.md) | P1 | M | none | machine | done | The incus tier gives no CPU time and no peak memory of an instance |
| [T-1966](machine.md) | P2 | M | none | machine | done | The incus tier cannot run a container engine in an instance |
| [T-1967](machine.md) | P1 | S | none | machine | done | The machine tier has no limit of processes in a guest |
| [T-1968](machine.md) | P2 | L | none | machine | done | No tier gives a sized writable disk to a unit |
| [T-1969](supervise.md) | P1 | M | none | supervise | done | The run report holds no OOM fact of a unit |
| [T-1970](cli.md) | P1 | M | none | cli | partial | The outside pass of 2026-11-18: three P1 and seven P2 faults on the path of a unit |
| [T-1971](cli.md) | P1 | S | none | cli | done | `--env-file` cannot hold a value with a newline |
| [T-1972](enter.md) | P1 | L | none | enter | done | `--podvm-volume-base` holds on the namespace rung only, and job containers of podrunner run on the userland and supervise rungs |
| [T-1973](enter.md) | P1 | S | none | enter | done | On the syscall tier, a payload cannot connect to a Unix socket of a volume |
| [T-1974](cli.md) | P1 | M | none | cli | partial | The outside pass of 2026-11-18, the second: a P0 and four P1 faults of the new surface |
| [T-1975](enter.md) | P1 | M | none | enter | done | On the syscall tier, a link inside a volume that names an absolute path or leaves the volume does not resolve |
| [T-1976](enter.md) | P1 | M | none | enter | done | The handler of the syscall tier overflows a small alternate signal stack of the payload |
| [T-1977](enter.md) | P1 | S | none | enter | done | On the syscall tier, a relative path of a glibc program through an absolute link resolves on the host |
| [T-1978](enter.md) | P1 | S | none | enter | done | On the syscall tier, a child of posix_spawn cannot run a link in a volume |
| [T-1979](enter.md) | P1 | S | none | enter | done | On the default userland rung, a link that the payload makes in a volume holds the host path of the tree of the run |
| [T-1980](cli.md) | P2 | M | none | cli | open | The faults of the third outside pass (2026-11-18, the gate of beta.34) |
| [T-1981](enter.md) | P1 | S | none | enter | done | On the userland rung, a program whose PT_INTERP lies past the first 64 KiB is refused (Fedora 42) |
| [T-1982](enter.md) | P1 | S | none | enter | done | `run -d` with `--podvm-confine` on the syscall tier cannot start its payload |
| [T-1983](enter.md) | P1 | M | none | enter | done | On the syscall tier, `--podvm-confine exec=` is not enforced |
| [T-1984](enter.md) | P2 | M | none | enter | done | `--podvm-confine exec=` is bypassed through the dynamic loader |
| [T-1985](enter.md) | P2 | M | none | enter | done | A payload under `--podvm-confine` changes the mode and the times of a file that no rule names |
| [T-1986](enter.md) | P1 | S | none | enter | done | On the userland rung, `dnf` of Fedora 42 cannot write its state under `/root` |
| [T-1987](enter.md) | P1 | S | none | enter | done | On AlmaLinux 9, `ls -l` dies with SIGSEGV in the interposer's `lgetxattr` |
| [T-1988](enter.md) | P1 | S | none | enter | done | On the syscall tier, a volume whose guest path is not in the image cannot be stat-ed |
| [T-1989](cli.md) | P2 | M | none | cli | open | The faults of the fourth outside pass (2026-11-18, the gate of beta.35) |
| [T-1990](enter.md) | P1 | S | none | enter | partial | On the syscall tier, a dynamic program sometimes fails to start with "errno 11" |
| [T-1991](enter.md) | P1 | M | none | enter | done | On the syscall tier, `cargo doc` of ripgrep prints "Compiling ripgrep" and then nothing for 45 min |
| [T-1992](cli.md) | P1 | M | none | cli | done | Each release publishes its binaries on ghcr.io |
| [T-1993](machine.md) | P1 | M | none | machine | done | On the incus tier, the init of the image (systemd) runs as PID 1 |
| [T-1994](machine.md) | P1 | L | none | machine | partial | On the machine tier, a root on a disk for each image and the init of the image |
| [T-1995](enter.md) | P1 | S | none | enter | done | A Go program hangs on the userland rung at its first signal of preemption |
| [T-1996](machine.md) | P1 | M | none | machine | partial | On the incus tier, an instance of the hosted snapshot has 231,072 ids, `/dev/fuse` and a machine-id of its own |
| [T-1997](machine.md) | P1 | M | none | machine | done | On the incus tier, the import of a large image keeps no third copy |
| [T-1998](machine.md) | P1 | L | none | machine | open | On the incus tier, an OCI image with a kernel runs as a virtual machine |
| [T-1999](machine.md) | P1 | M | none | machine | partial | The `security.capability` xattrs of an image stay on the incus tier and the machine tier |
| [T-2000](interpose.md) | P1 | S | none | interpose | done | `apt-get install` fails on the userland rung: dpkg's `tar` has no ownership memo |
| [T-2001](cli.md) | P2 | M | none | cli | done | The faults of the fifth outside pass (2026-11-19, the gate of beta.36) |
| [T-2002](enter.md) | P1 | L | none | enter | partial | `--podvm-init=image` holds only on the incus tier: the namespace rung maps one uid, and the userland rung has no PID 1 |
| [T-2003](enter.md) | P2 | M | none | enter | done | On the userland rung, a program with a library in a directory of its RUNPATH does not load (`systemctl`) |
| [T-2004](enter.md) | P2 | M | none | enter | partial | A delegated cgroup2 subtree from the user manager of systemd, for an init of the image on the namespace rung |
| [T-2005](enter.md) | P2 | L | none | enter | blocked | An init of the image with no guest on a host that delegates no cgroup: a cgroup2 stand-in, or User-Mode Linux |
| [T-2006](supervise.md) | P1 | S | none | supervise | done | A SIGKILL of podvm leaves the processes of a foreground run in its PID namespace |
| [T-2007](enter.md) | P1 | M | none | enter | partial | The init of the image as PID 1 of a gVisor sandbox: the runsc tier, for a host with a user namespace and no delegated cgroup |
| [T-2008](machine.md) | P3 | L | none | machine | partial | The machine tier boots systemd at each run: a saved state of the guest for each digest |
| [T-2009](enter.md) | P2 | M | none | enter | blocked | podvm fetches no runsc: the gVisor route of an init is there only where the host has one |
| [T-2010](enter.md) | P1 | L | none | enter | partial | Each route of `--podvm-init=image` as fast as it can be: a measured baseline, then each gain |
| [T-2011](machine.md) | P1 | S | none | machine | done | `run -d --podvm-init=image` of an image whose Entrypoint is its init runs `/sbin/init CMD` and not the command |
| [T-2012](enter.md) | P1 | S | none | enter | done | `exec` into a `run -d` container of the namespace rung refuses its volumes where the probe of the host denies chroot |
| [T-2013](enter.md) | P1 | S | none | enter | done | On the userland rung, `--podvm-confine exec=` does not hold for the first program of a run |
| [T-2014](enter.md) | P2 | S | none | enter | done | On the userland rung, `--podvm-confine read=/` does not cover a volume |
| [T-2015](enter.md) | P2 | M | none | enter | done | On the syscall tier, the interpreter of a `#!` script that the payload starts is not checked against `exec=` |
| [T-2016](machine.md) | P2 | M | none | machine | partial | QEMU before 10.0.4 can end on SIGSEGV in the boot of a guest with more than one vCPU under TCG |
| [T-2017](enter.md) | P1 | M | none | enter | partial | A run with the init of the image on the namespace rung dropped `--podvm-confine`, `--ulimit`, no-new-privileges and `--device` with no line |
| [T-2018](supervise.md) | P3 | S | none | supervise | done | The launcher waits for one process of the tree at a time, and the init of a PID namespace waits for the reap of a process that the launcher holds |
| [T-2040](enter.md) | P1 | M | none | enter | done | On the userland rung, a payload writes a `:ro` volume through its host path |
| [T-2041](machine.md) | P1 | M | none | machine | done | On the machine tier, a SIGTERM, SIGINT or SIGHUP to a foreground run ends podvm and does not reach the payload |
| [T-2042](machine.md) | P1 | M | none | machine | done | After `run -d --podvm-tier=machine`, `inspect`, `kill`, `wait` and the filters of `ps` do not find the container |
| [T-2043](cli.md) | P1 | S | none | cli | done | `exec -t` gives the command no terminal and does not refuse |
| [T-2044](cli.md) | P2 | M | none | cli | partial | The faults of the sixth outside pass (2026-11-20, the gate of the next beta) |
| [T-2045](enter.md) | P1 | M | none | enter | done | On the userland rung, a link to the host path of the rootfs with `..` reads host files |
| [T-2046](cli.md) | P1 | S | none | cli | done | `logs` loads the whole log into memory and has no bound of bytes |
| [T-2047](machine.md) | P2 | M | none | machine | done | The memory that a guest of the machine tier needs for an initrd of N bytes, as a curve |
| [T-2048](machine.md) | P1 | M | none | machine | done | The save of the state of a guest hangs the run with no line when the monitor of the emulator answers late |
| [T-2050](deps.md) | P2 | L | none | deps | done | Take podcrates `v0.80.0`: the write table, `pod-beneath`, the loader rule, five `#!` levels, the signals, the saved state of a line, the static QEMU 11.0.0 |
| [T-2051](machine.md) | P2 | M | none | machine | done | The save and the load of the Linux guest of the machine tier keep their own code beside `pod_guest::warm::line` |
| [T-2052](cli.md) | P2 | S | none | cli | done | `run -it`: the interrupt key does not stop the foreground child of the payload |
| [T-2053](cli.md) | P2 | M | none | cli | done | The banner, `--strict` and `inspect` of the namespace rung deny the PID namespace and the `/proc` that the run has |
| [T-2054](enter.md) | P2 | M | none | enter | partial | The namespace rung: podvm's argv, its environment, host paths and descriptors of the store reach the payload |
| [T-2055](enter.md) | P2 | S | none | enter | done | `--podvm-volume-base` on the loader family: a link under DIR resolves into the image, not to ELOOP |
| [T-2060](cli.md) | P2 | S | none | cli | done | The sixth outside pass, OQ-06: `inspect --format` gives lists and maps as flat strings |
| [T-2061](cli.md) | P2 | S | none | cli | done | The sixth outside pass, OQ-09: docker differences of `-t`, `exec`, `logs`, `-v` and the manual |
| [T-2062](cli.md) | P2 | S | none | cli | done | The sixth outside pass, OQ-10: exit codes are not the documented ones |
| [T-2063](cli.md) | P3 | S | none | cli | done | The sixth outside pass, OQ-12: a SIGKILL of podvm leaves `staging/memo-*.tmp` |
| [T-2064](cli.md) | P3 | S | none | cli | done | The sixth outside pass, OQ-13: texts of the manual and of messages |
| [T-2065](image.md) | P1 | S | none | image | done | A layer whose PAX record `SCHILY.acl.access` holds a newline is refused whole |
| [T-2066](deps.md) | P2 | L | none | deps | done | Take podcrates `v0.81.0`: the warm line with a bounded connect, the static QEMU 11.1.3, the streamed log reads, one atomic replace of a file |
| [T-2067](gate.md) | P2 | S | none | gate | done | Tests of `podvm` and the machine tier fail with a long `TMPDIR`: a probe in the working directory, socket paths longer than `sun_path` |
| [T-2068](image.md) | P1 | S | none | image | done | `load` of an image of 14 layers or more is refused: the load holds one lock for each blob |
| [T-2069](image.md) | P2 | S | none | image | done | The staging files of the store: a name that a pid alone makes unique, and a lock that `put_bytes` drops before its rename |
| [T-2070](cli.md) | P1 | M | none | cli | partial | The faults of the seventh outside pass (2026-11-23, the gate of the next beta) |
| [T-2071](cli.md) | P1 | S | none | cli | done | `exec -it CTR sh`: a job-control shell takes the caller's terminal, and the relay gets no more input |
| [T-2072](cli.md) | P1 | S | none | cli | done | `-v DIR:/etc` and `-v DIR:/dev` write podvm's fixups into DIR |
| [T-2073](cli.md) | P2 | S | none | cli | done | A foreground `run` reports the owner of the store for each failure of its record |
| [T-2074](cli.md) | P2 | S | none | cli | done | `run -d --rm` keeps the container after its end |
| [T-2075](cli.md) | P2 | S | none | cli | done | Each `exec` rewrites `/etc/hosts` and drops the lines of `--add-host` and `--hostname` |
| [T-2076](machine.md) | P1 | M | none | machine | done | `inspect` of a stopped instance of the incus tier gives a dash for the CPU time, the peak memory and the OOM kills |
| [T-2077](machine.md) | P1 | M | none | machine | done | Under `--memory`, the init of an incus instance can end with no exit file: `inspect` gives no exit code |
| [T-2078](machine.md) | P1 | M | none | machine | partial | On the incus tier, a full storage pool gives "the response body is larger than allowed" with no word of the cause |
| [T-2079](cli.md) | P1 | M | none | cli | partial | The faults of the eighth outside pass (2026-11-24, the gate of 0.2.0-beta.39) |
| [T-2080](machine.md) | P1 | M | none | machine | done | On the incus tier, an exit of 126 or 127 is a refusal of incusd, and a signal leaves the instance running |
| [T-2081](machine.md) | P1 | S | none | machine | done | Parallel first runs of one image on the incus tier race on its import |
| [T-2082](machine.md) | P1 | S | none | machine | done | On the incus tier, an upload of the image that incusd cuts ends podvm with SIGPIPE and no line |
| [T-2083](machine.md) | P1 | S | none | machine | done | On the incus tier, `--podvm-init=image` takes "offline" of systemd as final and removes the instance |
| [T-2084](deps.md) | P2 | M | none | deps | done | Take podcrates `v0.82.1`: the title and the dump flag of the init and the relay of pod-enter, the join of the payload, the meter of a cgroup |
| [T-2085](machine.md) | P1 | M | none | machine | done | On the incus tier, an import of a large image keeps three copies of its tree on one file system |
| [T-2086](machine.md) | P1 | M | none | machine | done | On a hosted runner with cgroup v2, `inspect` of an ended incus instance gives no peak memory and no OOM kill of a child |
| [T-2087](cli.md) | P1 | M | none | cli | partial | The faults of the ninth outside pass (2026-11-24, the gate of 0.2.0-beta.40) |
| [T-2088](cli.md) | P2 | M | none | cli | open | The faults of the external QA of `0.2.0-beta.31` that stay open on `0.2.0-beta.40` |
| [T-2089](deps.md) | P1 | M | none | deps | done | Take podcrates `v0.83.0`: the classes and the bounds of pod-incus, the exit of an exec with 126 or 127, the mounts of a cgroup, the shared watchers |
| [T-2090](gate.md) | P2 | S | none | gate | done | The external QA of `0.2.0-beta.31`: the reports that `0.2.0-beta.40` passes |
| [T-2091](cli.md) | P2 | S | none | cli | done | `podvm-dev build` takes the error of a failed `build-state.py commit` as the commit |
| [T-2092](cli.md) | P2 | S | none | cli | done | The documents, the probe field and the refusal of `--pid` say that the namespace rung opens no user, PID or UTS namespace |
| [T-2093](machine.md) | P2 | M | none | machine | done | `podvm update --podvm-tier=incus NAME` changes the memory, CPU and process limits of a running instance |
| [T-2094](machine.md) | P2 | L | none | machine | done | The saved state of the machine tier serves a run with `--podvm-channel` and `--pids-limit` |
| [T-2095](gate.md) | P1 | M | none | gate | done | Claude Code and Codex share hosted setup and blind QA |
| [T-2096](deps.md) | P2 | M | none | deps | done | Take podcrates `v0.84.0` and its shared gzip writer |
| [T-2097](deps.md) | P2 | M | none | deps | done | Take podcrates `v0.85.0` and keep the causes of failures |
| [T-2098](image.md) | P1 | S | none | image | done | An OCI archive can name the same verified layer more than once |
| [T-2099](machine.md) | P2 | L | none | machine | partial | The bytes that a payload reads through each volume, in `--report` |
| [T-2100](cli.md) | P2 | M | none | cli | partial | The faults of the tenth outside pass (2026-11-26, the gate of the beta after 0.2.0-beta.40) |
| [T-2101](enter.md) | P2 | L | none | enter | done | `-v HOST:GUEST:overlay`: a read-only host directory with a writable layer of the container |
| [T-2102](deps.md) | P2 | M | none | deps | done | Take podcrates `v0.86.0` and the size-limited tmpfs of pod-enter |
| [T-2103](machine.md) | P2 | M | none | machine | done | A writable host volume that the host user can remove, on the machine and incus tiers |
| [T-2104](image.md) | P2 | S | none | image | done | `inspect` gives the unpacked bytes of an image before its first run |
| [T-2105](enter.md) | P1 | M | none | enter | open | On the namespace rung, the payload can remount a `:ro` volume writable and write the host file |

## Counts

676 entries: 20 open, 40 partial, 5 blocked, 611 done.

Counted from the rows above by
`./target/x86_64-unknown-linux-musl/release/podvm-count` and asserted
independently by `./target/x86_64-unknown-linux-musl/release/podvm-gate`, which is
the gate. A number here that disagrees with the rows cannot reach a commit.

| Priority | Open | Partial | Blocked | Done | Total |
| --- | --- | --- | --- | --- | --- |
| P0 | 0 | 0 | 0 | 75 | 75 |
| P1 | 2 | 19 | 0 | 276 | 297 |
| P2 | 13 | 16 | 4 | 215 | 248 |
| P3 | 5 | 5 | 1 | 45 | 56 |
| **All** | 20 | 40 | 5 | 611 | 676 |

## Work order

[PROGRESS.md](PROGRESS.md) gives the current order. A result that reports
success for a wrong answer takes priority over a visible failure. Each entry
names its own proof. The old ordering text is kept in
[`docs/history/index-order-before-2026-09-28.txt`](../docs/history/index-order-before-2026-09-28.txt).
