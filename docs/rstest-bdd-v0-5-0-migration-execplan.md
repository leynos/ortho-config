# Migrate behavioural suites to `rstest-bdd` v0.5.0

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

No `PLANS.md` file exists in this repository, so this document follows the
default ExecPlan workflow from the `execplans` skill.

## Purpose / big picture

Upgrade the workspace behavioural suites from `rstest-bdd` `0.3.2` to `0.5.0`
while improving test quality and maintainability. After completion,
contributors can run the full behavioural suite with the standard `make test`
flow, use v0.5.0 features to reduce boilerplate, and rely on clearer, type-safe
step and scenario signatures.

Observable success:

- Workspace dependencies point to `rstest-bdd = "0.5.0"` and
  `rstest-bdd-macros = "0.5.0"`.
- Behavioural tests in `ortho_config` and `examples/hello_world` pass under
  `make test`.
- Repeated scenario boilerplate in
  `examples/hello_world/tests/rstest_bdd/behaviour/scenarios.rs` is replaced by
  `scenarios!(..., fixtures = [...], tags = ...)`.
- Scenario signatures avoid aliases and use explicit `Result<(), E>` or
  `rstest_bdd::StepResult<(), E>` where fallible.

## Constraints

- Preserve behavioural intent of existing `.feature` files in:
  `ortho_config/tests/features/` and `examples/hello_world/tests/features/`.
- Keep production APIs stable; avoid changes to public crate interfaces in
  `ortho_config/src/`, `ortho_config_macros/src/`, and
  `examples/hello_world/src/` unless required to keep existing tests green.
- Maintain scenario isolation defaults. Cross-scenario mutable state remains
  forbidden; only infrastructure can be shared via `#[once]`.
- Keep all required quality gates green:
  `make check-fmt`, `make lint`, and `make test`.
- Keep docs coherent with actual behaviour. Any strategy changes must be
  captured in `docs/developers-guide.md`.

If satisfying the objective requires violating a constraint, stop and escalate.

## Tolerances (exception triggers)

- Scope: if migration requires touching more than 45 files or 2,500 net LOC,
  stop and escalate for scope confirmation.
- Interface: if any public API or derive contract must change in
  `ortho_config` or `ortho_config_macros`, stop and escalate.
- Dependencies: if migration requires new third-party crates beyond upgrading
  `rstest-bdd`/`rstest-bdd-macros`, stop and escalate.
- Iterations: if the same compile/test failure persists after 3 focused fix
  attempts, stop and escalate with options.
- Time: if any milestone exceeds 4 hours elapsed work, stop and escalate with
  a reduced-scope option.
- Ambiguity: if tag filtering semantics or fixture injection behaviour in
  v0.5.0 produce multiple valid interpretations with different outcomes, stop
  and request direction.

## Risks

- Risk: Scenario return type alias detection regresses (`anyhow::Result<()>` in
  scenario signatures is not classified as fallible in v0.5.0). Severity: high
  Likelihood: high Mitigation: change scenario signatures to explicit
  `Result<(), anyhow::Error>` or `rstest_bdd::StepResult<(), anyhow::Error>`
  before deeper refactors.

- Risk: `scenarios!` fixture injection migration changes test naming and filter
  patterns used by developers. Severity: medium Likelihood: medium Mitigation:
  document new generated test naming and keep one canary `#[scenario]` target
  per crate until confidence is established.

- Risk: Async step adoption may trigger nested runtime errors if steps create
  new runtimes inside async scenarios. Severity: medium Likelihood: low
  Mitigation: use sync steps by default; if async is introduced, use
  `runtime = "tokio-current-thread"` on generated scenarios and avoid per-step
  runtime construction.

- Risk: Broad behavioural coverage hides regressions behind long `make test`
  runs. Severity: medium Likelihood: medium Mitigation: run targeted suites
  (`cargo test -p <crate> --test rstest_bdd`) between milestones before full
  workspace gates.

## Progress

- [x] (2026-02-08 17:48Z) Baseline captured: workspace currently pins
  `rstest-bdd` and `rstest-bdd-macros` to `0.3.2`.
- [x] (2026-02-08 17:48Z) Baseline behavioural inventory captured:
  24 feature files and 104 step definitions across both suites.
- [x] (2026-02-08 17:48Z) Migration hotspots identified from docs and code
  (alias return types, scenario boilerplate, fixture injection opportunities).
- [ ] Approval gate pending: receive explicit user approval for this plan
  before implementation.
