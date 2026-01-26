# Implement OrthoConfigDocs IR v1.1 in derive macro

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DONE

PLANS.md is not present in the repository root, so this ExecPlan follows the
standard execplans format.

## Purpose / Big Picture

Deliver the documentation intermediate representation (IR) described in
`docs/cargo-orthohelp-design.md` by extending the `#[derive(OrthoConfig)]`
macro to emit `OrthoConfigDocs` metadata (schema v1.1). Users should be able to
call `<Config as OrthoConfigDocs>::get_doc_metadata()`, serialize the result to
JSON, and observe deterministic IDs for every required field. Success is
observable when new unit and behavioural tests pass and the example user-facing
docs show the new API.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not workarounds.

- Stay within scope of roadmap item 4.1.1; do not implement the
  `cargo-orthohelp` pipeline or generators yet.
- Follow the IR schema and rules in `docs/cargo-orthohelp-design.md` (v1.1),
  including Windows metadata and deterministic auto-ID generation.
- Keep all files under 400 lines; split modules if needed.
- Every module must begin with a `//!` module-level comment.
- Use en-GB spelling in new or updated documentation and comments.
- Do not add new dependencies without explicit approval.
- Use `cap_std`/`camino` instead of `std::fs`/`std::path` if filesystem work
  is introduced.
- Tests must use `rstest` fixtures and `rstest-bdd` v0.3.2 for behavioural
  coverage.
- Documentation updates must follow `docs/documentation-style-guide.md` and
  wrap paragraphs/bullets at 80 columns.

If satisfying the objective requires violating a constraint, do not proceed.
Document the conflict in `Decision Log` and escalate.

## Tolerances (Exception Triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Scope: stop if implementation requires changes to more than 12 files or
  more than 800 net lines of code.
- Interface: stop if a public API must be removed or have a breaking change.
- Dependencies: stop if a new external dependency is required.
- Iterations: stop if tests still fail after two fix attempts.
- Ambiguity: stop if multiple valid interpretations of the IR schema would
  materially change IDs or metadata.

## Risks

Known uncertainties that might affect the plan. Identify these upfront and
update as work proceeds. Each risk should note severity, likelihood, and
mitigation or contingency.

- Risk: Mapping Rust types to `ValueType` may be ambiguous for aliases or
  custom types. Severity: medium. Likelihood: medium. Mitigation: follow the
  design doc's `value(type = ...)` override rules and document any default
  heuristics in the design document.
- Risk: Subcommand metadata might not be derivable from existing `clap`
  attributes. Severity: medium. Likelihood: low. Mitigation: inspect existing
  macro parsing; if subcommand discovery is not possible without a breaking
  change, escalate.
- Risk: Deterministic ID generation could diverge from runtime naming rules,
  causing confusing docs. Severity: medium. Likelihood: medium. Mitigation:
  reuse existing naming helpers (CLI/env/file) where possible and cover with
  unit tests.

## Progress

Use a list with checkboxes to summarize granular steps. Every stopping point
must be documented here, even if it requires splitting a partially completed
task into two ("done" vs. "remaining"). This section must always reflect the
actual current state of the work.

- [x] (2026-01-17 14:00Z) Draft ExecPlan created.
- [x] Implement IR types and `OrthoConfigDocs` trait in `ortho_config`.
- [x] Extend macro parsing and generation for IR metadata and auto-IDs.
- [x] Add rstest unit tests for IR JSON and ID determinism.
- [x] Add rstest-bdd scenario covering IR behaviour.
- [x] Update docs and hello_world example for new API usage.
- [x] (2026-01-18 19:25Z) Update roadmap entry 4.1.1 and run validation.

## Surprises & Discoveries

Unexpected findings during implementation that were not anticipated as risks.
Document with evidence so future work benefits.

- Observation: None noted so far.
  Evidence: N/A. Impact: N/A.

## Decision Log

Record every significant decision made while working on the plan. Include
decisions to escalate, decisions on ambiguous requirements, and design choices.

- Decision: Emit precedence metadata even when `precedence(...)` is omitted,
  using the default order `[defaults, file, env, cli]`. Rationale: Precedence
  is deterministic in the loader; emitting it by default keeps docs consistent
  without extra attributes. Date/Author: 2026-01-18 (assistant).
- Decision: Emit discovery metadata only when `discovery(...)` is present and
  leave `search_paths` empty in the IR for now. Rationale: Discovery paths are
  platform-specific; tooling can reuse runtime discovery to render them later.
  Date/Author: 2026-01-18 (assistant).
