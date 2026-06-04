
# Add `--format agent-context` to `cargo-orthohelp`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS

This plan covers roadmap item 6.2.1 only (`docs/roadmap.md` §6.2.1). It does
not implement schema versioning or golden fixtures for nested or enum-bearing
CLIs (6.2.2), the downstream `<tool> context --json` command naming work
(6.2.3), skill manifest metadata (6.3.1), or skill manifest validation
(6.3.2). The plan also does not extend `ortho_config::agent_context` beyond a
single additive optional field; broader schema work is reserved for 6.2.2.

## Purpose / big picture

Phase 6 of the active roadmap (`docs/roadmap.md` §6, "Deliver whole-CLI
introspection") makes the command tree visible. With roadmap item 6.1.1
completed, the documentation intermediate representation (IR) now carries a
recursive `DocMetadata.subcommands` tree, but `cargo-orthohelp` still has no
output that an LLM agent can consume. This plan closes that gap by adding a
new `--format agent-context` output that emits a compact, machine-oriented
JSON document describing the CLI's command tree.

After this plan is approved and implemented, a maintainer working in a
downstream consumer crate should be able to:

1. annotate their root `clap::Parser` struct with `#[derive(OrthoConfig)]`,
   ensure any subcommand enum has `#[derive(OrthoConfigSubcommandDocs)]`,
   then run `cargo orthohelp --format agent-context --root-type ...`;
2. observe a single JSON file at `<out_dir>/agent-context.json` that contains
   `schema_version`, `kind` (in the form `<package>.agent_context`), `package`,
   and a flat `commands` array. Each entry carries the full command path
   (root plus subcommands), a canonical verb when the last path segment
   matches the canonical set, an `inputs` array with one entry per CLI flag
   and positional argument, and a short `summary` for command selection;
3. point that JSON file at an agent or test harness and confirm it never
   contains localized long prose, Fluent identifiers, roff fragments, or
   PowerShell help structures.

Observable success is checked by:

- new `rstest` unit tests in `cargo-orthohelp/src/agent_context/tests.rs`
  exercising the `bridge_ir_to_agent_context` transform on hand-built
  `DocMetadata` values (flat command, nested commands, enum-bearing input,
  default-bearing input, positional input, hidden field, deprecated field);
- a new `rstest-bdd` scenario in
  `cargo-orthohelp/tests/features/orthohelp_agent_context.feature` driving
  `cargo-orthohelp --format agent-context` against the existing fixture and
  asserting an `insta` golden file at
  `cargo-orthohelp/tests/golden/agent_context__fixture.json.snap`;
- updated CLI parser tests in `cargo-orthohelp/src/cli.rs` that accept
  `--format agent-context` and remove the negative-only assertion that
  currently rejects it (`format_rejects_unsupported_values`);
- a single very narrow `proptest` invariant on the transform ensuring command
  paths are unique and non-empty across arbitrary subcommand trees;
- `make check-fmt`, `make typecheck`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie` all passing at the close of each
  milestone;
- `coderabbit review --agent` returning clean before each milestone is marked
  done.

This plan does **not** change `ORTHO_DOCS_IR_VERSION` or bump
`ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`. A single additive field is added to
`ortho_config::agent_context::AgentCommand` under `#[serde(default)]` and
`#[serde(skip_serializing_if = "Option::is_none")]`, which is non-breaking
under serde semantics and consistent with §8.1 of
`docs/agent-native-cli-design.md` ("legacy defaulting"). All other agent-
context fields are emitted using their existing schema defaults.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violating any of them requires escalation in `Decision Log`, not
a workaround.

- Do not implement code, tests, examples, or documentation in this branch
  until this ExecPlan is explicitly approved by the maintainer. A "DRAFT" plan
  must remain a planning artefact only.
- Keep this work focused on roadmap item 6.2.1 ("Add `--format agent-context`
  to `cargo-orthohelp`"). Schema-version tests, multi-fixture golden coverage,
  downstream command naming, skill manifest metadata, and skill manifest
  validation are explicitly out of scope; if partial coverage falls out of
  6.2.1 work, mark it clearly and stop for separate approval before extending
  it.
- Do not change `ORTHO_DOCS_IR_VERSION` or
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`. The work in this plan is an additive
  field on `AgentCommand` (`summary: Option<String>`) plus a new emitter; the
  schema version remains `"1"` because the field is optional and absent in
  legacy payloads.
- Do not add or rename any of the existing fields on `ortho_config::docs::ir::*`
  types. The bridge IR shape is reused verbatim; cache invalidation
  (`CacheKey.ir_version`) therefore stays unchanged and existing
  `--format ir|man|ps|all` outputs are bit-identical for consumers whose
  IR is unchanged.
- Do not add new fields to `AgentInput`. Positional inputs are detected from
  existing IR data using the rule
  `cli.long.is_none() && cli.short.is_none() && cli.takes_value`. (See
  `Design overview` for rationale.)
- Preserve the boundary established by ADR-003
  (`docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`). The
  schema lives in `ortho_config::agent_context`; the new transform and emitter
  live in `cargo_orthohelp::agent_context`. The transform must not depend on
  `cargo-orthohelp`'s renderer modules (`roff`, `powershell`), and the
  reusable schema must not depend on `cargo-orthohelp`.
- The agent-context output is **not** localized. The transform must not call
  `ortho_config::Localizer`, must not include Fluent identifiers in the
  output, must not include `long_help` or `long_help_id` content, and must
  not introduce any locale-specific behaviour.
- `--format agent-context` writes exactly one file at
  `<out_dir>/agent-context.json`. The bridge cache, target directory layout,
  and `--cache`/`--no-build` flags continue to work unchanged.
- `--format all` does **not** include `agent-context` in this plan. Including
  it in the default bundle is deferred until 6.2.2 lands schema-versioning
  tests and at least one consumer relies on the bundle.
- Use `cap_std`/`camino` instead of `std::fs`/`std::path` for any new
  filesystem I/O. The existing crates already follow this rule.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests, per
  `docs/developers-guide.md` and the workspace `Cargo.toml` lints. Use
  `insta` for the single golden file. Use `proptest` only for the one narrow
  uniqueness invariant on the transform; do not introduce `kani` or `verus`
  for this work.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Scope: if implementation requires changes to more than 12 files or 700 net
  lines of code, stop and escalate. The transform plus emitter, plus tests,
  plus a small change to the CLI enum and one tiny `ortho_config` schema
  addition, should fit well inside that budget.
- Interface: if changing a public API beyond
  (a) adding `AgentContext` to `cargo_orthohelp::cli::OutputFormat`,
  (b) adding `pub fn bridge_ir_to_agent_context(...)` in the new module, and
  (c) adding `pub summary: Option<String>` to `AgentCommand`, stop and
  escalate. In particular, do not add or rename other public agent-context
  fields without approval.
- Dependencies: if a new external dependency is required (beyond what
  `cargo-orthohelp/Cargo.toml` and `ortho_config/Cargo.toml` already declare),
  stop and escalate. The transform is pure data manipulation and needs none.
- Iterations: if any test still fails after three focused fix attempts, stop,
  capture the failure in `Surprises & Discoveries`, and escalate.
- Time: if a single milestone consumes more than four hours of work, stop
  and escalate. Sub-milestones may be split if useful.
- Ambiguity: if the value of any IR field cannot be unambiguously mapped to
  an agent-context field (for example, an unrecognised `ValueType` variant
  or a field carrying both `cli.long` and `takes_value = false` other than
  `Bool`), stop and present options in `Decision Log` rather than guessing.

## Risks

Known uncertainties that might affect the plan. Update as work proceeds.

- Risk: the canonical-verb mapping picks up a false positive when a real
  command name happens to spell `list` or `get`.
  Severity: low. Likelihood: medium.
  Mitigation: derive the verb only from the **last** path segment, only when
  the segment exactly matches the canonical set in `docs/agent-native-cli-design.md`
  §5, and document the exact set in code. A consumer that uses `list` to
  mean something else can opt out in a future item (6.2.2 or 7.1).

- Risk: golden snapshot churn on unrelated IR changes (for example, a new
  field in `DocMetadata` that is unused by the transform but ends up in the
  fixture's IR).
  Severity: low. Likelihood: medium.
  Mitigation: the transform is purely projective. Snapshot only the
  agent-context JSON, never the upstream IR. The transform sorts commands by
  path and inputs by name before serialising so that input ordering changes
  in upstream IR do not flip the snapshot.

- Risk: positional-detection rule misclassifies an unusual field (for
  example, a field with `cli: Some(_)`, no `long`, no `short`, but
  `takes_value = false`). The IR macro does not currently produce such a
  field, so the rule should be tight.
  Severity: low. Likelihood: low.
  Mitigation: the rule rejects such fields entirely (treats them as neither
  flag nor positional) and emits a `tracing::warn!` so the gap surfaces in
  development. A unit test covers this branch.

- Risk: agents pin to the `agent_context` JSON shape and break when 6.2.2
  introduces semver-style schema-version bookkeeping.
  Severity: medium. Likelihood: low.
  Mitigation: `schema_version` is already emitted as `"1"`; 6.2.2 will lock
  it down with tests but not change it. This plan does not change the
  version. Document the compatibility policy in the user's guide.

- Risk: the new transform's `proptest` invariant produces flaky shrink output
  on CI.
  Severity: low. Likelihood: low.
  Mitigation: cap the strategy depth and width to small numbers (depth ≤ 3,
  per-level width ≤ 4) so the search space stays tiny and shrink times
  bounded. Persist regressions under `cargo-orthohelp/proptest-regressions/`
  per the `proptest` skill guidance.

## Progress

Use a list with checkboxes to summarise granular steps. Every stopping point
must be documented here, even if it requires splitting a partially completed
task into two. This section must always reflect the actual state of the work.

- [x] Milestone 0: ExecPlan approved by maintainer. (2026-06-04 00:59Z)
- [x] Milestone 1: Add `AgentCommand.summary: Option<String>` to
  `ortho_config/src/agent_context/mod.rs` under `#[serde(default,
  skip_serializing_if = "Option::is_none")]`. Update existing
  `ortho_config/src/agent_context/tests.rs` cases. Update
  `docs/cargo-orthohelp-design.md` §6.3.1 to record the inclusion of a short
  command summary in the transform. (2026-06-04 01:06Z)
- [ ] Milestone 2: Add `OutputFormat::AgentContext` to
  `cargo-orthohelp/src/cli.rs`. Update existing parser unit tests: keep the
  `--format` `value_enum` rejection coverage by switching the rejected token
  in `format_rejects_unsupported_values` (for example, to `xml`) and add a
  positive test that accepts `agent-context`.
- [ ] Milestone 3: Create `cargo-orthohelp/src/agent_context/mod.rs` with
  `bridge_ir_to_agent_context`, the deterministic verb-mapping table, and the
  helper that flattens a `DocMetadata` tree into `Vec<AgentCommand>`. Wire
  the transform from `cargo-orthohelp/src/main.rs::run` through
  `cargo-orthohelp/src/output.rs::write_agent_context`. Update the
  `mod` declaration in `cargo-orthohelp/src/lib.rs` and `main.rs`.
- [ ] Milestone 4: Add unit tests at
  `cargo-orthohelp/src/agent_context/tests.rs` (table-driven `rstest`
  cases) and the property test at
  `cargo-orthohelp/src/agent_context/proptests.rs` (single uniqueness
  invariant).
- [ ] Milestone 5: Add the behavioural scenario at
  `cargo-orthohelp/tests/features/orthohelp_agent_context.feature` plus step
  definitions at
  `cargo-orthohelp/tests/rstest_bdd/behaviour/steps_agent_context.rs`. Add
  the matching insta snapshot at
  `cargo-orthohelp/tests/golden/agent_context__fixture.json.snap` after
  running `cargo insta review`.
- [ ] Milestone 6: Update `docs/users-guide.md` with a "Agent-context output"
  subsection under the existing `cargo-orthohelp` material. Update
  `docs/developers-guide.md` with the positional-detection rule and the
  fact that the agent-context output is not localized.
- [ ] Milestone 7: Run `make check-fmt`, `make typecheck`, `make lint`,
  `make test`, `make markdownlint`, `make nixie`, then `coderabbit review
  --agent` and resolve all findings. Mark roadmap §6.2.1 as done in
  `docs/roadmap.md`.

Use timestamps (`(YYYY-MM-DD HH:MMZ)`) on each `[x]` line as work completes.

## Surprises & discoveries

Unexpected findings during implementation. Document with evidence so future
work benefits.

- Clippy denied direct indexing in the new summary serialisation test under
  `clippy::indexing_slicing`. The test now uses `first_mut().expect(...)`
  so failures report intent rather than panic at an implicit index.
- `coderabbit review --agent` completed with zero findings after Milestone 1
  validation.

## Decision log

Record every significant decision made while working on this plan.

- Decision: drop the proposed `AgentInput.kind` schema extension and detect
  positional inputs from existing IR data instead.
  Rationale: the Logisphere review (2026-06-02) flagged that adding a new
  schema field on `AgentInput` in this PR muddies ADR-003 ownership and
  requires version coordination. The
  `cli.long.is_none() && cli.short.is_none() && cli.takes_value` rule is
  sufficient given the IR macro's current output.
  Date/Author: 2026-06-02 / planning agent.

- Decision: include a `summary: Option<String>` field on `AgentCommand` as
  the only schema addition in this plan.
  Rationale: §6.2.1 of the roadmap explicitly permits "a concise summary
  needed for command selection." The agent-context manifest is meant to help
  agents choose commands, and shipping zero descriptive text undermines that
  use case. The field is optional and defaults to `None`, which keeps it
  non-breaking under serde semantics and consistent with §8.1's
  legacy-defaulting policy.
  Date/Author: 2026-06-02 / planning agent.

- Decision: exclude `agent-context` from `--format all` for this iteration.
  Rationale: the Logisphere review flagged that bundling an output whose
  semantic fields default to `unknown` into the default group could mislead
  consumers. Defer inclusion until 6.2.2 locks the schema version with
  tests and at least one downstream relies on bundled emission.
  Date/Author: 2026-06-02 / planning agent.

- Decision: write the agent-context output to `<out_dir>/agent-context.json`,
  not under `<out_dir>/agent-context/<package>.json` or `<out_dir>/ir/`.
  Rationale: the artefact is a single, non-localized JSON document; the IR
  per-locale layout does not apply, and a per-package directory makes sense
  only when a workspace bundles several packages' contexts (deferred work).
  Date/Author: 2026-06-02 / planning agent.

- Decision: emit the agent-context with `interaction_mode = unknown`,
  `mutation_effect = unknown`, and `policy.agent_native = warn` (schema
  defaults) for v1.
  Rationale: the schema already establishes these defaults, and §8.1 of
  `docs/agent-native-cli-design.md` documents them as the least-capable
  compatible state until later phases populate them. The users' guide and
  the new `docs/cargo-orthohelp-design.md` §6.3.1 amendment will state
  explicitly that these fields are placeholders pending later roadmap work
  (6.2.2 and 7.1.x).
  Date/Author: 2026-06-02 / planning agent.

- Decision: move this ExecPlan from `DRAFT` to `IN PROGRESS` and begin
  implementation.
  Rationale: the maintainer explicitly requested implementation of the
  planned functionality on 2026-06-04, satisfying the approval gate in the
  plan and repository instructions.
  Date/Author: 2026-06-04 / implementation agent.

## Outcomes & retrospective

Summarize outcomes, gaps, and lessons learned at major milestones or at
completion. Compare the result against the original purpose. Note what would
be done differently next time.

- (to be completed when the plan reaches `COMPLETE`)

## Context and orientation

The reader is assumed to know nothing about the repository. The relevant
files and modules are:

- `docs/roadmap.md` defines the work items. §6.2.1 is the scope of this plan.
- `docs/agent-native-cli-design.md` defines the agent-context contract. §3.2
  describes the JSON shape. §5 lists canonical verbs and flags. §8.1
  documents the defaulting policy for legacy derives.
- `docs/cargo-orthohelp-design.md` §6.3.1 names the cargo-orthohelp side of
  the transform.
- `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`
  establishes the boundary: schema lives in `ortho_config::agent_context`;
  the transform and emitter live in `cargo_orthohelp`.
- `ortho_config/src/agent_context/mod.rs` defines `AgentContext`,
  `AgentCommand`, `AgentInput`, `AgentExample`, `AgentPolicy`,
  `MutationEffect`, `InteractionMode`, `PaginationContract`,
  `AsyncSubmission`, `DeliveryRoute`, `SupportDeclaration`, and
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION = "1"`. These types are already
  re-exported from `ortho_config::lib.rs:61-65`.