- [ ] Upgrade dependencies and resolve compile-time migration errors.
- [ ] Refactor scenario bindings and step signatures to use v0.5.0 features.
- [ ] Expand behavioural coverage where v0.5.0 features reduce friction.
- [ ] Run and record full quality gates.

## Surprises & discoveries

- Observation: The project memory CLI (`qdrant-find`) is unavailable in this
  environment (`command not found`), so historical notes could not be queried.
  Evidence: shell output from `qdrant-find "project overview architecture"`.
  Impact: migration work must rely on repository docs and current code only.

- Observation: `docs/developers-guide.md` does not yet exist.
  Evidence: `rg --files | rg 'developers-guide\\.md$'` returned no matches.
  Impact: migration guidance must be authored from scratch and linked to
  existing testing docs.

- Observation: `examples/hello_world` behavioural scenario functions currently
  return `anyhow::Result<()>` aliases. Evidence:
  `examples/hello_world/tests/rstest_bdd/behaviour/scenarios.rs`. Impact: these
  signatures will fail v0.5.0 classification and need explicit return types.

## Decision log

- Decision: Use staged migration with a compile-first upgrade, then
  boilerplate-reduction refactors, then coverage expansion. Rationale: isolates
  unavoidable version breakages from optional improvements, reducing debugging
  surface. Date/Author: 2026-02-08 / Codex

- Decision: Prioritize `scenarios!`-driven fixture injection in
  `examples/hello_world` as the primary boilerplate reduction target.
  Rationale: this file currently carries repetitive macro-generated
  `#[scenario]` wrappers that v0.5.0 can replace directly. Date/Author:
  2026-02-08 / Codex

- Decision: Keep per-scenario fixture isolation as the default and treat
  `#[once]` as infrastructure-only. Rationale: aligns with the v0.5.0 migration
  guide and current suite design. Date/Author: 2026-02-08 / Codex

## Outcomes & retrospective

Pending implementation. Expected outcomes:

- Smaller, clearer scenario binding modules.
- Explicit, type-safe scenario and step signatures under v0.5.0.
- Behavioural coverage maintained or expanded without introducing shared mutable
  cross-scenario state.

Retrospective will be completed after implementation and validation.

## Context and orientation

The behavioural suite already uses `rstest-bdd`, but on `0.3.2`:

- Version pins live in `Cargo.toml` under `[workspace.dependencies]`.
- `ortho_config` suite root: `ortho_config/tests/rstest_bdd/`.
- `hello_world` suite root: `examples/hello_world/tests/rstest_bdd/`.
- Migration references:
  `docs/rstest-bdd-v0-5-0-migration-guide.md` and
  `docs/rstest-bdd-users-guide.md`.

Current binding patterns:

- `ortho_config` primarily uses `scenarios!` per feature file in
  `ortho_config/tests/rstest_bdd/behaviour/scenarios.rs`.
- `hello_world` currently uses many hand-written `#[scenario]` wrappers in
  `examples/hello_world/tests/rstest_bdd/behaviour/scenarios.rs`, including
  YAML-tag-gated variants.

Current scale baseline:

- 104 step definitions (`#[given]`/`#[when]`/`#[then]`).
- 24 feature files across both crates.

## Plan of work

Stage A: Baseline and migration preflight (no behavioural changes yet).

Update dependency versions to `0.5.0`, run targeted compile/test commands, and
catalogue breakages by file and error class. Fix mandatory v0.5.0 issues first:
scenario return types, import path changes, and any macro argument changes. Do
not refactor for style in this stage.

Go/no-go: proceed only when both crate-level `rstest_bdd` integration test
targets compile and run.

Stage B: Boilerplate reduction and clarity improvements.

Refactor scenario binding modules to leverage v0.5.0 macro features. Replace
repetitive hand-written wrappers in
`examples/hello_world/tests/rstest_bdd/behaviour/scenarios.rs` with
`scenarios!` invocations that apply tag filters and shared fixtures explicitly.
Keep generated tests readable and deterministic by grouping invocations by
feature and tag intent.

Go/no-go: proceed only when targeted behavioural suites still pass and
generated test names remain easy to filter by feature.

Stage C: Type-safety improvements in step contracts.

Introduce `StepArgs` for steps with multiple placeholder captures where this
improves readability and eliminates ad hoc parsing. Normalize fallible step and
scenario signatures to explicit `Result<(), E>` or
`rstest_bdd::StepResult<(), E>`. Use step return-value injection where it
reduces mutable fixture plumbing without obscuring behaviour.

