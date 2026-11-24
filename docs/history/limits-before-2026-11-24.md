# Text of docs/limits.md and docs/architecture.md before 2026-11-24

Verbatim: statements that the code or the record made false, and dated narrative, removed from the live pages on 2026-11-24. The live pages give the corrected text.

## Release assets of the SSH helpers (docs/limits.md, "SSH")

Each release target since `v0.1.0-beta.10` carries the helper archive
(T-1405 in [podssh](../../TODO/podssh.md)). The `v0.1.0-beta.9` assets contain
the CLI only.

## The interposer pair (docs/limits.md, "Host and payload"; docs/architecture.md, "Interposer")

False since T-1721 (an interposer for aarch64):

- The embedded interposer pair is x86_64, and only the x86_64 binary embeds it (T-1721). Other payload architectures need another execution path.
- The pair targets x86_64. The build embeds it only into an x86_64 binary;
  the aarch64 binary embeds no object and refuses a dynamic payload by name (T-1721).

## The machine tier (docs/architecture.md, "Machine execution")

False since T-1750 (`exec` into a guest of `run -d`) and T-1890 (the arm64 guest):

- The guest is linux/amd64, and an image of another
  architecture is refused before the boot (T-1889). `create` and `exec`
  assess the machine legs and exit 125.

## The Windows guest and the KVM deferral (docs/limits.md, "Guests")

False since T-1861 and T-1865 (the media and the NIC of a desktop guest):

The owner lifted the KVM deferral on 2026-11-09 ([decisions](../decisions.md)).
A session can start a KVM guest where `/dev/kvm` exists, with a timeout,
and no emulator may stay after the run. The cloud host has no `/dev/kvm`,
so a cloud session runs guests under TCG. `kvm.yml` (T-1682) runs the KVM
leg on a hosted runner.

The Windows guest has no network in the current driver.
Today the operator gives the disk image (`PODVM_WINDOWS_IMAGE`, or
`windows fetch --url`). The owner accepts the vendor terms
([decisions](../decisions.md), "Windows media"), so podvm will fetch and
verify the pinned media itself; the work is in the record.
T-1112 records the TCG, KVM, and DOS proofs separately.
The 2026-09-30 KVM runs on the Windows base failed at setup or command
shutdown. An earlier KVM success does not establish current reliability.
The operator removed the ReactOS clause from T-1112 on 2026-10-02.

## The init on the namespace rung (docs/limits.md, "The init of an image")

A run with the init on the namespace rung [...] refuses `--podvm-confine`,
`--ulimit`, `--security-opt no-new-privileges` and `--device` by name with
125 (T-2017): the supervisor of the init starts the init and the command
with no entry of pod-enter, and before 2026-11-20 it dropped each of
them with no line.
