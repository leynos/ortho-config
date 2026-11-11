# Architectural decision record (ADR) 0004: Cargo external-subcommand entry-point architecture

## Status

Accepted

## Date

2026-05-21

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
- Cargo-facing binaries should initialize a tracing or logging subscriber
  before parsing and emit a debug event at the dispatch boundary once the
  injected subcommand token has been accepted.
- Configuration precedence remains defaults → files → environment → explicit
  command-line arguments.

## Consequences

- Cargo-dispatched and direct binary invocations share the same inner parser.
- Dispatch-boundary observability stays lightweight: a debug event records that
  Cargo's injected token was accepted, while metrics remain deferred until the
  wider workspace has a metrics strategy.
- Future subcommand-dispatch changes need matching updates to the design
  documents and user-facing invocation examples.
- Regression coverage should continue to exercise both invocation forms.

## Amendment (2026-08-09)

Roadmap item 8.3.1 shipped `ortho_config::cargo::external_subcommand` on
2026-08-07. The implementation renders usage in the Cargo dispatch form: the
synthetic parent is `Command::new("cargo").bin_name("cargo")`, so top-level
usage renders `Usage: cargo <COMMAND>` and the inner `orthohelp` command's
usage renders `Usage: cargo orthohelp [OPTIONS]`. The installed binary name is
carried as the inner command's `display_name`, so `cargo <name> --version`
prints `cargo-<name> <version>`. The inner command's `bin_name` is reset so
both the help and parse paths derive the same `cargo <name>` shape. This choice
deviates from the literal `bin_name("cargo-<name>")` wording in the roadmap and
the original design sketch.

## Clarification (2026-11-11)

The 2026-08-09 amendment governs the hand-built helper from roadmap 8.3.1 only.
Roadmap 8.3.2 retains the derive template in design §4.17, whose
`bin_name = "cargo-<name>"` names the installed binary. Both shapes accept
Cargo's injected token; their usage strings differ:

- the helper renders `Usage: cargo <COMMAND>` and
  `Usage: cargo <name> [OPTIONS]`; and
- the derive template renders `Usage: cargo-<name> <COMMAND>` and
  `Usage: cargo-<name> <name> [OPTIONS]`.

The [user's guide](../users-guide.md#derive-based-template) documents both
templates.