- `cargo-orthohelp/src/cli.rs` defines the parser. `OutputFormat` (lines
  16-26) lists the accepted `--format` values. Test
  `format_rejects_unsupported_values` (lines 217-223) currently asserts
  `agent-context` is rejected; this plan inverts that assertion.
- `cargo-orthohelp/src/main.rs::run` (lines 77-129) calls the bridge, parses
  the IR JSON into `DocMetadata`, localizes it for each locale, and routes
  to the per-format emitters. Match arms on `OutputFormat` decide what gets
  written.
- `cargo-orthohelp/src/output.rs::write_localized_ir` (lines 11-48) is the
  existing emitter pattern using `cap_std`/`camino`. A new
  `write_agent_context` will follow the same shape.
- `cargo-orthohelp/src/schema/mod.rs` mirrors `ortho_config::docs::ir`. The
  transform consumes this mirror.
- `cargo-orthohelp/src/bridge.rs::load_or_build_ir` is unchanged.
- `cargo-orthohelp/tests/features/orthohelp_ir.feature` and
  `cargo-orthohelp/tests/rstest_bdd/behaviour/steps_ir.rs` show the existing
  behavioural-test patterns for IR.
- `cargo-orthohelp/src/cli.rs` and `cargo-orthohelp/src/error.rs` show how
  the existing CLI surface, including the unsupported-format error variant,
  is laid out.

