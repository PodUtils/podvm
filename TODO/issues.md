# Issues

Each GitHub issue and open pull request of the record, with its entries.
The State is the state on GitHub: `open`, `closed` or `deleted`.
`TODO/github.txt` lists the numbers: each number from #127 on, and each
open number before #127. The record does not hold the closed issues
before #127; their text is on GitHub, and the hidden ones are in
[the hidden issues](../docs/history/github-hidden-issues-2026-11-06.md).
[RULES.md](RULES.md) gives the rules of the record.

| Issue | Subject | State | Entries |
| --- | --- | --- | --- |
| #86 | [T-1317] userland: relative paths are not virtualised, so every relative open/stat/exec fails ENOENT | closed | T-1643 |
| #87 | [T-1317] every glibc payload is refused: the ABI check resolves imports against libc alone, not the payload's NEEDED set | closed | T-1644 |
| #88 | [T-1317] interposer does not intercept dlopen/dlmopen, so a dynamic payload cannot load guest shared objects | closed | T-1645 |
| #89 | [T-1317] interposer execve maps the path but not the loader, so every dynamic grandchild fails 127: promote clause 17 to a gate | closed | T-1646 |
| #90 | [T-1317] a 128 MiB payload-read ceiling refuses node:alpine (149 MB) and any large runtime | closed | T-1647 |
| #91 | [T-1409] a static Go payload is declined and then run into a GOROOT error instead of a named refusal | closed | T-1648 |
| #92 | [QOL] node:alpine is unusable without an undocumented --entrypoint; the #! refusal should name the way out | closed | T-1649 |
| #93 | [CLI] --strict does not refuse the userland grandchild limit, while --strict=all refuses every run | closed | T-1650 |
| #94 | [T-1317] static payloads have no rung: add an emulate tier on a prefix-preserving qemu-user | closed | T-1651 |
| #109 | [T-1317] static payloads can have a native rung with no ptrace and no qemu: userland-execve + Syscall User Dispatch + openat2(RESOLVE_IN_ROOT) | closed | T-1652 |
| #114 | [Reference] Dossier: podbox on the constrained errand host: every limit -> mechanism -> workaround (measured) | closed (lesson taken in T-1643 to T-1653) | T-1643 to T-1653 |
| #127 | [Recovered #95] a dynamic grandchild re-enters through the host loader: the loader substitution must cover children | closed | T-1646 |
| #128 | [Recovered #96] the absolute lib64/ld-linux-x86-64.so.2 symlink resolves on the host and decides the libc | closed | T-1644 |
| #129 | [Recovered #97] the libc ABI union omits ld-linux-x86-64.so.2 and libdl.so.2, so every glibc payload is refused | closed | T-1644 |
| #130 | [Recovered #104] no -v volumes on a host with no /dev/fuse and no mount: a two-root view, not an overlay | closed | T-1653 |
| #131 | [Recovered #105] a relative path gets the rootfs prefix twice when the cwd is already inside the rootfs | closed | T-1643 |
| #132 | [Reference] 89luca89/lilipod @ 872755a: precheck and exec lessons; refused as seed | closed (lesson taken in T-0502, T-0603, T-0806) | none: a reference (references/README.md) |
| #133 | [Reference] Azathothas/TEMPLATE @ 6206166: methodology docs, notice retained | closed (lesson taken: docs/ methodology; notice in THIRD_PARTY.md (T-1606)) | none: a reference (references/README.md) |
| #134 | [Reference] Azathothas/bit-cli @ cce8131: the TODO work model | closed (lesson taken: the TODO/ work model) | none: a reference (references/README.md) |
| #135 | [Reference] Azathothas/container-research @ 0f155e3: product spec and target measurements | open | none: a reference (references/README.md); open: `AGENTS.md`, section 13, routes the product contract to it |
| #136 | [Reference] Azathothas/memfd-ng @ 5da5803: memfd crate filed under T-0909 | closed (lesson taken in T-0909) | none: a reference (references/README.md) |
| #137 | [Reference] Azathothas/pg-toolkit @ 0da2ec9: build-plan digests and loader routes | closed (lesson taken in T-1349; the loader table is an unfiled candidate) | none: a reference (references/README.md) |
| #138 | [Reference] Azathothas/sandbox-insights @ bcf415c: seven-plane model and open questions | closed (lesson taken in T-0415, T-1308) | none: a reference (references/README.md) |
| #139 | [Reference] Obirvalger/vml @ 688496c: machine-as-a-directory shape | closed (ruled in T-1307) | none: a reference (references/README.md) |
| #140 | [Reference] RuriOSS/ruri @ 711673a: probe-then-refuse discipline | closed (lesson taken in T-0107, T-0804; podcrates pod-hostprobe) | none: a reference (references/README.md) |
| #141 | [Reference] RuriOSS/rurima @ 30a0637: refused: tar extraction as root | closed (refused; lesson taken in T-0301; podcrates pod-extract) | none: a reference (references/README.md) |
| #142 | [Reference] VHSgunzo/memfd-exec @ 9708cb7: memfd fallback and probe-before-use | closed (lesson taken in T-0203, T-0909; not vendored) | none: a reference (references/README.md) |
| #143 | [Reference] VHSgunzo/pathmap @ 98b3d2a: path half of the interposer | closed (lesson taken in T-0701 to T-0705) | none: a reference (references/README.md) |
| #144 | [Reference] VHSgunzo/runimage @ f1512f2: filed: fails where bubblewrap fails | closed (filed: no use; decision in TODO/reference-map.md) | none: a reference (references/README.md) |
| #145 | [Reference] VHSgunzo/sharun @ b1ef744: filed: relocation, not virtualization | closed (filed: no use; decision in TODO/reference-map.md) | none: a reference (references/README.md) |
| #146 | [Reference] VHSgunzo/ulexec @ 00934f8: two exec rungs driven from one tool | closed (lesson taken in T-0905, T-0909) | none: a reference (references/README.md) |
| #147 | [Reference] VHSgunzo/userland-execve @ none: no such repository; 404 evidence | closed (no repository; 404 evidence in TODO/reference-map.md) | none: a reference (references/README.md) |
| #148 | [Reference] apptainer/apptainer @ 6099bb1: confirms wall 1 for a Go runtime | closed (lesson taken in T-0706; podcrates pod-enter, pod-extract) | none: a reference (references/README.md) |
| #149 | [Reference] carlbomsdata/winquick @ 095dd47: doctor command shape | closed (its lesson is taken: T-1112 done) | none: a reference (references/README.md); open: the Source of T-1112 (partial) |
| #150 | [Reference] compforge/pathshim @ 8bcc34e: probe three-channel contract | closed (lesson taken in T-0101, T-0108, T-0110) | none: a reference (references/README.md) |
| #151 | [Reference] containers/bubblewrap @ 26bb788: clone-vs-mount refusal witness | closed (lesson taken in T-0102; podcrates pod-hostprobe) | none: a reference (references/README.md) |
| #152 | [Reference] containers/podman @ 7d39ce8: where the layer applier is reached | closed (lesson taken in T-0205) | none: a reference (references/README.md) |
| #153 | [Reference] containers/storage @ 83cf574: wall 1 at the line, and its option | closed (lesson taken in T-0205, T-0302; podcrates pod-extract) | none: a reference (references/README.md) |
| #154 | [Reference] cubic-vm/cubic @ 4f21a70: verb set and an argument-splitting defect | closed (lesson taken in T-1301, T-1302, T-1307) | none: a reference (references/README.md) |
| #155 | [Reference] dex4er/fakechroot @ b42d1fb: path model and exclude list | closed (lesson taken in T-0707) | none: a reference (references/README.md) |
| #156 | [Reference] dtolnay/faketty @ ce3e201: pty test shape for T-1402 | closed (lesson taken in T-1402) | none: a reference (references/README.md) |
| #157 | [Reference] fritzw/ld-preload-open @ 422b2bf: LD_PRELOAD rulings and musl recipe | closed (lesson taken in T-0701, T-0702, T-1002) | none: a reference (references/README.md) |
| #158 | [Reference] garywill/treesandbox @ 71acbee: refused: dies at getpwuid(0) | closed (refused; lesson taken in T-0404) | none: a reference (references/README.md) |
| #159 | [Reference] gevico/tcg-rs @ 88c020b: filed: pure-Rust TCG engine | closed (filed; ruled in T-1307) | none: a reference (references/README.md) |
| #160 | [Reference] hust-open-atom-club/Vex @ ebee4c7: Docker-like verbs over QEMU configs | closed (ruled in T-1307) | none: a reference (references/README.md) |
| #161 | [Reference] indigo-dc/udocker @ 638bc42: ownership-neutral extraction and modes | closed (lesson taken in T-0301 to T-0307, T-0404; podcrates pod-extract) | none: a reference (references/README.md) |
| #162 | [Reference] io12/userland-execve-rust @ 02ef0e0: userland exec loader, retained unlinked | closed (its lesson is taken: T-1652 and T-1684 done) | none: a reference (references/README.md); open: T-1652 and T-1684 need the vendored upstream |
| #163 | [Reference] mhx/dwarfs @ 9062b57: short write -20 is a libarchive status | closed (lesson taken in T-0203; podcrates pod-hostprobe) | none: a reference (references/README.md) |
| #164 | [Reference] multikernel/sandlock @ 841265d: anti-pattern: filtered read, false report | closed (lesson taken in T-0107, T-0606, T-0804, T-0903; podcrates pod-hostprobe) | none: a reference (references/README.md) |
| #165 | [Reference] novafacing/memfd-exec @ 0a15efe: upstream baseline for the memfd fork | closed (lesson taken in T-0909; not vendored) | none: a reference (references/README.md) |
| #166 | [Scope] Cage recipes: podvm profiles in strict TOML with extends and -s overrides | closed | T-1654 |
| #167 | [Scope] Container cages: docker/podman run parity for the sandbox hardening flags | closed | T-1655 |
| #168 | [Scope] MicroVMs: fast disposable guests from an OCI image, on KVM or TCG | closed | T-1656 |
| #169 | [Scope] User-space kernels (gVisor): a syscall-interpreting tier without user namespaces | closed | T-1657, T-1729 |
| #170 | [Scope] Wrapper sandboxes (bwrap, landrun, bailey): unprivileged Landlock and seccomp confinement | closed | T-1658 |
| #171 | [Reference] pkgforge-dev/cross-libc-dlopen @ 34482c7: cross-libc loading rules | closed (lesson taken in T-0701, T-0702, T-0706) | none: a reference (references/README.md) |
| #172 | [Scope] Provider shapes and bare userlands: Lambda, Cloudflare, Vercel, Nimbus, busybox, distroless profiles | closed | T-1659 |
| #173 | [Reference] proot-me/proot @ 65f3f7d: refused: ptrace is filtered | closed (refused; lesson taken in T-0706) | none: a reference (references/README.md) |
| #174 | [Scope] Notebooks: Jupyter and marimo runs, and a session that wipes and keeps one directory | closed | T-1660 |
| #175 | [Reference] QaidVoid/onelf @ 158b4af: launch ladder, exec lock, symlink check | closed (lesson taken in T-0202, T-0304, T-0305, T-0803, T-1001; podcrates pod-enter, pod-extract) | none: a reference (references/README.md) |
| #176 | [Scope] Language runtimes: Node, Deno and Bun permission models, wasmtime and workerd | closed | T-1661 |
| #177 | [Reference] qemu-rs/qemu-rs @ 4854135: refused: GPL plugin bindings | closed (refused (GPL); ruled in T-1307) | none: a reference (references/README.md) |
| #178 | [Scope] Network labs: NAT, IPv6, policy and impairment labs inside a guest | open | T-1662 |
| #179 | [Reference] clint/fakeroot (salsa.debian.org) @ 860de25: ownership model and the errno test | closed (lesson taken in T-0704, T-0710, T-0711) | none: a reference (references/README.md) |
| #180 | [Scope] Tunnels and live uplinks: WireGuard kill switch, WARP uplink, proxy-forced programs | open | T-1663 |
| #181 | [Reference] sigoden/fakepty @ a265016: pty trap catalogue for T-1402 | closed (lesson taken in T-1402) | none: a reference (references/README.md) |
| #182 | [Scope] Observability: packet capture, counters and connection logs without CAP_NET_RAW | open | T-1664 |
| #183 | [Reference] talaria0101/dropssh @ 0aafa21: concurrent relay and SSH server | closed (lesson taken by PodUtils/podssh) | none: a reference (references/README.md) |
| #184 | [Scope] Egress broker: a policy proxy and the guest's view of a loopback-only proxy | closed | T-1665 |
| #185 | [Reference] talaria0101/nix-experiment @ d8835f2: litmus payload and version forensics | closed (lesson taken in T-0414, T-1111) | none: a reference (references/README.md) |
| #186 | [Scope] Security probe and compare: an inside-the-payload report diffed against a reference | closed | T-1666 |
| #187 | [Reference] talaria0101/sandbox-insights @ 0889f5f: probe discipline and the four walls | closed (lesson taken in T-0415, T-0712, T-1308) | none: a reference (references/README.md) |
| #188 | [Scope] Doctor and installers: host readiness, verified downloads, disk guard | open | T-1667 |
| #189 | [Scope] Findings ledger: tool defects and host traits that change how a result reads | closed | T-1668 |
| #190 | [Reference] talaria0101/sandssh @ 4fc7f8c: one-pair relay design | closed (lesson taken by PodUtils/podssh) | none: a reference (references/README.md) |
| #191 | [Reference] talaria0101/vm-research @ 7697b9b: machine tier and TCG benchmarks | closed (lesson taken in T-1301 to T-1306, T-1308) | none: a reference (references/README.md) |
| #192 | [Reference] ylang-ylang/dockless @ ed35b5d: fail-fast CLI guardrails | closed (lesson taken in T-0801, T-0804) | none: a reference (references/README.md) |
| #193 | [Scope] podvm does what sand-net does, without root: index | open | T-1654 to T-1668 |
| #194 | COORD-001: the crates podvm hands over and takes, and the access of podcrates@T-040 (to podcrates) | closed (each take is done: T-1675 to T-1681; podvm takes each `pod-*` crate at v0.39.0; `pod-netlab` stays with COORD-041, #260) | T-1675 to T-1681 |
| #195 | COORD-002: watch-org.sh sees no repository but the last on Windows: CRLF in the names (to podcrates) | closed (podvm took the pin 54427f6) | none: a COORD issue (to podcrates); podcrates fixed it in `54427f6`, and podvm closes it when it takes that pin |
| #196 | COORD-003: podvm's inventory of the hosts and services of other parties, for pod-services (to podcrates) | closed (podcrates podcrates@T-054 and podcrates@T-066 done; the data of podvm is in pod-services from v0.5.0) | T-1679; the take stays under #194 |
| #197 | COORD-004: answers to Q2 and Q3 of the M6 import plan, and the pod-term take (to podcrates) | closed (pod-term taken 75ed306) | T-1675, T-1681 |
| #198 | COORD-005: GitHub Actions of podcrates: the owner's account dispatches ci.yml and release.yml (to podcrates) | closed | none: coordination; the ruling is in docs/decisions.md |
| #199 | [gh action] requested | closed (gate run 42177003497, secrets run 42177007581) | none: a request of a workflow run (AGENTS.md, section 9) |
| #201 | [gh action] requested | closed (gate run 42177673037) | none: a request of a workflow run for T-1604 (AGENTS.md, section 9) |
| #202 | [gh action] requested | closed (gate run 42179344015, green) | none: a request of a workflow run for T-1604 (AGENTS.md, section 9) |
| #203 | COORD-007: podvm took pod-tls 0.4.0 with no tokio, and pod-term moves to v0.4.0 (to podcrates) | closed (podcrates#7 closed) | T-1676 |
| #204 | [gh action] requested | closed (gate run 42209748294, green) | none: a request of a workflow run for T-1674 and T-1676 (AGENTS.md, section 9) |
| #205 | COORD-008: podvm took pod-proxy 0.4.0 (feature blocking) for the relay legs (to podcrates) | closed (podcrates#9 closed) | T-1677 |
| #206 | [gh action] requested | closed (gate run 42214046548, green) | none: a request of a workflow run for T-1674 and T-1677 (AGENTS.md, section 9) |
| #207 | COORD-009: podvm moved to podcrates v0.5.0; the registry patch is yours (to podcrates) | closed (registry patch taken 06f2427) | T-1677 |
| #208 | COORD-010: podvm took the registry patch; ureq leaves, and C10 takes pod-proxy's runtime (to podcrates) | closed (podcrates#13 closed) | T-1677 |
| #209 | [gh action] requested | closed (gate run 42227206423, green) | none: a request of a workflow run for T-1677 and T-1686 (AGENTS.md, section 9) |
| #210 | COORD-011: podvm moved to v0.6.0 and took pod-hostprobe; the freeze of the 29 files ends (to podcrates) | closed (podcrates#11 and #16 closed) | T-1681 |
| #211 | [gh action] requested | closed (gate run 42265769964, green) | none: a request of a workflow run for T-1681 (AGENTS.md, section 9) |
| #200 | COORD-006: the Alpine netboot pins: podvm fetches the 3.22.6 archive (to podcrates) | closed | T-1687 |
| #212 | doctor: no row answers whether a memfd can be executed (vm.memfd_noexec), and the memfd rung refuses late | closed | T-1688 |
| #213 | probe: the non-goal `remedy` field contradicts the `stance` in the same JSON object | closed | T-1689 |
| #214 | doctor: the disk-free row reads the hard `RLIMIT_FSIZE` while every other consumer reads the soft one | closed | T-1690 |
| #215 | supervise: `stop -t 0` reports a container as stopped while it is still running, because a bound-setting failure reads as the launcher having gone | closed | T-1691 |
| #216 | windows, parity: the `--podvm-mem` row claims an `RLIMIT_FSIZE` check that neither flavor of the verb performs | closed | T-1692 |
| #217 | release: README and SECURITY put the podbox identity boundary one tag early, and the verifier disagrees with both documents | closed | T-1693 |
| #218 | COORD-012: podvm took v0.7.0, and filed the asks of COORD-015 (to podcrates) | closed | T-1681; none: coordination |
| #219 | COORD-013: podvm took v0.8.0 and pod-supervise (to podcrates) | closed | T-1681 |
| #220 | COORD-014: podvm took v0.9.0, pod-guest and pod-enter (to podcrates) | closed | T-1681 |
| #221 | COORD-015: the whiteout escape is fixed in podvm-extract (T-1698) (to podcrates) | closed (pod-extract carries the tests, podcrates podcrates@T-141; taken 0682c0c) | T-1698 |
| #222 | COORD-016: pod-enter: classify and stage a payload over 128 MiB (T-1647) (to podcrates) | closed | T-1647 |
| #223 | COORD-017: podvm took v0.10.0 and pod-oci (to podcrates) | closed | T-1681 |
| #224 | COORD-018: pod-enter and pod-hostprobe: volume pairs, workdir and first program through the table (T-1653) (to podcrates) | closed | T-1653 |
| #225 | COORD-019: podvm took v0.11.0, pod-extract and pod-rootfs (to podcrates) | closed | T-1681, T-1647 |
| #226 | COORD-020: podvm took v0.12.0, the Maps API of pod-enter (to podcrates) | closed (the asks are podcrates podcrates@T-155) | T-1653 |
| #227 | COORD-021: podrunner: the design review of #47, what the crates must give before podrunner starts (to podcrates) | closed (podcrates#51: podcrates@T-160 to podcrates@T-165) | none: the hand-off of the podrunner plan to podcrates ([decisions](../docs/decisions.md), 2026-11-10, "podrunner") |
| #228 | COORD-022: podrunner: podnet pod with the official runner, and the pod-hold hand-off (to podnet) | closed (podnet#422) | none: the asks of the podrunner plan to podnet ([decisions](../docs/decisions.md), 2026-11-10, "podrunner") |
| #229 | [podrunner] interpose: realpath of /proc/self/exe names the image's loader, while readlink names the program, so the .NET runner exits 154 | closed | T-1704 |
| #230 | [podrunner] interpose: a host path that a loaded object reports gets the rootfs prefix a second time, so CoreCLR exits 137 | closed | T-1705 |
| #231 | [podrunner] the userland rung runs the unmodified actions/runner: it exits 154, then 137, and it dies on SIGXFSZ under a finite RLIMIT_FSIZE | closed | T-1706; T-1655 (amended) |
| #232 | [podrunner] enter: a relative first program resolves against the image root, not against -w, so -w /opt/runner ./run.sh exits 127 | closed | T-1707 |
| #233 | [podrunner] supervise: stop and rm -f signal the payload only, so orphaned grandchildren keep running after the container is gone | closed | T-1708; T-1655 (amended) |
| #234 | [podrunner] image: all containers of one image share one mutable rootfs, so a write in one container is read by another and --rm is not a fresh start | closed | T-1709 |
| #235 | [podrunner] interpose: a Unix socket path in connect and bind passes the path table, so bind creates the socket on the host and a -v pair does not apply | closed | T-1710 |
| #236 | [podrunner] cli: drive the podvm calls of the four container-hook commands on the userland rung, with the runner's mount table | closed | T-1711; T-1653 and T-1655 (amended) |
| #237 | [podrunner] cli: --podvm-tier=chroot does nothing; it runs the ladder, and on a chroot-denied host it enters the userland rung with exit 0 | closed | T-1712 |
| #238 | [podrunner] system: .EnteredRung reads chroot where the probe selects supervise and chroot is denied, while run enters the userland rung | closed | T-1713 |
| #239 | [podrunner] probe: podvm probe --json validates against the published schema of pod-hostprobe (blocked on podcrates podcrates@T-107) | closed (T-1714 is done) | T-1714; T-1670 (amended) |
| #240 | [podrunner] supervise: the container table keeps -e values in a file that the umask leaves readable to other users | closed | T-1715; T-1696 (amended, then dropped 2026-11-11) |
| #241 | COORD-023: podvm resumes; fuzz targets for pod-oci, pod-extract and pod-enter; T-1696 is dropped (to podcrates) | closed (podcrates#56: podcrates@T-173) | T-1684 (amended), T-1696 |
| #242 | COORD-024: podvm took v0.15.0 and the pod-enter change of podcrates@T-155 (to podcrates) | closed (podcrates#52 and #54) | T-1716 |
| #243 | COORD-025: the answer to COORD-031: T-1656 moves up for podrunner; incus is a study (T-1717), then an API back end (T-1718) (to podcrates) | closed (podcrates#56) | T-1656 (amended), T-1717, T-1718 |
| #244 | COORD-026: pod-supervise: private modes for the table, the logs and each container directory (T-1715); a tag with podcrates@T-162 for T-1708 (to podcrates) | closed (podcrates v0.16.0, taken 894cf13) | T-1708, T-1715 |
| #245 | COORD-027: pod-enter: resource limits and no_new_privs in the child before the exec (T-1655) (to podcrates) | closed (podcrates v0.17.0, taken 522b437) | T-1655 (amended) |
| #246 | COORD-028: the answer to PodUtils/podrunner#49: wait reads the record again until it has the exit code (to podrunner) | closed (podrunner#49; in v0.2.0-beta.3) | T-1720 |
| #247 | COORD-029: pod-enter: lib_dirs takes /usr/local/lib and the image's loader configuration (T-1719) (to podcrates) | closed | T-1719 |
| #248 | COORD-030: the official runner starts on the userland rung; the shape of M1's stand-in for run.sh (T-1706) (to podrunner) | closed | T-1706 |
| #249 | COORD-031: podvm takes podcrates v0.19.0 (to podcrates) | closed (podcrates#62) | T-1719 |
| #250 | COORD-032: the answer to PodUtils/podrunner#51: a unix socket path takes the table on main (T-1710) (to podrunner) | closed (podrunner#51) | T-1710 |
| #251 | [reference] Azathothas/errand-box: incus without root (route A) and its costs | closed (lesson taken in T-1717 and T-1718) | T-1717, T-1718 (open while T-1718 is open) |
| #252 | COORD-033: the answer to PodUtils/podrunner#52: no aarch64 interposer yet; podvm T-1721 (to podrunner) | closed | T-1721 |
| #253 | COORD-034: a client of the incus REST API as a podcrates crate (T-1718) (to podcrates) | closed (pod-incus, taken 1ffe87e) | T-1718 |
| #254 | COORD-035: the answer to PodUtils/podrunner#53: podvm v0.2.0-beta.3 is released (to podrunner) | closed (podrunner#53) | T-1706, T-1720, T-1710 |
| #255 | COORD-036: the answer to PodUtils/podrunner#61: an absolute symlink of the rootfs resolves on the host; podvm T-1725 fixes it (to podrunner) | closed | T-1725 |
| #256 | COORD-037: the answer to PodUtils/podrunner#62: exec ignores a :ro volume; podvm T-1726 fixes it (to podrunner) | closed | T-1726 |
| #257 | COORD-038: podvm takes podcrates v0.21.0 and pod-incus (the answer to PodUtils/podcrates#63) (to podcrates) | closed (podcrates#63 and #65) | T-1727 |
| #258 | COORD-039: each container has its own rootfs (T-1709, issue 234) (to podrunner) | closed | T-1709 |
| #259 | COORD-040: ask for pod-confine and a confine step of pod-enter's child (podvm T-1658, issue 170) (to podcrates) | closed | T-1658 |
| #260 | COORD-041: ask for pod-netlab, and who reads the guest's modloop (podvm T-1662, issues 178, 180, 182) (to podcrates) | open | T-1662 |
| #261 | COORD-042: pod-extract refuses a layer whose root directory entry has an empty name (public.ecr.aws/lambda/provided:al2023) (to podcrates) | closed | T-1659 |
| #262 | COORD-043: podvm takes podcrates v0.22.0; a detached payload ends with its launcher (the answer to PodUtils/podcrates#71) (to all) | closed (podcrates#71) | T-1732 |
| #263 | COORD-044: three gaps of podvm's inventory in pod-services: gcr.io, mirror.gcr.io and the NIC modules of the guest (podvm T-1679) (to podcrates) | closed | T-1679 |
| #264 | COORD-045: podvm takes podcrates v0.24.0; pod-services is taken (the answer to PodUtils/podcrates#69 and #70) (to podcrates) | closed | T-1658, T-1659, T-1679 |
| #265 | COORD-046: podvm v0.2.0-beta.4 is released: a tree for each container, absolute links, exec under :ro (the answer to PodUtils/podrunner#62) (to podrunner) | closed | T-1709, T-1725, T-1726, T-1732 |
| #266 | COORD-047: podvm takes podcrates v0.25.0 (the answer to PodUtils/podcrates#76) (to podcrates) | closed | T-1658 |
| #267 | COORD-048: Plan.confine and the /tmp that the namespace rung mounts in the child: the rule holds the directory under the mount (to podcrates) | closed | T-1658 |
| #268 | COORD-049: real labels and one JSON report for each run (A-V-07, A-V-09 and G-06 of PodUtils/podcrates#39) (to all) | closed | T-1695 |
| #269 | COORD-050: mcr.microsoft.com for pod-services (the image of podvm's profile playwright-chromium) (to podcrates) | closed | T-1694 |
| #270 | COORD-051: podvm takes podcrates v0.26.0 (the answer to PodUtils/podcrates#77) (to podcrates) | closed | T-1658, T-1694 |
| #271 | cfb asks: tracker for the 9 asks of cfb owned by podvm (from podcrates#35) | open | tracker: T-1658, T-1665, T-1663, T-1694, T-1695, T-1714, T-1723, T-1739 |
| #272 | CI plan items: tracker for the 3 items owned by podvm (from podcrates#47) | closed | tracker: T-1656, T-1723 (P3-3); #271 (P1-1) |
| #273 | COORD-052: podvm v0.2.0-beta.5 is released: confine for run -d, exec and the loader family; labels and run --report; headless Chromium (to all) | closed | T-1658, T-1694, T-1695, T-1711, T-1738 |
| #274 | COORD-053: registry.npmjs.org (workerd) for pod-services (to podcrates) | closed | T-1661 |
| #275 | COORD-054: the wasmtime release for pod-services (to podcrates) | closed | T-1661 |
| #276 | COORD-055: podvm takes podcrates v0.27.0 (the answer to PodUtils/podcrates#85) (to podcrates) | closed | T-1658 |
| #277 | COORD-056: pod-guest commit_argv: qemu-img commit takes no OUTFILE; podvm windows setup fails under KVM (to podcrates) | closed | T-1641 |
| #278 | COORD-057: podvm takes podcrates v0.28.0 (the answer to PodUtils/podcrates#86) (to podcrates) | closed | T-1641, T-1661 |
| #279 | COORD-058: podvm takes podcrates v0.29.0; the confine help; aarch64 interposer in progress (to all) | closed | T-1658, T-1721 |
| #280 | COORD-059: podvm takes podcrates v0.30.0 (the answer to PodUtils/podcrates#89) (to podcrates) | closed | T-1680 |
| #281 | COORD-060: pod-extract refuses a correct plain-tar layer: the hash stops at the end-of-archive marker (to podcrates) | closed | T-1741 |
| #282 | COORD-061: pod-enter libc_beside does not find the aarch64 glibc of a Debian image (the search is depth-first from the end) (to podcrates) | closed | T-1742 |
| #283 | COORD-062: the hand-off of pod-relay for podvm (blocking facade, reverse-v1) (to podcrates) | closed | T-1678 |
| #284 | COORD-063: podvm takes podcrates v0.31.0 (the answer to PodUtils/podcrates#90) (to podcrates) | closed | T-1741, T-1742 |
| #285 | COORD-064: podvm takes podcrates v0.32.0 and the trial of pod-relay (the answer to PodUtils/podcrates#93) (to podcrates) | closed | T-1678 |
| #286 | COORD-065: podvm v0.2.0-beta.6 carries the aarch64 interposer (for PodUtils/podrunner#52) (to podrunner) | closed | T-1721 |
| #287 | [gh action] requested | closed (gate 43345851281, secrets 43345855762, arm 43347185943, trial 43347752049, release 43348676353, all green) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.7` on `2edace91` (AGENTS.md, section 9) |
| #288 | COORD-066: podvm takes podcrates v0.34.0 (the answer to PodUtils/podcrates#103 and #104) (to podcrates) | closed | T-1662 |
| #289 | COORD-067: GNU tar with an absolute -C works where chroot is denied: the fortified opens take the path table (the answer to PodUtils/podcrates#102 and PodUtils/podrunner#70) (to podcrates) (to podrunner) | closed | T-1743 |
| #290 | COORD-068: podvm's answer to the asks V-1 to V-8 of podswarm (PodUtils/podcrates#124): entries T-1745 to T-1752 (to podcrates) | closed (podcrates read it on PodUtils/podcrates#124, 2026-11-13) | T-1745, T-1746, T-1747, T-1748, T-1749, T-1750, T-1751, T-1752 |
| #291 | COORD-069: ask for a reader of the probe report in pod-hostprobe, so that run can use the probe cache (podvm T-1746, V-2 of podswarm) (to podcrates) | closed (T-1746 is done in be29f5d9 and released in v0.2.0-beta.13) | T-1746 |
| #292 | [gh action] requested | closed (gate 43351811176, secrets 43351815639, arm 43351820031, all green on 845faba1) | none: a request of the gate, secrets and `arm.yml` on the head of `main` of 2026-11-13 (the commit that adds this row), for T-1662, T-1744, T-1748 and podcrates v0.35.0 (AGENTS.md, section 9) |
| #293 | COORD-070: podvm takes podcrates v0.35.0 (the answer to PodUtils/podcrates#127) (to podcrates) | closed | T-1662, T-1746 |
| #294 | COORD-071: ask for an overlay root in pod-enter's namespace rung: the image read-only under a tmpfs upper layer (podvm T-1749, V-5 of podswarm) (to podcrates) | closed (podcrates v0.42.0 carries the overlay root, podcrates@T-486) | T-1749 |
| #295 | COORD-072: podvm takes podcrates v0.36.0 (the answer to PodUtils/podcrates#129) (to podcrates) | closed | T-1746 |
| #296 | COORD-073: ask for a proc mount on the /proc of the image in pod-enter's namespace rung (podvm T-1758) (to podcrates) | closed | T-1758 |
| #297 | COORD-074: ask for an entry of pod-enter that joins the mount namespace of a running payload, for exec (podvm T-1765) (to podcrates) | closed | T-1765 |
| #298 | COORD-075: ask for three fixes of the knob flags of pod-netlab: an empty --udp, the range of a number, the words of a flag (podvm T-1770) (to podcrates) | closed | T-1770 |
| #299 | COORD-076: podvm takes podcrates v0.37.0 (the answer to PodUtils/podcrates#134) (to podcrates) | closed | T-1758, T-1765 |
| #300 | [gh action] requested | closed (gate 43384381442 and secrets 43384387248 green on 7f1b23b6; arm 43386208714 failed: experiment 356 asked for refusals that T-1761 and T-1772 removed) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.8` (AGENTS.md, section 9) |
| #301 | [gh action] requested | closed (secrets 43389382246 green on 2dde7df0; the gate 43389376336 was cancelled at the bound of a job beside a second dispatch of it) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.8` (AGENTS.md, section 9) |
| #302 | [gh action] requested | closed (gate 43394989985, arm 43397131676, trial 43398052471, release 43399714037, all green on 2dde7df0) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.8` (AGENTS.md, section 9) |
| #303 | COORD-077: the owners in a guest of the machine tier, and the guest with no NIC and one host channel (the answer to PodUtils/podrunner#81 and #83) (to podrunner) | closed | T-1776, T-1751 |
| #304 | COORD-078: podvm v0.2.0-beta.8 is released, and podvm takes podcrates v0.38.0 (the answer to PodUtils/podcrates#136) (to all) | closed | T-1760, T-1770 |
| #305 | [gh action] requested | closed (not run to its end: ce4a561c carried a symbolic link of the cloud host, crates/podvm-interpose/target, which the next commit removes) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.9` (AGENTS.md, section 9) |
| #306 | [gh action] requested | closed (gate 43403642262, arm 43406008482, trial 43406658216, release 43409438385, all green on 11008d62) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.9` (AGENTS.md, section 9) |
| #307 | COORD-079: stdin for an incus instance that stays (the answer to PodUtils/podrunner#85) (to podrunner) | closed (podrunner closed its ask) | T-1777 |
| #308 | [gh action] requested | closed (gate 43420276209, arm 43421607391, trial 43422400615, release 43423493082, all green on ce56cc26) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.10` (AGENTS.md, section 9) |
| #309 | COORD-080: podvm takes podcrates v0.39.0 (the answer to PodUtils/podcrates#137) (to podcrates) | closed (podcrates recorded the take `19892c50` on PodUtils/podcrates#137) | T-1751 |
| #310 | [gh action] requested | closed (the gate 43437287427 failed on 11c0d410: the native-static rung refused the device pair /dev/null of T-1774) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.11` (AGENTS.md, section 9) |
| #311 | COORD-081: the channel of a guest and a loopback proxy variable (the answer to PodUtils/podrunner#87) (to podrunner) | closed (podrunner closed its ask) | T-1778 |
| #312 | [gh action] requested | closed (gate 43440607600, arm 43442039218, trial 43442633044, release 43443421795, all green on 3db2126f) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.11` (AGENTS.md, section 9) |
| #315 | [gh action] requested | closed (gate 43468572134, arm 43471435973, trial 43472240628, release 43474394015, all green on c1a8d3e3) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.12` (AGENTS.md, section 9) |
| #316 | COORD-082: --podvm-net none and --podvm-channel on the incus tier (the answer to PodUtils/podrunner#88) (to podrunner) | closed (podrunner closed its ask) | T-1779 |
| #317 | COORD-083: podvm takes podcrates v0.40.0 (the answer to PodUtils/podcrates#140) (to podcrates) | closed (podcrates closed #140) | - |
| #318 | [gh action] requested | closed (gate 43485242857, arm 43486833467, trial 43487574960, release 43490018144, all green on f99be8fd) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.13` (AGENTS.md, section 9) |
| #319 | COORD-084: the channel of the incus tier needs no proxy device (the answer to PodUtils/podrunner#90) (to podrunner) | closed (podrunner closed its ask) | T-1779 |
| #320 | [gh action] requested | closed (gate 43509205484, arm 43510876931, trial 43511436790, release 43512350059, all green on d8443ce8) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.14` (AGENTS.md, section 9) |
| #321 | COORD-085: a container engine in a guest of the machine tier (the answer to PodUtils/podrunner#92) (to podrunner) | closed (podrunner closed its ask) | T-1780 |

