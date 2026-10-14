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
6.1.1, and 6.2. Keep the build script plus `const_assert_eq!` mechanism: count
all declared variants without classifying them in the build script. Never use a
hard-coded count, registry-directory glob, enum discriminant maximum, or a
second independently resolved dependency as the authoritative inventory.

Preserve the four existing Fluent identifiers and their argument contracts.
Ship en-US strings unconditionally. Add the public enum and constant through
both existing export layers. Use the matrix as the runtime mapping authority;
all three display-only entries bypass lookup and return the original error.
Future unrecognized kinds retain the current fallback path until the coverage
failure is repaired. Do not implement the coverage-report builder, richer clap
context preservation, locale lifecycle, embed bridge, or derive work. Those
belong to other roadmap entries.

Keep Rust 1.89 compatibility and caret dependency requirements. Do not change
the toolchain, compiler admission, shared Cargo cache, or Whitaker
installation. Use capability-oriented filesystem access and UTF-8 paths in new
build tooling. New Rust files must have module documentation and remain at most
400 lines. Do not add runtime dependencies for build-time source inspection.

Implementation uses test-first development. Do not commit an intentionally
failing red stage. Every code milestone must pass sequential `make check-fmt`,
`make lint`, and `make test` before its commit. Use the scrutineer agent for
full gates and keep logs under `/tmp`; build products stay under `target/`.

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

- High impact, unresolved likelihood: a nested Cargo metadata query may use
  the wrong consumer graph or contend with its parent. M0 must establish source
  identity and execution safety before production build integration.
- High impact, medium likelihood: equal matrix and enum lengths can conceal a
  duplicate plus an omission. Test uniqueness and exact inventory agreement.
- Medium impact, high likelihood: clap does not supply every context variable
  for every error construction. Inventory real contexts before writing FTL;
  generic errors must render without unresolved Fluent variables.
- Medium impact, medium likelihood: count output becomes stale after a lock,
  source, patch, or Cargo configuration change. Exercise rerun invalidation.
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
- [ ] Validate and publish this plan in a draft PR.
- [ ] Obtain explicit implementation approval.
- [ ] M0: establish exact-source build-probe feasibility.
- [ ] M1: ship canonical matrix, strings, and runtime integration.
- [ ] M2: ship mechanical count gate and fault-detection evidence.
- [ ] M3: finish consumer documentation and mark roadmap task done.

## Surprises & discoveries

The baseline is `dc979878b5728bbb23866df9c05c2570bdb5629e`. The lock file
contains clap and clap_builder 4.6.7; the design's 4.5.60 wording is
historical. The official 4.6.7 API inventory still contains 17 variants. The
design's total has also been accidentally rendered as an ordered-list item,
`1.`. Correct these documentary details during M0 without changing the
mechanism.

The requested nine additions plus four existing entries total thirteen.
`InvalidSubcommand`, explicitly named by §6.1, is also required: ten new
translations produce fourteen translated entries and three sentinels.

The sample `.parse().unwrap()` inside a const initializer in §6.1.1 is not an
executable Rust 1.89 recipe. Emit a numeric Rust constant from the same count
as the required environment value; do not create a speculative decimal parser
merely to reproduce the illustration.

`DisplayHelpOnMissingArgumentOrSubcommand` currently maps to the missing
subcommand message. The new sentinel contract requires pass-through instead.
The current formatter reconstructs translated errors and preserves only a
rendered tail. Rich structured context preservation remains task 11.2.2.

## Decision log

- 2026-10-14: retain the prescribed build-script mechanism, with M0 as an
  explicit feasibility decision. Cargo's documented build inputs do not expose
  arbitrary dependency source paths, so exact resolution is not assumed.
- 2026-10-14: include `InvalidSubcommand` and all three sentinels, following the
  complete-matrix requirement and the design rather than an incomplete list.
- 2026-10-14: use finite exhaustive tests for the matrix and property tests for
  source-count invariance. No Verus milestone is warranted for this bounded
  catalogue lookup and integration plumbing; reasoning and its limits follow.
- 2026-10-14: add no compatibility wrapper. Existing identifiers remain stable
  as required by the localization contract; the crate is pre-1.0.

## Outcomes & retrospective

Planning and expert review are complete; publication validation is pending. No
runtime changes or implementation verification have occurred. The primary
unresolved engineering risk is the provenance of the source counted inside a
consumer build. M0 prevents a superficially green count comparison from being
mistaken for evidence of coverage.

## Context and orientation

`ortho_config/src/localizer/clap_error.rs` owns `message_id`,
`localization_args`, and `localize_clap_error_with_command`. It currently
synthesizes identifiers from enum debug names, with exceptions for the four
shipped messages. `ortho_config/locales/en-US/messages.ftl` is the embedded
English catalogue. `ortho_config/src/localizer/mod.rs` and
`ortho_config/src/lib.rs` expose the public localization API.

