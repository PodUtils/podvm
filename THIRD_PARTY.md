# Third-party material

Project-owned work is [0BSD](LICENSE).
Copied material retains its own licence and notices.
[reference-map](TODO/reference-map.md) records each captured source decision.

## Imported documents and scripts

| Material | Source revision | Licence | Adaptation |
| --- | --- | --- | --- |
| Initial conventions, methodology, and security pages | TEMPLATE `6206166`, through container-research `0f155e3` | 0BSD | Project router and work model |
| Tooling, container, public-document pages, common checks, doctor scripts, hook, and repository settings | TEMPLATE commit `ea26c7d91a087a7132f56034fb77f8d40d7766cb` | 0BSD | Rust paths, corpus exclusions, Windows lane |
| Initial experiments 10 through 50 and target reconstruction | container-research `0f155e3` | 0BSD | Project measurements and proof |

The template notice is retained in
[a copy of its LICENSE](docs/notices/Azathothas__TEMPLATE-LICENSE.txt),
taken from the capture at TEMPLATE commit `ea26c7d`. The capture itself left
the tree with the corpus on 2026-11-06 (T-1606). The revision and the licence
are recorded in [reference-map](TODO/reference-map.md), and the imported
pages themselves carry the 0BSD terms, so the notice survives the corpus.
The live runbooks were revised on 2026-09-30 for the actual project workflow.
They are adapted documents, not unchanged template copies.

## Reference corpus

Reference source is not compiled into the runtime.
Each capture has provenance, retained notices, and an explicit scope.
The complete licence table has one home in
[reference-map](TODO/reference-map.md).
Selected captures state their omissions. They are not complete source trees.

Project prose checks exclude immutable reference text.
Saved project results remain in scope.
Private path substitutions in a published result must be stated explicitly.

## Vendored source

| Path | Captured source | Licence | Current state |
| --- | --- | --- | --- |
| `vendor/userland-execve` | io12/userland-execve-rust `02ef0e0` | MIT; LICENSE retained | Source retained. Its manifest has removed dependencies. It is not a workspace member or a dependency of `podvm-enter` or `pod-enter`, and is not linked. |
| `crates/podvm-ssh/shims/fakepwd.c` | sandssh `4fc7f8c` | MIT; adjacent LICENSE retained | Source retained. `experiments/382-restricted-sshd.sh` compiles it on a glibc-dynamic lane for the restricted-server proof of T-1401 (done). No workspace or release build compiles or links it. |

Do not describe retained source as the active userland loader.
The native loader is in `src/userland.rs` of `pod-enter` (PodUtils/podcrates).

## Licence texts kept for a package

[vendor/licence-texts](vendor/licence-texts/README.md) holds the upstream
licence texts of a package of crates.io that ships none of its own
(`asn1-rs-impl` 0.2.0), with the source of each.

## Fetched at run time

podvm does not carry these files. A host with no QEMU fetches them on the
first Windows or DOS run (noVNC: on the first `podvm windows view`), and podvm checks each size and sha256
(`crates/podvm-windows/src/tools.rs`, TODO/machine.md T-1875).

| Material | Source | Licence |
| --- | --- | --- |
| `qemu-system-x86_64`, `qemu-img` 9.2.0, static musl builds | pkgforge `bincache`, package `ppkg.qemu` | GPL-2.0 (QEMU) |
| noVNC 1.7.0 (`core/`, and `vendor/pako`), fetched by `podvm windows view` (T-1899) | registry.npmjs.org, package `@novnc/novnc` | MPL-2.0 (noVNC); MIT (pako) |
| `pc-bios/` firmware and option ROMs of QEMU `v9.2.0` (SeaBIOS, the VGA BIOS, iPXE EFI ROMs, edk2) | github.com/qemu/qemu at tag `v9.2.0` | SeaBIOS and the VGA BIOS LGPL-3.0; iPXE GPL-2.0; edk2 BSD-2-Clause-Patent; qboot GPL-2.0 |

## Build dependencies

Cargo resolves registry source through `Cargo.lock`.
[Source state](docs/runtime-state.md) lists enabled direct dependencies.
[deps](TODO/deps.md) retains the selection measurements.
[release-licenses.py](scripts/release-licenses.py) retains the locked package
licence texts in each SSH helper archive.
Rebuild that inventory for a release whose dependency set changed.
