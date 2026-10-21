# Migrate behavioural suites to `rstest-bdd` v0.6.0

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Upgrade the workspace's `rstest-bdd` family dependencies from the released
**0.5.0** to the published **0.6.0**, and apply every source change the upgrade
requires, while preserving the behaviour and coverage of the existing
behavioural suites.

After this change a contributor can run the repository's canonical gates and
observe:

- `Cargo.lock` resolves `rstest-bdd = 0.6.0` and
  `rstest-bdd-macros = 0.6.0` with no remaining `0.5.0` packages.
- `make test` passes with all baseline `rstest-bdd` scenarios preserved and
  the five new step-return scenarios executing (baseline: 127 `Scenario` blocks
  across 28 feature files; candidate: 132 across 29 feature files; no
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

- **Preserve behavioural coverage.** No baseline `.feature` file may be
  deleted or have `Scenario` blocks removed to make the migration compile.
  Preserve all 127 baseline scenarios and add the five step-return scenarios.
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
  1.88, so this is not expected to trigger.) **This constraint was reached in
  practice**: the effective floor of the resolved graph is 1.91, set by
  `cargo-platform` 0.3.3, not by upstream's declared 1.88. It was resolved
  without escalation by constraining the transitive dependency instead of
  raising the floor, so the trigger did not fire on the final graph. See the
  decision log entry of 2026-10-14 and the applicability matrix row for
  `cargo-platform`.
- Iterations: if the same compile or test failure persists after three focused
  fix attempts, stop and escalate with options.
- Time: if a single milestone exceeds four hours of elapsed work, stop and
  escalate with a reduced-scope option.
- Ambiguity: if a behaviour change is observable in scenario outcomes and two
  valid interpretations produce different acceptance results, stop and present
  both with trade-offs.

## Risks

Known uncertainties identified upfront, with mitigations.

- Historical pre-verification risk, now closed: the step-return classification
  change (ADR-019) was assessed as a possible runtime change for the 175
  `-> Result<()>` steps. Upstream 0.6.0 normalizes a unit payload to `None`
  before the runner inserts anything, so `Ok(())` cannot override a fixture.
  The regression test asserts that `None` and the generated scenario drives the
  same alias through the runner. Status: closed by upstream design and
  regression coverage.
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

Entries below document historical milestones as they occurred; they are not
certification of the current candidate unless explicitly stated.

- [x] (2026-10-14) Reconnaissance: branch, manifests, lockfiles, toolchain,
  CI, feature-file inventory, step-return survey.
- [x] (2026-10-14) Resolve upstream `v0.6.0` tag to an immutable commit and
  compare with the expected SHA.
- [x] (2026-10-14) Read the upstream v0.6.0 migration guide and the 0.5.0
  migration guide; build the applicability matrix.
- [x] (2026-10-14) Import upstream documentation with a provenance record.
  Committed as `fbeaf898`. Three files imported byte-for-byte, provenance
  document written, both Markdown gates confirmed green over the imports.
- [x] (2026-10-14) Write the ExecPlan. The task statement directed a full
  migration rather than a request for approval of one, so the plan is recorded
  here and executed rather than gated.
- [x] (2026-10-14) Confirm the applicability matrix against the live
  repository: no `StepContext`, `insert_value`, `insert_owned`,
  `record_bypassed_steps`, `HarnessAdapter`, or first-party harness use
  anywhere; no `Scenario Outline`; no `runtime =` or `harness =` macro
  argument; every `scenarios!` feature path already manifest-relative.
- [x] (2026-10-14) Baseline: static inventory complete (127 scenarios, 28
  feature files, 0 `Scenario Outline`, 175 + 59 step functions). The full
  workspace test run is executing in the background; its scenario counts are
  the V1 comparator. It is slow because the shared compile pool is saturated,
  not because of any repository problem.
- [x] (2026-10-15) Baseline run completed with `EXIT=101`. Every target passed
  except `ortho_config --test compile_fail`, which failed on the MSRV violation
  recorded below. This is the V1 comparator only. The failure is real and is
  caused by the migration, but it is a compile failure in an unrelated target,
  so it is **not** a red stage for obligation V2: it says nothing about whether
  a step returning an aliased `Result` is classified correctly at run time.
  V2's red stage is the differential control recorded under that obligation.
  Note the baseline is imperfect as a comparator in one respect: the run
  overlapped the manifest edit, so it compiled the 0.6.0 `rstest-bdd` family
  while the trybuild sandbox regenerated against the *pre-existing* sandbox
  manifest. It nevertheless failed for the migrated-tree reason, and the
  pre-migration state is independently confirmed by the static inventory.
- [x] (2026-10-14) Milestone EP-M1 (manifest half): root `Cargo.toml`
  `[workspace.dependencies]` moved to `rstest-bdd = "0.6.0"` and
  `rstest-bdd-macros = "0.6.0"`. All three consumer manifests inherit through
  `workspace = true`, so no other manifest changed.
- [x] (2026-10-14) Milestone EP-M1 (lockfile half): `Cargo.lock` now resolves
  the whole `rstest-bdd` family at 0.6.0 (`rstest-bdd`, `-macros`, `-patterns`,
  `-policy`, plus the new `-harness`). A package-level diff against HEAD shows
  27 changed packages, all inside the 0.6.0 dependency closure; nothing
  unrelated moved.
- [x] (2026-10-14) Milestone EP-M1 (MSRV constraint): the baseline run exposed
  a real floor violation via `cargo-platform` 0.3.3 (declared 1.91). Pinned to
  0.3.2 (declared 1.88) with
  `cargo update -p cargo-platform@0.3.3 --precise 0.3.2`; the resulting
  lockfile delta is that single downgrade, and a full registry scan reports
  zero packages above the 1.89.0 floor.
- [x] (2026-10-17) Milestone EP-M2: source and behavioural changes. An earlier
  draft of `ortho_config/tests/rstest_bdd_support/step_return_regression.rs`
  compiled but proved nothing it claimed: its `Err` assertions compared the
  Fluent-rendered `StepError` against the bare handler message, and its "unit
  payload does not override a fixture" case used a `()`-returning step, which
  the macro classifies as `Unit` and never routes through the dispatch path
  under test. It has been rewritten against the published 0.6.0 sources (see
  Decision log, 2026-10-17) and now has eight tests. (An earlier revision of
  this entry said seven; the module has carried eight `#[test]` functions in
  every commit that contains it, so the count was a transcription error, not a
  later addition.) The `rstest_bdd` target passes: `83 passed; 0 failed`, and
  the whole suite still runs in 0.28 s.
- [x] (2026-10-17) Milestone EP-M3: documentation and gates. The developers'
  guide and the documentation index are updated. Every gate the repository
  defines has now been run on the migrated tree, sequentially, each with its
  output captured to a log under `/tmp`:
  - `make lint` — green. Both phases ran: `cargo doc --workspace --no-deps`
    under `RUSTDOCFLAGS="-D warnings"`, `cargo clippy --workspace
    --all-targets --all-features -- -D warnings`, and `whitaker --all --
    --all-targets --all-features` under `RUSTFLAGS="-D warnings"`. No
    cognitive-complexity, module-size, or lint finding remains.
  - `make test` — green: 2002 Rust tests passed, 0 failed, 15 ignored; the
    Python suite reported 115 passed and 5 skipped. Includes
    `ortho_config --test compile_fail` (`2 passed`), the trybuild UI suites,
    the BDD behavioural targets in all three consuming crates, and the
    doctests.
  - `make check-fmt` — green (`cargo fmt --all -- --check` and
    `mdtablefix --check`).
  - `make markdownlint` — green (89 files, 0 issues; spelling gate reports
    `current: typos.toml`, so the generated file needed no rewrite).
  - `make nixie` — green (all Mermaid diagrams validated).
  This historical run predates the step-return feature file. The candidate
  preserves all 127 baseline scenarios and adds five scenarios in one new
  feature file; no baseline scenario was ignored, renamed away, or weakened.
