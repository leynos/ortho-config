# Add metadata for non-interactive execution and mutation boundaries (7.2.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

## Purpose / big picture

Agents that drive a command-line tool need to know, before running a command,
whether it will block on a prompt and whether it mutates state. Today the
agent-context output of `cargo orthohelp` (the machine-readable summary of a
CLI that OrthoConfig generates for AI agents) emits
`"interaction_mode": "unknown"` and `"mutation_effect": "unknown"` for every
command, because no mechanism exists for a project author to declare those
facts.

After this change, a project author can annotate a command's arguments struct:

```rust
#[derive(OrthoConfig, OrthoConfigDocs)]
#[ortho_config(
    prefix = "APP",
    behaviour(interaction = "interactive", mutation = "delete", bypass = "--force")
)]
struct PurgeArgs {
    /* ... */
}
```

and three things become observable:

1. The generated documentation intermediate representation (IR) carries a
   `behaviour` block for that command.
2. The agent-context JSON emitted by `cargo orthohelp --format agent-context`
   reports `"interaction_mode": "interactive"`, `"mutation_effect": "delete"`,
   and `"bypass_flag": "--force"` for that command instead of `"unknown"`.
3. Running `cargo orthohelp --check-agent-native` lints the command tree and
   reports destructive commands that lack a declared confirmation bypass flag
   (such as `--force`), as a machine-stable policy report.

This realizes roadmap item 7.2.1 and implements
`docs/agent-native-cli-design.md` §6.1 (non-interactive execution) and §6.4
(mutation boundaries). The design work follows well-established prior art: the
Model Context Protocol tool annotations (`readOnlyHint`, `destructiveHint`,
`idempotentHint` — declared hints that consumers treat as advisory, not proven)
and the Command Line Interface Guidelines (clig.dev) conventions for
`--no-input`, `--force`, and confirmation of dangerous actions.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not workarounds.

- `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` must remain `"1"`. Per
  `docs/agent-native-cli-design.md` §8.2, populating the existing
  `interaction_mode` and `mutation_effect` fields and adding new optional
  fields are additive changes within schema v1. If any step turns out to
  require a breaking change as defined by §8.2 (renaming fields, changing enum
  wire strings, changing serialized defaults, toggling null-versus-omitted),
  stop and escalate.
- The existing `InteractionMode` and `MutationEffect` enum variants and wire
  strings in `ortho_config/src/agent_context/mod.rs` must not be renamed or
  removed. New code maps onto them.
- Dependency direction must not change: `ortho_config_macros` →
  `ortho_config` → `cargo-orthohelp`. The `ortho_config::docs` module must not
  depend on `ortho_config::agent_context` or vice versa (ADR-003 ownership
  split); the bridge transform in `cargo-orthohelp` is the only place the two
  meet. Schema types owned by `cargo_orthohelp::policy` must not gain a `clap`
  derive; the CLI layer defines its own value enums. No new circular dependency
  between crates under any circumstances.
