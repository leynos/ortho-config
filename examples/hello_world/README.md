# Hello World example

This crate will showcase a minimal, end-to-end, configuration-driven
command-line application. It focuses on demonstrating the orthogonal
configuration concepts that power the wider project without adding
production-ready complexity.

## Demonstrated capabilities

- **Global parameters (switches and arrays)**: illustrate how the command-line
  parser exposes top-level configuration that applies to every command,
  covering boolean feature switches, repeated values, and precedence between
  defaults and caller-supplied input.
- **Subcommands**: implement a friendly `greet` command that accepts a name and
  configurable greeting, alongside a `take-leave` workflow that combines
  switches, optional arguments, and shared greeting customizations to decide
  how a farewell is delivered.
- **Testing disciplines**: add `rstest`-powered unit tests for deterministic
  components and `cucumber-rs` behavioural specifications that exercise the
  binary as a user would, capturing configuration precedence and cross-platform
  quirks.
- **Shell and Windows automation**: provide paired `.sh` and `.cmd` scripts
  highlighting how environment variables, configuration files, and command-line
  overrides interact. Include examples covering default configuration,
  per-subcommand overrides, and the precedence order across the sources.

## Planned project layout

- `src/` will contain a small `main.rs` and supporting modules for
  option-parsing, command dispatch, and domain logic.
- `tests/` will host Cucumber steps and supporting fixtures for behavioural
  coverage.
- `scripts/` will offer automation snippets, with mirrored POSIX shell and
  Windows `.cmd` scripts to showcase configuration strategies on each platform.
- `config/` will collect sample configuration files that the scripts reference
  during demonstrations.

## Implementation considerations

- Keep the greeting and farewell flows intentionally simple so that the focus
  remains on configuration handling rather than application behaviour.
- Ensure every configuration source is represented in both documentation and
  automated coverage to demonstrate reproducibility.
- Document how to run the example from a fresh checkout through the scripts and
  behavioural tests.

## Getting started

- Prerequisites: Rust toolchain (via rustup), Cargo, make, and
  markdownlint-cli2.
- Build: `cargo build`.
- Run formatting and Markdown lint checks: `make fmt && make markdownlint`.
- Validate Mermaid diagrams (if present): `make nixie`.
- Run static analysis: `cargo clippy -D warnings` (or `make lint`).
- Execute tests (unit and behavioural): `make test`.

## Implementation checklist

- [x] Scaffold the crate with `Cargo.toml`, `src/main.rs`, and supporting
      modules.
- [x] Define global command-line parameters, switches, and array parameters
      with defaults and validation.
- [x] Implement the `greet` subcommand with its arguments and options.
- [x] Implement the `take-leave` subcommand with its arguments and options.
- [x] Add `rstest` unit tests covering parsing, validation, and command logic.
- [x] Add `cucumber-rs` behavioural tests covering end-to-end workflows and
      configuration precedence.
- [ ] Create shell and Windows `.cmd` scripts showcasing configuration file
      usage and overrides.
- [ ] Provide sample configuration files aligned with the scripts and tests.
- [ ] Update documentation to reference the example and describe how to run it.
