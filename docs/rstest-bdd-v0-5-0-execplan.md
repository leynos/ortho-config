# Migrate behavioural tests to `rstest-bdd` v0.5.0

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

`PLANS.md` is not present in this repository at the time of drafting, so this
document is the canonical execution plan for this migration.

## Purpose / big picture

After this migration, the behavioural suite in `examples/hello_world` will run
on `rstest-bdd` v0.5.0 and use the newer APIs to reduce manual scenario
binding boilerplate, improve compile-time validation of step coverage, and make
step arguments more explicit and type-safe. Success is observable when:

- the suite still validates the same user-visible behaviours as today;
- new coverage additions run under `cargo test` without bespoke runners;
- `make check-fmt`, `make lint`, and `make test` all pass.

## Constraints

- Preserve user-visible behavioural coverage in
  `examples/hello_world/tests/features/global_parameters.feature` and
  `examples/hello_world/tests/features/rstest_bdd_canary.feature`.
- Keep the default developer workflow command surface unchanged:
  `make check-fmt`, `make lint`, and `make test`.
- Keep YAML behavioural scenarios gated so builds without the `yaml` feature do
  not attempt to execute `@requires.yaml` scenarios.
- Do not add new external crates unless the migration cannot be completed with
  existing workspace dependencies and `rstest-bdd` v0.5.0 capabilities.
- Keep documentation consistent with actual test usage and strategy.

If satisfying the objective requires violating a constraint, stop and escalate.

## Tolerances (exception triggers)

- Scope: if migration requires touching more than 16 files or introduces more
  than 900 net lines, stop and escalate with a narrowed slice.
- Interface: if `ortho_config` or `hello_world` public runtime APIs must change
  to satisfy test migration, stop and escalate.
- Dependencies: if migration requires any dependency beyond updating
  `rstest-bdd` and `rstest-bdd-macros`, stop and escalate.
- Iterations: if quality gates fail after 3 full fix attempts, stop and
  escalate with failure logs.
- Time: if any single stage below exceeds 2 hours of active implementation,
  stop and escalate with a reduced milestone proposal.
- Ambiguity: if `scenarios!` cannot target the required feature subset without
  unsafe workarounds, stop and present restructuring options.

## Risks

- Risk: `scenarios!` autodiscovery may require feature file reshaping to avoid
  binding the canary scenario to the wrong fixture set.
  Severity: medium
  Likelihood: medium
  Mitigation: isolate behavioural feature files into a dedicated directory or
  dedicated `scenarios!` module with explicit fixture mapping.

- Risk: stricter compile-time validation may surface latent step mismatches.
  Severity: medium
  Likelihood: medium
  Mitigation: enable validation in stages, fix diagnostics immediately, and
  keep scenario/step naming exact.

- Risk: typed argument migration can overfit step patterns and reduce
  readability if done indiscriminately.
  Severity: low
  Likelihood: medium
  Mitigation: apply `StepArgs` only to multi-placeholder steps where it
  improves clarity (for example environment key/value and file/value pairs).

- Risk: behavioural documentation drift across `docs/` files.
  Severity: low
  Likelihood: medium
  Mitigation: update `docs/developers-guide.md` in the same change and verify
  consistency against `examples/hello_world/tests/rstest_bdd`.

## Progress

- [x] (2026-02-08 17:22Z) Reviewed repository constraints, migration guides, and
  current behavioural suite structure.
- [x] (2026-02-08 17:22Z) Audited current `rstest-bdd` usage and identified
  high-boilerplate areas (`behaviour/scenarios.rs`, multi-arg steps).
- [x] (2026-02-08 17:22Z) Drafted this ExecPlan with concrete stages and
  acceptance criteria.
- [ ] Implement dependency upgrades and scenario binding refactor.
- [ ] Implement typed step argument and coverage improvements.
- [ ] Update behavioural strategy documentation and run quality gates.

## Surprises & Discoveries

- Observation: `docs/developers-guide.md` does not exist yet.
  Evidence: `sed -n '1,260p' docs/developers-guide.md` returned
  "No such file or directory".
  Impact: migration work must create this file and establish it as the
  behavioural testing strategy reference.

- Observation: workspace is currently pinned to `rstest-bdd = "0.3.2"` and
  `rstest-bdd-macros = "0.3.2"`.
  Evidence: `Cargo.toml` `[workspace.dependencies]`.
  Impact: migration includes explicit dependency bump and compatibility fixes.

