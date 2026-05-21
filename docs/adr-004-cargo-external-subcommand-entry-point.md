# ADR-004: Cargo external-subcommand entry-point architecture

## Context and Problem Statement

Cargo dispatches external subcommands by injecting the subcommand name as the
first positional argument. A `clap` parser that only models
`cargo-<name> [OPTIONS]` rejects that injected token before application logic
can run.

OrthoConfig needs a documented entry-point shape for Cargo-facing binaries,
such as `cargo-orthohelp`, but that dispatch contract belongs at the command
boundary. It must not be folded into `OrthoConfig::load` or the configuration
merge pipeline.

## Decision

Cargo external-subcommand support remains CLI entry-point structure:

- `cargo <name> [OPTIONS]` resolves to `cargo-<name>`.
- The installed binary parser must accept the injected `<name>` token.
- Hand-built callers should use a small wrapper around `clap::Command`.
- Derive-based callers should wrap their `Args` struct in a
  `#[command(subcommand)]` enum variant.
- Configuration precedence remains defaults → files → environment → explicit
  command-line arguments.

## Consequences

- Cargo-dispatched and direct binary invocations share the same inner parser.
- Future subcommand-dispatch changes need matching updates to the design
  documents and user-facing invocation examples.
- Regression coverage should continue to exercise both invocation forms.

# ADR-004: Cargo external-subcommand entry-point architecture


## Context and Problem Statement

Cargo dispatches external subcommands by injecting the subcommand name as the
first positional argument. A `clap` parser that only models
`cargo-<name> [OPTIONS]` rejects that injected token before application logic
can run.

OrthoConfig needs a documented entry-point shape for Cargo-facing binaries,
such as `cargo-orthohelp`, but that dispatch contract belongs at the command
boundary. It must not be folded into `OrthoConfig::load` or the configuration
merge pipeline.


## Status

Accepted


## Date

2026-05-21