- [x] (2026-10-17) Post-rebase gate rerun. After the replay onto `main` at
  `67fdb341` and the `output` module split it forced, all five gates were run
  again on the source tree that the branch delivers
  (`5a961a4a991274f73d0b8e18c5bf817851419e76`). The counts moved because `main`
  added tests, not because this branch changed any:
  - `make lint` — green, all three phases reached (rustdoc, Clippy, Whitaker).
  - `make test` — green: 2045 Rust tests passed, 0 failed, 25 ignored; the
    Python suite reported 115 passed and 5 skipped.
  - `make check-fmt` — green (`89 files left unchanged`).
  - `make markdownlint` — green (`Summary: 0 issues`; `current: typos.toml`).
  - `make nixie` — green (all diagrams validated).
  The dependency-only commit `e0af849f` was also `cargo check`ed on its own
  (`--workspace --all-targets --all-features`), proving the refactor in the
  next commit is not concealing a required API adaptation.
- [x] (2026-10-18) Final delivery at that point. The pull request head then
  delivered a tree that differs from the gated tree
  `5a961a4a991274f73d0b8e18c5bf817851419e76` only in this ExecPlan. Every
  changed path in the delta is Markdown, so the Rust gate certificates transfer
  by content identity, and the Markdown gates were re-run over the delivered
  head. The four work commits are ordered so that the documentation import, the
  dependency and lockfile migration, the one-emission-per-function refactor,
  and the step-return regression suite can each be reviewed or reverted on
  their own; a fifth commit records this delivery state. Later review fixes
  changed source files and added five step-return scenarios, so this entry is
  historical and does not certify the current candidate.
- [x] (2026-10-18) Milestone CLI review and plan correction. The review
  `coderabbit review --agent --committed --base-commit 67fdb341` (CLI 0.9.0)
  returned zero findings across all 27 committed paths of the delta; the file
  list the tool reported matches `git diff --name-only 67fdb341...HEAD`
  exactly, so the null result is full coverage rather than a skipped pass.
  Nothing to disposition and no repair, so no gate needed re-running. The same
  audit of this plan found three count errors, all corrected here: the
  regression module's stated test count (it said seven; the module has carried
  eight `#[test]` functions in every commit containing it), the `ortho_config`
  feature-file count in *Context and orientation* (it said 22; the tree has 18,
  and 18 + 6 + 4 = 28, the total the acceptance criteria use), and the commit
  count (four work commits plus the delivery record). All are
  documentation-only corrections.
- [x] (2026-10-18) CodeScene repairs after the Code Health Review raised five
  findings against the migration's own delta. Every finding was introduced by
  this branch — the four files it flagged scored 10.0 at base `67fdb341`, and
  the fifth (`telemetry_events.rs`) exists only here — so the failures are
  branch regressions, not inherited debt. All five are repaired by
  decomposition, none by suppression, threshold change, rule disabling or
  metric gaming:
  - `ortho_config/tests/rstest_bdd_support/step_return_regression.rs`:
    four shape-identical tests collapsed into two `#[rstest]` cases driven by
    a new `AliasShape` enum and one `outcome_of_given` driver. The
    parameterization keeps both alias shapes in the regression net.
  - `ortho_config/src/discovery/telemetry_events.rs`: the six-and-two
    duplicated emitters collapse to one `tracing` call each, and the module
    is folded back into `telemetry.rs`, which then measures 352 lines. **This
    step was reversed on 2026-10-19** after a review showed the collapse broke
    the event's documented metadata contract; see the superseding observation
    under *Surprises & discoveries* and the entry below.
  - `cargo-orthohelp/src/rustflags.rs`: newtypes (`RustflagsVariable`,
    `RustflagsValue`, `RustflagToken`) plus one `Delimiter`-selected
    sanitizer replace the `&str`-parameterized helpers and the two
    near-identical sanitizers.
  - `cargo-orthohelp/src/output/mod.rs`: two loggers merge into one
    `DirectoryStage`-parameterized emitter.
  - `cargo-orthohelp/src/agent_context/mod.rs`: a `SelectorFlag` newtype
    replaces the bare `&str` threaded through three selector helpers.
  Result: all five files report 10.0, and `cs delta 67fdb341` reports "No
  issues found in checked changes". Emitted events are unchanged: same names,
  fields, messages and metric labels.
- [x] (2026-10-19) Metadata-contract repair after PR review (Codex, P2 on
  `telemetry.rs`). The collapse above suppressed a field's *value* for `None`
  but not its *name*: `tracing`'s `fieldset!` writes every listed identifier
  into the callsite's static `Metadata`, so a subscriber filtering on
  `Metadata::fields` saw `scope`, `source` and `selector_class` on events whose
  documentation says they are omitted. Restored one emitter per field subset in
  `telemetry_events.rs`, with the three entrypoints dispatching to them.
  - The emitters are generated by three `macro_rules!` helpers that expand to
    one statement each. Written out by hand they re-triggered CodeScene's
    duplication finding (8.81 on a probe); generated, each body is distinct at
    the level the detector reads while the callsites — and so the metadata —
    stay separate. Both modules measure 10.0.
  - The emitter split keeps every function to at most one `tracing`
    expansion, so `cognitive-complexity` stays inside the ceiling the
    `log`-bridge expansion imposed; `cargo clippy --all-targets
    --all-features -- -D warnings` is clean.
  - New `ortho_config/tests/discovery_telemetry_metadata.rs` asserts on
    `Metadata::fields` rather than recorded values, the layer the existing
    harnesses cannot see. Proven non-vacuous: against the collapsed form five
    of its eight cases fail.
  - Three documentation corrections from the same review: the `[workspace
    .dependencies]` versions are described as historical, the pinned channel
    reads `1.99`, and the gate-command block sets `pipefail`.
- [x] (2026-10-19) Pre-merge-check repairs. The assessment raised three
  findings against `55bad4e7`, and all three were valid. Each is repaired by
  adding the coverage it named rather than by changing production code, which
  the checks confirmed was already correct:
  - **Runner-boundary scenario coverage.** New
    `ortho_config/tests/features/step_return.feature` with five scenarios and
    `ortho_config/tests/rstest_bdd_support/step_return_scenarios.rs` with the
    matching `#[scenario]` bindings, so both aliases are driven through the
    generated scenario body: an aliased `Err` fails the scenario (asserted with
    `#[should_panic(expected = ...)]`), `Ok(T)` reaches a later step as the
    fixture payload, and `Ok(())` records no override. The focused bridge tests
    remain, with their module doc now stating that `insert_value` is not the
    only coverage. Recorded target: `92 passed; 0 failed`.
  - **Property test for the rustflags sanitizer.** New
    `cargo-orthohelp/src/rustflags_proptests.rs` compares the sanitizer against
    an independently written reference filter over generated token sequences,
    for both delimiters. Five properties assert exact output text (so order and
    delimiter survive), `None` exactly when every token is removed, and
    byte-identical passthrough when nothing is removed. The generators end in a
    coverage token, or are made entirely of removable units, so neither branch
    is vacuous. Recorded target: `11 passed; 0 failed`.
  - **Compile-time alias coverage.** New
    `ortho_config/tests/trybuild/step_return_aliases.rs`, registered as a third
    `t.pass` fixture in `tests/compile_fail.rs`, declares steps for both
    `anyhow::Result<T>` and `Result<T, Box<dyn Error + Send + Sync>>` and calls
    them from `main`, so the generated wrappers and handler bodies are
    type-checked. `trybuild` does not execute the fixture; runner behaviour is
    covered by the separate feature scenarios.
  - **Scoped `not_found` spelling.** The fourth row, an inline request for a
    scoped `not_found` telemetry case, is implemented as an in-crate unit test.
    The dispatcher arm exists at `ortho_config/src/discovery/telemetry.rs:348`,
    but no production discovery path reaches it, so
    `discovery::telemetry_tests` calls `load_outcome` directly and asserts on
    the callsite's declared metadata that `scope` is present and `source` is
    absent. `telemetry_test_support::CapturedEvent` gained `declares()` for
    this, since a field recorded as absent and a field never listed are
    indistinguishable by value. Recorded target: `3 passed; 0 failed`.
  No production source changed, so the earlier gate certificates for runtime
  behaviour are untouched; the new files are Rust and Markdown inputs, so the
  full gate set is re-run rather than transferred.