- Decision: `bin_name` is emitted only when explicitly provided.
  Rationale: The derive macro cannot reliably infer the final binary name.
  Date/Author: 2026-01-18 (assistant).

## Outcomes & Retrospective

Summarize outcomes, gaps, and lessons learned at major milestones or at
completion. Compare the result against the original purpose. Note what would be
done differently next time.

- Outcome: IR v1.1 metadata is available via `OrthoConfigDocs`, with
  deterministic IDs, Windows metadata, and JSON serialization validated by unit
  and behavioural tests.

## Context and Orientation

The derive macro lives in `ortho_config_macros/src/lib.rs` and its supporting
modules under `ortho_config_macros/src/derive`. Attribute parsing is handled in
`ortho_config_macros/src/derive/parse`, and code generation for runtime traits
is under `ortho_config_macros/src/derive/generate`. The runtime crate is
`ortho_config`, with public APIs defined in `ortho_config/src/lib.rs`. Tests
for macros and runtime features are primarily under `ortho_config/tests` and
behavioural tests using `rstest-bdd` live in `ortho_config/tests/rstest_bdd`
and `examples/hello_world/tests/rstest_bdd`.

The IR schema to implement is defined in `docs/cargo-orthohelp-design.md`
(section 2). The plan must ensure the derive macro emits a new trait
implementation (`OrthoConfigDocs`) that returns a `DocMetadata` structure with
schema version `1.1`, optional Windows metadata, and deterministic default IDs
when IDs are not explicitly set via attributes. Documentation updates must be
aligned with `docs/users-guide.md` and the example crate at
`examples/hello_world`.

## Plan of Work

Stage A: Understand existing macro inputs and naming rules. Read
`docs/cargo-orthohelp-design.md` (IR schema and auto-ID rules) plus relevant
macro parsing modules. Identify where CLI/env/file naming helpers already
exist, so IR generation can reuse them.

Stage B: Add runtime IR types and the `OrthoConfigDocs` trait. Introduce a new
module in `ortho_config/src` (for example `docs.rs`) containing the IR structs
and enums from schema v1.1 with `serde::Serialize` derives and module-level
`//!` documentation. Re-export the trait and IR types from
`ortho_config/src/lib.rs` so downstream users can import them.

Stage C: Extend macro parsing to capture doc-related attributes. Update
`StructAttrs` and `FieldAttrs` to include fields for `about_id`, `synopsis_id`,
`help_id`, `long_help_id`, `value(type = ...)`, `deprecated`, `example`,
`link`, `note`, `headings`, `discovery`, `precedence`, and `windows` metadata.
Keep existing keys backward compatible and decide how unknown keys are handled.
Document any new parsing decisions in `docs/cargo-orthohelp-design.md`.

Stage D: Generate IR metadata and auto-IDs. Implement a new generator module
(e.g. `ortho_config_macros/src/derive/generate/docs.rs`) that builds
`DocMetadata` for the root config and any subcommands, including deterministic
IDs as per section 3.4 of the design document. Ensure `ir_version` is `"1.1"`
and that `windows` metadata is `None` unless attributes provide values. Include
collision detection for duplicate `env.var_name` and `file.key_path` and emit
hard errors with spans when detected, as specified in the design document.

Stage E: Tests. Add rstest unit tests (likely in
`ortho_config/tests/attribute_handling.rs` or a new test module) to validate
JSON serialization, default ID generation, Windows metadata emission, and
collision errors. Add rstest-bdd scenarios in `ortho_config/tests/features` and
step definitions in `ortho_config/tests/rstest_bdd/behaviour/steps` that
exercise the new `OrthoConfigDocs` output and assert deterministic IDs. Use
fixtures for sample config structs and call `serde_json::to_string_pretty` to
validate JSON output without panics.

Stage F: Documentation and examples. Update `docs/users-guide.md` to describe
how consumers derive `OrthoConfigDocs` and serialize the IR for
`cargo-orthohelp`. Update `examples/hello_world` (source and README if
necessary) to show calling `get_doc_metadata()` and serializing to JSON (or
printing) from an API consumer perspective. Record any design decisions in
`docs/cargo-orthohelp-design.md` and ensure all documentation is wrapped to 80
columns.

Stage G: Roadmap and validation. Mark roadmap item 4.1.1 as done in
`docs/roadmap.md` once implementation and tests are complete. Run
`make check-fmt`, `make lint`, and `make test`, plus documentation-specific
checks (`make fmt`, `make markdownlint`, `make nixie`) after doc updates.
Capture logs via `tee` as required by `AGENTS.md`.

