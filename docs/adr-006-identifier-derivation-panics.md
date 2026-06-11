# Architectural decision record (ADR) 006: Identifier derivation panics

## Status

Accepted.

## Date

2026-06-11.

## Context and problem statement

Roadmap item 11.1.1 promotes the `hello_world` example's `LocalizeCmd` helper
into the public `ortho_config` API. The promoted API derives Fluent message
identifiers from a `clap::Command` tree, then uses those identifiers to
localize command and argument help.

The derivation can fail when a developer-authored command path contains a
character outside Fluent's identifier grammar, or when two sibling command or
argument paths normalize to the same runtime identifier. The failure is a
programmer error in the command declaration: it is not caused by user input,
locale selection, or catalogue contents.

The question is whether `message_id_for` and `LocalizeCmd::localize` should
return `Result` for these failures, or panic and make the invalid command tree
fail at first use.

## Decision drivers

- Keep the promoted 11.1.1 API small and ergonomic for ordinary static clap
  command trees.
- Match clap's own convention for programmer errors in command mutation,
  including `Command::mut_arg` panicking for unknown argument ids.
- Preserve the roadmap requirement that identifier collisions are surfaced as
  a runtime panic in hand-built command trees.
- Leave room for a future fallible API without committing every consumer to
  fallible localization today.

## Options considered

### Option A: Panic on invalid derived identifiers

`message_id_for` panics when the final id cannot be represented as a Fluent
identifier. `LocalizeCmd::localize` panics when walking a command tree would
route two sibling command or argument paths to the same id.

This keeps the common static-command path direct and makes invalid command
declarations fail loudly at first use. It also mirrors clap's existing mutation
API, where invalid programmer-authored ids are panic conditions.

### Option B: Return `Result` from the promoted API

The promoted functions could return a domain error describing invalid
characters or collisions.

This supports applications that build command trees dynamically, but it makes
the default localization path fallible for every consumer, including the
ordinary derive-generated and static-command cases where invalid ids are
developer bugs.

### Option C: Skip invalid or colliding ids

The walker could ignore invalid ids, leave clap's stock text untouched, and
possibly emit diagnostics.

This avoids panics, but it makes two command paths silently share or lose
translations. That is worse than failing because translators and application
authors would see partial localization without a precise source of truth for
the missing copy.

## Decision outcome / proposed direction

Use Option A. The promoted `message_id_for` and `LocalizeCmd::localize` APIs
panic when a developer-authored command tree cannot produce a valid or unique
Fluent identifier.

The promoted API derives Fluent identifiers from compile-time-fixed clap
command trees. When a command path contains unrepresentable characters or
produces identifier collisions, `message_id_for` and `LocalizeCmd::localize`
panic rather than returning `Result`. This design keeps the extension trait
small and exposes declaration bugs immediately. Applications that build
command trees dynamically must validate names before localizing.

A future additive `try_message_id_for` or fallible walker can be introduced for
applications that construct command trees from runtime data. That future API
should share the same normalization and collision rules rather than define a
second identifier convention.

## Consequences

- Rustdoc for `message_id_for` and `LocalizeCmd` must document the panic
  cases clearly.
- Tests must cover unrepresentable characters and collision panics, so the
  behaviour is intentional rather than accidental.
- Applications that build command trees from external input must validate or
  sanitize names before calling `LocalizeCmd::localize`.
