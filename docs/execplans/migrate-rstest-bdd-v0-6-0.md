# Migrate behavioural suites to `rstest-bdd` v0.6.0

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Upgrade the workspace's `rstest-bdd` family dependencies from the released
**0.5.0** to the published **0.6.0**, and apply every source change the upgrade
requires, while preserving the behaviour and coverage of the existing
behavioural suites.

After this change a contributor can run the repository's canonical gates and
observe:

- `Cargo.lock` resolves `rstest-bdd = 0.6.0` and
  `rstest-bdd-macros = 0.6.0` with no remaining `0.5.0` packages.
- `make test` passes with the same set of `rstest-bdd` scenarios executing as
  before the change (baseline: 127 `Scenario` blocks across 28 feature files, no
  `Scenario Outline` blocks).
- `make check-fmt`, `make lint` and `make markdownlint` pass on the final
  tree.
- A provenance document records exactly which upstream documentation was
  imported, from which immutable commit, and with which checksums.

This is a migration of existing consumers. It does not introduce BDD to a
repository that lacks it, and it does not expand into a general test-framework
or application redesign.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not workarounds.

- **Preserve behavioural coverage.** No `.feature` file may be deleted or have
  `Scenario` blocks removed to make the migration compile. Scenario count after
  migration must be greater than or equal to the baseline of 127.
- **Preserve production interfaces.** No public API in `ortho_config/src/`,
  `ortho_config_macros/src/` or `examples/hello_world/src/` may change except
  where a change is strictly required to keep existing tests compiling. The BDD
  migration is a test-surface change.
- **Preserve intentional features.** Where the repository enables
  `compile-time-validation`, `strict-compile-time-validation`, or
  `default-features = false`, those settings must survive unchanged. They must
  not be relaxed to make the migration compile.
- **Do not introduce compatibility machinery.** No old/new dual-version
  branches, feature-gated compatibility shims, facade types or migration
  wrappers solely to preserve the pre-1.0 `rstest-bdd` API. Pre-1.0 test-only
  APIs are updated in place.
- **Do not copy upstream workspace paths or vendor shims.** No `[patch]`
  tables, no `path = "crates/..."` dependency substitutions, no upstream GPUI
  shim.
- **Do not adopt later-release APIs.** The imported upstream guide text
  contains sections labelled "v0.6.1 beta" and "v0.7.0". Those are not in
  scope. In particular the v0.6-compatible
  `#[from(rstest_bdd_harness_context)]` spelling is retained where it applies,
  and no guard-based borrowing or v0.7 lifecycle guarantees may be assumed.
- **Imported upstream documents are byte-for-byte intact.** Repository-specific
  migration notes live outside the upstream copies.
- **Do not clobber `docs/v0-6-0-migration-guide.md`.** That path already holds
  this repository's own v0.5.0→v0.6.0 migration guide. The imported upstream
  document takes the `rstest-bdd-` prefixed name.
- **Preserve unrelated work.** Any uncommitted changes in the working tree are
  preserved.
- **Respect the compile-admission pool.** Never unset or override
  `RUSTC_WRAPPER`/`RUSTDOC`, never invoke `rustc` directly, and never start a
  second build to "help" a queued one.

If satisfying the objective requires violating a constraint, stop and escalate.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached.

- Scope: if the migration requires changes to more than 40 tracked files, or
  more than 1,200 net lines of code, stop and escalate.
- Interface: if any public production API in the three production crates must
  change signature, stop and escalate.
- Dependencies: if the migration requires a new third-party runtime dependency
  beyond the `rstest-bdd` family and `tokio` (only if a harness is adopted),
  stop and escalate.
- MSRV: if the required compiler floor rises above the repository's declared
  `rust-version = "1.89.0"`, stop and escalate. (Survey found upstream MSRV is
  1.88, so this is not expected to trigger.)
- Iterations: if the same compile or test failure persists after three focused
  fix attempts, stop and escalate with options.
- Time: if a single milestone exceeds four hours of elapsed work, stop and
  escalate with a reduced-scope option.
- Ambiguity: if a behaviour change is observable in scenario outcomes and two
  valid interpretations produce different acceptance results, stop and present
  both with trade-offs.

