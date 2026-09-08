#!/usr/bin/env bash
# Run a Markdown command over the repository's tracked, regular Markdown files.
#
# The command receives `--` before its file arguments. Callers must accept that
# separator so a file name cannot be mistaken for an option.
set -euo pipefail

if [[ $# -eq 0 ]]; then
  echo "Usage: $(basename "$0") <command> [args...]" >&2
  exit 64 # EX_USAGE
fi

if ! repository_root="$(git rev-parse --show-toplevel)"; then
  echo "$(basename "$0"): must run inside a Git worktree." >&2
  exit 1
fi

cd "$repository_root"
file_list="$(mktemp)"
trap 'rm -f "$file_list"' EXIT

if ! git ls-files -z -- '*.md' '*.markdown' '*.mdx' > "$file_list"; then
  echo "$(basename "$0"): could not discover tracked Markdown files." >&2
  exit 1
fi

markdown_files=()
while IFS= read -r -d '' markdown_file; do
  # A symlink can point beyond the checkout or duplicate another document.
  # Formatting only regular tracked files keeps the selected scope explicit.
  if [[ -L "$markdown_file" ]]; then
    continue
  fi
  if [[ ! -f "$markdown_file" ]]; then
    echo "$(basename "$0"): tracked Markdown path is not a regular file: $markdown_file" >&2
    exit 1
  fi
  markdown_files+=("$markdown_file")
done < "$file_list"

if [[ ${#markdown_files[@]} -eq 0 ]]; then
  exit 0
fi

"$@" -- "${markdown_files[@]}"