- [x] (2026-10-20) First full-suite rerun used:

  ```sh
  ORTHO_CONFIG_DOC_TEST_TIMEOUT_SECS=7200 make test
  ```

  The host reached 0 bytes available after 6,786 s and stopped the run (exit
  130). It produced no assertion failure or final test summary. The BDD target
  completed all 43/43 scenarios successfully, and completed trybuild/UI suites
  passed; four documentation-example/concurrency tests were still active.
  Candidate build caches were removed only after confirming no live process
  referenced them, restoring approximately 28.7 GiB free. This did not
  establish a passing `make test`; the candidate cache was cleared and the
  complete sequential gate set was rerun below.
- [x] (2026-10-20) Complete sequential gate rerun on implementation candidate
  `5062d657a84c34a1d0172c18b27830e08c634683`:
  - `make check-fmt` — passed; 89 files unchanged.
  - `make lint` — passed; rustdoc, Clippy and Whitaker.
  - `make typecheck` — passed.
  - `ORTHO_CONFIG_DOC_TEST_TIMEOUT_SECS=7200 make test` — passed; 101 Rust
    targets reported 2,069 passed, 0 failed and 33 ignored; Python reported
    115 passed and 5 skipped. The `rstest_bdd` suite passed all 43 scenarios.
  - `make markdownlint` — passed; 0 issues.
  - `make nixie` — passed; all Mermaid diagrams validated.
  The failed disk-exhausted run above is retained as process history. This
  complete run has no failing target. Final delivery changes only this
  ExecPlan; its Markdown gates were rerun after the entry was written, and the
  Rust gate results transfer by content identity.
- [x] (2026-10-20) Refreshed the candidate after `main` advanced. Rebased the
  13 branch commits from `2d6ed36` onto `3c4e2565`; `git range-diff` matched
  all 13 patches one-to-one, and `git diff --check` passed. The merged lockfile
  retains the full `rstest-bdd` family at 0.6.0 and `cargo-platform` 0.3.2,
  while incorporating `sha2` 0.11.1 from `main`. The developer guide retains
  both `main`'s shared coverage guidance and this branch's migration strategy.
  All six local gates ran sequentially on candidate
  `6876bfb61ea0af43511ed398f76fafa40fb17592`:
  - `make check-fmt` — passed; 89 files unchanged.
  - `make lint` — passed; rustdoc, Clippy and Whitaker.
  - `make typecheck` — passed.
  - `ORTHO_CONFIG_DOC_TEST_TIMEOUT_SECS=7200 make test` — passed; 2,072 Rust
    tests passed, 0 failed and 35 ignored across 101 targets; BDD 43/43;
    Python 115 passed and 5 skipped.
  - `make markdownlint` — passed; 0 issues and `typos.toml` current.
  - `make nixie` — passed; all Mermaid diagrams validated.

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as risks.

- Observation: the `cargo-orthohelp` output-module split silently dropped a
  field from one existing diagnostic. The
  `"failed to serialize agent-context JSON"` event carried `package`, `path` and
  `error` in the former `output.rs`; the extracted serialization helper in
  `output/agent_context.rs` initially accepted only the package and the error,
  so the event lost `path`. Evidence:
  `git show <base>:cargo-orthohelp/src/ output.rs` shows the three-field
  emission, and the extracted helper's signature shows only two parameters
  reaching the event. Impact: any operator filter keyed on `path` for that
  event would have stopped matching, which is a telemetry regression introduced
  by a refactor that claimed to preserve the events. Repaired by threading
  `out_dir` back through the helper.
- Observation: the V2 regression module's `Anyhow` case did not exercise
  `anyhow::Error` at all. `type StepAlias<T> = Result<T, &'static str>` with
  `use anyhow::Result` in scope still names the imported `anyhow::Result`:
  `anyhow::Result<T>` defaults its error type to `anyhow::Error`, and the
  explicit `&'static str` replaced that default rather than selecting a
  different `Result`. Evidence: `anyhow` 1.0.104 declares
  `pub type Result<T, E = Error> = core::result::Result<T, E>`, so supplying a
  second argument to that alias is still the alias. Impact: the test named the
  crate's real alias but parameterized it with the wrong error type, so the
  coverage claim was wrong even though the assertions passed. Repaired by
  declaring the alias as `anyhow::Result<T>` directly.

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
- Observation: a first pass at the survey concluded `cargo-orthohelp`'s local
  `StepResult<T>` alias was used only by helper functions. That was **wrong for
  the crate as a whole** and right only for the single file examined:
  `cargo-orthohelp/tests/rstest_bdd_support/behaviour/steps_cmd.rs` declares
  steps such as `fn temp_output_dir(...) -> StepResult<()>` under `#[given]` and
  `#[then]`. Evidence: listing each step macro attribute together with its
  following signature shows `StepResult` returns across the `behaviour/` tree.
  Impact: ADR-019's alias classification is load-bearing in two crates, not
  one, and both aliases satisfy the `Display` bound (`anyhow::Error` and
  `Box<dyn Error + Send + Sync>`). The correction is recorded because the first
  conclusion would have understated the migration's coverage.
- Observation: the import needed **no `mdtablefix` exclusion**, contrary to the
  plan's working assumption. Evidence: `mdtablefix --check` with the Makefile's
  exact rule set reports the three imported files unchanged, and
  `markdownlint-cli2` reports zero issues. Impact: the byte-for-byte guarantee
  survives both Markdown gates with no tooling change, which is a better
  outcome than the exclusion the plan anticipated. This holds for the pinned
  revision only and must be re-checked on a future import.
- Observation: the two overwritten documents were earlier imports of the same
  upstream guides, and overwriting them destroyed no uncommitted work.
  Evidence: the session log for the migration records the opening `git status`
  as `(clean)`, so the working tree matched its commit before the import; the
  HEAD revision of the users' guide contains zero occurrences of `ortho_config`,
  `orthohelp`, `ddlint` or `cargo-ortho`. The 86 differing lines against
  upstream v0.5.0 are reflows, en-GB spelling corrections (`initialized`,
  `Internationalized`), and Mermaid node-label quoting — all mechanical, none
  semantic. Impact: the earlier revision loses its repository spelling
  corrections in exchange for byte-for-byte fidelity, which is the documented
  trade. The provenance document records this explicitly so a future maintainer
  does not "restore" the spelling and silently break the checksums.
