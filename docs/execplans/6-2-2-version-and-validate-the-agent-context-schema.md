# Version and validate the agent-context schema (6.2.2)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS (implementation started 2026-06-24; revised after roadmap
6.1.2 landed on `main`; see the revision note at the end)

## Purpose / big picture

OrthoConfig emits a compact, machine-readable "agent-context" JSON document so
that automated agents can learn how to invoke a command-line interface (CLI)
without scraping help text. Roadmap item 6.2.1 shipped the schema types in
`ortho_config::agent_context`, the `cargo orthohelp --format agent-context`
generator, and a single golden snapshot. What is still missing is the contract
that makes the schema *safe to depend on*: tests that fail loudly when the wire
shape changes by accident, golden fixtures that prove the generator behaves
across the shapes downstream consumers will meet (a flat CLI, a CLI with enum
values, and a nested command tree), and a written compatibility policy that
tells a maintainer when they must bump `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`.

After this change a maintainer can observe success directly:

1. Running `make test` passes, and a deliberate, accidental edit to any
   agent-context field name, enum wire string, or serialization attribute makes
   a named test fail (see `Validation and acceptance`).
2. `cargo orthohelp --format agent-context` produces JSON that matches a
   committed golden snapshot for each of three command-tree shapes, all rooted
   in the existing `orthohelp_fixture` crate and selected with `--root-type`:
   a simple flat CLI (a new minimal `SimpleFixtureConfig`), a CLI with enum
   values (the existing `FixtureConfig`, the crate's default root), and a nested
   command tree (the `NestedFixtureConfig` added by roadmap 6.1.2).
3. `docs/agent-native-cli-design.md` §8.2 states, in plain rules, what changes
   are additive (no version bump) versus breaking (version bump required), and
   `docs/developers-guide.md` tells a contributor how to evolve the schema.

This plan is the moment the v1 wire contract ossifies. Several small but
load-bearing decisions about the *exact* v1 shape are therefore recorded under
`Approved decisions`.

## Plain-language glossary

- **Agent-context schema**: the Rust types in `ortho_config::agent_context`
  (`AgentContext`, `AgentCommand`, `AgentInput`, and supporting enums) and the
  JSON they serialize to. The "wire shape" is the exact JSON: field names,
  nesting, enum string values, and whether absent optional fields appear as
  `null` or are omitted entirely.
- **`ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`**: a public string constant
  (currently `"1"`) that identifies the major version of the wire shape.
- **Documentation IR (intermediate representation)**: the localized,
  human-documentation model in `ortho_config::docs` (`DocMetadata`,
  `OrthoConfigDocs`, `ORTHO_DOCS_IR_VERSION`). It is a *sibling* of the
  agent-context schema, versioned independently (see ADR-003).
- **Bridge IR**: the `DocMetadata` value that `cargo-orthohelp` obtains for a
  target package by generating an ephemeral crate, compiling it, and running it
  so it calls `<RootType as OrthoConfigDocs>::get_doc_metadata()`. The
  agent-context generator transforms this bridge IR.
- **Generator / transform**: `cargo_orthohelp::agent_context::bridge_ir_to_agent_context`,
  the adapter that turns bridge IR into an `AgentContext`.
- **Golden / snapshot test**: a test that compares produced output against a
  committed reference file using `insta`. A change to output fails the test
  until a human reviews and re-blesses the snapshot.
- **Fixture crate**: a small workspace crate under `tests/fixtures/` whose only
  purpose is to be compiled and introspected by `cargo-orthohelp` during tests.
- **Forward compatibility**: a consumer pinned to version *N* keeps working when
  it reads output from a *newer* producer that still emits version *N* (it
  ignores fields it does not recognize). This is the primary guarantee for
  agent-context, because OrthoConfig (the producer) is upgraded before the
  agents (consumers) that read its output.

## Constraints

Hard invariants. Violation requires escalation, not a workaround.

1. **Schema ownership (ADR-003).** The reusable agent-context schema and
   `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` stay in `ortho_config::agent_context`.
   `ortho_config` must not gain a dependency on Cargo metadata loading, process
   I/O, filesystem writing, or renderer details. Schema-shape and version-pin
   tests live in `ortho_config`; end-to-end generator golden tests live in
   `cargo-orthohelp`.
2. **No contract duplication across the boundary.** `cargo-orthohelp` tests
   must consume `ortho_config`'s version constant rather than re-hardcoding
   `"1"`, and must assert *transform* behaviour, not re-assert schema invariants
   that are `ortho_config`'s job.
3. **No crate dependency cycles.** Fixture crates depend on `ortho_config`
   (and its derive). `cargo-orthohelp` must not gain a compile-time dependency
   on any fixture crate; it discovers fixtures via the ephemeral bridge at test
   time only.
4. **Legacy-compatible existing formats.** The `cargo-orthohelp` `ir`, `man`,
   `ps`, and `all` formats, their spellings, generated file paths, stdout and
   stderr contracts, and process success/failure behaviour must not change
   except through the explicit, documented `--format all` decision in this plan
   (Milestone 4). See `docs/developers-guide.md` lines 101-105.
5. **The transform stays projective and non-localized.** The generator may copy
   or derive compact metadata from bridge IR but must not inspect rendered
   roff, PowerShell, or localized long help, and must not copy Fluent
   identifiers or localized long prose into agent-context.
6. **British English, Oxford spelling** in all prose and documentation, per the
   `en-gb-oxendict` conventions and `docs/documentation-style-guide.md`.
7. **Test-first.** Every behavioural change follows Red-Green-Refactor. Goldens
   for the nested fixture are never blessed blind; a structural assertion must
   pass first (see Milestone 3).

## Tolerances (exception triggers)

Stop and escalate (record in `Decision Log`) when any of these is reached:

1. **New runtime dependency.** If any milestone appears to need a new
   *non-dev* dependency on `ortho_config` (for example `schemars`), stop and
   escalate. Dev-dependencies or a non-default feature may be proposed but must
   be confirmed before adding.
2. **Public API signature change.** If locking the schema requires changing the
   public signature of `bridge_ir_to_agent_context`, `write_agent_context`, or
   any public `ortho_config::agent_context` type *beyond* serde attribute
   adjustments approved under `Approved decisions`, stop and escalate.
3. **Generator logic rewrite.** If the nested fixture reveals a bug in 6.1.1's
   recursive metadata or in `bridge_ir_to_agent_context` whose fix exceeds ~80
   net changed lines or touches `ortho_config_macros` derive logic beyond the
   default-rendering normalization in Milestone 2, stop and escalate (it may be
   6.1.x work, not 6.2.2).
4. **Scope.** If total net new/changed non-test, non-doc lines exceed ~250,
   stop and escalate.
5. **Iterations.** If a milestone's gates still fail after 3 focused attempts,
   stop and escalate.
6. **Ambiguity.** If an approved decision is later changed in a way that
   materially enlarges scope (for example "redesign all Option fields"),
   re-cost the affected milestone and escalate if a tolerance would break.

## Approved decisions

These decide the exact v1 wire shape that this plan ossifies.

1. **`summary` omission asymmetry (D1).** `AgentCommand.summary` uses
   `#[serde(skip_serializing_if = "Option::is_none")]`, so it is *omitted* when
   absent, whereas every other optional field (for example `canonical_verb`,
   `pagination`, `AgentInput.default`) serializes as explicit `null`.
   - Decision: **lock the current shape**; keep `summary` omitted-when-absent
     (it suits a "compact" payload), keep the others as explicit `null`, and
     *document the asymmetry as intentional* in §8.2. The compatibility policy
     names "toggling `skip_serializing_if`" as a breaking change so the
     asymmetry cannot drift silently.
   - Rejected alternative: normalize all optional fields to one convention. This
     is a wire change to the 6.2.1 shape and is scope creep for a "version and
     validate" item.
2. **Default-display rendering brittleness (D2).** Field defaults are rendered
   by the derive macro as `proc_macro2` token strings, for example
   `"String :: from(\"localhost\")"` and `"LogLevel :: Info"` (see the existing
   golden). The spacing around `::` is a formatting artifact of the toolchain,
   not a stable contract, so a `quote`/`proc_macro2`/rustc upgrade could flip
   every default-bearing golden to red.
   - Decision: **(a) add a whitespace-normalization step** for the rendered
     default display in the generator path so token re-spacing does not churn
     goldens, **and (b) document** in §8.2 and the developers-guide that
     `AgentInput.default` is a best-effort human-readable display, not a
     normative or machine-parseable wire value. This keeps the deliverable
     goldens stable without redesigning how defaults are captured.
   - If normalization proves to need derive-macro changes beyond a localized
     string transform, that trips Tolerance 3 → escalate.
3. **Include agent-context in `--format all` (D3).** `OutputFormat::AgentContext`
   is currently "Excluded from `--format all` until schema versioning is locked
   in 6.2.2" (`docs/developers-guide.md:177`; `cargo-orthohelp/src/main.rs:154`
   special-cases it). 6.2.2 locks the versioning.
   - Decision: **include agent-context in `--format all`**, fulfilling the
     promise. This is additive to `all` (it writes an extra `agent-context.json`
     and does not alter existing outputs), is the one externally visible
     behaviour change in this item, and is governed by Constraint 4. Update the
     three docs and the `--format all` coverage accordingly (Milestone 4).
   - Rejected alternative: keep it excluded and only correct the stale
     doc-comment. This leaves a dangling, now-unmotivated deferral.
