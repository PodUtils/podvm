#!/bin/sh
# Set the Git identity and start the two podcrates watchers for the lead checkout.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
cd "$root"

git config --local user.name Azathothas
git config --local user.email ajamx101@gmail.com
git config --local core.hooksPath .githooks
git config --local commit.gpgsign false

if [ "${1:-}" = --global-git-identity ]; then
  git config --global user.name Azathothas
  git config --global user.email ajamx101@gmail.com
fi

git_dir=$(git rev-parse --absolute-git-dir)
common_dir=$(git rev-parse --path-format=absolute --git-common-dir)
if [ "$git_dir" != "$common_dir" ]; then
  echo "session-start: the lead checkout owns the watchers"
  exit 0
fi

sync_main() {
  changes=$(git status --porcelain) || return 1
  if [ -n "$changes" ]; then
    echo "session-start: keeping the checkout with local changes"
    return 0
  fi
  upstream=refs/remotes/origin/main
  if ! GIT_TERMINAL_PROMPT=0 timeout 60 git fetch -q origin "+refs/heads/main:$upstream" </dev/null; then
    echo "session-start: could not fetch origin/main" >&2
    return 1
  fi
  if [ "$(git symbolic-ref --short -q HEAD || :)" != main ]; then
    if ! git merge-base --is-ancestor HEAD "$upstream"; then
      echo "session-start: keeping the checkout with local commits or incomplete history" >&2
      return 0
    fi
    if git show-ref --verify --quiet refs/heads/main; then
      git switch -q main || return 1
    else
      git switch -q -c main --track origin/main || return 1
    fi
  fi
  if ! git merge -q --ff-only "$upstream"; then
    echo "session-start: local main could not fast-forward" >&2
    return 1
  fi
}

umask 077
# Watcher bytes and checksums come from podcrates v0.83.0.
pin=5fd6fb7e3495d5e9a591e36c5752e268309f6e4a
watcher_dir=.work/podcrates-agent/$pin
mkdir -p "$watcher_dir"
exec 9>.work/session-start.lock
if ! flock -w 30 9; then
  echo "session-start: could not lock .work/session-start.lock" >&2
  exit 1
fi

fetch_watcher() (
  file=$1
  checksum=$2
  cd "$watcher_dir"
  if printf '%s  %s\n' "$checksum" "$file" | sha256sum --check --status 2>/dev/null; then
    exit 0
  fi
  stage=$(mktemp "$file.XXXXXXXX")
  trap 'rm -f -- "$stage"' EXIT
  url=https://raw.githubusercontent.com/PodUtils/podcrates/$pin/scripts/agent/$file
  if ! curl -qfsSL --proto '=https' --proto-redir '=https' --max-time 60 -o "$stage" "$url" </dev/null; then
    printf 'session-start: could not fetch %s from podcrates at %s\n' "$file" "$pin" >&2
    exit 1
  fi
  if ! printf '%s  %s\n' "$checksum" "$stage" | sha256sum --check --status; then
    printf 'session-start: %s failed the SHA-256 check\n' "$file" >&2
    exit 1
  fi
  mv -f -- "$stage" "$file"
)

status=0
sync_main || status=1
fetch_watcher watch.py 14702c073ba50d68572531f29dbbc10b366b25216ef69658da385ac60d4e9391
fetch_watcher watch_test.py ff8d8a3d6ebbbde57135e174ee93b0330c237f611f479451f76e1173280a7568
fetch_watcher runs.py 80754a64ba6281d7b0e03d59932bf6a7fe93aaab956bda907315854d81d13d36
if ! timeout 20 python3 -I "$watcher_dir/watch.py" --self-test </dev/null >/dev/null ||
   ! timeout 20 python3 -I "$watcher_dir/runs.py" --self-test </dev/null >/dev/null; then
  echo "session-start: a podcrates watcher did not pass its self-test" >&2
  exit 1
fi

start_watcher() {
  name=$1
  shift
  lock=.work/$name.lock
  log=.work/$name.log
  : >>"$lock" || return 1
  if ! flock -n "$lock" true; then
    printf 'session-start: %s runs already. Log: %s\n' "$name" "$log"
    return 0
  fi
  if command -v setsid >/dev/null 2>&1; then
    setsid -f flock -nF "$lock" python3 -I -u "$watcher_dir/$name.py" "$@" \
      >>"$log" 2>&1 </dev/null 9>&- || return 1
  else
    nohup flock -nF "$lock" python3 -I -u "$watcher_dir/$name.py" "$@" \
      >>"$log" 2>&1 </dev/null 9>&- &
  fi
  for attempt in 1 2 3 4 5; do
    sleep 1
    if ! flock -n "$lock" true; then
      printf 'session-start: %s runs. Log: %s\n' "$name" "$log"
      return 0
    fi
  done
  printf 'session-start: %s did not start. See %s\n' "$name" "$log" >&2
  return 1
}

start_watcher watch podvm --state .work/agent || status=1
start_watcher runs --repo PodUtils/podvm --branch main --every 120 || status=1
exit "$status"