## Risks

Known uncertainties identified upfront, with mitigations.

- Risk: the step-return classification change (ADR-019) alters runtime
  behaviour for the 175 `-> Result<()>` steps, whose `Ok(())` payload now
  becomes eligible to override a matching `()`. This is benign only while at
  most one `Slot<()>` is in scope per scenario. Severity: medium Likelihood:
  medium Mitigation: enumerate every `Slot<()>` fixture and the scenarios that
  bind them; run the behavioural suite and compare per-scenario outcomes, not
  merely the aggregate pass count.
- Risk: `scenarios!` generated test names, and therefore the nextest
  `binary(rstest_bdd)` filter and the `rstest_bdd` serial test-group in
  `.config/nextest.toml`, may change shape. Severity: low Likelihood: low
  Mitigation: the filter keys on the target name, which did not change; verify
  the serial group still matches by running the suite with nextest.
- Risk: feature paths becoming manifest-relative may invalidate any expected
  output that embeds a path. Severity: low Likelihood: low Mitigation: the
  survey found no reporter, JUnit, JSON or `cargo bdd` consumers in this
  repository; confirm by re-grepping after the upgrade.
- Risk: upgrading drags in transitive churn (`gherkin` 0.16, `derive_more`
  2.1, `thiserror` 2.0, `hashbrown` 0.17, `convert_case` 0.12, `syn` 3). Some
  of these already appear in `Cargo.lock` at a second version. Severity: medium
  Likelihood: medium Mitigation: use targeted `cargo update --precise` rather
  than a full graph update, and review the lockfile diff for unrelated churn
  before committing.
- Risk: the compile pool is shared with other agents and is currently
  saturated, so gate runs may take far longer than usual. Severity: low
  Likelihood: high Mitigation: run gates sequentially, log to
  `/tmp/$ACTION-$(get-project)-$(git branch --show-current).out`, and treat a
  queued build as normal rather than as a failure.

## Progress

Use a list with checkboxes to summarize granular steps. Every stopping point
must be documented here, even if it requires splitting a partially completed
task into two.

- [x] (2026-10-14) Reconnaissance: branch, manifests, lockfiles, toolchain,
  CI, feature-file inventory, step-return survey.
- [x] (2026-10-14) Resolve upstream `v0.6.0` tag to an immutable commit and
  compare with the expected SHA.
- [x] (2026-10-14) Read the upstream v0.6.0 migration guide and the 0.5.0
  migration guide; build the applicability matrix.
- [ ] Import upstream documentation with a provenance record.
- [ ] Write the ExecPlan and obtain approval.
- [ ] Milestone EP-M1: dependency and lockfile upgrade to 0.6.0.
- [ ] Milestone EP-M2: source and behavioural changes.
- [ ] Milestone EP-M3: documentation and gates.

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as risks.

- Observation: `docs/v0-6-0-migration-guide.md` already exists in this
  repository and is **ortho-config's own** v0.5.0→v0.6.0 application migration
  guide, not the `rstest-bdd` document of the same upstream filename. Evidence:
  its heading reads "Migration guide: v0.5.0 to v0.6.0" and it describes
  `ortho-config` crate changes. Impact: the task's default destination path for
  the imported `rstest-bdd` document would have destroyed an unrelated
  document. The prefixed name `docs/rstest-bdd-v0-6-0-migration-guide.md` is
  used instead, matching the existing convention already set by
  `docs/rstest-bdd-users-guide.md` and
  `docs/rstest-bdd-v0-5-0-migration-guide.md`.
- Observation: the upstream `v0.6.0` tag resolves to
  `72fb22635670e456545ca368805ba4c1c9d7bd69`, exactly the SHA named in the
  task. Evidence: `git rev-parse 'refs/tags/v0.6.0^{commit}'`. Impact: no
  resolution discrepancy to investigate; the pinned commit is authoritative.
- Observation: `docs/rstest-bdd-users-guide.md` already exists locally at 1919
  lines but is an **older** revision of the upstream guide (the upstream v0.6.0
  revision is 3291 lines). It is a prior migration's import, not user-authored
  content. Impact: re-importing from the pinned commit is correct and does not
  destroy repository-authored prose; the existing v0.5.0-era file is superseded.
