# Maintenance

The procedures of the repository over time, in the form of the page of the
same name in PodUtils/podcrates (TODO/gate.md T-1685). The rules for code,
tests and the shell are in [development](development.md).

## Commits

Make a commit for a coherent change.
Update its task and live record in the same commit.
Use an imperative subject that names the change.
Use a short body that explains the reason when needed.

Write multiline messages through a file.
Read [shell rules](development.md#shell) before passing prose to a process.
Run the full gate before commit.
Record an interrupted checkpoint honestly when the operator stops the work.

The other rules of git (the branch, the identity, the remote) are in
[git](conventions/git.md).

## Releases

A release starts from `.github/workflows/release.yml`; dispatch
`nightly.yml` with `trial=true` on `main` first ([handoff](HANDOFF.md)).

The gate of a beta (T-1949, [Quality](methodology/quality.md), rules 1
and 5). Do each step on the commit that the release takes:

1. The gate run of the commit passes, and `sh scripts/dev/checks.sh`
   passes (it runs `tests/cli_contract.rs`).
2. Dispatch `workloads.yml` on `main`; its run passes.
3. Run the integration tests as uid 1000: build the test binaries of
   `podvm` and `podvm-supervise` (`cargo test --release --no-run
   --message-format=json`) and run each with `setpriv --reuid 1000 --regid
   1000 --clear-groups`, `HOME` and `TMPDIR` in a new directory of mode 777
   under `/var/tmp`, and `CARGO_MANIFEST_DIR=$PWD/crates/podvm`. Each
   failure has a cause in the record that is a fault of the host.
4. An outside pass: an agent with the release binary and `podvm man`
   only, no source, tests the surface for about one hour as uid 1000 and
   writes its findings. Each finding becomes an issue or an entry. No P0
   or P1 finding stays open.
5. Dispatch `nightly.yml` with `trial=true` and `arm.yml` on `main`; both
   pass.
6. The section of the version in `CHANGELOG.md` has a part "Known gaps":
   what the checks prove, and each open fault that a user can meet.
7. Bump the version, then dispatch `release.yml`. Push nothing to `main`
   until the release run ends (call C22).

## Dependencies and vendoring

podvm takes the crates of PodUtils/podcrates by tag (`AGENTS.md`,
section 9). Carried source follows [vendoring](methodology/vendoring.md)
and the [third-party notice](../THIRD_PARTY.md).