#313 and #314 are pull requests of Dependabot. Each judged update lands
on `main` as its own commit, and its pull request then closes (`AGENTS.md`,
section 11): #313 in `54258666`, #314 in `c5f9e315`; both are closed.
| #322 | COORD-086: the order of podcrates@T-486, @T-107 and @T-345 to @T-347, which the open entries of podvm wait on, before its beta testers start (to podcrates) | closed (podcrates answered in #142, and v0.42.0 carries podcrates@T-486 and podcrates@T-107) | T-1749, T-1714, T-1662 |
| #323 | COORD-087: the acceptance test of podcrates@T-486, an overlay root in the namespace rung (the answer to PodUtils/podcrates#142, item 2) (to podcrates) | closed (podcrates v0.42.0 carries the test with podcrates@T-486) | T-1749 |
| #324 | COORD-088: podvm takes podcrates v0.41.0 (the answer to PodUtils/podcrates#143) (to podcrates) | closed (podcrates closed #143) | - |
| #325 | COORD-089: podvm takes podcrates v0.42.0 (the answer to PodUtils/podcrates#145) (to podcrates) | closed (podcrates closed PodUtils/podcrates#145) | - |
| #326 | COORD-090: ask for a two-layer completion in pod-rootfs, so that podvm can use the overlay root of podcrates@T-486 (podvm T-1749) (to podcrates) | closed (podcrates@T-567 in `v0.43.0`, taken in `b0bb87f1`) | T-1749 |
| #327 | COORD-091: the acceptance test of podcrates@T-567, a completion of two layers in pod-rootfs (the answer to PodUtils/podcrates#146) (to podcrates) | closed (podcrates `v0.43.0` carries the test, two_layers of pod-rootfs) | T-1749 |
| #329 | `system df` reports 0 B after `image prune -a` while 8.5 MB of extracted rootfs stays on disk, and no verb reclaims it | closed (T-1787 in `429ca8dc`, released in `0.2.0-beta.15`) | T-1787 |
| #330 | A foreground run is recorded as `Created` while its payload runs, so `ps` hides it, `system df` counts 0 running, `kill` and `wait` refuse it, and `stop` exits 0 having stopped nothing | closed (T-1786 done: the forced ladder holds the record) | T-1786 |
| #332 | The `--user` identity notice repeats once per process, where T-0711 decides the run says it once | closed (T-1785 in `a96c3a5a`, released in `0.2.0-beta.15`) | T-1785 |
| #333 | The honest setuid refusal says the payload runs as uid 0; on this host it runs as uid 966 | closed (T-1784 in `a96c3a5a`, released in `0.2.0-beta.15`) | T-1784 |
| #334 | Nine verbs print their usage to stdout when an argument is missing, so `podvm save > image.tar` writes a file that is not a tar | closed (T-1783 in `a96c3a5a`, released in `0.2.0-beta.15`) | T-1783 |
| #336 | `podvm load` spools stdin to `$TMPDIR/podvm-load-in-<pid>`, so a planted symlink truncates a file the user never named | closed (T-1782 in `a96c3a5a`, released in `0.2.0-beta.15`) | T-1782 |
| #328 | [gh action] requested | closed (gate 43556212876 green on 772734fa; arm 43557049704 failed: experiment 356 wrote into the image tree under a spare tree) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.15` (AGENTS.md, section 9) |
| #339 | COORD-094: ask for a foreground holder in pod-supervise, so that a foreground run is Running while its payload runs (PodUtils/podvm#330) (to podcrates) | closed (podcrates v0.44.0, commit 14e972d2) | T-1786 |
| #338 | [gh action] requested | closed (the gate 43557980918 failed on 08abecca: rustfmt of crates/podvm-interpose) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.15` (AGENTS.md, section 9) |
| #340 | [gh action] requested | closed (gate 43559084201, arm 43559993532, nightly trial 43560303295 and release 43560802211 green on 551ba939) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.15` (AGENTS.md, section 9) |
| #341 | COORD-095: pod-rootfs writes the host CA bundle into an image with no bundle when host_cas is false (podvm T-1818) (to podcrates) | closed (podcrates v0.44.0, commit 14e972d2) | T-1818 |
| #342 | COORD-096: pod-extract drops S_ISUID and S_ISGID of a 4755 or 2755 file (podvm T-1813) (to podcrates) | closed (podcrates v0.44.0, commit 14e972d2) | T-1813 |
| #343 | COORD-097: pod-supervise keeps the end fields on start and the pid of a Dead record (podvm T-1807) (to podcrates) | closed (podcrates v0.44.0, commit 14e972d2) | T-1807 |
| #344 | COORD-098: an exec= rule of pod-confine and pod-enter does not deny a program started through the image's loader (podvm T-1819) (to podcrates) | closed (podcrates `v0.45.0`, commit 4a02d45e; the table of the interposer is T-1892) | T-1819 |
| #345 | COORD-099: pod-enter applies two limits of one resource in order, so a raise after a lower fails with EPERM (podvm T-1827) (to podcrates) | closed (T-1827, commit 4a02d45e) | T-1827 |
| #346 | COORD-100: pod-oci takes an insecure-registry host with a space or "!", and its platform list repeats entries and shows attestations (podvm T-1842, T-1836) (to podcrates) | closed (T-1842, commit 4a02d45e) | T-1842, T-1836 |
| #347 | COORD-101: pod-netlab knob errors lack the value and the unit, and its surface differs from sand-net (podvm T-1837, T-1847) (to podcrates) | closed (podcrates `v0.45.0`, podcrates@T-586 partial there; podvm keeps T-1837 and T-1847) | T-1837, T-1847 |
| #348 | COORD-102: podvm takes podcrates v0.43.0 (the answer to PodUtils/podcrates#149) (to podcrates) | closed (podcrates closed PodUtils/podcrates#149) | - |
| #349 | [gh action] requested | closed (gate 43569997386, arm 43570892970, nightly trial 43571247092 and release 43571818694 green on 53c48d34) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.16` (AGENTS.md, section 9) |
| #350 | COORD-103: seven asks for the Windows guest: fetch by range, pod-udf, the agent, plan options, pod-qga, provision, warm start (podvm T-1862 to T-1874) (to podcrates) | closed (podcrates `v0.45.0`; T-1862, T-1865, T-1866; the warm start is T-1874) | T-1862, T-1863, T-1864, T-1865, T-1868, T-1873, T-1874 |
| #351 | COORD-104: the answer to PodUtils/podrunner#100: a glibc 2.34 image with no libdl.so.2 on the loader family (podvm T-1880) (to podrunner) | closed (T-1880 in `0.2.0-beta.17`) | T-1880 |
| #352 | COORD-106 of Nemo-010: pod-extract counts the `./` archive root as an entry it could not materialise (to podcrates) | closed (T-1882, commit 4a02d45e) | T-1882 |
| #353 | create and run do not report the entries the extraction could not materialise | closed | T-1883 |
| #354 | COORD-107 of Nemo-010: pod-extract applies the mode of the image to files but not to directories, and its widening pass can chmod a symlink target outside the tree (to podcrates) | closed (T-1881, commit 4a02d45e) | T-1881 |
| #355 | [gh action] requested | closed (gate 43576720202, arm 43577573447, nightly trial 43578086980 and release 43580765158 green on 0f13267c) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.17` (AGENTS.md, section 9) |
| #356 | `exec` exits 125 for an image name that resolves to nothing, where the exit table and T-1805's own decision say 1 | closed (`0955fcab`, re-measured by the reporter) | T-1842, T-1805 |
| #357 | `rm` leaves a detached spare-tree fill holding the image's payload lock, so an `rmi` or `image prune` in the next second is refused as "in use by a running container" while `ps -a` is empty | closed (`0814cab0`) | T-1781 |
| #358 | Concurrent `create`, `run`, `cp` or `extract` of a never-extracted image all write the same tree, and the first to fail removes it under the others | closed (`2003aff9`) | T-1886 |
| #359 | COORD-108: pod-broker: a 502 names no upstream status, and a no-rule 403 does not name the port (podvm T-1820) (to podcrates) | closed (podcrates `v0.45.0`; the test of podvm is T-1820) | T-1820 |
| #360 | COORD-109: pod-guest fat16: a long file name, for the autounattend.xml of an unattended Windows install (podvm T-1866) (to podcrates) | closed (podcrates `v0.45.0`; the media of podvm is T-1866) | T-1866, T-1867 |
| #361 | COORD-110: pod-services: list software-download.microsoft.com (to podcrates) | closed (podcrates `v0.45.0`; WAITING is empty) | T-1679 |
| #362 | COORD-111: podvm took v0.44.0 in 14e972d2 (the answer to PodUtils/podcrates#154) (to podcrates) | closed (podcrates closed its issue 154) | T-1786, T-1881 |
| #363 | [gh action] requested | closed (gate 43598389668 failed: plant case 27d did not restore `dos.rs`; fixed, a new request follows) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.18` (AGENTS.md, section 9) |
| #364 | `--podvm-confine` on the no-chroot rung resolves a rule path against the host root, so an image's absolute symlink is refused as missing | closed (T-1887 and T-1888, commit 019e7dcd) | T-1887, T-1888 |
| #365 | [gh action] requested | closed (gate 43603709240 failed: the check of T-1859 found nine stores of `podvm-extract` left by a user who is not root; fixed, a new request follows) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.18` at `b09df48b` (AGENTS.md, section 9) |
| #366 | COORD-112: the answer to PodUtils/podrunner#102: no guest of another architecture yet (to podrunner) | closed (PodUtils/podrunner#102 closed) | T-1889, T-1890 |
| #367 | COORD-113: pod-extract remove_tree cannot remove a file in a directory of mode 0555 as a user who is not root (podvm T-1891) (to podcrates) | closed (T-1891, commit 4a02d45e) | T-1891 |
| #368 | [gh action] requested | closed (gate 43605290339, arm 43606602497, nightly trial 43607102100 and release 43607691009 green on aaea18eb) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.18` (AGENTS.md, section 9) |
| #369 | COORD-114: podvm took v0.45.0 in 4a02d45e (the answer to PodUtils/podcrates#157) (to podcrates) | closed (superseded by #374) | T-1827, T-1842, T-1881, T-1882, T-1891, T-1892 |
| #370 | COORD-115: pod-guest provision leaves the emulator running when the typing into the console fails (podvm T-1893) (to podcrates) | closed (v0.48.0 taken in 47357595) | T-1893 |
| #371 | [gh action] requested | closed (kvm 43619241341 failed: the readiness of podcrates `v0.45.0`, T-1894) | T-1864, T-1867, T-1894 |
| #372 | COORD-116: pod-guest v0.45.0 provision never types into a Windows guest: the screen-settle readiness fails under KVM and TCG (podvm T-1894) (to podcrates) | closed (v0.48.0 taken in 47357595) | T-1894 |
| #373 | [gh action] requested | closed (kvm 43657522107 failed: 412 read the machine notice that T-1824 made quiet; 450 looked for the wrong output path, and `windows setup` passed under KVM in 37 s) | T-1864, T-1867, T-1894 |
| #374 | COORD-117: podvm took v0.48.0 in 47357595 (the answer to PodUtils/podcrates#158, #159, #160, #161, #162) (to podcrates) | closed (PodUtils/podcrates#158 to #162 closed) | T-1893, T-1894 |
| #375 | COORD-118: the answer to PodUtils/podrunner#104: save by an index digest after a pull by tag (podvm T-1895) (to podrunner) | closed (`0.2.0-beta.19`) | T-1895 |
| #376 | COORD-119: the answer to PodUtils/podrunner#100, case 2: a detached run of a static image where chroot is denied (podvm T-1896) (to podrunner) | closed (`0.2.0-beta.19`) | T-1896 |
| #377 | [gh action] requested | closed (kvm 43660534832: 390, 412 and 440 pass; `windows setup` passed in 37 s; 450 gave 42 once and a timeout twice, issue 380) | T-1864, T-1867, T-1894 |
| #378 | [gh action] requested | closed (gate 43660867122 failed: `tests/cp_tree.rs` created with `/bin/true`, which the image lacks where chroot is denied; fixed in 88019eed) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.19` (AGENTS.md, section 9) |
| #379 | [gh action] requested | closed (gate 43664227298: each test passed; two tests left a directory as a user who is not root, fixed in the next commit) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.19` (AGENTS.md, section 9) |
| #380 | COORD-120: podvm took v0.49.0 in 7d30dd75; a Validation OS run under KVM times out at 600 s on one boot and passes on the next (podvm T-1864) (to podcrates) | closed (podcrates `v0.51.0` took it, PodUtils/podcrates#172 closed) | T-1864 |
| #381 | [gh action] requested | closed (gate 43666264498, arm 43667556798, nightly trial 43667929703 and release 43668566087 green on f5a757c6: `0.2.0-beta.19`) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.19` (AGENTS.md, section 9) |
| #382 | COORD-121: the answer to PodUtils/podrunner#110: limits of memory, CPU and processes (podvm T-1897, T-1747) (to podrunner) | closed (released in `v0.2.0-beta.20`) | T-1897, T-1747 |
| #383 | [gh action] requested | closed (kvm 43672193303: 390, 412, 440, 392 and 450 pass under KVM; the new step of 443 failed on two defects of the experiment, fixed after) | T-1852, T-1864, T-1894 |
| #384 | COORD-122: run_ladder of pod-enter gives the caller no pid before its wait (podvm T-1786) (to podcrates) | closed (answered by `pod_enter::spawn_ladder` of `v0.63.0`) | T-1786 |
| #385 | [gh action] requested | closed (arm 43675339957 failed on a stale clause of experiment 356, corrected in 35d5b686) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.20` (AGENTS.md, section 9) |
| #386 | COORD-123: a FAT image writer with directories and a size from the content, for `-v` of a Windows guest (podvm T-1872) (to podcrates) | open | T-1872 |
| #387 | [gh action] requested | closed (gate 43676505508, arm 43677392885, trial 43677661305, release 43678160011: `v0.2.0-beta.20`) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.20` at 35d5b686 (AGENTS.md, section 9) |
| #388 | [gh action] requested | closed (run 43676508681) | T-1867: a request of `kvm.yml` with the job `install` (experiment 452) |
| #389 | COORD-124: pod-services: list the host api.ghcr.pkgforge.dev (podvm T-1875) (to podcrates) | closed (podcrates v0.51.0) | T-1875 |
| #390 | COORD-125: pod-hostprobe: report a delegated cgroup2 subtree (podvm T-1747) (to podcrates) | closed (v0.51.0; T-1747 wires it) | T-1747 |
| #391 | COORD-126: the answer to PodUtils/podrunner#111: a :ro volume on the detached run of a static image where chroot is denied (podvm T-1901) (to podrunner) | closed (released in `v0.2.0-beta.21`) | T-1901 |
| #392 | [gh action] requested | closed (gate 43682978612, arm 43683769568, trial 43684116751, release 43684654940: `v0.2.0-beta.21`) | none: a request of the gate, `arm.yml`, the nightly trial and the release `0.2.0-beta.21` at e4dd9168 (AGENTS.md, section 9) |
| #393 | COORD-127: a client of the QEMU Guest Agent for a running Windows guest (pod-qga?) (podvm T-1868) (to podcrates) | closed (v0.51.0; T-1868 wires it) | T-1868 |
| #394 | COORD-128: pod-services: the aarch64 netboot of Alpine 3.22.6 for an arm64 guest (podvm T-1890) (to podcrates) | closed (v0.51.0, 3f92dab3) | T-1890 |
| #395 | COORD-129: pod-oci: one registry token for the parallel blob fetches of a pull (podvm T-1850) (to podcrates) | closed (podcrates v0.52.0) | T-1850 |
| #396 | --podvm-confine stops name resolution in the payload, while TCP to an address works | closed (T-1902) | T-1902 |
| #397 | podvm probe takes userland on a host with user namespaces and KVM | closed (T-1903) | T-1903, T-1909 |
| #398 | podvm load names an image docker.io/library/runner | closed (T-1904) | T-1904 |
| #399 | The release has no attestation in GitHub's store, and its .sigstore cannot be read by gh attestation verify | closed (T-1905) | T-1905 |
| #400 | The store keeps machine/run-PID-* after each guest of the machine tier | closed (T-1906) | T-1906 |
| #401 | Under QEMU user mode, probe says kvm: yes for the device of the host | closed | T-1907 |
| #402 | A podvm payload has the host name of the host | closed (T-1908) | T-1908 |
| #403 | COORD-130: pod-enter: a UTS namespace and a host name on the namespace rung (to podcrates) | closed (not needed: T-1908, T-1909) | T-1908 |
| #404 | COORD-131: run --hostname is on main, for PodUtils/podrunner#115 (to podrunner) | closed (PodUtils/podrunner#115 closed: the answer holds, COORD-173) | T-1908 |
| #405 | [gh action] requested | closed (run 43689812413) | T-1867: a request of `kvm.yml` (experiment 452 with screenshots) |
| #406 | COORD-132: pod-enter: enter the namespace rung through a user namespace (to podcrates) | closed | T-1909 |
| #407 | COORD-133: pod-hostprobe: kvm is yes only for a device that runs a guest of the build's architecture (to podcrates) | closed (podcrates@T-676 done; T-1907 done) | T-1907 |
| #408 | [gh action] requested | closed (stopped at the gate: the snapshot) | T-1905: the release of 0.2.0-beta.22 at 7e45abc4 |
| #409 | [gh action] requested | closed (stopped at the gate: plant 27f) | T-1905: the release of 0.2.0-beta.22 at 7189f0e6 |
| #410 | [gh action] requested | closed (release 43708756057) | T-1905: the release of 0.2.0-beta.22 at d2004f12 |
| #411 | [gh action] requested | closed (run 43709734103) | T-1867: a request of `kvm.yml` with the install in three legs |
| #412 | [gh action] requested | closed (gate, arm, trial green; the release tag got 403) | the release of 0.2.0-beta.23 at 86574c96 |
| #413 | COORD-134: podvm took podcrates v0.51.0 (to podcrates) | closed (PodUtils/podcrates#176 closed) | T-1890, T-1747, T-1868 |
| #414 | COORD-135: pod-enter: the device nodes of the host on the namespace rung through a user namespace (to podcrates) | closed | T-1912 |
| #415 | [gh action] requested | closed (run 43716914244) | T-1747: the job cgroup of kvm.yml (experiment 459) |
| #416 | [gh action] requested | closed (403 again; call C22) | the rerun of the release run 43716584904 of 0.2.0-beta.23 |
| #417 | COORD-136: the answer to PodUtils/podrunner#168: --store and the attestations (to podrunner) | closed (answered) | T-1913, T-1905 |
| #418 | [gh action] requested | closed (run 43726504386) | T-1747: the job cgroup of kvm.yml after a2227588 |
| #419 | [gh action] requested | closed (run 43726557269) | T-1867: the job install of kvm.yml with the logs of Setup |
| #420 | [gh action] requested | closed (run 43730876201) | the release of 0.2.0-beta.23 at 437bd12d |
| #421 | COORD-137: the answer to PodUtils/podrunner#177: descriptor 17 (to podrunner) | closed (PodUtils/podrunner#177 closed: the answer holds, COORD-173) | T-1914 |
| #422 | COORD-138: pod-enter: the root directory never on a number of a passed descriptor (to podcrates) | closed | T-1914 |
| #423 | COORD-139: the answer to PodUtils/podrunner#178: tar and a link before its target (to podrunner) | closed (PodUtils/podrunner#178 closed: the answer holds, COORD-173) | T-1915 |
| #424 | COORD-140: the answer to PodUtils/podrunner#180: lab profiles from podvm (to podrunner) (to podcrates) | closed (PodUtils/podrunner#180 closed: the part of podvm holds, COORD-173) | T-1916 |
| #425 | COORD-141: the answer to PodUtils/podnet#584: what podvm's help and manual need (to podnet) (to podcrates) | closed (PodUtils/podnet#584 closed) | T-1917 |
| #426 | [gh action] requested | closed (run 43735766607 failed) | the release of 0.2.0-beta.24 from b6f8958e (call C22); the trial failed on the cross targets |
| #427 | COORD-142: the answer to PodUtils/podcrates#182: podvm has no installer yet (to podcrates) | open | T-1918 |
| #428 | COORD-143: the answer to PodUtils/podcrates#183: how podvm maps an issue to its entries (to podcrates) | closed (PodUtils/podcrates#183 closed 2026-11-18; T-1919 done) | T-1919 |
| #429 | [gh action] requested | closed (run 43739088606) | the release of 0.2.0-beta.24 from 95d1f3eb (call C22) |
| #430 | COORD-144: the answer to PodUtils/podrunner#182: podvm lab run is in v0.2.0-beta.24 (to podrunner) | closed (PodUtils/podrunner#182 closed: the answer holds, COORD-173) | T-1916 |
| #431 | [gh action] requested | closed (a wrong hash; replaced by #432) | the kvm.yml run for T-1747, T-1907 and T-1867 |
| #432 | [gh action] requested | closed (run 43740331439) | the kvm.yml run for T-1747, T-1907 and T-1867 from 283e3492 |
| #433 | [gh action] requested | closed (run 43754060132) | the kvm.yml rerun for T-1747, T-1907 and T-1867 from 6539446a |
| #434 | [gh action] requested | closed (run 43767666467) | the kvm.yml rerun for T-1747 (swap) and T-1867 (the sampler) from 7a6c1edb |
| #435 | COORD-145: pod-todo: a kind `reference` for an issue that holds no work (to podcrates) | closed | T-1919 |
| #436 | [gh action] requested | closed (run 43770947835) | the first dry run of issue-sync.yml (T-1919) |
| #437 | COORD-146: issue-sync.yml builds pod-todo under the consumer's .cargo/config.toml (to podcrates) | closed | T-1919 |
| #438 | [gh action] requested | closed (run 43775969796) | the dry run of issue-sync.yml at podcrates v0.57.0 (T-1919) |
| #439 | COORD-147: the answer to PodUtils/podrunner#189: raw syscalls on the loader family; a volume moves a run off the namespace rung (to podrunner) | open | T-1920, T-1740 |
| #440 | COORD-148: pod-enter: bind the volumes of a plan on the namespace rung through a user namespace (to podcrates) | closed (podcrates `v0.59.0`, taken in `aa281ba7`) | T-1920 |
| #441 | [gh action] requested | closed (run 43783602911, read into T-1867 and T-1350) | the kvm.yml run for the install of 8 GiB (T-1867) and ValidationOS (T-1350) from 1298b66a |
| #442 | COORD-149: the answer to PodUtils/podrunner#193: Node 26 on the loader family; the microVM of a user with no root (to podrunner) | closed (PodUtils/podrunner#193 closed) | T-1920 |
| #443 | COORD-150: pod-hostprobe: chroot_usable takes the row chroot(/tmp) in unshare(NEWUSER+maps) as the caller's chroot (podvm T-1921) (to podcrates) | closed (podcrates@T-758 in podcrates `v0.61.0`) | T-1921 |
| #444 | COORD-151: the answer to PodUtils/podrunner#195: a user with no root passes the machine tier; run your probe with v0.2.0-beta.25 for the cause (to podrunner) | closed (PodUtils/podrunner#195 closed) | T-1922 |
| #445 | COORD-152: podvm took podcrates v0.60.0 in 959af827 (the answer to PodUtils/podcrates#206) (to podcrates) | closed (podcrates recorded the take on PodUtils/podcrates#206) | T-1921 |
| #446 | COORD-153: podvm took podcrates v0.61.0 in 013e76a1; COORD-150 is answered (the answer to PodUtils/podcrates#209) (to podcrates) | closed (podcrates recorded the take on PodUtils/podcrates#209) | T-1921, T-1924, T-1925 |
| #447 | COORD-154: pod-enter: the id inside the user namespace of the namespace rung, so that a payload runs as the user of its image (podvm T-1927, PodUtils/podrunner#196) (to podcrates) | closed (podcrates@T-786 in podcrates `v0.62.0`, taken in `e62a7dc1`) | T-1927 |
| #448 | COORD-155: the answer to PodUtils/podrunner#196: --hostname of run -d is fixed on main; the user of the image waits for pod-enter (to podrunner) | closed (PodUtils/podrunner#196 closed) | T-1926, T-1927 |
| #449 | COORD-156: podvm took podcrates v0.62.0 in e62a7dc1; Plan::userns_ids is wired for run -d (the answer to PodUtils/podcrates#210) (to podcrates) | closed (podcrates recorded the take on PodUtils/podcrates#210) | T-1927 |
| #450 | COORD-157: the answer to PodUtils/podrunner#197: descriptor 17 is fixed in v0.2.0-beta.26 (to podrunner) | closed (PodUtils/podrunner#197 closed) | T-1928 |
| #451 | COORD-158: the answer to PodUtils/podrunner#198: not the size and not the user; run your probe with v0.2.0-beta.26 for the timing (to podrunner) | closed (PodUtils/podrunner#198 closed) | T-1922 |
| #452 | COORD-159: pod-enter: Plan::userns_ids for a caller that is uid 0 of a user namespace (a nested namespace), for a foreground podvm run (podvm T-1927) (to podcrates) | closed (podcrates@T-787 in podcrates `v0.63.0`) | T-1927 |
| #453 | COORD-160: podvm took podcrates v0.63.0; the foreground run has the ids of the plan (the answer to PodUtils/podcrates#211) (to podcrates) | closed (podcrates recorded the take on PodUtils/podcrates#211) | T-1927 |
| #454 | COORD-161: the answer to PodUtils/podrunner#199: a 1.4 GiB initrd needs more than 4G of guest memory; give the unit 8G (to podrunner) | closed (PodUtils/podrunner#199 closed) | T-1922 |
| #455 | COORD-162: pod-lab: the certificate file of a lab with a way out has no roots of the host; the stand-in dies of SIGPIPE in a consumer with the default SIGPIPE (to podcrates) | closed (podcrates `v0.64.0`: podcrates@T-792, podcrates@T-796) | T-1924 |
| #456 | COORD-163: pod-services: list fedorapeople.org, the host of the virtio-win guest tools (podvm T-1868) (to podcrates) | closed (podcrates `v0.64.0`: podcrates@T-800, podcrates@T-804) | T-1868 |
| #457 | COORD-164: pod-enter: a PID namespace with its own /proc on the namespace rung (podvm T-1930, PodUtils/podrunner#200) (to podcrates) | closed (podcrates `v0.65.0`: podcrates@T-806; the defect is COORD-167, #460) | T-1930 |
| #458 | COORD-165: the answer to PodUtils/podrunner#200: a PID namespace for run -d needs pod-enter first (COORD-164) (to podrunner) | closed (PodUtils/podrunner#200 closed after `v0.2.0-beta.30`) | T-1930 |
| #459 | COORD-166: podvm took podcrates v0.64.0; COORD-162 and COORD-163 are answered (the answer to PodUtils/podcrates#214) (to podcrates) | closed (podcrates read it: the takes of PodUtils/podcrates#214) | T-1924 |
| #460 | COORD-167: pod-enter v0.65.0: the init of the PID namespace keeps the ready pipe of pod-supervise, so run -d never returns (podvm T-1930) (to podcrates) | closed (podcrates `v0.66.0`: podcrates@T-811) | T-1930 |
| #461 | COORD-168: podvm took podcrates v0.66.0 in b1fa48f; the PID namespace of run -d is on (the answer to PodUtils/podcrates#216) (to podcrates) | closed (podcrates read it: the takes of PodUtils/podcrates#216) | T-1930 |
| #462 | COORD-169: podvm took podcrates v0.67.0 (the answer to PodUtils/podcrates#218) (to podcrates) | closed (podcrates read it: the takes of PodUtils/podcrates#218) | T-1930 |
| #463 | COORD-170: pod-rootfs: the source fixup reads URLs in comments, so it probes help.ubuntu.com (podvm T-1931, PodUtils/podrunner#202) (to podcrates) | closed (podcrates `v0.68.0`: podcrates@T-846) | T-1931 |
| #464 | COORD-171: the answer to PodUtils/podrunner#202: the connections are the source fixup; pass --no-source-fixup (to podrunner) | closed (PodUtils/podrunner#202 closed: podrunner passes `--no-source-fixup` since `2d481dd`) | T-1931 |
| #465 | COORD-172: podvm took podcrates v0.68.0; COORD-170 is answered (the answer to PodUtils/podcrates#219) (to podcrates) | closed (podcrates read it: the takes of PodUtils/podcrates#219) | T-1931 |
| #466 | COORD-173: six answers of podvm wait for podrunner to close its asks #115, #177, #178, #180, #182, #189 (to podrunner) | open | T-1908, T-1914, T-1915, T-1916, T-1920 |
| #467 | COORD-174: pod-enter stage::copy_tree refuses the device nodes of a tree of root, so PODVM_MODE=rundir and tmpfs never stage (podvm T-1929) (to podcrates) | closed (podcrates@T-857 in podcrates `v0.70.0`, PodUtils/podcrates#222, taken in `6b5a6cb`) | T-1929 |
| #468 | COORD-175: pod-guest: evidence of a mailbox run that times out before WQREADY (podvm T-1933) (to podcrates) | closed (podcrates@T-861 in podcrates `v0.70.0`, PodUtils/podcrates#222, taken in `6b5a6cb`) | T-1933 |
| #469 | COORD-176: pod-enter: a new devpts instance and /dev/ptmx on the namespace rung, so that openpty works in a unit (podvm T-1934, PodUtils/podrunner#216) (to podcrates) | closed (podcrates@T-864 and @T-868 in podcrates `v0.70.0`, PodUtils/podcrates#222, taken in `6b5a6cb`; T-1934 done) | T-1934 |
| #470 | COORD-177: the answers to PodUtils/podrunner#216 (no pty: namespace rung) and #189 (raw renameat2 on the userland rung) (to podrunner) | open | T-1934, T-1935 |
| #471 | COORD-178: the answer to PodUtils/podnet#649: what the gates of podvm prove, and the gap that real tools found (to podnet) | closed (podnet read the answer, PodUtils/podnet#649 state of 2026-11-18 10:40 (UTC)) | T-1936 |
| #472 | `run -d` of a payload that exits immediately is recorded `Dead` with no exit code, and `wait` refuses | open | T-1938 |
| #473 | `--device` opens the host file but the payload never sees it; reads get `ENOENT` and writes land in the image | open | T-1939 |
| #474 | `run -d --rm` leaves the container record and its rootfs behind | open | T-1939 |
| #475 | Every refused `run --strict=all` leaves a zero-byte `memo-*.tmp` file in the store | open | T-1938 |
| #476 | `images --filter` ignores the filter for the `windows:dos` record | open | T-1940 |
| #477 | `load` rejects a valid OCI-layout tarball that contains directory entries | open | T-1940 |
| #478 | `man` silently ignores `--json` or `--roff` when `--llms` is also given | open | T-1937 |
| #479 | `doctor --strict=false` still enables strict, and `--strict=0` reports an internal podvm bug | open | T-1937 |
| #480 | `--add-host NAME:host-gateway` writes the literal string `host-gateway` as an IP address | open | T-1940 |
| #481 | The built-in `machine` profile always refuses on a host with the default `RLIMIT_FSIZE` | open | T-1941 |
| #482 | `run --profile NAME IMAGE CMD` silently treats IMAGE as the command when the profile names an image | open | T-1937 |
| #483 | `abi`, `df`, `prune`, and `install-names` are listed as Native top-level verbs but exit 125 | open | T-1937 |
| #484 | Container names are not validated: empty, slash, space, and 300-character names are accepted | open | T-1937 |
| #485 | `logout` help promises an idempotent logout, but a second logout exits 1 | open | T-1937 |
| #486 | `logs` and `version` silently ignore extra positional arguments | open | T-1937 |
| #487 | `image <unknown>` says "not implemented yet" instead of "not a subverb" | open | T-1937 |
| #488 | `--` works as an option terminator in `run` but is rejected by `exec`, `pull`, and `images` | open | T-1937 |
| #489 | `exec --platform` is accepted but has no effect on a container | open | T-1937 |
| #490 | `verify` reports a mismatched blob, but `pull` says "Already exists" and does not repair it | open | T-1940 |
| #491 | `doctor` ends with an unexplained `profile: refused` line | open | T-1937 |
| #492 | `--tls-verify=B` never consumes its value, so TLS verification cannot be disabled | open | T-1943 |
| #493 | `notebook --cycles N` silently clamps N above 1000 to 1000 | open | T-1943 |
| #494 | `notebook --restart-kernel-after CELL` accepts a cell past the end and silently does nothing | open | T-1943 |
| #495 | `cp` from a container rewrites absolute symlink targets to the store's rootfs path | open | T-1944 |
| #496 | Credential helpers receive `https://HOST/v2/`, not the configured server key | open | T-1944 |
| #497 | `run --help` omits working flags that the parser accepts and the parity table lists | open | T-1943 |
| #498 | `cp` to an image mutates only the extracted cache; `extract --force` and `save` lose the change | open | T-1944 |
| #499 | `--podvm-confine` help omits the implicit `/proc`, device, and TCP grants | open | T-1943 |
| #500 | `login` with a missing or empty password prints the whole usage instead of naming the missing input | open | T-1943 |
| #501 | Concurrent `podvm run` fails with `containers.json: No such file or directory` | open | T-1938 |
| #502 | `remote ssh forward exec` discards the target's exit code | open | T-1944 |
| #503 | `import` records an image from a tar with an absolute hardlink that `extract` refuses | open | T-1944 |
| #504 | `compare --strict` treats an empty reason as an explanation | open | T-1943 |
| #505 | Any non-UTF-8 command-line argument panics podvm with SIGABRT | open | T-1943 |
| #506 | `notebook --kill-after` panics on a large value instead of refusing it | open | T-1943 |
| #507 | The image-`USER` warning says podvm runs as uid 0, but on the userland rung it runs as the caller's uid | open | T-1943 |
| #508 | `--podvm-tier=syscall -u UID` says the identity is faked, but the payload runs as the caller | open | T-1943 |
| #509 | COORD-179: the answer to PodUtils/podrunner#277: exec --env-file is on main; a -v source with no link is in progress (to podrunner) | open | T-1945 |
| #510 | `podvm exec` joins a container's namespaces or shares the host PID table, while its help and `.Exec.Mode` say fresh chroot, filesystem only | open | T-1948 |
| #511 | Machine-tier records ignore `PODVM_STORE`: `ps` lists them from `$HOME`, while `inspect` and `wait` cannot find them | open | T-1954 |
| #512 | `-w /tmp/<new dir>` always fails with ENOENT after podvm creates the directory | open | T-1953 |
| #513 | A `--report` path that cannot be written runs the payload first, then returns 125 in place of its exit code | open | T-1951 |
| #514 | `podvm inspect` with more than one container prints several JSON arrays, so its output is not valid JSON | open | T-1951 |
| #515 | `podvm import` refuses a compressed file with a raw tar error, not the named refusal the help promises | open | T-1952 |
| #516 | A normal `tar -C DIR -cf bundle.tar .` is refused by `--podvm-sealed` and blamed on `..` or a leading `/` | open | T-1954 |
| #517 | A sealed bundle with an empty or missing `argv` exits 0 and runs nothing | open | T-1951 |
| #518 | `podvm man --roff` and `--llms` print every usage line twice, as `Usage: podvm run usage: podvm run ...` | open | T-1951 |
| #519 | Seven verbs exit 125 for a missing required argument, where the exit table says 1 | open | T-1951 |
| #520 | `podvm --help` omits the working verbs `port` and `logout`, and the unknown-command refusal points at it | open | T-1951 |
| #521 | `system abi` exits 2 for an unreadable file, a code the published exit table does not contain | open | T-1951 |
| #522 | `podvm save -o FILE` prints the file name on stdout, which the help reserves for the tarball | open | T-1951 |
| #523 | `podvm inspect NAME` returns the image when a container and an image share the name, while every other verb returns the container | open | T-1952 |
| #524 | The digest `inspect` prints is not accepted by `inspect`, `rmi`, `tag` or `cp`, and two printed fields are not selectable by `--format` | open | T-1951 |
| #525 | `import` accepts a rootfs tar whose hard link precedes its target, then every `extract` and `run` fails with a false "already extracted" message | open | T-1952 |
| #526 | `system df` counts one extracted rootfs once per record, and its reclaimed figures do not match the disk | open | T-1952 |
| #527 | A pull killed with SIGTERM leaves zero-byte `.partial` files and blobs no image names, and no verb reclaims them | open | T-1952 |
| #528 | `pull --platform linux/amd64/v3` stores a platform the index does not offer, and the tag then refuses `extract` | open | T-1952 |
| #529 | Refusals for `--tmpfs` name `-v` and print an internal store path the caller never passed | open | T-1952 |
| #530 | The syscall tier's `uname -v` contains an internal task reference: `podvm syscall tier (TODO/enter.md T-1657)` | open | T-1951 |
| #531 | `podvm version --verbose` reports `commit:` as a build error string instead of a commit | open | T-1951 |
| #532 | Five command groups print help and exit 0 for a bare name, three exit 125, and the exit table names only the first behavior | open | T-1951 |
| #533 | `run --help` says a `-w` or a first program inside a volume is refused, but both work | open | T-1951 |
| #534 | A foreground run piped into `head` dies of SIGPIPE and leaves a permanent `Created` record and its tree | open | T-1953 |
| #535 | A direct SIGINT to `podvm run` is ignored, although the help says SIGINT is forwarded | open | T-1953 |
| #536 | A symlink at `containers/<id>/log` makes the launcher append the payload output to a file outside the store | open | T-1953 |
| #537 | The machine tier replaces non-UTF-8 stdout bytes with U+FFFD and appends a newline | open | T-1954 |
| #538 | A sealed bundle with `./` members and a directory entry never returns: the guest channel fails to unpack and podvm hangs | open | T-1954 |
| #539 | `stop` on a machine guest powers the VM off with QMP quit, so the payload never gets SIGTERM and no exit code is recorded | open | T-1954 |
| #540 | `logs` on a machine-tier container drops payload stderr, and `--tail` is ignored | open | T-1954 |
| #541 | `podvm exec` on a machine-tier container enters the running guest, while the help and limits say a guest exec is refused | open | T-1951 |
| #542 | The machine-tier `--device` refusal also says `-v` needs the chroot tier, but `-v` works on the machine tier | open | T-1951 |
| #543 | `docs/limits.md` says an arm64 guest is refused before boot and also that it runs with `qemu-system-aarch64` | open | T-1951 |
| #544 | Pulling `linux/arm64` and `linux/arm64/v8` creates two records for one manifest | open | T-1952 |
| #545 | `podvm run -t` writes a NUL byte to stdout before the payload output | open | T-1951 |
| #546 | The probe, README, architecture, SECURITY, the limits page and the banner disagree with the run about user, PID and UTS namespaces and `/proc` | open | T-1951 |
| #547 | An image with no `/tmp` downgrades the namespace rung to chroot, and the report still says `namespace` | open | T-1953 |
| #548 | `podvm rmi TAG` refuses to remove the tag whenever any container references the image | open | T-1952 |
| #549 | `system install-names` creates the `podman` link and then exits 125 because it refused the `docker` name | open | T-1953 |
| #550 | `podvm logs -f` stops following after `restart` and exits 0 while the new run is still writing | open | T-1953 |
| #551 | `--tmpfs P` is listed in the payload's `/proc/mounts` as the host's ext4 root, and `/tmp` appears twice | open | T-1952 |
| #552 | `--env-file` reads a file the caller cannot read, because the run is root of a user namespace | open | T-1953 |
| #553 | `--unsafe-host-paths` is accepted and does nothing on the namespace rung | open | T-1953 |
| #554 | `create --podvm-tier=emulate` refuses with a chroot-denied message that is false on this host and never names the tier | open | T-1953 |
| #555 | `cp` drops a trailing slash on a missing destination and creates a file where a directory was implied | open | T-1952 |
| #556 | `-v` is a real bind mount, `/proc/mounts` lists it, and `mountinfo` exposes the host path, while the help says it is not a mount | open | T-1951 |
| #557 | `--strict=all` refuses every run on this host because the mtab fixup says `/proc` is not mounted, and it is | open | T-1953 |
| #558 | The probe cache key is namespace-local, so `--cached` alternates "served" and "the confinement changed", and every pull re-probes | open | T-1953 |
| #559 | `podvm login` prints `Login Succeeded` for a registry that is not listening | open | T-1951 |
| #560 | The source fixup leaves `http://security.ubuntu.com` although it answers HTTPS, and names the redirect target in the error | open | T-1953 |
| #561 | `podvm probe` leaves `/tmp/execprobe`, and every later probe skips the exec row because the file exists | open | T-1953 |
| #562 | SIGHUP to a foreground run leaves the payload and `podvm --podvm-ns-reaper` running, and `--rm` does not remove the Dead record | open | T-1953 |
| #563 | `cp` resolves a container/image name collision to the image and writes into the shared base image | open | T-1952 |
| #564 | Under a low `RLIMIT_NOFILE`, a run prints that its record is not live, exits 0, and leaves no record | closed (by design: the record lands at the end of the run, T-1950) | T-1950 |
| #565 | `-v` with a file, FIFO or device host path fails with `bind: Not a directory` and names the guest path | open | T-1953 |
| #566 | SIGTERM to a foreground Windows/DOS guest kills podvm and leaves QEMU running, and Ctrl-C to the group does not stop it | open | T-1953 |
| #567 | COORD-180: podvm called beta.31 ready and an outside tester confirmed 37 faults in one hour; what podvm changes (to all) | open | T-1946, T-1947, T-1948, T-1949 |
| #568 | COORD-181: five faults of the external QA of podvm that are in crates of podcrates: the table's temp file, the exec probe's name, a helper's ServerURL, run -d --rm, a -v source with no link (to podcrates) | closed (podcrates@T-872 to @T-876 in podcrates `v0.70.0`, PodUtils/podcrates#222, taken in `6b5a6cb`) | T-1938, T-1939, T-1944, T-1945 |
| #569 | COORD-182: ten more faults of the external QA of podvm in crates of podcrates: signals, the entered rung, logs, mtab, the probe key, census and guest (to podcrates) | closed (podcrates@T-884 to @T-895 in podcrates `v0.70.0`, PodUtils/podcrates#222, taken in `6b5a6cb`) | T-1953 |
| #570 | COORD-183: the answer to PodUtils/podcrates#221: the two rulings are in podvm's AGENTS.md; the four copies go at the next take (to podcrates) | open | T-1955 |
| #571 | COORD-184: the state of podvm at the start of its session of 2026-11-18, for the coordinator (to podcrates) | open | T-1955 |
| #572 | COORD-185: the answers to PodUtils/podrunner#277, #216, #189, #168 and #110: each is in podvm 0.2.0-beta.32 (to podrunner) | open | T-1934, T-1935, T-1945 |
| #573 | [Donor] Syscall tiers: selective trap, coverage guard, io_uring, raw paths | open | T-1956 |
| #574 | [Donor] Limits: byte accounting, cgroup kill, resource flags, WASM | open | T-1957 |
| #575 | [Donor] Confinement and policy: verdicts, ELF closure, credentials, seccomp profile | open | T-1958 |
| #576 | [Donor] Machine tier: QEMU quoting, guest limits, guest agent, warm start, confine | open | T-1959 |
| #577 | [Donor] Images and store: unique base archive, block image, lazy pull, layers | open | T-1960 |
| #578 | [Donor] Network gateway: ICMP relay, runtime expose, RST, MTU | open | T-1961 |
| #579 | [Donor] Host probes and namespaces: inward probe, per-namespace probe, nsfs handle | open | T-1962 |
| #580 | [Donor] Research backlog: UML, VM islands, checkpoint, translators, record and replay | open | T-1963 |
| #581 | [Donor] Index: the podvm donor study of 2026-11-17 | open | T-1956 to T-1963 |
| #582 | COORD-186: pod-incus: the state of an instance (CPU time, peak memory, disk) and the pool of its root disk, for asks 1, 2 and 6 of PodUtils/podrunner#296 (to podcrates) | closed (podcrates@T-925 in podcrates `v0.71.0`, PodUtils/podcrates#228, taken in `c213375`) | T-1964, T-1965, T-1969 |
| #583 | COORD-187: the answer to PodUtils/podrunner#296: the state of each of the seven asks, and when each comes (to podrunner) | open | T-1964 to T-1969 |
| #584 | COORD-188: pod-enter: a session with the pty as the controlling terminal of the payload (Fds), so that Ctrl-C and job control work under run -t (to podcrates) | open | T-1970 |
| #585 | COORD-189: the answer to PodUtils/podrunner#300: --env-file0 is on main; --env-file holds on the machine and incus tiers; --podvm-volume-base off the namespace rung is T-1972 (to podrunner) | open | T-1971, T-1972 |
| #586 | COORD-190: the answer to PodUtils/podrunner#301: the syscall tier now maps the path of a Unix socket (on main, for the next beta) (to podrunner) | closed (podrunner closed PodUtils/podrunner#301 with `v0.2.0-beta.33`) | T-1973 |
| #587 | COORD-191: podvm took podcrates v0.71.0 in c213375 (and v0.70.0 in 6b5a6cb) (to podcrates) | closed (PodUtils/podcrates#228 lists the take; `v0.2.0-beta.33` released) | T-1955 |
| #588 | COORD-192: the answer to PodUtils/podrunner#303: the syscall tier took glibc's AT_FDCWD for a descriptor; fixed on main (to podrunner) | closed (podrunner closed PodUtils/podrunner#303 with `v0.2.0-beta.34`) | T-1977 |
| #589 | COORD-193: pod-enter: abi interp() refuses a PT_INTERP past the head read; Fedora 42's bash has it at 0xf2000 (to podcrates) | closed (podcrates `v0.72.0` carries it; podvm took it in b3ff9c3) | T-1981 |
| #590 | COORD-194: the answer to PodUtils/podrunner#304: the tier hides no host process; run -d with --podvm-confine on the tier failed to start (fixed on main) (to podrunner) | open | T-1982 |
| #591 | COORD-195: podvm took podcrates v0.72.0 in b3ff9c3 (to podcrates) | closed (PodUtils/podcrates#229 lists the take) | T-1981 |
| #592 | COORD-196: three limits of --podvm-confine that podvm's fourth outside pass found: exec= on the syscall tier, a program through ld.so, chmod and times (to podrunner) | open | T-1983, T-1984, T-1985 |
| #593 | COORD-197: the answer to PodUtils/podrunner#308: errno 11 is an openat2 that a rename or a mount raced (a retry on main); the cargo doc hang does not reproduce here (to podrunner) | open | T-1990, T-1991 |
| #594 | COORD-198: podvm took podcrates v0.73.0 in 20313a0 (to podcrates) | closed (PodUtils/podcrates#231 lists the take) | T-1955 |
| #595 | COORD-199: podvm's entry for the ruling "ghcr.io": T-1992, image.yml in the form of C789 (to podcrates) | closed (T-1992 done: run 44452103325, index sha256:e11a9a70 in the notes of v0.2.0-beta.35) | T-1992 |
| #596 | COORD-200: the answer to PodUtils/podrunner#313: systemd on the incus tier first (--podvm-init=image, T-1993), then the machine tier in three steps (T-1994) (to podrunner) | open | T-1993, T-1994 |
| #597 | COORD-201: podvm took podcrates v0.74.0 in ef56d16 (to podcrates) | closed (PodUtils/podcrates#237 lists the take) | T-1955 |
| #598 | COORD-202: the answer to PodUtils/podrunner#315: Go's SIGURG of preemption stopped podvm's SIGSYS handler; fixed on main in fbc8fc7 (T-1995) (to podrunner) | closed (PodUtils/podrunner#315 closed: the row rbg-gate passes on 0.2.0-beta.36, user-gate.yml run 44489222156) | T-1995 |
| #599 | COORD-203: the answer to PodUtils/podrunner#316: the hosted snapshot on podvm is T-1996, T-1999, T-1997, T-1994 and T-1998, in that order (to podrunner) | open | T-1994, T-1996, T-1997, T-1998, T-1999 |
| #600 | COORD-204: podvm took podcrates v0.75.0 in f3beec1 (to podcrates) | closed (PodUtils/podcrates#238 lists the take) | T-1955 |
| #601 | COORD-205: podvm took podcrates v0.76.0 in 16f2395; the form of the sidecar's xattrs fits (to podcrates) | closed (PodUtils/podcrates#239 lists the take and the answer on the form) | T-1955, T-1999 |
| #602 | COORD-206: a User-Mode Linux kernel in seccomp mode for a guest with no KVM, no ptrace and no namespace (to podcrates) | open | T-2005 |
| #603 | COORD-207: pod-enter adds the RUNPATH directories of a program to the library path of the loader family (to podcrates) | open | T-2003 |
| #604 | COORD-208: a pinned fetch of a static tool (gVisor's runsc) for a project that needs it at run time (to podcrates) | open | T-2009 |
| #605 | COORD-209: --podvm-init=image now holds on a foreground run with no tier, on --podvm-tier=runsc and on --podvm-tier=machine (to podrunner) | open | T-2002, T-2007 |
| #606 | COORD-210: podvm took podcrates v0.78.1 (and v0.77.0) in 3272e3d (to podcrates) | closed (PodUtils/podcrates#240 and #241 list the take) | T-1955 |
| #607 | [gh action] requested | open | the gate, secrets and issue-sync runs of 163c231, the take of podcrates v0.78.1 (T-1955) |
| #608 | COORD-211: pod-enter's wait_forwarding forwards SIGHUP, SIGINT and SIGTERM only: a SIGUSR1 or SIGQUIT kills the waiter and the payload stays (to podcrates) | open | T-2001 |

## Messages whose title does not start with COORD-NNN

The sessions of other people wrote these messages with a title of their own. The column Kind names them as messages (pod-todo).

| Issue | Subject | State | Kind | Entries |
| --- | --- | --- | --- | --- |
| #331 | COORD-091 of Nemo-010: `system abi` refusals carry their own verdict label (to podcrates) | closed (podcrates@T-568 in podcrates `v0.43.0`, taken in `b0bb87f1`) | coord | none: podcrates@T-568 (PodUtils/podcrates#147)  |
| #335 | COORD-092 of Nemo-010: `store_exec` attributes an `EACCES` to the filesystem on a mount that is not noexec (to podcrates) | closed (podcrates@T-569 in podcrates `v0.43.0`, taken in `b0bb87f1`) | coord | none: podcrates@T-569 (PodUtils/podcrates#147)  |
| #337 | COORD-093 of Nemo-010: the machine-tier refusal runs its legs together, and `probe` prints the verdict twice (to podcrates) | closed (podcrates@T-570 in podcrates `v0.43.0`, taken in `b0bb87f1`) | coord | none: podcrates@T-570 (PodUtils/podcrates#147)  |