- Observation: no `.feature` file uses `Scenario Outline`, and no step
  function returns a non-unit alias. Steps return only `Result<()>` (175) and
  `StepResult<()>` (59). Evidence: AST-free scan of every `#[given]`, `#[when]`
  and `#[then]` attribute and its following signature. Impact: the migration
  guide's headline "audit step return aliases" work is mostly inapplicable, but
  the `Result<T, E>` classification change still touches every `Result<()>`
  step and must be reasoned about explicitly.
- Observation: the risk that `Ok(())` from a `Result<()>` step would override
  a `Slot<()>` flag fixture is **closed by upstream design, not merely by
  absence of a matching fixture**. Evidence: in the published 0.6.0 source,
  `StepReturnResultTag::normalize` passes the `Ok` payload to
  `__rstest_bdd_payload_from_value`, which opens with
  `if TypeId::of::<T>() == TypeId::of::<()>() { None }` and therefore emits no
  override payload for any unit value. The repository does contain `Slot<()>`
  flag fields (`extends_flag`, `cyclic_flag`, `missing_base_flag`,
  `multi_level_flag`, `replace_strategy_flag`, `non_string_flag` on
  `ExtendsContext`), so this branch was reachable in principle. Impact: the
  step-return classification change is behaviourally inert for this repository,
  which contains no step returning a non-unit `Result<T, E>`. The V2 regression
  coverage remains worthwhile as a guard against a future step introducing a
  non-unit return, but is not needed to protect current behaviour.

## Decision log

Record every significant decision made while working on the plan.

- Decision: treat the compile-time-validation feature flags as
  cannot-relax, and confirm their current state before touching any manifest.
  Rationale: the task explicitly forbids relaxing them to make the migration
  compile, and the root manifest as surveyed does not visibly set them, so
  their location must be established rather than assumed. Date/Author:
  2026-10-14, migration agent.
- Decision: import the upstream migration guide as
  `docs/rstest-bdd-v0-6-0-migration-guide.md` rather than
  `docs/v0-6-0-migration-guide.md`. Rationale: the latter path is occupied by
  an unrelated repository-authored document (see Surprises). Date/Author:
  2026-10-14, migration agent.
- Decision: do not adopt `TokioHarness`, `StdHarness` or `GpuiHarness`.
  Rationale: the repository declares no `runtime = "tokio-current-thread"`
  scenarios, no custom `HarnessAdapter` implementations and no GPUI dependency.
  Harness adoption is explicitly optional in the guide, and the harness
  dependency matrix says a plain-BDD consumer needs only `rstest`, `rstest-bdd`
  and `rstest-bdd-macros`. Adding harness crates here would be unjustified
  scope. Date/Author: 2026-10-14, migration agent.
- Decision: no MSRV or toolchain-pin change is planned. Rationale: upstream
  MSRV is 1.88 and the repository declares and pins 1.89.0, so the floor is
  already satisfied. This is recorded as a decision because the task authorises
  a bump and a reader should know it was considered and found unnecessary
  rather than overlooked. Date/Author: 2026-10-14, migration agent.

## Outcomes & retrospective

Summarize outcomes, gaps, and lessons learned at major milestones or at
completion. To be completed as milestones land.

## Context and orientation

This repository is a Rust workspace (`resolver = "2"`, edition 2024) with ten
members declared in the root `Cargo.toml`. Only three members consume
`rstest-bdd`:

- `ortho_config` — the primary behavioural suite, 22 feature files.
- `cargo-orthohelp` — a cargo subcommand helper, 6 feature files.
- `examples/hello_world` — a worked example, 4 feature files.

The root `Cargo.toml` declares the family in `[workspace.dependencies]` as
`rstest-bdd = "0.5.0"` and `rstest-bdd-macros = "0.5.0"`. Each of the three
members inherits them with `.workspace = true` in `[dev-dependencies]`. The
single lockfile is `Cargo.lock` at the repository root; the fixture crates under
`tests/fixtures/` are workspace members but have no lockfile of their own and
no `rstest-bdd` dependency.

