# Generate recursive DocMetadata.subcommands values

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: APPROVED

This plan covers roadmap item 6.1.1 only. It does not implement the fixtures,
nested behavioural coverage, or rendering assertions described by roadmap item
6.1.2; those follow once 6.1.1 is approved, implemented, and merged.

## Purpose / big picture

Phase 6 of the active roadmap (see `docs/roadmap.md` §6, "Deliver whole-CLI
introspection") cannot proceed while the documentation intermediate
representation (IR) emitted by `#[derive(OrthoConfig)]` reports an empty
`DocMetadata.subcommands` array for every command, regardless of whether the
target binary has subcommands. The agent-native CLI design document
(`docs/agent-native-cli-design.md` §4) names this gap explicitly and lists it
in §9 ("Current gaps to resolve") as a precondition for compact agent-context
output and vocabulary policy work in phases 6.2 and 7.

After this plan is approved and implemented, a maintainer working in a
downstream consumer crate should be able to:

1. annotate a top-level `clap::Parser` struct with `#[derive(OrthoConfig)]`,
   keep its `#[command(subcommand)]` field referencing a `clap::Subcommand`
   enum, and add `#[derive(OrthoConfigSubcommandDocs)]` to that enum;
2. run `cargo orthohelp --format ir` against the consumer crate and observe a
   JSON document whose `subcommands` array contains one entry per enum
   variant, in declaration order, each entry carrying that variant's full
   `DocMetadata` (fields, headings, examples, windows metadata, and any further
   nested subcommands);
3. point `cargo orthohelp --format man` or `--format ps` at the same root type
   and see the generated man pages or PowerShell wrappers include sections for
   every subcommand without bespoke per-app glue code.

Observable success is checked by:

- new `rstest` unit tests in `ortho_config/tests/docs_ir.rs` and a dedicated
  `ortho_config/tests/subcommand_docs.rs` asserting populated, recursive
  `DocMetadata.subcommands` values with deterministic ordering, correct command
  labels, and an empty parent-level `fields` entry for the subcommand selector
  field;
- new `rstest-bdd` scenarios in `ortho_config/tests/features/docs_ir.feature`
  exercising a nested fixture;
- updated round-trip and rendering smoke tests in
  `cargo-orthohelp/src/schema/tests.rs` (and supporting fixtures) that prove
  the existing renderers consume non-empty subcommand trees without
  regressions;
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all passing at the close of each milestone; and
- `coderabbit review --agent` returning clean (or with all concerns resolved)
  before each milestone is marked done.

This plan does not change the IR schema version, the bridge pipeline,
`cargo-orthohelp`'s CLI surface, the agent-context schema, or policy reports.
It also does not add new external dependencies; the necessary `heck` traits
are already declared in `ortho_config_macros/Cargo.toml:21`.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violating any of them requires escalation in `Decision Log`, not
a workaround.

- Do not implement code, tests, examples, or documentation in this branch
  until this ExecPlan is explicitly approved by the maintainer. A "DRAFT" plan
  must remain a planning artefact only.
- Keep this work focused on roadmap item 6.1.1 ("Generate recursive
  `DocMetadata.subcommands` values"). Behavioural fixtures and Windows
  wrapper assertions described by item 6.1.2 are explicitly out of scope; if
  partial coverage of 6.1.2 falls out of 6.1.1 work, mark it clearly and stop
  for separate approval before extending it.
- Do not change `ORTHO_DOCS_IR_VERSION`, `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`,
  or `ORTHO_POLICY_REPORT_SCHEMA_VERSION`. The added `subcommands`
  population is an IR data fix, not a schema migration.
- Do not add or rename fields on `ortho_config::docs::ir::DocMetadata`,
  `SectionsMetadata`, `HeadingIds`, `FieldMetadata`, `CliMetadata`,
  `EnvMetadata`, `FileMetadata`, `ValueType`, `DefaultValue`, `Deprecation`,
  `ConfigDiscoveryMeta`, `ConfigFormat`, `PathPattern`, `PrecedenceMeta`,
  `SourceKind`, `WindowsMetadata`, `Example`, `Link`, or `Note`. The existing
  `DocMetadata.subcommands: Vec<DocMetadata>` (see
  `ortho_config/src/docs/ir.rs:26`) and the existing
  `HeadingIds.commands: Option<String>` (see
  `ortho_config/src/docs/ir.rs:72-73`) are the canonical carriers.
- Keep the mirrored schema in `cargo-orthohelp/src/schema/mod.rs` byte-for-byte
  aligned with `ortho_config/src/docs/ir.rs`. The existing version-alignment
  test in `cargo-orthohelp/src/schema/tests.rs` must continue to pass.
- Preserve the boundary established by ADR-003
  (`docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`). The
  new `OrthoConfigSubcommandDocs` trait belongs to the human-documentation IR
  contract and stays in `ortho_config::docs`; it must not introduce any
  dependency on `cargo-orthohelp`, the bridge, the agent-context schema, or
  policy reports.
- Keep `cargo-orthohelp/src/bridge.rs` unchanged in its public behaviour. The
  bridge's `<RootType as OrthoConfigDocs>::get_doc_metadata()` invocation at
  `cargo-orthohelp/src/bridge.rs:174-183` already serializes the recursive
  IR; this work plumbs data into that structure, it does not change how the
  bridge runs.
- Preserve `cargo orthohelp --format ir`, `--format man`, `--format ps`, and
  `--format all` output for any consumer whose top-level config has no
  subcommand selector. New behaviour only manifests when the consumer opts in
  by adding the new derive on a subcommand enum and the new clap-subcommand
  field marker on the parent struct (see Design Overview below).
- Keep the new trait additive: the default `subcommands: Vec::new()` must
  still be emitted for any config without a `#[command(subcommand)]` /
  `#[clap(subcommand)]` field. No existing `#[derive(OrthoConfig)]`
  invocation may begin to fail after this work.
- Use `cap_std`/`camino` instead of `std::fs`/`std::path` if any test or
  example introduces filesystem I/O. The existing crates already follow this
  rule; no new I/O is anticipated for this plan.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests, per
  `docs/developers-guide.md` and the project's `Cargo.toml` lints. Do not
  introduce `proptest`, `kani`, or `verus` for this work: there is no
  invariant beyond declaration-order preservation that a deterministic test
  cannot cover.
- Use `heck::ToKebabCase` (already in `ortho_config_macros/Cargo.toml:21`) for
  the default variant-to-command-name conversion. Do not introduce any new
  crate dependency.
- Keep every Rust file under 400 lines. Every module must begin with a `//!`
  comment. Use en-GB-oxendict spelling and grammar in documentation and
  comments, except for external API names such as `color`.
- Follow `docs/documentation-style-guide.md` for all documentation edits:
  Markdown wrapping at 80 columns, fenced code blocks must have a language
  identifier (`plaintext` for non-code text), and ADR / design-document
  structure.
- Run validation commands sequentially and capture output with `tee` into
  `/tmp` log files. Do not run format checks, lints, and tests in parallel
  (they share the same Cargo cache and the makefile relies on serial
  execution). Use the filename template
  `/tmp/$ACTION-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out`.
- Do not mark roadmap item 6.1.1 complete in `docs/roadmap.md` until every
  validation gate listed in "Validation and acceptance" has passed and the
  draft pull request created from this plan has been moved out of draft state.

If satisfying the objective requires violating a constraint, stop, document
the conflict in `Decision Log`, and ask the maintainer for direction.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Approval: stop after drafting this plan and wait for explicit maintainer
  approval before any milestone other than Milestone 0 (`Approval and ADR
  drafting`) is started.
- Scope: stop if the implementation requires changes to more than 22 files or
  more than 1100 net lines of code and documentation (excluding `.stderr`
  fixtures and golden snapshots, which are reviewed separately).
- Public API: stop if any existing public type, trait, constant, function,
  command flag, derived attribute key, or feature flag must be renamed or
  removed. Additive new items (a trait, a derive, a field attribute) do not
  trip this tolerance.
- Schema shape: stop if a plausible alternative shape for the new trait or
  the new derive attribute would materially affect downstream
  compatibility — for example, if the derive must expose a stable
  `command_name` override on the IR itself. Present the alternatives with
  trade-offs.
- IR schema: stop if any field on any of the existing IR types must change.
  This work is not authorised to evolve the schema; that requires a separate
  ADR and a bump of `ORTHO_DOCS_IR_VERSION`.
- Dependencies: stop if any new crate, build script, generated file, or
  non-standard Cargo feature is required.
- Proof tooling: stop if a proposed addition of `kani`, `verus`, or
  `proptest` would add tooling without a substantive invariant that
  deterministic tests cannot cover.
- Tests: stop if `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, or `make nixie` still fails after two focused fix
  attempts.
- Documentation: stop if `docs/design.md`, `docs/cargo-orthohelp-design.md`,
  `docs/agent-native-cli-design.md`, and the new ADR cannot describe the same
  trait surface and ownership without contradiction.
- Process: stop if branch rename, push, draft pull-request creation, or
  `coderabbit review --agent` fails in a way that might hide review feedback
  or leave the repository in an inconsistent state.
- Iteration: stop if a single milestone takes more than three working sessions
  without observable progress on its acceptance criteria. Record the cause in
  `Surprises & Discoveries`.

Adjust these values only with explicit maintainer approval recorded in
`Decision Log`.

## Risks

Known uncertainties that might affect the plan. Each risk records severity,
likelihood, and mitigation. Update this section as work proceeds and as new
risks emerge.

- Risk: extending the `OrthoConfig` struct derive to gracefully tolerate a
  `#[command(subcommand)]` field is a non-trivial pipeline change. Today every
  per-field loop in the derive (CLI struct generation in
  `ortho_config_macros/src/derive/build/cli/cli_flags.rs:298-323`, defaults
  generation in `ortho_config_macros/src/derive/build/defaults.rs:19-34`,
  collection-strategy collection in
  `ortho_config_macros/src/derive/build/override/mod.rs:111`,
  `CliFieldMetadata` collection in
  `ortho_config_macros/src/derive/build/cli/cli_flags.rs:325-351`,
  `cli_field_info` collection in
  `ortho_config_macros/src/derive/build/cli_tokens.rs:80-85`, value-type
  inference in
  `ortho_config_macros/src/derive/generate/docs/fields/value_types.rs:1-60`,
  and env-name validation in
  `ortho_config_macros/src/derive/generate/docs/fields/mod.rs:178-194`) treats
  every named field as a configuration field. A subcommand field is none of
  those things. Severity: high. Likelihood: high. Mitigation: thread a single
  `is_subcommand` flag on `FieldAttrs` (set during `parse_field_attrs`,
  `ortho_config_macros/src/derive/parse/mod.rs:343`) and add an early
  `if attrs.is_subcommand { continue; }` guard to every loop listed above.
  Cover with `ortho_config_macros/src/tests.rs` unit tests that prove the
  guard fires before any per-field code is emitted.
- Risk: the `serde::Deserialize` bound enforced by
  `ortho_config_macros/src/derive/generate/ortho_impl.rs:43-46` (the
  `_assert_deser` helper) is transitive. Even with every per-field generator
  short-circuiting, the outer struct itself still has to satisfy
  `DeserializeOwned`, which transitively demands `Commands: Deserialize`. Real
  consumer `clap::Subcommand` enums do not derive `Deserialize`. Severity:
  high. Likelihood: high. Mitigation: change `_assert_deser` (or whatever
  contains the `where Self: DeserializeOwned` bound) so that the assertion
  only fires for fields the loader will actually deserialize. The simplest
  expression is to add `#[serde(skip)]` to the generated CLI/default-struct
  field for the subcommand selector or to omit the field from those generated
  structs entirely (preferred). Add an `rstest` case that derives `OrthoConfig`
  on a `Parser` struct with a non-`Deserialize` `Commands` enum field and
  proves it compiles.
- Risk: clap forbids combining `#[command(subcommand)]` with `#[arg(long =
  ...)]` on the same field. If the generated CLI struct still emits an
  `#[arg]` for the subcommand selector, compilation fails inside clap, not
  inside the derive. Severity: high. Likelihood: high. Mitigation: the
  subcommand guard in `build_cli_struct_fields` must skip the field entirely
  (not merely change its attributes). Cover with a `trybuild` smoke case
  asserting the derive expansion compiles end-to-end alongside a real
  `clap::Parser` derive on the same struct.
- Risk: ambiguity over where the enum-level `DocMetadata` derivation should
  live. The roadmap (`docs/roadmap.md:119-120`) frames the choice as "Introduce
  a small companion trait if enum-level documentation cannot be represented
  cleanly through the existing `OrthoConfigDocs` trait." The recommended
  design (below) introduces a new trait. Severity: medium. Likelihood: medium.
  Mitigation: capture the choice in ADR-004 (see Milestone 0); the ADR is the
  approval gate. If review prefers to extend `OrthoConfigDocs` instead, stop
  and revise the plan.
- Risk: divergence between the variant-name convention used by clap and the
  one chosen for IR labels. Clap defaults a `Subcommand` variant's external
  name to a kebab-cased lowercase of the variant ident; an
  `#[command(name = "...")]` attribute overrides it. Severity: medium.
  Likelihood: medium. Mitigation: reuse `heck::ToKebabCase` (already in
  `ortho_config_macros/Cargo.toml:21`) for the default and reuse the existing
  `clap_variant_name` helper logic from
  `ortho_config_macros/src/selected_subcommand_merge.rs:27-45` for the
  override. Lift `clap_variant_name` into
  `ortho_config_macros/src/derive/parse/clap_attrs.rs` so both derives share
  it without `SelectedSubcommandMerge` taking a `serde_json` dependency.
- Risk: validation drift between `SelectedSubcommandMerge` and the new
  `OrthoConfigSubcommandDocs` derive. Today `SelectedSubcommandMerge` accepts
  only tuple variants with exactly one field (see
  `ortho_config_macros/src/selected_subcommand_merge.rs:47-63`). For docs,
  unit variants (subcommands without arguments) are a legitimate clap
  pattern. Severity: medium. Likelihood: medium. Mitigation: keep the
  derives separate; share only `clap_variant_name` and an unvalidated variant
  iterator. The first cut of the docs derive may keep the same single-tuple
  rule and reject unit variants with a clear compile-time error; lifting that
  restriction is an explicitly deferred follow-up tracked in
  `Decision Log`.
- Risk: variant-naming overrides may need to flow into the child
  `DocMetadata.app_name`. The default app-name resolver in
  `ortho_config_macros/src/derive/generate/docs/sections.rs:29-35` builds
  `app_name` from the consumer struct's identifier (or `discovery(app_name =
  ...)`). For a subcommand variant whose inner type is `RunArgs`, the
  generated `app_name` would be `run-args` — not `run`. Severity: medium.
  Likelihood: high. Mitigation: the enum derive overrides `app_name` (and the
  derived `about_id`, which is `format!("{app_name}.about")`) for each child
  `DocMetadata` before pushing it into the parent's `subcommands` vector.
  Cover with `rstest` cases asserting both the kebab-case default and the
  `#[command(name = "...")]` override.
- Risk: hidden, aliased, and deprecated commands have no current IR shape.
  Firecrawl research found clap models them via `hide(true)`,
  `alias()`/`visible_alias()`, and a deprecation note; gcloud models a
  `hidden` flag on tree nodes. The current `DocMetadata` schema does not
  represent any of those. Severity: low (for 6.1.1). Likelihood: medium.
  Mitigation: explicitly scope this plan to populated, visible, non-aliased
  subcommands; record hidden/alias/deprecated support as deferred work in
  `Decision Log` and surface in the ADR.
- Risk: existing renderers (`cargo-orthohelp/src/roff/mod.rs`,
  `cargo-orthohelp/src/powershell/wrapper.rs`) already iterate
  `metadata.subcommands` but have only ever been exercised with empty
  vectors. Filling those vectors may expose latent rendering bugs (heading
  Fluent ID lookups, command-name resolution, or nesting depth assumptions).
  Severity: medium. Likelihood: medium. Mitigation: add a renderer smoke
  test in Milestone 4 that drives the existing public renderer entry points
  with a `DocMetadata` containing two subcommands and asserts the runs
  succeed and emit the expected section headers. Defer richer rendering
  assertions to roadmap item 6.1.2.
- Risk: `markdownlint` or `nixie` may report pre-existing line-length or
  diagram issues unrelated to this work. Severity: low. Likelihood: medium.
  Mitigation: keep all edited paragraphs at ≤80 columns; record any
  unrelated failures (with exact file:line references) in `Surprises &
  Discoveries` and ask the maintainer whether to expand scope.
- Risk: `leta` may fail to provide Rust symbols if `rust-analyzer` is not
  available or fails to start. Severity: low. Likelihood: medium.
  Mitigation: install `rust-analyzer` via `rustup component add
  rust-analyzer` at session start; fall back to `Grep` and direct file
  inspection if `leta` is unavailable; record the limitation in `Surprises
  & Discoveries`.

## Skills and source signposts

The implementation must use these skills (loaded via the `Skill` tool)
deliberately:

- `rust-router`: route Rust-specific implementation questions to the
  smallest useful skill.
- `leta`: use as the default tool for Rust symbol navigation
  (`leta show`, `leta refs`, `leta grep`, `leta calls`); fall back to `Grep`
  only when `rust-analyzer` is unavailable.
- `rust-types-and-apis`: design the `OrthoConfigSubcommandDocs` trait shape,
  the new field attribute, and the public surface re-exports.
- `arch-crate-design`: keep the new trait inside `ortho_config::docs` and the
  derive inside `ortho_config_macros` without leaking dependencies.
- `rust-errors`: design compile-time error messages emitted by the new derive
  with the same tone as the existing `SelectedSubcommandMerge` errors at
  `ortho_config_macros/src/selected_subcommand_merge.rs:47-63`.
- `domain-cli-and-daemons`: keep `cargo-orthohelp` stdout, stderr, exit
  codes, and machine-readable output stable across this work.
- `hexagonal-architecture`: protect the metadata contract in `ortho_config`
  from any awareness of bridge, renderer, filesystem, or process I/O.
- `rust-types-and-apis` (when shaping API ergonomics): prefer a small,
  obvious public surface over flexible-but-mysterious extension points.
- `rust-unused-code`: ensure conditional code paths (such as the
  subcommand-field guard) do not produce `dead_code` warnings.
- `rust-testing-with-rstest-fixtures` (`docs/rust-testing-with-rstest-fixtures.md`)
  and `docs/reliable-testing-in-rust-via-dependency-injection.md` for unit
  test patterns.
- `docs/rstest-bdd-users-guide.md` for behavioural scenario authoring.
- `docs/rust-doctest-dry-guide.md` for doctest patterns on the new derive.
- `docs/localizable-rust-libraries-with-fluent.md` because the recursive IR
  carries Fluent identifiers in every subcommand entry.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for the
  shared-helper refactor that lifts `clap_variant_name` into
  `derive/parse/clap_attrs.rs`.
- `en-gb-oxendict` for documentation spelling and grammar.

The implementation must review and keep aligned with:

- `docs/roadmap.md` (especially §6.1.1, §6.1.2, and §6.2);
- `docs/design.md` §4.2 ("The `#[derive(OrthoConfig)]` Macro") and §9
  ("Decision log");
- `docs/cargo-orthohelp-design.md` §2.1 (top-level metadata), §3.1 (trait),
  §3.5 (implementation notes), and §13.1 (IR JSON excerpt);
- `docs/agent-native-cli-design.md` §3.1 (human documentation IR), §4
  (whole-CLI introspection), and §9 (current gaps to resolve);
- `docs/users-guide.md` (the `OrthoConfigDocs` section near line 1156, the
  subcommand walkthroughs near lines 781 and 860);
- `docs/developers-guide.md` (the "Schema ownership" section);
- `docs/documentation-style-guide.md` (ADR template, design-document
  guidance, en-GB-oxendict rules);
- `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md` (the
  governing ownership boundary);
- `docs/contents.md` (add a link to the new ADR).

External prior art checked during planning (firecrawl, 2026-05-22):

- `clap_serde` (<https://docs.rs/clap-serde/latest/clap_serde/>) models a
  recursive `subcommands: Vec<Command>` field per node. This is the closest
  prior art to the existing `DocMetadata.subcommands: Vec<DocMetadata>` shape.
- clap issue #918
  (<https://github.com/kbknapp/clap-rs/issues/918>) and clap discussion #3603
  (<https://github.com/clap-rs/clap/discussions/3603>) confirm that recursive
  JSON export is a long-standing community request that clap does not satisfy
  natively; consumers walk the `Command` tree themselves.
- gcloud `meta cli-trees`
  (<https://www.fig.io/manual/gcloud/meta/cli-trees>) emits a recursive tree
  with children under a `commands` field. This is the closest tool-side
  analogue to what `cargo-orthohelp` will produce once 6.1.1 lands.
- AWS CLI (<https://docs.aws.amazon.com/cli/latest/userguide/cli-usage-commandstructure.html>),
  `kubectl` (<https://kubernetes.io/docs/reference/kubectl/>), and Model
  Context Protocol `tools/list`
  (<https://modelcontextprotocol.io/specification/2025-06-18/server/tools>)
  all use flat command listings — informative for phase 6.2 (agent-context
  output), not for 6.1.1's recursive IR shape.
- Declaration order is the de facto convention across clap, botocore, gcloud
  trees, and MCP listings; no tool guarantees alphabetical ordering. The plan
  preserves Rust enum declaration order.

These sources inform the design but do not override repository documents or
require adopting any external format wholesale.

## Repository orientation

The relevant code is concentrated in three crates:

- `ortho_config` (runtime crate). Documentation IR lives in
  `ortho_config/src/docs/`. The trait `OrthoConfigDocs` is declared at
  `ortho_config/src/docs/mod.rs:18-21`. The IR schema lives in
  `ortho_config/src/docs/ir.rs`; `DocMetadata.subcommands: Vec<DocMetadata>`
  is at line 26 and `HeadingIds.commands: Option<String>` is at lines 72-73.
  The subcommand-merge surface (`SelectedSubcommandMerge` trait and helper)
  lives at `ortho_config/src/subcommand/selected.rs` and is feature-gated on
  `serde_json`.
- `ortho_config_macros` (procedural-macro crate). The struct derive entry
  point is `ortho_config_macros/src/lib.rs:44-99`; the docs-generation hook
  inside it is at line 80. The enum derive for subcommand merging is
  declared at `ortho_config_macros/src/lib.rs:109-116` and implemented in
  `ortho_config_macros/src/selected_subcommand_merge.rs`. The docs-generation
  module is `ortho_config_macros/src/derive/generate/docs/`; the literal
  `subcommands: Vec::new(),` that this plan replaces is at
  `ortho_config_macros/src/derive/generate/docs/mod.rs:67`. Clap-attribute
  parsing helpers live at
  `ortho_config_macros/src/derive/parse/clap_attrs.rs`; the parallel struct
  attribute parser is `ortho_config_macros/src/derive/parse/mod.rs`. The
  `heck` crate is already declared at `ortho_config_macros/Cargo.toml:21`.
- `cargo-orthohelp` (reference tool). The bridge wrapper that prints the IR
  for a target crate is `cargo-orthohelp/src/bridge.rs`. The bridge invokes
  `<RootType as OrthoConfigDocs>::get_doc_metadata()` at lines 174-183. The
  localizer that walks the recursive IR is
  `cargo-orthohelp/src/ir.rs:198-202`. The mirrored schema lives at
  `cargo-orthohelp/src/schema/mod.rs:13`. Renderers iterate
  `metadata.subcommands` today (see `cargo-orthohelp/src/roff/mod.rs` and
  `cargo-orthohelp/src/powershell/wrapper.rs`) but have only ever been
  exercised with empty vectors.

Tests live at:

- `ortho_config/tests/docs_ir.rs` (asserts `subcommands.is_empty()` at
  line 96 — must change);
- `ortho_config/tests/features/docs_ir.feature` (rstest-bdd feature file);
- `ortho_config/tests/rstest_bdd/behaviour/steps/docs_steps.rs` (step
  definitions);
- `ortho_config/tests/clap_subcommand.rs` and
  `ortho_config/tests/selected_subcommand_merge.rs` (existing subcommand
  fixtures);
- `cargo-orthohelp/src/schema/tests.rs` (round-trip plus version-alignment
  tests);
- `ortho_config_macros/src/tests.rs` (token-generation unit tests).

Examples live at `ortho_config/examples/registry_ctl.rs` and the
`examples/hello_world` crate.

## Recommended design

The implementation must record these decisions, subject to plan approval and
the ADR drafted in Milestone 0. The decisions point inward: the trait sits in
`ortho_config::docs`; the derive sits in `ortho_config_macros`; the bridge
remains a thin adapter.

### A new public trait `OrthoConfigSubcommandDocs`

Declare in `ortho_config/src/docs/mod.rs`, next to `OrthoConfigDocs`:

```rust
/// Trait implemented for `clap::Subcommand` enums that can emit
/// per-variant documentation metadata.
///
/// Each entry in the returned vector is the variant's full
/// [`DocMetadata`], with `app_name` overridden to the clap command label
/// (kebab-cased variant ident or the value of `#[command(name = "...")]`)
/// and `about_id` regenerated accordingly. Variants appear in declaration
/// order to keep generated documentation and agent context deterministic.
pub trait OrthoConfigSubcommandDocs {
    /// Returns one [`DocMetadata`] per subcommand variant, in declaration
    /// order.
    fn get_subcommand_doc_metadata() -> Vec<DocMetadata>;
}
```

Re-export from `ortho_config/src/lib.rs` so consumers can write
`use ortho_config::OrthoConfigSubcommandDocs;` (and the docs-module
re-export remains the canonical path for downstream tooling that depends on
the IR contract).

The trait is unconditional (no `#[cfg(feature = ...)]`), mirroring
`OrthoConfigDocs`, because the documentation IR module is itself
unconditional (`ortho_config/src/docs/mod.rs:1-21`).

### A new derive macro `OrthoConfigSubcommandDocs`

Add a `#[proc_macro_derive(OrthoConfigSubcommandDocs, attributes(ortho_config,
ortho_subcommand))]` entry point to `ortho_config_macros/src/lib.rs`, sitting
beside `derive_selected_subcommand_merge` at lines 109-116. The
implementation lives in a new file
`ortho_config_macros/src/subcommand_docs.rs` (sibling of
`selected_subcommand_merge.rs`).

The derive:

1. validates the input is a `Data::Enum` (compile error otherwise with the
   same tone as `SelectedSubcommandMerge`);
2. for each variant in declaration order, requires a single-tuple variant
   `Variant(Args)` and produces a `DocMetadata` token expression equal to
   `{
       let mut md = <Args as #krate::docs::OrthoConfigDocs>::get_doc_metadata();
       md.app_name = <command-name-literal>.to_string();
       md.about_id = format!("{}.about", md.app_name);
       md
   }`;
3. emits a `Vec<DocMetadata>` collected from those expressions;
4. wraps the lot in `impl #krate::docs::OrthoConfigSubcommandDocs for #ident`;
5. supports the same `#[ortho_config(crate = "...")]` override that
   `SelectedSubcommandMerge` already supports (reuse
   `ortho_config_macros/src/derive/crate_path.rs`).

Unit variants and multi-tuple variants emit the same kind of compile-time
error that `SelectedSubcommandMerge` produces today. Document the unit-variant
restriction explicitly in `Decision Log` so a follow-up plan can lift it.

The variant command label is resolved by a helper lifted out of
`selected_subcommand_merge.rs:27-45` into
`ortho_config_macros/src/derive/parse/clap_attrs.rs`. The default is
`variant.ident.to_string().to_kebab_case()`; the override is
`#[command(name = "...")]` (also accepted as `#[clap(name = "...")]`).

### Struct-side detection of `#[command(subcommand)]`

The struct derive (`OrthoConfig`) learns to recognize a single field marked
`#[command(subcommand)]` or `#[clap(subcommand)]` and treats it as a
subcommand selector rather than a configuration field.

The mechanism:

1. Add a helper
   `pub(crate) fn clap_field_is_subcommand(field: &syn::Field) -> syn::Result<bool>`
   to `ortho_config_macros/src/derive/parse/clap_attrs.rs`. The shape mirrors
   `clap_arg_id` (`clap_attrs.rs:59-65`) and uses `parse_nested_meta` to flip
   a `bool` when `meta.path.is_ident("subcommand")` with no `= value` (the
   exact pattern used by `variant_has_matches` in
   `selected_subcommand_merge.rs:10-25`).
2. Extend `FieldAttrs` (`derive/parse/mod.rs:74-84`) with a non-public
   `pub(crate) is_subcommand: bool` field.
3. Set it inside `parse_field_attrs` (`derive/parse/mod.rs:343`) by calling
   the new helper before any other handling. Reject the combination of
   `is_subcommand` with any other `#[ortho_config(...)]` field-level option
   (such as `skip_cli`, `default`, `merge_strategy`, etc.) with a compile-time
   error: a subcommand selector is not a configuration field.
4. Skip the field in every per-field loop:
   - `build_cli_struct_fields` (`derive/build/cli/cli_flags.rs:298-323`);
   - `build_default_struct_fields` (`derive/build/defaults.rs:19-34`);
   - `build_default_struct_init` (`derive/build/defaults.rs:36+`);
   - `collect_collection_strategies` (`derive/build/override/mod.rs:111`);
   - `build_cli_field_metadata` (`derive/build/cli/cli_flags.rs:325-351`);
   - `cli_field_info` filter (`derive/build/cli_tokens.rs:80-85`);
   - `build_fields_metadata` (`derive/generate/docs/fields/mod.rs:40-74`);
   - any other location that touches `field_attrs.iter().zip(fields.iter())`
     (audit before edits).
5. In `generate_docs_impl` (`derive/generate/docs/mod.rs:32-73`), replace
   `subcommands: Vec::new()` with an expression that, for each subcommand
   field, expands to
   `<#FieldType as #krate::docs::OrthoConfigSubcommandDocs>::get_subcommand_doc_metadata()`
   and concatenates the results (single field expected; reject more than one
   subcommand selector per struct as a compile-time error, mirroring clap's
   own behaviour).
6. Repair the `_assert_deser` bound check
   (`derive/generate/ortho_impl.rs:43-46`) so the subcommand-selector field is
   not transitively forced to implement `Deserialize`. The preferred approach
   is for the generated CLI/default structs to omit the subcommand field
   entirely; with no generated field, no transitive bound applies.

This combination keeps the public surface tiny (one trait, one derive, one
clap-attribute marker) while leaving the bridge and the renderers completely
unaffected.

### Naming and ordering

- Default variant command name: `variant.ident.to_string().to_kebab_case()`
  via `heck::ToKebabCase`. Matches clap's own default for `Subcommand`
  variants without an explicit `name`.
- Override: `#[command(name = "...")]` (also `#[clap(name = "...")]`).
- Ordering: enum source declaration order. The generated `Vec<DocMetadata>` is
  built by `push`ing entries in the order produced by
  `enum_data.variants.iter()`, which mirrors source order. This matches
  declaration-order conventions in clap, botocore, gcloud trees, and the
  precedent set by `SelectedSubcommandMerge`.

### Recursion

A subcommand variant's inner type is itself a `#[derive(OrthoConfig)]` struct.
If that struct has its own `#[command(subcommand)]` field referencing a
further `clap::Subcommand` enum (also annotated with
`#[derive(OrthoConfigSubcommandDocs)]`), recursion happens naturally: the
inner struct's `get_doc_metadata` calls the inner enum's
`get_subcommand_doc_metadata`, and so on. No explicit recursion depth limit is
imposed beyond what clap accepts, because subcommand depth is bounded by the
consumer's enum definitions.

### Out of scope

The following are deliberately not addressed by this plan:

- hidden, aliased, and deprecated subcommand annotations (no IR shape for
  them yet; record as deferred work);
- per-variant Fluent identifiers for command descriptions (the derived
  `about_id` is `<kebab>.about`, identical to the existing top-level
  default);
- unit variants of `clap::Subcommand` enums (no inner type; deferred);
- `#[command(flatten)]` on the subcommand selector (clap allows it, but it
  changes the field meaning entirely and is not a "subcommand selector");
- `#[command(external_subcommand)]` on the variant (these are dynamic and
  cannot be modelled statically; reject with a clear compile error if
  encountered, but the v1 scope simply rejects unit variants which covers the
  common case);
- emitting subcommands into the agent-context schema (`6.2.1`) or into
  policy reports (`7.1`–`7.3`).

## Planned implementation milestones

Each milestone ends with a validation gate. Do not begin the next milestone
until the previous one's gate is green and `coderabbit review --agent` is
clear. Commit at the end of each milestone (or more frequently if local
checkpoints are useful) with descriptive messages following
`docs/documentation-style-guide.md` and `AGENTS.md`.

### Milestone 0: approve plan, draft ADR

Goal: turn this plan into an approved-and-recorded design decision before
touching code.

Steps:

1. Submit this ExecPlan for review and wait for explicit maintainer approval.
   Record the approval date in `Progress`. Update `Status:` to `APPROVED`.
2. Draft `docs/adr-004-subcommand-docs-companion-trait.md` following the ADR
   template in `docs/documentation-style-guide.md:411-489`. Cover at minimum:
   - context (the gap named by
     `docs/agent-native-cli-design.md` §4 and §9);
   - alternatives considered (extend `OrthoConfigDocs`, introduce a
     `OrthoConfigSubcommandDocs` companion trait, require manual user-side
     stitching);
   - the accepted decision (the companion trait described above);
   - consequences (additive surface, no IR schema change, follow-ups for
     hidden/alias/deprecated metadata and unit-variant support).
3. Add the new ADR to `docs/contents.md` next to ADR-003.
4. Update `docs/developers-guide.md` "Schema ownership" (line 18-) with a
   single sentence noting that `OrthoConfigSubcommandDocs` is part of the
   human-documentation IR and is versioned by `ORTHO_DOCS_IR_VERSION`.

Validation:

```sh
set -o pipefail
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make nixie 2>&1 \
  | tee /tmp/nixie-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
```

Expected: both commands exit successfully. Unrelated pre-existing failures
must be recorded in `Surprises & Discoveries` before continuing.

Run `coderabbit review --agent` and clear concerns.

Acceptance: ADR-004 exists, the plan is `APPROVED`, the documentation index
links to the ADR, and the developers guide mentions the new trait.

### Milestone 1: introduce the trait and the shared parsing helper

Goal: land the trait and the parsing primitives the derives need, without
emitting any new generated code.

Steps:

1. Add the trait `OrthoConfigSubcommandDocs` to
   `ortho_config/src/docs/mod.rs` (immediately after `OrthoConfigDocs`). Keep
   the file under 400 lines. Include a Rustdoc example that uses a derive
   the consumer will see (acceptable as `rust,ignore` until Milestone 3 makes
   the derive available; flip back to a compiled `rust,no_run` doctest as
   part of Milestone 3).
2. Re-export from `ortho_config/src/lib.rs` alongside the existing docs
   re-exports.
3. Lift `clap_variant_name` from
   `ortho_config_macros/src/selected_subcommand_merge.rs:27-45` into
   `ortho_config_macros/src/derive/parse/clap_attrs.rs` (export as
   `pub(crate) fn clap_variant_name(variant: &syn::Variant) ->
   syn::Result<Option<syn::LitStr>>`). Update
   `selected_subcommand_merge.rs` to call the moved helper. Keep the test
   coverage in `ortho_config_macros/src/derive/parse/tests/clap_attrs.rs`
   and add a focused case asserting the helper round-trips
   `#[command(name = "foo")]` and `#[clap(name = "foo")]`.
4. Add a new helper
   `pub(crate) fn clap_field_is_subcommand(field: &syn::Field) ->
   syn::Result<bool>` to `clap_attrs.rs`. Add unit tests for the helper
   covering `#[command(subcommand)]`, `#[clap(subcommand)]`, neither, and
   the conflict case `#[command(subcommand, long = "foo")]` (which the
   helper itself need not flag; clap rejects it at expansion time).

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
```

Expected: all four commands exit successfully. Existing tests must continue
to pass because no generator behaviour has changed yet; only the parsing
helper has moved.

Run `coderabbit review --agent` and clear concerns.

Acceptance: the trait is published, the helper is shared, no existing test
has changed in intent, and lint/format are clean.

### Milestone 2: implement the enum derive (prototyping milestone)

Goal: stand up `#[derive(OrthoConfigSubcommandDocs)]` end-to-end on a small
fixture and prove the shape of the generated `Vec<DocMetadata>` before
plumbing the struct side. This is an explicit prototyping milestone; the
fixture introduced here is permanent test code, not throwaway.

Steps:

1. Create `ortho_config_macros/src/subcommand_docs.rs` with a
   `pub(crate) fn derive_subcommand_docs(input: DeriveInput) ->
   syn::Result<TokenStream>` mirroring
   `selected_subcommand_merge::derive_selected_subcommand_merge`. The function:
   - validates `Data::Enum`;
   - iterates variants in declaration order;
   - validates single-tuple variants; rejects unit and multi-tuple variants
     with messages mirroring
     `selected_subcommand_merge.rs:47-63`;
   - resolves each variant's command label via `clap_variant_name` with a
     `heck::ToKebabCase` fallback;
   - emits an `impl #krate::docs::OrthoConfigSubcommandDocs for #ident`
     whose body returns `Vec<DocMetadata>` built by `vec![ ... ]` of
     per-variant block expressions that override `app_name` and `about_id`
     as described in the design.
2. Register the proc-macro entry point in
   `ortho_config_macros/src/lib.rs` immediately after
   `derive_selected_subcommand_merge`:

   ```rust
   #[proc_macro_derive(OrthoConfigSubcommandDocs, attributes(ortho_config))]
   pub fn derive_subcommand_docs(input_tokens: TokenStream) -> TokenStream {
       let derive_input = parse_macro_input!(input_tokens as DeriveInput);
       match subcommand_docs::derive_subcommand_docs(derive_input) {
           Ok(tokens) => tokens.into(),
           Err(err) => err.to_compile_error().into(),
       }
   }
   ```

   Add `mod subcommand_docs;` at the same place as `mod
   selected_subcommand_merge;`.
3. Add `ortho_config/tests/subcommand_docs.rs`. Use `rstest` fixtures (see
   `docs/rust-testing-with-rstest-fixtures.md`) for an enum with two
   variants, one with a `#[command(name = "...")]` override and one without.
   Assert:
   - the returned vector has length two;
   - ordering matches declaration order (not alphabetical);
   - each child's `app_name` equals the kebab-cased label or the override;
   - each child's `about_id` equals `<app_name>.about`;
   - nested cases work (a variant whose inner type itself has subcommands)
     by including a two-level fixture.
4. Add `ortho_config_macros/src/tests.rs` unit cases that drive
   `subcommand_docs::derive_subcommand_docs` directly with synthetic
   `DeriveInput` values and assert the emitted token stream contains the
   expected `<Ty as #krate::docs::OrthoConfigDocs>::get_doc_metadata()`
   calls in order. Mirror the existing parser-test pattern at
   `ortho_config_macros/src/derive/parse/tests/`.
5. Add a `trybuild` ui case under `ortho_config/tests/ui/` for each rejected
   shape (unit variant, named-field variant, multi-tuple variant). Include
   `.stderr` snapshots. Register the cases in
   `ortho_config/tests/compile_fail.rs`.

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
```

Expected: all commands pass, including the new `subcommand_docs.rs` tests
and the new `trybuild` cases.

Run `coderabbit review --agent` and clear concerns.

Acceptance: a developer can derive `OrthoConfigSubcommandDocs` on a
hand-written `clap::Subcommand` enum and observe the populated
`Vec<DocMetadata>` in tests.

### Milestone 3: extend the struct derive to recurse into subcommand fields

Goal: connect the existing `#[derive(OrthoConfig)]` to the new enum derive so
top-level config structs emit populated `subcommands` arrays.

Steps:

1. Extend `FieldAttrs`
   (`ortho_config_macros/src/derive/parse/mod.rs:74-84`) with
   `pub(crate) is_subcommand: bool`.
2. Populate it in `parse_field_attrs` (`derive/parse/mod.rs:343`) by calling
   `clap_field_is_subcommand` immediately after attribute parsing. Reject
   subcommand fields that also carry incompatible `#[ortho_config(...)]`
   options (`skip_cli`, `default`, `merge_strategy`, `cli_default_as_absent`,
   `cli_long`, `cli_short`) with a compile-time error message of the form
   `"#[command(subcommand)] fields cannot be combined with
   #[ortho_config(skip_cli)]; remove the conflicting attribute"`.
3. Add an `if attrs.is_subcommand { continue; }` early-skip guard to every
   per-field loop enumerated in the risk register above:
   - `build_cli_struct_fields`
     (`derive/build/cli/cli_flags.rs:298-323`);
   - `build_default_struct_fields`
     (`derive/build/defaults.rs:19-34`);
   - `build_default_struct_init` (`derive/build/defaults.rs:36+`);
   - `collect_collection_strategies` (`derive/build/override/mod.rs:111`);
   - `build_cli_field_metadata`
     (`derive/build/cli/cli_flags.rs:325-351`);
   - `cli_field_info` collection (`derive/build/cli_tokens.rs:80-85`);
   - `build_fields_metadata` (`derive/generate/docs/fields/mod.rs:40-74`).
   Audit `ortho_config_macros/src/derive/build/` and
   `ortho_config_macros/src/derive/generate/` for any other
   `field_attrs.iter().zip(fields.iter())` patterns and apply the same
   guard.
4. In `generate_docs_impl`
   (`derive/generate/docs/mod.rs:32-73`), compute a `subcommands_tokens`
   value:
   - iterate `args.fields.iter().zip(args.field_attrs.iter())` looking for
     `attrs.is_subcommand`;
   - if zero are found, emit the existing `Vec::new()`;
   - if exactly one is found, emit
     `<#FieldType as #krate::docs::OrthoConfigSubcommandDocs>
     ::get_subcommand_doc_metadata()`;
   - if more than one is found, emit a compile-time error stating that clap
     itself rejects multiple `#[command(subcommand)]` fields on a single
     struct.
   Substitute `subcommands: #subcommands_tokens,` for the existing literal
   `Vec::new()` on line 67.
5. Repair the deserialize bound in
   `derive/generate/ortho_impl.rs:43-46`. With the subcommand field removed
   from the generated CLI and default structs (Step 3), the assertion no
   longer reaches the enum type. Confirm by adding an `rstest` case in
   `ortho_config/tests/docs_ir.rs` (or a sibling test file) that derives
   `OrthoConfig` on a `Parser` struct whose `command: Commands` field's
   `Commands` enum does not derive `Deserialize`.
6. Update `ortho_config/tests/docs_ir.rs`:
   - introduce a fixture type with a `#[command(subcommand)]` field and a
     `Commands` enum that derives `OrthoConfigSubcommandDocs`;
   - replace the `metadata.subcommands.is_empty()` assertion at line 96 with
     an assertion that the returned vector has the expected length and
     ordering;
   - assert that the parent's `fields` array does not include the subcommand
     selector field;
   - assert nested cases (one level deep is enough for 6.1.1; deeper nesting
     is 6.1.2 work);
   - keep an "empty subcommand" case (no `#[command(subcommand)]`) so the
     existing `Vec::new()` path is still exercised.

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
```

Expected: all tests pass. The new `docs_ir.rs` assertions exercise the
populated `subcommands` array; the existing fixture for the no-subcommand
case still passes; every `trybuild` case from Milestone 2 still passes.

Run `coderabbit review --agent` and clear concerns.

Acceptance: a consumer can derive `OrthoConfig` on a top-level `Parser`
struct with a `#[command(subcommand)]` field and observe a populated
`subcommands` array in the returned `DocMetadata`.

### Milestone 4: behavioural coverage and renderer smoke tests

Goal: prove the data reaches the renderers and survives the bridge pipeline.
Defer fixture-rich behavioural tests for nested trees and Windows wrapper
output to roadmap item 6.1.2.

Steps:

1. Extend `ortho_config/tests/features/docs_ir.feature` with at least two
   new scenarios:
   - "Subcommand metadata is recursively populated" (asserts the
     subcommands array length, ordering, and the inner command's
     `app_name`);
   - "Commands heading id is emitted when subcommands exist" (asserts
     `HeadingIds.commands` is populated and resolves through the localizer).
2. Add the matching step definitions to
   `ortho_config/tests/rstest_bdd/behaviour/steps/docs_steps.rs` (and
   supporting fixtures in the same module tree). Keep assertions
   user-observable per `docs/developers-guide.md` "Adding or changing
   behavioural tests".
3. Extend `cargo-orthohelp/src/schema/tests.rs::sample_metadata()` (line
   28-) to include a non-empty subcommand vector. Confirm the existing
   round-trip test continues to pass.
4. Add a renderer smoke test in `cargo-orthohelp` (under
   `cargo-orthohelp/src/roff/tests.rs` or a new `tests` submodule of
   `cargo-orthohelp/src/powershell/`) that drives the existing renderer
   public entry points with a `LocalizedDocMetadata` containing two
   subcommands and asserts the output is non-empty and includes the child
   command names. The aim is to prove the existing recursion paths do not
   panic on populated data; deep rendering assertions belong to 6.1.2.
5. Update `ortho_config/examples/registry_ctl.rs` and at least one
   `examples/hello_world` binary so that:
   - the subcommand enum derives `OrthoConfigSubcommandDocs`;
   - the example's IR-dumping helper (if any) prints the populated
     subcommand tree.

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make nixie 2>&1 \
  | tee /tmp/nixie-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
```

Expected: all five commands exit successfully; the new behavioural
scenarios pass; the renderer smoke tests pass; the updated examples
compile.

Run `coderabbit review --agent` and clear concerns.

Acceptance: behavioural coverage proves the populated tree reaches the
renderers, and the reference examples demonstrate the recommended pattern.

### Milestone 5: documentation, changelog, roadmap close-out

Goal: bring documentation in line with the implementation and mark the
roadmap entry done.

Steps:

1. Update `docs/design.md`:
   - extend §4.2 "The `#[derive(OrthoConfig)]` Macro" with a paragraph
     describing the new subcommand-field detection;
   - add a dated entry to §9 "Decision log" in the same style as the
     2025-12-19 "Merge selected subcommand enums" entry at
     `docs/design.md:933-944`, citing ADR-004.
2. Update `docs/cargo-orthohelp-design.md`:
   - §2.1 (top-level metadata): explain that `subcommands` is now populated
     by `OrthoConfigSubcommandDocs`;
   - §3.1 (trait): add a sibling block describing the new trait alongside
     `OrthoConfigDocs`;
   - §3.5 (implementation notes): add a bullet covering subcommand recursion
     and naming defaults;
   - §13.1 (IR JSON excerpt): include a non-empty `subcommands` example.
3. Update `docs/agent-native-cli-design.md`:
   - §4 (Whole-CLI introspection): rewrite the paragraph at lines 257-275 to
     state the gap is now closed and cite the new trait;
   - §9 (Current gaps to resolve): strike or update the bullet at line 613
     ("generated `OrthoConfigDocs` subcommand metadata is currently empty").
4. Update `docs/users-guide.md`:
   - in the "Documentation metadata (OrthoConfigDocs)" section, add a
     subsection explaining the new derive on subcommand enums and showing
     usage on the `Commands` enum referenced by the existing "Subcommand
     configuration" and "Merging a selected subcommand enum" walkthroughs.
5. Update `CHANGELOG.md` "Unreleased / Added" with:
   - "`OrthoConfigSubcommandDocs` trait and derive (`ortho_config_macros`)";
   - "Recursive `DocMetadata.subcommands` population from top-level
     `OrthoConfig` structs that hold a `#[command(subcommand)]` field".
6. Update `ortho_config/README.md` and `ortho_config_macros/README.md`:
   - extend the derive list to include `OrthoConfigSubcommandDocs`;
   - add a short example after the existing subcommand sections so
     consumers see the recommended pattern.
7. Update `docs/v0-8-0-migration-guide.md` if it exists, or note the
   additive nature of the change under the appropriate "Migration notes" in
   `ortho_config/README.md`. No breaking-change entry is needed.
8. Mark the relevant `docs/roadmap.md` entries done:
   - `[x] 6.1.1. Generate recursive DocMetadata.subcommands values.` (line
     116);
   - `[x] Reuse information already parsed by SelectedSubcommandMerge ...`
     (line 117) — satisfied by the lifted `clap_variant_name` helper;
   - `[x] Introduce a small companion trait ...` (line 119) — satisfied by
     `OrthoConfigSubcommandDocs`;
   - `[x] Preserve deterministic command ordering ...` (line 121) —
     satisfied by declaration-order iteration.

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
make nixie 2>&1 \
  | tee /tmp/nixie-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
```

Expected: all five commands pass. Run `coderabbit review --agent` and
clear concerns. Move the draft pull request out of draft state once the
maintainer has reviewed.

Acceptance: the roadmap entry is closed, every doc references the new trait
consistently, the draft PR is moved to ready-for-review, and the change has
landed on `main`.

## Concrete steps

The following commands are the canonical operations a fresh agent should run.
They are deliberately idempotent: re-running them after a partial failure
recreates the same state without drift.

Repository-orientation (read-only):

```sh
git fetch origin
git branch --show-current
ls docs/execplans/6-1-1-recursive-doc-metadata-subcommands-values.md
```

Per-milestone validation (sequential, with `tee`):

```sh
set -o pipefail

make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out

make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out

make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out

make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out

make nixie 2>&1 \
  | tee /tmp/nixie-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
```

Targeted iteration during development:

```sh
cargo test -p ortho_config_macros --tests
cargo test -p ortho_config --tests
cargo test -p cargo-orthohelp --tests
```

CodeRabbit gate after each milestone:

```sh
coderabbit review --agent 2>&1 \
  | tee /tmp/coderabbit-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out
```

Branch hygiene (only after maintainer approval; do not run while plan is
DRAFT):

```sh
git push -u origin 6-1-1-recursive-doc-metadata-subcommands-values
```

Draft pull-request creation (once Milestone 0 completes; see PR template
notes at the end of this document):

```sh
gh pr create --draft \
  --title "Plan: recursive DocMetadata.subcommands (6.1.1)" \
  --body-file /tmp/pr-body-6-1-1-recursive-doc-metadata-subcommands-values.md
```

Update this `Concrete steps` section whenever a milestone changes the
commands a new contributor must run.

## Validation and acceptance

A change implementing this plan is "done" when all of the following hold,
verified by the commands above:

- Tests
  - `make test` passes from a clean checkout, with the new
    `ortho_config/tests/subcommand_docs.rs`, updated
    `ortho_config/tests/docs_ir.rs`, new feature scenarios in
    `ortho_config/tests/features/docs_ir.feature`, new step definitions,
    expanded `cargo-orthohelp/src/schema/tests.rs` round-trip, and renderer
    smoke tests passing.
  - `trybuild` UI cases under `ortho_config/tests/ui/` cover unit variant,
    named-field variant, and multi-tuple variant rejections, plus the
    "subcommand field whose inner type does not implement
    `OrthoConfigSubcommandDocs`" case.
  - Macro-internals unit tests in `ortho_config_macros/src/tests.rs` assert
    the expected token shapes.
- Lint and format
  - `make check-fmt` passes.
  - `make lint` passes (including `clippy::pedantic` rules already enabled
    via workspace lints; no new `#[allow]` or `#[expect]` annotations are
    needed beyond those already in place).
  - `make markdownlint` passes for all new and edited markdown.
- Documentation rendering
  - `make nixie` passes for diagrams referenced from the design and ADR.
- Behaviour
  - `cargo run -p cargo-orthohelp --bin cargo-orthohelp -- --format ir`
    against a consumer crate that opts into the new derive emits a JSON
    document with a populated `subcommands` array; the array preserves
    declaration order.
  - The same invocation with `--format man` and `--format ps` emits
    sections referring to the child subcommands without bespoke per-app
    glue code.
- Process
  - `coderabbit review --agent` is clean at the end of each milestone.
  - The draft pull request is moved out of draft state by the maintainer
    after a review pass.
  - `docs/roadmap.md` `[ ] 6.1.1. ...` is updated to `[x]`.

Quality criteria for "done":

- Public API: only additive items (`OrthoConfigSubcommandDocs` trait,
  `OrthoConfigSubcommandDocs` derive) appear. No existing public item is
  renamed or removed.
- Schema versions: unchanged.
- Files: no Rust file exceeds 400 lines; every new module starts with `//!`.
- Language: en-GB-oxendict spelling and grammar in documentation and
  comments.
- Tests: no fixture is shared mutably across scenarios; `#[once]` is used
  only for effectively read-only infrastructure.

## Idempotence and recovery

- All validation steps are read-only and re-runnable.
- All edits to source code, documentation, and roadmap are tracked by git;
  recovery from a half-completed milestone is `git status` followed by
  reverting unstaged changes or committing the partial work as a checkpoint.
- `coderabbit review --agent` is idempotent per branch state; re-running it
  after addressing comments produces a fresh report.
- The draft pull request can be force-recreated only by closing the
  existing draft; do not delete the branch unless the maintainer approves
  the destructive operation.
- The renamed branch
  `6-1-1-recursive-doc-metadata-subcommands-values` tracks
  `origin/6-1-1-recursive-doc-metadata-subcommands-values`; re-pushing is
  safe because no other agent or human is expected to push to that branch.

## Artefacts and notes

This section captures evidence that helped shape the plan. Update it as
implementation proceeds (transcripts of failing test runs, comparative
output before and after a change, etc.).

- The existing literal `subcommands: Vec::new()` lives at
  `ortho_config_macros/src/derive/generate/docs/mod.rs:67`. That single line
  is the focal point of the change; every other touched site exists either
  to allow the substitution (by suppressing per-field code generation for
  the subcommand selector) or to demonstrate the result (by populating it).
- The bridge writes the following `main.rs` template
  (`cargo-orthohelp/src/bridge.rs:171-208`):

  ```rust
  use ortho_config::docs::OrthoConfigDocs;

  fn main() -> Result<(), Box<dyn std::error::Error>> {
      let metadata = <RootType as OrthoConfigDocs>::get_doc_metadata();
      serde_json::to_writer(std::io::stdout(), &metadata)?;
      Ok(())
  }
  ```

  No change to the bridge template is required; the `metadata` value will
  carry populated `subcommands` automatically once the consumer's
  `RootType` is a top-level `Parser` struct with `#[command(subcommand)]`
  and the new derive.

- ADR-003
  (`docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`)
  governs the ownership boundary; ADR-004 will sit alongside it and inherit
  its versioning rules.

## Interfaces and dependencies

Be prescriptive about names and locations. The end-state public surface
introduced by this plan is exactly:

```rust
// ortho_config/src/docs/mod.rs (next to OrthoConfigDocs)
pub trait OrthoConfigSubcommandDocs {
    fn get_subcommand_doc_metadata() -> Vec<DocMetadata>;
}
```

```rust
// ortho_config_macros/src/lib.rs (next to derive_selected_subcommand_merge)
#[proc_macro_derive(OrthoConfigSubcommandDocs, attributes(ortho_config))]
pub fn derive_subcommand_docs(input_tokens: TokenStream) -> TokenStream { /* ... */ }
```

```rust
// Consumer code, after this plan ships
use clap::{Parser, Subcommand};
use ortho_config::{OrthoConfig, OrthoConfigSubcommandDocs};
use serde::{Deserialize, Serialize};

#[derive(Parser, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    // global flags here, as normal OrthoConfig fields
}

#[derive(Subcommand, OrthoConfigSubcommandDocs)]
enum Commands {
    Run(RunArgs),
    #[command(name = "take-leave")]
    TakeLeave(TakeLeaveArgs),
}

#[derive(Parser, Serialize, Deserialize, Default, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct RunArgs { /* ... */ }

#[derive(Parser, Serialize, Deserialize, Default, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct TakeLeaveArgs { /* ... */ }
```

No new external crate dependency is introduced. The implementation reuses:

- `heck = "0.5.0"` (already in `ortho_config_macros/Cargo.toml:21`) for
  `ToKebabCase`;
- `syn`, `quote`, `proc-macro2` (already declared) for the new derive;
- `rstest`, `rstest-bdd`, `figment`, `serde_json` (already declared) for new
  tests.

The new trait depends on no other module; the new derive depends only on
`syn`/`quote`/`proc-macro2`/`heck` and on the shared helpers in
`ortho_config_macros/src/derive/parse/clap_attrs.rs`,
`ortho_config_macros/src/derive/crate_path.rs`.

## Progress

Use a list with checkboxes to summarise granular steps. Every stopping point
must be documented here, even if it requires splitting a partially completed
task into two ("done" vs. "remaining"). This section must always reflect the
actual current state of the work.

- [x] (2026-05-23 ??:??Z) Draft ExecPlan created.
- [x] (2026-05-24 12:40Z) Plan approved by maintainer instruction to
  proceed with implementation; status set to `APPROVED`.
- [x] (2026-05-24 12:58Z) Milestone 0 complete (ADR-004 drafted,
  contents index updated,
  developers guide note added, markdownlint and nixie clean, CodeRabbit
  clear).
- [ ] Milestone 1 complete (trait published, shared parsing helper lifted,
  helper unit-tested).
- [ ] Milestone 2 complete (enum derive emits populated `Vec<DocMetadata>`,
  trybuild cases cover rejected shapes).
- [ ] Milestone 3 complete (struct derive recurses; existing tests still
  green; new `docs_ir.rs` cases pass).
- [ ] Milestone 4 complete (behavioural scenarios pass; renderer smoke
  tests pass; examples updated).
- [ ] Milestone 5 complete (documentation, changelog, README updates,
  roadmap entry marked done).
- [ ] Draft pull request moved to ready-for-review.
- [ ] Pull request merged into `main`.

Use timestamps to detect tolerance breaches and to feed retrospectives.

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as risks.
Document with evidence so future work benefits.

- Observation: `make fmt` failed during Milestone 0 because
  `markdownlint --fix` reports pre-existing line-length violations across
  unrelated Markdown files, including `cargo-orthohelp/README.md`,
  `docs/behavioural-testing-in-rust-with-cucumber.md`,
  `docs/repository-layout.md`, and `docs/rstest-bdd-users-guide.md`.
  Evidence:
  `/tmp/fmt-ortho-config-6-1-1-recursive-doc-metadata-subcommands-values.out`.
  Impact: the failed formatter run was reverted for unrelated files; the
  milestone validation will still run the planned gates and record exact
  failures if they persist.

## Decision log

Record every significant decision made while working on the plan. Include
decisions to escalate, decisions on ambiguous requirements, and design
choices.

- Decision: introduce a new public trait `OrthoConfigSubcommandDocs` rather
  than extending `OrthoConfigDocs` or asking consumers to combine
  `Vec<DocMetadata>` manually. Rationale: the roadmap leaves the choice open
  ("Introduce a small companion trait if enum-level documentation cannot be
  represented cleanly through the existing `OrthoConfigDocs` trait", roadmap
  line 119-120). Extending `OrthoConfigDocs` would require the trait to
  return per-type metadata that distinguishes "I am a config struct" from
  "I am a subcommand selector enum", which is a single-method trait change
  the existing IR schema does not need. A companion trait is the smallest
  additive surface and decouples docs concerns from `SelectedSubcommandMerge`
  (which is `serde_json`-gated). Date/Author: 2026-05-23 (planner).
- Decision: keep the new derive separate from `SelectedSubcommandMerge` and
  share only `clap_variant_name` and (potentially) an unvalidated variant
  iterator. Rationale: the two derives have different validation rules.
  `SelectedSubcommandMerge` rejects unit variants because it has nowhere to
  merge configuration for them; docs would naturally accept them (a
  subcommand with no flags is a valid clap pattern). Sharing only the
  parsing primitives keeps each derive's validation honest. The current
  scope keeps the same single-tuple constraint to minimize risk; lifting it
  is a deferred follow-up. Date/Author: 2026-05-23 (planner).
- Decision: default the variant command label to
  `variant.ident.to_string().to_kebab_case()` via `heck::ToKebabCase`,
  matching clap's own default. Honour `#[command(name = "...")]` (and the
  `#[clap(name = "...")]` synonym) as an override. Rationale: matches clap's
  observable behaviour, reuses an existing dependency, and avoids a
  divergent IR convention. Date/Author: 2026-05-23 (planner).
- Decision: defer support for hidden, aliased, and deprecated subcommand
  metadata. Rationale: the IR schema does not model them today, and adding
  schema fields requires a separate ADR and a `ORTHO_DOCS_IR_VERSION` bump.
  Date/Author: 2026-05-23 (planner).
- Decision: defer support for unit variants of `clap::Subcommand` enums.
  Rationale: unit variants must still produce a `DocMetadata` entry, but
  their `app_name` resolution path is different (no inner Args struct to
  source headings, fields, and Fluent IDs from), and the v1 scope mirrors
  `SelectedSubcommandMerge`'s constraint. Lifting it later is additive.
  Date/Author: 2026-05-23 (planner).
- Decision: keep the bridge unchanged. Rationale: the bridge already
  invokes `<RootType as OrthoConfigDocs>::get_doc_metadata()` and
  serializes the full recursive structure; populating `subcommands` happens
  inside the derive output, not at the bridge boundary. Date/Author:
  2026-05-23 (planner).
- Decision: treat the maintainer's 2026-05-24 instruction to proceed with
  implementation as explicit approval of this restored ExecPlan. Rationale:
  the requested implementation names this exact plan and asks that it be kept
  up to date, which supersedes the restored draft status without changing the
  technical scope. Date/Author: 2026-05-24 (Codex).

## Outcomes & retrospective

Summarise outcomes, gaps, and lessons learned at major milestones or at
completion. Compare the result against the original purpose. Note what
would be done differently next time.

- Outcome: not yet recorded; this plan has not been approved or
  implemented.

## Notes for the accompanying draft pull request

When opening the draft pull request that ships this ExecPlan, follow these
guidelines (the actual PR is opened by the agent at the close of plan
authoring, not by an implementer):

- Title: `Plan: recursive DocMetadata.subcommands (6.1.1)`.
- Body must mention this plan file
  (`docs/execplans/6-1-1-recursive-doc-metadata-subcommands-values.md`) and
  the roadmap entry `(6.1.1)`.
- Body must include a `## References` section that links to the lody
  session via the `LODY_SESSION_ID` environment variable.
- Mark the pull request as draft until the plan is approved; mark it
  ready-for-review once approval is recorded in `Decision Log` and the
  status field above is updated to `APPROVED`.

## Revision history

This section records edits to this plan after the first draft. Each entry
must state what changed, why it changed, and how it affects remaining work.

- 2026-05-23 (planner): initial draft created.
- 2026-05-24 (Codex): restored the plan from repository history into this
  worktree, recorded implementation approval from the maintainer's latest
  instruction, and set status to `APPROVED`.
