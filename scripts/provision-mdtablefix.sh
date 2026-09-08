#!/usr/bin/env bash
# Install the repository's pinned mdtablefix release into an isolated prefix.
set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
  echo "Usage: $(basename "$0") <cargo> <version> <absolute-bin-dir> [executable-suffix]" >&2
  exit 64 # EX_USAGE
fi

cargo_command="$1"
version="$2"
bin_dir="$3"
executable_suffix="${4:-}"
if [[ "$executable_suffix" != "" && "$executable_suffix" != .exe ]]; then
  echo "$(basename "$0"): executable suffix must be empty or '.exe'." >&2
  exit 64 # EX_USAGE
fi
executable="$bin_dir/mdtablefix$executable_suffix"
expected_version="mdtablefix $version"

if [[ ! "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  echo "$(basename "$0"): version must have three numeric components." >&2
  exit 64 # EX_USAGE
fi
if [[ "$bin_dir" != /* || "$bin_dir" == *$'\n'* || "$bin_dir" == *$'\r'* ]]; then
  echo "$(basename "$0"): bin directory must be an absolute single-line path." >&2
  exit 64 # EX_USAGE
fi
if [[ "/$bin_dir/" == *"/../"* ]]; then
  echo "$(basename "$0"): bin directory must not contain parent components." >&2
  exit 64 # EX_USAGE
fi

reported_version=
if [[ -x "$executable" ]]; then
  reported_version="$("$executable" --version 2>/dev/null | head -n 1 | tr -d '\r' || true)"
fi
if [[ "$reported_version" == "$expected_version" ]]; then
  exit 0
fi

if ! "$cargo_command" binstall -V >/dev/null 2>&1; then
  echo "$(basename "$0"): cargo-binstall is required to provision mdtablefix." >&2
  exit 127
fi

mkdir -p -- "$bin_dir"
"$cargo_command" binstall \
  --no-confirm \
  --locked \
  --disable-strategies compile \
  --disable-telemetry \
  --install-path "$bin_dir" \
  "mdtablefix@$version"

if [[ ! -x "$executable" ]]; then
  echo "$(basename "$0"): mdtablefix was not installed at '$executable'." >&2
  exit 1
fi
actual_version="$("$executable" --version 2>/dev/null | head -n 1 | tr -d '\r')"
if [[ "$actual_version" != "$expected_version" ]]; then
  echo "$(basename "$0"): expected '$expected_version', got '${actual_version:-nothing}'." >&2
  exit 1
fi