Key terms:

- **Agent context**: a compact, machine-readable JSON document that
  describes how an agent should invoke a CLI. Not a localized help artefact.
- **Bridge IR**: the JSON produced by the ephemeral `cargo orthohelp` bridge
  crate; it deserialises into `cargo_orthohelp::schema::DocMetadata`.
- **Canonical verb**: one of `get`, `list`, `create`, `update`, `delete`,
  `jobs`, `profile`, `feedback` (per `docs/agent-native-cli-design.md` §5).
- **Command path**: the ordered list of names from root binary to leaf
  subcommand, for example `["mytool", "profile", "save"]`.
- **Positional input**: a CLI argument that takes a value but is not bound
  to a `--long` or `-s` flag.

## Design overview

The transform reads the bridge IR (`cargo_orthohelp::schema::DocMetadata`),
walks the recursive `subcommands` tree, and produces an `AgentContext`
whose `commands` array contains one entry per node, sorted by full command
path. The transform is pure: same IR in, byte-identical JSON out.

For each `DocMetadata` node visited:

- `path` is built by pushing `app_name` (or `bin_name` if present at the
  root) onto the path inherited from the parent. The root node uses
  `bin_name.unwrap_or(app_name)` as the only element.
- `canonical_verb` is `Some(...)` only when the last element of the
  produced path matches one of the canonical verbs in the table maintained
  alongside the transform. The table is the literal list from
  `docs/agent-native-cli-design.md` §5. Any other case yields `None`.
