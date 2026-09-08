#!/usr/bin/env bash
# Format an explicit, tracked Markdown file list with the repository policy.
set -euo pipefail

if [[ $# -lt 4 ]]; then
  echo "Usage: $(basename "$0") <mdtablefix> <markdownlint-cli2> -- <file>..." >&2
  exit 64 # EX_USAGE
fi

mdtablefix="$1"
markdownlint="$2"
shift 2
if [[ "$1" != -- ]]; then
  echo "$(basename "$0"): expected -- before the Markdown file list." >&2
  exit 64 # EX_USAGE
fi
shift
markdown_files=("$@")

if [[ ${#markdown_files[@]} -eq 0 ]]; then
  exit 0
fi

: "${MDTABLEFIX_ARGS:=--wrap --renumber --breaks --ellipsis --fences --in-place}"
: "${MDLINT_FIX_ARGS:=--fix --no-globs}"
: "${MDLINT_ROOT_CONFIG:=.markdownlint-cli2.jsonc}"
read -r -a mdtablefix_args <<< "$MDTABLEFIX_ARGS"
read -r -a markdownlint_fix_args <<< "$MDLINT_FIX_ARGS"

has_fix=false
has_no_globs=false
for markdownlint_argument in "${markdownlint_fix_args[@]}"; do
  if [[ "$markdownlint_argument" == --fix ]]; then
    has_fix=true
  fi
  if [[ "$markdownlint_argument" == --no-globs ]]; then
    has_no_globs=true
  fi
done
if [[ "$has_fix" != true || "$has_no_globs" != true ]]; then
  echo "$(basename "$0"): Markdownlint arguments must include --fix and --no-globs." >&2
  exit 64 # EX_USAGE
fi

"$mdtablefix" "${mdtablefix_args[@]}" "${markdown_files[@]}"
"$markdownlint" "${markdownlint_fix_args[@]}" --config "$MDLINT_ROOT_CONFIG" -- \
  "${markdown_files[@]}"
