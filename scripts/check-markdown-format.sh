#!/usr/bin/env bash
# Verify that Markdown sources already match the canonical formatter output.
#
# `mdtablefix` has no check-only mode, so the checker formats staged copies and
# compares them with their sources without modifying tracked files. The
# comparison accepts LF or CRLF because Git may check text out with either.
set -euo pipefail

if [[ ${1:-} == --formatter ]]; then
  if [[ $# -lt 2 || -z ${2:-} ]]; then
    echo "Usage: $(basename "$0") [--formatter <path>] [--] <file>..." >&2
    exit 64 # EX_USAGE
  fi
  MDTABLEFIX="$2"
  shift 2
fi
if [[ ${1:-} == -- ]]; then
  shift
fi
if [[ $# -eq 0 ]]; then
  echo "Usage: $(basename "$0") [--formatter <path>] [--] <file>..." >&2
  exit 64 # EX_USAGE
fi

mdtablefix="${MDTABLEFIX:-mdtablefix}"
if ! command -v "$mdtablefix" >/dev/null 2>&1; then
  echo "$(basename "$0"): '$mdtablefix' is not installed or not on PATH." >&2
  exit 127
fi

staged_directory="$(mktemp -d)"
trap 'rm -rf "$staged_directory"' EXIT

original_files=()
staged_files=()
for file in "$@"; do
  if [[ -L "$file" ]]; then
    echo "$(basename "$0"): refusing to follow Markdown symlink '$file'." >&2
    exit 1
  fi
  if [[ ! -f "$file" ]]; then
    echo "$(basename "$0"): '$file' is not a regular Markdown file." >&2
    exit 1
  fi
  staged_file="$staged_directory/${#original_files[@]}.md"
  cp -- "$file" "$staged_file"
  original_files+=("$file")
  staged_files+=("$staged_file")
done

"$mdtablefix" --in-place --wrap --renumber --breaks --ellipsis --fences \
  "${staged_files[@]}"

unformatted=()
for index in "${!original_files[@]}"; do
  original_file="${original_files[$index]}"
  staged_file="${staged_files[$index]}"
  if ! cmp -s "$staged_file" "$original_file" \
    && ! sed $'s/$/\r/' "$staged_file" | cmp -s - "$original_file"; then
    unformatted+=("$original_file")
  fi
done

if [[ ${#unformatted[@]} -gt 0 ]]; then
  echo "The following Markdown files are not formatted; run 'make fmt':" >&2
  printf '  %s\n' "${unformatted[@]}" >&2
  exit 1
fi
