# Netsuke v0.1.0 release-admission canary

This branch is one of Netsuke's three v0.1.0 release-admission canaries.
Netsuke's release workflow checks this branch out at a pinned commit, builds
the exact Netsuke release candidate, and runs the `Netsukefile` gates below. A
failure blocks publication of the candidate only when it exposes a defect in
behaviour that v0.1.0 claims to support.

## What the canary exercises

The Linux leg runs a mixed-tool slice: Rust formatting, rustdoc, Clippy,
Whitaker, the Rust tests, the Python helper tests through uv, Markdown lint,
and the generated spelling configuration (`check-fmt`, `lint`, `test`,
`markdownlint`, and `generated-config`).

A normal GitHub-hosted `windows-latest` leg runs `powershell-wrapper-validate`,
which invokes the native PowerShell helper with direct `pwsh -File` argument
execution. The Windows slice exercises that helper rather than the
multi-command Linux actions; the broader legacy shell contract is release work
tracked separately in `leynos/netsuke#599`.

`generated-config` renders `typos.toml` from the shared spelling dictionary at
the commit that produced the committed file, then requires no change. Rendering
from the dictionary's moving branch would fail whenever the shared dictionary
changed, which says nothing about Netsuke or this repository's generator.

## Retained boundaries

- Tool-heavy implementation stays in the existing helpers:
  `scripts/generate_typos_config.py`, the Python test suite, and
  `scripts/validate_powershell_wrapper.ps1`. The `Netsukefile` selects those
  gates with direct commands rather than turning the helpers into embedded
  shell scripts.
- The explicit empty `targets: []` remains because v0.1.0 requires the
  top-level key even when the manifest is action-only.
- The Makefile remains for the broader contributor and publication workflow,
  including the spelling helpers' own coverage gates and `make spelling`.