## Concrete Steps

1. Inspect existing macro parsing and naming helpers:

   - `rg -n "StructAttrs|FieldAttrs" ortho_config_macros/src/derive/parse`
   - `rg -n "env var|config key|clap" ortho_config_macros/src/derive`

2. Add IR types and trait in `ortho_config/src/docs.rs`, update
   `ortho_config/src/lib.rs` with `mod docs;` and re-exports.

3. Extend parsing in `ortho_config_macros/src/derive/parse/mod.rs` (and
   submodules as needed) to capture doc attributes and Windows metadata.

4. Add generator module for doc metadata and wire it into
   `ortho_config_macros/src/lib.rs` alongside existing
   `generate_trait_implementation`.

5. Add unit tests using rstest and new behavioural tests using rstest-bdd.
   Place new feature files under `ortho_config/tests/features` and step
   definitions under `ortho_config/tests/rstest_bdd/behaviour/steps`.

6. Update docs and example:

   - `docs/users-guide.md`
   - `examples/hello_world/src` (plus README if user guidance changes)
   - `docs/cargo-orthohelp-design.md` (decision notes)
   - `docs/roadmap.md` (mark 4.1.1 as done)

7. Run validation commands with logging:

   - `set -o pipefail && make fmt 2>&1 | tee /tmp/orthohelp-fmt.log`
   - `set -o pipefail && make markdownlint 2>&1 | tee /tmp/orthohelp-mdlint.log`
   - `set -o pipefail && make nixie 2>&1 | tee /tmp/orthohelp-nixie.log`
   - `set -o pipefail && make check-fmt 2>&1 | tee /tmp/orthohelp-check-fmt.log`
   - `set -o pipefail && make lint 2>&1 | tee /tmp/orthohelp-lint.log`
   - `set -o pipefail && make test 2>&1 | tee /tmp/orthohelp-test.log`

## Validation and Acceptance

Acceptance is met when:

- Calling `<Config as OrthoConfigDocs>::get_doc_metadata()` yields
  `DocMetadata { ir_version: "1.1", ... }` and JSON serialization succeeds.
- Required IDs (about, headings, field help) are deterministic and match the
  rules in `docs/cargo-orthohelp-design.md` when not explicitly provided.
- Windows metadata is present only when configured and matches schema v1.1.
- Unit tests using rstest cover ID generation, Windows metadata, and duplicate
  `env`/`file` collisions.
- Behavioural tests using rstest-bdd validate a user-facing IR scenario.
- `docs/users-guide.md` and `examples/hello_world` show how consumers use the
  new API.
- `docs/roadmap.md` item 4.1.1 is marked done.
- `make check-fmt`, `make lint`, and `make test` succeed, and documentation
  checks (`make fmt`, `make markdownlint`, `make nixie`) pass.

## Idempotence and Recovery

All steps are safe to re-run. If a test or lint step fails, inspect the
corresponding `/tmp/orthohelp-*.log`, fix the issue, and re-run the failed
command. Doc formatting can be re-run via `make fmt` without side effects.

## Artifacts and Notes

Include a small JSON excerpt in tests or docs to confirm expected structure,
for example (line breaks wrapped to keep code blocks under 120 columns):

    {
      "ir_version": "1.1",
      "app_name": "hello-world",
      "about_id": "hello-world.about",
      "sections": { "headings_ids": { "options": "ortho.headings.options" } },
      "fields": [ { "name": "greeting", "help_id": "hello-world.fields.greeting.help" } ]
    }

## Interfaces and Dependencies

The following public API should exist after the change:

- `ortho_config::docs::DocMetadata` and related IR structs/enums mirroring
  schema v1.1 (`SectionsMetadata`, `HeadingIds`, `FieldMetadata`, `ValueType`,
  `WindowsMetadata`, etc.), all `#[derive(Serialize)]`.
- `ortho_config::docs::OrthoConfigDocs` trait with
  `fn get_doc_metadata() -> DocMetadata`.
- `#[derive(OrthoConfig)]` emits an `OrthoConfigDocs` implementation that
  builds `DocMetadata` including subcommands, windows metadata, and default IDs.

No new external dependencies are expected; if new crates are required, escalate
per tolerances.

## Revision note (required when editing an ExecPlan)

Initial draft created for roadmap item 4.1.1.
