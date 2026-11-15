#!/usr/bin/env bash
# Shared helpers for the build tools the repository's standard needs.
#
# Sourced by scripts/install-build-tools.sh and scripts/check-build-tools.sh so
# both resolve the pinned versions and emit diagnostics identically. Every
# message is prefixed `build-tools:` and written to stderr, so a caller can
# capture a helper's stdout without the diagnostics contaminating it.

set -euo pipefail

# Locate the repository from this file rather than from the working directory,
# so the entry points run correctly when invoked directly as well as through the
# `install-build-tools` and `check-build-tools` targets. `BASH_SOURCE[0]` is
# this file even when sourced, which is what makes the derivation reliable.
BUILD_TOOLS_HELPER_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
BUILD_TOOLS_REPO_ROOT=$(cd -- "$BUILD_TOOLS_HELPER_DIR/.." && pwd)

# Pin files default to their committed locations. An explicit override still
# wins, which is what lets the tests point the scripts at fixtures. `read_pin`
# validates whichever path is selected, so a missing or empty file is reported
# the same way whether it came from a default or an override.
MOLD_VERSION_FILE="${MOLD_VERSION_FILE:-$BUILD_TOOLS_REPO_ROOT/tools/mold/VERSION}"
MOLD_SHA256SUMS_FILE="${MOLD_SHA256SUMS_FILE:-$BUILD_TOOLS_REPO_ROOT/tools/mold/SHA256SUMS}"
RUST_TOOLCHAIN_FILE="${RUST_TOOLCHAIN_FILE:-$BUILD_TOOLS_REPO_ROOT/rust-toolchain.toml}"

# Prefix for the mold installation tree. The Makefile prepends this prefix's
# `bin/` to PATH for every target -- this exact prefix, not a hard-coded
# ~/.local -- so an overridden BUILD_TOOLS_PREFIX is the one that wins PATH
# resolution for both `check-build-tools` and `-fuse-ld=mold`. Invoking these
# scripts outside `make` means arranging that PATH order separately.
BUILD_TOOLS_PREFIX="${BUILD_TOOLS_PREFIX:-$HOME/.local}"

# Emit a diagnostic. Always stderr, so a caller may capture a helper's stdout
# without the diagnostics contaminating the captured value.
note() { printf 'build-tools: %s\n' "$*" >&2; }

# Emit a diagnostic and abort. Used for conditions no caller can recover from,
# such as a missing pin file or an unverifiable download.
fail() {
  printf 'build-tools: %s\n' "$*" >&2
  exit 1
}

# Read a single-line version pin, trimming only its leading and trailing
# whitespace.
#
# A missing, blank, or multi-line pin aborts rather than yielding a version that
# would silently produce a nonsensical download URL. Deleting every whitespace
# character instead would corrupt rather than reject: a stray space would turn
# `1.2 3` into `1.23`, and a second line would concatenate into `1.2.34.5.6`.
# Internal whitespace is likewise rejected, because no pin legitimately contains
# any and silently rewriting one is worse than refusing it.
read_pin() {
  local file=$1 value lines
  [ -f "$file" ] || fail "missing version pin: $file"
  lines=$(grep -c '' <"$file")
  [ "$lines" -le 1 ] || fail "expected one line in version pin: $file, found $lines"
  # `$(...)` strips trailing newlines; the parameter expansions trim spaces and
  # tabs from each end without touching anything between them.
  value=$(cat -- "$file")
  value=${value#"${value%%[![:space:]]*}"}
  value=${value%"${value##*[![:space:]]}"}
  [ -n "$value" ] || fail "empty version pin: $file"
  case $value in
    *[[:space:]]*) fail "version pin contains whitespace: $file" ;;
  esac
  printf '%s' "$value"
}

# The pinned mold release tag, e.g. "2.41.0".
mold_version() { read_pin "$MOLD_VERSION_FILE"; }

# The repository's toolchain, read from `rust-toolchain.toml`.
#
# Deliberately the toolchain the ordinary gates use, not a second pin: the
# parallel frontend the standard relies on is nightly-only.
pinned_toolchain() {
  local file=$RUST_TOOLCHAIN_FILE value
  [ -f "$file" ] || fail "missing version pin: $file"
  value=$(awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ { print $2; exit }' "$file")
  [ -n "$value" ] || fail "no channel found in: $file"
  printf '%s' "$value"
}

# Print the host operating system, failing explicitly when `uname` cannot read it.
# Treating a failed query as a non-Linux host would skip the required linker.
detect_host_os() {
  local host_os
  if ! host_os=$(uname -s); then
    note 'could not determine host operating system using uname -s'
    return 1
  fi
  printf '%s' "$host_os"
}

# mold publishes per-architecture tarballs; map `uname -m` onto those names.
mold_arch() {
  local machine
  machine=$(uname -m)
  case "$machine" in
    x86_64 | amd64) printf 'x86_64' ;;
    aarch64 | arm64) printf 'aarch64' ;;
    *) fail "unsupported architecture for the pinned mold release: $machine" ;;
  esac
}

# The version of the linker executable that `-fuse-ld=mold` resolves through
# PATH, or a non-zero status when that executable cannot be run.
installed_mold_version() {
  # Clang resolves `-fuse-ld=mold` to `ld.mold`, not the `mold` driver. Capture
  # its output first so a failing linker propagates its status instead of being
  # masked by the exit status of a downstream awk.
  local output
  output=$(ld.mold --version 2>/dev/null) || return 1
  printf '%s' "$output" | awk 'NR == 1 { print $2 }'
}

# Print the components listed in the toolchain's multiline TOML array.
pinned_toolchain_components() {
  local file=$RUST_TOOLCHAIN_FILE
  [ -f "$file" ] || fail "missing version pin: $file"
  awk -F '"' '
    /^\[toolchain\][[:space:]]*$/ { in_toolchain = 1; next }
    /^\[/ { in_toolchain = 0 }
    in_toolchain && /^[[:space:]]*components[[:space:]]*=[[:space:]]*\[/ {
      in_components = 1
      next
    }
    in_components && /^[[:space:]]*\]/ { exit }
    in_components && /^[[:space:]]*"/ { print $2 }
  ' "$file"
}