- `summary` is the short, untranslated description fallback. It is computed
  from the `about_id` Fluent ID by looking up the **en-US** entry only,
  using `ortho_config::FluentLocalizer` against the same consumer-resource
  paths the existing IR pipeline uses. The result is trimmed; if the
  identifier resolves to the sentinel `[missing: <id>]`, `summary` is left
  `None`. The agent-context output remains otherwise unlocalized; only the
  short summary uses en-US so that command-selection prompts are not
  empty.
- `inputs` is built from `fields` by:
  - skipping any field where `cli.is_none()` (env-only or file-only fields
    are out of scope for this iteration);
  - skipping any field where `cli.hide_in_help == true`;
  - mapping the remaining fields to `AgentInput` with:
    - `name = field.name`;
    - `long = cli.long.clone()` (omit `short`; the schema has no short);
    - `value_type` derived from `field.value` via a small table:
      `ValueType::String -> "string"`, `Integer -> "integer"`,
      `Float -> "float"`, `Bool -> "bool"`, `Duration -> "duration"`,
      `Path -> "path"`, `IpAddr -> "ipaddr"`, `Hostname -> "hostname"`,
      `Url -> "url"`, `Enum { .. } -> "enum"`, `List { .. } -> "list"`,
      `Map { .. } -> "map"`, `Custom { name } -> name.clone()`. Missing
      `field.value` yields `None`.
    - `required = field.required`;
    - `default = field.default.as_ref().map(|d| d.display.clone())`;
    - `enum_values = match &field.value { Some(Enum { variants }) =>
      variants.clone(), _ => Vec::new() }`.
