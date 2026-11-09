# Ship the en-US clap error matrix (11.2.1)

Status: M0.1 complete — the governing design, ADR-0018, roadmap wording, and
developers' guide now describe the pre-publication workspace gate. Feature
implementation proceeds at M1 (2026-11-09).

This ExecPlan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current at every milestone.

## Purpose / big picture

Consumers will be able to enumerate the clap error kinds supported by this
release, identify which Fluent message to override, and receive embedded en-US
text for every non-display error. Help and version requests will pass through
untouched. A pre-publication test gate will compare the source inventory for
the supported clap identity with its reviewed count and fail when that
inventory changes. A compile-time assertion will separately ensure that the
matrix length equals that reviewed count. This is roadmap task 11.2.1 only.

The deliverable of this branch is the feature, its pre-publication gate, and
the governing documents that describe them. M0 established that the approved
build-time mechanism cannot bind consumers to their governing Clap resolution.
At the user's request the pre-publication workspace gate replaced it, and M0.1
amended the design, recorded ADR-0018, and reconciled the roadmap and the
developers' guide. M1 onward implements the matrix, strings, runtime
integration, and gate.

## Constraints

Implement the public matrix and en-US strings from
[CLI localization design](../cli-localization-design.md) §§6.1 and 6.2. Replace
§6.1.1's unsupported consumer build-script binding with a pre-publication
workspace gate. The gate must read the governing workspace `Cargo.lock` for the
selected production `clap` package identity (version, source, checksum) and its
`clap` → `clap_builder` dependency edge, then cross-check the resolution with
`cargo metadata --locked --offline` using the workspace's production
target/features. Resolve the metadata `manifest_path` for that exact
`clap_builder` package and count its top-level `ErrorKind` variants with `syn`.

Support only an explicitly reviewed identity and an unambiguous, audited
variant inventory. Fail the gate on unknown or overridden identities,
lock/metadata/source/edge mismatches, ambiguous resolution, or cfg-dependent
declarations whose complete inventory has not been audited. The parser counts
every declared variant without classifying display-only entries. Keep a
reviewed test constant for the supported identity's variant count.
Independently require the source count to equal that reviewed count, then use
`static_assertions::const_assert_eq!(CLAP_ERROR_IDS.len(), CLAP_ERROR_KIND_COUNT)`
to compare the matrix length to the reviewed constant. Source growth therefore
fails the publication gate until reviewed and the constant is deliberately
updated; the compile-time assertion alone does not detect dependency source
growth.

This gate describes the repository's selected workspace resolution at
publication time. It does not claim to identify an external consumer's Clap
resolution or reject that consumer's build. Do not add `build.rs`, `rustc-env`,
`OUT_DIR` count artefacts, or a consumer-side identity rejection. Do not use an
unbound hard-coded count, registry-directory glob, enum discriminant maximum,
or a second independently resolved dependency as the authoritative inventory.

Preserve the four existing Fluent identifiers and their argument contracts.
Ship en-US strings unconditionally. Add the public enum and constant through
both existing export layers. Use the matrix as the runtime mapping authority;
all three display-only entries bypass lookup and return the original error. The
publication gate detects a changed supported source inventory before release;
runtime builds retain the crate's existing fallback behaviour for any kind the
current matrix does not translate. Do not implement the coverage-report
builder, richer clap context preservation, locale lifecycle, embed bridge, or
derive work. Those belong to other roadmap entries.

Keep the declared MSRV, Rust 1.89, and validate with the workspace toolchain in
`rust-toolchain.toml` (currently Rust 1.99); retain caret dependency
requirements. Do not change the toolchain, compiler admission, shared Cargo
cache, or Whitaker installation. Use capability-oriented filesystem access and
UTF-8 paths in new Rust test tooling. New Rust files must have module
documentation and remain at most 400 lines. Do not add runtime or build
dependencies for source inspection. Keep `cargo_metadata` and `syn` scoped to
the publication gate and its tests, and `static_assertions` scoped to the
matrix test.