`ortho_config/src/localizer/tests.rs` already tests fallback, captured context,
and rendered tails. Extend coverage in separate focused modules rather than
growing that file beyond the repository limit. Behavioural tests use
`ortho_config/tests/features/localizer.feature`, steps in
`ortho_config/tests/rstest_bdd_support/behaviour/steps/localizer_steps.rs`, and
the `ortho_config/tests/rstest_bdd.rs` target. Check scenario registration when
adding cases. There is no current `ortho_config/build.rs`.

Fluent is the message format; a catalogue identifier names a message, and its
variables receive clap context. A sentinel is an enum value indicating that
clap itself owns display. Cargo metadata describes resolved package identities
and dependency edges; a package name alone is not an identity.

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
  [complexity guidance][complexity], for fallback and small modules. The
  requested `docs/scripting-standards.md` is absent from this checkout; use
  repository guidance for any scripts and resolve that reference before
  introducing a new scripting convention.
- [Documentation style](../documentation-style-guide.md),
  [contents](../contents.md), and [layout](../repository-layout.md): document
  placement and test target registration.

Use the `execplans`, `codegraph-mcp`, `rust-router`, `rust-unit-testing`,
`rust-verification`, and `proptest` skills. Use `firecrawl-mcp` for unresolved
external contracts, `logisphere-experts` for design review, `commit-message`
for commits, and `pr-creation` for publication. Reuse the existing metadata
helpers where their scope fits; do not introduce a dependency from the runtime
crate to the cargo-orthohelp executable. Before any helper extraction, search
for an equivalent and document ownership and permitted reuse in the developer's
guide. Build-probe helpers belong only to build support and its tests.

## Verification plan

### Model, preconditions, and representation

Let E be the finite set of declared variants in the exact dependency enum
re-exported by the compiled clap facade. Let M be the sequence of public matrix
pairs. Each pair's first component is a real `ErrorKind`; its second is either
`Translated(id)` or `DisplayOnly`. Let D be the three display variants listed
in §6.1. The source-probe result N must equal the cardinality of E.

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
`rstest` table of all seventeen kinds. Check pairwise key uniqueness,
identifier uniqueness, exact expected kind membership, and the sentinel
partition. Compare source-derived inventory in M2 as well as the count. Compile
imports through both `ortho_config` and `ortho_config::localizer`. Use
`googletest` matchers for variants and collections and `pretty_assertions` for
equality.

The witnesses include every real kind and all three sentinels. Replace a row
with a duplicate while keeping length unchanged: the uniqueness/inventory test
must fail. Remove a row: the M2 const assertion must fail. These controls
prevent a shared production/test omission from looking exhaustive.

### V2: messages, overrides, and missing context

Add `ortho_config/tests/clap_error_translations.rs`. Parse the embedded FTL with
`fluent-syntax` and require an actual message value for every translated
identifier, rather than accepting fallback as evidence. Render each with a real
`FluentLocalizer` and representative arguments; require no formatting issues
and nonempty output. Exercise consumer overrides, en-US fallback from an
incomplete locale, and unchanged stock errors with `NoOpLocalizer`.

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

### V4: probe correctness and freshness

Add `ortho_config/tests/clap_error_build_probe.rs` sharing the pure parser and
package-selection helpers from `ortho_config/build_support/`. Use `syn` to
parse Rust syntax, not a regex that mistakes docs, commas, or nested items for
variants. Count every declaration, including display shapes, without a name
classification list. If conditional or macro-generated variants prevent
matching the compiled public enum, fail diagnostically and revisit M0.

Use `proptest` to generate small enums (1–64 distinct variants) with comments,
attributes, discriminants, reordered declarations, and unrelated enums.
Insertion increases the total by one; reordering and comments leave it
unchanged. Include a named zero-variant fixture and missing/ambiguous enum
failures. Generate valid syntax directly, without rejecting most inputs. A
deliberately skipped last variant must fail these properties.

The decisive integration controls are source growth and identity: add a new
variant to a disposable path-patched clap_builder fixture without changing M.
Add the minimal supporting arms in clap_builder's own exhaustive matches
(including `ErrorKind::as_str`) and first show that the patched dependency
compiles. Then compile the coverage test and require the const assertion
failure; an upstream match error is not evidence for our gate. A same-length
wrong source fixture must be rejected by inventory/provenance checks. Change
the selected source/lock/config inputs and show the build script reruns with
the new count. Restore controls before gates and commits.

These checks do not prove portability across every Cargo configuration. M0 must
explicitly bound supported source resolution against real workspace, consumer,
patched, and vendored builds; unsupported ambiguity is a failure, never a
silently substituted count.

## Plan of work and milestones

### M0: establish build-probe feasibility after approval

