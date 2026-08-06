# Add the `OrthoConfigLocalization` trait and derive emission (11.1.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Today an application using `#[derive(OrthoConfig)]` that wants localized
command-line help must hand-author every Fluent identifier: it calls
`ortho_config::message_id_for(["hello_world", "cli"], "about")` (or worse,
concatenates strings) and hopes the result matches what the runtime localizer
looks up. Nothing checks the identifiers at compile time, and a typo or a
collision between two fields only surfaces as a runtime panic (see
`docs/adr-006-identifier-derivation-panics.md`).

After this change:

- Every `#[derive(OrthoConfig)]` struct implements a new public trait,
  `OrthoConfigLocalization`, carrying the command's Fluent identifiers as
  associated constants: `ABOUT_ID`, `LONG_ABOUT_ID`, `USAGE_ID`, and a
  per-argument `ARG_IDS` table of `(help_id, long_help_id, value_name_id)`
  triples. Application code and tests refer to identifiers by constant, never
  by string concatenation.
- Two fields whose identifiers normalize to the same Fluent id fail to
  *compile*, with the error pointing at the offending field, instead of
  panicking at runtime. This discharges the derive-time guard promised in
  ADR-006.
- The documentation intermediate representation (IR) emitted by the derive
  (`OrthoConfigDocs::get_doc_metadata`) reports the same identifiers as the
  localization trait, so the docs pipeline, the runtime localizer, and
  application code agree byte-for-byte.
- When explicitly requested, the derive emits a build-time inventory of every
  generated identifier at `${OUT_DIR}/ortho-config/cli-identifiers.json` (split
  across capped files for large trees) for consumption by `cargo-orthohelp` and
  translator tooling.

Observable success: the fixture and example crates compile with derived
identifier constants; `ortho_config/tests/localized_parse.rs` proves the
derived constants match `message_id_for` output over a real
`#[derive(OrthoConfig)]` tree; a trybuild compile-fail test shows the collision
diagnostic; and an end-to-end test drives `cargo build` on a fixture crate and
inspects the emitted JSON artefact.

