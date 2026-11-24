#!/bin/sh
# Set the git identity, the hooks path and the branch for each session.
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
git config user.name Azathothas
git config user.email ajamx101@gmail.com
# The commit-msg hook refuses agent attribution lines.
git config core.hooksPath .githooks
# No commit signature. The signing key of a cloud session belongs to the
# identity of the platform, not to the author above, so GitHub shows a
# signed commit as unverified, and the stop hook of the session asks for a
# rewrite of the author. A local setting overrides the global one
# (docs/decisions.md, "Commit signing").
git config commit.gpgsign false
# A cloud session owns its global identity; a local session keeps the
# owner's own global settings.
if [ "${CLAUDE_CODE_REMOTE:-}" = true ]; then
  git config --global user.name Azathothas
  git config --global user.email ajamx101@gmail.com
fi
# Work on main. Switch only from a clean tree, and never reset a local
# main: it can hold commits that origin/main does not have.
cur=$(git symbolic-ref --short -q HEAD)
if [ "$cur" != main ] && [ -z "$(git status --porcelain 2>/dev/null)" ]; then
  git fetch -q origin main 2>/dev/null
  if git rev-parse -q --verify refs/heads/main >/dev/null; then
    git checkout -q main 2>/dev/null || true
  elif git rev-parse -q --verify refs/remotes/origin/main >/dev/null; then
    git checkout -q -b main origin/main 2>/dev/null || true
  fi
fi
# The owner's ruling "Watchers" (docs/decisions.md, 2026-11-24; AGENTS.md,
# sections 8 and 9): the watcher of the organization and the watcher of the
# Actions runs start with each session, as background processes with a log.
# The session reads the logs; it does not wait for a notice of the harness.
# A watcher that runs already is not started again. No step here can fail
# the session.
# Both are the scripts of podcrates at a pinned tag (v0.83.0), each with its
# self-test.
mkdir -p .work/podcrates-agent
pin=5fd6fb7e3495d5e9a591e36c5752e268309f6e4a
if [ "$(cat .work/podcrates-agent/pin 2>/dev/null)" != "$pin" ]; then
  rm -f .work/podcrates-agent/watch.py .work/podcrates-agent/watch_test.py .work/podcrates-agent/runs.py
fi
ok=1
for f in watch.py watch_test.py runs.py; do
  [ -s ".work/podcrates-agent/$f" ] && continue
  curl -qfsSL --max-time 60 -o ".work/podcrates-agent/$f.part" \
    "https://raw.githubusercontent.com/PodUtils/podcrates/$pin/scripts/agent/$f" &&
    mv ".work/podcrates-agent/$f.part" ".work/podcrates-agent/$f" || ok=0
done
[ "$ok" = 1 ] && echo "$pin" >.work/podcrates-agent/pin
if [ "$ok" = 1 ] && python3 -I .work/podcrates-agent/watch.py --self-test >/dev/null 2>&1 &&
  python3 -I .work/podcrates-agent/runs.py --self-test >/dev/null 2>&1; then
  tested=1
else
  tested=0
  echo "session-start: the watchers of podcrates did not pass their fetch or self-test: start them by AGENTS.md, section 9"
fi
if ! pgrep -f "watch.py podvm --state .work/agent" >/dev/null 2>&1; then
  if [ "$tested" = 1 ]; then
    nohup python3 -I .work/podcrates-agent/watch.py podvm --state .work/agent \
      >>.work/watch.log 2>&1 </dev/null &
    echo "session-start: the watcher of the organization runs; its log is .work/watch.log"
  else
    echo "session-start: the watcher of the organization did not start"
  fi
else
  echo "session-start: the watcher of the organization runs already; its log is .work/watch.log"
fi
if ! pgrep -f "runs.py --repo PodUtils/podvm --branch main --every" >/dev/null 2>&1; then
  if [ "$tested" = 1 ]; then
    nohup python3 -I .work/podcrates-agent/runs.py --repo PodUtils/podvm --branch main --every 120 \
      >>.work/runs.log 2>&1 </dev/null &
    echo "session-start: the watcher of the Actions runs runs; its log is .work/runs.log"
  else
    echo "session-start: the watcher of the Actions runs did not start"
  fi
else
  echo "session-start: the watcher of the Actions runs runs already; its log is .work/runs.log"
fi
exit 0