Go/no-go: proceed only when clippy runs without new suppressions and step
signatures are more explicit than baseline.

Stage D: Coverage expansion and documentation alignment.

Add or refine behavioural cases that were previously awkward due boilerplate,
prioritizing scenario outlines and tag-filtered coverage in existing feature
files. Record final conventions and migration rationale in
`docs/developers-guide.md`, keeping guidance consistent with the two rstest-bdd
reference docs.

Go/no-go: proceed to completion only after required quality gates pass.

## Concrete steps

All commands run from repository root (`/home/user/project`).

1. Baseline and dependency update.

    rg -n "rstest-bdd" Cargo.toml

    update `Cargo.toml` workspace dependencies to `0.5.0`

    cargo update -p rstest-bdd -p rstest-bdd-macros

Expected signal:

    Cargo.lock updates include rstest-bdd 0.5.0 packages.

1. Targeted behavioural verification while fixing migration errors.

    cargo test -p ortho_config --test rstest_bdd
    cargo test -p hello_world --test rstest_bdd --all-features

Expected signal:

    test result: ok. <N> passed; 0 failed

1. Full repository quality gates with logs.

    set -o pipefail; make check-fmt 2>&1 | tee /tmp/make-check-fmt.log
    set -o pipefail; make lint 2>&1 | tee /tmp/make-lint.log
    set -o pipefail; make test 2>&1 | tee /tmp/make-test.log

Expected signal:

    all three commands exit 0, with no lint warnings promoted to errors.

1. Documentation verification (when markdown changes are made).

    set -o pipefail; make markdownlint 2>&1 | tee /tmp/make-markdownlint.log
    set -o pipefail; make nixie 2>&1 | tee /tmp/make-nixie.log

Expected signal:

    markdown lint and Mermaid checks exit 0.

## Validation and acceptance

Acceptance is behavioural and observable:

- Behavioural tests remain executable from stock test harness in both crates.
- v0.5.0 migration requirements are satisfied:
  - no scenario returns a non-unit payload;
  - no scenario relies on alias return-type classification;
  - any sync-to-async wrappers use `rstest_bdd::async_step::sync_to_async`.
- Boilerplate in hello_world scenario bindings is materially reduced by using
  `scenarios!` fixture/tag support.
- Contributor guidance in `docs/developers-guide.md` matches the new approach.

Quality criteria:

- Tests: `make test` passes.
- Lint/type/doc: `make lint` passes.
- Formatting: `make check-fmt` passes.

Quality method:

- Run commands in `Concrete steps` and retain logs in `/tmp` for inspection.

## Idempotence and recovery

- All commands are safe to rerun.
- If a migration edit introduces failures, recover by reverting only the
  current file-level change and rerunning targeted
  `cargo test -p <crate> --test rstest_bdd` before continuing.
- Keep commits small by stage so rollback is limited to one concern.

## Artifacts and notes

Baseline evidence captured during planning:

    Cargo.toml currently pins:
    rstest-bdd = "0.3.2"
    rstest-bdd-macros = "0.3.2"

    Behavioural inventory:
    - 24 feature files
    - 104 step definitions

Notable file hotspots for migration edits:

- `Cargo.toml`
- `examples/hello_world/tests/rstest_bdd/behaviour/scenarios.rs`
- `examples/hello_world/tests/rstest_bdd/behaviour/steps/global.rs`
- `ortho_config/tests/rstest_bdd/behaviour/scenarios.rs`
- `docs/developers-guide.md`

## Interfaces and dependencies

Dependencies to target:

- `rstest-bdd = "0.5.0"`
- `rstest-bdd-macros = "0.5.0"`

Required interface usage after migration:

- Scenario signatures:

    fn scenario_name(…) -> Result<(), E>

  or:

    fn scenario_name(…) -> rstest_bdd::StepResult<(), E>

- Scenario auto-discovery with fixtures/tags:

    scenarios!(
        "tests/features",
        fixtures = [hello_world_harness: Harness],
        tags = "not @requires.yaml"
    );

- Step argument typing for multi-placeholder steps:

    #[derive(StepArgs)]
    struct Args { … }

- Sync-to-async wrapper path (only if wrappers remain):

    use rstest_bdd::async_step::sync_to_async;

## Revision note

2026-02-08: Initial draft created from repository baseline, migration docs, and
current behavioural suite structure. No implementation has started; approval
gate remains open.