- Positional detection: a flag-style input becomes a positional input when
  `cli.long.is_none() && cli.short.is_none() && cli.takes_value`. For the
  v1 schema (which has no `kind` field on `AgentInput`), positional inputs
  carry their name in `name` and have `long = None`; the rule lets later
  schema work add a discriminator without forcing it now.

The resulting `AgentContext` is sorted by command path; each command's
`inputs` are sorted by `name`. Both sorts are stable Unicode lexicographic
sorts, applied in the transform so the snapshot does not depend on the IR's
field ordering.

The emitter writes the document to `<out_dir>/agent-context.json` using
`cap_std` + `camino` and `serde_json::to_string_pretty`, matching the
existing IR emitter's atomicity discipline (open with `create + truncate`,
write, close).

`--format all` continues to mean `ir + man + ps` and does **not** include
`agent-context` in this plan.

## Plan of work

The work is split into seven milestones, each ending with a validation gate.

### Milestone 1: schema additive field

1. In `ortho_config/src/agent_context/mod.rs`, add to `AgentCommand`:

   ```rust
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub summary: Option<String>,
   ```

   Place the field between `path` and `canonical_verb` to keep related
   metadata adjacent. Update the doc-comment on the struct to describe the
   new field.
2. In `ortho_config/src/agent_context/tests.rs`, add table-driven `rstest`
   cases covering: (a) serialising `AgentCommand` with `summary = None`
   omits the field; (b) round-tripping a payload with `summary = Some("…")`
   preserves the field; (c) deserialising a legacy payload without `summary`
   succeeds. Update any tests that pin the exact serialised JSON to expect
   the new field.
3. Update `docs/cargo-orthohelp-design.md` §6.3.1 to record that the
   transform emits a short en-US summary per command and document the
   positional-detection rule.

Validation: `make check-fmt`, `make typecheck`, `make lint`, `make test`
(scoped to `ortho_config` with `cargo test -p ortho_config`) all pass.
Existing `ortho_config::agent_context::tests` snapshots remain stable.

### Milestone 2: CLI flag and parser tests

1. In `cargo-orthohelp/src/cli.rs`, add `AgentContext` to `OutputFormat`
   (clap derives `ValueEnum`, which kebab-cases the variant to
   `agent-context`).
2. Replace the unsupported-format-rejection assertion: change
   `format_rejects_unsupported_values` to use an unmistakably invalid token
   such as `xml`. Add a new test `format_accepts_agent_context` that
   confirms `--format agent-context` parses successfully and yields the
   new variant.
3. Update the `proptest` strategy in
   `parses_option_and_bool_flag_combinations` to include `"agent-context"`
   in the format sample so the parser sees the new variant under random
   inputs.

Validation: `cargo test -p cargo-orthohelp cli::` passes.

### Milestone 3: transform and emitter

1. Add `cargo-orthohelp/src/agent_context/mod.rs` containing:
   - `pub fn bridge_ir_to_agent_context(meta: &DocMetadata, package: &str,
     localizer: Option<&dyn Localizer>) -> AgentContext`. The optional
     localizer is provided only when an en-US localizer is available; the
     function does not build one itself.
   - A private `walk(meta: &DocMetadata, parent_path: &[String], out:
     &mut Vec<AgentCommand>, localizer: Option<&dyn Localizer>)` recursive
     helper.
   - `fn canonical_verb_for(last_segment: &str) -> Option<String>` backed
     by a `const CANONICAL_VERBS: &[&str] = &["get", "list", "create",
     "update", "delete", "jobs", "profile", "feedback"];`.
   - `fn map_value_type(value: &Option<ValueType>) -> Option<String>`.
   - `fn build_input(field: &FieldMetadata) -> Option<AgentInput>` that
     returns `None` for hidden or non-CLI fields and applies the positional
     detection rule for the rest. Repeated/multiple fields keep the same
     `value_type` (variadic handling stays implicit in the `multiple`
     bridge metadata).
   - Sort `out` by `path` after walking; sort each command's `inputs` by
     `name`.
2. Add `cargo-orthohelp/src/output.rs::write_agent_context` mirroring
   `write_localized_ir`, writing to `<out_dir>/agent-context.json`.
3. In `cargo-orthohelp/src/main.rs::run`:
   - Add a `should_generate_agent_context = matches!(args.format,
     OutputFormat::AgentContext)` boolean and a matching `if` branch that
     calls `agent_context::bridge_ir_to_agent_context(&doc_metadata,
     &selection.package_name, Some(en_us_localizer.as_ref()))` and then
     `output::write_agent_context`.
   - Build an en-US localizer once (using existing `locale::build_localizer`
     plus `locale::load_consumer_resources` with the literal `"en-US"`) and
     pass it to the transform. If no en-US resources resolve, fall back to
     the first localized doc's localizer; if none is available, pass
     `None`.