Each consuming crate has a `tests/rstest_bdd.rs` (or `tests/rstest_bdd/mod.rs`)
entrypoint that includes a `rstest_bdd_support` module tree. That tree holds
step definitions (files under `behaviour/steps/`), `ScenarioState` structs
(usually `scenario_state.rs`), rstest fixtures, and a `scenarios.rs` that wires
feature files to the registry with `scenarios!(..., fixtures = [...])`.
Scenario state types are declared with
`#[derive(Debug, Default, ScenarioState)]` from `rstest_bdd_macros`, and their
fields are `rstest_bdd::Slot<T>` values.

A term used throughout: a **fixture key** is the name under which a value is
inserted into the scenario's `StepContext`. A step parameter with no
`#[from(...)]` attribute requests the implicit fixture key derived from the
parameter name. In 0.5.0 and earlier that derivation did not strip a leading
underscore; in 0.6.0 it strips exactly one. An explicit `#[from(name)]` is
always exact and is unaffected.

The canonical gates are the `Makefile` targets. `make check-fmt` runs
`cargo fmt --all -- --check` followed by `mdtablefix --check`. `make lint` runs
`cargo doc`, `cargo clippy --all-targets --all-features -- -D warnings`, and
the Whitaker Dylint suite. `make test` runs
`RUSTFLAGS="-D warnings" cargo test --all-targets --all-features` followed by a
pytest suite over `scripts/`. `make markdownlint` runs `markdownlint-cli2` and
then the pinned `typos-config-builder` spelling gate.

Note a standing trap recorded in this repository's developer guidance:
`make lint` treats Clippy and Whitaker as sibling prerequisites, so a Clippy
failure aborts the target before Whitaker runs at all. A green `make lint`
therefore means both passed, but a red one may name only Clippy.

## Applicability matrix

Every breaking change listed in the pinned upstream
`docs/v0-6-0-migration-guide.md` is dispositioned here. "Inapplicable" entries
record the evidence, not an assumption.

| Breaking change                                                      | Status                   | Evidence                                                                                                                                                                                                                                                                                                                                                    |
| -------------------------------------------------------------------- | ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Implicit fixture names normalize one leading underscore              | Inapplicable             | A structural scan of every `#[given]`/`#[when]`/`#[then]`/`#[scenario]` parameter across all three suites found zero underscore-prefixed parameters without an explicit `#[from(...)]`. The only underscore fixture in the repository is `_env_lock` in `ortho_config/tests/discovery_attributes.rs`, a plain `#[rstest]` parameter, not a BDD fixture key. |
| `runtime = "tokio-current-thread"` becomes a deprecated alias        | Inapplicable             | No `runtime = ...` argument appears in any `scenarios!` invocation, and no async step or scenario exists.                                                                                                                                                                                                                                                   |
| Custom `HarnessAdapter` must return `HarnessResult<T>`               | Inapplicable             | No custom harness is declared; `rstest-bdd-harness` is not a dependency of any crate.                                                                                                                                                                                                                                                                       |
| Feature paths become manifest-relative                               | Inapplicable             | No consumer of `ScenarioMetadata::feature_path`, the JSON reporter, JUnit output, or `cargo bdd --dump-steps`. Only prose mentions of JUnit exist, in unrelated documents. One feature path (`tests/features/rstest_bdd_canary.feature`) is already manifest-relative and is bound by `#[scenario(path = ...)]`, whose argument form is unchanged.          |
| `StepContext::insert_value` returns `InsertOutcome`                  | Inapplicable             | No direct caller; the method is not referenced anywhere in the workspace.                                                                                                                                                                                                                                                                                   |
| Step return aliases of `Result<T, E>` propagate `Err` and inject `T` | **Applicable, reasoned** | 175 steps return `Result<()>`, which under ADR-019 is dispatched as `Result<T, E>` with `T = ()`, so `Ok(())` becomes an override candidate for fixtures of type `()`. Traced in decision D4 below.                                                                                                                                                         |
| `E` must implement `Display`                                         | Satisfied                | All 175 use `anyhow::Result<()>`, whose `E` is `anyhow::Error` and implements `Display`. No other error type is used by a step.                                                                                                                                                                                                                             |
| `record_bypassed_steps` takes `BypassedScenario`                     | Inapplicable             | No direct caller.                                                                                                                                                                                                                                                                                                                                           |
| Language-server indexing entry points return `RustStepIndexResult`   | Inapplicable             | `rstest-bdd-server` is not a dependency.                                                                                                                                                                                                                                                                                                                    |
| `publish_rust_diagnostics` removed                                   | Inapplicable             | Same as above.                                                                                                                                                                                                                                                                                                                                              |
| `find_feature_files` returns `Result<...>`                           | Inapplicable             | Same as above.                                                                                                                                                                                                                                                                                                                                              |
| MSRV rises to 1.88                                                   | Already satisfied        | The repository declares `rust-version = "1.89.0"` and pins `channel = "1.89"`; upstream requires 1.88.                                                                                                                                                                                                                                                      |