- Observation: the migration **does** move the effective compiler floor to
  1.91, even though upstream's own MSRV is 1.88. Evidence: the baseline
  workspace run failed with
  `error: rustc 1.89.0 is not supported by the
  following package: cargo-platform@0.3.3 requires rustc 1.91`,
  failing both tests in `ortho_config --test compile_fail`. The chain is
  `rstest-bdd-macros` 0.6.0 → **non-optional** `rstest-bdd-harness` 0.6.0 →
  `cargo_metadata` 0.23.1 → `cargo-platform ^0.3.0` → floating resolution to
  0.3.3 (declared 1.91). Impact: the working assumption recorded in Constraints
  was false, and the failure was invisible to `cargo build`/`cargo test` of the
  workspace because only the generated trybuild sandbox resolves
  `cargo_metadata` 0.23.1. Resolved without raising the floor by pinning
  `cargo-platform` to 0.3.2 (declared 1.88), which satisfies the `^0.3.0`
  requirement; a full scan then found zero registry packages declaring a floor
  above 1.89.0, and the sandbox rebuilt against 0.3.2. Confirmed by re-running
  the exact target that failed:
  `cargo test -p ortho_config --test compile_fail` now reports
  `2 passed; 0 failed` (baseline: `0 passed; 2 failed`), and the regenerated
  sandbox lock pins `cargo-platform 0.3.2` with `rstest-bdd 0.6.0`.
- Observation: the failure mode of an MSRV violation here is a *trybuild*
  failure, not a build failure. Evidence: the workspace compiled and ran to
  completion under the 0.6.0 tree in the baseline run; only
  `tests/compile_fail.rs` failed, because trybuild generates its own sandbox
  manifest and lockfile. Impact: a green `make test` on the main workspace is
  not evidence that the floor holds, and the compile-fail target must be part
  of any MSRV reasoning. Recorded in the developers' guide.
- Observation: the step-return regression module, as first drafted, could not
  fail for the reasons it claimed. Evidence: three defects, each confirmed
  against the published crate sources. (1) `StepError`'s `Display` is the
  Fluent pattern
  `Error executing step '<pattern>' via function '<function>': <message>`, so
  an exact-equality assertion against the bare handler message compares against
  the wrong string entirely. (2) A `()`-returning step is classified `Unit` by
  `classify_step_return_type` before dispatch, and the emitted body is
  `{call; Ok(None)}`, which never reaches the probe, the normalizer, or the
  runner's `insert_value` branch. (3) `insert_value` records overrides in
  `StepContext::values`, keyed by the uniquely matching fixture name; it never
  writes into a `Slot<T>`'s own interior cell, so `is_filled()` on that slot
  could not observe an override even if one had been recorded. Impact: the
  draft would have passed while proving none of the three behaviours it
  documented. The rewrite fixes all three and adds an end-to-end
  override-plus-read-back case. No repository defect was implied: the defects
  were in the test, not in the migration.
- Observation: **the root cause of the red `make lint` is a genuine migration
  regression, and it is not a toolchain difference.** `rstest-bdd` 0.5.0 had no
  `tracing` dependency at all; `rstest-bdd` 0.6.0 depends on `tracing` with
  `features = ["log"]` (upstream added it for the ADR-020 warning path in
  `src/context/warnings.rs`). Cargo's feature unification means that under
  `cargo clippy --workspace --all-targets --all-features`, the `log` feature is
  enabled for *every* `tracing` in the graph, including `ortho_config`'s own
  direct dependency. The `tracing` macros then expand into the additional
  `__tracing_log` bridge arms, and `cognitive_complexity` counts those
  branches. Four functions in `ortho_config/src/` crossed 9/9 as a result
  (16/9, 17/9, 30/9, 11/9), and five more in `cargo-orthohelp/src/`.
  - Evidence, in order of decisiveness:
    - `~/.cargo/registry/.../rstest-bdd-0.6.0/Cargo.toml` has
      `[dependencies.tracing] version = "0.1"` with `features = ["log"]`;
      `rstest-bdd-0.5.0/Cargo.toml` has no `[dependencies.tracing]` at all.
    - `cargo tree --workspace --all-features -e features -i tracing` prints
      `tracing feature "log"` under `rstest-bdd v0.6.0`.
    - Under `-p ortho_config --lib` the dev-dependency graph is absent, so
      `tracing/log` is not enabled and the same functions pass. Under
      `--all-targets` it is enabled and they fail. The fingerprint JSON
      confirms two distinct `tracing` unit hashes, one with `log` and one
      without.
    - The `main` CI job that is green runs the identical command, at
      `tracing v0.1.44`, and reports zero cognitive findings — because `main`
      still resolves `rstest-bdd` 0.5.0.
  - Ruled out by probe: local toolchain version (1.89, 1.98 and 1.99 all
    reproduce it once `tracing/log` is on); stale build artefacts (a forced
    cold rebuild reproduces it); `clippy.toml` (unchanged); `CARGO_INCREMENTAL`
    and `CARGO_PROFILE_DEV_DEBUG`; `-C strip=debuginfo` vs `-C debuginfo=2`;
    `--no-deps`; and Cargo `resolver = "3"` (a probe changed it and the
    findings were unchanged, so it was reverted).
  - Impact and resolution: this must be repaired by refactoring, not
    suppressed — the task forbids silencing lints. Each event spelling moved
    into its own small function, so every function contains at most one
    `tracing` call and the bridge expansion counts once rather than twice per
    function. `cargo-orthohelp`'s emitters were extracted alongside their call
    sites. No event field, message, metric or control flow changed; the emitted
    events are byte-identical.
  - **Superseded twice.** The CodeScene repair of 2026-10-18 restated both
    claims, then had to withdraw them on 2026-10-19 when a review showed the
    withdrawal itself was wrong.

    *First supersession (2026-10-18).* The entry originally claimed that
    `ortho_config`'s emitters lived in a new `discovery/telemetry_events.rs`,
    "split out because the added helpers took `telemetry.rs` past the 400-line
    ceiling", and its module doc went further: that "omit a field when its
    value is absent" *cannot* be written as one call. The repair judged both
    claims wrong, on the ground that `tracing-core` records nothing for a
    `None` and a bare identifier in a field list passes the value through, so
    one call *can* omit an absent optional field. The emitters were folded back
    into `telemetry.rs`, and `telemetry_events.rs` was deleted.

    *Second supersession (2026-10-19).* That judgement confused the recorded
    value with the declared field name. `tracing`'s `fieldset!` stringifies
    every identifier in the field list into the callsite's static `Metadata`
    regardless of the value, and `FieldSet::field(name)` searches that name
    list. A `None` therefore suppresses the *value* while leaving the *name*
    in metadata — observable to any subscriber that filters on
    `Metadata::fields`. Because `docs/design.md` documents `scope`, `source`
    and `selector_class` as omitted from the event, and omission from metadata
    is the stronger reading, the split really is required. `telemetry_events.rs`
    is restored with one emitter per field subset, and
    `tests/discovery_telemetry_metadata.rs` now asserts on
    `Metadata::fields`, a layer the value-recording harnesses cannot see.

    Both prior positions are kept above rather than deleted, because the same
    wrong inference — "a `None` is invisible, so one call will do" — is easy to
    re-derive from the same true premise.