- Undeclared behaviour must remain undeclared. The derive and the bridge must
  never infer interaction or mutation semantics from command names, verbs, or
  flags (design doc §8.1: "Read/write/delete boundaries must not be inferred
  from names"). Absent metadata stays `unknown`.
- No new external dependency. The work uses `syn`/`quote` (already in
  `ortho_config_macros`), `serde` (already everywhere), and the existing dev
  dependencies (`rstest`, `rstest-bdd`, `insta`, `proptest`, `trybuild`,
  `googletest`, `pretty_assertions`).
- All commit gates (`make check-fmt`, `make typecheck`, `make lint`,
  `make test`) must pass at every commit. Red test states are demonstrated and
  recorded transiently within a milestone but are never committed; each
  milestone's commit lands red tests and the code that turns them green
  together. Note the standing repo caveat: `make lint` may be red on `main`
  itself for files outside this diff; check that any lint failure cites files
  this plan touches before treating it as ours.
- British English (en-GB Oxford spelling) in all prose and identifiers
  exposed to users (`behaviour`, not `behavior`), matching the existing
  documentation corpus.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 30 files or roughly
  1,500 net lines of Rust (excluding snapshots and lockfiles), stop and
  escalate.
- Interface: if an existing public API signature in `ortho_config` must
  change (as opposed to gaining new items), stop and escalate.
- Schema: if either `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` or the shape of
  already-serialized agent-context fields must change, stop and escalate.
- Iterations: if a gate still fails after 3 fix attempts on the same failure,
  stop and escalate.
- Ambiguity: if the CodeRabbit review or the gates surface a conflict between
  this plan and `docs/agent-native-cli-design.md`, stop, record the conflict in
  the Decision Log, and escalate.

## Risks

- Risk: the hand-maintained IR mirror in `cargo-orthohelp/src/schema/mod.rs`
  drifts from `ortho_config/src/docs/ir.rs` when the new `behaviour` block is
  added. Severity: medium. Likelihood: medium. Mitigation: milestone B changes
  both files in the same commit, and the golden bridge tests exercise
  deserialization of IR produced by the real derive, so drift fails tests
  immediately.
- Risk: bumping `ORTHO_DOCS_IR_VERSION` from `"1.1"` to `"1.2"` and adding
  the two explicit-null agent-context fields each invalidate many golden
  snapshots at once, making the review diff noisy. Severity: low. Likelihood:
  high. Mitigation: isolate each mechanical churn — the IR version bump and the
  new-null-fields snapshot refresh — in its own commit so behavioural changes
  remain reviewable.
- Risk: the lint's exit-code behaviour pre-empts the exit-code taxonomy work
  scheduled for 7.2.5. Severity: low. Likelihood: medium. Mitigation: use exit
  code 3 for deny-mode findings (distinct from 1 for runtime errors and 2 for
  clap usage errors), document it as provisional in the developers' guide, and
  record the decision so 7.2.5 can supersede it.
- Risk: the attribute surface chosen here constrains later phase-7 items
  (7.2.2 dual renderer, 7.2.3 structured output) that will add more metadata.
  Severity: medium. Likelihood: medium. Mitigation: use one nested
  `behaviour(...)` attribute group scoped to runtime execution semantics only,
  with the admission criterion recorded in ADR-008 (output-contract metadata
  gets sibling groups such as `output(...)` in later items).
- Risk: `#[ortho_config(...)]` parsing silently discards unknown keys at
  struct and field level (`discard_unknown` in
  `ortho_config_macros/src/derive/parse/mod.rs`), so a typo such as
  `behavior(...)`, or `behaviour(...)` placed on a field, would be swallowed
  rather than rejected. Severity: medium. Likelihood: high (en-US spelling and
  field placement are both natural mistakes). Mitigation: inside the recognized
  `behaviour(...)` group, unknown nested keys and invalid values are hard
  `syn::Error`s with spans; the exact key `behavior` is rejected at struct
  level with a spelling hint; both `behaviour` and `behavior` are rejected at
  field level with "behaviour(…) is a struct-level attribute". All four paths
  get trybuild compile-fail fixtures.

## Progress

- [x] (2026-08-06 14:20Z) Reconnaissance of code, design docs, and prior art
  completed (three read-only survey passes plus web research on MCP tool
  annotations and clig.dev).
- [x] (2026-08-06 14:40Z) ExecPlan drafted.
- [x] (2026-08-06 17:30Z) Community-of-experts design review completed (six
  Logisphere lenses across three reviewer panels); revisions applied — see the
  revision note at the bottom of this document.
- [x] (2026-08-07) Plan approved by the user; implementation begun.
- [x] (2026-08-12) Milestone B: IR and agent-context schema types implemented
  red-first; goldens updated; all gates green (check-fmt, typecheck, lint,
  test, markdownlint); CodeRabbit review (`coderabbit review --agent`, PR
  #417) returned 0 findings across 33 reviewed files; pass clear. A follow-up
  spelling fix (`normalisation` -> `normalization`) landed and all gates
  re-verified green.
- [x] (2026-08-12) Milestone C: derive attribute surface (`behaviour(...)`)
  parsed, validated, emitted; trybuild fixtures; gates green; committed
  (`a2ee0e5`); CodeRabbit review (`coderabbit review --agent`) returned 0
  findings across the full branch diff (Milestones B+C); pass clear.
- [x] (2026-08-12/13) Milestone D: bridge population (IR `behaviour` block
  mapped onto `interaction_mode`, `mutation_effect`, `bypass_flag`,
  `dry_run_flag` with no inference), fixture annotations (`admin purge`,
  `admin prune`, `greet`), golden snapshot refreshed, BDD scenario and steps
  added, all gates green (check-fmt, typecheck, lint, test, markdownlint);
  CodeRabbit pass pending.
- [ ] Milestone E: `--check-agent-native` lint with policy report; BDD
  scenarios; gates green; CodeRabbit clear.
- [ ] Milestone F: documentation (design doc §8.1 rows, users' guide,
  developers' guide, ADR-008), roadmap ticked, final gates, final CodeRabbit
  pass.

## Surprises & discoveries

- Observation: the Milestone D plan step "annotate one hello_world command and
  refresh its snapshot" does not apply: `examples/hello_world/src/cli/context.rs`
  hand-authors `AgentCommand` values directly (setting
  `InteractionMode::NonInteractive`/`MutationEffect::ReadOnly` literally) and
  never passes through the derive IR or the bridge, so there is no
  `behaviour(...)` annotation or golden refresh to perform there. The plan's
  fixture-annotation and BDD work in `orthohelp_fixture` exercises the real
  derive-to-bridge path.
- Observation: agent-context schema v1 already reserves `interaction_mode`
  and `mutation_effect` as realized v1 fields defaulting to `"unknown"` (design
  doc §8.1 table), and the Rust enums already exist with the exact variants the
  roadmap asks for. Evidence: `ortho_config/src/agent_context/mod.rs` defines
  `InteractionMode { Unknown, NonInteractive, Interactive }` and
  `MutationEffect { Unknown, ReadOnly, Write, Delete, Submit }`; the bridge
  hardcodes both to default in `cargo-orthohelp/src/agent_context/mod.rs`
  (`walk`). Impact: 7.2.1 is a wiring task plus two new optional fields, not a
  schema redesign. No agent-context version bump is needed.
- Observation: the design doc's §3.3 policy-report example flattens `file`
  and `range` onto each result, but the implemented `PolicyResult` nests them
  under `location`. Evidence: `cargo-orthohelp/src/policy/mod.rs` versus design
  doc lines 248–288. Impact: milestone E follows the implemented Rust types
  (the schema owner per ADR-003) and milestone F corrects the design-doc
  example to match.
- Observation: `init_tracing` in `cargo-orthohelp/src/main.rs` uses the
  `tracing_subscriber::fmt()` default writer, which is stdout, and the codebase
  emits `tracing::debug!` on the main path. Evidence:
  `cargo-orthohelp/src/main.rs` (`init_tracing`, no
  `.with_writer(std::io::stderr)`), found during the expert review. Impact: any
  `RUST_LOG` run would interleave log lines with the stdout JSON policy report.
  Milestone E step 1 redirects tracing to stderr as a prerequisite of the
  stdout report contract.
- Observation: when Milestone C work began, the working tree already
  contained incomplete, non-compiling edits to
  `ortho_config_macros/src/derive/parse/doc_attrs.rs` and `doc_types.rs`
  (mtimes before this session; an unclosed delimiter broke the build). These
  predated the session and matched no commit, so they were discarded and the
  milestone is being implemented cleanly from the committed tree.
- Observation: `ORTHO_DOCS_IR_VERSION` is stamped from the constant by the
  derive, but the literal `"1.1"` is also hard-coded in several test fixtures
  and a doctest, none of which validate against the constant. Evidence:
  `cargo-orthohelp/src/powershell/test_fixtures.rs`,
  `cargo-orthohelp/src/agent_context/proptests.rs`,
  `cargo-orthohelp/src/roff/mod.rs` (test module),
  `cargo-orthohelp/src/agent_context/tests_support.rs`, and the doctest in
  `cargo-orthohelp/src/agent_context/mod.rs`. Impact: milestone B replaces the
  literals with the constant so the version bump cannot leave stale fixtures
  silently green.
- Observation: sub-agent spawning (the `agent` delegation tool) is not
  available in the build environment — attempts are denied by the active tool
  policy. Impact: the execplan's instruction to delegate gate runs and
  CodeRabbit to a `scrutineer` sub-agent cannot be followed literally; the
  build agent runs the full gate sequence and `coderabbit review --agent`
  directly, treating that as the scrutineer role. Evidence: `agent spawn`
  returned "Blocked by policy"; no alternative agent-type is exposed. The
  quality bar is unchanged: every gate and a CodeRabbit review still precede
  each milestone's declaration of done.

## Decision log

- Decision: populate the existing schema v1 fields rather than introduce new
  ones for interaction and mutation; add only `bypass_flag` and `dry_run_flag`
  as new optional `AgentCommand` fields, placed adjacent to `mutation_effect`.
  Rationale: §8.1 reserved the fields for exactly this task; §8.2 classifies
  populating them and adding optional fields as additive within version "1"
  (verified against the §8.2 text during review). Placement is deliberate
  because key order is part of the byte-exact snapshot contract. Date/Author:
  2026-08-06, planning session.
- Decision: authors declare behaviour with a single nested struct-level
  attribute group `#[ortho_config(behaviour(...))]` on the command's arguments
  struct, not on subcommand enum variants. The group is scoped to runtime
  execution semantics only; later roadmap items add sibling groups (for example
  `output(...)`) rather than growing this one indefinitely. Rationale:
  struct-level attributes already flow through the `StructAttrs`/
  `DocStructAttrs` parse path and reach subcommand metadata via the ADR-005
  companion-trait delegation (`metadata_expr` overwrites only `app_name` and
  `about_id`, verified in review). Variant-level attributes have no parse path
  today and would duplicate state. The admission criterion is recorded in
  ADR-008. Date/Author: 2026-08-06, planning session; scope rule added after
  expert review.
- Decision: represent §6.1's three states ("non-interactive, may prompt, or
  requires a bypass flag") as the pair `interaction` × `bypass`:
  `non_interactive`; `interactive` with no bypass (lint fires); `interactive`
  with a declared bypass. No third `InteractionMode` variant. Rationale: §6.1
  treats the bypass flag as a property ("which flag bypasses prompting"), not a
  distinct mode; adding a v1 wire-enum variant needs an unknown-variant
  fallback contract for no expressive gain. This mirrors the MCP annotation
  style of orthogonal hints. ADR-008 records the explicit mapping so the choice
  is not reopened. Date/Author: 2026-08-06, planning session; confirmed by
  expert review.
- Decision: the derive rejects `interaction = "non_interactive"` combined
  with `bypass = ...` as a contradictory declaration (compile error), and the
  destructive-bypass lint exempts commands declared `non_interactive`.
  Rationale: a bypass flag exists to skip a confirmation prompt; a command that
  never prompts has nothing to bypass (clig.dev). A declared non-interactive
  destructive command is the "equivalent approved metadata" the roadmap allows
  in place of `--force`. This resolves both cells of the interaction × bypass
  matrix the first draft left open. Date/Author: 2026-08-06, added after expert
  review.
- Decision: bump `ORTHO_DOCS_IR_VERSION` from `"1.1"` to `"1.2"` for the new
  optional `behaviour` block, and record the IR compatibility reasoning in
  ADR-008, including the skew contract: an older reader given 1.2 IR ignores
  `behaviour` (no `deny_unknown_fields`); a newer reader given 1.1 IR gets
  `behaviour: None` via `#[serde(default)]`. Rationale: prior execplans treat
  IR schema additions as requiring an IR version bump plus an ADR; the design
  doc has no explicit additive-change policy for the IR (unlike §8.2 for agent
  context), so the conservative precedent stands. Date/Author: 2026-08-06,
  planning session; skew contract added after expert review.
- Decision: the lint ships four rules under a new `behaviour` category:
  `agent-native.behaviour.destructive-bypass` (code
  `destructive_bypass_missing`), `agent-native.behaviour.prompt-bypass` (code
  `prompt_bypass_missing`), `agent-native.behaviour.bypass-unknown` (code
  `bypass_flag_unknown`, fired when a declared bypass flag does not match any
  declared input's long flag — contradiction detection between two
  declarations, not name inference), and `agent-native.behaviour.undeclared`
  (codes `interaction_unknown`, `mutation_unknown`). Severity mapping follows
  §8.1: in `warn` mode every finding is `warn`; in `deny` mode every finding,
  including `undeclared`, is `deny` ("the same omitted fields fail CI"). Rule
  identifiers follow the existing fixture convention
  `agent-native.<category>.<check>`. Rationale: the roadmap names the
  destructive check; §6.1 names the prompt-without-bypass check; §8.1 mandates
  that omitted metadata warns in warn mode and fails in deny mode — the first
  draft's "undeclared stays warn in deny mode" contradicted §8.1 and was
  corrected during review. The bypass cross-check was added on review advice
  because the bridge already populates `AgentInput.long`, making it nearly
  free. Date/Author: 2026-08-06, revised after expert review.
- Decision: the CLI takes `--check-agent-native[=off|warn|deny]` via a new
  CLI-layer value enum `CheckMode` with
  `From<CheckMode> for cargo_orthohelp::policy::PolicyMode`, using
  `require_equals = true` and `default_missing_value = "warn"`. The schema type
  `PolicyMode` gains no clap derive. `check_behaviour` takes and reports
  `cargo_orthohelp::policy::PolicyMode` (not the distinct
  `ortho_config::agent_context::PolicyMode`). Rationale: the 7.1.1 policy
  configuration file does not exist yet, so the flag carries the mode; when
  7.1.1 lands the flag becomes an override. Keeping clap out of the policy
  module preserves the ADR-003 ownership split and matches the existing
  `OutputFormat` precedent; `require_equals` avoids the optional-value
  ambiguity footgun. Date/Author: 2026-08-06, revised after expert review.
- Decision: `check_behaviour` is a total function: for `PolicyMode::Off` it
  returns `PolicyReport::empty(Off)` without evaluating rules; `main.rs` may
  additionally skip the call entirely. The report and exit behaviour: the
  policy report is written to stdout as exactly one JSON document, the
  human-readable summary goes to stderr, and the process exits 3 when the
  report contains at least one `deny` finding (0 otherwise). Runtime errors
  keep exit 1 and clap usage errors keep exit 2, so CI can distinguish "policy
  violation" from "tool failure" by exit code, and additionally by the presence
  of a well-formed report on stdout. On bridge failure the lint emits no report
  at all — an empty report would misread as "clean". Writing the report to
  `<out_dir>/policy-report.json` like the format generators was considered and
  rejected: a check is a question, and its answer belongs on stdout for CI
  pipelines; §3.3 requires a machine-stable report "when JSON output is
  requested". 7.2.3 may formalize the stream tier; the interim contract is
  recorded in the developers' guide. Date/Author: 2026-08-06, revised after
  expert review.
- Decision: when `--check-agent-native` is present and `--format` was not
  explicitly provided (detected via clap's value source), the run skips
  artefact generation entirely: the bridge still compiles the target crate to
  obtain IR, but no IR/man/PowerShell files are written. An explicit `--format`
  composes normally with the check (artefacts to `out_dir`, report to stdout).
  Rationale: `--format` defaults to `ir`, so a lint-only CI run would otherwise
  localize and write artefacts nobody asked for. The streams do not conflict
  because format generators write files, never stdout. Date/Author: 2026-08-06,
  added after expert review.
- Decision: the agent-context `policy.agent_native` field keeps its default
  (`warn`) regardless of the `--check-agent-native` mode used in a given run;
  threading the run mode into the emitted context is deferred to 7.1.1, which
  owns per-project policy declaration. Rationale: the emitted context describes
  the project's declared policy, not one invocation's flag; conflating them
  would make the context unstable across CI runs. Recorded here so the mismatch
  is not mistaken for a bug. Date/Author: 2026-08-06, added after expert review.
- Decision: dry-run support is declared as a flag name, not a boolean:
  attribute key `dry_run = "--dry-run"`, IR field `dry_run: Option<String>`,
  agent-context field `dry_run_flag: Option<String>`. Rationale: §6.1's escape
  hatch ("if a project chooses a different convention, it must configure that
  convention once and expose it in agent context") applies to preview flags
  exactly as it does to bypass flags, and §8.2 makes a later bool-to-string
  migration a breaking change — so the string shape must be chosen now.
  Trade-off accepted: the tri-state bool's "declared absent" state is lost;
  absence of declaration means unknown, and an explicit declared-absent marker
  is deferred until a consumer needs it (recorded in ADR-008). Date/Author:
  2026-08-06, revised after expert review (two review lenses disagreed;
  symmetry and §8.2 irreversibility decided it).
- Decision: `mutation = "submit"` neither requires nor implies the existing
  `async_submission` contract on `AgentCommand`; 7.2.1 does not couple them and
  the lint does not cross-check them. Recorded in ADR-008 so 7.2.3 inherits a
  stated position rather than an ambiguity. Date/Author: 2026-08-06, added
  after expert review.
- Decision: the declared bypass grammar is pinned: a bypass value must match
  `--[a-z0-9]+(-[a-z0-9]+)*`. The same grammar applies to `dry_run` values.
  Rationale: "plausible long flag" is unreviewable; a pinned grammar goes in
  ADR-008 and the trybuild `.stderr` goldens. Date/Author: 2026-08-06, added
  after expert review.
- Decision: use `proptest` for serde round-trip invariants of the new types
  and the totality/severity-monotonicity of `check_behaviour`; skip `kani`/
  `verus`. Rationale: the invariants are shallow data-shape and classification
  properties with no unsafe code, no state machine, and no arithmetic — bounded
  model checking or deductive proof would restate the property without adding
  assurance. Date/Author: 2026-08-06, planning session.
- Decision: deferred items recorded for later phases: per-code counts in
  `PolicySummary` (additive, revisit when the report has consumers); advisory
  contradiction detection between an inferred `canonical_verb` of `delete` and
  a declared non-delete mutation (7.1 policy work); threading source spans into
  `PolicyResult.location` (7.1). Date/Author: 2026-08-06, added after expert
  review.

## Outcomes & retrospective

To be completed at milestones and at the end.

## Context and orientation

OrthoConfig is a Rust workspace providing layered configuration and
agent-native CLI documentation. The relevant crates:

- `ortho_config/` — the library. `ortho_config/src/docs/ir.rs` defines the
  documentation IR: `DocMetadata` (one node per command, recursive via
  `subcommands: Vec<DocMetadata>`), with `ORTHO_DOCS_IR_VERSION = "1.1"`
  declared in `ortho_config/src/docs/mod.rs`. That module also declares the
  traits `OrthoConfigDocs` (`get_doc_metadata() -> DocMetadata`) and
  `OrthoConfigSubcommandDocs`
  (`get_subcommand_doc_metadata() -> Vec<DocMetadata>`, ADR-005).
  `ortho_config/src/agent_context/mod.rs` owns the compact agent-facing schema:
  `AgentContext`, `AgentCommand` (which already has
  `interaction_mode: InteractionMode` and `mutation_effect: MutationEffect`,
  both defaulting to `Unknown`), and `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION = "1"`.
- `ortho_config_macros/` — proc macros. Struct- and field-level
  `#[ortho_config(...)]` attributes are parsed in
  `ortho_config_macros/src/derive/parse/` (`mod.rs`, `doc_attrs.rs`,
  `doc_types.rs`); the `get_doc_metadata` body is emitted by
  `generate_docs_impl` in
  `ortho_config_macros/src/derive/generate/docs/mod.rs`, delegating to builders
  in `generate/docs/sections.rs`. The subcommand derive is
  `ortho_config_macros/src/subcommand_docs.rs`; it delegates each variant to
  the inner struct's `get_doc_metadata` (overwriting only `app_name` and
  `about_id`), so struct-level metadata reaches subcommands automatically.
  Errors are `syn::Error` with spans, surfaced as compile errors; compile-fail
  coverage uses `trybuild` fixtures in `ortho_config/tests/ui/*.rs` with
  `*.stderr` goldens.
- `cargo-orthohelp/` — the `cargo orthohelp` tool. It keeps a hand-maintained
  mirror of the IR in `cargo-orthohelp/src/schema/mod.rs` ("Keep this in sync
  with `ortho_config::docs`"); a test in
  `cargo-orthohelp/src/schema/tests/mod.rs` pins the mirrored version constant.
  `cargo-orthohelp/src/agent_context/mod.rs` contains
  `bridge_ir_to_agent_context`, whose internal `walk` currently sets
  `interaction_mode: InteractionMode::default()` and
  `mutation_effect: MutationEffect::default()` unconditionally — the wiring gap
  this plan closes. `cargo-orthohelp/src/policy/mod.rs` defines the
  policy-report schema (`PolicyReport`, `PolicyResult`,
  `PolicyMode { Off, Warn, Deny }`, `ORTHO_POLICY_REPORT_SCHEMA_VERSION = "1"`)
  but no rule or runner exists yet. The CLI (`cargo-orthohelp/src/cli/mod.rs`)
  has `--format <ir|man|ps|all|agent-context>` (defaulting to `ir`); dispatch
  lives in `cargo-orthohelp/src/main.rs` (`run`): the bridge compile
  (`bridge::load_or_build_ir`) runs once per invocation before any format
  branching, and `bridge_ir_to_agent_context` is a pure in-memory transform
  over the resulting `DocMetadata` — so the lint re-runs the transform, not the
  bridge. Format generators write files to `out_dir`; nothing currently writes
  to stdout.
- `tests/fixtures/orthohelp_fixture/` — a fixture crate compiled by
  cargo-orthohelp's ephemeral bridge during tests (`SimpleFixtureConfig`,
  `FixtureConfig`, `NestedFixtureConfig` with a three-level subcommand tree
  including `admin` → `audit`/`grant-access`).
- `examples/hello_world/` — a downstream-style example with its own
  agent-context BDD feature and insta snapshot.

The BDD harness for cargo-orthohelp shells out to the real binary via
`std::process::Command`
(`cargo-orthohelp/tests/rstest_bdd/behaviour/ steps_cmd.rs`, `run_orthohelp`)
and stores the resulting `std::process::Output`, so process exit codes and
stdout are directly assertable in scenarios.

Terms: "IR" is the localized documentation intermediate representation (JSON).
"Agent context" is the compact machine-oriented JSON summary
(`kind: "<tool>.agent_context"`). "Bridge" is cargo-orthohelp's build step that
compiles the target crate, obtains IR, and transforms it. A "golden snapshot"
is an `insta` snapshot committed under `cargo-orthohelp/tests/` asserting
byte-exact output. "Destructive" in this plan means `mutation = "delete"`.

Normative sources: `docs/agent-native-cli-design.md` §6.1 ("Metadata should
state whether a command is non-interactive, may prompt, or requires a bypass
flag… The preferred non-interactive flag is `--no-input`. The preferred
destructive bypass flag is `--force`."), §6.4 ("Mutating commands should
declare whether they are read-only, write, delete, or submit asynchronous work.
Destructive commands should declare their confirmation bypass flag.
Consequential commands should declare whether `--dry-run` exists."), §3.3
(policy modes and report fields), §8.1 (defaults for legacy derives, and the
warn/deny handling of omitted metadata), §8.2 (schema v1 compatibility policy).

Relevant skills for implementers: `rust-router` (entry point),
`rust-types-and-apis` (attribute and enum shape), `rust-unit-testing`
(rstest/googletest/insta patterns), `proptest`, `leta` (navigation),
`commit-message`, `comenq-coderabbit` (review loop), and the repo guides
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`,
`docs/localizable-rust-libraries-with-fluent.md` (why behaviour metadata must
not carry Fluent identifiers into agent context), and
`docs/documentation-style-guide.md` (ADR format).

## Plan of work

The work proceeds in five milestones, B–F (the former standalone red-test
milestone A was folded into B during review: committing non-compiling tests
would violate the gates-green constraint, so each milestone demonstrates its
red state transiently — run the new tests, record the expected failure — and
commits red tests and implementation together once green). Every milestone ends
with the full gate sequence run by the `scrutineer` subagent (`make check-fmt`,
`make typecheck`, `make lint`, `make test`), one or more commits, and a
`coderabbit review --agent` pass whose concerns are cleared before the next
milestone.

### Milestone B — IR and agent-context schema types

Red first: write the unit tests below, run them, and record the expected
failures; then implement and commit tests plus implementation together.

Red tests: in `ortho_config/tests/docs_ir.rs`, a test asserting that
`DocMetadata` deserializes a JSON document containing a `behaviour` object with
`interaction`, `mutation`, `bypass`, and `dry_run` keys, and that a document
without the key deserializes with `behaviour: None`. In
`ortho_config/src/agent_context/tests_json.rs` (and the wire-contract snapshot
test), assertions that `AgentCommand` serializes `bypass_flag` and
`dry_run_flag` as explicit `null` when absent — matching the existing
convention in which only `summary` is omitted when absent — and round-trips
declared values.

Implementation:

1. `ortho_config/src/docs/ir.rs`: add

   ```rust
   /// Declared execution behaviour for a command (agent-native metadata).
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
   pub struct BehaviourMetadata {
       /// Interaction declaration; `None` means undeclared.
       #[serde(default)]
       pub interaction: Option<InteractionKind>,
       /// Mutation boundary declaration; `None` means undeclared.
       #[serde(default)]
       pub mutation: Option<MutationKind>,
       /// Confirmation/prompt bypass flag, for example `--force`.
       #[serde(default)]
       pub bypass: Option<String>,
       /// Dry-run flag name, for example `--dry-run`; `None` means
       /// undeclared.
       #[serde(default)]
       pub dry_run: Option<String>,
   }
   ```

   with `InteractionKind { NonInteractive, Interactive }` and
   `MutationKind { ReadOnly, Write, Delete, Submit }`
   (`#[serde(rename_all = "snake_case")]`), and on `DocMetadata` a new
   `#[serde(default)] pub behaviour: Option<BehaviourMetadata>` following the
   `HeadingIds::commands` precedent. The IR enums deliberately have no
   `Unknown` variant: undeclared is represented by `None` at the IR layer, and
   only the agent-context layer has `Unknown`.
2. `ortho_config/src/docs/mod.rs`: bump `ORTHO_DOCS_IR_VERSION` to `"1.2"`.
   Replace hard-coded `"1.1"` literals with the constant where fixtures and
   doctests currently embed it
   (`cargo-orthohelp/src/powershell/ test_fixtures.rs`,
   `cargo-orthohelp/src/agent_context/proptests.rs`,
   `cargo-orthohelp/src/roff/mod.rs` test module,
   `cargo-orthohelp/src/agent_context/tests_support.rs`, doctest in
   `cargo-orthohelp/src/agent_context/mod.rs`).
3. Mirror the IR changes in `cargo-orthohelp/src/schema/mod.rs` (same
   commit as step 1).
4. `ortho_config/src/agent_context/mod.rs`: add to `AgentCommand`, directly
   after `mutation_effect`: `#[serde(default)] pub bypass_flag: Option<String>`
   and `#[serde(default)] pub dry_run_flag: Option<String>` (explicit null when
   absent).
5. Update the wire-contract snapshot, the three golden agent-context
   snapshots under `cargo-orthohelp/tests/golden/`, the hello_world snapshot,
   and any IR snapshots that embed `ir_version`. Keep the two mechanical churns
   — the IR version bump and the new-null-fields refresh — in separate commits
   from the type changes.
6. Add proptest round-trip properties (in the existing proptest homes)
   asserting serialize→deserialize identity for `BehaviourMetadata` and for
   `AgentCommand` values covering the new fields.

### Milestone C — derive attribute surface

Follow the documented end-to-end pattern for a new struct-level key (the
`windows(...)`/`precedence(...)` template):

1. `ortho_config_macros/src/derive/parse/doc_types.rs`: add a
   `BehaviourAttrs` struct and a field on `DocStructAttrs`.
2. `ortho_config_macros/src/derive/parse/doc_attrs.rs`: add a `behaviour`
   match arm in `apply_struct_doc_attr` and a `parse_behaviour_meta` helper.
   Valid keys: `interaction` (string, one of `non_interactive`, `interactive`),
   `mutation` (string, one of `read_only`, `write`, `delete`, `submit`),
   `bypass` (string matching `--[a-z0-9]+(-[a-z0-9]+)*`), `dry_run` (string,
   same grammar). Unknown nested keys and invalid values are `syn::Error`s with
   the offending span. Cross-key validation: `interaction = "non_interactive"`
   combined with `bypass` is a compile error (contradictory declaration). At
   struct level, explicitly reject the key `behavior` with "unknown attribute
   `behavior`; use the en-GB spelling `behaviour`". At field level
   (`parse_field_attrs`/`apply_field_doc_attr`), explicitly reject both
   `behaviour` and `behavior` with "behaviour(…) is a struct-level attribute"
   so the silent `discard_unknown` path cannot swallow the misplacement.
   Refresh the stale doc comment on `parse_struct_attrs` (it still claims only
   the `prefix` key is supported) while in the file.
3. `ortho_config_macros/src/derive/generate/docs/sections.rs` (or a small
   sibling): `build_behaviour_metadata` emitting the
   `Option<BehaviourMetadata>` token stream; wire it into the `quote!` block of
   `generate_docs_impl` in `generate/docs/mod.rs`.
4. Tests: rstest unit tests for the parser under
   `ortho_config_macros/src/derive/parse/tests/`; IR-shape tests in
   `ortho_config/tests/docs_ir.rs` and
   `ortho_config/tests/docs_ir_subcommands.rs` proving behaviour metadata flows
   through the ADR-005 subcommand delegation unchanged, including a test
   pinning that an args struct reused as both root and subcommand carries
   identical behaviour in both positions (correct by design; the test stops
   anyone "fixing" it later). Trybuild compile-fail fixtures with `.stderr`
   goldens: `ortho_config/tests/ui/behaviour_invalid_interaction.rs`,
   `behaviour_invalid_mutation.rs`, `behaviour_bad_bypass.rs`,
   `behaviour_unknown_nested_key.rs` (for example
   `behaviour(interation = ...)`), `behaviour_noninteractive_bypass.rs` (the
   contradiction), `behaviour_en_us_spelling.rs`, and `behaviour_on_field.rs`.
5. Extend the doctest example in `ortho_config/src/docs/mod.rs` minimally,
   or add a new one, following `docs/rust-doctest-dry-guide.md`.

Red first within the milestone: the parser unit tests and IR-shape tests are
written and run before the parse/emit code exists, with failures recorded;
trybuild fixtures are red-by-construction until the errors they expect are
implemented.

### Milestone D — bridge population and fixtures

1. `cargo-orthohelp/src/agent_context/mod.rs`: in `walk`/`build_input`, map
   the IR `behaviour` block: `Some(NonInteractive)` →
   `InteractionMode::NonInteractive`, `Some(Interactive)` →
   `InteractionMode::Interactive`, `None` → `Unknown`; likewise for
   `MutationKind` → `MutationEffect`; copy `bypass` → `bypass_flag` and
   `dry_run` → `dry_run_flag`. No inference, no defaults beyond `Unknown`.
2. Annotate the fixture crate with all the states later milestones need:
   one fully declared destructive command
   (`behaviour(interaction =
   "interactive", mutation = "delete", bypass = "--force")`,
   for example on the `grant-access` args struct or a new `purge` subcommand),
   one *declared destructive command without a bypass* (for example a new
   `admin prune` subcommand declaring only `mutation = "delete"` — this is the
   command milestone E's `destructive_bypass_missing` scenarios lint), one
   read-only non-interactive command
   (`behaviour(interaction = "non_interactive", mutation = "read_only")`), and
   at least one command left unannotated to lock the `unknown` passthrough.
   These annotations appear in milestone D's golden snapshots — that is
   intended and keeps milestone E fixture-neutral.
3. Update golden snapshots and the hello_world example (annotate one
   command there and refresh its snapshot).
4. BDD: extend
   `cargo-orthohelp/tests/features/orthohelp_agent_context.feature` with a
   scenario, for example:

   ```gherkin
   Scenario: agent context reports declared behaviour metadata
     Given the nested fixture workspace
     When I generate agent context for the nested fixture
     Then the command "admin purge" reports interaction mode "interactive"
     And the command "admin purge" reports mutation effect "delete"
     And the command "admin purge" reports bypass flag "--force"
     And the command "greet" reports interaction mode "non_interactive"
     And the command "version" reports interaction mode "unknown"
   ```

   with steps in the adjacent rstest-bdd harness, using `googletest` assertions
   and `pretty_assertions` for diffs, per `docs/rstest-bdd-users-guide.md`.

Red first: the BDD scenario and bridge unit tests are written against the
annotated fixture before the `walk` mapping exists, and fail with `unknown`
values; the mapping turns them green.

### Milestone E — the `--check-agent-native` lint

1. Prerequisite: point `init_tracing` in `cargo-orthohelp/src/main.rs` at
   stderr (`.with_writer(std::io::stderr)`), so tracing output can never
   interleave with the stdout JSON report. This is a behaviour fix in its own
   right and lands as the milestone's first commit.
2. `cargo-orthohelp/src/cli/mod.rs`: add a CLI-layer value enum and flag:

   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
   pub enum CheckMode {
       Off,
       Warn,
       Deny,
   }

   #[arg(long = "check-agent-native", value_enum, num_args = 0..=1,
         require_equals = true, default_missing_value = "warn")]
   pub check_agent_native: Option<CheckMode>,
   ```

   with `From<CheckMode> for cargo_orthohelp::policy::PolicyMode`. The schema
   `PolicyMode` gains no clap derive.
3. New module `cargo-orthohelp/src/policy/rules/behaviour.rs`: a pure,
   total function
   `fn check_behaviour(context: &AgentContext, mode:
   PolicyMode) -> PolicyReport`
   implementing the four rules and the severity mapping from the Decision Log
   (`Off` returns `PolicyReport::empty(Off)` without evaluating rules). Each
   `PolicyResult` carries `rule_id`, `code`, `severity`, and a `message` that
   is the entire operator experience (source spans are unavailable from agent
   context, so `location: None`): the message names the command path *and* the
   exact remedy as an annotation snippet, for example: "command `admin prune`
   is destructive but declares no bypass flag; add
   `behaviour(bypass = \"--force\")` to its arguments struct". The exact
   wording is locked by an insta snapshot. The destructive rule exempts
   commands declared `non_interactive` (see Decision Log).
4. `cargo-orthohelp/src/main.rs`: after the bridge IR is loaded, when the
   flag is present, run the transform (not a second bridge build) to obtain the
   `AgentContext`, run the check, serialize the `PolicyReport` as exactly one
   JSON document to stdout, write a human-readable summary to stderr, and exit
   3 if and only if the report contains at least one `deny` finding. Skip
   default-format artefact generation when `--format` was not explicitly given
   (clap value source); compose normally when it was. On bridge failure, emit
   no report (existing error path, exit 1).
5. Tests: rstest unit tests for `check_behaviour` covering happy paths
   (fully declared tree yields an empty report), each of the four rules firing,
   the severity mapping per mode, and edge cases (declared `bypass` on a
   non-destructive command produces no finding; `submit` and `write` commands
   do not trigger the destructive rule; a `non_interactive` destructive command
   without bypass produces no `destructive_bypass_missing` finding; empty
   command list; `Off` returns an empty report). An insta snapshot locks the
   JSON policy report for the fixture tree in warn mode (multivariant
   output-format consistency). A new BDD feature
   `cargo-orthohelp/tests/features/orthohelp_policy.feature`:

   ```gherkin
   Scenario: destructive command without a bypass flag fails deny mode
     Given the nested fixture workspace
     When I run cargo orthohelp with --check-agent-native=deny
     Then the policy report on stdout contains code "destructive_bypass_missing"
     And the process exit code is 3

   Scenario: warn mode reports findings without failing
     Given the nested fixture workspace
     When I run cargo orthohelp with --check-agent-native=warn
     Then the policy report on stdout contains code "destructive_bypass_missing"
     And the process exit code is 0

   Scenario: the check composes with agent-context generation
     Given the nested fixture workspace
     When I run cargo orthohelp with --format=agent-context and --check-agent-native=warn
     Then the agent context file is written to the output directory
     And the policy report on stdout is valid JSON
   ```

   Step implementations read stdout from the harness's stored
   `std::process::Output` (`last_output`), assert `code() == Some(3)` rather
   than merely "not success", and always pair the exit-code assertion with a
   well-formed-report assertion so a crash exiting with a different code cannot
   false-pass. These scenarios exercise the real binary because the lint is an
   externally observable CLI contract.
6. Proptest: properties over arbitrary `AgentCommand` vectors asserting
   `check_behaviour` is total, that a report never contains `deny` severity
   when the mode is `warn` or `off`, and that `off` always yields an empty
   result list.

Red first: the unit tests and BDD scenarios are written before the rule module
and flag exist; the flag's absence makes the scenarios fail with a clap usage
error, recorded as the red evidence.

### Milestone F — documentation and closure

1. `docs/agent-native-cli-design.md`: add `bypass_flag` and `dry_run_flag`
   rows (status v1, default `null`) to the §8.1 table; correct the §3.3 example
   to the implemented `location` nesting; note in §6.1/§6.4 that the metadata
   is now realized.
2. New `docs/adr-008-behavioural-metadata-attribute-surface.md` (per
   `docs/documentation-style-guide.md`): records the attribute shape and its
   admission criterion (runtime execution semantics only), the §6.1 three-state
   mapping onto the interaction × bypass pair, the non-interactive/bypass
   contradiction rule, the pinned bypass/dry-run flag grammar, the dry-run
   string-not-bool trade-off, the `mutation = "submit"` versus
   `async_submission` non-coupling, the IR-version bump policy applied and the
   version-skew contract, and the no-inference rule; referenced from
   `docs/design.md`'s decision log.
3. `docs/users-guide.md`: extend the "Documentation and agent contracts"
   section and the `OrthoConfigDocs` worked examples with `behaviour(...)`;
   document the lint flag, the report shape, the exit codes, and — up front —
   that a first run over an unannotated CLI will report `undeclared` findings
   for every command: annotate incrementally, starting with destructive
   commands. Note the current no-source-location limitation where users will
   look for it.
4. `docs/developers-guide.md`: update schema-ownership and
   agent-context-surface sections with the new fields, the rule-id convention,
   the stdout/stderr stream contract, and the provisional exit-code decision (3
   = policy findings; superseded by 7.2.5).
5. `docs/roadmap.md`: mark 7.2.1 and its three sub-bullets done.
6. Final full gate run via `scrutineer`, final `coderabbit review --agent`,
   final commit.

## Concrete steps

All commands run from the repository root. Long outputs are captured with `tee`
to `/tmp/$ACTION-$(get-project)-$(git branch --show-current).out` per
repository convention; gate runs are delegated to the `scrutineer` subagent,
which does this automatically.

Branch setup (done at plan time):

```bash
git branch -m 7-2-1-metadata-for-non-interactive-execution-and-mutation-boundaries
git push -u origin 7-2-1-metadata-for-non-interactive-execution-and-mutation-boundaries
```

Red evidence, per milestone (run before implementing, record the failure, do
not commit the red state):

```bash
cargo test -p ortho_config docs_ir 2>&1 | tee /tmp/red-b.out
# expect: compile error naming BehaviourMetadata / missing field `behaviour`
```

Green evidence per milestone (representative):

```bash
cargo test -p ortho_config 2>&1 | tee /tmp/test-ortho.out          # B, C
cargo test -p cargo-orthohelp 2>&1 | tee /tmp/test-orthohelp.out   # D, E
cargo insta review   # only to inspect; snapshots are committed deliberately
```

Full gates after each milestone (delegated to scrutineer):

```bash
make check-fmt && make typecheck && make lint && make test
```

CodeRabbit after gates are green:

```bash
coderabbit review --agent 2>&1 | tee /tmp/coderabbit-7-2-1.out
```

## Validation and acceptance

Acceptance is behavioural:

1. Annotating a fixture command with
   `behaviour(interaction = "interactive", mutation = "delete", bypass = "--force")`
   and running the bridge yields agent-context JSON in which that command
   reports `"interaction_mode": "interactive"`, `"mutation_effect": "delete"`,
   `"bypass_flag": "--force"`, while an unannotated command still reports
   `"unknown"` for both enums and `null` for the new fields. Proven by the
   golden snapshots and the `orthohelp_agent_context.feature` scenario above.
2. `cargo orthohelp --check-agent-native` (warn) on the fixture tree prints
   exactly one JSON `PolicyReport` (schema version "1") to stdout listing
   `destructive_bypass_missing` for the fixture's declared-destructive,
   bypass-less command and exits 0; `--check-agent-native=deny` exits 3. Proven
   by the `orthohelp_policy.feature` scenarios.
3. Misdeclarations fail to compile: `behaviour(interaction = "sometimes")`,
   `behaviour(mutation = "destroy")`, `behaviour(bypass = "force")`,
   `behaviour(interation = ...)`,
   `behaviour(interaction = "non_interactive", bypass = "--force")`,
   `behavior(...)` at struct level, and `behaviour(...)` on a field each
   produce their trybuild-goldened compile error.
4. `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` still equals `"1"`;
   `ORTHO_DOCS_IR_VERSION` equals `"1.2"`; asserted by existing and new unit
   tests.
5. Red-Green-Refactor evidence is recorded per milestone in `Progress` and
   `Artefacts and notes`: the red command and its expected failure, the green
   run, and the post-refactor gate run. Red states are demonstrated transiently
   and never committed (see Constraints).

Quality criteria: all four make gates pass at every commit; CodeRabbit concerns
cleared at each milestone; no new dependencies; snapshot diffs reviewed rather
than blindly accepted.

## Idempotence and recovery

Every milestone is one or more ordinary commits on the task branch; recovery is
`git revert` or resetting to the previous milestone commit. Snapshot
regeneration (`cargo insta`) is idempotent. The IR version bump commit is
isolated so it can be reverted independently while milestone B is in flight;
once milestone C lands, later work depends on `"1.2"` (snapshots and the bridge
cache key embed `ir_version`), so rollback from that point is by reverting
ranges, not the single commit. No step touches state outside the repository
except `/tmp` logs.

## Artefacts and notes

Prior-art evidence gathered during planning:

- MCP tool annotations (spec 2025-06-18): `readOnlyHint`, `destructiveHint`,
  `idempotentHint`, `openWorldHint`; "Clients MUST consider tool annotations to
  be untrusted unless they come from trusted servers." The declared-hint (not
  proven) stance matches this plan's no-inference constraint: OrthoConfig
  transports declarations, it does not verify runtime behaviour.
- clig.dev: "Never require a prompt… If `--no-input` is passed, don't
  prompt or do anything interactive"; "-f, --force… doing something destructive
  that usually requires user confirmation"; `-n, --dry-run` as the standard
  dry-run flag. These are the canonical flag names §6.1 already adopted.

Implementation transcripts will be appended here per milestone.

### Milestone B transcript (2026-08-07)

Red evidence (types absent):
`cargo test -p ortho_config --test docs_ir behaviour` failed with E0432 (no
`InteractionKind`/`MutationKind` in `ortho_config::docs`) and E0609 (no field
`behaviour` on `DocMetadata`); `cargo test -p ortho_config --lib agent_context`
failed with E0609 (no `bypass_flag`/`dry_run_flag` on `AgentCommand`). Logs:
`/tmp/red-b-docs-ir-ortho-config-<branch>.out`,
`/tmp/red-b-agent-context-ortho-config-<branch>.out`.

Green evidence: after adding `BehaviourMetadata`/`InteractionKind`/
`MutationKind` plus `DocMetadata::behaviour` (IR and cargo-orthohelp mirror),
`AgentCommand::bypass_flag`/`dry_run_flag`, the derive emitting
`behaviour: None`, and the bridge defaulting both flags to `None`, the full
workspace suite passes (61 `test result: ok` lines, 0 failures; log
`/tmp/test-b-workspace-<branch>.out`). One red-to-green iteration: the
snake-case wire-value test initially compared against the partial input JSON,
but `BehaviourMetadata` serializes absent keys as explicit `null` (matching the
IR convention), so the expectation was corrected to the explicit-null form.

Snapshot churn reviewed and accepted (diffs limited to the two new explicit
nulls after `mutation_effect`, plus insta `assertion_line` metadata):
`cargo-orthohelp/tests/golden/agent_context__{fixture,nested_fixture,simple_fixture}.json.snap`
and
`examples/hello_world/tests/snapshots/agent_context_snapshot__context_agent_context_json.snap`.
The wire-contract fixture gained the same two nulls. No `.snap` file embeds
`ir_version`, so the IR version bump (next commit) churns no snapshots.

Commit structure: schema types and their snapshot churn land first; the
`ORTHO_DOCS_IR_VERSION` bump from `"1.1"` to `"1.2"` and the replacement of
hard-coded `"1.1"` literals with the constant land in a separate commit.

Additional literal sites found during the bump (not in the original list):

- `ortho_config/tests/features/docs_ir.feature` names the version literally
  (`Then the IR version is 1.1` → `1.2`); the `scenarios!` macro embeds feature
  files at compile time, so cargo does not rebuild on feature edits alone —
  touch the scenario source to force it.
- `cargo-orthohelp/tests/fixtures/nested_fixture_impl.rs` is a shared macro
  expanded in both the lib test-support tree (`crate::schema`) and the
  integration-test tree (`cargo_orthohelp::schema`); it now references the
  unqualified constant and each call site imports it.
- `docs/cargo-orthohelp-design.md` records the IR version in §2, §6.4.1,
  §12, and §13.1; refreshed alongside the bump (Milestone F's doc list did not
  include this file, so the version references were updated here).

Red-to-green notes for the bump commit: the first workspace run after the
constant change failed on the `docs_ir` BDD scenario (expected 1.1, got 1.2),
fixed by the feature-file update above. Clippy then denied four
`indexing_slicing` uses of `value["behaviour"]` in the new `docs_ir.rs` tests;
the tests were rewritten to use `as_object_mut().insert` and `.get(...)` with
`anyhow` errors instead. Final state: 61 `test result: ok` groups, clippy clean.

Milestone B closure (2026-08-12): all deterministic gates are green on the
schema-type commits plus the IR version bump: `make check-fmt`,
`make typecheck`, `make lint` (rustdoc, clippy, Whitaker), `make test` (886
Rust tests plus 106 pytest cases pass, 0 failures), and `make markdownlint` all
exit 0.

One gate finding was ours and fixed during closure: the Whitaker
`module_max_lines` rule flagged `cargo-orthohelp/src/agent_context/mod.rs` at
402 lines (> 400). The branch base (including 6.2.2) already sat at 397 lines;
the milestone's two construction lines and the doctest addition pushed it over
the ceiling. The fix is a separate atomic refactor commit: the self-contained
Rust-literal/path display normalization block (quote and raw-string state
tracking, character-literal detection, path-separator rewriting — 152 lines)
moved to a sibling `default_display` module, and `mod.rs` re-exports
`normalize_default_display` so the unit and property tests' `use super::...`
call-sites are unchanged. This is a behaviour-neutral split; the agent-context
unit and property tests still pass unchanged.

### Milestone C transcript (2026-08-12)

Red evidence (attribute surface absent): the new parser unit tests
(`ortho_config_macros/src/derive/parse/tests/behaviour_attrs.rs`, 10 cases)
failed to compile with E0412 (`BehaviourAttrs` missing) and E0609 (no
`behaviour` field on `DocStructAttrs`) before the parse types existed. Logs:
`/tmp/red-c-macros-7-2-1.out`, `/tmp/red-c-macros-7-2-1b.out`.

Green evidence: after adding `BehaviourAttrs` (parse) plus the `behaviour`
match arms, the grammar/contradiction checks, the generate-side
`build_behaviour_metadata`, and the emission wiring in `generate_docs_impl`,
all 10 parser unit tests pass and the full `ortho_config_macros` lib suite is
green (133 passed). The integration tests in `ortho_config/tests/docs_ir.rs`
(3 new derive-emission cases) and `docs_ir_subcommands.rs` (3 new ADR-005
delegation cases, including the reused-args pinning test) pass. Full workspace
`make test` reports 902 passed / 0 failed (up from 886 in Milestone B).

Two implementation adaptations worth recording:

- The `lit_str(...)?.value()` pattern cannot reuse `nested.value()?.span()` for
  the error branch afterwards (input already consumed); the functions capture
  the `LitStr` span once and report errors against it. Without this, invalid
  values surfaced as a confusing "expected `=`" parse error.
- The flag grammar requires the `--` prefix: `strip_prefix("--").unwrap_or(...)`
  would accept a bare word. `unwrap_or("")` makes `force` invalid and
  `--force` valid, matching ADR-008's pinned grammar.

Trybuild compile-fail fixtures (7) added under `ortho_config/tests/ui/` with
`.stderr` goldens: `behaviour_invalid_interaction.rs`,
`behaviour_invalid_mutation.rs`, `behaviour_bad_bypass.rs`,
`behaviour_unknown_nested_key.rs`, `behaviour_noninteractive_bypass.rs`,
`behaviour_en_us_spelling.rs`, `behaviour_on_field.rs`. Each `.stderr` locks
the exact span-bearing error text.

The derive doctest example on `OrthoConfigSubcommandDocs` in
`ortho_config/src/docs/mod.rs` gained a `behaviour(...)` declaration on the
`RunArgs` struct.

Gate state at milestone commit: `make check-fmt`, `make typecheck`,
`make lint` (rustdoc, clippy, Whitaker), and `make test` all green. CodeRabbit
pass for the milestone is run after this update is committed (see Progress).

Note: when Milestone C work began, the working tree already held incomplete,
non-compiling edits to the parse files from before this session; these were
discarded (see Surprises & discoveries) and the milestone was implemented
cleanly from the committed tree.

### Milestone C closure (2026-08-12)

The `#[ortho_config(behaviour(...))]` surface is parsed, validated, and
emitted by the derive, committed as `a2ee0e5` and pushed.

- Parser (`ortho_config_macros/src/derive/parse/behaviour_attrs.rs`):
  nested keys `interaction`, `mutation`, `bypass`, `dry_run`; hard `syn::Error`
  with spans for unknown nested keys and invalid values; the pinned
  `--[a-z0-9]+(-[a-z0-9]+)*` flag grammar for `bypass`/`dry_run`; the
  `non_interactive` + `bypass` contradiction rejection; struct-level `behavior`
  en-GB spelling hint and field-level rejection of both spellings.
- Emitter (`ortho_config_macros/src/derive/generate/docs/behaviour.rs`):
  builds the `Option<BehaviourMetadata>` token stream, mapping validated
  strings onto `InteractionKind`/`MutationKind`; undeclared stays `None`
  (no inference). Replaces the Milestone B `behaviour: None` placeholder in
  `generate/docs/mod.rs`. `unreachable!` arms were replaced with total
  `None`-mapping matches during gate fixing (clippy denies `unreachable`).
- Tests: rstest parser tests (`parse/tests/behaviour_attrs.rs`), derive
  emission tests in `ortho_config/tests/docs_ir.rs`, ADR-005 subcommand
  delegation tests in `docs_ir_subcommands.rs`, seven trybuild compile-fail
  fixtures (`tests/ui/behaviour_*.rs` + `.stderr`), and an extended doctest in
  `ortho_config/src/docs/mod.rs`.
- Gates: full sequence green on the committed tree (`make check-fmt`,
  `make typecheck`, `make lint`, `make test`, `make markdownlint`,
  `make nixie`). Logs: `/tmp/lint-salvage-ortho-config-7-2-1.out`,
  `/tmp/test-salvage-ortho-config-7-2-1.out`,
  `/tmp/md-salvage-ortho-config-7-2-1.out`.
- CodeRabbit pass: `coderabbit review --agent` returned 0 findings across
  the full branch diff (Milestones B+C), reviewed through the
  `7-2-1-metadata-...` worktree (session `05f1f271`); pass clear. Log:
  `/tmp/scrut-mc-coderabbit.out`.
- Provenance note: the working tree was shared with a concurrent session; the
  Milestone C implementation and tests were verified, the clippy `unreachable`
  finding fixed, formatted, and committed as a single atomic commit to avoid
  losing the verified work.

### Milestone D transcript (2026-08-13)

The bridge now populates the agent-context behaviour fields from the IR
`behaviour` block; the fixture tree declares all four target states; and the
BDD scenario asserts them end to end through the real binary.

- Bridge (`cargo-orthohelp/src/agent_context/mod.rs`): `walk` maps
  `meta.behaviour` through `map_interaction`/`map_mutation` (total match arms
  covering both IR enums; absent stays `Unknown`) and copies `bypass` and
  `dry_run` verbatim. No inference, no defaults beyond `Unknown`.
- Fixture (`tests/fixtures/orthohelp_fixture/src/lib.rs`): `admin purge`
  declared `interactive`/`delete`/`--force`; `admin prune` declared
  `mutation = "delete"` (the milestone-E `destructive_bypass_missing` target);
  `greet` declared `non_interactive`/`read_only`; `version` and the remaining
  commands left unannotated to lock the `unknown` passthrough.
- Golden snapshot `agent_context__nested_fixture.json.snap` refreshed; the diff
  was reviewed field-by-field (purge/prune entries added; greet now
  non_interactive/read_only) before accepting.
- BDD: `orthohelp_agent_context.feature` gained the "agent context reports
  declared behaviour metadata" scenario with dedicated steps in
  `steps_agent_context.rs` (rstest-bdd placeholders capture single tokens, so
  multi-segment command paths use literal per-command step text backed by a
  shared `assert_command_string_field` helper).
- `nested_subcommand_end_to_end.rs` updated: the IR admin subcommand list now
  includes `purge`/`prune`, and the admin man page assertions cover `.SS purge`
  and `.SS prune`.
- hello_world: no change needed (hand-authored agent context; see Surprises).
- Gates: `make check-fmt`, `make typecheck`, `make lint`, `make test`,
  `make markdownlint` all green. CodeRabbit pass pending (recorded in
  Progress). Logs: `/tmp/scrut-md-*.out`.

## Interfaces and dependencies

At the end of the work these items exist:

- `ortho_config::docs::ir::BehaviourMetadata`, `InteractionKind`,
  `MutationKind` (public, serialized snake_case), and
  `DocMetadata::behaviour: Option<BehaviourMetadata>`;
  `ORTHO_DOCS_IR_VERSION == "1.2"`.
- `ortho_config::agent_context::AgentCommand::bypass_flag: Option<String>`
  and `::dry_run_flag: Option<String>`, placed directly after `mutation_effect`;
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION == "1"` unchanged.
- Derive support for `#[ortho_config(behaviour(...))]` with the keys
  `interaction`, `mutation`, `bypass`, and `dry_run` on structs deriving the
  docs metadata, flowing through `OrthoConfigSubcommandDocs` unchanged.
- `check_behaviour(&AgentContext, PolicyMode) -> PolicyReport` in
  `cargo_orthohelp::policy::rules::behaviour`, the CLI enum
  `cargo_orthohelp::cli::CheckMode`, and the flag
  `cargo orthohelp --check-agent-native[=off|warn|deny]` (exit 3 on deny
  findings; tracing on stderr).
- `docs/adr-008-behavioural-metadata-attribute-surface.md`, updated design
  doc, users' guide, developers' guide, and a ticked roadmap entry 7.2.1.

No new external dependencies.

## Revision note (2026-08-06)

Revised after the community-of-experts review (six Logisphere lenses across
three panels). What changed and why:

- Folded the former Milestone A into Milestone B and made red states
  explicitly transient: committing non-compiling tests contradicted the
  gates-green-at-every-commit constraint (blocker, two panels).
- Corrected the deny-mode severity mapping: `undeclared` findings now
  escalate to `deny` in deny mode, as §8.1 requires; the draft's "stays warn"
  contradicted the design doc (blocker).
- Resolved the Milestone D/E fixture contradiction by adding a
  declared-destructive-without-bypass command (`admin prune`) to the fixture
  tree in D, so E's scenarios have a real target (blocker).
- Added the tracing-to-stderr prerequisite: the fmt subscriber currently
  writes to stdout and would corrupt the stdout JSON report (blocker).
- Switched the lint exit code from 1 to 3 to avoid colliding with runtime
  errors (1) and clap usage errors (2); BDD asserts the exact code plus a
  well-formed report.
- Replaced direct reuse of the schema `PolicyMode` in clap with a CLI-layer
  `CheckMode` value enum plus `From` impl, keeping clap out of the ADR-003
  schema type; added `require_equals = true`.
- Changed `dry_run` from `Option<bool>` to a flag-name string
  (`dry_run_flag: Option<String>`), symmetrical with `bypass`, because §8.2
  makes a later bool-to-string migration breaking.
- Decided the interaction × bypass matrix: `non_interactive` + `bypass` is
  a compile error; `non_interactive` destructive commands are exempt from the
  destructive-bypass rule as declared "approved metadata".
- Added a fourth lint rule (`bypass_flag_unknown`) cross-checking the
  declared bypass against declared inputs; pinned the bypass/dry-run flag
  grammar; specified the finding message format (command path plus exact
  annotation snippet) and locked it with a snapshot.
- Defined `check_behaviour` as total (`Off` → empty report), specified
  lint-only runs skip default-format artefact generation, recorded that the
  emitted context's `policy.agent_native` stays at its default until 7.1.1, and
  stated the bridge-failure contract (no report emitted).
- Widened the IR-version work to replace hard-coded `"1.1"` literals with
  the constant; documented the version-skew contract for ADR-008; isolated the
  two snapshot-churn sources in separate commits; softened the
  independent-revert claim.
- Added trybuild fixtures for the unknown-nested-key, contradiction, and
  field-placement cases; field-level `behaviour`/`behavior` now errors instead
  of being silently discarded.
- Expanded Milestone F documentation duties (first-run `undeclared` noise
  warning in the users' guide; stream and exit-code contract in the developers'
  guide) and the ADR-008 contents list; recorded deferred items (per-code
  summary counts, verb/mutation contradiction advisory, source spans).

Effect on remaining work: milestone count drops to five (B–F); Milestone E
grows by the tracing prerequisite and the artefact-skip logic; everything else
is clarification rather than new scope.
