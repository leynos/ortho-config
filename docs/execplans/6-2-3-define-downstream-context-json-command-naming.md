# Define downstream `context --json` command naming (6.2.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

OrthoConfig already generates a compact, machine-readable agent-context
document (roadmap 6.2.1). The reusable types live in
`ortho_config::agent_context`, and the `cargo-orthohelp` generator emits them
through `cargo orthohelp --format agent-context`, which writes
`<out_dir>/agent-context.json`.

What is not yet settled is how a *downstream application* (for example Weaver
or Netsuke) should expose that same contract on its own command surface, and
how the payload should identify itself. Roadmap item 6.2.3 fixes that
convention:

- downstream application command surfaces expose `<tool> context --json`;
- the JSON payload self-identifies through `kind: "<tool>.agent_context"`;
- the generator keeps `cargo orthohelp --format agent-context`; and
- no public `agent-context` command or alias is shipped before the first public
  release unless a migration explicitly requires one.

After this change, a developer can run the worked example and observe the
convention working end to end:

```console
$ cargo run -p hello_world -- context --json
{"schema_version":"1","kind":"hello_world.agent_context","package":"hello_world",...}
```

The output parses as JSON, carries `kind` equal to `hello_world.agent_context`,
and reports `schema_version` equal to `1`. The convention is also codified as
reusable crate API (naming constants, a single `kind` constructor, and a JSON
render helper), recorded in a new ADR, and protected by a guard test that fails
if a public `agent-context` or `context` alias ever leaks into the
`cargo-orthohelp` command tree.

Success is observable in four ways: the example binary emits the payload; the
crate exposes the convention as documented, tested API; the docs describe the
convention normatively with its prior-art rationale; and the guard test proves
the reserved-name rule holds.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

- `ortho_config` must not gain a dependency on `cargo-orthohelp`. The reusable
  convention flows one way: the library defines it; the generator and
  downstream applications consume it. No circular crate dependency may be
  introduced (see `docs/developers-guide.md` schema-ownership notes and
  `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`).
- The agent-context wire contract is owned by
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` (currently `"1"`). This plan must not
  change the schema version, rename fields, alter serde renames, or change the
  serialized shape of any existing `ortho_config::agent_context` type. The
  existing snapshot/wire test in `ortho_config/src/agent_context/tests.rs` must
  continue to pass unchanged.
- The JSON render helper that calls `serde_json` must be gated behind the
  existing optional `serde_json` cargo feature. `--no-default-features` builds
  must continue to compile. (`serde_json` is optional and default-on in
  `ortho_config/Cargo.toml`; the `agent_context` module today references
  `serde_json` only under `#[cfg(test)]`.)
- `cargo-orthohelp` must keep `--format agent-context` working and must not gain
  a public `context` or `agent-context` subcommand or alias. The generator
  surface is a build-tool format, not an application command.
- The `kind` discriminator format (`<tool>.agent_context`) must be governed by
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` and `AGENT_CONTEXT_KIND_SUFFIX`.
  Consumers must be told to detect compatibility via `schema_version`, never by
  parsing `kind`. The ADR must state this explicitly.
- All changes must keep `make check-fmt`, `make typecheck`, `make lint`, and
  `make test` green at each milestone boundary.
- Documentation prose follows en-GB Oxford spelling and the
  `docs/documentation-style-guide.md` conventions (no first/second person
  outside `README.md`, sentence-case headings, 80-column prose wrap, 120-column
  code, fenced blocks carry a language tag, tables carry captions). Machine
  identifiers inside code and JSON (for example `color`, `kind`,
  `schema_version`, enum string values) keep their as-is spelling.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than roughly 12 source files
  or more than about 400 net lines of code (excluding generated snapshots and
  docs), stop and escalate.
- Interface: any change to an existing public signature, field, serde rename, or
  schema version is out of scope; stop and escalate.
- Dependencies: adding any new runtime dependency is out of scope; stop and
  escalate. Adding a *dev*-dependency is permitted only for `proptest` on
  `ortho_config` (see Risks) and only if the property tests are adopted; if any
  other new dev-dependency seems required, stop and escalate.
- Boundary: if making the `hello_world` example emit a *faithful auto-generated*
  command tree appears to require a runtime metadata-to-`AgentContext` path in
  `ortho_config` or the example, stop and escalate — that is 6.2.2 /
  skill-manifest scope, not this item.
- Iterations: if a milestone's tests still fail after 3 focused attempts, stop
  and escalate with the failing transcript.
- Ambiguity: the bare-`context` behaviour decision (Decision D2) is settled in
  this plan; if review rejects it, stop and re-confirm before writing the
  affected scenarios.

## Risks

- Risk: an embedded or auto-generated example context drifts from the example's
  real command set, so the demo silently lies about `hello_world`. Severity:
  medium. Likelihood: medium. Mitigation: scope the example's context as a
  *hand-authored, illustrative* demonstration of the naming/payload convention,
  owned by the example and its tests — not a faithful generated tree. Document
  this explicitly in code and in the users' guide. (Raised by Doggylump and
  Pandalump in design review.)
- Risk: agents key off the `kind` string instead of `schema_version`, turning an
  unversioned string into a de facto contract. Severity: medium. Likelihood:
  medium. Mitigation: ADR-007 states that `kind` shape is governed by the
  schema version and the suffix constant, and that compatibility detection uses
  `schema_version`. The users' guide repeats this. (Raised by Telefono.)
- Risk: the `context` command, if modelled as a normal merged subcommand in the
  example, is forced through `load_globals_and_merge_selected_subcommand` and
  inherits config-merge semantics it does not want. Severity: low. Likelihood:
  medium. Mitigation: handle `context` as an early short-circuit before the
  globals/subcommand merge (mirroring `is_display_request`), so it is a pure
  introspection command. See Decision D3.
- Risk: adding `proptest` as an `ortho_config` dev-dependency is rejected.
  Severity: low. Likelihood: low. Mitigation: fold the single property into a
  parameterized `#[rstest]` table in `tests.rs`; no new dependency. See
  Decision D5.