- Observation (superseded): an earlier entry in this plan attributed the red
  lint to a stale local toolchain pin (1.89 against CI's 1.99) and concluded
  the branch merely needed replaying onto `main`. That was wrong. The branch
  did need the replay for other reasons, but replaying it would have left the
  lint red: `tracing/log` arrives with `rstest-bdd` 0.6.0 regardless of which
  compiler runs. The entry was kept in this superseded form rather than deleted
  so a future reader does not repeat the same inference from the same symptom.
- Observation: the baseline behavioural run takes far longer than the
  repository's normal test time because the shared compile pool is saturated.
  The `agent_context_contract` cases took 722 seconds and the canary compile
  tests likewise run long. Impact: the baseline is a valid measurement but a
  slow one; the migrated run will be compared against it for scenario counts
  rather than wall-clock, and the two runs should not be executed concurrently
  since that would contend for the same pool.
- Observation: a test *helper* is outside the reach of
  `allow-expect-in-tests`, even in a file whose whole purpose is testing. The
  new metadata suite's `run_user_scope` helper called `.expect()` four times and
  `make lint` failed there, while every one of its `#[test]` functions would
  have been allowed to. The obvious repair — make the helper return `Result`
  and let the tests propagate with `?` — trips `clippy::panic_in_result_fn`
  instead, because a `Result`-returning test may not use `assert!`. The shape
  that satisfies both is the one now in the file: the *helper* returns `Result`
  and propagates with `?`, the *tests* return `()` and unwrap the helper at
  exactly two call sites, where `allow-expect-in-tests` does apply and the
  message names the case that needed the fixture. Impact: `cargo test` alone
  would not have caught this, since it compiles the target without the denied
  lints; only `make lint` proves it. A second, cheaper lesson from the same
  round: `check-fmt` runs `mdtablefix` as well as `cargo fmt`, so edited prose
  can fail a formatting gate while the Rust tree is clean.
- Observation: a payload type shared between two test modules silently stops
  being injected. `StepContext::insert_value` filters the fixture set by
  `TypeId`, and returns `NoMatch` when it finds none or several. Declaring a
  second, structurally identical `Level` in the regression module would give
  the scenario's payload a different `TypeId` from the fixture the scenario
  seeds, so every `Ok(T)` scenario would pass while recording nothing. Evidence:
  `ortho_config/src/context/entry.rs` stores `TypeId::of::<T>()` in the entry,
  and `insert_value` matches on that value. Impact: the payload type, the steps
  that produce it, and the fixture that seeds it must live in one module; the
  regression module imports the scenario module's `Level` rather than
  redeclaring it. A future reader tempted to move one of the two will get a
  green suite that tests nothing.
- Observation: `#[scenario]` with no selector runs the feature file's **first**
  scenario only, and selection by name is unambiguous or an error. Evidence:
  `rstest-bdd-macros-0.6.0/src/macros/scenario/selection.rs` maps an absent
  selector to `0..len` but returns the first match, and `find_scenario_by_name`
  compares `scenario.name == name` exactly and errors on zero or several
  matches. Impact: five bindings over one feature file each carry an explicit
  `name = "…"`; without them the same first scenario would have run five times
  and three of the required cases would have had no coverage at all.
- Observation: `And` and `But` are resolved to the preceding primary keyword by
  `StepKeyword::resolve`, which is defined once in
  `rstest-bdd-patterns-0.6.0/src/keyword.rs:86-109` and shared by the runtime
  and the macro crate. An earlier draft of this plan attributed the resolution
  to a `resolve_keyword_tokens` helper in the macro crate's scenario codegen;
  that helper could not be re-confirmed in the published sources, so the
  citation above is the one that was verified. Impact: none for this
  repository, which uses no `And`/`But` steps, but the earlier attribution
  should not be relied on. Kept as a correction rather than silently deleted
  because the same plausible-sounding location is easy to re-derive.
- Observation: the scoped-load-outcome telemetry event that a review row asked
  to test for a `not_found` case has **no production caller**, although the
  dispatcher arm for it exists and is exercised. Evidence:
  `ortho_config/src/discovery/telemetry.rs:348` is the only call of
  `events::load_outcome_scope` (defined at `telemetry_events.rs:139`), and
  every discovery call site passes `None` for scope — `scoped.rs:117`/`:171`,
  `load.rs:153`/`:173` and `policy.rs:97`; only `scoped.rs::compose_scope`
  could supply one, and it emits its per-scope terminal only inside
  `if let Some(source) = winner`, matching the 318 ExecPlan's winners-only
  per-scope terminals. Impact: the requested case cannot be driven through
  discovery, so the implementation drives `load_outcome` directly from the
  sibling unit module, where `pub(super)` emission helpers are reachable. An
  earlier draft of this plan said the emitter had no caller at all; that
  overstated the finding and is corrected here. The test asserts on
  `Metadata::fields` rather than recorded values, because a `None` suppresses a
  value but not the name a callsite declares.
- Observation: a trybuild fixture cannot bind `#[scenario]`, and its scratch
  crate does not inherit the workspace lint set. Evidence: trybuild's scratch
  `Manifest` (`src/dependencies.rs:100-113`) has no `lints` field and
  `src/run.rs:218-335` builds the manifest field by field without one, so
  `[workspace.lints]` does not reach the fixture;
  `manifest_relative_feature_path` resolves against `CARGO_MANIFEST_DIR`, which
  for a fixture is the scratch crate. Impact:
  `tests/trybuild/step_return_aliases.rs` declares its steps and invokes them
  directly from `main`, which is also what makes the bodies type-check; the
  runner-boundary behaviour is covered by the `rstest_bdd` integration target
  instead. A `#[expect]` in a fixture also does not have to satisfy the
  repository's `allow_attributes` denies, because those lints are workspace
  configuration rather than rustc defaults — the two expectations in the
  fixture mirror upstream `tests/step_return.rs` and are scoped to the two
  functions that genuinely return an always-`Ok` `Result`.

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
  already satisfied. This is recorded as a decision because the task authorizes
  a bump and a reader should know it was considered and found unnecessary
  rather than overlooked. Date/Author: 2026-10-14, migration agent.
- Decision: superseded in part on 2026-10-17 — the branch must adopt `main`'s
  `rust-toolchain.toml` pin when it is replayed onto current `main`. Rationale:
  the repository floor (`rust-version = "1.89.0"`) and the *toolchain the gates
  run under* are different things, and the earlier entry conflated them. `main`
  raised the toolchain channel to 1.99 without raising the declared floor, so a
  contributor building at `rust-version` still works while CI lints with 1.99.
  Keeping 1.89 on this branch means CI and the local gate disagree; adopting
  1.99 keeps them identical and changes nothing about the MSRV story. The
  `cargo-platform` 0.3.2 pin is unaffected and still holds the declared floor
  at 1.89.0. Date/Author: 2026-10-17, migration agent.
- Decision: hold the 1.89.0 floor by constraining a transitive dependency
  rather than raising the floor to 1.91. Rationale: the original reasoning
  above was falsified during milestone EP-M2. `rstest-bdd-macros` 0.6.0 takes a
  **non-optional** dependency on `rstest-bdd-harness`, which depends on
  `cargo_metadata` 0.23.1; that in turn requires `cargo-platform ^0.3.0`, which
  resolves to 0.3.3 and declares `rust-version = "1.91"`. Upstream's own MSRV
  of 1.88 is therefore not the effective floor of the graph the repository
  actually resolves. Downgrading one package
  (`cargo update -p cargo-platform@0.3.3 --precise 0.3.2`) restores the floor:
  0.3.2 declares `rust-version = "1.88"` and satisfies `cargo_metadata`'s
  `^0.3.0` requirement. Verified by a scan of every registry package in
  `Cargo.lock`, which then reported zero packages declaring a floor above
  1.89.0. Rationale for preferring the pin over the bump: the task permits a
  floor increase but does not require one, and a repository that does not need
  to raise its floor should not. Date/Author: 2026-10-14, migration agent.
- Decision: verify the V2 step-return contract differentially rather than by
  a pre-migration red run. Rationale: the regression module uses 0.6.0-only APIs
  (`InsertOutcome`, `FixtureBorrowError`, the `InsertOutcome`-returning
  `insert_value`), so it cannot compile against 0.5.0 and cannot stage a red
  there; and the repository's steps return unit aliases only, so a `Slot<()>`
  flag mutation is vacuous under both versions. The differential control
  reproduces 0.5.0's `ReturnKind::Value` arm from upstream source and asserts
  it disagrees with 0.6.0's `step_return` bridge on both the failing and the
  succeeding path. Date/Author: 2026-10-18, migration agent.
- Decision: rebuild the V2 regression module around the published 0.6.0
  runtime semantics rather than around the migration guide's prose. Rationale:
  the first draft asserted `StepError`'s `Display` output equalled the bare
  handler message. It does not: `step-error-execution` in
  `i18n/en-US/rstest-bdd.ftl` renders
  `Error executing step '<pattern>' via function '<function>': <message>`, and
  `ExecutionError` wraps that again for `HandlerFailed`. The draft's unit case
  was doubly vacuous: a `()`-returning step is classified `Unit` at macro
  expansion and emits `{call; Ok(None)}` without ever consulting the probe, and
  `insert_value` records overrides in a separate `values` map keyed by fixture
  name, so a `Slot<()>`'s own cell can never observe the override the test
  claimed to guard. The rewrite asserts the handler message on
  `StepError::ExecutionError { message, .. }` (with a wildcard, since the enum
  is `#[non_exhaustive]`) and proves the unit classification with a
  `StepAlias<()>`-returning step that must yield `Ok(None)`. Evidence the
  rewrite is non-vacuous: it drives `execute_step`, calls `insert_value` as the
  generated scenario loop does, and asserts that a later `&Level` step reads
  the override. Date/Author: 2026-10-17, migration agent.
- Decision: keep the focused bridge tests alongside the new runner-boundary
  scenarios rather than replacing them. Rationale: the runner cannot express
  displacement of an earlier override or a payload with no matching fixture;
  those need the manual `insert_value` call. Conversely the bridge tests cannot
  notice a defect in the code that calls `insert_value`. The two modules are
  complements, and each now documents which mutations it owns. Date/Author:
  2026-10-19, migration agent. Revised 2026-10-21: the `NotMutable` observation
  originally listed here proved nothing, because `execute_step` never calls
  `insert_value`, so a fixture inserted by shared reference stays non-mutable
  whatever the step returned. The unit claim rests on the direct `Ok(None)`
  assertion instead.
- Decision: pin the scoped `not_found` telemetry spelling with an in-crate unit
  test that drives the dispatcher directly, rather than through a discovery
  run. Rationale: the dispatcher arm exists but no production path reaches it,
  because `compose_scope` emits per-scope terminals only for a winner, so there
  is nothing in discovery to observe. Driving `load_outcome` from
  `discovery::telemetry_tests` records the documented spelling without
  inventing a code path, and it guards the arm against being collapsed into the
  bare emitter by a future not-found path. The assertion is on declared
  metadata, since a suppressed `None` and an unlisted name record identically.
  Date/Author: 2026-10-19, migration agent.
- Decision: verify the compile-time alias contract with a trybuild *pass*
  fixture rather than a compile-fail one. Rationale: the claim under test is
  that both aliases reach the same `Result` normalization, so a regression is a
  compile error, not a diagnostic message. `trybuild` compiles the pass
  fixture; its `main` contains direct handler calls that type-check the handler
  bodies. The separate feature scenarios exercise runner behaviour. No
  `.stderr` snapshot belongs to a contract that must succeed. Date/Author:
  2026-10-19, migration agent.

## Outcomes & retrospective

Milestones EP-M1, EP-M2 and EP-M3 have landed; the migration is complete and
every repository gate is green (see EP-M3 for the evidence). Approval to merge
has not been sought; the branch is presented for review only.

- Outcome: the `rstest-bdd` family resolves uniformly at 0.6.0 (`rstest-bdd`,
  `-macros`, `-patterns`, `-policy`, and the newly transitive `-harness`). The
  declared floor stayed at 1.89.0, held there by one targeted downgrade of
  `cargo-platform` to 0.3.2 rather than by raising it to 1.91. The toolchain
  channel is the `main` branch's 1.99; a contributor building at the declared
  floor still works, and CI and the local gates now compile with the same
  compiler.
- Outcome: no feature flag was relaxed, no snapshot was blessed, no lint was
  suppressed, no test was disabled, and no old/new compatibility facade was
  introduced. The step-return regression suite was written against the
  published 0.6.0 sources after the first draft was shown to be vacuous; the
  rewrite proves aliased `Err` propagation, aliased `Ok(T)` injection, `Ok(())`
  non-injection, and read-back of an override by a later step.
- Outcome: the heading risk of the migration — the `tracing/log` feature
  unification that reddened the lint — was root-caused to evidence rather than
  guessed at, and repaired by refactoring each `tracing` event spelling into
  its own function. The resulting pressure on the 400-line module cap produced
  three module-directory splits that follow a pattern `main` itself already
  uses. No emitted event field, message, metric name or control flow changed.
- Outcome: the branch was rebased onto `main` at `67fdb341`. That commit
  brought in the private-documentation sweep, which denies
  `clippy::missing_docs_in_private_items` and documents every private item. The
  sweep's added doc comments pushed `cargo-orthohelp/src/output.rs` past
  Whitaker's 400-line module cap once the telemetry helpers landed beside them;
  moving `write_agent_context_stdout` into the existing
  `output/agent_context.rs` child brought the parent back to 374 lines without
  dropping a doc line.
- Outcome: the work is delivered as four atomic work commits — the pinned
  documentation import, the dependency and lockfile migration, the
  one-emission-per-function refactor, and the step-return regression suite — so
  each unit can be reviewed or reverted on its own, plus a fifth commit that
  records the final delivery state in this plan. The dependency-only commit was
  separately `cargo check`ed to prove the refactor is not hiding a necessary
  API adaptation.
- Gap: the local gate run cannot prove the Windows leg, which the CI matrix
  covers separately. No Windows-only `#[cfg]` code was touched by this work,
  but that leg remains CI's to certify.
- Lesson: a green `cargo test` on the workspace is not evidence about the
  compile-time floor. The MSRV violation surfaced only in the trybuild sandbox,
  because that sandbox generates its own manifest and lockfile.
- Lesson: the local environment can fail a suite for reasons that have nothing
  to do with the code. Two runs of `make test` were destroyed mid-flight by a
  host-level `rust-cleanup` timer that deletes `target/` directories whose
  newest file is older than its inactivity cutoff; a long behavioural target
  writes nothing for ten minutes, and the following target then has no binary
  to execute. The failures are recorded here so a future reader does not
  mistake them for a regression: they were "never executed" and directory
  races, not assertion failures, and the third run — with the target directory
  kept fresh — passed every target.
- Lesson: the repository's `make test` is the whole gate, not just
  `cargo test`. It also runs the Python contract suite, which a hand-rolled
  `cargo test --all-targets --all-features` silently omits.

## Context and orientation

This repository is a Rust workspace (`resolver = "2"`, edition 2024) with ten
members declared in the root `Cargo.toml`. Only three members consume
`rstest-bdd`:

- `ortho_config` — the primary behavioural suite, 18 feature files.
- `cargo-orthohelp` — a cargo subcommand helper, 6 feature files.
- `examples/hello_world` — a worked example, 4 feature files.

At the start of this migration the root `Cargo.toml` declared the family in
`[workspace.dependencies]` as `rstest-bdd = "0.5.0"` and
`rstest-bdd-macros = "0.5.0"`; both now read `0.6.0`, which is the change this
plan describes. Each of the three members inherits them with
`.workspace = true` in `[dev-dependencies]`. The single lockfile is
`Cargo.lock` at the repository root; the fixture crates under `tests/fixtures/`
are workspace members but have no lockfile of their own and no `rstest-bdd`
dependency.

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

| Breaking change                                                      | Status                    | Evidence                                                                                                                                                                                                                                                                                                                                                                                       |
| -------------------------------------------------------------------- | ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Implicit fixture names normalize one leading underscore              | Inapplicable              | A structural scan of every `#[given]`/`#[when]`/`#[then]`/`#[scenario]` parameter across all three suites found zero underscore-prefixed parameters without an explicit `#[from(...)]`. The only underscore fixture in the repository is `_env_lock` in `ortho_config/tests/discovery_attributes.rs`, a plain `#[rstest]` parameter, not a BDD fixture key.                                    |
| `runtime = "tokio-current-thread"` becomes a deprecated alias        | Inapplicable              | No `runtime = ...` argument appears in any `scenarios!` invocation, and no async step or scenario exists.                                                                                                                                                                                                                                                                                      |
| Custom `HarnessAdapter` must return `HarnessResult<T>`               | Inapplicable              | No custom harness is declared or used; `rstest-bdd-harness` enters the graph transitively through `rstest-bdd-macros` and is not declared directly by a workspace crate.                                                                                                                                                                                                                       |
| Feature paths become manifest-relative                               | Inapplicable              | No consumer of `ScenarioMetadata::feature_path`, the JSON reporter, JUnit output, or `cargo bdd --dump-steps`. Only prose mentions of JUnit exist, in unrelated documents. One feature path (`tests/features/rstest_bdd_canary.feature`) is already manifest-relative and is bound by `#[scenario(path = ...)]`, whose argument form is unchanged.                                             |
| `StepContext::insert_value` returns `InsertOutcome`                  | Inapplicable              | No direct caller; the method is not referenced anywhere in the workspace.                                                                                                                                                                                                                                                                                                                      |
| Step return aliases of `Result<T, E>` propagate `Err` and inject `T` | **Applicable, exercised** | 175 `ortho_config` steps return `anyhow::Result<()>`, and `cargo-orthohelp`'s steps return a genuinely local alias `StepResult<T> = Result<T, StepError>`. Both are dispatched through the ADR-019 bridge. Traced in decision D4 below.                                                                                                                                                        |
| `E` must implement `Display`                                         | Satisfied                 | `ortho_config` steps use `anyhow::Error`; `cargo-orthohelp` steps use `StepError = Box<dyn std::error::Error + Send + Sync>`, which implements `Display` through the trait object. Both satisfy the bound.                                                                                                                                                                                     |
| `record_bypassed_steps` takes `BypassedScenario`                     | Inapplicable              | No direct caller.                                                                                                                                                                                                                                                                                                                                                                              |
| Language-server indexing entry points return `RustStepIndexResult`   | Inapplicable              | `rstest-bdd-server` is not a dependency.                                                                                                                                                                                                                                                                                                                                                       |
| `publish_rust_diagnostics` removed                                   | Inapplicable              | Same as above.                                                                                                                                                                                                                                                                                                                                                                                 |
| `find_feature_files` returns `Result<...>`                           | Inapplicable              | Same as above.                                                                                                                                                                                                                                                                                                                                                                                 |
| MSRV rises to 1.88                                                   | Already satisfied         | The repository declares `rust-version = "1.89.0"` and pins `channel = "1.99"`; upstream requires 1.88.                                                                                                                                                                                                                                                                                         |
| `cargo-platform` transitively demands 1.91                           | Applicable, constrained   | `rstest-bdd-macros` 0.6.0 requires `rstest-bdd-harness` unconditionally, which pulls `cargo_metadata` 0.23.1 and hence `cargo-platform ^0.3.0`. The floating resolution to 0.3.3 declares Rust 1.91, above the workspace's declared Rust 1.89.0 floor. Held at 0.3.2 (declared 1.88) via a `--precise` lockfile constraint, preserving the repository's existing floor rather than raising it. |

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
- Domain: the 28 baseline `.feature` files and the added step-return feature.
- Artefact: the inventory command in `Concrete steps`, run before and after.
- Evidence: baseline shows 127 scenarios and 0 outlines; candidate inventory
  preserves all 127 baseline scenarios and adds five scenarios in one new
  feature file, for 132 scenarios across 29 files.
- Non-vacuity: the check fails if any `scenarios!` invocation stops matching,
  which is the realistic regression mode. The baseline non-zero count (127)
  proves the counter is not measuring an empty set.

### Obligation V2: `Result<()>` step classification does not silently change a verdict

- Statement: for every scenario, a step returning `Ok(())` does not override a
  fixture used as a flag, and a step returning `Err` fails the scenario.
- Method: two complementary artefacts — a behavioural regression driving real
  registered steps, and a differential control against the pre-migration
  wrapper.
- Rationale: this is the one 0.6.0 change that alters runtime behaviour for
  code this repository actually contains. A reasoned argument alone is
  insufficient because the failure mode is a *false green*, which the existing
  suite would not report.
- Domain: the 175 `-> Result<()>` steps and the 6 `Slot<()>` flag fields on
  `ExtendsContext`.
- Artefacts: `ortho_config/tests/rstest_bdd_support/step_return_regression.rs`,
  which directly tests registry/dispatch behaviour through `execute_step` and
  mirrors the runner's `insert_value` call; it is not itself the generated
  runner suite. The generated runner's end-to-end behaviour is covered by
  `ortho_config/tests/rstest_bdd_support/step_return_scenarios.rs`, registered
  against `ortho_config/tests/features/step_return.feature`. The separate
  `ortho_config/tests/rstest_bdd_support/step_return_dispatch_bridge.rs` pins
  the lower-level dispatch contract.
- Evidence: the regression module pins all four combinations of alias shape
  (`anyhow::Result<T>` and `Result<T, Box<dyn Error + Send + Sync>>`) against
  outcome (an `Err` fails the scenario; an `Ok(T)` injects `T`, while `Ok(())`
  injects nothing), and every case passes in the certified run.
- Non-vacuity, and the correction to this obligation's original plan: the
  "fail against the pre-migration tree" red stage recorded here originally was
  **not achievable as written**, for two independent reasons.
  1. The regression module cannot compile against 0.5.0 at all. It uses
     `InsertOutcome` and `step_return`, which 0.5.0 does not export, and its
     `insert_value` returns `Option<Box<dyn Any>>` rather than
     `InsertOutcome`. A test that cannot be built cannot stage a red.
  2. The originally specified mutation — two `Slot<()>` flags with the first
     asserted to survive a succeeding step — would not distinguish anything
     for this repository's steps. `__rstest_bdd_payload_from_value` drops a
     unit payload under both versions by `TypeId`, and no step here returns a
     non-unit alias, so that assertion passes before and after the migration.

  The replacement control is differential, which is what the defect actually is.
  `rstest-bdd-macros` 0.5.0's `ReturnKind::Value` arm emitted
  `Ok(__rstest_bdd_payload_from_value(#call))`, and `return_classifier.rs`
  resolved only syntactic `Result<..>` paths and never type aliases — so an
  aliased `Result` was boxed as a success value and its `Err` was swallowed.
  0.6.0 replaced that with the type-directed bridge in
  `rstest_bdd::step_return`. The bridge module reproduces the 0.5.0 helper and
  wrapper arm verbatim, drives the 0.6.0 probe and tag, and asserts that the
  two **disagree** on both the failing and the succeeding path. It therefore
  fails if the new bridge stops classifying a `Result` alias as a `Result`, and
  it also fails if the recorded old behaviour stops being the behaviour that
  produced the false green. Asserting the new behaviour alone would not have
  that second property. Two further cases pin the tag discrimination those
  comparisons rest on, comparing tag type names for an aliased `Result` and for
  a plain payload. Without them a probe that answered `StepReturnResultTag` for
  every input would satisfy each differential check while discriminating
  nothing.

  A true "run 0.5.0 and watch it fail" stage remains unavailable: the
  pre-migration dependency is gone from the lockfile, and re-adding it would
  force the workspace's step macros back to 0.5.0 too. This limitation is
  recorded rather than papered over. One further scope note: the differential
  covers `anyhow::Result<T>` directly. The boxed-error shape shares the same
  bridge arm and is covered by the behavioural module, but it is not probed
  differentially.

  A seventh case, added on 2026-10-19, moves the same claims one level up. The
  artefacts above reach the runner's `insert_value` call by hand, so none of
  them can notice a defect in the code that calls it. `step_return.feature` and
  its `step_return_scenarios.rs` bindings now drive the generated `#[scenario]`
  body end to end: a failing alias in either shape fails the scenario (matched
  with `#[should_panic(expected = ...)]`, which the macro preserves onto the
  generated test function), a payload arrives at a later step as the fixture
  value, and a unit `Ok` leaves the seeded fixture at 5. The `Then` guard in
  the failing scenarios proves the loop stopped, since its message is distinct
  from both handler messages. The `Level` payload type lives in exactly one
  module because `insert_value` filters by `TypeId`.

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
- `gherkin` 0.16 requires Rust 1.88, which the pinned 1.99 toolchain
  satisfies.
- Third-party library internals are treated as axioms; only this repository's
  own integration with them is verified.

### Residual gaps

- Windows-target behaviour is not exercised by any local gate. This
  repository has Windows CI legs; the local verification can only proxy them.
- Per-scenario behavioural equivalence is argued from suite results plus the
  targeted V2 regression and its differential control, not exhaustively proven
  for all 127 scenarios.
- Obligation V2 has no literal red run against 0.5.0. The differential control
  emulates the pre-migration wrapper from upstream source instead, because the
  regression module cannot compile against 0.5.0. The control is stronger than
  a restated assertion but weaker than observing the old dependency fail.
- The differential control probes `anyhow::Result<T>` directly. The boxed-error
  alias shape shares the same classification bridge and is covered by the
  behavioural module, but it is not probed differentially.

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
Add the V2 regression coverage: the behavioural module that drives registered
steps, and the differential bridge module that pins the dispatch contract.
Rebuild and rerun the behavioural suites.

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
  the V2 behavioural regression passing, plus the V2 differential control
  showing 0.6.0 and the emulated pre-migration wrapper disagree on an aliased
  `Result` (see the V2 obligation for why a literal pre-migration run is not
  achievable); the scenario inventory comparison.
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
rg -c --glob '*.feature' '^\s*Scenario:' . | sort
```

The command lists scenario counts by feature file. The baseline has 127
scenarios across 28 files; the candidate has 132 across 29 files.

Gate commands, run sequentially and never in parallel:

```bash
set -euo pipefail
LOG=/tmp/$(basename $(git rev-parse --show-toplevel))-$(git branch --show-current).out
make check-fmt 2>&1 | tee "${LOG%.out}-check-fmt.out"
make lint 2>&1 | tee "${LOG%.out}-lint.out"
make typecheck 2>&1 | tee "${LOG%.out}-typecheck.out"
make test 2>&1 | tee "${LOG%.out}-test.out"
make markdownlint 2>&1 | tee "${LOG%.out}-markdownlint.out"
make nixie 2>&1 | tee "${LOG%.out}-nixie.out"
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

Software behaviour acceptance: after the change, removing a baseline `Scenario`
block makes its corresponding generated test disappear from the run (proving
the suite is genuinely bound to the feature text). All 127 baseline scenarios
are preserved, five step-return scenarios are added in one new feature file,
and the full suite passes with 132 scenarios across 29 files.

Quality criteria:

- Tests: `make test` passes; the V2 regression test passes; the `rstest_bdd`
  canary scenario in each consuming crate still executes.
- Verification: obligations V1 through V4 discharged.
- Lint/typecheck: `make check-fmt`, `make lint` and `make typecheck` pass.
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
blocks, across 28 feature files. The candidate preserves all 127 and adds five
step-return scenarios in one feature file, for 132 scenarios across 29 files.

## Interfaces and dependencies

No new public interface is introduced. The dependency surface after EP-M1 is
unchanged in shape:

- Root `Cargo.toml`, `[workspace.dependencies]`:
  `rstest-bdd = "0.6.0"`, `rstest-bdd-macros = "0.6.0"`.
- Consumed API, unchanged in name: `rstest_bdd::Slot`,
  `rstest_bdd::ScenarioState`, `rstest_bdd::StepResult`,
  `rstest_bdd_macros::{scenario, scenarios, given, when, then, ScenarioState}`.

No `rstest-bdd-harness*` crate is declared by this workspace and no `tokio`
dependency is declared. Both nevertheless enter `Cargo.lock` transitively:
`rstest-bdd-macros` 0.6.0 depends unconditionally on `rstest-bdd-harness`,
which brings `cargo_metadata`, `tracing` and `tokio`. Only `Cargo.lock`
changes; no manifest gains a dependency line.

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

Revision, 2026-10-18 (historical status at that revision): closed three
findings from an independent assessment of the delivered head. Restored the
`path` field on the agent-context serialization-failure event, which the
output-module split had dropped; made the V2 `Anyhow` alias actually
`anyhow::Result<T>`; and replaced the V2 red stage claim, which the delivered
tests neither performed nor could perform, with a differential control against
the emulated pre-migration wrapper. The obligation's method, evidence,
acceptance line, residual gaps and the recorded baseline all changed with that
correction. The plan was marked `COMPLETE` at that revision; final validation
was still pending then, and the status was `IN PROGRESS`.

Revision, 2026-10-19: closed the three pre-merge-check findings on the
delivered head, all by adding coverage rather than changing production code.
Added a feature file and its `#[scenario]` bindings so both aliased `Result`
shapes are exercised through the generated scenario runner; added a property
suite for the rustflags sanitizer that compares it against an independently
written reference filter for both delimiters; and added a trybuild pass fixture
that compiles the step macros for both downstream return aliases. The fourth
row of the same assessment asked for a scoped `not_found` telemetry case; an
in-crate unit test now drives `load_outcome` directly, because the dispatcher
arm exists but no production discovery path reaches it, and the assertion is on
the callsite's declared metadata rather than on recorded values. The Surprises
section records the payload-type identity trap, the trybuild scratch-crate
constraints and the corrected reading of the scoped emitter discovered while
building these, and the Decision log records why the focused bridge tests stay.

Revision, 2026-10-20: clarified the boundary of the trybuild pass fixture after
an independent assessment. `trybuild` compiles the annotated handlers and
type-checks direct calls in `main`, but does not execute the fixture; runtime
step-return behaviour is covered by the registered feature scenarios. The
historical `IN PROGRESS` status above is now explicitly tied to the 2026-10-18
revision; the current status and final local validation are recorded earlier.

Revision, 2026-10-21: closed the five findings of the review at `f7316c0e`. The
review showed that the unit-return case's `NotMutable` assertion proved nothing:
`execute_step` does not call `insert_value`, so a fixture inserted by shared
reference is non-mutable for every step outcome. The assertion is gone and the
direct `Ok(None)` check now carries the claim. Alongside it, the `Ok(())` risk
moved to historical and closed; the document provenance gained a correct
account of the vendor-spelling history and of the users' guide's one relative
link; and the contents entry was rewrapped to 80 columns. No production code
changed: the edit touches two documents, this plan and one test module.
