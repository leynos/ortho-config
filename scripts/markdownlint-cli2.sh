#!/usr/bin/env bash
# Run the Markdown formatter version pinned by the repository.
set -euo pipefail

: "${BUN:=bun}"
: "${MARKDOWNLINT_VERSION:=0.23.2}"
exec "$BUN" x "markdownlint-cli2@${MARKDOWNLINT_VERSION}" "$@"