Implementation uses test-first development. Do not commit an intentionally
failing red stage. Every code milestone must pass sequential `make check-fmt`,
`make test`, `make typecheck`, and `make lint` before its commit. Use the
scrutineer agent for full gates and keep logs under `/tmp`; build products stay
under `target/`.

## Tolerances (exception triggers)

Stop implementation and set this plan to BLOCKED if the publication gate cannot
uniquely match the locked production facade edge to an explicitly supported
`clap_builder` identity, resolve its source manifest, or count its complete
`ErrorKind` inventory. Unknown identities, source overrides, mismatches,
ambiguous graphs, and unaudited cfg-dependent declarations must fail closed. Do
not claim or add a downstream consumer rejection guarantee. Record the evidence
and return to the design owner if the selected workspace-gate contract cannot
be met. Escalate public APIs beyond the requested enum and constant, changes
outside the runtime localizer, publication-gate/test support, and their
documentation, toolchain changes, or dependencies outside the test-only
candidates listed below. There is no time limit. Infrastructure failures are
blocked validation, not permission to weaken a gate. Disk exhaustion requires
stopping and reporting.

## Risks

- High impact, **realized and addressed by the selected plan revision**
  (2026-11-09): a dependency build script cannot establish the consumer's
  governing resolution. M0 found this limitation for external path and
  unpacked-package consumers; the workspace probe could observe its governing
  resolution. The selected contract checks the repository's supported workspace
  resolution at publication time only; see "M0 outcome". It makes no claim
  about arbitrary downstream dependency graphs.
- High impact, medium likelihood: equal matrix and enum lengths can conceal a
  duplicate plus an omission. Test uniqueness and exact inventory agreement.
- Medium impact, high likelihood: clap does not supply every context variable
  for every error construction. Inventory real contexts before writing FTL;
  generic errors must render without unresolved Fluent variables.
- Medium impact, medium likelihood: the reviewed count can become stale after
  a lock, source, patch, or Cargo configuration change. Recompute source
  inventory in the publication gate and fail when it differs from the reviewed
  identity record; never accept stale count evidence.
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
- [x] (2026-11-09) Obtain explicit implementation approval.
- [x] (2026-11-09) M0: investigate identity-manifest and facade-edge
      feasibility; original build-time mechanism reached its stop condition.
- [x] (2026-11-09) Select the pre-publication workspace gate after M0.
- [x] (2026-11-09) M0.1: amended design §6.1.1, §6.1, and §12; recorded
      [ADR-0018](../adrs/adr-0018-pre-publication-clap-coverage-gate.md);
      reconciled roadmap 11.2.1 and the developers' guide; indexed the ADR.
- [ ] M1: implement the canonical matrix, strings, and runtime integration.
- [ ] M2: implement the test-only reviewed count assertion, pre-publication
      inventory gate, and fault-detection evidence.
- [ ] M3: finish consumer documentation and mark roadmap task done.

## Surprises & discoveries

The plan first incorporated merged PR #651 at
`ddba3578fae04227e426ea3a61aa2b24aa043232`. On 2026-11-09 it was restacked onto
the timeout-fix branch for PR #664 at
`501ab83134eaba0f9e97e1daeaa29c793a819c53`. The M0 probe observed `clap` and
`clap_builder` 4.6.7 in the workspace. The design's 4.6.0 identity is
illustrative, not the observed workspace resolution. M0 found that consumer
build scripts cannot bind to downstream resolution; the selected gate records
and checks only the explicitly supported workspace identity. The upstream API
inventory observed during M0 had 17 variants; the implementation must recount
the source resolved at gate time and compare it with a reviewed constant.

M0 (2026-11-09) falsified that binding. A dependency's build script cannot
observe its dependent's dependency resolution. The only consumption form where
the walk-up from `CARGO_MANIFEST_DIR` finds the governing lock is our own
workspace build; an external path consumer and an unpacked packaged consumer
both resolve to a lock that does not govern the build. The packaged case is
worse than a miss: Cargo 1.84 and later place a publish-time `Cargo.lock` next
to the packaged manifest, so the naive rule silently binds a file that
describes the publish-time resolution, including dev-dependencies the consumer
never compiles. Details and citations are in "M0 outcome".

