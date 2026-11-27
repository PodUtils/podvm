# podvm

podvm runs Linux payloads on hosts that can deny namespaces, mounts, chroot,
device creation, ownership changes, or ptrace. It takes Docker and Podman
commands, selects a mechanism from the host probes, and names that
mechanism in its output. Each released binary is static musl.

## Current behavior

The runtime has image acquisition, layer extraction, rootfs completion,
payload entry, lifecycle control, interposition, and a machine tier.
The CLI also provides remote and machine SSH commands.
Most mechanisms are shared crates of
[PodUtils/podcrates](https://github.com/PodUtils/podcrates); [the code map](docs/code-map.md) names them.
The machine tier boots a Linux guest from an OCI image (`run
--podvm-tier=machine`, TODO/machine.md T-1656), and Windows, DOS, and
`machine ssh` guests.
A Linux guest can have no NIC (`--podvm-net none`), one channel to a host
socket (`--podvm-channel`), its run as a bundle on stdin with no host
file of the run (`--podvm-sealed`, T-1751), and a container engine
(`--podvm-engine`, T-1780); the incus tier also takes the first two
(T-1779). [The limits](docs/limits.md#guests) state each flag.
`--podvm-tier` also selects `incus` (an incusd over its REST API,
T-1718; `tier = incus` in `$store/config` selects it with no flag,
T-1722), `runsc` (gVisor, rootless, T-1729), `emulate` (a static
qemu-user, T-1651) and `syscall` (each system call through podvm's
handler, T-1657, T-1731). `run --podvm-init=image` boots the init of the
image (systemd) as PID 1 on the namespace rung, the runsc tier, the
machine tier or the incus tier (T-1993, T-2002, T-2007; [the
limits](docs/limits.md#the-init-of-an-image) state each). `podvm lab` plans
a network lab in one guest (T-1662), and the crate `podvm-api` runs podvm
from Rust (T-1745).

Read [the architecture](docs/architecture.md) for the execution paths.
Read [the limits](docs/limits.md) before you select a path.
[The generated source state](docs/runtime-state.md) names the current build inputs.
[The work record](TODO/PROGRESS.md) gives current verification and remaining work.

## Install

Each release carries `podvm-install.sh`. It finds the
binary of your machine, checks it against the pins in the installer, and
puts it into `~/.local/bin` (`--prefix DIR` sets another place). It needs
no root and no compiler. A second run changes nothing. With
`--offline podvm-VERSION.tar.gz` it needs no network.

```sh
curl -fsSLO https://github.com/PodUtils/podvm/releases/download/TAG/podvm-install.sh
sh podvm-install.sh
```

Each release carries `SHA256SUMS` and its keyless
signature `SHA256SUMS.sigstore.json`. Where `cosign` is on `PATH`, the
installer checks them before a download. To check the
installer itself by hand before the first run:

```sh
curl -fsSLO https://github.com/PodUtils/podvm/releases/download/TAG/SHA256SUMS
curl -fsSLO https://github.com/PodUtils/podvm/releases/download/TAG/SHA256SUMS.sigstore.json
cosign verify-blob --bundle SHA256SUMS.sigstore.json \
  --certificate-identity https://github.com/PodUtils/podvm/.github/workflows/nightly.yml@refs/tags/TAG \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com SHA256SUMS
sha256sum -c --ignore-missing SHA256SUMS
```

By hand: download the binary of your architecture from
[the releases](https://github.com/PodUtils/podvm/releases), check it
against its `.sha256`, and put it on `PATH`:

```sh
curl -fLO https://github.com/PodUtils/podvm/releases/download/TAG/podvm-x86_64
curl -fLO https://github.com/PodUtils/podvm/releases/download/TAG/podvm-x86_64.sha256
sha256sum -c podvm-x86_64.sha256
install -m 0755 podvm-x86_64 ~/.local/bin/podvm
podvm version --verbose
```

Each release also publishes its binaries as an OCI index on ghcr.io
(T-1992). The image is `FROM scratch` with the binary at
`/usr/local/bin/podvm` as its entry point, one image for each
architecture under one index. The tag has the `v` of the release; `beta` or
`latest` names the newest. The notes of the release name the digest of the
index and the SHA-256 of each binary.

```sh
podvm pull ghcr.io/podutils/podvm:vTAG
docker run --rm ghcr.io/podutils/podvm:vTAG --version
```

Each binary is static: it needs no library of the host. The release has
binaries for x86_64, aarch64, armv7, i686, loongarch64, powerpc64le and
riscv64gc; the interposer of the userland rung is in the x86_64 and
aarch64 binaries. `podvm run --help` and `podvm man` list each flag, with
the parity of each docker flag.

## Report a problem

Open [an issue](https://github.com/PodUtils/podvm/issues/new/choose) with
the bug form. Give the command, the image with its tag, what you expected,
and the output of:

```sh
podvm version --verbose
podvm system info --format '{{.Version}} {{.Rung}} {{.EnteredRung}} {{.Platform}} {{.Kernel}}'
podvm probe
```

Run the failing command again with `--verbose` (the banner names the
mechanism that podvm entered) and, for `run`, with `--report report.json`,
which records the rung, the exit code and the times of the run.

## Build on Linux

```sh
sh scripts/cloud-setup.sh
export PATH="$HOME/.local/bin:$PATH"
if [ -f "${CARGO_HOME:-$HOME/.cargo}/env" ]; then
    . "${CARGO_HOME:-$HOME/.cargo}/env"
fi
cargo build --release -p podvm-gate
./target/x86_64-unknown-linux-musl/release/podvm-dev build
./target/x86_64-unknown-linux-musl/release/podvm probe
./target/x86_64-unknown-linux-musl/release/podvm run alpine:latest /bin/echo hello
```

`scripts/cloud-setup.sh` installs the pinned Zig and Rust toolchains,
including Cargo and the musl target, and reports the host. The workspace
builds static musl binaries under `target/x86_64-unknown-linux-musl/release/`.
`podvm-dev build` builds both interposer objects before the CLI embeds them.
The build status verifies input and output bytes.

For repository work, read [AGENTS.md](AGENTS.md) first.
For Claude Code, Codex, and Gemini / Antigravity Cloud, see [Hosted sessions](docs/hosted-sessions.md).
Use `podvm-dev session --check` for a lane report. Use `podvm-dev check`
for the complete Linux check ([the gate](docs/methodology/gate.md)).

The Windows lane on wsl-toolkit is parked. [Container procedures](docs/containers.md)
keep its steps.

## Run a payload

```sh
podvm probe
podvm doctor
podvm run alpine:latest /bin/echo hello
podvm man
```

The probe measures this host. A run is quiet; `--verbose` prints the
banner, which names the selected mechanism. `inspect` and `system info`
report runtime state, and `--report FILE` writes one JSON document of the
run. Each run with `--name` leaves a record that `ps -a` lists;
`podvm container prune` removes each exited record and its tree.

A chroot changes path resolution. The namespace path creates a mount namespace
and private temporary filesystem. For a caller that is not root, a run there also
opens a user, a PID and a UTS namespace; the parts depend on the caller and the
verb, and the banner of the run names them (`podvm man`, ISOLATION). It does not
create network isolation.
Where chroot is denied, the userland path enters a dynamic payload through the image loader.
A static x86_64 payload there runs on the native-static path: podvm resolves each
path of its system calls in the image (Syscall User Dispatch, TODO/enter.md T-1652).
Under `--unsafe-host-paths` it runs from a memfd instead.
Read the named refusal when a required mechanism is absent.

## Windows and DOS guests

The machine tier runs a disposable disk guest through an emulator
(`qemu-system-x86_64`; KVM where `/dev/kvm` opens, else TCG). A host with
no QEMU needs no install: podvm fetches a pinned static QEMU 11.1.3 and its
firmware into its store on the first run, and a QEMU of the host
wins where it is present. `podvm pull windows:NAME` fetches a medium of
the catalog from its vendor URL and checks its pinned sha256; podvm stores
no Windows image of its own.

```sh
podvm run windows:dos ver
podvm run windows:validationos ver
podvm pull windows:2025
podvm run windows:2025 ver
podvm run -d --name dos1 windows:dos
podvm ps
podvm stop dos1
podvm rm dos1
podvm windows doctor
podvm images
```

`podvm pull windows:2025` (also `11` and `10`) installs Windows from its
setup ISO under KVM into a golden image, with one line for each phase; the
password of the user `podvm` is in a file of mode 0600 beside the image.
Under KVM on a hosted runner, `windows:2025` installs in about 9 min and
`windows:11` in 11 to 13 min (TODO/machine.md T-1867).
The first run of `windows:validationos` provisions it one time. `podvm
images` lists the Windows names that the store holds, and `podvm rmi
windows:NAME` removes their files. The operator accepts the terms of each
medium.
A run uses a fresh overlay and a mailbox for command output and exit status.
The [guest task](TODO/milestones.md) records proof per accelerator and guest flavor.

## SSH

```sh
podvm remote ssh --help
podvm machine ssh --help
```

The remote commands use the `node`, `operator`, and `proxy` helper executables.
Place compatible helpers beside podvm, or on PATH as `podvm-ssh-NAME`.
A source workspace build produces them. Read [the SSH limits](docs/limits.md#ssh)
before you use a downloaded standalone binary for remote SSH.

The release workflow also packages all four helpers in an archive for each
architecture. Keep the CLI and helpers from the same release in one directory.
Use the archive's licence directory with the distributed executables.
The archive does not include an external SSH server.
The server endpoint also needs the SSH server named by its configuration.

Select a verified tag and architecture from
[the release assets](https://github.com/PodUtils/podvm/releases).
From this checkout, verify the selected binary and helper archive:

```sh
sh scripts/verify-release.sh TAG ARCH binary
sh scripts/verify-release.sh TAG ARCH ssh
```

Binary `podvm-verify` owns that verification, and the two lines above
are an exec shim.
The verifier requires `gh` and `cosign`.
It checks the tag's workflow identity.
`v0.1.0-beta.12` is the last podbox release, and `v0.2.0-beta.2` is the first podvm release.
The tags `v0.1.0-beta.13` and `v0.2.0-beta.1` have no release.
Tags up to `v0.1.0-beta.13` keep the podbox identity: `podbox-*` assets, signed by the nightly workflow of `Azathothas/podbox`.
Each later tag has the podvm identity and `podvm-*` assets. `podvm-verify` decides the identity from the tag.
`Azathothas/podbox` redirects to this repository, so nobody may create a repository of that name.

## Documents and evidence

| Page | Purpose |
| --- | --- |
| [AGENTS.md](AGENTS.md) | Session entry point and task routing |
| [HUMAN.md](HUMAN.md) | Operator procedure |
| [Hosted sessions](docs/hosted-sessions.md) | Claude Code, Codex, and Gemini / Antigravity startup and worker roles |
| [Architecture](docs/architecture.md) | Execution paths and data boundaries |
| [Code map](docs/code-map.md) | Source ownership |
| [Limits](docs/limits.md) | Current constraints and proof gaps |
| [Findings](docs/findings.md) | Defects of tools and host traits, with the host of each row |
| [TODO index](TODO/INDEX.md) | Tasks and derived counts |
| [Experiments](experiments/README.md) | Scripts and result captures |
| [Security](SECURITY.md) | Trust boundaries |
| [Third-party notice](THIRD_PARTY.md) | Licenses and carried material |

## Contributions

Read the router and live work record. Work on `main`.
Update the affected task in the same change.
Run the full gate before commit and authorized publication.
The [review checklist](.github/PULL_REQUEST_TEMPLATE.md) defines review evidence.

## License

podvm uses [0BSD](LICENSE). Carried material retains its own terms.