Additive features that are available but not adopted, with rationale:

- `harness = ...` and `attributes = ...` macro arguments: not adopted. No
  scenario needs a harness, and adopting one would add a dependency for no
  behavioural gain.
- Result-returning fixtures passed by value: not adopted. No current fixture
  is fallible in a way that needs propagation; the existing fixtures either
  return a value directly or construct `Result` values that are stored in
  scenario state and asserted on.
- GPUI stateful pattern: inapplicable, no GPUI dependency.

## Conformance basis

No Terms of Reference or technical-design revision governs this task. The
authoritative upstream artefacts are:

- `leynos/rstest-bdd` tag `v0.6.0`, resolved commit
  `72fb22635670e456545ca368805ba4c1c9d7bd69`.
- `docs/v0-6-0-migration-guide.md` at that commit.
- `docs/v0-5-0-migration-guide.md` and `docs/users-guide.md` at that commit.
- ADR-006 (fallible scenario functions), ADR-007 (harness context injection),
  ADR-009 (implicit fixture-name normalization), ADR-019 (type-directed
  step-return classification) at that commit.

Trace links for the gaps this migration must close:

```plaintext
RBDD-060-BREAK-underscore   -> EP-M2 -> ortho_config/tests/rstest_bdd_support/*
RBDD-060-BREAK-step-alias   -> EP-M2 -> behavioural suite scenario outcomes
RBDD-060-BREAK-feature-path -> EP-M2 -> (verified inapplicable, no consumers)
RBDD-060-BREAK-harness      -> EP-M2 -> (verified inapplicable, no harness use)
RBDD-060-DEP-version        -> EP-M1 -> Cargo.lock resolved versions
RBDD-060-DOC-provenance     -> EP-M3 -> docs/rstest-bdd-doc-provenance.md
```

## Verification plan

State every invariant the planned implementation introduces or preserves, with
its verification method.

### Obligation V1: scenario coverage is preserved

- Statement: the number of executing `rstest-bdd` scenarios after migration is
  at least the baseline, and no feature file has lost a `Scenario` block.
- Method: parameterized inventory comparison (static counts per feature file,
  baseline versus post-migration) combined with the behavioural suite's own
  pass/fail report.
- Rationale: a count is a finite, exactly checkable partition of the suite and
  cannot be satisfied by a partially migrated tree.
- Domain: all 28 `.feature` files; scenarios per file.
- Artefact: the inventory command in `Concrete steps`, run before and after.
- Evidence: baseline shows 127 scenarios and 0 outlines; post-migration must
  show >= 127 and the same per-file counts.
- Non-vacuity: the check fails if any `scenarios!` invocation stops matching,
  which is the realistic regression mode. The baseline non-zero count (127)
  proves the counter is not measuring an empty set.

### Obligation V2: `Result<()>` step classification does not silently change a verdict

- Statement: for every scenario, a step returning `Ok(())` does not override a
  fixture used as a flag, and a step returning `Err` fails the scenario.
- Method: targeted behavioural test plus adversarial mutation.
- Rationale: this is the one 0.6.0 change that alters runtime behaviour for
  code this repository actually contains. A reasoned argument alone is
  insufficient because the failure mode is a *false green*, which the existing
  suite would not report.
