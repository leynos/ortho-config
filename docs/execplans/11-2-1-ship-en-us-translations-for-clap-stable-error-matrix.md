# Ship the en-US clap error matrix (11.2.1)

Status: DRAFT — awaiting explicit approval before implementation.

This ExecPlan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current at every milestone.

## Purpose / big picture

Consumers will be able to enumerate every supported clap error kind, identify
which Fluent message to override, and receive embedded en-US text for every
non-display error. Help and version requests will pass through untouched. A
compile-time coverage assertion will fail when the resolved clap enum grows
without a corresponding matrix entry. This is roadmap task 11.2.1 only.

The deliverable of this branch is this reviewed plan, not the feature. Approval
of the plan authorizes the milestones below, including the bounded feasibility
investigation; it does not authorize replacing the design's coverage mechanism
if that investigation fails.

## Constraints

Implement [CLI localization design](../cli-localization-design.md) §§6.1,
6.1.1, and 6.2. The coverage gate must bind the total declared-variant count to
a supported `clap_builder` package identity (version, source identity, and
checksum), verify that the compiled `clap` facade's dependency edge reaches
that exact package, and fail closed for unknown, overridden, or ambiguous
identities. Only then may the build script emit the manifest's total declared
variant count and the test's `const_assert_eq!` compare it with the matrix
length. The build script does not classify variants; translated versus
display-only is represented solely by the exhaustive public matrix. Never use
an unbound hard-coded count, registry-directory glob, enum discriminant
maximum, or a second independently resolved dependency as the authoritative
inventory.

Preserve the four existing Fluent identifiers and their argument contracts.
Ship en-US strings unconditionally. Add the public enum and constant through
both existing export layers. Use the matrix as the runtime mapping authority;
all three display-only entries bypass lookup and return the original error.
Future unrecognized kinds retain the current fallback path until the coverage
failure is repaired. Do not implement the coverage-report builder, richer clap
context preservation, locale lifecycle, embed bridge, or derive work. Those
belong to other roadmap entries.

Keep the declared MSRV, Rust 1.89, and validate with the workspace toolchain in
`rust-toolchain.toml` (currently Rust 1.99); retain caret dependency
requirements. Do not change the toolchain, compiler admission, shared Cargo
cache, or Whitaker installation. Use capability-oriented filesystem access and
UTF-8 paths in new build tooling. New Rust files must have module documentation
and remain at most 400 lines. Do not add runtime dependencies for build-time
source inspection.

Implementation uses test-first development. Do not commit an intentionally
failing red stage. Every code milestone must pass sequential `make check-fmt`,
`make test`, `make typecheck`, and `make lint` before its commit. Use the
scrutineer agent for full gates and keep logs under `/tmp`; build products stay
under `target/`.

## Tolerances (exception triggers)

Stop implementation and set this plan to BLOCKED if the active dependency
source cannot be identified without guessing, if the build probe requires a
network request or recursive compilation, or if ordinary downstream builds
require an undocumented user-supplied path. Record the failed assumption,
options, and upstream design impact, then obtain explicit acceptance before
changing the mechanism. A workspace-only success is insufficient.

Escalate public APIs beyond the requested enum and constant, changes outside
the runtime localizer/build support/tests and their documentation, toolchain
changes, or dependencies outside the build/test candidates listed below. There
is no time limit. Infrastructure failures are blocked validation, not
permission to weaken a gate. Disk exhaustion requires stopping and reporting.

## Risks

- High impact, unresolved likelihood: the identity manifest may not describe
  every Cargo source form that consumers use, or the resolver may not prove
  that the compiled facade reaches that exact builder. M0 must establish the
  supported source boundary and fail-closed behaviour before build integration.
- High impact, medium likelihood: equal matrix and enum lengths can conceal a
  duplicate plus an omission. Test uniqueness and exact inventory agreement.
- Medium impact, high likelihood: clap does not supply every context variable
  for every error construction. Inventory real contexts before writing FTL;
  generic errors must render without unresolved Fluent variables.
- Medium impact, medium likelihood: count output becomes stale after a lock,
  source, patch, or Cargo configuration change. Bind it to the resolved
  identity and exercise rerun invalidation; never reuse stale output.
- Medium impact, medium likelihood: changing the missing-argument help variant
  to pass-through affects existing BDD expectations. Update those expectations
  deliberately while retaining actual `MissingSubcommand` translation tests.

