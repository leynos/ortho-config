# Repository layout

This document explains the shape of the OrthoConfig repository for contributors
who need to find source code, tests, fixtures, generated artefacts, plans, and
long-lived references quickly.

The tree below is intentionally compact. It shows the authoritative top-level
layout and the critical subdirectories new contributors usually need first.

```plaintext
.
├── .github/
│   └── workflows/
├── cargo-orthohelp/
│   ├── src/
│   └── tests/
├── docs/
│   ├── archive/
│   └── execplans/
├── examples/
│   └── hello_world/
├── ortho_config/
│   ├── locales/
│   ├── src/
│   └── tests/
├── ortho_config_macros/
│   └── src/
├── scripts/
│   └── tests/
├── target/
├── test_helpers/
│   └── src/
└── tests/
    └── fixtures/
```

Diagram 1: compact repository tree.

## Top-level paths

Table 1 describes the repository paths that define the workspace shape.

| Path                    | Responsibility and conventions                                                                                                                                                                                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `.github/`              | GitHub automation. Workflow definitions live under `.github/workflows/`; keep workflow changes small and validate YAML when possible.                                                                                                                                                      |
| `cargo-orthohelp/`      | Workspace binary crate for generating localized documentation artefacts from `OrthoConfigDocs` metadata. Source code lives in `cargo-orthohelp/src/`; integration and golden tests live in `cargo-orthohelp/tests/`.                                                                       |
| `docs/`                 | Long-lived project documentation. Use `docs/contents.md` as the index, `docs/users-guide.md` as the user's guide, `docs/developers-guide.md` as the developer's guide, `docs/design.md` and focused design documents for architecture, and `docs/roadmap.md` for active delivery planning. |
| `docs/archive/`         | Historical documentation that should remain available but is no longer the active source for future work. The archived v0.8.0 roadmap lives here.                                                                                                                                          |
| `docs/execplans/`       | Living and historical execution plans for substantial tasks. Update an active ExecPlan as work proceeds so progress, decisions, and validation survive context loss.                                                                                                                       |
| `examples/hello_world/` | Example application crate used to demonstrate layered configuration, subcommands, localization, generated docs metadata, and behavioural coverage.                                                                                                                                         |
| `ortho_config/`         | Core runtime crate. This is where public configuration loading, merge logic, discovery, error types, documentation IR types, localization support, and user-facing APIs live.                                                                                                              |
| `ortho_config/locales/` | Embedded Fluent resources used by localization support. Keep locale identifiers and message identifiers aligned with generated clap and documentation metadata.                                                                                                                            |
| `ortho_config/src/`     | Runtime source modules for configuration discovery, file parsing, merge behaviour, docs metadata, error handling, localization, and supporting utilities. Every module should start with a module-level `//!` comment.                                                                     |
| `ortho_config/tests/`   | Core crate integration and behavioural tests. Use shared helpers rather than direct environment mutation.                                                                                                                                                                                  |
| `ortho_config_macros/`  | Procedural macro crate for deriving OrthoConfig-related implementations. Macro parsing, build, and generation code lives under `ortho_config_macros/src/`.                                                                                                                                 |
| `scripts/`              | Python and shell-adjacent maintenance scripts, including shared spelling-policy cache management, the renderer, and the `generate_typos_config.py` entrypoint. Focused script tests live under `scripts/tests/`.                                                                           |
| `target/`               | Cargo build output, generated documentation, trybuild artefacts, and temporary build products. Do not edit or commit files from this directory.                                                                                                                                            |
| `test_helpers/`         | Shared test helper crate. Put cross-crate fixtures, environment guards, and reusable assertions here rather than duplicating test infrastructure.                                                                                                                                          |
| `tests/fixtures/`       | Workspace-level fixture crates and data used by integration, documentation, and generator tests. Fixtures should be minimal but representative.                                                                                                                                            |

## Important root files

Table 2 lists root files that carry project policy or workspace behaviour.

| Path                       | Responsibility and conventions                                                                                                                   |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `AGENTS.md`                | Repository instructions for automated coding agents and contributors working through the same workflow. Read it before making changes.           |
| `Cargo.toml`               | Workspace manifest. Keep dependency and member changes deliberate and consistent with the dependency policy in `AGENTS.md`.                      |
| `Cargo.lock`               | Workspace lock file. For rebase conflicts, prefer the `main` branch version first, then rebuild the lock file through Cargo.                     |
| `Makefile`                 | Canonical local task runner. Prefer Make targets over raw commands for formatting, linting, tests, documentation checks, and release validation. |
| `.markdownlint-cli2.jsonc` | Markdown lint configuration. Documentation changes should pass `make markdownlint`.                                                              |
| `typos.local.toml`         | Narrow repository-specific spelling policy merged with the shared estate dictionary.                                                             |
| `typos.toml`               | Generated en-GB-oxendict spelling configuration for the `typos` gate. Regenerate with `scripts/generate_typos_config.py`; do not hand-edit.      |
| `clippy.toml`              | Clippy configuration. Lint suppressions should be rare, tightly scoped, and justified.                                                           |
| `rust-toolchain.toml`      | Rust toolchain pin. Do not change it incidentally while working on unrelated tasks.                                                              |
| `CHANGELOG.md`             | Release history. Update it when user-visible behaviour changes or migration notes are required.                                                  |
| `README.md`                | Repository-level introduction. Keep deep design and contributor detail in `docs/` and link to it from the README when needed.                    |

## Generated and scratch artefacts

`target/` is the normal build and generated-output location. It may contain
compiled crates, generated Rust documentation, `trybuild` crates, and temporary
documentation output. It is not a source directory.

Temporary logs and scratch files should use `/tmp`, following the command-log
patterns in `AGENTS.md`. Do not use `/tmp` as a build target, and do not create
isolated Cargo caches unless the repository instructions explicitly change.

## Documentation ownership

Long-lived documentation belongs under `docs/`. Active plans belong under
`docs/execplans/`, while historical plans or roadmaps that should remain
readable but should not drive future work belong under `docs/archive/`.

When adding, renaming, or removing documentation, update
[Documentation contents](contents.md) in the same change.