The requested nine additions plus four existing entries total thirteen.
`InvalidSubcommand`, explicitly named by §6.1, is also required: ten new
translations produce fourteen translated entries and three sentinels.

The design's build-script outputs are removed by the selected mechanism. The
source inventory is read by a pre-publication test; because that runtime test
value cannot initialize a Rust const, keep a reviewed count constant in the
test source, compare the parsed source count to it, and use `const_assert_eq!`
only for matrix length versus the reviewed count.

PR #617 completed the reusable `ortho_l10n` extraction; PR #651 added pinned,
opt-in Kani/Verus tooling. The Fluent runtime and embedded catalogues now live
in `ortho_l10n`; the Clap adapter, exhaustive matrix, and Clap-specific
pre-publication test gate remain in `ortho_config`. The prover tooling's
integration fixtures validate the runner and its negative controls, not this
feature's production claims. This finite enum/catalogue mapping and bounded
source counter do not warrant a Verus proof; tests and explicit inventory
review are proportionate.

`DisplayHelpOnMissingArgumentOrSubcommand` currently maps to the missing
subcommand message. The new sentinel contract requires pass-through instead.
The current formatter reconstructs translated errors and preserves only a
rendered tail. Rich structured context preservation remains task 11.2.2.

## Decision log

- 2026-10-14: propose the design's build-script mechanism, with M0 as an
  explicit feasibility decision. M0 later falsified its downstream identity
  assumption; see the outcome below.
- 2026-11-08: require identity and facade-edge evidence; the chosen mechanism
  narrows that guarantee to the supported workspace publication gate.
- 2026-11-09: M0 completed with a no-go for the original build-time binding.
  At the user's request, this revision selects the pre-publication workspace
  gate, preserving the public total-count requirement while removing
  unsupported consumer claims.
- 2026-11-09: amend design §6.1.1 and record the mechanism in an ADR before
  runtime implementation. Reconcile roadmap wording. The present change records
  the plan decision only.
- 2026-11-09: put metadata/source-count checks in a dedicated test-only
  publication gate. Keep the reviewed count constant in test source; compare
  the parsed count to it at test time, and assert matrix length against it at
  compile time. No build-script fixture or generated build output is needed.
- 2026-11-09: M0.1 complete. Design §6.1.1 is retitled "Pre-publication
  coverage gate" and states the workspace-only limit; §6.1 and §12 point at it;
  ADR-0018 records the decision and the rejected alternatives; roadmap 11.2.1
  and the developers' guide no longer claim a build script or a consumer
  guarantee; `docs/contents.md` indexes the ADR.
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
revised plan is ready for review.

The earlier plan was approved on 2026-11-09, and M0 ran immediately. M0 ended
with a no-go for the original build-time identity binding: the evidence showed
that a dependency build script cannot reliably establish an external consumer's
governing Clap resolution. At the user's request, this revision selects the
recommended pre-publication workspace gate. The governing design and roadmap
wording still need reconciliation, and the mechanism decision must be recorded
in an ADR before implementation. No feature code exists yet.

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
- R3, design §6.1.1 total-count mechanism, as amended by
  [ADR-0018](../adrs/adr-0018-pre-publication-clap-coverage-gate.md): M0
  evidence and selected deviation; M0.1, design/ADR/roadmap reconciliation; M2,
  workspace identity gate, source count comparison, matrix const assertion, and
  deliberate drift controls.

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
- [ADR 0018](../adrs/adr-0018-pre-publication-clap-coverage-gate.md) owns the
  pre-publication coverage gate and supersedes the build-time identity
  mechanism design §6.1.1 previously proposed.
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
introduce a dependency from the standalone localization runtime to Clap. Before
any helper extraction, search for an equivalent and document ownership and
permitted reuse in the developers' guide. Publication-gate helpers belong only
to test support and the gate.

