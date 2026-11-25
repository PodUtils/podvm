#!/bin/sh
# Keep Claude's cloud identity behavior in its hook.
root=${CLAUDE_PROJECT_DIR:-"$(dirname -- "$0")/.."}
set --
if [ "${CLAUDE_CODE_REMOTE:-}" = true ]; then
  set -- --global-git-identity
fi
if ! sh "$root/scripts/session-start.sh" "$@"; then
  echo "session-start: run sh scripts/session-start.sh in the checkout. See AGENTS.md, section 9." >&2
fi
exit 0