4. Add `pub mod agent_context;` to `cargo-orthohelp/src/main.rs` and
   `cargo-orthohelp/src/lib.rs` (private/public visibility as appropriate).
5. Extend `cargo-orthohelp/src/error.rs::OrthohelpError` only if the
   transform actually needs a new variant; if the transform is total (it is),
   no new variant is required.

Validation: `cargo build -p cargo-orthohelp` and `cargo check --workspace
--all-targets --all-features` succeed.

### Milestone 4: unit and property tests

1. Add `cargo-orthohelp/src/agent_context/tests.rs` driven by `rstest`:
   - Single-level fixture with one flag and one positional argument.
   - Nested fixture with two layers of subcommands.
   - Enum-bearing input with three variants.
   - Default-bearing input.
   - Hidden field (asserted absent).
   - Deprecated field (asserted present; deprecation metadata is otherwise
     ignored in v1).
   - Field with `cli: None` (asserted absent).
   - Field with `value = Some(Custom { name: "Bytes" })` (asserted to map
     to `value_type = "Bytes"`).
   - Localizer present vs absent: asserts `summary` is populated when the
     Fluent ID resolves and `None` when it returns the missing-sentinel.
   - Verb mapping table: a parameterised case asserting `get`, `list`,
     `create`, `update`, `delete`, `jobs`, `profile`, `feedback` map; any
     other token (including `add`, `set`, and capitalised forms) does not.
2. Add `cargo-orthohelp/src/agent_context/proptests.rs` driven by `proptest`
   with one invariant: for any randomly generated `DocMetadata` tree with
   depth ≤ 3 and per-level width ≤ 4, the resulting `AgentContext.commands`
   has unique non-empty paths. Persist regressions under
   `cargo-orthohelp/proptest-regressions/agent_context.txt`.
3. Document in the test file's module comment why the property test is
   intentionally minimal (per the Logisphere review).

Validation: `cargo test -p cargo-orthohelp agent_context::` passes.
`make test` for the full workspace passes.

### Milestone 5: behavioural test and golden snapshot

1. Add `cargo-orthohelp/tests/features/orthohelp_agent_context.feature` with
   one scenario (the fixture pipeline, no cache reuse required):

   ```gherkin
   Feature: cargo-orthohelp agent context output
     Scenario: Generate agent-context JSON
       Given a temporary output directory
       And the orthohelp cache is empty
       When I run cargo-orthohelp with format agent-context for the fixture
       Then the output contains an agent-context JSON document
       And the agent-context JSON matches the golden snapshot
   ```

2. Add step definitions at
   `cargo-orthohelp/tests/rstest_bdd/behaviour/steps_agent_context.rs`
   following the patterns in `steps_ir.rs`:
   - "When I run cargo-orthohelp with format agent-context for the fixture":
     uses the existing step-context helper to invoke the binary with
     `--format agent-context`.
   - "Then the output contains an agent-context JSON document": opens
     `agent-context.json` from the scenario output directory, asserts it
     deserialises into `AgentContext`, asserts `schema_version == "1"`,
     `kind == "<package>.agent_context"`, and `commands` is non-empty.
   - "And the agent-context JSON matches the golden snapshot": uses
     `insta::assert_snapshot!` against the parsed JSON normalised to a
     stable byte sequence with `serde_json::to_string_pretty`. Apply
     `insta::Settings::set_sort_maps(true)`; no redactions are needed
     because the transform itself produces a stable sort order.
3. Register the new step file in
   `cargo-orthohelp/tests/rstest_bdd/behaviour/mod.rs`.
4. Generate the golden snapshot by running
   `INSTA_UPDATE=always cargo test -p cargo-orthohelp \
   --test rstest_bdd agent_context` once, then commit the resulting
   `cargo-orthohelp/tests/golden/agent_context__fixture.json.snap`. Inspect
   the snapshot by hand against the design document before committing.

Validation: `cargo test -p cargo-orthohelp --test rstest_bdd` passes.
`make test` for the full workspace passes.

### Milestone 6: documentation updates

1. Add an "Agent-context output" subsection to `docs/users-guide.md`
   describing `cargo orthohelp --format agent-context`, the output file
   location, the omitted fields (long prose, Fluent identifiers, roff,
   PowerShell wrappers), and the explicit note that `interaction_mode`,
   `mutation_effect`, and `policy.agent_native` are placeholders pending
   later roadmap items.
2. Add a "Generating agent-context output" section to
   `docs/developers-guide.md` documenting the positional-detection rule
   and the en-US-only summary rule.
3. Update `docs/cargo-orthohelp-design.md` §6.3.1 to mark the implemented
   bits and to record the additive `summary` field.
4. Re-check `docs/agent-native-cli-design.md` §9 ("Current gaps to resolve")
   and remove the bullet that says "no compact agent-context format exists"
   once 6.2.1 ships.

Validation: `make markdownlint` and `make nixie` pass.

### Milestone 7: gates and roadmap update