- Risk: SemVer / public-API baseline snapshots (if the repo keeps any) flag the
  new symbols. Severity: low. Likelihood: low. Mitigation: the additions are
  purely additive (minor bump). Regenerate any `cargo-public-api` baseline as a
  checklist step; run `cargo-semver-checks` across the `--no-default-features`
  and `--features serde_json` matrix.

## Progress

- [x] (2026-06-24 12:29Z) Milestone 0 — approval gate satisfied by the user's
  explicit implementation request on PR #352.
- [x] (2026-06-24 12:29Z) Repository administration aligned before code work:
  local branch tracks
  `origin/6-2-3-define-downstream-context-json-command-naming`; PR title is
  `Downstream context --json command naming (6.2.3)`; Lody session title
  matches the PR title; PR description references
  `https://lody.ai/leynos/sessions/3d5cd4de-ad94-49af-8867-b403bd1bbf77`.
- [x] (2026-06-24 12:45Z) Milestone 1 — convention API in
  `ortho_config::agent_context` implemented: `agent_context_kind`, the
  feature-gated `agent_context::json` serializer adapter, corresponding
  crate-root re-exports, rstest coverage, and one proptest round-trip property.
  Application-owned command and flag literals remain outside the reusable API.
  Red-Green-Refactor evidence is recorded under Validation.
- [x] (2026-06-24 12:48Z) Milestone 1 CodeRabbit review completed with zero
  findings after commit `bb721ab`. Standard gates passed; the extra
  `--no-default-features` check fails on pre-existing discovery/file feature
  imports and is recorded below.
- [x] (2026-06-24 13:03Z) Milestone 2 — guard test in `cargo-orthohelp`
  proving no public `context` / `agent-context` alias, plus positive control for
  `--format agent-context`. Mutation check with a temporary
  `alias = "context"` failed for the expected reason, then the alias was
  reverted and the guard passed.
- [x] (2026-06-24 14:28Z) Milestone 2 CodeRabbit review completed with zero
  findings after commit `1c581a3`. The first attempt hit CodeRabbit rate
  limiting; after a 77-minute `vsleep`, the retry completed cleanly.
- [x] (2026-06-24 14:31Z) Milestone 3 — wired an illustrative
  `context --json` command into the `hello_world` example (early
  short-circuit), with BDD, insta snapshot, `assert_cmd` e2e tests, localized
  help copy, and updated help snapshots.
- [x] (2026-06-24 14:35Z) Milestone 3 CodeRabbit review completed with zero
  findings after commit `15b4d39`.
- [x] (2026-06-24 14:55Z) Milestone 4 — documentation: ADR-007, normative
  promotion of `agent-native-cli-design.md` §3.2, users' guide subsection,
  developers' guide note, design-doc decision-log entry,
  `cargo-orthohelp-design.md` clarification, `contents.md` registration.
- [x] (2026-06-24 16:01Z) Milestone 4 CodeRabbit review completed with zero
  findings after commit `393b54c`. The first attempt hit CodeRabbit rate
  limiting; after a 58-minute `vsleep`, the retry completed cleanly.
- [x] (2026-06-24 16:16Z) Milestone 5 — marked roadmap 6.2.3 done and ran
  the final full deterministic gate set.
- [x] (2026-06-24 18:45Z) Final CodeRabbit review completed with zero findings
  after commit `173507f`. Two final-review attempts hit CodeRabbit rate
  limiting; after 75-minute and 88-minute `vsleep` waits, the retry completed
  cleanly.

Use timestamps (for example `(2026-06-14 13:00Z)`) when ticking items.

## Surprises & discoveries

- Observation: the `kind` payload requirement of 6.2.3 is already implemented.
  Evidence: `ortho_config/src/agent_context/mod.rs:55` — `AgentContext::new`
  sets `kind = format!("{package_name}.{AGENT_CONTEXT_KIND_SUFFIX}")`, producing
  `<package>.agent_context`. Impact: 6.2.3 is primarily a naming-convention,
  reusable-machinery, documentation, and guard-test task, not a
  re-implementation. The `kind` constructor is *extracted* into a single
  testable function rather than newly invented.
- Observation: no public `agent-context` subcommand or alias exists today; the
  only surface is the `--format agent-context` enum value. Evidence:
  `cargo-orthohelp/src/cli/mod.rs:16-28` (the `OutputFormat` enum) and
  `cargo-orthohelp/src/cli/mod.rs:42-47` (`CargoSubcommand` has only `Orthohelp`).
  Impact: the "avoid public aliases" requirement is satisfied today; the task
  is to *lock it in* with a guard test and document the rule.
- Observation: prominent prior art names the introspection *command*
  `agent-context`, not `context`. Evidence: Trevin Chow, "10 Principles for
  Agent-Native CLIs" (§7, three-layer introspection) shows
  `mycli agent-context | jq ...`; Cloudflare's Wrangler standardizes `--json`
  (never `--format=json`). Impact: OrthoConfig deliberately diverges on the
  command name (`context`) while keeping `--json` canonical. This divergence
  and its justification are the substance of ADR-007.
- Observation: the example's subcommands flow through
  `load_globals_and_merge_selected_subcommand`. Evidence:
  `examples/hello_world/src/main.rs:24-39`. Impact: `context` should
  short-circuit before that merge (Decision D3).
- Observation: `assert_cmd` is already a `hello_world` dev-dependency, and
  `proptest` is already an `ortho_config` dev-dependency. Evidence:
  `examples/hello_world/Cargo.toml` and `ortho_config/Cargo.toml`
  dev-dependencies. Impact: Milestone 1 used the property-test path without
  adding or changing dependencies; Decision D5's fallback is unnecessary.
- Observation: the active worktree path differs from the original planning
  worktree path. Evidence: current working directory is
  `/home/leynos/.lody/repos/github---leynos---ortho-config/worktrees/3d5cd4de-ad94-49af-8867-b403bd1bbf77`;
  the initial plan named
  `/home/leynos/.lody/repos/github---leynos---ortho-config/worktrees/da2a7f44-0569-4a05-87f2-08429d969afb`.
  Impact: the concrete command section now names the active worktree so future
  agents can resume without following stale paths.