## Progress

- [x] (2026-10-14) Read repository guidance and requested skills; renamed
      branch.
- [x] (2026-10-14) Gathered CodeGraph and Wyvern source/documentation evidence.
- [x] (2026-10-14) Checked Cargo and clap documentation through Firecrawl.
- [x] (2026-10-14) Completed six-perspective expert review and incorporated
  its fixture, identity, failure-diagnostic, and invalidation findings.
- [x] Publish this plan in draft PR #588.
- [x] (2026-11-08) Rebase the published draft plan on PR #651 (`ddba3578`) and
      refresh its baseline and integration contracts.
- [x] (2026-11-09) Rebase onto the timeout-fix branch for PR #664.
- [x] Retarget draft PR #588 to `issue-644-nested-cargo-timeouts`.
- [x] Complete fresh formatting, lint, typecheck, and full test validation on
  parent `501ab831`.
- [x] Validate documentation and workflow contracts; prepare the revised plan
  for publication.
- [ ] Obtain explicit implementation approval.
- [ ] M0: establish identity-manifest and facade-edge feasibility.
- [ ] M1: ship canonical matrix, strings, and runtime integration.
- [ ] M2: ship mechanical count gate and fault-detection evidence.
- [ ] M3: finish consumer documentation and mark roadmap task done.

## Surprises & discoveries

The plan first incorporated merged PR #651 at
`ddba3578fae04227e426ea3a61aa2b24aa043232`. On 2026-11-09 it was restacked onto
the timeout-fix branch for PR #664 at
`501ab83134eaba0f9e97e1daeaa29c793a819c53`. The current workspace lockfile
resolves clap_builder 4.6.7, but the plan must resolve and match the supported
identity at implementation time rather than treating this snapshot as a
consumer-wide constant. The design's 4.6.0 identity is an illustrative example,
not this lockfile's current resolution; reconcile that wording during M0. The
current upstream API inventory has 17 variants; M0 will bind that count to the
identity manifest and verify the compiled facade edge.

The requested nine additions plus four existing entries total thirteen.
`InvalidSubcommand`, explicitly named by §6.1, is also required: ten new
translations produce fourteen translated entries and three sentinels.

The sample `.parse().unwrap()` inside a const initializer in §6.1.1 is not an
executable const recipe. Emit a numeric Rust constant in `OUT_DIR` alongside
the required environment value, both derived from the same validated manifest
entry; do not create a speculative decimal parser merely to reproduce the
illustration.

PR #617 completed the reusable `ortho_l10n` extraction; PR #651 added pinned,
opt-in Kani/Verus tooling. The Fluent runtime and embedded catalogues now live
in `ortho_l10n`; the Clap adapter, exhaustive matrix, and Clap-specific build
gate remain in `ortho_config`. The prover tooling's integration fixtures
validate the runner and its negative controls, not this feature's production
claims. This finite enum/catalogue mapping does not warrant a Verus proof.

`DisplayHelpOnMissingArgumentOrSubcommand` currently maps to the missing
subcommand message. The new sentinel contract requires pass-through instead.
The current formatter reconstructs translated errors and preserves only a
rendered tail. Rich structured context preservation remains task 11.2.2.

## Decision log

- 2026-10-14: retain the prescribed build-script mechanism, with M0 as an
  explicit feasibility decision. The original source-resolution approach was
  not proven and is superseded by the identity-manifest contract below.
- 2026-11-08: a count is authoritative only when bound to a supported package
  identity and the compiled facade edge; unknown identities fail closed.
- 2026-10-14: include `InvalidSubcommand` and all three sentinels, following the
  complete-matrix requirement and the design rather than an incomplete list.
- 2026-10-14: use finite exhaustive tests for the matrix and property tests for
  source-count invariance. No Verus milestone is warranted for this bounded
  catalogue lookup and integration plumbing; reasoning and its limits follow.
- 2026-10-14: add no compatibility wrapper. Existing identifiers remain stable
  as required by the localization contract; the crate is pre-1.0.

## Outcomes & retrospective