- Domain: the 175 `-> Result<()>` steps and the 6 `Slot<()>` flag fields on
  `ExtendsContext`.
- Artefact: a focused regression test in the `ortho_config` behavioural
  support tree.
- Evidence: the regression test must fail against the pre-migration tree for
  the intended reason and pass afterwards. Specifically, a "successful unit
  step does not overwrite a set unit flag" case, and an "erroring aliased step
  fails the scenario" case.
- Non-vacuity: the negative control is a mutation that sets two `Slot<()>`
  flags in one scenario and asserts the first survives a succeeding step. If
  the harness ignored `Ok(())` payloads entirely the test would pass vacuously;
  asserting the *post-condition* on the flag distinguishes the two.

### Obligation V3: dependency and lockfile state is fully migrated

- Statement: no `rstest-bdd*` package resolves to a version below 0.6.0, no
  pre-release `rstest-bdd` is present, and the declared requirement resolves to
  the published 0.6.0 packages.
- Method: `cargo tree` and direct lockfile inspection.
- Rationale: a version disappearing from the root manifest is not completion;
  the resolved graph is the observable state.
- Domain: `Cargo.lock` and every `Cargo.toml` naming an `rstest-bdd` variant.
- Artefact: `cargo tree -i rstest-bdd` and a lockfile grep.
- Evidence: `cargo tree` shows `rstest-bdd v0.6.0` for every consumer; the
  lockfile contains no `rstest-bdd` version entry other than 0.6.0.
- Non-vacuity: the check must be run against the pre-migration tree too, where
  it reports 0.5.0. A check that reports success on both trees is not a check.

### Obligation V4: imported documentation is unmodified

- Statement: the imported upstream documents are byte-for-byte identical to
  the pinned upstream revisions.
- Method: checksum comparison against the pinned checkout.
- Rationale: the task requires byte-for-byte integrity and separately recorded
  provenance.
- Domain: the two (or three) imported files.
- Artefact: a provenance document recording source repository, tag, commit,
  source path, destination path and SHA-256.
- Evidence: `sha256sum` of the destination matches the recorded upstream
  value.
- Non-vacuity: an accidental re-wrap or link rewrite by a Markdown formatter
  would change the checksum; this check detects exactly that. It is the reason
  `make fmt` must not be allowed to rewrite the imported files, and the
  conflict between "byte-for-byte intact" and this repository's mandatory
  Markdown formatting gate must be resolved explicitly and recorded.

### Axioms assumed

- The published crates.io `rstest-bdd` 0.6.0 packages correspond to the source
  at the pinned v0.6.0 commit. Evidence: the local registry cache holds both
  the published `.crate` archives and the pinned checkout; the extracted
  published `Cargo.toml` declares `version = "0.6.0"` and the same dependency
  set as the workspace manifest.
- `gherkin` 0.16 requires Rust 1.88, which the pinned 1.89.0 toolchain
  satisfies.
- Third-party library internals are treated as axioms; only this repository's
  own integration with them is verified.

### Residual gaps

- Windows-target behaviour is not exercised by any local gate. This
  repository has Windows CI legs; the local verification can only proxy them.
- Per-scenario behavioural equivalence is argued from suite results plus the
  targeted V2 regression, not exhaustively proven for all 127 scenarios.

## Plan of work

Structure as stages with explicit go/no-go points.

### Stage A: understand and propose (no code changes)

Complete. Delivered the survey, the applicability matrix, the pinned upstream
checkout and this plan. Go/no-go: user approval of this plan.

### Stage B: import authoritative documentation

Import from the pinned checkout `72fb22635670e456545ca368805ba4c1c9d7bd69` into
`docs/`:

- `docs/users-guide.md` → `docs/rstest-bdd-users-guide.md`
- `docs/v0-6-0-migration-guide.md` → `docs/rstest-bdd-v0-6-0-migration-guide.md`
- `docs/v0-5-0-migration-guide.md` → `docs/rstest-bdd-v0-5-0-migration-guide.md`

Write `docs/rstest-bdd-doc-provenance.md` recording repository, tag, commit,
original path, destination path and SHA-256 for each.