- Observation: `cargo check -p ortho_config --no-default-features` does not
  compile before any Milestone 1-specific code path is involved. Evidence:
  `/tmp/check-no-default-ortho-config-6-2-3-define-downstream-context-json-command-naming.out`
  reports unresolved imports for `MergeLayer`, `figment::providers::Toml`,
  `serde_json`, and `toml` from `ortho_config/src/discovery/*` and
  `ortho_config/src/file/*`. Impact: the new render helpers are correctly
  feature-gated, but the plan's no-default acceptance check is currently
  blocked by an existing broader feature-boundary issue outside the
  agent-context API change.
- Observation: the planned focused command
  `cargo test -p cargo-orthohelp --lib cli` does not exercise
  `cargo-orthohelp/src/cli/mod.rs`; it runs unrelated library tests whose names
  contain `cli`. Evidence: the command reported two
  `agent_context::tests::*visible_cli*` tests and did not run
  `cli::tests::no_context_or_agent_context_subcommand_alias`. Impact: use the
  exact filters
  `cargo test -p cargo-orthohelp no_context_or_agent_context_subcommand_alias`
  and `cargo test -p cargo-orthohelp format_accepts_agent_context` for
  Milestone 2 evidence.
- Observation: exposing the hello_world `context` subcommand necessarily
  changes localized help and missing-subcommand diagnostics. Evidence: the
  first full `make test` after wiring Milestone 3 failed only the
  `localised_help` insta snapshots, showing the new `context` row and the
  extended valid-subcommands list. Impact: added explicit en-US and Japanese
  Fluent entries for `hello_world-cli-context-about`, then regenerated the
  affected snapshots with `INSTA_UPDATE=always`.
- Observation: the dormant
  `examples/hello_world/tests/rstest_bdd/behaviour/...` module tree is not the
  right insertion point for Milestone 3 right now. Evidence: exposing it as a
  new test target surfaced pre-existing fixture/trait wiring incompatibilities
  unrelated to the new command. Impact: Milestone 3 uses a focused
  `examples/hello_world/tests/agent_context_bdd.rs` target wired directly to
  `tests/features/agent_context.feature`; this keeps the BDD coverage active
  without widening the task into a harness repair.
- Observation: the project-wide `make fmt` target can rewrite many historical
  Markdown files before failing on unrelated line-length issues. Evidence: the
  first Milestone 3 documentation update run failed on
  `docs/execplans/6-1-2-nested-command-tree-behavioural-fixtures.md` and left
  broad unrelated Markdown edits. Impact: unrelated formatter edits were
  restored, and Markdown validation for this work relies on `make markdownlint`
  plus targeted review of touched files unless the full formatter can complete
  cleanly.

## Decision log

