#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXAMPLE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
WORKDIR="$(mktemp -d)"

cleanup() {
  rm -rf "${WORKDIR}"
}
trap cleanup EXIT

prepare_config() {
  local sample="$1"
  rm -f "${WORKDIR}/.hello_world.toml"
  for file in "${EXAMPLE_ROOT}/config/"*.toml; do
    cp "${file}" "${WORKDIR}/$(basename "${file}")"
  done
  cp "${WORKDIR}/${sample}" "${WORKDIR}/.hello_world.toml"
}

run_hello() {
  local description="$1"
  shift
  printf '==> %s\n' "${description}"
  printf '    $ %s\n' "$*"
  (
    cd "${WORKDIR}"
    "$@"
  )
  printf '\n'
}

prepare_config baseline.toml
run_hello "Running greet with baseline config defaults" \
  cargo run -p hello_world --manifest-path "${EXAMPLE_ROOT}/Cargo.toml" --quiet -- greet
run_hello "Running take-leave with baseline config defaults" \
  cargo run -p hello_world --manifest-path "${EXAMPLE_ROOT}/Cargo.toml" --quiet -- take-leave

run_hello "Overriding recipient via HELLO_WORLD_RECIPIENT" \
  env HELLO_WORLD_RECIPIENT="Environment override" \
    cargo run -p hello_world --manifest-path "${EXAMPLE_ROOT}/Cargo.toml" --quiet -- greet

run_hello "Overriding salutations via CLI arguments" \
  cargo run -p hello_world --manifest-path "${EXAMPLE_ROOT}/Cargo.toml" --quiet -- \
    -s "CLI hello" -r "CLI crew" greet

prepare_config overrides.toml
run_hello "Running greet with overrides.toml extending baseline" \
  cargo run -p hello_world --manifest-path "${EXAMPLE_ROOT}/Cargo.toml" --quiet -- greet