The plan incorporates the merged prover tooling from PR #651 and is now based on
[PR #664](https://github.com/leynos/ortho-config/pull/664), which addresses
nested Cargo timeouts that blocked validation on main. Draft PR #588 now
targets that branch. Formatting, lint, typecheck, and full tests passed on the
rebased candidate. Documentation and workflow-contract checks also passed; the
revised plan is ready for review. No feature implementation has occurred;
explicit approval is still required before M0. The primary unresolved
engineering risk remains proving that the manifest identity matches the enum
exposed by the compiled Clap facade.

Earlier validation on main hit inherited 240-second packaging and 300-second
consumer deadlines. On the initial PR #664 parent (`26275a4e`), the ordinary
formatting, typecheck, lint, Markdown, diagram, and workflow-contract gates
passed. Packaging preparation with verification enabled and matching Makefile
flags allowed `make boundary` to pass 32 tests, with none failed and one
ignored. The subsequent full `make test` still failed two workspace consumers
at the 440-second deadline: 30 boundary tests passed and one was ignored.
Retained output showed dependency compilation and compiler-admission waits.
Neither an earlier exit-143 run nor that focused pass constitutes a full
test-gate pass.

The parent subsequently added whole-process-tree cleanup, bounded reader
collection, and an operating-system lease for the identifier fixture target.
Replay of the plan commit onto `501ab831` was conflict-free and unchanged under
`git range-diff`. Those implementation changes belong to PR #664. The current
candidate preserves them intact. Fresh `make fmt`, `make check-fmt`,
`make lint`, `make typecheck`, and `make test` passed. The full test run
included 35 passing boundary tests, none failed, and seven ignored; all three
workspace consumers passed. Python tests reported 119 passed and 16 skipped. No
deadlines, assertions, compiler-admission settings, or shared Cargo-cache
policy were changed by this plan revision. `make markdownlint`, `make nixie`,
and `git diff --check` passed; `make test-workflow-contracts` passed 394 tests
with two skipped. No validation exception is needed.

### Temporary PR base and return to main

Target draft PR #588 at `issue-644-nested-cargo-timeouts` until PR #664 merges.
The inherited parent head is `501ab83134eaba0f9e97e1daeaa29c793a819c53`;
preserve that replay boundary when publishing this stack. Only the plan commit
and its revisions belong to this branch above that boundary. Fetch the parent
and main again before implementation; any further restack requires validation
of the resulting candidate.

After PR #664 merges, the implementing agent can rebase this branch onto a
freshly fetched `origin/main` and retarget the PR to `main`. First verify the
parent's merge and landing, record the current remote head for an explicit
force-with-lease, and preserve recovery refs. Use the `rebase` skill to replay
only this branch's commits above the inherited parent boundary; do not replay
the timeout fix again if GitHub squash-merges it. Review any conflicts, retain
the parent fix, run the ordinary gates, then publish with the recorded lease.
If the parent is rewritten before merging, reconcile its old and new history
before choosing a replacement boundary. Returning to main does not grant
implementation approval.

## Context and orientation

`ortho_config/src/localizer/clap_error.rs` owns `message_id`,
`localization_args`, and `localize_clap_error_with_command`. It currently
synthesizes identifiers from enum debug names, with exceptions for the four
shipped messages. `ortho_l10n/locales/en-US/messages.ftl` is the embedded
English catalogue, and `ortho_l10n` owns Fluent formatting, catalogue
embedding, and shared localization types. `ortho_config/src/localizer/mod.rs`
is the compatibility facade and owns the Clap adapter;
`ortho_config/src/lib.rs` exposes the crate-level API. The standalone
`ortho_l10n` runtime must not gain a Clap dependency.

`ortho_config/src/localizer/tests.rs` already tests Clap fallback, captured
context, and rendered tails; Fluent runtime tests live in
`ortho_l10n/src/tests.rs`. Extend coverage in separate focused modules rather
than growing a file beyond the repository limit. Behavioural tests use
`ortho_config/tests/features/localizer.feature`, steps in
`ortho_config/tests/rstest_bdd_support/behaviour/steps/localizer_steps.rs`, and
the `ortho_config/tests/rstest_bdd.rs` target. Check scenario registration when
adding cases. There is no current `ortho_config/build.rs`. Existing Cargo
metadata parsing helpers under
`ortho_l10n/tests/boundary/fixture_cargo/metadata.rs` belong to boundary-test
fixtures; inspect their scope before reusing or extracting them.

Fluent is the message format; a catalogue identifier names a message, and its
variables receive clap context. A sentinel is an enum value indicating that
clap itself owns display. Cargo metadata describes resolved package identities
and dependency edges; a package name alone is not an identity. `fluent-syntax`
is owned by `ortho_l10n`; keep syntax and formatting assertions there unless a
specific `ortho_config` integration test justifies an explicit dev dependency.

## Conformance basis

This plan is based on the roadmap and design files at the baseline commit
above. No separate Terms of Reference governs this task. Roadmap 11.1.1 is
checked complete and its promoted command-localization API exists. Recheck that
prerequisite and the live source after approval or any rebase.

Trace the three requirements as follows:

- R1, design §6.1 and roadmap 11.2.1 strings: M1, fourteen catalogue-rendering
  cases and CLI tests; M3, consumer documentation.
- R2, design §6.2 exhaustive public matrix: M1, independent inventory,
  uniqueness, public-import, override, and pass-through tests.
- R3, design §6.1.1 total-count mechanism: M0 feasibility evidence; M2 exact
  source identity, compile-time assertion, and deliberate count-drift failure.

Read these references before implementation; they own the surrounding rules:

- [Core design](../design.md) and
  [CLI localization design](../cli-localization-design.md): localizer
  ownership, fallback semantics, matrix, and the authoritative gate.
- [cargo-orthohelp design](../cargo-orthohelp-design.md),
  [clap-dispatch integration][dispatch],
  [agent-native CLI design](../agent-native-cli-design.md), and
  [improved errors](../improved-error-message-design.md): adjacent consumers
  and contracts; this task does not redesign their schemas or dispatch.
- [rstest fixtures](../rust-testing-with-rstest-fixtures.md),
  [doctest guidance](../rust-doctest-dry-guide.md),
  [dependency injection][injection], and
  [rstest-bdd guide](../rstest-bdd-users-guide.md): fixture ownership,
  executable API examples, injected inputs, and scenario registration. The
  requested `rtest-bdd-users-guide.md` spelling has no file; this is its actual
  repository path.
- [Fluent libraries](../localizable-rust-libraries-with-fluent.md),
  [complexity guidance][complexity], for fallback and small modules; the
  [scripting standards](../scripting-standards.md) govern repository scripts.
- [ADR 0015](../adrs/adr-0015-localization-runtime-boundary.md) and the
  [users' guide](../users-guide.md) describe the extracted runtime boundary;
  keep Clap-specific APIs in `ortho_config`.
- [ADR 0017](../adrs/adr-0017-consumer-side-prover-tooling.md), the
  [Kani/Verus tooling plan](import-prover-tools.md), and the
  [developers' guide](../developers-guide.md#kani-and-verus-verification-tools)
  describe optional proof tooling. Ordinary gates do not install or run provers.
  `FORMAL-EMPTY` is a non-success result, and tooling fixtures are not feature
  proofs. If later evidence warrants production verification, register
  production sources, their proof/refinement relationship, inputs and bounds,
  assumptions, expected check, and a negative control; run the exact registered
  proof gate. Do not use the import plan's fixtures as proof of this feature.
- [Documentation style](../documentation-style-guide.md),
  [contents](../contents.md), and [layout](../repository-layout.md): document
  placement and test target registration.

Use the `execplans`, `codegraph-mcp`, and `rust-router` skills for planning and
Rust design; follow the router to `rust-unit-testing`, `rust-verification`, and
`proptest` for the relevant verification work. Use `firecrawl-mcp` for
unresolved external contracts, `logisphere-experts` for design review,
`commit-message` for commits, and `pr-creation` for publication. Reuse existing
metadata helpers only if their boundary-test ownership permits it; do not
introduce a dependency from the standalone localization runtime to Clap or from
runtime code to an executable. Before any helper extraction, search for an
equivalent and document ownership and permitted reuse in the developers' guide.
Build-probe helpers belong only to build support and its tests.

## Verification plan

### Model, preconditions, and representation

Let E be the finite set of declared variants in the exact dependency enum
re-exported by the compiled clap facade. Let M be the sequence of public matrix
pairs. Each pair's first component is a real `ErrorKind`; its second is either
`Translated(id)` or `DisplayOnly`. Let D be the three display variants listed
in §6.1. The validated manifest count N must equal the cardinality of E.

The required postconditions are: each member of E occurs exactly once in M;
exactly D receives the sentinel; every other row names a unique, renderable
embedded English message; runtime lookup agrees with M; and display-only errors
return unchanged without consulting the localizer.

The finite coverage argument is that distinct keys drawn from E, together with
`len(M) == N == |E|`, cover E. Length equality alone lacks the distinctness
premise. This is a review argument discharged for the concrete inventory, not a
claim of a machine-checked theorem. No contractual business algorithm or
unbounded semantic kernel is introduced, so Verus, Kani, and solver scaffolding
would not improve the relevant assurance. If implementation introduces such a
kernel, stop and revise the specification and architecture before selecting
Verus; never use `assume`, `admit`, or trusted bodies to bypass a proof.

Axioms are Cargo's package-resolution and build-output contracts, clap's
published type and context contracts, Rust's typed enum constructors and const
evaluation, and Fluent's parser/formatter contract. Tests exercise our use of
these boundaries; they do not purport to prove those dependencies correct.

### V1: exact inventory and uniqueness

Add `ortho_config/tests/clap_error_matrix.rs` with an independently enumerated
`rstest` table for the exact supported `clap_builder` identity. Check pairwise
key uniqueness, identifier uniqueness, exact expected kind membership, and the
sentinel partition. Compare the table with the identity's declared count in M2.
Compile imports through both `ortho_config` and `ortho_config::localizer`. Use
`googletest` matchers for variants and collections and `pretty_assertions` for
equality.

The witnesses include every real kind and all three sentinels. Replace a row
with a duplicate while keeping length unchanged: the uniqueness/inventory test
must fail. Remove a row: the M2 const assertion must fail. These controls
prevent a shared production/test omission from looking exhaustive.

### V2: messages, overrides, and missing context

Keep catalogue syntax and Fluent rendering assertions with `ortho_l10n`, which
owns `fluent-syntax` and the embedded resources. In
`ortho_config/tests/clap_error_translations.rs`, verify that every translated
matrix identifier has an actual embedded message rather than accepting fallback
as evidence; add an explicit `fluent-syntax` dev dependency only if this
cross-crate test needs to parse the catalogue itself. Render each identifier
with a real `FluentLocalizer` and representative arguments; require no
formatting issues and nonempty output. Exercise consumer overrides, en-US
fallback from an incomplete locale, and unchanged stock errors with
`NoOpLocalizer`.

Use real clap parsing to produce each reachable parser error and assert its
kind before testing the translation. Use explicit `Error::raw` constructions for
`Io` and `Format`, which need no argv parser reproducer. Test context-free
errors as well: never emit unresolved variables or lose the original diagnostic
on a missing-argument formatting failure. Keep required-context translations on
the existing fallback path when information is absent. Prefer context-free new
strings when clap cannot supply a stable variable contract. Inventory and
record each new string's variables before implementing it.

Use `insta` snapshots for the finite, colour-disabled rendered error matrix,
including the existing usage tail. Snapshot review must be paired with semantic
assertions for kind and selected identifier. Delete one FTL message and alter
one mapping as negative controls; direct catalogue and override tests must fail
even if fallback happens to look like valid English.

### V3: display-only and CLI integration

Use a recording localizer that would return conspicuous replacement text. For
all three display variants, assert zero lookups, unchanged text, kind, exit
code, stream selection, and context. Include both raw unit witnesses and real
clap help/version/missing-input paths. This exposes the existing bug in the
third variant instead of testing pass-through with a no-op localizer.

Extend `localizer.feature` with these scenarios and register their steps:

```gherkin
Feature: Complete clap error localization
  Scenario: A newly covered parser error is translated
    Given a localized command requiring equals for its option
    When I parse that option without equals
    Then the error kind is NoEquals
    And the en-US no-equals translation is rendered

  Scenario: Missing-input help remains clap output
    Given a localized command showing help when arguments are absent
    When I parse no arguments
    Then the error kind is DisplayHelpOnMissingArgumentOrSubcommand
    And the localizer is not called
    And the output and exit status equal stock clap

  Scenario: An unavailable override retains usable English
    Given an incomplete locale and the embedded en-US catalogue
    When I trigger a newly covered parser error
    Then the embedded en-US translation is rendered without formatting errors
```

Add a small subprocess fixture in `tests/fixtures/clap_error_matrix/` and a
runner `ortho_config/tests/clap_error_cli.rs` to exercise stdout, stderr, and
exit status for help, version, missing-input help, and a translated parser
error. Keep fixture compilation sequential and outside `/tmp`; inject locale
and colour settings through the child process rather than mutate global env.

### V4: identity-manifest correctness and freshness

Test the manifest parser/generator independently from the build-time runtime
identity check. If a generator derives count data from source, use `syn` rather
than regex and count every declaration, including display shapes, without a
classification list. Use `proptest` for bounded syntax cases: insertion adds
one declaration; reordering, comments, attributes, and discriminants do not
change the count; malformed or ambiguous declarations fail. If the design's
manifest is wholly checked in rather than generated, apply property tests only
to any actual parser/normalizer introduced, not to a speculative source parser.

The decisive controls distinguish identity from count. An unsupported or
source-overridden identity must fail closed before emitting a count, even if
its source appears to match. For a supported identity, change only its manifest
count while leaving the matrix unchanged and require the `const_assert_eq!`
failure; restore it, then update the manifest and matrix together and require
the semantic inventory tests to pass. A mismatched version, source identity,
checksum, or `clap` facade edge must fail before the assertion. Change an input
used by the resolver and show that Cargo reruns the gate. Never claim that an
unsupported path-patched source reaches the const assertion. Restore all
controls before gates and commits.

## Plan of work and milestones

### M0: establish identity-manifest feasibility after approval

Read current source, dependency graph, and relevant Cargo documentation again.
Use disposable fixtures under `target/` to establish how the build script
identifies the resolved `clap_builder` package, matches its version/source/
checksum against the supported count manifest, and verifies the resolved `clap`
facade edge points to that exact package. Determine which Cargo source forms
can be resolved deterministically (for example the registry source identity);
unsupported source replacements, vendored sources, path patches, ambiguous
graphs, or unknown checksums must fail closed unless the identity contract can
be proved for them. Do not silently fall back to the workspace lockfile or a
package-name match.

Record commands and evidence for the workspace build, an external path
consumer, and an unpacked packaged consumer. Exercise renamed dependencies,
feature selection, target-specific edges, multiple Clap versions, and
source-override cases to establish supported and rejected boundaries. A
packaged consumer must build the unpacked Cargo archive outside this workspace;
a checkout path dependency is not equivalent. Check that resolution neither
recursively compiles nor mutates lock files, fetches network data, or hangs
behind a parent Cargo process. Preserve compiler admission and the shared Cargo
cache throughout.

Acceptance E0 requires deterministic identity resolution, a verified facade
edge, documented supported identities/source forms, and explicit failures for
unsupported or ambiguous cases. If this cannot be established, stop: record
BLOCKED and propose a design clarification rather than accepting an unbound
count or adding a consumer escape hatch. No feature code is implemented before
this decision. Record reproducible fixture recipes under
`ortho_config/tests/fixtures/clap_error_build_probe/`; generated projects and
build products live under `target/`, while the durable recipe remains tracked.
Production code is still unchanged. Commit only a coherent, gated result.

### M1: canonical matrix and executable translations

First add failing public matrix, catalogue, sentinel, and override tests.
Record the expected red result, then define the enum and matrix in a focused
`ortho_config/src/localizer/clap_error_ids.rs` module and expose both public
paths. Add ten en-US strings, including `clap-error-invalid-subcommand`. The
other nine new identifiers use the existing kebab naming convention. Preserve
`clap-error-missing-argument`, `clap-error-unknown-argument`,
`clap-error-invalid-value`, and `clap-error-missing-subcommand` exactly.

Make `clap_error.rs` consult this matrix; sentinel rows return immediately.
Keep unknown-kind fallback and existing rendered-tail behaviour. Avoid the
rich-context redesign of 11.2.2. Expand context extraction only where a new
message requires information actually supplied by clap, with tests for absent
context. Add the BDD and CLI cases from V3 and snapshots from V2. Add API
documentation and consumer guidance alongside the behaviour change.

Acceptance E1 is all V1–V3 tests passing, red evidence retained in this plan,
all ordinary gates green, and unchanged existing-identifier override tests.
Review R1/R2 conformance and upstream assumptions before committing. Remaining
gap: the automatic upstream-growth detector in M2. Recovery is to fix forward
or revert this atomic milestone, without adding compatibility shims.

### M2: total-count build script and compile-time coverage

Only after E0, add `ortho_config/build.rs` and narrow build-support modules
using the proven identity method. Read the count from the checked-in supported
identity manifest (or its deterministic generator, if M0 establishes one), then
verify the compiled dependency resolution and facade edge before emitting
`cargo:rustc-env=ORTHO_CONFIG_CLAP_ERROR_KIND_COUNT=<n>` and a numeric constant
in `OUT_DIR/clap_error_kind_count.rs`. Both values must come from the same
validated identity entry. Do not ship an unbound hard-coded count or a
best-effort source fallback. Generate any test inventory only where the design
requires it and never classify variants in build support. Register every
manifest and resolution input for rerun and demonstrate invalidation. Do not
overwrite outputs until inspection succeeds, and never silently reuse stale
output. Unknown identity, source/checksum mismatch, facade-edge mismatch,
ambiguity, and resolution failures must fail with an actionable diagnostic
naming the operation and package/version when known.

In `clap_error_matrix.rs`, include the generated constant and call
`static_assertions::const_assert_eq!(CLAP_ERROR_IDS.len(), CLAP_ERROR_KIND_COUNT)`.
Also verify agreement with the emitted environment value at runtime. Keep the
assertion in the test suite as §6.1.1 prescribes: ordinary downstream library
builds do not run dependency tests. The build script's identity check still
runs for consumers and rejects unknown identities; the const assertion is a
pre-publication test gate for a supported identity, not a claim that every
consumer build rejects a new variant added under an already-supported source.

Start with a supported-identity manifest-count mismatch fixture and verify it
fails at the const assertion; separately verify that a new or overridden
unsupported source fails closed before count emission. Restore the supported
manifest/matrix and prove semantic inventory tests pass. Run V4 and all
ordinary gates. Acceptance E2 includes E0 identity cases, count consistency for
the resolved supported identity, fail-closed unknown identity, duplicate row
rejection, rerun evidence, and no display classification in build support.
Commit after a conformance review of R3; remaining gap is final documentation
reconciliation.

### M3: reconcile documentation and complete delivery

Update `docs/users-guide.md` with matrix enumeration, override IDs, sentinel
handling, and the missing-input help change. Update `docs/developers-guide.md`
with build-probe ownership, prerequisites, troubleshooting, freshness inputs,
and the procedure when clap adds a variant. Update §6.1.1 of the localization
design with the established mechanism and its limits; use `docs/design.md` for
the component boundary where appropriate. Record substantive choices in an ADR
and link it from both design and contents. Do not advertise deferred APIs.

Run all ordinary gates, documentation gates, focused integration tests, and new
executable doctests. Reconcile all R1–R3 evidence and upstream decisions. Only
when the feature is complete mark roadmap 11.2.1 `done` in the existing
checkbox convention. Use the `mapsplice` skill for that structural roadmap
edit. The planning PR must leave its checkbox open. Update this plan's status
and retrospective, then gate and commit the final milestone.

## Concrete steps and acceptance commands

Run from the repository root. Before each major milestone run the relevant
focused suites to establish the baseline. During its red stage run the newly
added test before production edits and record the expected failure. After
green, repeat the focused test, then the full gates sequentially. Do not
interpret a filtered run with zero tests as success.

```bash
cargo test -p ortho_config --test clap_error_matrix
cargo test -p ortho_config --test clap_error_translations
cargo test -p ortho_config --test clap_error_cli
cargo test -p ortho_config --test rstest_bdd
cargo test -p ortho_config --test clap_error_build_probe
cargo test -p ortho_config --doc
```

New target commands become available in their named milestones. Their expected
result is a nonzero executed test count, all named cases passing, and no
ignored failures. Keep seed/failure records for property tests. Record
fixture-build commands and exact expected compile diagnostics in M0/M2 once
established; they are not invented here before the resolver experiment.

Use the canonical Make targets, not a reduced substitute. The live Makefile
runs rustdoc, Clippy, and Whitaker for lint; test includes all-feature Rust
coverage and Python tests. Capture every command's true status using pipefail:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-11-2-1.out
make test 2>&1 | tee /tmp/test-ortho-config-11-2-1.out
make typecheck 2>&1 | tee /tmp/typecheck-ortho-config-11-2-1.out
make lint 2>&1 | tee /tmp/lint-ortho-config-11-2-1.out
```

Run these as separate sequential invocations and stop on failure. After
Markdown edits run `make fmt`, inspect for unrelated formatter changes, then run
`make check-fmt`, `make markdownlint`, and `make nixie` with separate logs. Do
not hand-edit generated spelling configuration. Planning-only delivery requires
these documentation gates; Rust implementation gates apply to M0–M3 whenever
code is introduced and to every major implementation milestone.

## Interfaces and dependencies

The new public shape, also re-exported from the crate root, is:

```rust
#[non_exhaustive]
pub enum ClapErrorTranslation {
    Translated(&'static str),
    DisplayOnly,
}

pub const CLAP_ERROR_IDS: &[(clap::error::ErrorKind, ClapErrorTranslation)];
```

The declaration above specifies the constant's type; its actual definition must
supply every row. Build support should use the narrowest mechanism that
resolves and validates the identity manifest against the compiled facade edge.
Use `cargo_metadata` only if M0 demonstrates that route observes the required
consumer graph. Add a source parser such as `syn` only if the accepted manifest
workflow needs one. Reuse workspace versions compatible with Rust 1.89; add
`static_assertions` as a caret dev-dependency. Existing dev dependencies
already include `rstest`, `rstest-bdd`, `googletest`, `pretty_assertions`,
`insta`, and `proptest`. No solver dependency is planned.

## Idempotence and recovery

Keep scratch fixtures under a task-specific `target/` directory and logs under
`/tmp`. Never modify the shared cached clap sources for negative controls; copy
fixtures and apply local patches there. Preserve users' unrelated edits. After
any interruption inspect Git status, fixture paths, logs, and this plan before
resuming. Remove deliberate mutations before final gates. Each passing
milestone is a rollback point; revert its commit rather than resetting shared
history. Do not rerun a failed gate until its cited log has been inspected and
the cause fixed or an infrastructure condition has demonstrably changed.

## Artefacts and notes

External contracts checked through Firecrawl on 2026-10-14:

- [clap_builder 4.6.7 ErrorKind](https://docs.rs/clap_builder/4.6.7/clap_builder/error/enum.ErrorKind.html):
  seventeen declared variants, with a non-exhaustive public enum.
- [Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html):
  package IDs, resolved edges, manifest paths, and locked/offline behaviour.
  `--no-deps` cannot provide the required dependency graph.
- [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html):
  build inputs, `OUT_DIR`, environment output, and rerun directives. These
  contracts do not by themselves prove the candidate's consumer graph identity.

Wyvern evidence was exchanged using context packs `pk_nry25fav` and
`pk_n7jcfn3g`, plus documentation pack `pk_qmi5dgej`; this document contains
the durable decisions and needs no pack to execute. Expert review covered
Pandalump (architecture), Wafflecat (alternatives), Telefono (contracts),
Dinolump (maintenance), Buzzy Bee (build operations), and Doggylump (failure
handling). All actionable findings were incorporated: renamed dependencies and
target/feature cases; tracked fixture recipes; actual packaged consumer builds;
environment invalidation; actionable failures; and source-growth controls. The
2026-11-08 revision supersedes the original growth control with separate
unsupported-identity and supported-manifest-count controls. Implementation
feasibility remains subject to M0. Record validation receipts in the PR; keep
it draft and include its branch-wise plan link and the Lody session link in a
final References section.

Revision note (2026-10-14): initial plan records contracts before
implementation, adds the omitted variant, and makes exact-source feasibility an
explicit gate.

Revision note (2026-10-14): expert review strengthened M0 and M2 acceptance
evidence without widening the requested public API or changing the gate.

[dispatch]: ../clap-dispatch-and-ortho-config-integration.md
[injection]: ../reliable-testing-in-rust-via-dependency-injection.md
[complexity]: ../complexity-antipatterns-and-refactoring-strategies.md

Revision note (2026-11-08): reconcile PR #651 tooling and the current identity
manifest contract, preserve PR #617 runtime boundaries, and retain the explicit
implementation approval gate.

Revision note (2026-11-09): temporarily base the plan on PR #664 to inherit the
nested Cargo timeout fix; record the parent boundary and the implementing
agent's return-to-main procedure after that PR merges.
