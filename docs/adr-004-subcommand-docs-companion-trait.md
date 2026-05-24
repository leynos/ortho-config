# Architectural decision record (ADR) 004: Subcommand docs companion trait

## Status

Accepted.

## Date

2026-05-24.

## Context and problem statement

`#[derive(OrthoConfig)]` already emits documentation intermediate
representation (IR) through `OrthoConfigDocs`, and the IR schema already has
`DocMetadata.subcommands: Vec<DocMetadata>`. The generated value is currently
empty for every command, even when the `clap::Parser` root has a
`#[command(subcommand)]` selector.

This blocks whole-CLI introspection. Human documentation renderers and future
agent-facing summaries need the same recursive command tree: a top-level
configuration node with one child metadata node per subcommand, preserving the
order and command labels that clap exposes to users.

The question is where enum-level subcommand metadata belongs without changing
the IR schema, moving bridge logic into `cargo-orthohelp`, or making downstream
applications stitch trees by hand.

## Decision drivers

- Keep `ortho_config` as the owner of reusable human-documentation IR.
- Preserve the existing `DocMetadata` schema and `ORTHO_DOCS_IR_VERSION`.
- Keep the `cargo-orthohelp` bridge as a thin adapter that serializes
  `OrthoConfigDocs::get_doc_metadata()`.
- Make the consumer pattern explicit and discoverable at compile time.
- Preserve clap declaration order and command naming conventions.

## Requirements

### Functional requirements

- A top-level `OrthoConfig` struct with a `#[command(subcommand)]` field can
  emit populated recursive `DocMetadata.subcommands` values.
- Each subcommand enum variant contributes one child `DocMetadata` value in
  declaration order.
- Default command labels use clap-compatible kebab-case variant names, with
  `#[command(name = "...")]` and `#[clap(name = "...")]` overrides honoured.

### Technical requirements

- Do not add, remove, or rename fields in the documentation IR schema.
- Do not change `ORTHO_DOCS_IR_VERSION`.
- Keep the new public surface additive.
- Keep enum-level docs generation independent of `SelectedSubcommandMerge`,
  while sharing clap attribute parsing helpers where useful.

## Options considered

### Option A: Extend `OrthoConfigDocs`

`OrthoConfigDocs` could grow a second method for subcommand enums, or its
single method could be interpreted differently for structs and enums.

This keeps one trait name, but it weakens the trait contract. A config struct
returns one `DocMetadata`; a subcommand selector enum returns many child nodes.
Combining those meanings in one trait would either require a breaking trait
change or a less obvious return shape.

### Option B: Introduce `OrthoConfigSubcommandDocs`

Add a companion trait in `ortho_config::docs`:

```rust
pub trait OrthoConfigSubcommandDocs {
    fn get_subcommand_doc_metadata() -> Vec<DocMetadata>;
}
```

A new derive macro implements this trait for `clap::Subcommand` enums. The
existing `OrthoConfig` derive detects `#[command(subcommand)]` fields and
delegates to the companion trait for the child metadata vector.

This keeps each trait's contract narrow. Structs still implement
`OrthoConfigDocs`; selector enums implement `OrthoConfigSubcommandDocs`.

### Option C: Require manual stitching

Consumers could manually call each subcommand argument type's
`OrthoConfigDocs::get_doc_metadata()` implementation and append child nodes to
the root metadata.

This avoids a new trait, but it makes recursive IR easy to omit, duplicates
clap naming logic in applications, and defeats the goal of generated
documentation metadata.

The companion trait is the only option that is additive, keeps the trait
contract narrow, gives consumers a generated pattern, leaves the bridge
unchanged, and keeps command naming generated rather than application-owned.

## Decision outcome / proposed direction

Use Option B. `OrthoConfigSubcommandDocs` is the accepted companion trait for
enum-level subcommand documentation metadata. It is part of the
human-documentation IR contract owned by `ortho_config::docs`.

The derive macro for subcommand enums belongs in `ortho_config_macros`. The
existing struct derive remains responsible for root metadata generation and
uses the companion trait only when it finds a clap subcommand selector field.
`cargo-orthohelp` continues to serialize the root `DocMetadata` value without
knowing how the recursive tree was generated.

## Goals and non-goals

- Goals:
  - Populate `DocMetadata.subcommands` for opted-in clap subcommand enums.
  - Preserve declaration order and clap-compatible command names.
  - Keep the bridge, renderers, and schema versions stable.
- Non-goals:
  - Model hidden, aliased, or deprecated command metadata.
  - Add agent-context or policy-report fields.
  - Support unit subcommand variants in the first implementation.

## Migration plan

1. Add `OrthoConfigSubcommandDocs` to `ortho_config::docs` and re-export it
   from the crate root.
2. Add a derive macro for tuple-variant `clap::Subcommand` enums.
3. Teach `OrthoConfig` to skip subcommand selector fields when generating
   configuration fields, and to use the companion trait for
   `DocMetadata.subcommands`.
4. Add unit, compile-fail, behavioural, and renderer smoke coverage for the
   recursive tree.
5. Update user, design, roadmap, and changelog documentation once validation
   passes.

## Known risks and limitations

- Unit variants are deferred because they have no inner argument type from
  which to source field metadata, headings, and examples.
- Hidden commands, aliases, and deprecation metadata are deferred because the
  current IR schema has no fields for those concepts.
- The struct derive must skip the subcommand selector everywhere configuration
  fields are generated, otherwise clap or serde bounds can fail in unrelated
  generated code.

## Outstanding decisions

- Decide in a later roadmap item whether the IR should grow explicit fields
  for hidden, aliased, and deprecated subcommands.
- Decide in a later additive change how unit variants should produce minimal
  `DocMetadata` values.