4. **Enum casing inconsistency (D4).** `AsyncSubmissionMode` uses
   `#[serde(rename_all = "kebab-case")]` while `InteractionMode` and
   `PolicyMode` use `snake_case`, and `MutationEffect` uses per-variant
   `#[serde(rename = "...")]` kebab strings. The variants in play today
   (`inline`, `submit`) are single words, so kebab vs snake is invisible *now*,
   but it is a latent inconsistency a future cleanup could "fix", silently
   breaking the wire contract.
   - Decision: **standardize all enums to `snake_case` before locking.** This is
     a deliberate pre-1.0 wire change, folded into Milestone 1. The 6.2.1 wire
     snapshot and golden fixtures are re-baselined once, and variant-exhaustive
     tests pin every enum wire string after the change.
   - Rejected alternative: lock and document the current per-type casing as
     intentional. The clean pre-1.0 contract is preferred before downstream
     consumers depend on the schema.
5. **§8.1 defaulting-table reconciliation (D5).** The §8.1 "Defaulting for
   legacy derives" table lists fields that are *not* present on the shipped v1
   types (for example `supports_json`, `exit_classes`, `renderer.human`,
   `profile_support`, `capability_id`). The table is forward-looking across the
   whole agent-native phase, but §8.2's "readers apply §8.1 defaults" clause
   would otherwise reference fields that do not exist yet.
   - Decision: **annotate §8.1** to mark which rows are *realized in schema v1*
     versus *planned for a later schema version*, and scope §8.2's defaulting
     clause to the realized fields. No field is removed from the forward-looking
     table; it is only labelled.

## Risks

1. Risk: 6.2.2 is the first to run the *agent-context transform*
   (`bridge_ir_to_agent_context`, with its `walk`/`command_path` recursion) over
   a nested command tree. Roadmap 6.1.2 (now merged) validated the recursive
   `DocMetadata.subcommands` IR and its roff/PowerShell/man rendering thoroughly,
   but explicitly *not* the agent-context format. So the recursion is sound at
   the IR layer; the residual risk is confined to the agent-context projection of
   that tree (for example root-vs-child `bin_name`/`app_name` handling in
   `command_path`, summary localization per node, and command/input sort order).
   - Severity: low-medium (narrowed by 6.1.2). Likelihood: low-medium.
   - Mitigation: Milestone 2 adds an in-process structural assertion on the
     agent-context projection of a nested tree (expected paths, ordering) that
     must pass *before* any nested golden is blessed. Reuse 6.1.2's nested
     fixtures where possible (the `NestedFixtureConfig` crate type for the
     end-to-end golden; the in-crate `define_nested_fixture!` /
     `LocalizedDocMetadata` helper, or a hand-built `DocMetadata`, for the
     in-process test). If a real bug appears and its fix exceeds Tolerance 3,
     escalate as possible 6.1.x work rather than absorbing it silently.
2. Risk: Default-display strings churn goldens on toolchain upgrades (D2).
   - Severity: medium. Likelihood: medium.
   - Mitigation: D2 normalization plus the "non-normative display" policy.
3. Risk: Guard tests silently compile out. `ortho_config`'s `serde_json` is an
   optional feature (in the `default` set). A guard test that is not reachable
   without `serde_json` provides no protection in a no-default-features build.
   - Severity: low. Likelihood: low.
   - Mitigation: gate new `ortho_config` guard tests exactly as the existing
     `agent_context::tests` are (they already use `serde_json` under default
     features); verify with `cargo test -p ortho_config` (default features) that
     they run.
4. Risk: The shell-out golden tests are slow (each compiles an ephemeral bridge
   crate that builds the fixture crate).
   - Severity: low. Likelihood: high.
   - Mitigation: reuse the existing `orthohelp_fixture` crate (add only one
     minimal root type rather than new crates, so no new crate enters the
     workspace build); push most assertions to cheap in-process tests on
     `bridge_ir_to_agent_context`; reserve exactly one shell-out golden per root
     type as an end-to-end smoke (see test matrix).
