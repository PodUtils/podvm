# Hosted sessions

Use Claude Code or Codex for hosted repository work. Both use [AGENTS.md](../AGENTS.md).

## Start the lead session

From the checkout, run `sh scripts/session-start.sh` in Codex. Claude
Code calls the same script through its `SessionStart` hook. The script
sets the repository Git identity and hooks path. It also starts the
organization and Actions watchers in the lead checkout.

Use the [cold start](../TODO/RESUME.md) to find the current work.
Follow the start order in `AGENTS.md`, section 8. Use the toolchain setup
in [Operator procedure](../HUMAN.md#cloud-or-linux-session), including the
`PATH` updates for a fresh shell. If the checkout is shallow, unshallow
it before you read a history range.

## Lead and workers

Select the model and reasoning effort for each assignment. Use a fresh
brief for each worker.

| Assignment | Claude Code | Codex |
| --- | --- | --- |
| Implementation and other work that needs deeper reasoning | Opus 5.5, medium or high | GPT-6.2 Sol, high or xhigh |
| Housekeeping, QA, and other work | Sonnet 5.5, high | GPT-6.2 Sol, medium |
| Hardest or apparently impossible tasks, and advisors | Fable 5.2, high | GPT-6.2 Astra, high |

The lead gives independent source tasks to parallel workers. Each worker
uses a local branch and worktree under `.claude/worktrees/`. Give each
worker its task, file scope, proof, and shared resource limits. Use the
[blind QA procedure](methodology/quality.md#blind-qa) for a tester instead
of a source worktree.

The lead schedules one Cargo build at a time and one guest run at a time
across worktrees of this repository. Each guest run needs a timeout and
a cleanup check. Workers report their changes, evidence, and any remaining
clauses. The lead checks for conflicts, integrates on `main`, and runs the
checks on the combined changes. The lead owns the task record, authorized
publication, and follow-up checks.

The lead reads `.work/watch.log` for organization activity and COORD
messages, and `.work/runs.log` for Actions. Check the logs between steps
and after each push or dispatch. Restart a watcher that stops. Follow the
fallback in `AGENTS.md`, section 9, when the session cannot dispatch a
workflow.

## Check the current host

Run `sh scripts/cloud-setup.sh` for the initial host report. Check disk
space, the cgroup layout and delegation, and Linux permissions before you
select a test. Check the proxy policy on this host. Keep network calls
and child waits bounded and non-interactive.

Use `podvm probe` once a binary is available. Check whether this caller
can open `/dev/kvm` before a KVM proof. If the host provides QEMU but lacks
usable KVM, use TCG for a proof that permits emulation. Use `kvm.yml` for
proof that requires KVM. See [container procedures](containers.md) for
lane setup and cleanup.

Save proof and handoff in tracked files. A cold reader must be able to
repeat the proof from the recorded revision and host conditions. A prior
host result applies to the host that produced it.

[Sessions](methodology/sessions.md) owns the general procedure.
[Git](conventions/git.md) owns identity and publication.