- Observation: no `PLANS.md` is present.
  Evidence: `test -f PLANS.md` returned false.
  Impact: this ExecPlan must be self-sufficient for execution.

- Observation: project memory helpers `qdrant-find`/`qdrant-store` are not
  available in this environment.
  Evidence: `command -v qdrant-find` and `command -v qdrant-store` returned no
  path.
  Impact: memory retrieval/storage steps cannot run; decisions are captured
  directly in this plan.

## Decision Log

- Decision: use `rstest_bdd_macros::scenarios!` as the default scenario binding
  mechanism for the behavioural suite.
  Rationale: it removes manual per-scenario boilerplate and allows compile-time
  fixture injection and tag filtering at generation time.
  Date/Author: 2026-02-08 17:22Z / Codex

- Decision: preserve a dedicated canary scenario path if fixture requirements
  differ from the main behavioural harness.
  Rationale: forcing heterogeneous fixtures into one autodiscovery call would
  reduce clarity and increase coupling.
  Date/Author: 2026-02-08 17:22Z / Codex

- Decision: track testing strategy updates in a new
  `docs/developers-guide.md`.
  Rationale: this path is explicitly required and currently missing.
  Date/Author: 2026-02-08 17:22Z / Codex

## Outcomes & Retrospective

Draft complete. Implementation has not started yet, so no behavioural outcomes
are recorded in this revision.

Expected completion outcome:

- behavioural scenarios are generated with less glue code;
- step argument handling is clearer and more type-safe;
- documentation reflects the active strategy;
- all quality gates pass.

## Context and orientation

The behavioural suite lives in `examples/hello_world/tests/rstest_bdd`. The
key files are:

- `examples/hello_world/tests/rstest_bdd/behaviour/scenarios.rs`: currently
  binds each scenario via a local `macro_rules!` helper.
- `examples/hello_world/tests/rstest_bdd/behaviour/steps/global.rs`: primary
  Given/When/Then implementations for CLI execution, environment handling, file
  setup, and merge assertions.
- `examples/hello_world/tests/features/global_parameters.feature`: main
  behavioural specification with 23 scenarios.
- `examples/hello_world/tests/features/rstest_bdd_canary.feature` and
  `examples/hello_world/tests/rstest_bdd/canary.rs`: minimal canary path with
  separate fixture state.
- `Cargo.toml` and `examples/hello_world/Cargo.toml`: dependency and
  test-related configuration.

The migration context documents are:

- `docs/rstest-bdd-v0-5-0-migration-guide.md`
- `docs/rstest-bdd-users-guide.md`

## Plan of work

### Stage A: Baseline and dependency migration prep

Update workspace test dependencies to `rstest-bdd` v0.5.0 and
`rstest-bdd-macros` v0.5.0 in `Cargo.toml`, then refresh lockfile metadata via
Cargo. Keep behavioural code unchanged in this stage to isolate dependency
errors from refactors.

Validation gate:

- run a focused behavioural test compile/execution command for `hello_world`;
- if compile errors arise, classify by API change and map each to affected
  files before editing.

Go/no-go:

- proceed only when dependency update compiles or all compile failures are
  understood and scoped.

### Stage B: Scenario binding boilerplate reduction

Refactor `examples/hello_world/tests/rstest_bdd/behaviour/scenarios.rs` to use
`scenarios!` autodiscovery with fixture injection, replacing manual
`hello_world_scenario!` invocations. Keep YAML-tagged scenarios feature-gated
through compile-time tags and `#[cfg(feature = "yaml")]` where needed.

If autodiscovery cannot cleanly separate canary and behavioural fixtures,
reorganize feature file directories to keep fixture sets explicit and avoid
cross-binding.

Validation gate:

- confirm generated tests still cover all existing behavioural scenario names;
- confirm non-`yaml` builds exclude `@requires.yaml` scenarios.

Go/no-go:

- proceed only when scenario generation is deterministic and lint-clean.

### Stage C: Type safety and clarity upgrades in step definitions

Adopt v0.5.0 step ergonomics where they provide measurable clarity:

- introduce `#[derive(StepArgs)]` structs for multi-placeholder steps that
  currently pass several `String` values positionally;
- where structured table input improves clarity, introduce typed table parsing
  with `Rows<T>` and `#[derive(DataTableRow)]` instead of hand-parsed
  collections;
- keep scenario signatures explicit (`Result<(), E>` or `StepResult<(), E>`)
  and avoid alias-based ambiguity.

