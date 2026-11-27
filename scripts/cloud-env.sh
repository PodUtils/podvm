# SPDX-License-Identifier: 0BSD
# shellcheck shell=sh
# Source from the checkout root in each hosted shell.
if [ ! -f scripts/cloud-env.sh ] || [ ! -f rust-toolchain.toml ]; then
  echo "cloud-env: source this file from the podvm checkout" >&2
  return 1
fi

podvm_cargo_bin=${CARGO_HOME:-$HOME/.cargo}/bin
case "$podvm_cargo_bin" in /*) ;; *) podvm_cargo_bin=$PWD/$podvm_cargo_bin ;; esac
case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) PATH=$HOME/.local/bin:$PATH ;; esac
case ":$PATH:" in *":$podvm_cargo_bin:"*) ;; *) PATH=$podvm_cargo_bin:$PATH ;; esac
unset podvm_cargo_bin
if [ -z "${CARGO_BUILD_JOBS:-}" ]; then
  podvm_cpus=$(nproc 2>/dev/null || printf 4)
  case "$podvm_cpus" in
    [1-3]) CARGO_BUILD_JOBS=$podvm_cpus ;;
    *) CARGO_BUILD_JOBS=4 ;;
  esac
  unset podvm_cpus
fi
: "${CARGO_NET_GIT_FETCH_WITH_CLI:=true}"
export PATH CARGO_BUILD_JOBS CARGO_NET_GIT_FETCH_WITH_CLI