Validation: checksums of the destinations match the recorded values; the
provenance document itself passes `make markdownlint`.

### Stage C: dependency and lockfile upgrade (milestone EP-M1)

Edit the root `Cargo.toml` `[workspace.dependencies]` to `0.6.0` for both
declared crates. Run a targeted `cargo update -p rstest-bdd --precise 0.6.0`
(and the same for `rstest-bdd-macros`, plus any transitive
`rstest-bdd-patterns` and `rstest-bdd-policy` that remain at 0.5.0). Do not run
an unrestricted `cargo update`.

Validation: `cargo tree -i rstest-bdd` reports 0.6.0; the lockfile has no 0.5.0
`rstest-bdd*` entries; the workspace still typechecks.

### Stage D: source and behavioural changes (milestone EP-M2)

Classify and apply only the changes the applicability matrix marks as required.
Add the V2 regression coverage. Rebuild and rerun the behavioural suites.

### Stage E: documentation and gates (milestone EP-M3)

Update `docs/developers-guide.md`, `docs/behavioural-tests.md`,
`docs/contents.md` and `CHANGELOG.md` as the repository's documentation
maintenance rules require. Run every gate.

## Milestones and plateaus

### EP-M0 (complete): survey and plan

- Outcome: a pinned upstream checkout, an applicability matrix, and this
  ExecPlan.
- Acceptance evidence: the checksum and tag-resolution transcripts in
  `Artefacts and notes`.
- Compatibility decision: none.

### EP-M1: dependency and lockfile upgrade

- Outcome: the workspace resolves the `rstest-bdd` family at 0.6.0 and still
  builds.
- Requirements and gaps: `RBDD-060-DEP-version`.
- Acceptance evidence: `cargo tree -i rstest-bdd` output; lockfile grep.
- Conformance check:
  - `RBDD-060-DEP-version` satisfied.
  - No production interface change.
  - No new third-party crate beyond the `rstest-bdd` family.
  - No `[patch]` or path substitution introduced.
  - Trace links current.
- Recovery: revert `Cargo.toml` and `Cargo.lock`
  (`git checkout -- Cargo.toml Cargo.lock`) and retry.
- Remaining gaps: source-level changes not yet applied; the tree may not
  compile its tests until EP-M2.
- Compatibility decision: none. This is a pre-1.0 test-only dependency; there
  is no external consumer to keep compatible.

### EP-M2: source and behavioural changes

- Outcome: the workspace's test targets compile and the behavioural suites
  execute against 0.6.0 with coverage preserved.
- Requirements and gaps: `RBDD-060-BREAK-underscore`,
  `RBDD-060-BREAK-step-alias`, and the recorded inapplicability of the
  feature-path and harness items.
- Acceptance evidence: the migration checklist with each item dispositioned;
  the V2 regression test failing pre-migration and passing post-migration; the
  scenario inventory comparison.
- Conformance check:
  - Applicability matrix fully dispositioned.
  - No production API changed.
  - No compatibility shim introduced.
  - No later-release (0.6.1/0.7.0) API adopted.
  - Trace links current.
- Recovery: each source change is a separate commit, so a failing change can
  be reverted in isolation.
- Remaining gaps: documentation updates.
- Compatibility decision: none, as EP-M1.

### EP-M3: documentation and gates

- Outcome: repository documentation describes the 0.6.0 baseline and all
  canonical gates pass.
- Acceptance evidence: gate transcripts; document diffs.
- Conformance check: all items from EP-M2 plus documentation traceability.
- Recovery: `make fmt` then re-run the failing gate.
- Remaining gaps: none expected.
- Compatibility decision: none.

## Concrete steps

All commands run from the worktree root. Log every gate with `tee`.

Inventory command, used for V1 on both trees:

```sh
make test
```

Gate commands, run sequentially and never in parallel:

```sh
make check-fmt 2>&1 | tee /tmp/check-fmt-$(basename $(git rev-parse --show-toplevel))-$(git branch --show-current).out
make lint      2>&1 | tee /tmp/lint-$(basename $(git rev-parse --show-toplevel))-$(git branch --show-current).out
make test      2>&1 | tee /tmp/test-$(basename $(git rev-parse --show-toplevel))-$(git branch --show-current).out
make markdownlint 2>&1 | tee /tmp/markdownlint-$(basename $(git rev-parse --show-toplevel))-$(git branch --show-current).out
make nixie     2>&1 | tee /tmp/nixie-$(basename $(git rev-parse --show-toplevel))-$(git branch --show-current).out
```

Dependency verification, used for V3:

```sh
cargo tree -i rstest-bdd
cargo tree -i rstest-bdd-macros
grep -n 'name = "rstest-bdd' -A1 Cargo.lock
```

The pre-migration tree reports `rstest-bdd v0.5.0`; the post-migration tree
must report `v0.6.0`.

## Validation and acceptance

Software behaviour acceptance: after the change, removing a `Scenario` block
from any feature file makes the corresponding generated test disappear from the
run (proving the suite is genuinely bound to the feature text), and the full
suite passes with at least 127 scenarios.

Quality criteria:

- Tests: `make test` passes; the V2 regression test passes; the `rstest_bdd`
  canary scenario in each consuming crate still executes.
- Verification: obligations V1 through V4 discharged.
- Lint/typecheck: `make lint` and `make check-fmt` pass.
- Documentation: `make markdownlint` passes; the provenance document exists.
- Performance: no benchmark target exists for the test suite; the nextest
  per-test and whole-run budgets in `.config/nextest.toml` already bound the
  suite and were not tightened.

## Idempotence and recovery

Every step is re-runnable. The upstream import is a file copy from a pinned
commit and can be repeated. The dependency edit is a version string. The
lockfile update is `cargo update --precise`, which converges. Source edits are
committed individually so any one can be reverted with `git revert`.

Before any mutating probe on an unstaged file, back it up with `cp` and prove
restoration with `diff`; `git checkout --` restores the *index*, not the
working tree, and destroys unstaged work.

## Artefacts and notes

Tag resolution proving the pinned commit matches the task's expected SHA:

```plaintext
$ git rev-parse 'refs/tags/v0.6.0^{commit}'
72fb22635670e456545ca368805ba4c1c9d7bd69
```

Upstream document checksums at the pinned commit (the integrity reference for
V4), recorded in `docs/rstest-bdd-doc-provenance.md`:

```plaintext
$ sha256sum docs/users-guide.md docs/v0-6-0-migration-guide.md docs/v0-5-0-migration-guide.md
1e9f4d1b6607fdf83df979676f1a682218cd6751381d374cb98b701778b798b2  docs/users-guide.md
6e76c10028f9962134f6158732cf1645ae1b0beba3018bccd3bc3e88b2c38985  docs/v0-6-0-migration-guide.md
508d5b55cb2025915ced366b917a2327e572a6aa920fd290684b858b7afa2abc  docs/v0-5-0-migration-guide.md
```

Baseline scenario inventory: 127 `Scenario` blocks, 0 `Scenario Outline`
blocks, across 28 feature files.

## Interfaces and dependencies

No new public interface is introduced. The dependency surface after EP-M1 is
unchanged in shape:

- Root `Cargo.toml`, `[workspace.dependencies]`:
  `rstest-bdd = "0.6.0"`, `rstest-bdd-macros = "0.6.0"`.
- Consumed API, unchanged in name: `rstest_bdd::Slot`,
  `rstest_bdd::ScenarioState`, `rstest_bdd::StepResult`,
  `rstest_bdd_macros::{scenario, scenarios, given, when, then, ScenarioState}`.

No `rstest-bdd-harness*` crate is added. No `tokio` dependency is added.

## Revision note

Initial draft, 2026-10-14. Created after reconnaissance established: the
baseline is 0.5.0; the upstream tag resolves to the expected SHA; the
repository consumes no harness, no custom `HarnessAdapter`, no direct
`insert_value` or `record_bypassed_steps` caller, no reporter integration and
no language-server API; and the destination path in the task instructions
collides with an unrelated repository document, so a prefixed name is used
instead. The applicability matrix and the full migration checklist are
maintained in the Progress, Decision log and Surprises sections as work
proceeds.
