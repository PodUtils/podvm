#!/bin/sh
# SPDX-License-Identifier: 0BSD
#
# The checks before each push (AGENTS.md, section 12), as one command that
# exits non-zero when any one of them fails. docs/methodology/quality.md:
# on 2026-11-17 a hand-made chain of these checks lost an exit code twice
# (a pipe into `tail`, and a test run that the chain did not test), and a
# commit went out red. Use: sh scripts/dev/checks.sh && git commit ...
#
# PODVM_CHECKS_QUICK=1 leaves out the unit and contract tests (a change of
# the record only).

set -u
cd "$(dirname "$0")/../.."
fail=0
step() {
	name=$1
	shift
	out=$("$@" 2>&1)
	rc=$?
	if [ "$rc" -eq 0 ]; then
		printf 'ok    %s\n' "$name"
	else
		printf 'FAIL  %s (exit %s)\n' "$name" "$rc"
		printf '%s\n' "$out" | tail -15 | sed 's/^/      /'
		fail=1
	fi
}
lock() { flock /tmp/podvm-cargo.lock "$@"; }
step fmt cargo fmt --all --check
step clippy lock cargo clippy --workspace --all-targets -- -D warnings
if [ "${PODVM_CHECKS_QUICK:-0}" != 1 ]; then
	step unit-tests lock cargo test --release -p podvm --bin podvm
	step cli-contract lock cargo test --release -p podvm --test cli_contract
fi
step gate ./target/x86_64-unknown-linux-musl/release/podvm-gate
step check-gate sh scripts/common/check-gate.sh
[ "$fail" -eq 0 ] && echo "all checks pass" || echo "a check failed: do not push"
exit "$fail"
