**Resume**

Start from the checkout. The current work and its proof are in tracked files.

1. Read [AGENTS.md](../AGENTS.md) and follow section 8. Claude Code starts
   the shared script through its hook. In Codex, run
   `sh scripts/session-start.sh` from the checkout. In Antigravity / Gemini
   Cloud, `.agents/environment.json` runs the setup and the lead starts
   both watchers as daemon tasks (`--run-watcher watch` and `--run-watcher runs`).
2. Read the [handoff](../docs/HANDOFF.md) when it exists, then
   [PROGRESS](PROGRESS.md) and [the owner's decisions](../docs/decisions.md).
   Check recent CI and issues. Use PROGRESS for the work order and
   [INDEX](INDEX.md) for each entry and its proof.
3. Run the setup and record gate in
   [Operator procedure](../HUMAN.md#cloud-or-linux-session).
   You can read the current task while the record tools build.
   Build the product when a task needs it.

[Hosted sessions](../docs/hosted-sessions.md) gives the worker roles,
watcher recovery, and host checks.