## Verification plan

### Model, preconditions, and representation

Let I be the explicitly supported `clap_builder` identity, recorded by package
version, registry/source identity, and Cargo.lock checksum. Let E(I) be the set
of top-level `ErrorKind` variants parsed from the `manifest_path` resolved for
that identity by workspace `cargo metadata --locked --offline`. The gate also
checks that the selected production `clap` package and its normal dependency
edge resolve to I for the configured publication target and features. Unknown,
overridden, ambiguous, mismatched, or incompletely audited cfg-dependent
inventories have no accepted E(I) and fail the gate.

Let N(I) be the reviewed integer constant in the matrix test for I, and M the
public sequence of matrix pairs. Each pair has a real `ErrorKind` and either
`Translated(id)` or `DisplayOnly`; D is the three display variants from §6.1.
The publication test requires the freshly parsed source count to equal N(I).
Rust's `const_assert_eq!` separately requires `len(M) == N(I)`. The source
parser's runtime result cannot initialize a Rust const. Therefore a source
variant addition fails the publication test until reviewed; after updating
N(I), a missing or extra matrix row fails the compile-time assertion. The
assertion alone is not an upstream-source-growth detector.

The remaining postconditions are that each member of E(I) occurs exactly once
in M; exactly D receives the sentinel; every other row names a unique,
renderable embedded English message; runtime lookup agrees with M; and
display-only errors return unchanged without consulting the localizer. The
finite coverage argument is that distinct keys drawn from E(I), together with
`len(M) == N(I) == |E(I)|`, cover the inventory. Length equality alone lacks
the distinctness premise. This is a review argument discharged for the concrete
inventory, not a machine-checked theorem. No contractual business algorithm or
unbounded semantic kernel is introduced, so Verus, Kani, and solver scaffolding
would not improve the relevant assurance. Never use `assume`, `admit`, or
trusted bodies to bypass a proof.

Axioms are Cargo's locked workspace resolution and metadata contracts, the
selected registry package's source layout, clap's published type and context
contracts, Rust's typed enum constructors and const evaluation, and Fluent's
parser/formatter contract. Tests exercise our use of these boundaries; they do
not purport to prove those dependencies correct. The gate establishes only the
repository's configured publication graph, not downstream consumer graphs.

### V1: exact inventory and uniqueness

Add `ortho_config/tests/clap_error_matrix.rs` with an independently enumerated
`rstest` table for the exact supported `clap_builder` identity. Check pairwise
key uniqueness, identifier uniqueness, exact expected kind membership, and the
sentinel partition. Compare the table length with the reviewed identity count
using the compile-time assertion; the publication gate independently counts
source variants. Compile imports through both `ortho_config` and
`ortho_config::localizer`. Use `googletest` matchers for variants and
collections and `pretty_assertions` for equality.

The witnesses include every supported real kind and all three sentinels.
Replace a row with a duplicate while keeping length unchanged: the
uniqueness/inventory test must fail. Remove or add a row: the M2 const
assertion must fail. These controls prevent a shared production/test omission
from looking exhaustive.

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

### V4: publication identity, source inventory, and freshness

Implement the publication check as a test-only gate, not a build script. Read
the root workspace `Cargo.lock` and select the unique locked `clap` package
used by `ortho_config` as a normal production dependency for the configured
target and features. Verify its package version, source identity, and checksum
against the lock entry. Follow the resolved `clap` → `clap_builder` normal
dependency edge and require one unambiguous `clap_builder` package identity.
Cross-check both package IDs and the edge with workspace
`cargo metadata --locked --offline` for that same target/features selection.
Reject lock/metadata disagreement, source override, multiple candidates,
missing edges, unexpected dependency kinds, or unsupported identities. For the
supported registry identity, use metadata's `manifest_path` to locate the exact
source defining `ErrorKind`; do not infer the path from registry directory
names.