5. Risk: Documentation drift — a hand-authored per-field schema table would rot
   against the Rust source.
   - Severity: medium. Likelihood: high if attempted.
   - Mitigation: do not author a per-field table; the rustdoc on
     `ortho_config::agent_context` plus the §3.2 JSON example plus the wire
     snapshot are the canonical field references. §8.2 holds only the rules.

## Progress

- [x] Stage A: approval of this plan and the `Approved decisions` (no code).
- [x] 2026-06-24: Milestone 1 complete — schema shape and version guards in
      `ortho_config`; deterministic gates and CodeRabbit review passed.
- [x] 2026-06-24: Milestone 2 complete — generator determinism property,
      default-display normalization, in-process nested structural assertions,
      deterministic gates, and CodeRabbit review passed.
- [ ] Milestone 3: three golden roots (simple / enum / nested) in the existing
      `orthohelp_fixture` crate — add a minimal `SimpleFixtureConfig`, reuse
      `FixtureConfig` (enum) and 6.1.2's `NestedFixtureConfig` (nested) — plus
      the parametrized golden + nested BDD scenario.
- [ ] Milestone 4: include agent-context in `--format all` (if D3 approved).
- [ ] Milestone 5: documentation — §8.2 policy, §8.1 reconciliation, ADR-003
      cross-reference, users-guide and developers-guide updates, roadmap tick.

Each milestone ends by running, in this order and sequentially (never in
parallel, to benefit from build caching): `make check-fmt`, `make typecheck`,
`make lint`, `make test`, then `coderabbit review --agent` with all concerns
cleared before the next milestone. Commit after each green milestone.

## Surprises & discoveries

- Observation: Most of the "version-pin / shape-guard" scaffolding the roadmap
  asks for already exists from 6.2.1.
  Evidence: `ortho_config/src/agent_context/tests.rs` already contains a
  version-independence test, a wire-contract snapshot, required-field
  deserialization guards, and a per-variant `MutationEffect` wire-value test.
  Impact: 6.2.2 is mostly *extending* existing modules, not greenfield work;
  the plan must audit and extend rather than duplicate.