Read current source, dependency graph, and relevant Cargo documentation again.
Use a disposable fixture under `target/` to investigate source identification
before installing a production build script. A candidate is an offline, locked
Cargo metadata query following the normal dependency edges from the actual
ortho_config package to clap and then clap_builder. Select package IDs and
`manifest_path`, not the first package with a matching name. Verify the clap
facade's re-export path reaches the counted enum. The candidate is not yet
proven to observe the active consumer graph.

Record exact commands and evidence for workspace, external path consumer,
packaged consumer, source replacement/vendor configuration, path patch, and
multiple clap versions. Include renamed dependencies and consumer feature
selection/target-specific configurations; follow package identities without
assuming dependency labels. Include a consumer whose selected clap differs from
the repository lock. A packaged consumer must build the unpacked Cargo archive
outside this workspace; inspect its inclusion of the build script, support
modules, and resources. A checkout path dependency is not equivalent. Check
that metadata does not recursively compile, mutate lock files, fetch network
data, or hang behind the parent Cargo process. Preserve compiler admission and
the shared Cargo cache throughout.

Acceptance E0 requires a repeatable source-identity method for every supported
case and an explicit error for ambiguity. If it cannot be established, stop:
record BLOCKED and propose a design clarification rather than accepting a
workspace-only counter or making consumers configure an escape hatch. No
feature code is implemented before this decision. A successful M0 ends with a
validated design clarification in §6.1.1, any substantive ADR linked there,
tracked fixture recipes and exact commands under
`ortho_config/tests/fixtures/clap_error_build_probe/`, and an updated plan.
Only generated projects and build products live under `target/`; cleaning build
output must not erase the reproducible experiment. Production code is still
unchanged. Commit only a coherent, gated result.

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
using the proven identity method. Emit exactly
`cargo:rustc-env=ORTHO_CONFIG_CLAP_ERROR_KIND_COUNT=<n>` and a numeric constant
in `OUT_DIR/clap_error_kind_count.rs` from the same validated count. Generate
any test inventory from the same source parse without classifying variants.
Register all selected source, manifest, lock, and resolution-config inputs for
rerun. Inventory environment inputs actually used by the accepted resolver and
emit `cargo:rerun-if-env-changed` where Cargo does not already track them.
Demonstrate that changing a relevant environment input triggers reinspection.
Overwrite outputs only after a successful inspection so stale output is never
accepted following failure. Test missing source, ambiguous identity, malformed
enum, and metadata failure. Each must fail the build with an actionable
diagnostic naming the operation and package/version when known, and explaining
remediation. Preserve subordinate Cargo stderr without exposing credential
values; never silently reuse the previous generated count.

In `clap_error_matrix.rs`, include the generated constant and call
`static_assertions::const_assert_eq!(CLAP_ERROR_IDS.len(), CLAP_ERROR_KIND_COUNT)`.
Also verify agreement with the emitted environment value at runtime. Keep the
assertion in the test suite as §6.1.1 prescribes: ordinary downstream library
builds do not run dependency tests. This is a pre-publication CI gate, not a
claim that every consumer build rejects a newly added enum variant.

Start with the growth fixture failing for the expected missing gate, implement
the counter, then show that the fixture now fails specifically at the const
assertion. The unmodified source must compile successfully. Run V4 and all
ordinary gates. Acceptance E2 includes E0 identity cases, generated count 17
for the present lock, growth rejection, duplicate-row rejection, rerun
evidence, and no display classification in build support. Commit after a
conformance review of R3; remaining gap is final documentation reconciliation.

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
make lint 2>&1 | tee /tmp/lint-ortho-config-11-2-1.out
make test 2>&1 | tee /tmp/test-ortho-config-11-2-1.out
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
supply every row. Build support may use `syn` with full syntax parsing,
`cargo_metadata` if M0 validates that route, `camino`, and `cap-std` under
build-dependencies and matching test dependencies. Reuse workspace versions
compatible with Rust 1.89; add `static_assertions` as a caret dev-dependency.
Existing dev dependencies already include `rstest`, `rstest-bdd`, `googletest`,
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
  build inputs, `OUT_DIR`, environment output, and rerun directives. These
  contracts do not by themselves prove the candidate's consumer graph identity.

Wyvern evidence was exchanged using context packs `pk_nry25fav` and
`pk_n7jcfn3g`, plus documentation pack `pk_qmi5dgej`; this document contains
the durable decisions and needs no pack to execute. Expert review covered
Pandalump (architecture), Wafflecat (alternatives), Telefono (contracts),
Dinolump (maintenance), Buzzy Bee (build operations), and Doggylump (failure
handling). All actionable findings were incorporated: renamed dependencies and
target/feature cases; tracked fixture recipes; actual packaged consumer builds;
environment invalidation; actionable failures; and a compilable upstream-growth
fixture. No review blocker remains. Record validation receipts in the PR; keep
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