Parse the enum with `syn` and count all top-level variants without assigning
meaning to any of them. Audit attributes and cfg conditions for the supported
source. If the inventory's compiled shape cannot be established exhaustively,
fail closed and require an explicit source-support decision. Use `proptest` on
the implemented parser: adding a variant adds one, while declaration order,
comments, and discriminants do not change the count; malformed input fails, and
unsupported conditional declarations fail closed. Record generator acceptance
and classification so every relevant case is exercised. Do not filter away
declaration forms relevant to the supported source.

Keep `CLAP_ERROR_KIND_COUNT` as a reviewed test constant associated with the
allow-listed package identity. The gate independently requires the parsed
source count to equal that constant. In the matrix test, compile
`static_assertions::const_assert_eq!(CLAP_ERROR_IDS.len(),
CLAP_ERROR_KIND_COUNT)`.
Thus source growth fails the publication gate until reviewed; after the
constant is updated, a matrix omission or extra row fails the const assertion.
The assertion does not parse source or detect upstream growth by itself.

Negative controls must prove the checks are live: alter the supported identity
checksum/source or facade edge and require identity validation to fail; supply
an unknown identity and require a fail-closed diagnostic; alter only the parsed
source count and require the publication test to fail against the reviewed
constant; update the constant without the matrix and require compile failure;
and introduce a duplicate plus omission with unchanged length and require the
semantic inventory test to fail. Restore all controls before the final gates.
The gate validates this repository's publication graph only; consumer
end-to-end tests continue to verify observable behaviour, not a downstream
dependency-identity guarantee.

## Plan of work and milestones

### M0: establish the original identity-manifest feasibility

M0 tested the proposed build-script identity resolver against workspace,
external path, and unpacked packaged consumption. Preserve its evidence and
historical probe commands below. The experiment found the build script could
not establish the external consumer's governing lock and compiled facade edge.
M0 therefore ended the original mechanism at its stop condition. It did not
implement a fallback or feature code. Its acceptance result is a documented
no-go for the original design, not a go for the feature.

#### M0 outcome (2026-11-09): original mechanism stopped; evidence retained