Add or refactor behavioural cases to expand coverage using concise Gherkin
patterns (for example scenario outlines or table-driven variants) without
duplicating step glue.

Validation gate:

- all updated steps compile without `clippy::needless_pass_by_value` suppression
  unless captures genuinely require ownership;
- feature files remain readable and behaviour-focused.

Go/no-go:

- proceed only when new/updated scenarios are understandable by non-authors and
  the step API is simpler than before.

### Stage D: Compile-time validation and documentation

Enable compile-time step validation for `rstest-bdd-macros` in
`examples/hello_world/Cargo.toml` (and any other relevant crate manifests) so
missing steps fail early. Select strictness level based on whether step
definitions are local-only.

Create/update `docs/developers-guide.md` to document:

- the behavioural test layout and ownership;
- approved usage of `scenarios!`, `StepArgs`, and typed table arguments;
- how to add new scenarios while keeping fixture isolation.

Validation gate:

- run full repository quality gates.

Go/no-go:

- migration is complete only when quality gates and documentation checks pass.

## Concrete steps

Run commands from repository root (`/home/user/project`).

1. Capture baseline and dependency impact:

       set -o pipefail
       cargo tree -p hello_world -i rstest-bdd 2>&1 | tee /tmp/rstest-bdd-tree-before.log
       cargo test -p hello_world rstest_bdd 2>&1 | tee /tmp/rstest-bdd-before.log

2. Apply dependency and test harness refactors (Stages A-C).

3. Validate behavioural suite directly:

       set -o pipefail
       cargo test -p hello_world rstest_bdd 2>&1 | tee /tmp/rstest-bdd-after.log

4. Run mandatory quality gates:

       set -o pipefail
       make check-fmt 2>&1 | tee /tmp/make-check-fmt.log
       make lint 2>&1 | tee /tmp/make-lint.log
       make test 2>&1 | tee /tmp/make-test.log

Expected success transcript tail:

       Finished `test` profile ...
       test result: ok. ... passed; 0 failed

       ... cargo fmt --check exits 0 ...
       ... cargo clippy ... -D warnings exits 0 ...

## Validation and acceptance

Acceptance is behaviour-first:

- Existing global-parameter scenarios still pass, including YAML-tagged paths
  when `yaml` is enabled.
- Scenario registration no longer requires one Rust function per feature
  scenario in `behaviour/scenarios.rs`.
- Step argument handling is more explicit and less positional where updated.
- `docs/developers-guide.md` explains active behavioural testing strategy and
  migration-era conventions.

Quality criteria:

- Tests: `make test` passes.
- Lint/typecheck/docs: `make lint` passes (includes docs build warnings denied
  and clippy warnings denied).
- Formatting: `make check-fmt` passes.

Quality method:

- run commands listed in `Concrete steps`, inspect generated logs under `/tmp`.

## Idempotence and recovery

This migration is designed to be re-runnable:

- dependency/version edits can be re-applied safely via Cargo manifests;
- `scenarios!` refactors are source-level and do not mutate external state;
- behavioural tests use per-scenario temporary directories and do not persist
  environment mutations across runs.

Recovery path if a stage fails:

- revert only the stage-specific files;
- rerun the stage validation command before proceeding.

## Artifacts and notes

Baseline artefacts captured during planning:

- `examples/hello_world/tests/features/global_parameters.feature` contains 23
  scenarios.
- `examples/hello_world/tests/features/rstest_bdd_canary.feature` contains 1
  scenario.
- Manual scenario bindings currently exist in
  `examples/hello_world/tests/rstest_bdd/behaviour/scenarios.rs`.

## Interfaces and dependencies

Migration target interfaces and dependencies:

- `Cargo.toml` workspace dependency versions:
  `rstest-bdd = "0.5.0"` and `rstest-bdd-macros = "0.5.0"`.
- `examples/hello_world/tests/rstest_bdd/behaviour/scenarios.rs` should expose
  `scenarios!`-generated tests with explicit fixtures (for example
  `hello_world_harness: Harness`) and tag filters.
- Step modules should continue to use `rstest_bdd_macros::{given, when, then}`
  and may add `StepArgs`/`DataTableRow` derives where chosen.
- Scenario functions must return `()` or explicit unit `Result`/`StepResult`
  forms only.

Revision note (2026-02-08 17:22Z):

Initial draft created from current repository state, migration guide context,
and behavioural test audit. No implementation work has started yet.