This is roadmap item 11.1.3 (`docs/roadmap.md`, "Promote and widen the CLI
localization surface"). The governing design is
`docs/cli-localization-design.md` §8.1 and §8.2, with the identifier convention
in §4.1.

## Constraints

- Do not create a dependency from `ortho_config_macros` on `ortho_config`.
  The macro crate is a `proc-macro` crate consumed by `ortho_config`; a reverse
  dependency is circular and forbidden. Domain boundaries between workspace
  crates must hold.
- The identifier convention is fixed by
  `docs/cli-localization-design.md` §4.1 and implemented by
  `ortho_config::message_id_for` (`ortho_config/src/localizer/identifier.rs`).
  Derive-generated identifiers must agree byte-for-byte with `message_id_for`
  output. The convention itself must not change.
- The runtime localization surface shipped by 11.1.1 and 11.1.2
  (`LocalizeCmd`, `LocalizedParse`, `parse_localized_command`,
  `message_id_for`) must remain source-compatible. Additive changes only.
- The derive must not write to the filesystem during ambient builds. Artefact
  emission is opt-in (see Decision Log D-3): it requires both `OUT_DIR` to be
  present and an explicit environment opt-in. Unconditional proc-macro
  filesystem writes break `docs.rs` (read-only sandbox), `rust-analyzer`, and
  the nightly derive-expansion cache, and are contrary to Cargo team guidance
  (rust-lang/cargo#9084).
- The workspace lint policy is strict (`clippy::unwrap_used`,
  `expect_used`, `indexing_slicing`, `panic_in_result_fn`, `missing_docs`, and
  friends are denied). New code must pass `make check-fmt`, `make typecheck`,
  `make lint`, and `make test` at every milestone.
- All prose follows `docs/documentation-style-guide.md` and en-GB-oxendict
  spelling; Markdown is gated by `make markdownlint` and `make nixie`.
- No single code file may exceed 400 lines.

## Tolerances (exception triggers)

- Scope: if the implementation (excluding tests, fixtures, snapshots, and
  documentation) requires changes to more than 25 files or more than ~2,500 net
  lines, stop and escalate.
- Interface: if any *existing* public API signature must change (as opposed
  to additive surface), stop and escalate. The docs IR content change in
  Milestone 4 is pre-authorized by this plan (Decision Log D-2); any change
  beyond identifier values and the IR version string is not.
- Dependencies: adding `serde`/`serde_json` as unconditional dependencies of
  `ortho_config_macros` is pre-authorized (they are already optional
  dependencies). Any other new external dependency: stop and escalate.
- Iterations: if a gate still fails after three fix attempts on the same
  failure, stop, record the failure mode, and escalate.
- Ambiguity: if the docs-IR reconciliation (Milestone 4) turns out to require
  changes to `cargo-orthohelp` beyond regenerating golden snapshots, stop and
  present options before touching generator logic.
- CodeRabbit: if `coderabbit review --agent` stalls at `preparing_sandbox`
  for more than two attempts (a known failure mode recorded in the 11.1.1 and
  11.1.2 ExecPlans), record the attempt and continue; do not block the
  milestone on it.

## Risks

- Risk: divergence between the macro crate's identifier normalization and
  `ortho_config::message_id_for` (they cannot share code; see Constraints).
  Severity: high. Likelihood: medium. Mitigation: cross-crate agreement tests in
  `ortho_config/tests/` compare derive output against `message_id_for` across
  a fixture tree (Milestone 3), plus shared documented rules in
  `docs/developers-guide.md` naming both implementations. A property test
  drives both functions with generated segment lists.
- Risk: the docs IR identifier change (dotted `{app}.fields.{field}.help` to
  dashed `{app}-args-{field}-help`) breaks consumers of the IR beyond the
  workspace. Severity: medium. Likelihood: medium. Mitigation: bump
  `ORTHO_DOCS_IR_VERSION`, update all pinned snapshots in one commit, record
  the change in `CHANGELOG.md` and the users' guide, and keep explicit `help_id`
  /`about_id` attribute overrides working unchanged.
- Risk: artefact emission interacts badly with parallel or incremental
  compilation (multiple derive expansions writing one file). Severity: medium.
  Likelihood: medium. Mitigation: per-type fragment files merged
  deterministically (Milestone 5 design), gated behind an explicit opt-in so
  ambient builds never write.
- Risk: `make lint` (Whitaker suite) may be red on `main` for reasons
  unrelated to this change. Severity: low. Likelihood: medium. Mitigation:
  establish the baseline in Milestone 0; only failures citing files in this
  branch's diff block progress.
- Risk: trybuild `.stderr` expectations are toolchain-sensitive.
  Severity: low. Likelihood: medium. Mitigation: follow the existing harness
  patterns in `ortho_config/tests/compile_fail.rs` and pin expectations to the
  workspace toolchain; regenerate with `TRYBUILD=overwrite` when needed.

## Progress

- [ ] Milestone 0: baseline gates and orientation.
- [ ] Milestone 1: `OrthoConfigLocalization` trait in `ortho_config`.
- [ ] Milestone 2: macro-side identifier generation and collision detection.
- [ ] Milestone 3: derive emission of `OrthoConfigLocalization` impls plus
  cross-crate agreement tests.
- [ ] Milestone 4: docs IR delegation to the localization identifiers.
- [ ] Milestone 5: opt-in build-time identifier artefact.
- [ ] Milestone 6: documentation, ADR-008, roadmap completion, final gates.

## Surprises & discoveries

- Observation: `OUT_DIR` is not set during proc-macro expansion unless the
  *consuming* crate has a `build.rs`, and the Cargo team explicitly rejects
  proc macros persisting data to disk (rust-lang/cargo#9084, closed wontfix;
  rust-lang/cargo#14035 likewise). Nightly now caches derive expansions, so a
  cached expansion never runs the macro body and any ambient side effect
  silently stops happening. Evidence: pre-planning research pass over Cargo
  issue tracker and internals threads, 2026-08-06. Impact: the design
  document's §8.2 artefact bullet cannot be implemented as an unconditional
  write; Decision D-3 narrows it to an explicit opt-in and ADR-008 records the
  reasoning.
- Observation: the existing docs derive emits dotted identifiers
  (`{app}.about`, `{app}.fields.{field}.help`) under a `fields.` namespace,
  while `message_id_for` emits dash-joined identifiers under an `args.`
  namespace (`{app}-args-{field}-help`). Evidence:
  `ortho_config_macros/src/derive/generate/docs/sections.rs` and
  `docs/fields/defaults.rs` versus `ortho_config/src/localizer/identifier.rs`.
  Impact: Milestone 4 is a reconciliation, not a mechanical delegation; every
  pinned IR snapshot changes and `ORTHO_DOCS_IR_VERSION` must be bumped.

## Decision log

- Decision D-1: the "blanket `OrthoConfigDocs` impl" named in the roadmap is
  realized as *generated delegation*, not a literal Rust blanket impl.
  Rationale: a blanket `impl<T: OrthoConfigLocalization> OrthoConfigDocs for T`
  is impossible — it would conflict with the derive-emitted `OrthoConfigDocs`
  impls under coherence rules, and `get_doc_metadata` requires per-field data
  that associated constants cannot supply. Instead, the derive's docs generator
  populates `about_id`, `help_id`, `long_help_id`, and `value_name` identifier
  defaults by referencing `<Self as OrthoConfigLocalization>` constants in the
  emitted code, which guarantees byte-for-byte agreement by construction. The
  roadmap intent ("the docs IR picks up the same identifiers") is met exactly.
  Date/Author: 2026-08-06, planning session.
- Decision D-2: the docs IR identifier values change from the dotted
  `{app}.fields.{field}.help` form to the canonical `message_id_for` form
  (`{app}-args-{field}-help`), and `ORTHO_DOCS_IR_VERSION` bumps from "1.1" to
  "1.2". Explicit `help_id`/`long_help_id`/`about_id` attribute overrides keep
  working unchanged. Rationale: two identifier conventions for the same strings
  is precisely the defect this roadmap item exists to remove; the IR version
  field exists so consumers can detect the change. The alternative (keeping
  dotted IR ids and translating in `cargo-orthohelp`) preserves the split brain
  forever. Date/Author: 2026-08-06, planning session; requires plan approval
  since IR output is externally observable.
- Decision D-3: the `${OUT_DIR}/ortho-config/cli-identifiers.json` artefact
  is emitted only when both (a) `OUT_DIR` is present in the environment at
  expansion time and (b) `ORTHO_CONFIG_EMIT_IDENTIFIERS=1` is set. Ambient
  `cargo build`/`cargo check`/rust-analyzer runs never write. The authoritative
  consumption path for `cargo-orthohelp` (§11 of the design, a later roadmap
  item) is the compiled-in constants via its existing bridge shim; the JSON
  artefact is a translator-tooling convenience. Rationale: prior art (sqlx's
  `cargo sqlx prepare` gating, uniffi's extract-from-binary model) and Cargo
  team guidance both reject ambient proc-macro writes; an env-gated write
  neutralizes the docs.rs, rust-analyzer, and expansion-cache failure modes
  while still honouring the design document's artefact contract. Recorded as
  ADR-008 in Milestone 6. Date/Author: 2026-08-06, planning session.
- Decision D-4: `localized_default` embedding (design §8.2, second bullet) is
  out of scope. The roadmap checklist for 11.1.3 does not include it; the
  artefact schema reserves an `embedded_default` field that this task always
  emits as `null`. Rationale: keeps the milestone bounded; the field
  reservation means the artefact schema does not change when embedding lands.
  Date/Author: 2026-08-06, planning session.
- Decision D-5: the derive gains an optional struct-level attribute
  `#[ortho_config(localization_base = "hello_world.cli")]` naming the
  identifier root as a dotted path. When absent, the base defaults to the same
  application-name resolution the docs derive already uses (the `app_name` doc
  attribute when present, otherwise the kebab-cased struct name). Rationale:
  11.1.1 shipped `LocalizeCmd::with_base` for multi-segment catalogue roots and
  the `hello_world` example deliberately kept its `hello_world.cli` base for
  this task (11.1.2 Decision D-1). Without a derive-side equivalent, the
  generated constants would be wrong for every application that uses
  `with_base`. Date/Author: 2026-08-06, planning session.
- Decision D-6: no Kani or Verus harnesses. The interesting invariants
  (normalization agreement between two implementations; split-file round-trip)
  range over unbounded strings and are exercised with `proptest` property tests
  instead. There is no introduced lemma or contractual business logic that a
  bounded model check or proof would cover more rigorously than the property
  tests plus the cross-crate agreement suite. Date/Author: 2026-08-06, planning
  session.

## Outcomes & retrospective

(To be completed as milestones land.)

## Context and orientation

The workspace (`Cargo.toml`, version 0.8.0, edition 2024) contains:

- `ortho_config/` — the main library crate. The localization runtime lives
  under `ortho_config/src/localizer/`: the `Localizer` trait and Fluent
  implementations (`mod.rs`, `fluent.rs`), the identifier convention
  (`identifier.rs`: `message_id_for` public, `normalize_segment`
  crate-private), command-tree localization (`clap_command/mod.rs`:
  `LocalizeCmd`, `WithBase`, per-parent collision `assert!`s), and localized
  parsing (`clap_command/parse.rs`: `LocalizedParse`,
  `parse_localized_command`). The docs IR lives under `ortho_config/src/docs/`
  (`OrthoConfigDocs` with a single `get_doc_metadata()` method, `DocMetadata`
  and friends in `ir.rs`, `ORTHO_DOCS_IR_VERSION = "1.1"`).
- `ortho_config_macros/` — the proc-macro crate implementing
  `#[derive(OrthoConfig)]` (`src/lib.rs`, `derive_ortho_config`). Parsing lives
  under `src/derive/parse/` (`StructAttrs`, `FieldAttrs`, `clap_attrs.rs` with
  `clap_arg_id` reading `#[arg(id = "…")]`); generation under
  `src/derive/generate/` (docs emission in `generate/docs/`, with the spanned
  duplicate-id helper `ensure_unique` in `generate/docs/fields/validation.rs`).
  Kebab-casing uses `heck` (`derive/build/cli/cli_flags.rs`). Errors are
  reported as `syn::Result` converted via `to_compile_error()` — never `panic!`.
- `cargo-orthohelp/` — the docs/agent-context CLI; consumes the IR through a
  generated bridge shim (`src/bridge.rs`).
- `examples/hello_world/` — the canonical localized example (catalogue base
  `hello_world.cli`, set via `with_base`); insta snapshots under
  `examples/hello_world/tests/snapshots/`.
- `tests/fixtures/orthohelp_fixture/` — a workspace-member fixture crate with
  its own `locales/` tree.
- `test_helpers/` — shared test utilities (`ortho_config_test_helpers`).

Terms used below:

- "Fluent" is the Mozilla localization system; an FTL (Fluent Translation
  List) file maps identifiers to translated strings. A Fluent identifier matches
  `[a-zA-Z][a-zA-Z0-9_-]*`.
- "Identifier convention" (design §4.1): author-facing FTL keys are dotted
  (`hello_world.cli.about`); the runtime id joins normalized segments with `-`
  (`hello_world-cli-about`). Argument ids insert an `args` segment:
  `hello_world-cli-args-recipient-help`.
- "Docs IR": the JSON-serializable `DocMetadata` structure the derive emits
  for documentation generators.

Prior work this task builds on: 11.1.1 promoted `LocalizeCmd` and
`message_id_for`; 11.1.2 promoted `LocalizedParse` and added an
identifier-coverage test (`ortho_config/tests/localized_parse.rs`, the
`RecordingLocalizer` at line ~47 and
`identifier_coverage_matches_message_id_for` at line ~360) that this task
extends. ADR-006 records that runtime identifier derivation panics on invalid
or colliding segments *until this task* adds the compile-time guard.

Relevant skills for the implementer: `leta` (code navigation), `rust-router`
(then `rust-types-and-apis` for the trait surface and `rust-unit-testing` for
the test work), `execplans` (this document's maintenance), `commit-message`,
`comenq-coderabbit` (review loop), `arch-decision-records` (ADR-008), and
`en-gb-oxendict` (prose). Relevant repository documentation: `docs/design.md`,
`docs/cli-localization-design.md`, `docs/localizable-rust-libraries-with-`
`fluent.md`, `docs/rust-testing-with-rstest-fixtures.md`,
`docs/rstest-bdd-users-guide.md`, `docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`, and
`docs/complexity-antipatterns-and-refactoring-strategies.md`.

## Plan of work

### Milestone 0: baseline and orientation

No code changes. Run the four gates (`make check-fmt`, `make typecheck`,
`make lint`, `make test`) via the `scrutineer` subagent to record the baseline,
especially whether the Whitaker lint gate is red on files outside this task's
diff (a known possibility). Record the result in `Progress`.

### Milestone 1: the `OrthoConfigLocalization` trait

New file `ortho_config/src/localizer/localization_ids.rs` (module
`localization_ids`, registered in `ortho_config/src/localizer/mod.rs`,
re-exported from `ortho_config/src/lib.rs` alongside the existing localizer
exports):

```rust
/// Compile-time Fluent identifiers for a derived command-line surface.
pub trait OrthoConfigLocalization {
    /// Identifier for the command's `about` text.
    const ABOUT_ID: &'static str;
    /// Identifier for `long_about`.
    const LONG_ABOUT_ID: &'static str;
    /// Identifier for the override usage string.
    const USAGE_ID: &'static str;
    /// Identifier triples for every argument, in declaration order. Each
    /// element is `(help_id, long_help_id, value_name_id)`.
    const ARG_IDS: &'static [(&'static str, &'static str, &'static str)];
}
```

Red: a unit test module (rstest, `googletest` assertions) with a handwritten
impl asserting the constants round-trip through a `FluentLocalizer` lookup,
plus a doc example. The test fails to compile until the trait exists; use the
smallest possible hand impl, then make it pass. A trybuild "pass" case under
`ortho_config/tests/trybuild/` locks the public path
`ortho_config::OrthoConfigLocalization`.

Validation: `cargo test -p ortho-config` (via `make test`), then the full gate
set. Commit.

### Milestone 2: macro-side identifier generation and collision detection

New module `ortho_config_macros/src/derive/generate/localization/` with:

- `identifier.rs`: a strict segment normalizer mirroring
  `ortho_config::localizer::identifier::normalize_segment` (lowercase ASCII
  alphanumerics pass through, `-` and `_` pass through, anything else is an
  error; empty segments are errors; the *joined* identifier must start with an
  ASCII letter). Unlike the runtime twin it returns `syn::Result<String>` with
  errors spanned to the offending field or attribute
  (`syn::Error::new_spanned`), never panicking. Document, in both files' module
  comments, that the two implementations must agree and that
  `ortho_config/tests/` locks the agreement.
- `mod.rs`: the identifier-generation pass. Inputs: the resolved base
  segments (Decision D-5: `localization_base` attribute split on `.`, else the
  docs derive's application-name resolution), and the field list. For each
  non-subcommand, non-`skip_cli` field, the argument id is the field's clap
  `id` override (`clap_arg_id` in `derive/parse/clap_attrs.rs`) or the
  kebab-cased field name (same rule as `cli_flags.rs`). Outputs a struct-shaped
  model (`LocalizationIds { about, long_about, usage, args: Vec<ArgIds> }`)
  used by Milestones 3–5.
- Collision detection: normalized argument ids are checked for uniqueness
  with the `ensure_unique` pattern from `generate/docs/fields/validation.rs`
  (spanned error at the colliding field, combined "first defined here" note at
  the earlier one). Two fields such as `foo_bar: String` and
  `#[arg(id = "foo-bar")] other: String` collide, because kebab-casing maps
  `foo_bar` to `foo-bar`.

Attribute parsing: extend `StructAttrs` and `parse_struct_attrs`
(`derive/parse/mod.rs`) with `localization_base: Option<String>`, validated at
parse time (each dotted segment must normalize cleanly; errors are spanned to
the attribute).

Red: rstest unit tests in the macro crate (inline test modules, following
`ortho_config_macros/src/derive/parse/tests/`) for the normalizer, base
resolution, argument-id selection, and collision detection — written first
against the not-yet-existing module so they fail, then implemented. A
`proptest` property in the macro crate: for any generated list of valid
segments, normalization is idempotent and produces a valid Fluent identifier
(`[a-zA-Z][a-zA-Z0-9_-]*`).

Validation: full gate set. Commit.

### Milestone 3: derive emission and cross-crate agreement

Wire the Milestone 2 model into `derive_ortho_config`
(`ortho_config_macros/src/lib.rs`, alongside the existing `generate_docs_impl`
call): emit

```rust
impl #krate::OrthoConfigLocalization for #ident {
    const ABOUT_ID: &'static str = "hello_world-cli-about";
    // …
    const ARG_IDS: &'static [(&'static str, &'static str, &'static str)] = &[
        ("hello_world-cli-args-recipient-help", /* … */),
    ];
}
```

with all values computed at expansion time as string literals.

Red first, in this order:

1. A trybuild compile-fail case
   `ortho_config/tests/ui/localization_id_collision.rs` (two fields whose ids
   normalize identically) with a `.stderr` expectation showing the spanned
   diagnostic. This fails (compiles cleanly) until emission and collision
   wiring land.
2. Extend `ortho_config/tests/localized_parse.rs`: a new fixture struct
   using `#[derive(OrthoConfig)]` (per 11.1.2 Decision D-4, which reserved
   exactly this upgrade), asserting with `googletest`/`pretty_assertions` that
   every constant equals the corresponding `message_id_for(...)` call, and that
   the `RecordingLocalizer` coverage set equals the set of derived constants
   after a `parse_localized_command` run — the cross-crate agreement lock
   required by design §8.2.
3. A property test (`proptest`) in `ortho_config/tests/`: for generated
   valid segment paths, `message_id_for` output equals the macro-side
   normalizer's output. The macro-side function is exercised through a small
   `#[doc(hidden)] pub` re-export from `ortho_config_macros` (or, if exposing
   it is unpalatable, through token-level tests in the macro crate plus the
   fixture-based agreement in item 2; decide at implementation time and record
   in the Decision Log).
4. An rstest-bdd scenario in
   `ortho_config/tests/features/localizer.feature` plus steps in
   `tests/rstest_bdd/behaviour/steps/localizer_steps.rs`: "Given a derived
   configuration struct with a localization catalogue, When the command line is
   parsed with a Fluent localizer, Then the help text resolves through the
   derive-generated identifiers" — exercising the end-to-end behavioural
   contract.

Then migrate `examples/hello_world` to reference the derived constants where it
currently hand-builds identifiers, keeping the existing insta snapshots green
(identifier *values* do not change — the derive reproduces the same convention).

Validation: full gate set; `coderabbit review --agent` for Milestones 1–3 as
one review unit (gates must be green first). Commit per sub-step.

### Milestone 4: docs IR delegation

Per Decisions D-1 and D-2: change the docs generator
(`ortho_config_macros/src/derive/generate/docs/`) so that *default* identifier
values (`resolve_about_id` in `sections.rs`, `default_field_id` in
`fields/defaults.rs`) are the canonical localization identifiers, emitted as
references to `<Self as OrthoConfigLocalization>` constants where the emission
site allows, or as identical literals computed by the same Milestone 2 pass
where it does not. Explicit attribute overrides (`help_id`, `long_help_id`,
`about_id`, heading ids) are untouched.

Bump `ORTHO_DOCS_IR_VERSION` to "1.2" in `ortho_config/src/docs/mod.rs`.

Red: update one docs IR test first (`ortho_config/tests/docs_ir.rs`) to expect
the new identifier shape and version — it fails — then implement, then sweep
the remaining pinned expectations: `docs_ir_subcommands.rs`,
`nested_docs_ir.rs`, `subcommand_docs.rs`, the `cargo-orthohelp` golden files
(`cargo-orthohelp/tests/golden/`, `tests/snapshots/`), agent-context snapshots
(`ortho_config/src/agent_context/snapshots/`), and the `hello_world` snapshots,
regenerating insta snapshots deliberately (`cargo insta review` equivalent:
inspect every diff; only identifier values and the version string may change).

Validation: full gate set; `coderabbit review --agent`; a `Tolerances` check —
if anything beyond identifier values and version strings shifts in the
snapshots, stop and escalate. Commit.

### Milestone 5: opt-in build-time identifier artefact

New module `ortho_config_macros/src/derive/generate/localization/artefact.rs`
(plus a sibling `artefact/` split if the 400-line cap demands):

- A pure, filesystem-free core (dependency-injection style, per
  `docs/reliable-testing-in-rust-via-dependency-injection.md`): given the
  Milestone 2 model plus span data, produce an in-memory artefact set — either
  `[("cli-identifiers.json", contents)]` or, when the merged JSON would exceed
  1 MiB (1,048,576 bytes), a split set `cli-identifiers.<n>.json` plus
  `cli-identifiers.index.json` naming the parts. Entry schema per identifier:
  `id`, `kind` (`about`/`long_about`/`usage`/`help`/`long_help`/`value_name`),
  `type` (the deriving type's path), `field`, `source` (file, line, column —
  from `proc_macro2::Span`), `embedded_default` (always `null`; Decision D-4).
  Serialization via `serde_json` (promoted to an unconditional macro-crate
  dependency).
- A thin filesystem shell: runs only when `OUT_DIR` is set and
  `ORTHO_CONFIG_EMIT_IDENTIFIERS=1` (Decision D-3). Each derive expansion
  writes a fragment `${OUT_DIR}/ortho-config/cli-identifiers.d/<Type>.json`,
  then re-merges all fragments into the capped top-level file set. Fragment
  files make concurrent expansions and incremental rebuilds safe: writes are
  per-type and idempotent, and the merge is a deterministic fold over the
  fragment directory. I/O failures are reported as spanned derive errors only
  when emission was explicitly requested; they can never affect an ambient
  build because the shell never runs in one.

Red tests, in order:

1. rstest unit tests on the pure core: schema shape (locked with an `insta`
   JSON snapshot), the 1 MiB cap boundary (one byte under stays single-file;
   one byte over splits), deterministic ordering.
2. A `proptest` property: for any generated entry set, merging the split
   output reproduces exactly the input entries, and every emitted file except a
   single oversized entry respects the cap.
3. An end-to-end test (new file
   `ortho_config/tests/identifier_artefact_e2e.rs`, marked `#[ignore]`-free but
   serialized via `serial_test` because it drives Cargo): add a minimal
   `build.rs` to `tests/fixtures/orthohelp_fixture` (so `OUT_DIR` exists), run
   `cargo build -p orthohelp_fixture --message-format=json` as a subprocess with
   `ORTHO_CONFIG_EMIT_IDENTIFIERS=1`, locate `OUT_DIR` from the JSON build
   messages, and assert the artefact exists, parses, and contains the fixture's
   derived identifiers. Uses the shared default Cargo cache (never an isolated
   one) and tolerates the package-cache lock.

Validation: full gate set; `coderabbit review --agent`. Commit.

### Milestone 6: documentation, ADR, roadmap, and closure

- Write `docs/adr-008-opt-in-identifier-artefact-emission.md` (Y-Statement,
  per `arch-decision-records`): ambient proc-macro writes rejected; env-gated
  emission chosen; alternatives (unconditional write, extract-from-binary,
  source-parsing CLI) recorded with the evidence from the research pass. Index
  it in `docs/contents.md`.
- Amend `docs/cli-localization-design.md` §8.2: artefact emission is opt-in
  (reference ADR-008); record Decisions D-1/D-2 (docs IR reconciliation and
  version bump) and D-5 (`localization_base`); note `localized_default`
  deferral (D-4). Update ADR-006's "known risk" paragraph to state the
  derive-time guard has landed.
- `docs/users-guide.md` ("Localizing CLI copy" section, ~line 305): document
  the trait, the derived constants, `localization_base`, the collision compile
  error, and the artefact opt-in with a worked example.
- `docs/developers-guide.md` ("Schema ownership" area, ~lines 81–103):
  document the twin-normalizer rule (runtime `normalize_segment` and the
  macro-side twin must agree; the agreement tests are the lock), the docs-IR
  delegation, and the artefact fragment/merge design. Update the paragraph that
  says runtime panic tests remain "until derive-emitted identifiers move
  validation to compile time".
- `CHANGELOG.md`: entries for the new trait, the collision diagnostic, the
  IR identifier change (with version bump), and the artefact opt-in.
- `docs/roadmap.md`: tick all five 11.1.3 checkboxes and the parent item,
  with the customary Decision/Finding notes mirroring this plan's Decision Log.
- Final full gate run (`make check-fmt`, `make typecheck`, `make lint`,
  `make test`, `make markdownlint`, `make nixie`) via `scrutineer`; final
  `coderabbit review --agent`; clear all findings.

## Concrete steps

All commands run from the repository root. Long outputs go through `tee`, for
example:

```sh
make test 2>&1 | tee "/tmp/test-ortho-config-$(git branch --show-current).out"
```

- Gates (every milestone): `make check-fmt`, `make typecheck`, `make lint`,
  `make test`; documentation milestones add `make markdownlint` and
  `make nixie`. Prefer delegating the full run to the `scrutineer` subagent and
  reading its cited logs on failure.
- Focused loops: `cargo test -p ortho-config --test localized_parse`,
  `cargo test -p ortho_config_macros`,
  `cargo test -p ortho-config --test compile_fail` (set `TRYBUILD=overwrite`
  only to intentionally regenerate `.stderr`).
- Snapshot review (Milestone 4/5): `cargo insta test` / accept via explicit
  inspection of each `.snap.new`.
- E2E artefact test (Milestone 5):
  `cargo test -p ortho-config --test identifier_artefact_e2e`.
- Commit after every green sub-step with `commit-message`-skill-formatted
  messages; never commit on a red gate.

Expected red-stage evidence examples: the trybuild collision case initially
*fails the harness* by compiling successfully; `localized_parse.rs`'s new
assertions initially fail with a missing-trait compile error (Milestone 1 red)
or identifier mismatch (Milestone 3 red). Record actual transcripts in
`Artefacts and notes` as they occur.

## Validation and acceptance

Acceptance is behavioural:

1. `make test` passes. The new tests
   `localized_parse::derived_constants_match_message_id_for` (name indicative),
   the trybuild case `tests/ui/localization_id_collision.rs`, the macro-crate
   localization unit and property tests, the artefact unit/property/snapshot
   tests, and the e2e artefact test all exist and pass; each failed first for
   the documented reason.
2. A collision reproduces a compile error pointing at the second field:
   building a struct with fields `foo_bar` and `#[arg(id = "foo-bar")]` fails
   with `duplicate localization identifier 'X-args-foo-bar-help'` (wording
   indicative) at the offending field span.
3. `cargo build -p orthohelp_fixture` with
   `ORTHO_CONFIG_EMIT_IDENTIFIERS=1` produces
   `${OUT_DIR}/ortho-config/cli-identifiers.json` listing the fixture's
   identifiers; the same build *without* the variable writes nothing.
4. `cargo run -p hello_world --bin emit_docs` (docs IR) reports identifiers
   equal to the `OrthoConfigLocalization` constants and IR version "1.2".
5. `make check-fmt`, `make typecheck`, `make lint`, `make markdownlint`, and
   `make nixie` all pass; CodeRabbit findings are cleared.

## Idempotence and recovery

Every milestone is an ordinary additive code change committed on a green gate;
`git revert` of the milestone commits is the rollback path. Snapshot
regeneration is repeatable (`cargo insta test` and re-review). The e2e test
builds into the shared `target/` directory and is `serial_test`-guarded; if it
is interrupted, re-running it is safe because fragment writes are idempotent
and the merge is deterministic. No step mutates state outside the repository and
`/tmp` logs.

## Artefacts and notes

(To be populated during implementation: red/green transcripts, the collision
diagnostic as rendered, a sample `cli-identifiers.json`.)

## Interfaces and dependencies

At completion the following exist:

- `ortho_config::OrthoConfigLocalization` (trait, §8.1 shape above), defined
  in `ortho_config/src/localizer/localization_ids.rs`, re-exported at the crate
  root.
- `#[derive(OrthoConfig)]` additionally emits
  `impl ortho_config::OrthoConfigLocalization for T` with literal-valued
  constants, honouring `#[ortho_config(localization_base = "…")]`.
- `ortho_config_macros::derive::generate::localization` (private): strict
  normalizer (`syn::Result`-based twin of `normalize_segment`), identifier
  model, collision detection, artefact core and shell.
- Docs IR defaults delegate to the localization identifiers;
  `ORTHO_DOCS_IR_VERSION == "1.2"`.
- Opt-in artefact under `${OUT_DIR}/ortho-config/` as specified in
  Milestone 5.
- New unconditional macro-crate dependencies: `serde`, `serde_json` (already
  in the workspace dependency set). No other dependency changes.