- Decision D1: Implement the recommended scope — reusable convention API in
  `ortho_config::agent_context` + guard test + illustrative example wiring +
  ADR/docs. Rationale: the user's detailed testing requirements (rstest,
  rstest-bdd, insta, e2e, proptest) and roadmap 5.2.3 ("OrthoConfig owns
  reusable command-contract machinery") point past a docs-only change; the
  example wiring supplies the observable end-to-end behaviour the ExecPlan
  skill requires. The lighter alternatives (API + doctests only; or
  docs/convention only) remain available if the approver wants to dial scope
  down. Date/Author: 2026-06-14, planning agent + design-review panel.

- Decision D2: Bare `context` (without `--json`) prints a short human-readable
  pointer to `--json` on stdout and exits 0; `--json` switches to the machine
  payload. Rationale: a command that always errors is hostile; a human-default
  plus a machine flag matches the dual-renderer intent in
  `agent-native-cli-design.md` §6.2. (Telefono flagged the fork; the test-plan
  agent had assumed bare `context` fails — this plan overrides that
  assumption.) Date/Author: 2026-06-14, design-review panel.

- Decision D3: In the example, handle `context` as an early short-circuit before
  `load_globals_and_merge_selected_subcommand`, not as a config-merged
  subcommand. Rationale: `context` is pure introspection; it must not inherit
  config-merge semantics. Mirrors the existing `is_display_request`
  short-circuit in `examples/hello_world/src/main.rs`. Date/Author: 2026-06-14,
  planning synthesis.

- Decision D4: Record the decision as ADR-007 (next free number), referenced
  from `design.md` and `agent-native-cli-design.md`, rather than recording it
  in the design doc only. Rationale: a narrow, hard-to-reverse naming/contract
  decision with explicit prior-art divergence is exactly the ADR threshold in
  `docs/documentation-style-guide.md`; it mirrors ADR-003 owning the schema
  split. Date/Author: 2026-06-14, docs-planning agent.

- Decision D5: Add `proptest` as an `ortho_config` dev-dependency for one
  round-trip property; if rejected at review, fall back to a parameterized
  `#[rstest]` table. Rationale: the render helper's "always produces parseable
  JSON across the full enum/Option matrix" property is high-value and
  unbounded; a property test is the right tool. The fallback keeps the work
  unblocked. Date/Author: 2026-06-14, test-planning agent.

- Decision D6: The `--json` flag constant stores the clap *long name* `"json"`
  (not the display form `"--json"`). Rationale: clap's `Arg::long` /
  `#[arg(long = ...)]` prepend `--`; storing `"--json"` would produce
  `----json` or force every caller to strip the prefix. It also matches the
  existing `AgentInput.long` convention (long names without punctuation). The
  display form is `format!("--{flag}")` when needed. Date/Author: 2026-06-14,
  API-design agent.

- Decision D7: Do not widen Milestone 1 to repair the existing
  `--no-default-features` discovery/file feature-boundary failure. Rationale:
  the failure is outside the new `agent_context` API surface, standard
  workspace gates pass, and repairing no-default mode would require a separate
  feature-boundary design decision across file loading, discovery, TOML, and
  declarative merge APIs. Date/Author: 2026-06-24, implementation agent.

## Outcomes & retrospective

Milestone 1 outcome (2026-06-24): the crate API portion is implemented and
validated. `ortho_config::agent_context` now exposes the downstream command
name, JSON flag name, and canonical `kind` constructor; `AgentContext::new`
uses the constructor; and compact/pretty JSON render helpers live in
`ortho_config::agent_context::json` behind `serde_json`. The focused red test
failed for the expected missing API, the green run passed 33 `agent_context`
tests, and the standard workspace gates passed. CodeRabbit reviewed commit
`bb721ab` with zero findings. The only gap is the pre-existing
`--no-default-features` compile failure recorded in Surprises &
discoveries and Decision D7.

Milestone 2 outcome (2026-06-24): `cargo-orthohelp` now has a guard test that
walks the clap command tree, including hidden aliases, and rejects public
`context` or `agent-context` command names. The existing
`--format agent-context` value remains accepted. A temporary mutation adding
`alias = "context"` to `Orthohelp` failed the guard with
`alias 'context' on 'orthohelp'`; the mutation was reverted and the guard
passed. Standard workspace gates passed after extracting helper functions to
satisfy Clippy's excessive-nesting lint. CodeRabbit reviewed commit `1c581a3`
with zero findings after the required rate-limit wait and retry.

Milestone 3 outcome (2026-06-24): the `hello_world` example now exposes
`context --json` as a downstream application command. `context --json`
short-circuits before config merging, writes compact agent-context JSON to
stdout, and exits successfully; bare `context` prints a human pointer to
`--json`. The illustrative payload is hand-authored in
`examples/hello_world/src/cli/context.rs`, documented as a convention demo, and
validated by BDD, process-level e2e tests, an insta snapshot, and localized
help snapshot updates. Standard workspace gates passed after adding localized
context command copy and accepting the intended help snapshot drift. CodeRabbit
reviewed commit `15b4d39` with zero findings.

Milestone 4 outcome (2026-06-24): ADR-007 records the downstream
`context --json` naming decision, the accepted Y-Statement, the rejected
alternatives, the `schema_version` compatibility rule, and prior-art
references. The agent-native design now treats `context --json` as the
normative downstream application command while preserving
`cargo-orthohelp --format agent-context` as the generator format. The users'
guide, developers' guide, design decision log, cargo-orthohelp design, and
contents index now all reference the convention. CodeRabbit reviewed commit
`393b54c` with zero findings after the required rate-limit wait and retry.

Milestone 5 outcome (2026-06-24): `docs/roadmap.md` now marks 6.2.3 and its
three subrequirements complete, with ADR-007 added to the item's references.
The final deterministic gate set passed before the closeout commit:
`make markdownlint`, `make check-fmt`, `make typecheck`, `make lint`, and
`make test`. CodeRabbit reviewed commit `173507f` with zero findings after the
required rate-limit waits and retries.

## Context and orientation

The reader is assumed to know nothing about this repository. Key locations:

- `ortho_config/src/agent_context/mod.rs` — the reusable agent-context types.
  Defines `AgentContext`, `AgentContext::new(package)` (sets
  `kind = "<package>.agent_context"`), constants
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION = "1"` and
  `AGENT_CONTEXT_KIND_SUFFIX = "agent_context"`, and the command/input/enum
  types. `json.rs` provides the feature-gated compact and pretty render
  helpers. All types derive serde `Serialize`/`Deserialize` and
  `PartialEq`/`Eq`.
- `ortho_config/src/agent_context/tests.rs` and
  `ortho_config/src/agent_context/tests_json.rs` — unit tests (`rstest`,
  `insta`, `serde_json`), including `new_context_uses_legacy_defaults` and an
  insta wire-contract snapshot. The sibling JSON module keeps both test files
  within the file-size limit.
- `ortho_config/src/lib.rs` — the crate's public re-export block (around lines
  44 and 61-65) that re-exports `agent_context` items, `agent_context_kind`,
  and feature-gated JSON adapter functions. It does not re-export command or
  flag literals.
- `ortho_config/src/error/conversions.rs` — already provides
  `#[cfg(feature = "serde_json")] impl From<serde_json::Error> for OrthoError`,
  so `serialize_agent_context(ctx)?` composes in any `Result<_, OrthoError>`
  function. No new error type is needed.
- `ortho_config/Cargo.toml` — `serde_json` is an optional, default-on feature.
- `cargo-orthohelp/src/cli/mod.rs` — the generator CLI. `OutputFormat` enum (with
  `AgentContext`) and `CargoSubcommand` (only `Orthohelp`). The positive
  control lives in `cli::tests`; the reserved-name guard lives in
  `cli/reserved_agent_context_tests.rs`.
- `cargo-orthohelp/src/agent_context/mod.rs` and
  `cargo-orthohelp/src/output.rs` — the generator transform
  (`bridge_ir_to_agent_context`) and the file writer (`write_agent_context`,
  using `to_string_pretty`, target file `agent-context.json`). Unchanged by
  this plan.
- `examples/hello_world/` — a multi-subcommand example using clap derive. Parser
  re-exported as `CommandLine`; subcommand enum `Commands` with `Greet` and
  `TakeLeave` (`examples/hello_world/src/cli/mod.rs`); dispatch in
  `examples/hello_world/src/main.rs:30-39`; `package.metadata.ortho_config`
  declares `root_type` and locales. `assert_cmd` is already a dev-dependency.
  The BDD harness runs the compiled binary via `std::process::Command`
  (`examples/hello_world/tests/rstest_bdd/behaviour/harness/process.rs`).
- `docs/agent-native-cli-design.md` §3.2 (lines ~145-218) and §5 (lines
  ~335-381) — the prose preference for `context --json` and the canonical-flag
  vocabulary. `docs/adr-003-...md`, `docs/cargo-orthohelp-design.md` §6.3.1,
  `docs/users-guide.md` (§"Documentation and agent contracts", ~196-220),
  `docs/developers-guide.md` (§"Schema ownership" ~52-113, §"Generating
  agent-context output" ~114-193), `docs/contents.md`, `docs/roadmap.md` (item
  6.2.3 at lines 159-166).

Terms of art:

- *Agent context*: a compact, machine-readable JSON document describing how to
  invoke a CLI, intended to be cheap for an automated agent to load. It is a
  sibling of, not nested within, the localized documentation IR.
- *`kind` discriminator*: a string field that lets a consumer recognize the
  payload type without inspecting its shape, here `<tool>.agent_context`. The
  same idea appears as Kubernetes `kind` and Dapr reverse-DNS-prefixed kinds.
- *Generator format* vs *application command*:
  `cargo orthohelp --format agent-context` is a build-time generator that
  writes a file; `<tool> context --json` is a runtime application command that
  prints the payload to stdout.

## Plan of work

The work proceeds as four implementation milestones after the approval gate,
each ending in validation. Stages within a milestone follow Red-Green-Refactor.

### Milestone 1 — convention API in `ortho_config::agent_context`

Edit `ortho_config/src/agent_context/mod.rs`:

1. Keep the schema constants and add a public `kind` constructor. Downstream
   command and flag literals remain owned by each application, including the
   illustrative `hello_world` CLI:

   ```rust
   /// Builds the canonical agent-context document `kind` for a package:
   /// `"{package}.{AGENT_CONTEXT_KIND_SUFFIX}"`. Single authoritative place for
   /// the rule; downstream callers must not hand-format the discriminator.
   #[must_use]
   pub fn agent_context_kind(package: &str) -> String {
       format!("{package}.{AGENT_CONTEXT_KIND_SUFFIX}")
   }
   ```

   `AgentContext::new` changes only its `kind` line to call
   `agent_context_kind(&package_name)`; signature and behaviour are unchanged.

2. Add `serde_json`-gated render helpers in
   `ortho_config::agent_context::json`:

   ```rust
   #[cfg(feature = "serde_json")]
   pub fn serialize_agent_context(
       context: &AgentContext,
   ) -> Result<String, serde_json::Error> {
       let mut json = serde_json::to_string(context)?;
       json.push('\n');
       Ok(json)
   }

   #[cfg(feature = "serde_json")]
   pub fn serialize_agent_context_pretty(
       context: &AgentContext,
   ) -> Result<String, serde_json::Error> {
       serde_json::to_string_pretty(context)
   }
   ```

   Compact is the command-surface wire form (cheap to load, matches prior art);
   pretty is offered only for parity with the generator's file output. The
   bare `serde_json::Error` return composes with the existing `From` impl, so
   no new error type is added.

3. Re-export `agent_context_kind` and the feature-gated JSON adapter functions
   from `ortho_config/src/lib.rs`. No reusable command or flag constants, and
   no `AgentContext::to_json` or `AgentContext::to_json_pretty` methods, are
   part of the final public API.

Tests (extend `ortho_config/src/agent_context/tests.rs`): see Validation.

### Milestone 2 — guard test in `cargo-orthohelp`

The guard-test module
`cargo-orthohelp/src/cli/reserved_agent_context_tests.rs` owns
`no_context_or_agent_context_subcommand_alias`. It walks `Cli::command()` via
`clap::CommandFactory` and asserts that no command in the tree is named
`context` or `agent-context` and that no command exposes either as an alias
(using `get_all_aliases()` so hidden aliases cannot slip through). The existing
`format_accepts_agent_context` test remains the positive control. No new
dependency.

### Milestone 3 — illustrative `context --json` in the example

Add a small `context` command to `examples/hello_world`:

1. New module `examples/hello_world/src/cli/context.rs` that builds a
   *hand-authored, illustrative* `AgentContext` for the example using the
   public types (`AgentContext::new("hello_world")` plus a couple of
   `AgentCommand` entries for `greet`/`take-leave`), clearly commented as a
   convention demonstration, not an auto-generated tree. Provide a function
   returning the `AgentContext` and a renderer using
   `serialize_agent_context`.
2. Wire a `context` surface with a `--json` flag using application-owned
   literals. Handle it as an early short-circuit in
   `examples/hello_world/src/main.rs` before
   `load_globals_and_merge_selected_subcommand` (Decision D3): if the parsed
   invocation is `context`, render and print, then return — bypassing config
   merge. With `--json`, print `serialize_agent_context` output to stdout and
   exit 0; without
   `--json`, print a one-line human pointer to `--json` on stdout and exit 0
   (Decision D2).
3. Keep `context` output on stdout and diagnostics on stderr.

If integrating `context` as a clap-derive subcommand forces it through the
`Commands` merge machinery, prefer adding it as a top-level optional flag/route
that is inspected before the merge, or a `Commands::Context` variant that
`main.rs` peels off before calling the merge. If neither fits cleanly within
the file/line tolerance, stop and escalate (this is the boundary tolerance).

Tests: BDD feature + scenarios, insta snapshot, and `assert_cmd` e2e — see
Validation.

### Milestone 4 — documentation and ADR

1. New `docs/adr-007-downstream-context-command-naming.md` in the existing
   `docs/adr-NNN-*.md` house format (use ADR-006 as the structural template;
   load the `arch-decision-records` skill). Sections: Status (`Accepted.`, date
   2026-06-14), Context and problem statement (including the prior-art
   tension), Decision drivers, Options considered (Option A: follow prior art
   `agent-context`; Option B chosen: `context --json` + `kind` discriminator +
   no pre-release alias; Option C: shape-only, no `kind`) with a captioned
   comparison table, Decision outcome with an explicit Y-Statement sentence,
   Goals and non-goals, Known risks and limitations (including the
   `kind`-is-not-a-version warning), References (links to
   `agent-native-cli-design.md` §3.2/§5, `cargo-orthohelp-design.md` §6.3.1,
   ADR-003, and external prior art: Chow's "10 Principles", Cloudflare
   Wrangler, the Kubernetes `kind` convention, Dapr reverse-DNS kinds).
2. `docs/agent-native-cli-design.md`: promote the §3.2 prose (lines ~176-193)
   from "should prefer" to a normative rule (downstream surfaces *expose*
   `context --json`; payload *identifies* via `kind`; aliases avoided
   pre-release; generator *retains* `--format agent-context`), add the
   prior-art citation and a "see ADR-007" link; in §5 note `--json` stays
   canonical and the `context` command name is fixed by ADR-007.
3. `docs/users-guide.md`: new subsection under "Documentation and agent
   contracts" giving downstream authors a worked `context --json` example built
   with `AgentContext::new`, stating plainly that applications expose `context`
   (not `agent-context`) and that compatibility is detected via
   `schema_version`, not by parsing `kind`. Link ADR-007.
4. `docs/developers-guide.md`: in "Schema ownership", note
   `AGENT_CONTEXT_KIND_SUFFIX` + `agent_context_kind` as the single source for
   the `kind` discriminator (no hand-formatting); in "Generating agent-context
   output", add one sentence distinguishing generator format from the downstream
   `context --json` command, citing ADR-007.
5. `docs/design.md`: add a dated decision-log bullet linking ADR-007 and
   `agent-native-cli-design.md` §3.2 (motivation + link only).
6. `docs/cargo-orthohelp-design.md` §6.3.1: add a sentence clarifying that
   `--format agent-context` is the generator format, unchanged by ADR-007, and
   that `context --json` applies only to downstream application surfaces.
7. `docs/contents.md`: register the ADR-007 entry under the decisions section.

### Milestone 5 — finalize

Mark `docs/roadmap.md` item 6.2.3 (and its three sub-bullets) done, adding the
ADR-007 reference to its `See` line. Run the full gate. Clear CodeRabbit.

## Concrete steps

Run all commands from the repository root
(`/home/leynos/.lody/repos/github---leynos---ortho-config/worktrees/3d5cd4de-ad94-49af-8867-b403bd1bbf77`).
Per the global agent instructions, pipe long gate output through `tee` to a
log, for example
`make test 2>&1 | tee /tmp/test-ortho-config-$(git branch --show-current).out`.
Do not run gates in parallel (the build cache benefits from sequential runs).

Per-milestone loop:

1. Write the red test(s) first; run the focused test and confirm it fails for
   the intended reason: `cargo test -p ortho_config agent_context` (Milestone
   1), `cargo test -p cargo-orthohelp
   no_context_or_agent_context_subcommand_alias` and `cargo test -p
   cargo-orthohelp format_accepts_agent_context` (Milestone 2),
   `cargo test -p hello_world` (Milestone 3).
2. Make the minimal production change; rerun the focused test (green).
3. Refactor; rerun the focused test and then the milestone gates:
   `make check-fmt`, `make typecheck`, `make lint`, `make test` (sequentially).
4. Commit. Then run `coderabbit review --agent` and clear all concerns before
   the next milestone.

After Milestone 1, additionally verify the feature matrix:
`cargo check -p ortho_config --no-default-features` and
`cargo test -p ortho_config --features serde_json` (and, if available,
`cargo semver-checks check-release` across those features) to prove the render
helpers are correctly gated and the change is additive.

## Validation and acceptance

### Milestone 1 — crate API

Red-Green-Refactor in `ortho_config/src/agent_context/tests.rs`:

- Constants: `agent_context_command_name_const_is_context` (equals `"context"`);
  `agent_context_json_flag_const_is_long_json` (equals `"json"`, pinning
  Decision D6). Reference both via the `ortho_config::` crate-root path to
  prove re-export.
- `agent_context_kind` parameterized `#[rstest]` table
  `agent_context_kind_appends_suffix`:
  `("example-cli", "example-cli.agent_context")`,
  `("hello_world", "hello_world.agent_context")`, `("", ".agent_context")`
  (locks the empty-name edge), `("ns.tool", "ns.tool.agent_context")` (locks
  the dotted-name behaviour), `("Foo-Bar_9", "Foo-Bar_9.agent_context")`.
- `new_uses_agent_context_kind`:
  `AgentContext::new(pkg).kind == agent_context_kind(pkg)` for a couple of
  packages; the pre-existing `new_context_uses_legacy_defaults` stays as the
  regression anchor.
- Serializer adapter: `to_json_is_valid_parseable_json` (parses as a JSON
  object); `to_json_round_trips_via_serde` (`from_str::<AgentContext>` equals
  the original); `to_json_is_deterministic` (byte-for-byte equal across two
  calls); `to_json_includes_kind_and_schema_version` (parsed `kind` ends with
  `AGENT_CONTEXT_KIND_SUFFIX`; `schema_version` equals
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`); `to_json_has_trailing_newline` and
  `pretty_json_is_indented_without_a_trailing_newline` (pin the newline policy).
- Property (Decision D5; `ortho_config` dev-dependency `proptest`, or
  `#[rstest]` fallback): `to_json_always_round_trips` — for an arbitrary
  `AgentContext` (arbitrary package and a small `Vec<AgentCommand>` spanning
  the enum variants), `from_str(to_json(&ctx)?)? == ctx`.

Acceptance evidence (2026-06-24):

- Red:
  `cargo test -p ortho_config agent_context` failed with missing
  `agent_context_kind` and the feature-gated serializer adapter functions.
- Green:
  `cargo test -p ortho_config agent_context` passed with 33 `agent_context`
  tests.
- Feature check:
  `cargo test -p ortho_config --features serde_json agent_context` passed with
  33 `agent_context` tests.
- Standard gates:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all passed.
- Caveat:
  `cargo check -p ortho_config --no-default-features` failed on pre-existing
  discovery/file feature imports unrelated to the new render helpers. See
  Decision D7 before treating this milestone as a complete no-default feature
  cleanup.

### Milestone 2 — guard test

Acceptance: `no_context_or_agent_context_subcommand_alias` passes and *fails*
if a `context`/`agent-context` subcommand or alias is added (verify by
temporarily adding one and observing the failure, then reverting).
`format_accepts_agent_context` continues to pass.

Acceptance evidence (2026-06-24):

- Positive guard:
  `cargo test -p cargo-orthohelp no_context_or_agent_context_subcommand_alias`
  passed and ran `cli::tests::no_context_or_agent_context_subcommand_alias`.
- Mutation red:
  after temporarily adding `#[command(version, alias = "context")]` to
  `CargoSubcommand::Orthohelp`, the same focused test failed with
  `alias 'context' on 'orthohelp'`.
- Positive format control:
  `cargo test -p cargo-orthohelp format_accepts_agent_context` passed and ran
  `cli::tests::format_accepts_agent_context`.
- Standard gates:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all passed.

### Milestone 3 — example behaviour

BDD feature `examples/hello_world/tests/features/agent_context.feature`, wired
through the dedicated `examples/hello_world/tests/agent_context_bdd.rs` target:

```gherkin
Feature: hello_world agent context command

  Scenario: Emitting machine-readable context as JSON
    When I run the hello world example with arguments "context --json"
    Then the command succeeds
    And stdout contains a valid agent-context payload with kind "hello_world.agent_context"

  Scenario: Bare context prints a pointer to --json
    When I run the hello world example with arguments "context"
    Then the command succeeds
    And stdout contains "--json"
```

(The second scenario encodes Decision D2; if review overturns D2, swap it for a
failure assertion.)

insta snapshot: `context_agent_context_json_snapshot` over the rendered string
(library-level, no process spawn); keep deterministic by relying on serde field
order and sorting any collections by `path`/`name` to mirror the generator.

e2e (`assert_cmd`, already a dev-dependency)
`examples/hello_world/tests/agent_context_e2e.rs`:
`context_json_emits_parseable_payload` (stdout parses;
`kind == "hello_world.agent_context"`; `schema_version == "1"`),
`context_json_writes_only_to_stdout` (stderr empty), and
`context_exit_code_is_zero`.

Acceptance: `cargo test -p hello_world` passes, including BDD, snapshot, and
e2e; `cargo run -p hello_world -- context --json` prints a payload whose `kind`
is `hello_world.agent_context`.

Acceptance evidence (2026-06-24):

- Red:
  `cargo test -p hello_world agent_context` failed before implementation with
  unresolved import `hello_world::cli::context::render_agent_context_json`.
- Green focused:
  `cargo test -p hello_world context` passed the two BDD scenarios, three
  `assert_cmd` e2e tests, and the `context_agent_context_json_snapshot` insta
  test.
- Snapshot update:
  `INSTA_UPDATE=always cargo test -p hello_world context_agent_context_json_snapshot`
  wrote `agent_context_snapshot__context_agent_context_json.snap`.
- Help snapshot update:
  the first full `make test` failed only the localized help snapshots because
  the new public `context` subcommand appears in help and missing-subcommand
  output. Added en-US and Japanese Fluent copy for the command, then ran
  `INSTA_UPDATE=always cargo test -p hello_world --test localised_help`; all 19
  localized-help tests passed and the affected snapshots were updated.
- Standard gates:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all passed
  after the snapshot and localization updates.

### Milestone 4 — documentation

Acceptance: `make check-fmt` passes (covers Markdown formatting); the ADR
follows the house format with a non-empty Y-Statement (named alternative, named
downside); all cross-links resolve; `contents.md` lists ADR-007. A manual read
confirms the `kind`-is-not-a-version warning appears in both ADR-007 and the
users' guide, and that en-GB Oxford spelling holds in prose while code/JSON
identifiers are untouched.

Acceptance evidence (2026-06-24):

- ADR shape:
  `docs/adr-007-downstream-context-command-naming.md` contains a non-empty
  Y-Statement, three considered options, a captioned comparison table, known
  risks, consequences, and external prior-art references.
- Cross-links:
  `docs/contents.md` registers ADR-007; `docs/design.md`,
  `docs/agent-native-cli-design.md`, `docs/users-guide.md`,
  `docs/developers-guide.md`, and `docs/cargo-orthohelp-design.md` link or
  refer to the accepted convention.
- Compatibility wording:
  ADR-007 and the users' guide state that consumers compare `schema_version`
  rather than parsing `kind`.
- Formatting caveat:
  `make fmt` was attempted and failed on unrelated line-length findings in
  `docs/execplans/6-1-2-nested-command-tree-behavioural-fixtures.md`. Unrelated
  formatter edits were restored; touched files were formatted with `mdtablefix`
  and `markdownlint-cli2 --fix`.
- Markdown gate:
  `make markdownlint` passed with zero errors.
- Standard gates:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all passed.

### Whole-task quality gates

- Tests: `make test` passes with the new unit, property, BDD, snapshot, e2e, and
  guard tests.
- Lint/format/type: `make lint`, `make check-fmt`, `make typecheck` all pass.
- Public API: additions are additive (minor); regenerate any public-API baseline
  and run `cargo-semver-checks` across `--no-default-features` and
  `--features serde_json` if those tools are wired into CI.
- Review: `coderabbit review --agent` is clear at each milestone before moving
  on; all deterministic gates pass before each CodeRabbit run.

Final acceptance evidence (2026-06-24):

- Roadmap:
  `docs/roadmap.md` marks item 6.2.3 and its three subrequirements complete and
  references ADR-007.
- Final documentation gate:
  `make markdownlint` passed with zero errors.
- Final standard gates:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all passed.
- Review:
  final CodeRabbit review completed with zero findings after commit `173507f`.

## Idempotence and recovery

Every step is re-runnable. Code edits are additive and guarded by tests; if a
milestone's gate fails, fix forward or `git restore` the milestone's files and
retry. Commit at each milestone for an easy rollback point. Snapshots are
regenerated with `cargo insta review` / `INSTA_UPDATE=always` only after
confirming the change is intended. No destructive or irreversible operations
are involved.

## Artifacts and notes

Expected `context --json` payload shape (top level), for comparison during
validation:

```json
{
  "schema_version": "1",
  "kind": "hello_world.agent_context",
  "package": "hello_world",
  "commands": [],
  "profiles": { "supported": false },
  "feedback": { "supported": false },
  "policy": { "agent_native": "warn" }
}
```

(`commands` is populated by the example's hand-authored illustrative entries.)

## Interfaces and dependencies

At the end of Milestone 1, `ortho_config::agent_context` exposes the schema
model and `kind` constructor. The crate root re-exports only these public
symbols and the feature-gated JSON adapter functions:

```rust
pub fn agent_context_kind(package: &str) -> String;

#[cfg(feature = "serde_json")]
pub fn serialize_agent_context(
    context: &AgentContext,
) -> Result<String, serde_json::Error>;
pub fn serialize_agent_context_pretty(
    context: &AgentContext,
) -> Result<String, serde_json::Error>;
```

The example CLI owns its `context` and `json` literals; `AgentContext` has no
JSON rendering methods.

No new runtime dependencies. The only candidate dev-dependency is `proptest` on
`ortho_config` (Decision D5), with a zero-dependency `#[rstest]` fallback.
`cargo-orthohelp` gains only a test. The `hello_world` example gains a
`context` route and tests, using the already-present `assert_cmd`
dev-dependency.

## Signposts — documentation and skills

- Design and rationale: `docs/agent-native-cli-design.md` §3.2 and §5;
  `docs/design.md`; `docs/cargo-orthohelp-design.md` §6.3.1;
  `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`.
- Style and decisions: `docs/documentation-style-guide.md`; skills
  `arch-decision-records` (ADR-007), `en-gb-oxendict` (prose),
  `arch-crate-design` and `arch-supply-chain` (public-API/SemVer boundary).
- Testing: `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/rstest-bdd-users-guide.md`; skills `rust-unit-testing`, `proptest`,
  `nextest`.
- Localization context (example):
  `docs/localizable-rust-libraries-with-fluent.md`.
- Complexity discipline:
  `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- Rust routing: `rust-router`, then `rust-types-and-apis` (API shape) and
  `rust-errors` (the `serde_json::Error` return).
- Prior art consulted: Trevin Chow, "10 Principles for Agent-Native CLIs"
  (three-layer introspection, canonical `--json`); Cloudflare Wrangler
  (`--json`, schema-generated surface); Kubernetes `kind` and Dapr reverse-DNS
  kinds (discriminator convention).

## Revision note

- Initial draft (2026-06-14): created from roadmap 6.2.3, grounded by repository
  exploration, a three-agent planning team (API, tests, docs), prior-art
  research via Firecrawl, and a Logisphere design-review panel. The panel's
  verdict was "proceed with conditions"; all four conditions are folded in as
  constraints and decisions (`kind` governed by `schema_version`;
  bare-`context` human default per D2; illustrative example context per the
  drift risk; `serde_json` feature gate). Awaiting approval before
  implementation.
- Implementation start update (2026-06-24): marked the plan `IN PROGRESS` after
  explicit approval, recorded branch/PR/Lody-session metadata, and corrected
  the concrete working-directory path. This does not change the planned
  implementation; it only makes the plan resumable from the active worktree.
- Milestone 1 update (2026-06-24): recorded the implemented crate API, red and
  green test evidence, standard gate results, the already-present `ortho_config`
  `proptest` dev-dependency, and the pre-existing `--no-default-features`
  failure in discovery/file loading. Remaining work is unchanged except that no
  new dependency is needed for the property test.
- Milestone 1 review update (2026-06-24): recorded CodeRabbit's zero-finding
  review of commit `bb721ab`. Milestone 2 may proceed after this checkpoint.
- Milestone 2 update (2026-06-24): recorded the cargo-orthohelp guard test,
  the corrected focused test commands, the temporary-alias mutation failure,
  and the standard gate results. Remaining work is unchanged except that future
  agents should not use the ineffective `--lib cli` filter for this binary
  module.
- Milestone 2 review update (2026-06-24): recorded CodeRabbit's rate-limit
  response, the 77-minute `vsleep`, and the zero-finding retry for commit
  `1c581a3`. Milestone 3 may proceed after this checkpoint.
- Milestone 3 update (2026-06-24): recorded the hello_world `context --json`
  implementation, the dedicated BDD target, the intended localized-help
  snapshot drift, the added Fluent context copy, and the green focused and
  workspace gates.
- Milestone 3 review update (2026-06-24): recorded CodeRabbit's zero-finding
  review of commit `15b4d39`. Milestone 4 may proceed after this checkpoint.
- Milestone 4 update (2026-06-24): recorded ADR-007 and the supporting
  documentation updates, the `make fmt` caveat, and green Markdown and standard
  workspace gates.
- Milestone 4 review update (2026-06-24): recorded CodeRabbit's rate-limit
  response, the 58-minute `vsleep`, and the zero-finding retry for commit
  `393b54c`. Milestone 5 may proceed after this checkpoint.
- Milestone 5 update (2026-06-24): marked roadmap item 6.2.3 complete,
  recorded the final deterministic gates, and left the final CodeRabbit review
  pending the closeout commit.
- Final review update (2026-06-24): recorded CodeRabbit's two final rate-limit
  responses, the 75-minute and 88-minute `vsleep` waits, and the zero-finding
  retry for commit `173507f`.
- Rebase update (2026-06-24): rebased the branch onto `origin/main`. Conflicts
  were resolved by keeping both the upstream command-localization parser path
  and this branch's `hello_world context --json` short-circuit, combining
  crate-root agent-context exports, and merging the documentation so ADR-007's
  downstream command naming guidance coexists with the skill-manifest schema
  notes. Post-rebase validation required restoring dropped `AgentContext`
  derives and rstest attributes, updating the `hello_world` snapshot for the
  default `skill_manifests: []` field, and removing a Clippy-reported Rustdoc
  spacing issue. `make check-fmt`, `make test`, `make typecheck`, and
  `make lint` passed; the Cargo-heavy gates used `CARGO_BUILD_JOBS=1` after
  unconstrained nested Cargo builds hit OS process/thread limits.
- Review remediation update (2026-07-15): moved JSON formatting to the
  feature-gated `agent_context::json` adapter boundary, kept `AgentContext` as
  the schema model, and moved downstream command and flag literals into the
  example CLI. The cargo-orthohelp reserved-name guard moved to a sibling test
  module.
- Review remediation validation (2026-07-15): `make check-fmt`, `make test`,
  `make typecheck`, `make lint`, and `make markdownlint` passed. CodeRabbit
  reviewed the final remediation with zero findings; no rate-limit retry was
  needed.
- Rebase update (2026-07-15): rebased cleanly onto `origin/main` at
  `2cca782`. Upstream's Oxford spelling and lint-policy updates introduced no
  additional branch changes because the touched code and documentation already
  conform; post-rebase validation stayed green before publishing.
- Review follow-up (2026-07-15): verified the reserved-name mutex, complete
  `TakeLeaveCommand` context inputs, context-path observability, and all
  documentation findings against the rebased tree. The mutex now recovers from
  poison and covers only the shared bridge subprocess; the example context
  declares every command input and emits payload-free tracing at dispatch and
  failure boundaries. The polymer vocabulary request was stale because the
  shared spelling source and generated configuration already contain its full
  mapping. `make check-fmt`, `make test`, `make typecheck`, `make lint`, and
  `make markdownlint` passed; final CodeRabbit review reported zero findings.