1. Run, in order: `make check-fmt`, `make typecheck`, `make lint`, `make
   test`, `make markdownlint`, `make nixie`, redirecting each into
   `/tmp/$ACTION-cargo-orthohelp-6-2-1-cargo-orthohelp-agent-context-format.out`
   for review.
2. Run `coderabbit review --agent` and resolve every finding before the
   PR is taken out of draft.
3. Mark `docs/roadmap.md` §6.2.1 as `[x]` and tick the three sub-bullets.
4. Update this plan's `Outcomes & retrospective` section.

Validation: every gate passes; `coderabbit review --agent` returns clean.

## Concrete steps

State the exact commands and where to run them. Update as work proceeds.

```bash
# Always run from the repository root.

# Milestone 1
$EDITOR ortho_config/src/agent_context/mod.rs
$EDITOR ortho_config/src/agent_context/tests.rs
$EDITOR docs/cargo-orthohelp-design.md
RUSTFLAGS="-D warnings" cargo test -p ortho_config agent_context:: \
  | tee /tmp/test-cargo-orthohelp-6-2-1-cargo-orthohelp-agent-context-format.out
make check-fmt typecheck lint

# Milestone 2
$EDITOR cargo-orthohelp/src/cli.rs
cargo test -p cargo-orthohelp cli:: \
  | tee /tmp/test-cargo-orthohelp-6-2-1-cargo-orthohelp-agent-context-format.out

# Milestone 3
$EDITOR cargo-orthohelp/src/agent_context/mod.rs
$EDITOR cargo-orthohelp/src/output.rs
$EDITOR cargo-orthohelp/src/main.rs
$EDITOR cargo-orthohelp/src/lib.rs
cargo check --workspace --all-targets --all-features

# Milestone 4
$EDITOR cargo-orthohelp/src/agent_context/tests.rs
$EDITOR cargo-orthohelp/src/agent_context/proptests.rs
cargo test -p cargo-orthohelp agent_context::

# Milestone 5
$EDITOR cargo-orthohelp/tests/features/orthohelp_agent_context.feature
$EDITOR cargo-orthohelp/tests/rstest_bdd/behaviour/steps_agent_context.rs
$EDITOR cargo-orthohelp/tests/rstest_bdd/behaviour/mod.rs
INSTA_UPDATE=always cargo test -p cargo-orthohelp --test rstest_bdd agent_context
$EDITOR cargo-orthohelp/tests/golden/agent_context__fixture.json.snap
cargo test -p cargo-orthohelp --test rstest_bdd

# Milestone 6
$EDITOR docs/users-guide.md
$EDITOR docs/developers-guide.md
$EDITOR docs/cargo-orthohelp-design.md
$EDITOR docs/agent-native-cli-design.md

# Milestone 7
make check-fmt | tee /tmp/check-fmt-cargo-orthohelp-6-2-1-cargo-orthohelp-agent-context-format.out
make typecheck | tee /tmp/typecheck-cargo-orthohelp-6-2-1-cargo-orthohelp-agent-context-format.out
make lint | tee /tmp/lint-cargo-orthohelp-6-2-1-cargo-orthohelp-agent-context-format.out
make test | tee /tmp/test-cargo-orthohelp-6-2-1-cargo-orthohelp-agent-context-format.out
make markdownlint | tee /tmp/markdownlint-cargo-orthohelp-6-2-1-cargo-orthohelp-agent-context-format.out
make nixie | tee /tmp/nixie-cargo-orthohelp-6-2-1-cargo-orthohelp-agent-context-format.out
coderabbit review --agent
$EDITOR docs/roadmap.md
```

Expected outputs:

- Each milestone's `cargo test` invocation reports `test result: ok. N
  passed; 0 failed`.
- `make lint` reports zero clippy or rustdoc warnings.
- `make markdownlint` reports zero violations.
- `coderabbit review --agent` returns "no issues" or an explicitly resolved
  set.

## Validation and acceptance

How to start or exercise the system and what to observe.

Run from the repository root:

```bash
make check-fmt && make typecheck && make lint && make test
```

Then exercise the new format against the existing fixture:

```bash
cargo run -p cargo-orthohelp -- orthohelp \
  --package orthohelp_fixture \
  --format agent-context \
  --out-dir /tmp/orthohelp-agent-context