The build-time identity contract holds for our own workspace build and for no
other consumption form. A dependency's build script cannot observe its
dependent's dependency resolution, and the Cargo team treats the reverse
direction as out of scope: "we do not want to allow side-banned communication
between dependencies and dependents"
([rust-lang/cargo#3946](https://github.com/rust-lang/cargo/issues/3946),
2025-08-26). There is no `CARGO_WORKSPACE_DIR`.

Evidence came from a temporary probe build script, since removed, and from
disposable consumers under `target/m0/`:

| Consumption form           | Governing lock reachable | Observed                                                                                                                                                                                                                                                             |
| -------------------------- | ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Workspace build            | Yes                      | `CARGO_MANIFEST_DIR=<worktree>/ortho_config`; the walk-up finds `<worktree>/Cargo.lock`, which does govern the build.                                                                                                                                                |
| External path consumer     | No                       | `CARGO_MANIFEST_DIR` still resolves inside our checkout, so the walk-up binds *our* workspace lock, not the consumer's lock.                                                                                                                                         |
| Unpacked packaged consumer | No                       | An adjacent publish-time `Cargo.lock` exists (80,043 bytes, 332 packages, including dev-only `trybuild`, `proptest`, `cargo_metadata`, `process-wrap`, and `insta`), while the consumer's governing lock is a different file (32,629 bytes, none of those packages). |

Four routes to the compiled identity were each closed:

- `DEP_*` bridge: none. `clap` and `clap_builder` declare no `links`, and the
  probe observed no `DEP_` variable of any kind.
- Target-directory introspection: at build-script time the target directory
  names no `clap` package. The build-script unit's fingerprint carries
  `deps: []`; the unit that does list `clap` is written only after our build
  script runs, and records a numeric package-id hash with no version, source,
  or checksum.
- `OUT_DIR` ancestry: this does reach the consumer's target directory, and one
  level further reaches the consumer's own lock, but the layout is not
  contractual and breaks under `CARGO_TARGET_DIR`, `--target-dir`, and
  `build.target-dir`
  ([rust-lang/cargo#9661](https://github.com/rust-lang/cargo/issues/9661)).
  Tools that do this describe it as guessing, not resolving.
- `cargo metadata` from the build script: since Cargo 1.84 a library package
  ships its own lock, so the call honours that publish-time lock, can require
  the network, and can resolve versions the consumer never compiles
  ([rust-lang/cargo#15272](https://github.com/rust-lang/cargo/issues/15272)).

Renamed dependencies, feature selection, target-specific edges, multiple `clap`
versions, and source overrides were not exercised: the decisive failure occurs
earlier, because no governing lock is reachable at all, so those sub-cases
cannot change the verdict.

This satisfied the original tolerance trigger, "the active dependency source
cannot be identified without guessing", and ruled out the original build-time
mechanism. The revised decision accepts a workspace publication gate with an
explicitly limited guarantee. No feature code was written.

**Upstream design impact.** Design §6.1.1 requires a build-time check binding
the count to the resolved `clap_builder` identity and to the compiled `clap`
facade edge. That requirement rests on an assumption Cargo does not support for
consumers, so §6.1.1 needs amendment whichever option is accepted. The public
surface in §6.2, the enum and the constant, is unaffected.

**Decision and rejected options.** At the user's request, this revision selects
the recommended option: move identity binding to the pre-publication workspace
test gate. Read the workspace `Cargo.lock` for package identity and edge;
cross-check with `cargo metadata --locked --offline` (M0 observed exit 0,
`clap` 4.6.7, `clap_builder` 4.6.7, the facade edge, and a `manifest_path` into
the resolved source); count source variants with `syn`; and compare with a
reviewed identity-specific test constant. Keep `const_assert_eq!` for matrix
length versus that constant. This retains total-count coverage and fail-closed
publication checks without claiming downstream consumer rejection. It removes
`build.rs`, `rustc-env`, `OUT_DIR` count output, and build-time identity
resolution.

The following alternatives remain rejected:

1. Emit nothing outside a workspace build. This creates the consumer escape
   hatch the original tolerance forbade and leaves downstream checking vacuous.
2. Ask clap upstream for a version constant or a `links` key. Neither is
   available in the observed API and this task cannot add it.
3. Publish a bridge crate declaring `links`. Its build script is equally blind
   to its dependent's resolution, so the value would be a hard-coded literal.

Recipes, kept out of the product tree:

```bash
# Workspace build: add a probe build script, then
cargo build -p ortho_config
# and read target/debug/build/ortho_config-*/stderr.

# External path consumer: standalone `[workspace]` package under target/m0/
# with `ortho_config = { path = "<worktree>/ortho_config" }`, then
cargo build --offline --manifest-path target/m0/path-consumer/Cargo.toml

# Packaged consumer: package, extract outside the workspace, consume the
# unpacked archive (a checkout path dependency is not equivalent).
cargo package -p ortho_config --allow-dirty --no-verify
tar xzf target/package/ortho_config-0.10.0.crate -C target/m0/unpacked
cargo build --offline --manifest-path target/m0/pkg-consumer/Cargo.toml
```

#### Selected mechanism handoff

At the user's request, this revision selects the pre-publication gate. Before
feature code, perform M0.1: amend `docs/cli-localization-design.md` §6.1.1 and
record the substantive change in an ADR; reconcile any roadmap wording that
implies consumer-side failure; and update this ExecPlan's conformance links.
The selected mechanism limits identity assurance to the repository's configured
publication graph. It does not require a consumer to run our test gate.

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
Review R1/R2 conformance and upstream assumptions before committing. The
publication inventory check remains for M2. Recovery is to fix forward or
revert this atomic milestone, without adding compatibility shims.

### M2: pre-publication inventory gate and compile-time matrix assertion

After completing M0.1, add a test-only `ortho_config` publication gate. The
implementation must read the workspace lock, identify the selected production
`clap` package and normal facade edge, and confirm both against
`cargo metadata --locked --offline` using the configured target/features. Check
version, source, and checksum against the locked supported identity. Follow the
metadata-resolved `clap` dependency edge to exactly one `clap_builder`, resolve
its `manifest_path`, and use `syn` to count every top-level `ErrorKind`
variant. Do not read consumer locks or pretend to reject consumer builds. Fail
closed before publishing for unknown identities, altered sources, checksum or
edge mismatches, ambiguous candidates, metadata disagreement, and unaudited cfg
shapes. Do not add `build.rs`, `rustc-env`, `OUT_DIR` artefacts, or generated
build outputs for this check.

Keep the supported identity and reviewed total in the test as explicit data.
The gate compares its parsed source count to the reviewed count at test time.
Use
`static_assertions::const_assert_eq!(CLAP_ERROR_IDS.len(),
CLAP_ERROR_KIND_COUNT)`
to check the matrix against that reviewed constant at compile time. The
test-time parsed value cannot initialize the Rust const. A new source variant
therefore fails the publication gate until reviewed; after the count is
intentionally updated, a missing or extra matrix row fails the const assertion.
Unknown identities fail our publication gate only; no consumer build rejection
is promised.

Add durable fixtures for supported and unsupported lock/source identities, the
`clap` facade edge, variant-count drift, and matrix drift. Require each
negative control to fail for its stated reason; restore all mutations before
running gates. Acceptance E2 requires the supported workspace graph to pass,
unknown or mismatched identities to fail closed, source-count drift to fail
test-time comparison, matrix length drift to fail the const assertion, and
semantic tests to reject duplicate-plus-omission errors. Include an explicit
rerun/freshness check if the gate caches parsed results; otherwise do not add
cache invalidation machinery. No support code classifies display-only variants.
Commit after a conformance review of revised R3.

### M3: reconcile documentation and complete delivery

Update `docs/users-guide.md` with matrix enumeration, override IDs, sentinel
handling, and the missing-input help change. Update `docs/developers-guide.md`
with publication-gate ownership, prerequisites, troubleshooting, and the
procedure when clap adds a variant. M0.1 must update §6.1.1 of the localization
design to state the supported workspace-only guarantee and its limits, and
record the deviation in an ADR linked from the design and contents. Use
`docs/design.md` for the component boundary where appropriate. Do not advertise
deferred APIs or consumer-side identity rejection.

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
cargo test -p ortho_config --test clap_error_coverage --locked --offline
cargo test -p ortho_config --doc
```

New target commands become available in their named milestones. Their expected
result is a nonzero executed test count, all named cases passing, and no
ignored failures. Keep seed/failure records for property tests. Record the gate
fixture commands and exact expected diagnostics in M2 when the test target is
implemented; M0 historical probe commands are retained in the outcome above.

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
supply every row. The publication test should use `cargo_metadata` to validate
the workspace production facade edge and `syn` to count variants from the
resolved package source. Both are test-only; neither participates in a build
script. Keep the reviewed supported identity/count beside the matrix test and
use `static_assertions` there for matrix-length comparison. Reuse workspace
versions compatible with Rust 1.89 and caret requirements. Existing dev
dependencies already include `rstest`, `rstest-bdd`, `googletest`,
`pretty_assertions`, `insta`, and `proptest`. No solver dependency is planned.

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
  historical M0 context for the rejected build-time mechanism. These contracts
  do not prove a dependency's consumer graph identity.

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

Revision note (2026-11-09): record M0's evidence that the approved consumer
build-time identity binding is unavailable, select the pre-publication
workspace gate at the user's request, and replace build-script milestones with
a test-time source inventory comparison plus a matrix-versus-reviewed-count
const assertion. The implementor must reconcile §6.1.1 and the roadmap and
record the choice in an ADR before feature implementation. The guarantee covers
the configured workspace publication graph only.