- Observation: Roadmap 6.1.2 ("Nested command tree behavioural fixtures",
  commit `5073b6d`, PR #340) merged to `main` while this plan was in draft, and
  was picked up by a rebase. It added a real nested clap tree
  (`NestedFixtureConfig` and its subcommand enums) *to the existing*
  `orthohelp_fixture` crate, a hand-built in-crate nested IR
  (`define_nested_fixture!` / `LocalizedDocMetadata` in
  `cargo-orthohelp/src/test_support/nested_fixture.rs`), behavioural nested IR
  fixtures in `ortho_config/tests/rstest_bdd/`, and roff/PowerShell/IR nested
  golden snapshots.
  Evidence: `git show 5073b6d --stat`; `tests/fixtures/orthohelp_fixture/src/lib.rs`
  now defines `NestedFixtureConfig` alongside the original flat `FixtureConfig`;
  `cargo-orthohelp/tests/nested_subcommand_end_to_end.rs` drives the binary with
  `--root-type orthohelp_fixture::NestedFixtureConfig`.
  Impact: this plan no longer needs to author new fixture crates. It reuses
  `NestedFixtureConfig` for the nested golden via `--root-type`, reuses the flat
  `FixtureConfig` (the crate's default root) for the enum golden, and adds only
  a minimal `SimpleFixtureConfig` root for the simple golden. See the revised
  Decision Log and Milestone 3.
- Observation: 6.1.2 covers the nested tree only for IR, roff, and PowerShell —
  *not* the agent-context format.
  Evidence: `nested_subcommand_end_to_end.rs` and
  `tests/golden/nested_subcommand_snapshots.rs` invoke `--format ir|man|ps`
  only; no nested test calls `bridge_ir_to_agent_context` or
  `--format agent-context`.
  Impact: 6.2.2 is still the first to exercise the agent-context transform over
  a nested tree, but the underlying recursion is now well-tested, narrowing
  Risk 1.
- Observation: Implementation began on 2026-06-24 from branch
  `6-2-2-version-and-validate-the-agent-context-schema`, tracking
  `origin/6-2-2-version-and-validate-the-agent-context-schema`.
  Evidence: `git status --short --branch` reported a clean branch tracking
  origin before edits; PR #349 title was updated to
  `Version and validate the agent-context schema (6.2.2)`; Lody session
  `b88005e0-6d82-4ef3-bd6c-998039c3c514` was renamed to the same title.
  Impact: the plan has moved from approval to execution; subsequent entries
  record implementation progress and validation evidence.
- Observation: Milestone 1 extended the existing schema guard module rather
  than adding parallel tests.
  Evidence: `ortho_config/src/agent_context/tests.rs` now widens the inline
  wire snapshot, pins `AGENT_CONTEXT_KIND_SUFFIX`, proves unknown top-level
  fields deserialize, checks absent optional fields serialize as documented
  `null`s except for omitted `summary`, and adds variant-exhaustive tests for
  `AsyncSubmissionMode`, `InteractionMode`, `PolicyMode`, and
  `MutationEffect`.
  Impact: the guard suite now directly covers the approved D1 and D4 wire-shape
  decisions.
- Observation: D4's only visible current wire change is
  `MutationEffect::ReadOnly`, which now serializes as `read_only`.
  Evidence: the first focused red run,
  `/tmp/test-red-m1-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`,
  failed on `read-only` versus `read_only`; after changing
  `MutationEffect` to `#[serde(rename_all = "snake_case")]`, the focused green
  run passed 27 `agent_context` tests.
  Impact: the 6.2.1 schema snapshot and downstream goldens must be re-baselined
  once during later milestones, as approved in D4.
- Observation: The explicit red proofs for field and version drift passed.
  Evidence:
  `/tmp/test-red-field-m1-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`
  shows the snapshot failing when `canonical_verb` temporarily serialized as
  `verb`; `/tmp/test-red-version-m1-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`
  shows the version pin failing when `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` was
  temporarily changed to `"2"`. Both temporary mutations were reverted before
  the final focused green run.
  Impact: the Milestone 1 Red-Green-Refactor evidence required by this plan is
  captured in `/tmp` logs.
- Observation: Milestone 1 passed all deterministic gates and CodeRabbit review.
  Evidence: `make check-fmt`, `make typecheck`, `make lint`, and `make test`
  passed with logs in `/tmp/check-fmt-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`,
  `/tmp/typecheck-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`,
  `/tmp/lint-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`,
  and `/tmp/test-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`.
  `coderabbit review --agent` initially hit a rate limit, the required `vsleep`
  backoff ran for 63 minutes, and the retry completed with `findings: 0`.
  Impact: Milestone 2 may begin after the Milestone 1 commit.
- Observation: Milestone 2 used a hand-built nested `DocMetadata` for the
  in-process structural test rather than the 6.1.2 localized nested fixture.
  Evidence: `cargo-orthohelp/src/test_support/nested_fixture.rs` produces
  `LocalizedDocMetadata`, while `bridge_ir_to_agent_context` consumes
  `DocMetadata`; no reverse bridge-IR view exists. The test now pins the
  `fixture admin audit` and `fixture admin grant-access` paths, root/child
  inputs, sorted command order, and sorted input order directly against
  `DocMetadata`.
  Impact: the test remains on the adapter boundary under review and avoids
  adding conversion code solely for tests.
- Observation: D2 default-display normalization deliberately re-baselines the
  existing end-to-end agent-context golden.
  Evidence: the focused red run
  `/tmp/test-red-m2-agent-context-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`
  failed on `String :: from("localhost")`, `LogLevel :: Info`, and equivalent
  nested fixture defaults. After adding generator-side `::` spacing
  normalization and accepting the reviewed snapshot, the focused green run
  `/tmp/test-green-m2-agent-context-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`
  passed all `agent_context`-filtered tests.
  Impact: Milestone 3 starts from the normalized default-display contract, so
  new simple/enum/nested goldens should use tight `::` defaults.
- Observation: The first Milestone 2 red command did not use `set -o pipefail`,
  so the shell returned the `tee` exit status even though Cargo reported test
  failures.
  Evidence: the log named above contains failing tests despite the command
  wrapper reporting exit code 0.
  Impact: subsequent piped validation commands include `set -o pipefail` so
  command status reflects Cargo failures.
- Observation: Milestone 2 deterministic gates passed after refactoring the
  nested structural test into helper fixtures.
  Evidence: the first `make lint` run,
  `/tmp/lint-m2-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`,
  rejected the long nested test with `clippy::too_many_lines`. After extracting
  named nested fixture and expected-summary helpers, `make check-fmt`,
  `make typecheck`, `make lint`, `make test`, and `make markdownlint` passed
  with logs in `/tmp/check-fmt-m2-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`,
  `/tmp/typecheck-m2-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`,
  `/tmp/lint-m2-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`,
  `/tmp/test-m2-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`,
  and `/tmp/markdownlint-m2-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`.
  Impact: CodeRabbit review can be requested for Milestone 2 with deterministic
  issues already cleared.
- Observation: Milestone 2 CodeRabbit review passed with no concerns after two
  rate-limit backoffs.
  Evidence: `coderabbit review --agent` was rate-limited twice; the requested
  random `vsleep` backoffs ran for 64 minutes and 48 minutes. The third review
  attempt completed with `findings: 0`, logged at
  `/tmp/coderabbit-m2-retry2-ortho-config-6-2-2-version-and-validate-the-agent-context-schema.out`.
  Impact: Milestone 3 may begin after committing the Milestone 2 changes.

## Decision log

- Decision (SUPERSEDED 2026-06-15): originally, three *new* minimal fixture
  crates (`orthohelp_simple_fixture`, `orthohelp_enum_fixture`,
  `orthohelp_nested_fixture`). Superseded because roadmap 6.1.2 landed a reusable
  nested fixture and established the multi-root-per-crate convention; see the next
  entry.
- Decision: Use three golden *root types* within the existing `orthohelp_fixture`
  crate, selected via `--root-type`, rather than new fixture crates: add a
  minimal `SimpleFixtureConfig` (flat, scalars only, no enum) for the simple
  case; reuse the existing `FixtureConfig` (the crate's default root, which has
  the `LogLevel` enum and an existing agent-context golden) for the enum case;
  reuse 6.1.2's `NestedFixtureConfig` for the nested case.
  Rationale: 6.1.2 established the multi-root pattern
  (`--root-type orthohelp_fixture::NestedFixtureConfig`) and already ships a real
  nested tree, so authoring new crates would duplicate it and add workspace build
  cost for no benefit. Reusing root types keeps the three golden shapes isolated
  (each is a distinct root) while matching the now-canonical convention. The
  simple case still needs a *new* minimal root because `FixtureConfig` carries an
  enum and many fields and is not "simple"; adding one small struct to an existing
  crate is far cheaper than a new crate and needs no `Cargo.toml`/`members`
  change.
  Date/Author: 2026-06-15, planning (revised after 6.1.2 rebase).
- Decision: Dependency-free shape guard (insta snapshot + variant-exhaustive
  wire-value tests + version pin); no `schemars`.
  Rationale: a byte-exact snapshot is strictly stronger than a permissive JSON
  Schema diff for detecting accidental shape changes (it catches null-vs-absent
  and enum-string renames). schemars' real value is a *publishable* artifact,
  which is a different, deferred requirement; ADR-003 calls JSON Schema prior
  art, not a compatibility target. Avoids Tolerance 1.
  Date/Author: 2026-06-14, planning.
- Decision: Compatibility policy's primary guarantee is *forward* compatibility
  for pinned consumers; consumers must ignore unknown fields; producers must
  never add `#[serde(deny_unknown_fields)]` within a major version.
  Rationale: OrthoConfig (producer) is upgraded before agents (consumers); the
  risky path is an old consumer reading newer producer output. Confluent's
  compatibility-type framing maps this to forward compatibility. The types today
  correctly omit `deny_unknown_fields`.
  Date/Author: 2026-06-14, planning.
- Decision: Accept D1, D2, D3, and D5 as recommended, and choose D4's clean
  pre-1.0 enum casing change: all agent-context enums are standardized to
  `snake_case` before the v1 contract is locked.
  Rationale: D1, D2, D3, and D5 preserve the intended compact payload,
  stabilize default displays, fulfil the documented `--format all` deferral, and
  clarify realized versus future defaulting fields. For D4, a deliberate
  pre-1.0 re-baseline is less risky than freezing mixed enum casing and asking
  future maintainers to remember that the inconsistency is contractual.
  Date/Author: 2026-06-24, reviewer direction.

## Context and orientation

A reader new to this repository needs these anchors.

The workspace (`Cargo.toml`) contains `ortho_config` (the library that owns the
schema), `ortho_config_macros` (the derive macro), `cargo-orthohelp` (the CLI
tool / adapter that generates output), `test_helpers`, the
`examples/hello_world` crate, and `tests/fixtures/orthohelp_fixture` (the single
existing fixture crate). This plan adds *no* new workspace member: the three
golden roots all live in `orthohelp_fixture`.

Nested fixtures (added by roadmap 6.1.2, PR #340): the `orthohelp_fixture` crate
now defines a real nested clap tree, `NestedFixtureConfig`
(`tests/fixtures/orthohelp_fixture/src/lib.rs`), alongside the original flat
`FixtureConfig`. The tree is `nested-app` → `greet` / `version` / `admin`, with
`admin` → `audit` / `grant-access` (a two-level tree with a leaf command that has
no subcommands). It is selected at generation time with
`--root-type orthohelp_fixture::NestedFixtureConfig`. The crate's default root
(`[package.metadata.ortho_config] root_type`) is
`orthohelp_fixture::FixtureConfig`, so the existing agent-context golden already
covers the enum-bearing flat tree. For cheap in-process tests,
`cargo-orthohelp/src/test_support/nested_fixture.rs` exposes a `pub(crate)`
`define_nested_fixture!` macro producing a `LocalizedDocMetadata` value; note the
agent-context transform takes `&DocMetadata`, so an in-process nested test either
converts from that helper or hand-builds a small nested `DocMetadata` (as the
existing proptests do via their `doc()` helper). 6.1.2 covers the nested tree for
IR, roff, and PowerShell only — not agent-context.

Schema types and the version constant:
`ortho_config/src/agent_context/mod.rs` defines `AgentContext`,
`AgentCommand`, `AgentInput`, `AgentExample`, `AsyncSubmission`,
`DeliveryRoute`, `PaginationContract`, `SupportDeclaration`, `AgentPolicy`, and
the enums `AsyncSubmissionMode`, `PolicyMode`, `InteractionMode`,
`MutationEffect`, plus `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` (`"1"`) and
`AGENT_CONTEXT_KIND_SUFFIX` (`"agent_context"`). Existing tests are in
`ortho_config/src/agent_context/tests.rs`.

Generator (adapter): `cargo-orthohelp/src/agent_context/mod.rs` defines
`bridge_ir_to_agent_context(meta, package, localizer)` and the recursive
`walk`/`command_path` helpers. Its unit tests are in
`cargo-orthohelp/src/agent_context/tests.rs`; its property tests in
`cargo-orthohelp/src/agent_context/proptests.rs` (already covering unique paths,
path sort, input sort, hidden-field omission).

Generation pipeline: `cargo-orthohelp/src/main.rs` parses CLI
(`cargo-orthohelp/src/cli.rs`, `OutputFormat::AgentContext`), loads cargo
metadata (`metadata.rs`), builds bridge IR by compiling an ephemeral crate
(`bridge.rs`) that calls `OrthoConfigDocs::get_doc_metadata()`, transforms it,
and writes `<out>/agent-context.json` atomically (`output.rs`,
`write_agent_context`). `--format all` currently does *not* include
agent-context (`main.rs:154` special-cases it; ir/man/ps include `All`).

Existing end-to-end golden: `cargo-orthohelp/tests/golden/agent_context_tests.rs`
runs the built binary against `--package orthohelp_fixture` and snapshots
`cargo-orthohelp/tests/golden/agent_context__fixture.json.snap`. Behavioural
test: `cargo-orthohelp/tests/features/orthohelp_agent_context.feature` with
steps in `cargo-orthohelp/tests/rstest_bdd/behaviour/steps_agent_context.rs`.

Nested-subcommand mechanism: a `clap::Subcommand` enum deriving
`OrthoConfigSubcommandDocs` populates `DocMetadata.subcommands`; see
`ortho_config/tests/docs_ir_subcommands.rs` for the IR-level pattern.

Default rendering: defaults are turned into display strings in
`ortho_config_macros/src/derive/generate/docs/fields/tokens.rs` via
`expr.to_token_stream().to_string()` (the source of the space-separated `::`
strings).

Documentation homes: `docs/agent-native-cli-design.md` (§3.2 schema purpose,
§8 versioning/compatibility, §8.1 legacy defaulting), ADR-003
(`docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`),
`docs/users-guide.md` (consumer-facing note ~lines 204, 1297),
`docs/developers-guide.md` (schema ownership ~line 107, agent-context generation
~line 114, Public API ~line 137).

## Plan of work

### Stage A — approval (no code changes)

Present this plan and the `Approved decisions`. Do not begin implementation
until the user explicitly approves execution. Silence is not approval. If a
reviewer later changes an approved decision, update the `Decision Log` and
re-cost the affected milestone before proceeding.

### Milestone 1 — schema shape and version guards (`ortho_config`)

Goal: make accidental wire-shape changes fail a named test. Extend
`ortho_config/src/agent_context/tests.rs` (do not duplicate the existing tests).

Red → Green → Refactor for each addition:

1. **Comprehensive wire snapshot.** Add an `insta` snapshot test that serializes
   a maximally populated `AgentContext` (every field set; `pagination` `Some`;
   `delivery_route` `Some`; `async_submission` `Some`; at least one fully
   populated `AgentInput` with `enum_values`). This locks field presence,
   nesting, ordering, and the `null`-vs-omitted behaviour of every optional
   field. The existing `agent_context_json_snapshot_covers_wire_contract` test
   may be widened to this rather than adding a second near-duplicate.
2. **Variant-exhaustive wire-value tests.** A single value cannot exercise every
   enum variant (scalar fields hold one each). Add table tests (rstest
   `#[case]`) asserting the exact serialized string of *every* variant of
   `InteractionMode`, `PolicyMode`, and `AsyncSubmissionMode`, mirroring the
   existing `mutation_effect_serializes_canonical_wire_values`. This is what
   actually guards a rename such as `non_interactive` → `noninteractive` or a
   casing change on a variant not used by the snapshot.
3. **Version + kind pin.** Keep `agent_context_version_is_independent_from_docs_ir`.
   Extend it (or add a sibling) to assert `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION
   == "1"`, `AGENT_CONTEXT_KIND_SUFFIX == "agent_context"`, and that
   `AgentContext::new(pkg).kind` ends with the suffix. Add a doc-comment
   checklist on the snapshot test: "If this snapshot changed: (1) is the change
   additive-only? If not, bump `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` and add a
   §8.2 history row; (2) update the §8.1 realized-fields annotation;
   (3) confirm no `deny_unknown_fields` was added."
4. **Forward-compat tolerance test.** Add a test proving an agent-context
   payload with an *unexpected extra* top-level key still deserializes
   successfully (locks the "consumers ignore unknown fields" guarantee and
   prevents a future accidental `deny_unknown_fields`). Keep the existing
   missing-required-field rejection tests.
5. **Apply D4 before locking.** Standardize every agent-context enum to
   `snake_case`, then re-baseline the 6.2.1 wire snapshot and golden fixtures
   once, intentionally. D1 remains locked as the current `summary`
   omitted-when-absent asymmetry. Variant-exhaustive tests pin every enum's wire
   strings after the re-baseline.
6. **Feature gating.** Confirm the new tests are reachable under the same
   features as the existing `agent_context::tests` and run under default
   features.

Validation: `cargo test -p ortho_config agent_context` passes; then make the
Red proof — temporarily rename one enum variant's wire string and one field,
confirm the relevant tests fail, revert. Use `pretty_assertions` and
`googletest` matchers where they sharpen failure output.

### Milestone 2 — generator determinism and default-display policy (`cargo-orthohelp`)

Goal: replace a near-trivial property with one that constrains *our* code, and
stabilize default rendering. Work in
`cargo-orthohelp/src/agent_context/{proptests.rs,tests.rs}` and the generator.

1. **Determinism property (proptest).** Add a property: transforming the *same*
   arbitrary `DocMetadata` tree twice yields byte-identical pretty-printed JSON.
   This guards the generator's promised sort/normalization (the existing
   round-trip-style property is trivial for a serde type and is not added).
   Existing properties (unique paths, sorted commands/inputs, hidden-field
   omission) stay.
2. **Nested structural assertions (in-process, the Red gate for Milestone 3).**
   Add rstest unit tests that drive `bridge_ir_to_agent_context` over a *nested*
   `DocMetadata` (root with subcommands, one two-level branch, one leaf with no
   subcommands, one enum field) and assert the resulting `AgentContext`: expected
   `commands[].path` values (including a two-segment path), canonical verbs,
   per-command inputs, and that commands/inputs are sorted. Prefer reusing
   6.1.2's in-crate nested fixture
   (`cargo-orthohelp/src/test_support/nested_fixture.rs`,
   `define_nested_fixture!` → `LocalizedDocMetadata`) if a `DocMetadata` view is
   available; otherwise hand-build a small nested `DocMetadata` as the existing
   proptests do (`proptests.rs` `doc()` helper). These must pass before any
   nested golden is blessed; if they cannot be made to pass, that indicates a bug
   in the agent-context transform's recursion (Risk 1 / Tolerance 3) — note that
   6.1.2 already validated the IR-level recursion, so any failure is likely
   specific to `walk`/`command_path`.
3. **Default-display normalization (D2).** Add a normalization step so the
   rendered `AgentInput.default` display is insensitive to `proc_macro2` token
   spacing (for example collapse the spaced `::` separator to a tight `::`),
   with a focused unit test
   asserting the normalized form. Document the field as non-normative display
   (text lands in Milestone 5). Keep this a localized string transform; if it
   would require derive-macro restructuring, escalate (Tolerance 3).

Validation: `cargo test -p cargo-orthohelp agent_context` passes; Red proof for
the determinism property is inherently satisfied by construction, so instead
prove the nested structural test fails first against an empty/flat tree, then
passes against the nested tree.

### Milestone 3 — golden roots (simple / enum / nested)

Goal: prove end-to-end generation across the three shapes the roadmap names,
reusing the existing `orthohelp_fixture` crate and 6.1.2's nested fixture rather
than authoring new crates.

1. **Add one minimal root type** to `tests/fixtures/orthohelp_fixture/src/lib.rs`:
   `SimpleFixtureConfig`, a flat struct with two or three scalar fields (string,
   integer, bool), no enum, no subcommands. This is the only new fixture code;
   no new crate, no `Cargo.toml` or workspace `members` change is needed. Add the
   matching Fluent message keys to the crate's `locales/*/messages.ftl` as 6.1.2
   did for its nested commands. The other two roots already exist:
   - **enum**: `FixtureConfig` — the crate's default `root_type`, which carries
     the `LogLevel` enum and already has a committed agent-context golden.
   - **nested**: `NestedFixtureConfig` — 6.1.2's two-level tree, selected with
     `--root-type orthohelp_fixture::NestedFixtureConfig`.
2. **Parametrized golden test.** Refactor
   `cargo-orthohelp/tests/golden/agent_context_tests.rs` into an rstest
   `#[case]` test over `(root_type, snapshot_name)` (snapshot name explicitly
   coupled to the root, not case index), covering the three roots. Each commits
   its own `.snap`. The simple and nested cases pass `--root-type`; the enum case
   uses the crate default (or passes `--root-type orthohelp_fixture::FixtureConfig`
   explicitly for clarity). Keep the existing default-root golden so it is not
   orphaned.
   - Red gate: the Milestone 2 in-process nested structural test must already be
     green. Generate the nested golden, then review the diff line-by-line
     (paths, default strings, ordering) before committing — never blind-bless.
   - Fence vs 6.1.2: 6.2.2 asserts only the *agent-context* projection of
     `NestedFixtureConfig`. It does not add or modify roff/PowerShell/man render
     assertions over that tree — those are owned by 6.1.2's
     `tests/golden/nested_subcommand_snapshots.rs` and stay untouched.
3. **Nested BDD scenario.** Add a scenario to
   `cargo-orthohelp/tests/features/orthohelp_agent_context.feature` (+ steps)
   that runs agent-context generation over `NestedFixtureConfig` (via
   `--root-type`) and asserts a *two-segment* command path is present in
   `commands[].path`, so the behaviour layer distinguishes nested from flat (not
   merely "JSON exists").

Validation: `make test` passes; the three goldens exist and match; the nested
BDD scenario fails before its step assertions/generation are added and passes
after.

### Milestone 4 — include agent-context in `--format all` (if D3 approved)

1. In `cargo-orthohelp/src/main.rs`, make `--format all` also generate
   agent-context (treat `OutputFormat::All` like `AgentContext` for the
   agent-context branch) while leaving ir/man/ps behaviour unchanged.
2. Update or add the `--format all` coverage (golden/BDD) to include
   `agent-context.json`; confirm no existing `all` output path or content
   regresses (Constraint 4).
3. Update the `OutputFormat::AgentContext` doc-comment to state inclusion.

Validation: `cargo orthohelp --format all --package orthohelp_fixture`
(optionally with `--root-type orthohelp_fixture::SimpleFixtureConfig`) writes
`agent-context.json` alongside the existing artefacts; existing `all` goldens
still pass (except the intended additive change).

### Milestone 5 — documentation

1. **`docs/agent-native-cli-design.md` §8.2 "Agent-context schema compatibility
   policy"** (new subsection). Assert, in plain rules:
   - The version is `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`, an integer-valued
     string; it is a breaking-change generation counter, not semver, and is
     bumped *only* for breaking changes. Additive changes do not bump it.
   - Primary guarantee: forward compatibility for pinned consumers; consumers
     MUST ignore unknown fields; producers MUST NOT add `deny_unknown_fields`
     within a major version.
   - Permitted within a major version: adding an optional/defaulted field;
     adding an enum variant only where consumers treat unknown variants as the
     documented legacy default; widening accepted values. Absent fields take
     their §8.1 default, applied by the reader, never by validation.
   - Breaking (require a bump): renaming/removing a field; changing a JSON type;
     changing an enum variant's wire string (`serde(rename)`); changing a
     container's `rename_all` casing; making optional↔required in either
     direction; changing a serialized default or `Default` impl; toggling
     `skip_serializing_if` (present-as-null vs absent); adding
     `deny_unknown_fields`; removing an enum variant.
   - Key/field ordering is not semantically meaningful to consumers but is
     order-sensitive in the wire snapshot, so a reorder requires a deliberate
     snapshot review.
   - The contract is pinned by a byte-exact snapshot plus variant-exhaustive
     wire-value tests for every enum.
   - On a bump, retain the prior version's frozen wire fixture and a round-trip
     test so overlap compatibility can be demonstrated.
   - `AgentInput.default` is best-effort human-readable display, not a
     normative or machine-parseable value (D2).
   - The current `summary` omission asymmetry (D1) is intentional and locked by
     tests. All enum wire strings use the standardized `snake_case` policy from
     D4 and are locked by variant-exhaustive tests.
   - schemars/JSON-Schema emission is a deferred enhancement; if added it lives
     as a dev-dependency or behind a non-default feature.
2. **Reconcile §8.1 (D5).** Annotate each row as *realized in schema v1* or
   *planned for a later schema version*; do not delete forward-looking rows.
3. **ADR-003 cross-reference.** Add a one-line pointer from ADR-003's
   "Consequences" to §8.2 for the compatibility mechanism. No decision change;
   no new ADR (per `arch-decision-records`, this is mechanism layered on the
   existing ownership decision, not a new hard-to-reverse choice).
4. **`docs/users-guide.md`.** Extend the existing agent-contract note with the
   consumer stability promise (what a consumer may rely on across a given
   `schema_version`, and that it must ignore unknown fields). Update the
   `--format all` description if D3 was approved.
5. **`docs/developers-guide.md`.** Retire/fulfil the
   `OutputFormat::AgentContext` "until 6.2.2" doc-comment; under "Generating
   agent-context output" add the schema-evolution convention (when to bump the
   version, how to add an agent-context golden root, the nested-fixture
   6.2.2-vs-6.1.2 fence) and the default-display policy. Build on — do not
   duplicate — the fixture-isolation and `insta` snapshot guidance 6.1.2 added
   (developers-guide lines ~238-254); reference the multi-root pattern
   (`--root-type`) that 6.1.2 established. Do not author a per-field table —
   point to the rustdoc, the §3.2 example, and the wire snapshot.
6. **`docs/cargo-orthohelp-design.md`.** Update only if it documents the
   `--format all` bundle composition (then note agent-context inclusion).
7. **`docs/roadmap.md`.** On completion, tick 6.2.2's three checkboxes and the
   item.

Validation: `make markdownlint` and `make nixie` pass; cross-references resolve.

## Concrete steps

Run from the worktree root
`/home/leynos/.lody/repos/github---leynos---ortho-config/worktrees/b88005e0-6d82-4ef3-bd6c-998039c3c514`.

Gating sequence after each milestone (sequential, never parallel; tee to a log
per `AGENTS.md`):

```bash
# bash
ACTION=test; LOG="/tmp/${ACTION}-ortho-config-$(git branch --show-current).out"
make check-fmt 2>&1 | tee "/tmp/check-fmt-ortho-config-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-ortho-config-$(git branch --show-current).out"
make lint      2>&1 | tee "/tmp/lint-ortho-config-$(git branch --show-current).out"
make test      2>&1 | tee "$LOG"
coderabbit review --agent
```

Generating each golden root by hand (for inspection before blessing):

```bash
# bash — run from the worktree root; writes <out>/agent-context.json
cargo orthohelp --out-dir /tmp/ac-simple --package orthohelp_fixture \
  --root-type orthohelp_fixture::SimpleFixtureConfig --format agent-context
cargo orthohelp --out-dir /tmp/ac-enum --package orthohelp_fixture \
  --root-type orthohelp_fixture::FixtureConfig --format agent-context
cargo orthohelp --out-dir /tmp/ac-nested --package orthohelp_fixture \
  --root-type orthohelp_fixture::NestedFixtureConfig --format agent-context
```

Focused test commands during development:

```bash
# bash
cargo test -p ortho_config agent_context
cargo test -p cargo-orthohelp agent_context
cargo insta review   # to bless intentional snapshot changes only
```

Re-blessing a golden after an *intended* change:

```bash
# bash
cargo test -p cargo-orthohelp 2>&1 | tee /tmp/golden.out   # see the diff
cargo insta review                                          # accept consciously
```

## Validation and acceptance

Acceptance is behavioural and observable:

1. **Shape guard works (Red proof).** With the schema unchanged, `cargo test -p
   ortho_config agent_context` passes. Temporarily rename one field
   (for example `canonical_verb` → `verb`) and one enum wire string (for example
   `MutationEffect::ReadOnly`'s `"read-only"` → `"readonly"`); rerun and observe
   the comprehensive snapshot test and the relevant variant-exhaustive test
   fail. Revert; rerun; pass.
2. **Version pin works.** Temporarily change
   `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` to `"2"`; observe the pin test fail.
   Revert.
3. **Three goldens.** Generating agent-context for each of the three roots
   (`SimpleFixtureConfig`, `FixtureConfig`, `NestedFixtureConfig`) via
   `--root-type` produces JSON matching the committed `.snap`. The nested golden
   contains at least one two-segment `commands[].path`.
4. **Forward compatibility.** The unknown-extra-key deserialization test passes,
   proving consumers tolerate unknown fields.
5. **Determinism.** The proptest proving identical JSON across two transforms of
   the same tree passes with no shrink failures.
6. **`--format all` (if D3).** `--format all` writes `agent-context.json`
   alongside the existing artefacts and existing `all` outputs are unchanged.
7. **Docs.** §8.2 exists with the rules above; §8.1 rows are annotated;
   ADR-003 links to §8.2; users-guide and developers-guide updated;
   `make markdownlint` and `make nixie` pass.

Quality criteria ("done"):

- Tests: `make test` green, including the three goldens, the variant-exhaustive
  enum tests, the determinism property, the forward-compat test, and the nested
  BDD scenario.
- Lint/format/typecheck: `make check-fmt`, `make typecheck`, `make lint` all
  green.
- Review: `coderabbit review --agent` run after each milestone with all concerns
  cleared *before* the review (deterministic gates first).

Quality method: the gating sequence in `Concrete steps`, run sequentially after
each milestone, plus the explicit Red proofs in items 1-2.

## Idempotence and recovery

- All steps are re-runnable. `cargo insta` snapshots are only re-blessed by an
  explicit `cargo insta review`; never auto-accept.
- Adding the `SimpleFixtureConfig` root type is additive and lives in the
  existing `orthohelp_fixture` crate; no `Cargo.toml`/`members` change is needed.
  If a `--root-type` fails to resolve, the most common cause is a missing or
  mistyped fully-qualified path (`orthohelp_fixture::SimpleFixtureConfig`) or a
  missing Fluent message key; cross-check against how `NestedFixtureConfig` is
  declared and localized.
- If a milestone's gates fail, fix forward within tolerances; if blocked, the
  work is committed per milestone so `git` provides clean rollback points.
- Leave `/tmp` logs in place for review; they do not pollute the work tree.

## Artifacts and notes

Recommended new/changed files (final shape may refine during implementation):

- `ortho_config/src/agent_context/tests.rs` — extended guards (Milestone 1).
- `cargo-orthohelp/src/agent_context/proptests.rs` — determinism property.
- `cargo-orthohelp/src/agent_context/tests.rs` — nested structural + default-
  display normalization tests.
- `cargo-orthohelp/src/agent_context/mod.rs` — default-display normalization.
- `tests/fixtures/orthohelp_fixture/src/lib.rs` — add the minimal
  `SimpleFixtureConfig` root type (no new crate; `FixtureConfig` and
  `NestedFixtureConfig` already exist).
- `tests/fixtures/orthohelp_fixture/locales/*/messages.ftl` — Fluent keys for
  the new `SimpleFixtureConfig`.
- `cargo-orthohelp/tests/golden/agent_context_tests.rs` — parametrized over the
  three roots via `--root-type`.
- `cargo-orthohelp/tests/golden/agent_context__{simple,enum,nested}.json.snap`.
- `cargo-orthohelp/tests/features/orthohelp_agent_context.feature` + steps —
  nested scenario.
- `cargo-orthohelp/src/main.rs` (+ `all` coverage) — Milestone 4 (if D3).
- `docs/agent-native-cli-design.md`, `docs/adr-003-...md`, `docs/users-guide.md`,
  `docs/developers-guide.md`, `docs/cargo-orthohelp-design.md` (if applicable),
  `docs/roadmap.md`.

## Interfaces and dependencies

No new public API is required. The schema types and
`ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` are already public from 6.2.1; this item
adds tests, one test-only fixture root type inside the existing
`orthohelp_fixture` crate (`publish = false`), and documentation, plus (if D3)
an additive change to `--format all` behaviour. No new workspace member is
added. The generator public surface stays:

```rust
// cargo-orthohelp/src/agent_context/mod.rs
pub fn bridge_ir_to_agent_context(
    meta: &DocMetadata,
    package: &str,
    localizer: Option<&dyn Localizer>,
) -> AgentContext;
```

`ortho_config` gains no new runtime dependency (Tolerance 1). Any default-
display normalization is internal to the generator path. No new fixture crate is
added; the new `SimpleFixtureConfig` root reuses the existing
`orthohelp_fixture` crate's dependencies (`ortho_config` + derive, `clap`,
`serde`).

## Test matrix (layer → concern → type)

- `ortho_config` unit (`rstest`, `insta`, `pretty_assertions`, `googletest`):
  version/kind pin and docs-IR independence; comprehensive wire snapshot;
  variant-exhaustive enum wire strings; missing-required rejection; unknown-key
  tolerance.
- `cargo-orthohelp` unit (`rstest`): path construction, enum mapping, hidden-
  field skip, and nested structural assertions over a nested `DocMetadata`
  (reusing 6.1.2's `define_nested_fixture!` helper or a hand-built tree);
  default-display normalization.
- `cargo-orthohelp` property (`proptest`): unique paths, command/input sort,
  hidden-field omission (existing) + transform determinism (new).
- `cargo-orthohelp` integration golden (`rstest` parametrized, shell-out):
  one end-to-end smoke per root (simple `SimpleFixtureConfig` / enum
  `FixtureConfig` / nested `NestedFixtureConfig`), selected via `--root-type`,
  comparing committed `.snap`.
- `cargo-orthohelp` BDD (`rstest-bdd`): agent-context emitted (existing) +
  nested-depth scenario (new).

## Signposted skills and documentation

Skills to load during implementation: `rust-router` first, then
`rust-unit-testing` (assertion/fixture shape, `serial_test` if needed),
`proptest` (determinism property), `nextest` (running/filtering tests),
`arch-crate-design` (fixture root-type boundaries), `arch-decision-records` (the
ADR-003 cross-reference judgement), `leta` (navigation/refactor), and
`execplans` (keeping this document current). Use `rstest-bdd` guidance for the
behavioural scenario. Study how 6.1.2 (PR #340) wired `NestedFixtureConfig` and
its `--root-type` selection before adding `SimpleFixtureConfig`.

Documentation to consult: `docs/agent-native-cli-design.md` (§3.2, §8, §8.1),
`docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`,
`docs/cargo-orthohelp-design.md`, `docs/developers-guide.md`,
`docs/users-guide.md`, `docs/rust-testing-with-rstest-fixtures.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/localizable-rust-libraries-with-fluent.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`, and
`docs/rstest-bdd-users-guide.md`.

## Outcomes & retrospective

To be completed at milestone boundaries and on completion: compare the result
against `Purpose / big picture`, record what running the agent-context transform
over 6.1.2's `NestedFixtureConfig` surfaced about the `walk`/`command_path`
recursion (Risk 1), note any approved decision that later changed and its
impact, and capture lessons for 6.2.3 (downstream `context --json` naming) and
6.3 (skill manifests), which build on this locked schema.

## Revision note

- 2026-06-15 — Revised after roadmap 6.1.2 ("Nested command tree behavioural
  fixtures", PR #340, commit `5073b6d`) merged to `main` and was picked up by a
  rebase of this branch.
  - What changed: the plan no longer creates three new fixture crates
    (`orthohelp_simple_fixture`, `orthohelp_enum_fixture`,
    `orthohelp_nested_fixture`). It now reuses the multi-root pattern 6.1.2
    established inside the existing `orthohelp_fixture` crate — reusing
    `NestedFixtureConfig` (nested) and `FixtureConfig` (enum, the default root),
    and adding only a minimal `SimpleFixtureConfig` root for the simple case,
    each golden selected via `--root-type`. Affected sections: Purpose,
    Risks (Risk 1 narrowed, Risk 4 reworded), Progress, Surprises, Decision Log
    (old fixture decision superseded), Context and orientation, Milestone 2
    (reuse `define_nested_fixture!`), Milestone 3 (root types not crates, plus a
    6.2.2-vs-6.1.2 render-assertion fence), Concrete steps, Validation,
    Idempotence, Artifacts, Interfaces, Test matrix, and Signposted docs.
  - Why: 6.1.2 shipped a reusable real nested tree and the `--root-type`
    convention, so authoring parallel crates would duplicate it and add build
    cost; reuse is cheaper and matches the now-canonical pattern.
  - Effect on remaining work: net scope is reduced (one new struct + locale keys
    instead of three crates). Risk 1 is narrowed but not eliminated — 6.2.2 is
    still the first to exercise the agent-context transform over a nested tree,
    so the in-process structural Red gate (Milestone 2) remains mandatory. The
    `Approved decisions` (D1-D5) were later resolved on 2026-06-24: D1, D2, D3,
    and D5 were accepted as recommended, while D4 now standardizes all enum wire
    strings to `snake_case` before locking.
