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

# Keep a dirty checkout and all local commits.
branch=$(git symbolic-ref --short -q HEAD || :)
if [ "$branch" != main ] && status=$(git status --porcelain) && [ -z "$status" ]; then
  if git show-ref --verify --quiet refs/heads/main; then
    git checkout -q main
  else
    timeout 60 git fetch -q origin main </dev/null
    if git show-ref --verify --quiet refs/remotes/origin/main; then
      git checkout -q -b main origin/main
    else
      git checkout -q -b main FETCH_HEAD
    fi
  fi
fi

umask 077
watcher_dir=.work/podcrates-agent
mkdir -p "$watcher_dir"
exec 9>.work/session-start.lock
if ! flock -w 30 9; then
  echo "session-start: could not lock .work/session-start.lock" >&2
  exit 1
fi

# Both watchers come from the pinned podcrates v0.83.0 revision.
pin=5fd6fb7e3495d5e9a591e36c5752e268309f6e4a
if [ "$(cat "$watcher_dir/pin" 2>/dev/null || :)" != "$pin" ]; then
  rm -f -- "$watcher_dir/watch.py" "$watcher_dir/watch_test.py" "$watcher_dir/runs.py"
fi
for file in watch.py watch_test.py runs.py; do
  if [ ! -s "$watcher_dir/$file" ]; then
    url="https://raw.githubusercontent.com/PodUtils/podcrates/$pin/scripts/agent/$file"
    if ! curl -qfsSL --max-time 60 -o "$watcher_dir/$file.part" "$url" </dev/null; then
      printf 'session-start: could not fetch %s from podcrates at %s\n' "$file" "$pin" >&2
      exit 1
    fi
    mv -- "$watcher_dir/$file.part" "$watcher_dir/$file"
  fi
done
printf '%s\n' "$pin" >"$watcher_dir/pin"
if ! timeout 20 python3 -I "$watcher_dir/watch.py" --self-test </dev/null >/dev/null ||
   ! timeout 20 python3 -I "$watcher_dir/runs.py" --self-test </dev/null >/dev/null; then
  echo "session-start: a podcrates watcher did not pass its self-test" >&2
  exit 1
fi

watcher_is_running() {
  script=$1
  shift
  for pid in $(pgrep -fx "python3 -I \.work/podcrates-agent/${script}\.py $*"); do
    if [ "$(readlink "/proc/$pid/cwd" 2>/dev/null)" = "$root" ]; then
      return 0
    fi
  done
  return 1
}

start_watcher() {
  name=$1
  shift
  log=.work/$name.log
  if watcher_is_running "$name" "$@"; then
    printf 'session-start: %s runs already. Log: %s\n' "$name" "$log"
    return 0
  fi
  if command -v setsid >/dev/null 2>&1; then
    setsid -f python3 -I "$watcher_dir/$name.py" "$@" >>"$log" 2>&1 </dev/null 9>&-
  else
    nohup python3 -I "$watcher_dir/$name.py" "$@" >>"$log" 2>&1 </dev/null 9>&- &
  fi
  for attempt in 1 2 3 4 5; do
    if watcher_is_running "$name" "$@"; then
      printf 'session-start: %s runs. Log: %s\n' "$name" "$log"
      return 0
    fi
    sleep 1
  done
  printf 'session-start: %s did not start. See %s\n' "$name" "$log" >&2
  return 1
}

status=0
start_watcher watch podvm --state .work/agent || status=1
start_watcher runs --repo PodUtils/podvm --branch main --every 120 || status=1
exit "$status"
