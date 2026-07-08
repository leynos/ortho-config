# Architectural decision record (ADR) 006: Identifier-derivation panics

## Status

Accepted.

## Date

2026-06-09.

## Context and problem statement

Roadmap item 11.1.1 promotes the `hello_world` example's `LocalizeCmd` helper
into the public `ortho_config` API. The promoted API derives Fluent message
identifiers from a `clap::Command` tree and uses those identifiers to localize
command and argument help.

Roadmap item 11.1.2 builds on that surface with `LocalizedParse`, a blanket
trait for every `clap::Parser`, and `parse_localized_command`, the
base-agnostic parsing primitive. These helpers call `LocalizeCmd::localize`
before parsing, so the same identifier panic contract is reachable from any
consumer parser that opts into localized parsing.

Identifier derivation can fail when a developer-authored command path contains
a segment that cannot be represented as a Fluent identifier, or when two
sibling commands or arguments normalize to the same identifier. These failures
come from command declarations, not from user input, locale selection, or
catalogue contents.

The question is whether the public identifier helpers should return `Result`,
panic, or silently leave invalid command-tree nodes unlocalized.

## Decision drivers

- Keep the 11.1.1 `LocalizeCmd` API ergonomic for ordinary static clap command
  trees.
- Match clap's convention for programmer errors in command mutation, including
  `Command::mut_arg` panicking for invalid argument ids.
- Honour the §4.1 design mandate that unrepresentable identifiers and
  collisions are surfaced instead of hidden.
- Preserve an additive path for dynamic command-tree builders that need a
  fallible validation API later.

## Options considered

### Option A: `Result`-returning API

`message_id_for` and `LocalizeCmd::localize` could return a domain error for
invalid segments and collisions.

This gives dynamic command-tree builders a direct validation path, but it makes
the common static-command path fallible for every consumer. It also infects
otherwise straightforward command construction with error plumbing for bugs
that are usually authored in source and found during testing or first run.

### Option B: Panic on invalid derived identifiers

`message_id_for` panics when the final id cannot be represented as a Fluent
identifier. `LocalizeCmd::localize` panics when the tree walk finds a collision
among sibling commands or arguments under the same parent node.

This keeps the default API direct, mirrors clap's mutation conventions, and
surfaces declaration bugs immediately. It is the accepted option.

### Option C: Silent fallback

The walker could skip invalid or colliding ids and leave clap's stock copy in
place.

This is rejected unconditionally. Silent fallback would make translations
partially disappear, hide invalid identifiers from application authors, and
leave translators without a precise signal that two paths cannot be
distinguished.

## Decision outcome / proposed direction

In the context of deriving Fluent identifiers from compile-time-fixed clap
command trees, facing the need to surface unrepresentable or colliding ids, we
decided to panic (matching clap's `mut_arg` convention and the §4.1 mandate)
and neglected a `Result`-returning API, accepting that hand-built dynamic trees
must validate names before localizing, because the inputs are
developer-authored constants surfaced at first run.

`message_id_for` owns the strict segment normalization rule. The command-tree
walker owns collision detection while it traverses each parent node. Collision
checks are scoped per parent, so two commands in different subtrees may share a
local name, but two siblings that normalize to the same id panic.

The future additive extension is
`try_message_id_for -> Result<String, IdentifierError>`. It should share the
same normalization rule as `message_id_for`, expose structured errors for
invalid segments, and avoid changing the existing panic contract.

## Goals and non-goals

- Goals:
  - Record the accepted panic contract for the promoted 11.1.1 helpers.
  - Keep invalid static command declarations loud and easy to diagnose.
  - Preserve a non-breaking path for future fallible identifier validation.
- Non-goals:
  - Add a fallible command-tree walker in 11.1.1.
  - Define translator diagnostics for missing catalogue entries.
  - Change Fluent load-time resource-id normalization.

## Migration plan

1. Document the panic contract in the CLI localization design.
2. Document the panic cases in rustdoc for `message_id_for` and
   `LocalizeCmd::localize`.
3. Add tests for invalid segments and per-parent collision panics.
4. Leave `try_message_id_for` as a later additive API.

## Known risks and limitations

- Applications that build command trees from runtime data must validate names
  before calling `LocalizeCmd::localize`, otherwise invalid external data can
  become a process panic.
- `LocalizedParse` widens the reachable panic surface from explicit command
  localization calls to every `clap::Parser` that opts into localized parsing.
  This is accepted until the planned derive-time guard in 11.1.3 can emit a
  compile-time error for generated identifiers.
- Panic contracts are harder to relax than ordinary internal implementation
  choices because downstream tests may begin to rely on the exact failure
  surface.
- The future fallible API must avoid diverging from `message_id_for`, or the
  crate will have two subtly different identifier conventions.

## Outstanding decisions

- Decide whether a future fallible command-tree walker should be named
  `try_localize`, `try_localize_self`, or exposed through a separate trait.
- Decide the exact `IdentifierError` variants and whether collision errors
  belong in that enum or in a walker-specific error type.
- Decide whether dynamic command-tree validation should expose all detected
  errors at once or fail on the first invalid segment or collision.
