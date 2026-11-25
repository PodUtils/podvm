#!/bin/sh
# Keep Claude's cloud identity behavior in its hook.
root=${CLAUDE_PROJECT_DIR:-"$(dirname -- "$0")/.."}
if [ "${CLAUDE_CODE_REMOTE:-}" = true ]; then
  sh "$root/scripts/session-start.sh" --global-git-identity
else
  sh "$root/scripts/session-start.sh"
fi
exit 0