jq . /tmp/orthohelp-agent-context/agent-context.json
```

Expected: a JSON document with `schema_version: "1"`, a `kind` of
`orthohelp_fixture.agent_context`, and a non-empty `commands` array whose
first entry has `path: ["orthohelp_fixture"]` (or the configured binary
name) and at least one `inputs` entry for the fixture's port field.

Quality criteria:

- Tests: `cargo test -p cargo-orthohelp --all-targets` passes;
  `cargo test -p ortho_config agent_context::` passes; the new
  rstest-bdd scenario passes; the new property test runs cleanly.
- Lint/typecheck: `make lint` and `make typecheck` pass with `-D warnings`.
- Format: `make check-fmt` passes.
- Markdown: `make markdownlint` passes.
- Docs: `make nixie` passes against the updated user's and developer's
  guides.
- Security: no new dependencies and no new I/O paths beyond the existing
  `cap_std` ambient-authority pattern.

Quality method:

- CI runs `make all` plus the rstest-bdd job.
- `coderabbit review --agent` returns clean before the PR is taken out of
  draft.

## Idempotence and recovery

- Re-running `cargo orthohelp --format agent-context` overwrites
  `<out_dir>/agent-context.json` atomically (via cap-std `create +
  truncate`). No cleanup is required between runs.
- Re-running individual milestone command blocks is safe; cargo's
  incremental compilation handles repeated builds.
- If the `insta` snapshot diverges from the transform output, run
  `cargo insta review` to inspect the diff; never accept changes blindly.
- If the property test produces a regression file, commit it under
  `cargo-orthohelp/proptest-regressions/` so future runs replay the
  shrunk case before exploring new ones.

## Artifacts and notes

A sketch of the expected agent-context JSON for the existing fixture
(`cargo-orthohelp/tests/fixtures/...`), to be replaced by the actual
snapshot at Milestone 5:

```json
{
  "schema_version": "1",
  "kind": "orthohelp_fixture.agent_context",
  "package": "orthohelp_fixture",
  "commands": [
    {
      "path": ["orthohelp_fixture"],
      "summary": "Orthohelp fixture configuration.",
      "inputs": [
        {
          "name": "port",
          "long": "port",
          "value_type": "integer",
          "required": false,
          "default": "8080",
          "enum_values": []
        }
      ],
      "output_modes": [],
      "interaction_mode": "unknown",
      "mutation_effect": "unknown",
      "examples": []
    }
  ],
  "profiles": { "supported": false },
  "feedback": { "supported": false },
  "policy": { "agent_native": "warn" }
}
```

The actual snapshot may differ in field ordering, but the transform sorts
commands by path and inputs by name before serialising.

## Interfaces and dependencies

Name the libraries, modules, and services to use and why. Specify the types
that must exist at the end of the milestone.

In `ortho_config/src/agent_context/mod.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCommand {
    pub path: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub canonical_verb: Option<String>,
    // … existing fields unchanged …
}
```

In `cargo-orthohelp/src/cli.rs`:

```rust
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Ir,
    Man,
    Ps,
    All,
    AgentContext,
}
```

In `cargo-orthohelp/src/agent_context/mod.rs`:

```rust
use ortho_config::{AgentCommand, AgentContext, AgentInput, Localizer};
use crate::schema::{DocMetadata, FieldMetadata, ValueType};

pub fn bridge_ir_to_agent_context(
    meta: &DocMetadata,
    package: &str,
    localizer: Option<&dyn Localizer>,
) -> AgentContext;
```

In `cargo-orthohelp/src/output.rs`:

```rust
pub fn write_agent_context(
    out_dir: &Utf8Path,
    payload: &AgentContext,
) -> Result<Utf8PathBuf, OrthohelpError>;
```

Dependencies used:

- `ortho_config` (already declared) for the schema types and `Localizer`.
- `serde_json` (already declared) for serialisation.
- `cap_std` + `camino` (already declared) for filesystem I/O.
- `insta` (dev-dep, already declared) for the golden snapshot.
- `proptest` (dev-dep, already declared) for the single uniqueness
  invariant.
- `rstest` and `rstest-bdd` (already declared) for unit and behavioural
  tests.

No new dependencies are added.

## Documentation signposts

The following documentation must be consulted while implementing this plan:

- `docs/design.md` — overall design philosophy.
- `docs/agent-native-cli-design.md` — the agent-context contract (§3.2),
  canonical verbs and flags (§5), legacy defaulting (§8.1).
- `docs/cargo-orthohelp-design.md` — the bridge pipeline (§6.2) and the
  agent-context pipeline additions (§6.3.1).
- `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md` —
  schema ownership boundary.
- `docs/users-guide.md` — consumer-facing documentation to extend.
- `docs/developers-guide.md` — internal conventions and practices to
  document.
- `docs/rust-testing-with-rstest-fixtures.md` — `rstest` patterns.
- `docs/rust-doctest-dry-guide.md` — doctest discipline; the new public
  functions should include a single doctest each.
- `docs/reliable-testing-in-rust-via-dependency-injection.md` — testing
  patterns relevant to the localizer injection seam.
- `docs/localizable-rust-libraries-with-fluent.md` — Fluent semantics
  needed for the en-US summary lookup.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` —
  refactoring discipline if the transform grows.
- `docs/rtest-bdd-users-guide.md` — `rstest-bdd` patterns.
- ADR-003 — applicable schema-ownership reminders.

The following Rust skills should be loaded while implementing the relevant
sections:

- `rust-router` — entry point for Rust work.
- `rust-types-and-apis` — for the additive `summary` field and any
  trait-bound nuances.
- `rust-errors` — for the transform's failure handling and the (avoided)
  new error variant.
- `arch-crate-design` — to confirm crate boundaries when adding the new
  `cargo_orthohelp::agent_context` module.
- `proptest` — for the single uniqueness invariant.
- `python-testing`-equivalent Rust testing skills via `python-router`
  do not apply; use `rust-testing-with-rstest-fixtures.md` instead.

## Revision note

- 2026-06-02: initial DRAFT created. Reflects the Logisphere design review
  outcome: schema change scoped to a single optional `summary` field;
  positional detection sniffs `cli` data; agent-context output is omitted
  from `--format all`; commands and inputs are sorted in the transform;
  property testing is scoped to one invariant.
