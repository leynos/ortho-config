# Add metadata for non-interactive execution and mutation boundaries (7.2.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

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
  meet. No new circular dependency between crates under any circumstances.
- Undeclared behaviour must remain undeclared. The derive and the bridge must
  never infer interaction or mutation semantics from command names, verbs, or
  flags (design doc §8.1: "Read/write/delete boundaries must not be inferred
  from names"). Absent metadata stays `unknown`.
- No new external dependency. The work uses `syn`/`quote` (already in
  `ortho_config_macros`), `serde` (already everywhere), and the existing dev
  dependencies (`rstest`, `rstest-bdd`, `insta`, `proptest`, `trybuild`,
  `googletest`, `pretty_assertions`).
- All commit gates (`make check-fmt`, `make typecheck`, `make lint`,
  `make test`) must pass at every commit. Note the standing repo caveat in
  memory: `make lint` may be red on `main` itself for files outside this diff;
  check that any lint failure cites files this plan touches before treating it
  as ours.
- British English (en-GB Oxford spelling) in all prose and identifiers exposed
  to users (`behaviour`, not `behavior`), matching the existing documentation
  corpus.

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
- Risk: bumping `ORTHO_DOCS_IR_VERSION` from `"1.1"` to `"1.2"` invalidates
  many golden snapshots at once, making the review diff noisy. Severity: low.
  Likelihood: high. Mitigation: isolate the version bump and its snapshot churn
  in a dedicated commit so behavioural changes remain reviewable.
- Risk: the lint's exit-code behaviour pre-empts the exit-code taxonomy work
  scheduled for 7.2.5. Severity: low. Likelihood: medium. Mitigation: use exit
  code 1 for deny-mode findings, document it as provisional in the developers'
  guide, and record the decision so 7.2.5 can supersede it.
- Risk: the attribute surface chosen here constrains later phase-7 items
  (7.2.2 dual renderer, 7.2.3 structured output) that will add more behaviour
  metadata. Severity: medium. Likelihood: medium. Mitigation: use one nested
  `behaviour(...)` attribute group so later items add keys rather than new
  top-level attributes; record the shape in ADR-008.
- Risk: `#[ortho_config(...)]` parsing silently discards unknown keys at
  struct and field level (`discard_unknown` in
  `ortho_config_macros/src/derive/parse/mod.rs`), so a typo such as
  `behavior(...)` would be swallowed rather than rejected. Severity: medium.
  Likelihood: high (it is a natural en-US typo). Mitigation: inside the
  recognized `behaviour(...)` group, unknown nested keys and invalid values are
  hard `syn::Error`s with spans; additionally, reject the exact key `behavior`
  at top level with a spelling hint. Covered by trybuild compile-fail tests.

## Progress

- [x] (2026-08-06 14:20Z) Reconnaissance of code, design docs, and prior art
  completed (three read-only survey passes plus web research on MCP tool
  annotations and clig.dev).
- [x] (2026-08-06 14:40Z) ExecPlan drafted.
- [ ] Community-of-experts design review of this plan; revisions applied.
- [ ] Plan approved by the user. Implementation must not begin before this.
- [ ] Milestone A: red tests for IR `behaviour` block and agent-context
  `bypass_flag`/`dry_run` fields.
- [ ] Milestone B: IR and agent-context schema types implemented; goldens
  updated; gates green; CodeRabbit clear.
- [ ] Milestone C: derive attribute surface (`behaviour(...)`) parsed,
  validated, emitted; trybuild fixtures; gates green; CodeRabbit clear.
- [ ] Milestone D: bridge population and fixture/example updates; BDD
  scenario green; gates green; CodeRabbit clear.
- [ ] Milestone E: `--check-agent-native` lint with policy report; BDD
  scenarios; gates green; CodeRabbit clear.
- [ ] Milestone F: documentation (design doc §8.1 rows, users' guide,
  developers' guide, ADR-008), roadmap ticked, final gates, final CodeRabbit
  pass.

## Surprises & discoveries

- Observation: agent-context schema v1 already reserves `interaction_mode` and
  `mutation_effect` as realized v1 fields defaulting to `"unknown"` (design doc
  §8.1 table), and the Rust enums already exist with the exact variants the
  roadmap asks for. Evidence: `ortho_config/src/agent_context/mod.rs` defines
  `InteractionMode { Unknown, NonInteractive, Interactive }` and
  `MutationEffect { Unknown, ReadOnly, Write, Delete, Submit }`; the bridge
  hardcodes both to default in `cargo-orthohelp/src/agent_context/mod.rs`
  (`walk`). Impact: 7.2.1 is a wiring task plus one new optional field, not a
  schema redesign. No agent-context version bump is needed.
- Observation: the design doc's §3.3 policy-report example flattens `file` and
  `range` onto each result, but the implemented `PolicyResult` nests them under
  `location`. Evidence: `cargo-orthohelp/src/policy/mod.rs` versus design doc
  lines 248–288. Impact: milestone E follows the implemented Rust types (the
  schema owner per ADR-003) and milestone F corrects the design-doc example to
  match.

## Decision log

- Decision: populate the existing schema v1 fields rather than introduce new
  ones for interaction and mutation; add only `bypass_flag` and `dry_run` as
  new optional `AgentCommand` fields. Rationale: §8.1 reserved the fields for
  exactly this task; §8.2 classifies populating them and adding optional fields
  as additive within version "1". Date/Author: 2026-08-06, planning session.
- Decision: authors declare behaviour with a single nested struct-level
  attribute group `#[ortho_config(behaviour(...))]` on the command's arguments
  struct, not on subcommand enum variants. Rationale: struct-level attributes
  already flow through the `StructAttrs`/`DocStructAttrs` parse path and reach
  subcommand metadata via the ADR-005 companion-trait delegation
  (`metadata_expr` calls the inner struct's `get_doc_metadata`). Variant-level
  attributes have no parse path today and would duplicate state between the
  variant and its argument struct. A nested group leaves room for 7.2.2+ keys.
  Recorded as ADR-008. Date/Author: 2026-08-06, planning session.
- Decision: represent "needs a bypass flag" as `interaction = "interactive"`
  plus a declared `bypass` flag, rather than adding a third `InteractionMode`
  variant. Rationale: §6.1 treats the bypass flag as a property ("which flag
  bypasses prompting"), not a distinct mode; adding an enum variant to a v1
  wire enum needs an unknown-variant fallback contract and buys nothing the
  pair does not already express. The lint distinguishes the cases without a new
  variant. This mirrors the MCP annotation style of orthogonal hints.
  Date/Author: 2026-08-06, planning session.
- Decision: bump `ORTHO_DOCS_IR_VERSION` from `"1.1"` to `"1.2"` for the new
  optional `behaviour` block, and record the IR compatibility reasoning in
  ADR-008. Rationale: prior execplans treat IR schema additions as requiring an
  IR version bump plus an ADR; the design doc has no explicit additive-change
  policy for the IR (unlike §8.2 for agent context), so the conservative
  precedent stands. Date/Author: 2026-08-06, planning session.
- Decision: the lint ships three rules under a new `behaviour` category:
  `agent-native.behaviour.destructive-bypass` (code
  `destructive_bypass_missing`), `agent-native.behaviour.prompt-bypass` (code
  `prompt_bypass_missing`), and `agent-native.behaviour.undeclared` (codes
  `interaction_unknown`, `mutation_unknown`). Rationale: the roadmap names the
  destructive check; §6.1 names the prompt-without-bypass check; §8.1 requires
  omitted metadata that blocks an agent-native guarantee to warn. Rule
  identifiers follow the existing fixture convention
  `agent-native.<category>.<check>`. Date/Author: 2026-08-06, planning session.
- Decision: `--check-agent-native` takes an optional mode value
  (`warn` default; `deny` escalates findings to a non-zero exit; `off`
  short-circuits), because the 7.1.1 policy configuration file does not exist
  yet. Rationale: 7.2.1 requires only step 6.2; blocking on 7.1.1 would invert
  the roadmap order. When 7.1.1 lands, the flag becomes an override of the
  configured mode. Date/Author: 2026-08-06, planning session.
- Decision: include `dry_run` declaration in scope as passive metadata (no
  lint on it yet). Rationale: §6.4 groups dry-run declaration with mutation
  boundaries and no later roadmap item covers it; adding the field now avoids a
  second IR bump. Date/Author: 2026-08-06, planning session.
- Decision: use `proptest` for serde round-trip invariants of the new types
  and skip `kani`/`verus`. Rationale: the only invariants over an input range
  are serialization round-trips and the total lint classification function;
  both are shallow data-shape properties with no unsafe code, no state machine,
  and no arithmetic — bounded model checking or deductive proof would restate
  the property without adding assurance. Date/Author: 2026-08-06, planning
  session.

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
  the inner struct's `get_doc_metadata`, so struct-level metadata reaches
  subcommands automatically. Errors are `syn::Error` with spans, surfaced as
  compile errors; compile-fail coverage uses `trybuild` fixtures in
  `ortho_config/tests/ui/*.rs` with `*.stderr` goldens.
- `cargo-orthohelp/` — the `cargo orthohelp` tool. It keeps a hand-maintained
  mirror of the IR in `cargo-orthohelp/src/schema/mod.rs` ("Keep this in sync
  with `ortho_config::docs`"). `cargo-orthohelp/src/agent_context/mod.rs`
  contains `bridge_ir_to_agent_context`, whose internal `walk` currently sets
  `interaction_mode: InteractionMode::default()` and
  `mutation_effect: MutationEffect::default()` unconditionally — the wiring gap
  this plan closes. `cargo-orthohelp/src/policy/mod.rs` defines the
  policy-report schema (`PolicyReport`, `PolicyResult`,
  `PolicyMode { Off, Warn, Deny }`, `ORTHO_POLICY_REPORT_SCHEMA_VERSION = "1"`)
  but no rule or runner exists yet. The CLI (`cargo-orthohelp/src/cli/mod.rs`)
  has `--format <ir|man|ps|all|agent-context>`; dispatch lives in
  `cargo-orthohelp/src/main.rs` (`run`, `generate_agent_context_if_requested`).
- `tests/fixtures/orthohelp_fixture/` — a fixture crate compiled by
  cargo-orthohelp's ephemeral bridge during tests (`SimpleFixtureConfig`,
  `FixtureConfig`, `NestedFixtureConfig` with a three-level subcommand tree
  including `admin` → `audit`/`grant-access`).
- `examples/hello_world/` — a downstream-style example with its own
  agent-context BDD feature and insta snapshot.

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
(policy modes and report fields), §8.1 (defaults for legacy derives), §8.2
(schema v1 compatibility policy).

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

The work proceeds in six milestones, A–F. Every milestone ends with the full
gate sequence run by the `scrutineer` subagent (`make check-fmt`,
`make typecheck`, `make lint`, `make test`), a commit, and — for B through F — a
`coderabbit review --agent` pass whose concerns are cleared before the next
milestone. Tests are written red-first within each milestone.

### Milestone A — red tests for the new data shapes

Add failing unit tests describing the target schema before touching production
types.

In `ortho_config/src/docs/` tests (alongside existing IR tests in
`ortho_config/tests/docs_ir.rs`): a test asserting that `DocMetadata`
deserializes a JSON document containing a `behaviour` object with `interaction`,
`mutation`, `bypass`, and `dry_run` keys, and that a document without the key
deserializes with `behaviour: None`.

In `ortho_config/src/agent_context/tests_json.rs` (and the wire-contract
snapshot test in `tests.rs`): assertions that `AgentCommand` serializes
`bypass_flag` and `dry_run` as explicit `null` when absent, matching the
existing optional-field convention (only `summary` is omitted when absent), and
round-trips declared values.

These tests fail to compile until milestone B adds the types; per the execplans
convention, the red state is demonstrated by running the focused tests and
observing the expected compile/assert failure before implementing.

### Milestone B — IR and agent-context schema types

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
       /// Whether the command offers `--dry-run`.
       #[serde(default)]
       pub dry_run: Option<bool>,
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

3. Mirror both changes in `cargo-orthohelp/src/schema/mod.rs`.

4. `ortho_config/src/agent_context/mod.rs`: add to `AgentCommand`
   `#[serde(default)] pub bypass_flag: Option<String>` and
   `#[serde(default)] pub dry_run: Option<bool>` (explicit null when absent).

5. Update the wire-contract snapshot, the three golden agent-context
   snapshots under `cargo-orthohelp/tests/golden/`, the hello_world snapshot,
   and any IR snapshots that embed `ir_version`. Keep the version-bump snapshot
   churn in its own commit.

6. Add proptest round-trip properties (in the existing proptest homes:
   `ortho_config` dev-deps and
   `cargo-orthohelp/src/agent_context/proptests.rs`) asserting
   serialize→deserialize identity for `BehaviourMetadata` and for
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
   `bypass` (string, must begin with `--` and be a plausible long flag),
   `dry_run` (bool). Unknown nested keys and invalid values are `syn::Error`s
   with the offending span. At the top level of `parse_struct_attrs`/
   `apply_struct_doc_attr`, explicitly reject the key `behavior` with the
   message "unknown attribute `behavior`; use the en-GB spelling `behaviour`"
   so the silent `discard_unknown` path cannot swallow the likely typo.
3. `ortho_config_macros/src/derive/generate/docs/sections.rs` (or a small
   sibling): `build_behaviour_metadata` emitting the
   `Option<BehaviourMetadata>` token stream; wire it into the `quote!` block of
   `generate_docs_impl` in `generate/docs/mod.rs`.
4. Tests: rstest unit tests for the parser under
   `ortho_config_macros/src/derive/parse/tests/`; IR-shape tests in
   `ortho_config/tests/docs_ir.rs` and
   `ortho_config/tests/docs_ir_subcommands.rs` proving behaviour metadata flows
   through the ADR-005 subcommand delegation unchanged; trybuild compile-fail
   fixtures `ortho_config/tests/ui/behaviour_invalid_interaction.rs`,
   `behaviour_invalid_mutation.rs`, `behaviour_bad_bypass.rs`, and
   `behaviour_en_us_spelling.rs` with `.stderr` goldens.
5. Extend the doctest example in `ortho_config/src/docs/mod.rs` minimally, or
   add a new one, following `docs/rust-doctest-dry-guide.md`.

### Milestone D — bridge population and fixtures

1. `cargo-orthohelp/src/agent_context/mod.rs`: in `walk`/`build_input`, map
   the IR `behaviour` block: `Some(NonInteractive)` →
   `InteractionMode::NonInteractive`, `Some(Interactive)` →
   `InteractionMode::Interactive`, `None` → `Unknown`; likewise for
   `MutationKind` → `MutationEffect`; copy `bypass` → `bypass_flag` and
   `dry_run` → `dry_run`. No inference, no defaults beyond `Unknown`.

2. Annotate the fixture crate: give `NestedAdminSubcommand`'s destructive
   variant's argument struct (or add a `purge`-style struct if none is
   naturally destructive)
   `behaviour(interaction = "interactive", mutation = "delete", bypass = "--force")`,
   one read-only command
   `behaviour( interaction = "non_interactive", mutation = "read_only")`, and
   leave at least one command unannotated to lock the `unknown` passthrough.

3. Update golden snapshots and the hello_world example (annotate one command
   there and refresh its snapshot).

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

### Milestone E — the `--check-agent-native` lint

1. `cargo-orthohelp/src/cli/mod.rs`: add a new optional flag to `Args`
   (reusing `PolicyMode` from the policy module as the value enum):

   ```rust
   #[arg(long = "check-agent-native", value_enum, num_args = 0..=1,
         default_missing_value = "warn")]
   pub check_agent_native: Option<PolicyMode>,
   ```

2. New module `cargo-orthohelp/src/policy/rules/behaviour.rs`: a pure
   function
   `fn check_behaviour(context: &AgentContext, mode: PolicyMode) -> PolicyReport`
   implementing the three rules from the Decision Log. Each `PolicyResult`
   carries `rule_id`, `code`, `severity`, `message` naming the command path and
   the canonical remedy (for the destructive rule, the message recommends
   `--force` per §6.1), and `location: None` (source spans are unavailable from
   agent context; noted as a documented limitation until the policy work of 7.1
   threads spans through). Severity mapping: in `warn` mode every finding is
   `warn`; in `deny` mode `destructive-bypass` and `prompt-bypass` findings are
   `deny` while `undeclared` findings stay `warn`; in `off` mode the check does
   not run.

3. `cargo-orthohelp/src/main.rs`: after the agent context is built (build it
   on demand if the run did not already), run the check when the flag is
   present; serialize the `PolicyReport` as JSON to stdout; write a
   human-readable summary to stderr; exit non-zero (code 1) only if the report
   contains at least one `deny`-severity result.

4. Tests: rstest unit tests for `check_behaviour` covering happy paths (fully
   declared tree yields an empty report), each rule firing, mode mapping, and
   the edge cases (declared `bypass` on a non-destructive command produces no
   finding; `submit` and `write` commands do not trigger the destructive rule;
   empty command list). An insta snapshot locks the JSON policy report for the
   fixture tree in warn mode (multivariant output consistency). A new BDD
   feature `cargo-orthohelp/tests/features/orthohelp_policy.feature`:

   ```gherkin
   Scenario: destructive command without a bypass flag fails deny mode
     Given a fixture command tree with an undeclared destructive command
     When I run cargo orthohelp with check-agent-native in "deny" mode
     Then the policy report contains code "destructive_bypass_missing"
     And the process exit code is 1

   Scenario: warn mode reports findings without failing
     Given a fixture command tree with an undeclared destructive command
     When I run cargo orthohelp with check-agent-native in "warn" mode
     Then the policy report contains code "destructive_bypass_missing"
     And the process exit code is 0
   ```

   These end-to-end scenarios exercise the real binary path because the lint is
   an externally observable CLI contract.

5. Proptest: a property over arbitrary `AgentCommand` vectors asserting the
   classification function is total and that a report never contains a `deny`
   severity when the mode is `warn` or `off`.

### Milestone F — documentation and closure

1. `docs/agent-native-cli-design.md`: add `bypass_flag` and `dry_run` rows
   (status v1, default `null`) to the §8.1 table; correct the §3.3 example to
   the implemented `location` nesting; note in §6.1/§6.4 that the metadata is
   now realized.
2. New `docs/adr-008-behavioural-metadata-attribute-surface.md` (per
   `docs/documentation-style-guide.md`): records the attribute shape, the
   IR-version bump policy applied, the no-inference rule, and the
   interactive-plus-bypass representation; referenced from `docs/design.md`'s
   decision log.
3. `docs/users-guide.md`: extend the "Documentation and agent contracts"
   section and the `OrthoConfigDocs` worked examples with `behaviour(...)`;
   document the lint flag and the report shape.
4. `docs/developers-guide.md`: update schema-ownership and
   agent-context-surface sections with the new fields, the rule-id convention,
   and the provisional exit-code decision.
5. `docs/roadmap.md`: mark 7.2.1 and its three sub-bullets done.
6. Final full gate run via `scrutineer`, final `coderabbit review --agent`,
   final commit.

## Concrete steps

All commands run from the repository root. Long outputs are captured with `tee`
to `/tmp/$ACTION-$(get-project)-$(git branch --show-current).out` per
repository convention; gate runs are delegated to the `scrutineer` subagent,
which does this automatically.

Branch setup (already applicable at plan time):

```bash
git branch -m 7-2-1-metadata-for-non-interactive-execution-and-mutation-boundaries
git push -u origin 7-2-1-metadata-for-non-interactive-execution-and-mutation-boundaries
```

Red evidence, milestone A (expected to fail before milestone B):

```bash
cargo test -p ortho_config docs_ir -- behaviour 2>&1 | tee /tmp/red-a.out
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
   and running the bridge yields
   agent-context JSON in which that command reports
   `"interaction_mode": "interactive"`, `"mutation_effect": "delete"`,
   `"bypass_flag": "--force"`, while an unannotated command still reports
   `"unknown"` for both enums and `null` for the new fields. Proven by the
   golden snapshots and the `orthohelp_agent_context.feature` scenario above.
2. `cargo orthohelp --check-agent-native` (warn) on the fixture tree prints a
   JSON `PolicyReport` (schema version "1") to stdout listing
   `destructive_bypass_missing` for an undeclared destructive command and exits
   0; the same invocation in `deny` mode exits 1. Proven by the
   `orthohelp_policy.feature` scenarios.
3. Misdeclarations fail to compile: `behaviour(interaction = "sometimes")`,
   `behaviour(mutation = "destroy")`, `behaviour(bypass = "force")`, and
   `behavior(...)` each produce the trybuild-goldened compile error.
4. `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` still equals `"1"`;
   `ORTHO_DOCS_IR_VERSION` equals `"1.2"`; asserted by existing and new unit
   tests.
5. Red-Green-Refactor evidence is recorded per milestone in `Progress` and
   `Artefacts and notes`: the red command and its expected failure, the green
   run, and the post-refactor gate run.

Quality criteria: all four make gates pass; CodeRabbit concerns cleared at each
milestone; no new dependencies; snapshot diffs reviewed rather than blindly
accepted.

## Idempotence and recovery

Every milestone is an ordinary commit on the task branch; recovery is
`git revert` or resetting to the previous milestone commit. Snapshot
regeneration (`cargo insta`) is idempotent. The IR version bump commit is
isolated so it can be reverted independently. No step touches state outside the
repository except `/tmp` logs.

## Artefacts and notes

Prior-art evidence gathered during planning:

- MCP tool annotations (spec 2025-06-18): `readOnlyHint`, `destructiveHint`,
  `idempotentHint`, `openWorldHint`; "Clients MUST consider tool annotations to
  be untrusted unless they come from trusted servers." The declared-hint (not
  proven) stance matches this plan's no-inference constraint: OrthoConfig
  transports declarations, it does not verify runtime behaviour.
- clig.dev: "Never require a prompt… If `--no-input` is passed, don't prompt
  or do anything interactive"; "-f, --force… doing something destructive that
  usually requires user confirmation"; `-n, --dry-run` as the standard dry-run
  flag. These are the canonical flag names §6.1 already adopted.

Implementation transcripts will be appended here per milestone.

## Interfaces and dependencies

At the end of the work these items exist:

- `ortho_config::docs::ir::BehaviourMetadata`, `InteractionKind`,
  `MutationKind` (public, serialized snake_case), and
  `DocMetadata::behaviour: Option<BehaviourMetadata>`;
  `ORTHO_DOCS_IR_VERSION == "1.2"`.
- `ortho_config::agent_context::AgentCommand::bypass_flag: Option<String>`
  and `::dry_run: Option<bool>`; `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION == "1"`
  unchanged.
- Derive support for `#[ortho_config(behaviour(...))]` with the keys
  `interaction`, `mutation`, `bypass`, and `dry_run` on structs deriving the
  docs metadata, flowing through `OrthoConfigSubcommandDocs` unchanged.
- `check_behaviour(&AgentContext, PolicyMode) -> PolicyReport` in
  `cargo_orthohelp::policy::rules::behaviour`, and the CLI flag
  `cargo orthohelp --check-agent-native[=off|warn|deny]`.
- `docs/adr-008-behavioural-metadata-attribute-surface.md`, updated design
  doc, users' guide, developers' guide, and a ticked roadmap entry 7.2.1.

No new external dependencies.
