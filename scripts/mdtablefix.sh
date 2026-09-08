#!/usr/bin/env bash
# Provision and run the repository-pinned mdtablefix release.
set -euo pipefail

repository_root="$(cd -- "$(dirname -- "$0")/.." && pwd -P)"
: "${CARGO:=cargo}"
: "${MDTABLEFIX_VERSION:=0.5.1}"
: "${MDTABLEFIX_BIN_DIR:=$repository_root/scripts/.tools/mdtablefix/$MDTABLEFIX_VERSION}"

bin_dir="$MDTABLEFIX_BIN_DIR"
if [[ "$bin_dir" =~ ^[A-Za-z]:[\\/] ]]; then
  if ! command -v cygpath >/dev/null 2>&1; then
    echo "$(basename "$0"): cygpath is required for a native Windows tool path." >&2
    exit 1
  fi
  bin_dir="$(cygpath -u "$bin_dir")"
elif [[ "$bin_dir" != /* ]]; then
  echo "$(basename "$0"): tool directory must be absolute." >&2
  exit 64 # EX_USAGE
fi

executable_suffix=
case "${RUNNER_OS:-}/${OSTYPE:-}" in
  Windows/*|*/msys*|*/cygwin*) executable_suffix=.exe ;;
esac

"$repository_root/scripts/provision-mdtablefix.sh" \
  "$CARGO" "$MDTABLEFIX_VERSION" "$bin_dir" "$executable_suffix"
exec "$bin_dir/mdtablefix$executable_suffix" "$@"
