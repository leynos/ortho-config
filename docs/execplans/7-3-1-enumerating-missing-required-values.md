# Enumerate missing required configuration values

Status: DRAFT — awaiting explicit approval before implementation.

This execution plan (ExecPlan) is a living document. Keep the constraints,
progress, discoveries, decisions, verification, conformance, and outcomes
current at every milestone. This change delivers the plan only. Publishing or
merging this plan does not authorize implementation.

## Purpose / big picture

A caller with several absent configuration values should receive one typed
error listing every missing requirement and the actual ways to supply it. An
agent should be able to use that diagnostic to construct a successful second
invocation, without discovering one more absent field on each attempt. Roadmap
task 7.3.1 follows the completed 5.1.1 truth audit.

Success means two absent fields, including a supported nested field such as
`database.host`, appear together, with verified CLI, environment, and file
supply paths where those sources are supported. Supplying those values makes
the next load succeed. Invalid values remain invalid-value errors; no values or
secrets are copied into the missing-values report.

## Constraints

- Obtain explicit approval of this plan before changing runtime code, macros,
  tests, dependencies, or the design's implemented-status claims.
- Preserve source precedence, collection merge behaviour, profile selection,
  injected source isolation, clap help/version behaviour, and source failures.
- Never derive requiredness from documentation-only overrides. Never classify
  every deserialization failure as absence or parse an English error string.
- Never advertise a source spelling that the actual provider cannot consume.
  A field excluded from CLI parsing must have no invented CLI suggestion.
- Enumerate supported nested requirements; do not pretend that arbitrary
  custom Serde implementations expose a schema. The coverage contract below
  must be documented and tested explicitly.
- Do not introduce source-API compatibility wrappers for this pre-1.0 crate.
  Update affected callers together. Do not change documentation IR, agent
  context, or policy-report schema versions merely to carry runtime errors.
- Use capability-based filesystem helpers and injected environment sources.
  Process-environment mutations require existing shared guards. Keep Rust files
  under 400 lines, dependencies on caret requirements, and warnings denied.
- Run gates sequentially through the scrutineer agent, with logs in `/tmp`.
  Preserve compiler admission and the shared Cargo cache. Do not install or
  alter Whitaker. Do not use `/tmp` as a build directory.

## Tolerances (exception triggers)

Stop and record a proposed deviation, with status BLOCKED, if recursive schema
coverage cannot meet the contract without changing unrelated provider or Serde
behaviour; if a new production dependency is needed; if error handling requires
replaying custom deserializers; or if source naming cannot be made truthful
without a broader configuration-format migration. Obtain explicit approval of
the deviation before continuing.

The new typed error and the narrowly scoped nested-schema API proposed here are
part of the approval request. M1 signature refinements implementing the
approved trait and marker contract are in scope; changes to supported cases,
bounds, or source semantics require a recorded deviation. Other public API
changes require renewed approval. Test dependencies named in this plan are
authorized. No arbitrary time limit applies. Report infrastructure failures
without bypassing them. After two targeted repairs of the same failing gate,
reassess the hypothesis and record evidence before further changes; do not
repeatedly rerun it.

## Risks

The highest risk is false requiredness: optional fields, defaults, collections,
and custom deserialization do not all follow a non-Option rule. Mitigate with
an explicit semantic matrix and real generated-loader fixtures in M1.

Nested types and flattened fields are a high-likelihood design risk. A derive
macro cannot inspect another type's syntax. Use explicit schema composition;
make unsupported opaque cases visible in documentation and the diagnostic
coverage contract, and stop if this cannot satisfy the approved scope.

Source names can diverge from documentation metadata, especially Serde renames,
clap overrides, subcommands, and environment separators. Test each advertised
route by actually supplying a value through it. Metadata snapshots alone are
insufficient.

An abstract model can describe the wrong loader. Keep the runtime-to-model
mapping explicit and test its adapters. Verus is not configured in this
checkout. The proposed collector is straightforward filtering; property tests
are proportionate, and no formal verification result is claimed.

## Progress

- [x] (2026-10-14) Read repository guidance and requested planning skills.
- [x] (2026-10-14) Rename the branch and configure its requested upstream.
- [x] (2026-10-14) Use three Wyvern investigations for runtime, requirements,
  and verification evidence, exchanged through context pack `pk_knocbild`.
- [x] (2026-10-14) Draft contracts, proof sketch, milestones, and acceptance.
- [x] (2026-10-14) Complete six-perspective community-of-experts review
  across three agents and reconcile its findings.
- [x] (2026-10-14) Preliminary formatting, Markdown/spelling, and diagram
  validation passed; the final candidate will be rechecked before commit.
- [ ] Commit and publish the reviewed plan as a draft PR.
- [ ] Obtain explicit plan approval.
- [ ] M1: establish exact schema and source contracts with executable evidence.
- [ ] M2: implement and property-test collection with typed diagnostics.
- [ ] M3: integrate all supported generated loading paths and recovery tests.
- [ ] M4: reconcile documentation, gates, and roadmap completion.

## Surprises & discoveries

The historical proposal names `ortho_config_macros/src/derive/load_impl.rs` and
a final Figment extraction. The current path is `derive/load_impl/source.rs` to
declarative merging, then generated `finish_tokens` in
`derive/generate/declarative/merge_tokens.rs`, followed by
`ortho_config/src/declarative/convert.rs::from_value_merge` on a JSON value.
Integrate at that final value boundary after collection overlays, rather than
adding a parallel Figment pipeline.

`OrthoError::MissingRequiredValues` does not exist. The current regression in
`ortho_config/tests/clap_integration/error_cases.rs` expects `Merge` for an
absent field. Discovery's `missing_required_error` concerns missing files,
which is a different condition and must retain its existing meaning.

`derive/generate/docs/fields/resolution.rs::resolve_required` accepts a
`doc.required` override and treats collections as non-required. It is useful
prior art, not an authoritative validation predicate. Documentation metadata is
recursive for subcommands, not for arbitrary nested data structures.

The requested `docs/rtest-bdd-users-guide.md` spelling refers to the existing
`docs/rstest-bdd-users-guide.md`; use the latter. The live Makefile's test gate
includes all targets/features and Python tests, beyond the older summary in
AGENTS.md. No repository Verus harness or proof target currently exists.
Ordinary config fields are wrapped in Option in the generated CLI, allowing
missing values to reach final merge; explicit clap constraints remain separate.

## Decision log

D1 (2026-10-14, proposed): add structured missing-value data rather than the
historical string-only variant. Callers must inspect missing paths without
parsing localized prose. Update the improved-error design and record an ADR
when implementation begins; this plan approval includes that design direction.

D2 (2026-10-14, proposed): audit and reuse existing key/flag/environment
resolution helpers, but keep runtime requiredness separate from descriptive
metadata. Avoid coupling loading to `OrthoConfigDocs` or `Default` merely to
obtain diagnostics.

D3 (2026-10-14, proposed): introduce explicit recursive schema composition for
nested config types, with an opt-in field marker and a companion trait, rather
than guessing nested leaves from JSON or repeatedly deserializing failures. M1
is a feasibility gate for this proposal, not permission to silently reduce
7.3.1 to top-level fields. Unsupported custom deserialization must retain its
existing error behaviour and be documented as outside schema enumeration.

D4 (2026-10-14, revised after expert review): use property tests and negative
controls for ordered aggregation. A formal proof of a Boolean filter would
prove straightforward control flow while leaving the actual risks, schema
fidelity and source resolution, outside the proof. Retain the independent
specification below but do not introduce an artificial Verus milestone. If M1
reveals non-trivial contractual logic requiring an exhaustive guarantee, stop
and revise the proof sketch and implementation plan together for approval.

## Outcomes & retrospective

The Wyvern team established the current merge boundary. Pandalump/Wafflecat
review removed disproportionate proof tooling and separated child schemas from
CLI derivation; Telefono/Buzzy Bee review corrected parent presence, naming,
activation, finite schema construction, and duplicate evaluation;
Doggylump/Dinolump review clarified checkpoint gates and classification tests.
The trade-off favours real-provider evidence over proving a trivial filter. No
feature implementation or proof has been executed. Documentation validation and
publication evidence is supplied in the draft PR. All six review perspectives
confirmed their findings resolved. Preliminary `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie` passed. A final wording correction
requires a final documentation gate run before commit; the PR records that
result without implying runtime validation. Implementation completion requires
every acceptance item below; a plan PR cannot mark roadmap item 7.3.1 done.

## Context and orientation

`ortho_config` owns runtime loading, errors, source providers, and merge logic.
`ortho_config_macros` generates configuration loading and merge implementations.
`test_helpers` contains shared test infrastructure. `cargo-orthohelp` consumes
documentation metadata; it must not become a runtime dependency.

Read `docs/contents.md` and `docs/repository-layout.md` first. The central
integration points are `ortho_config_macros/src/derive/load_impl/source.rs`,
`ortho_config_macros/src/derive/generate/declarative/merge_tokens.rs`,
`ortho_config/src/declarative/convert.rs`, and
`ortho_config/src/error/types.rs`. Inspect error re-exports, localization, and
telemetry matches when adding a variant. Existing naming helpers are in
`ortho_config_macros/src/derive/parse/serde_attrs.rs`, `derive/build/cli/`, and
`derive/generate/docs/fields/`.

## Conformance basis

The baseline is commit `dc979878b5728bbb23866df9c05c2570bdb5629e`. There is no
separate terms-of-reference document for this task. Governing requirements are
roadmap 7.3.1 and the user's planning and testing instructions. The completed
`docs/execplans/5-1-1-reconcile-design-with-actual-error-surface.md` is audit
evidence, not an implementation specification.

- R1: all missing requirements together, from
  `docs/improved-error-message-design.md` §§1–3. M1–M3 discharge through typed
  error tests, collection properties, and nested behavioural recovery.
- R2: truthful CLI/environment/file routes and precedence, from that design
  and `docs/design.md`. M1 and M3 discharge by replaying advertised routes.
- R3: an agent can self-correct from one diagnostic, from roadmap 7.3.1 and
  `docs/agent-native-cli-design.md` §6.3. M3 discharges by a second successful
  child-process invocation.
- R4: error and metadata ownership, from `docs/cli-localization-design.md`,
  `docs/cargo-orthohelp-design.md`, and
  `docs/clap-dispatch-and-ortho-config-integration.md`. M2–M4 preserve existing
  error classification outside this feature and avoid schema ownership drift.
- R5: quality and documentation requirements. M1–M4 use the testing references
  below and M4 updates user, developer, and architecture guidance.

Read `docs/rust-testing-with-rstest-fixtures.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/localizable-rust-libraries-with-fluent.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`,
`docs/rstest-bdd-users-guide.md` before writing the corresponding tests,
helpers, or localization. The requested `docs/scripting-standards.md` is absent
at the baseline. Do not invent its contents: use AGENTS.md and the developer
guide for existing script conventions, record this missing reference, and
consult the supplied standard if it becomes available. No new script is needed
for this implementation.

Use the `codegraph-mcp` and `rust-router` skills for navigation and Rust work;
route error design to `rust-errors`, fixtures to `rust-unit-testing`, and
verification to `rust-verification`, `proptest`, and `verus`. Use `execplans`
to maintain this file, `firecrawl-mcp` for external gaps, and
`logisphere-experts` to review material design changes. Follow `commit-message`
and `pr-creation` for delivery. Read each skill's SKILL.md before applying it.

## Behavioural contract and abstract specification

Define a requirement by its canonical sequence of configuration key segments,
its activation condition, and its supported supply routes. Segments distinguish
a literal key containing a dot from a nested path. Rendering joins or quotes
segments only at the presentation boundary. Schema order is declaration order,
with nested children visited in declaration order. Canonical paths are unique.
Distinguish canonical diagnostic identities, accepted deserialization
keys/aliases, and source serialization names. The existing Serde helpers
resolve serialization names and do not establish the complete deserialization
contract. M1 must account for deserialize-only and asymmetric renames and
aliases, including canonical-plus-alias duplicate input. Never label an
accepted alias as missing; preserve existing errors for unsupported name
combinations instead of inventing a mapping.

The final merged value includes defaults, file/profile layers, environment,
CLI, and collection overlays according to existing precedence. For each
activated requirement, classify its observation as absent, present, or blocked
by an invalid ancestor. Only absent observations belong in the missing set.
Explicit null is present and is left to Serde's type rules; an empty string or
collection is likewise present. A scalar where a nested object is expected is
invalid, not a collection of missing children.

Optional absent parents deactivate children. An absent required nested parent
activates its declared required leaves. If there are no such leaves, report the
parent itself with an object-valued supply route (including an empty
table/object): otherwise an absent required object containing only
optional/defaulted children would escape the report. A defaulted parent follows
the actual loader's default semantics. Selected subcommands activate only their
own requirements. A non-selected branch must never contribute missing fields.

The report is the ordered subsequence of activated requirements observed
absent. It contains no duplicates, no supplied values, and no unrelated
requirements. Every route in a reported item must be accepted by the actual
CLI/provider for that canonical path. Route precedence is CLI, environment,
configuration file; profile layering remains within the file contribution. Lack
of a route removes that suggestion, not the missing requirement.

Source/discovery/parse failures retain precedence because no trustworthy merged
value exists. For a valid merged object containing both absent requirements and
invalid present values, report the complete absence set first; after
correction, normal deserialization may report invalid values. One-round
recovery is promised when the sole defect is missing required values, not for
arbitrary invalid input. Help, version, unknown arguments, and clap-only
requirements keep clap semantics.

## Verification plan

### Proof sketch before implementation

Let D be a finite sequence of distinct canonical requirement identities, and
let A and P be equal-length Boolean sequences representing activation and
presence. The adapter admits only observations whose ancestors are valid;
blocked observations remain outside this kernel and preserve invalid-value
handling. Define the specification independently as the subsequence of D at
indices i where A[i] is true and P[i] is false.

The executable collector consumes these observations once in schema order.
Inspect the final value without replaying defaults or custom deserialization.
Collector cost is linear in observation count; path lookup also costs path
depth. Materialize route strings and report payloads only for missing entries
where practical. Use instrumented default/deserializer fixtures to detect
double evaluation, without adding benchmarks or a telemetry subsystem. Its
preconditions are equal sequence lengths and distinct identities. Its
postconditions are soundness (every output is activated and absent),
completeness (every activated absent identity occurs), uniqueness, and
preservation of input order. Representation relates a runtime path to exactly
one schema identity; identity assignment cannot depend on hash iteration or
rendered path strings.

Candidate semantic lemmas are prefix completeness after processing k entries,
order preservation when appending the next missing entry, and fixed-schema
repair: with activation and ancestor validity unchanged, changing an absent
requirement to present removes exactly that identity and cannot add another.
Supplying an optional parent can activate children; it is deliberately outside
that relation. Sketch induction on processed length with an empty-prefix base
and separate selected/unselected cases. These are hypotheses about proof
structure, not a requirement to retain redundant lemmas. Do not decide solver
triggers or auxiliary collection algebra before seeing verification conditions.

These candidate reasoning steps pressure-test the API; they are not introduced
formal theorem declarations. The collector is straightforward control flow, not
new contractual business logic warranting a prover. Discharge its executable
contracts through V1/V2 and deliberate omission, insertion, and ordering
faults. Use inhabited witnesses: empty input, no missing values, two missing
values separated by a supplied value, and an absent required object with no
required children. No unbounded proof is claimed by sampled tests.

Verus becomes warranted only if a later approved design introduces a stable,
non-trivial semantic kernel whose guarantee must be exhaustive. Before that
implementation, revise this section with preconditions, postconditions,
representation relationships, and semantic lemma sketches. Add a dedicated
milestone and pinned toolchain/runner, using the `verus` skill and
`rust-prover-tools`; its acceptance must include the following command as well
as `make check-fmt`, `make lint`, and `make test`:

```bash
prover-tools verus run --repo-root . --proof-file verus/missing_required_values.rs
```

That future proof must verify the production kernel or establish an explicit
refinement relation, with adapter tests covering the executable integration. It
must be substantive and non-vacuous, and use no `assume`, `admit`, external
proof bodies, or equivalent trusted shortcuts. Leave speculative theorem
statements in documentation until that approved milestone. Do not add passing
proof stubs, install Verus, or treat tool absence as a successful proof now.
Formatting, localization, and integration plumbing remain conventionally tested.

### Executable assurance and external assumptions

V1 tests requiredness against real generated loaders, using `rstest` tables for
Option, field/container defaults, inferred clap defaults, collection
strategies, renames, aliases, skipped fields, explicit null, empty values, and
nested parent activation. Documentation overrides must not affect runtime
decisions. Red failures must identify wrong membership, not merely an absent
API symbol.

V2 uses `proptest` for 0–64 distinct requirement paths, all presence/activation
combinations generated directly, and deterministic order and repair relations.
The expected set is computed independently. Include empty, all-present,
all-missing, interleaved, and nested cases without rejection-heavy generators.
Tests connect runtime observations to the abstract model; they are sampled
integration evidence, not an unbounded proof. Seeded omission, inversion, and
reordering faults must fail the appropriate assertion.

V3 uses macro expansion unit tests and trybuild fixtures to validate crate
renaming, generic fields, explicit nested schema bounds, compile diagnostics,
and supported Serde naming. Compile-fail tests are only for invalid attributes
or schema contracts; missing runtime values are not compile errors.

V4 uses `insta` snapshots for English display and the supported localized
presentation path if one is exposed. Keep typed assertions alongside snapshots.
Use `googletest` matchers for membership/classification and `pretty_assertions`
for exact ordered paths and multiline output. Reuse workspace versions; avoid
migrating unrelated tests. The upstream GoogleTest guidance supports placing
`#[gtest]` before `#[rstest]`; verify the pinned crate's integration with a
small compiling fixture.

V5 uses `rstest-bdd` and a child-process fixture for one-diagnostic recovery
through CLI, environment, file, and mixed sources. Reuse injected sources and
capability-based temporary directories. Child environment isolation must not
mutate the parent's process environment. Assert nonzero failure, diagnostic on
stderr, no success output or side effects, and zero exit on corrected input.
The fixture tests a consumer's exit policy; the library itself must not exit.

Third-party assumptions: Serde accepts its documented default/rename rules;
clap's argument definitions match accepted names and parse errors; existing
providers implement the documented source mappings. Exercise those boundaries
with real loaders, not mocks alone. Figment's `find_value` returns raw values
and can fail on combined-provider errors; it is not a safe Boolean absence
oracle and is not the proposed final-value implementation boundary.[^figment]

## Interfaces and dependencies

Propose `OrthoError::MissingRequiredValues(Box<MissingRequiredValues>)`, with
public inspectable payload types exported through the existing error facade.
The payload owns an ordered non-empty list of `MissingRequiredValue` entries,
canonical segment paths, and optional real supply routes. Keep actual values
out of it. Construct non-empty reports through a fallible/private constructor,
not unchecked caller assumptions. Display formats the payload; it does not
perform discovery or read process arguments/environment.

Propose a companion `RequiredConfigSchema` trait with a schema-construction
method and derive emission from `OrthoConfig`. A field marker
`#[ortho_config(nested)]` opts into composing a child's schema and adds a bound
only to that field's type. Offer a schema-only derive of `RequiredConfigSchema`
for child types, plus a documented manual implementation contract for custom
types; children must not need to derive a stand-alone CLI or implement
OrthoConfig merely to expose a schema. Avoid conflicting implementations when
OrthoConfig already emits the companion trait. M1 must demonstrate this wiring
and the real source path for nested values without imposing clap parsing bounds
on nested object types. No fabricated `--database-host` flag is permitted. This
marker composes diagnostic schemas only; it does not add nested provider
capabilities. Include a nested file-only requirement with no CLI suggestion,
generic children, and optional children in M1.

Schemas must be finite. Reject cyclic opt-in schema participation with a typed
schema error and a bounded traversal using a type-identity recursion stack; do
not eagerly expand optional boxed recursive children forever. Distinguish
revisiting a type on the current ancestry from reusing it in sibling fields.
Test direct and indirect cycles, finite repeated children, and document this
restriction. If this requires broader provider or type-system changes, stop
under the existing tolerance rather than silently pruning requirements.
Represent fields as leaf or nested schema nodes, including optional/default
activation. Flattened composition must be explicit and tested; do not infer it
from a dotted string. M1 fixes the precise signatures and attribute grammar in
the design document before M2 consumes them.

Do not require arbitrary Serde types to implement the companion trait. Such
opaque fields can be diagnosed as absent direct values, but absent leaves
inside a supplied opaque value remain Serde errors. Custom deserializers and
unsupported flatten contracts must not acquire fabricated recursive reports.
This restriction must be prominent in public documentation, with an example
showing how nested schema participation obtains full enumeration.

Put runtime schema and collector code in a focused
`ortho_config/src/required_values/` module; presentation belongs under
`ortho_config/src/error/`. Generated code needs a documented support surface
reachable from downstream macro expansion. Ownership and permitted call-sites
must be recorded in `docs/design.md`. Sweep for equivalent helpers before
adding each abstraction. Reuse naming functions where their semantics match;
never consume `doc.env` or `doc.file_key` overrides without provider evidence.

No production dependency is planned. `googletest` and `pretty_assertions` are
already workspace dependencies; use authorized dev dependencies for `insta` and
`proptest` where missing. No verifier dependency is introduced by this plan.

## Milestones and plateaus

### M1 — settle schema fidelity before integration

After approval, add focused characterization fixtures and the smallest red
regression for multiple absent fields. Audit default production, Serde naming,
CLI omission, and nested/flatten generation. Test the proposed schema approach
with required nested leaves and optional/defaulted parents. Run the aggregate
red reproducer as a temporary working-tree experiment, retain its failure log,
then remove it before the M1 checkpoint. Reintroduce the permanent regression
in the integration milestone and observe red again. Do not commit failing tests
or disable them to manufacture a green checkpoint.

Record a support matrix and exact API signatures in the improved-error design
and a new ADR linked from `docs/design.md` and `docs/contents.md`. This
milestone must show the nested schema is feasible without evaluating custom
defaults or deserializers twice, or it stops for an approved deviation. Do not
retrofit arbitrary Serde reflection. Resolve clap eager-required parsing
explicitly: configuration-backed fields must reach merging, while help/version
and genuine CLI-only constraints retain their existing paths.

Acceptance E1 is passing characterization coverage and a recorded expected red
failure for the aggregate contract. R1/R2/R4 are specified, not yet delivered.
Run all ordinary gates before the coherent schema/design checkpoint commit.
Remaining gap: production diagnostics and collection property coverage.
Recovery is to revert the checkpoint atomically; no migration layer is needed.

### M2 — typed report and property-tested collection

Implement runtime schema observations, the pure collector, the error payload,
and display. Write V1/V2 tests before their production logic, observe the
expected failure, then make the smallest implementation pass. Audit the
runtime-to-model mapping, especially canonical paths and ancestor validity.

Acceptance E2 comprises the schema/collector-only subset of V1 and V2 evidence
(the real-loader assertions become acceptance in M3), typed payload/display
snapshots, inhabited boundary witnesses, and rejection of deliberate omission,
insertion, and ordering faults. Restore the correct code after each negative
control. Run `make check-fmt`, `make lint`, and `make test` sequentially.
Remaining gap: generated loader wiring and user recovery. Revert M2 as one
coherent unit if its API or representation cannot satisfy the contract.

### M3 — generated loaders and one-diagnostic recovery

Generate schema observations in the declarative finish path after collection
overlays and before final deserialization. Route ordinary loads,
injected-source loads, profile loads, and selected-subcommand merge entrypoints
through the shared mechanism wherever they have the approved schema. Preserve
raw generic `from_value_merge<T>` behaviour when only `DeserializeOwned` is
known; do not claim it has metadata it cannot possess. Add tests proving this
boundary.

Add error classification tests for telemetry and localization consumers of
OrthoError. Preserve existing source attribution and metric labels for old
variants, choose a stable low-cardinality category for the new variant, and
never emit supplied values or secrets. The initial display contract is English;
existing localization entrypoints must retain it deliberately when no dedicated
translation exists, without panics or rewriting the canonical typed paths.
Document that fallback rather than expanding phase 11 localization scope.

Use the audited parsing and key-generation helpers to produce real routes.
Account for renamed keys, custom long/short flags, prefixes, no CLI route,
subcommand invocation context, and literal dotted keys. Take a program label
from the actual command/loading context; deterministic direct-merge fallback
uses the configuration type name. Never read ambient `std::env::args` inside
pure diagnostics or injected loaders. Keep canonical data locale-independent;
use existing localizer interfaces for any supported translated rendering.

Add the following specification at
`ortho_config/tests/features/missing_required_values.feature`, with a runner in
`ortho_config/tests/missing_required_values_bdd.rs`:

```gherkin
Feature: Recover all missing configuration from one diagnostic
  Scenario: Correct a root value and two nested required values together
    Given a schema-enabled configuration with reference, database.host and database.port required
    And no source supplies those values
    When the configuration is loaded
    Then one missing-values error names all three paths exactly once
    And every suggested route is accepted by its provider
    When valid values are supplied using the reported routes
    Then the next load succeeds without another missing-values error

  Scenario: Optional and supplied values are not reported
    Given an absent optional parent and a supplied required value
    When the configuration is loaded
    Then neither value nor the optional parent's children are reported

  Scenario: Invalid nested input is not absence
    Given a scalar where a nested configuration object is required
    When the configuration is loaded
    Then the existing invalid-value error is retained
```

Use scenario outlines or parameterized runners for each supply route. Add
unit/macro/e2e coverage for selected versus unselected subcommands and preserve
existing invalid-input and help/version regressions. An assertion must prove
all required paths appear, not merely that the message contains “missing”.

Acceptance E3 is V3–V5 passing, all standard gates passing, and E2 property
tests rerun if its kernel or representation changes. R1–R4 are now delivered
within the explicit schema coverage contract. Commit this integrated plateau;
no private compatibility adapters are necessary. Remaining gap: final public
guidance and roadmap closeout.

### M4 — documentation and final conformance

Update `docs/users-guide.md` for typed matching, coverage limits, and a worked
one-round correction. Update `docs/developers-guide.md` for schema ownership,
source injection, tests, and the verification scope and limits. Reconcile
`docs/improved-error-message-design.md`, `docs/design.md`, and the phase-5
status paragraph in `docs/agent-native-cli-design.md`. Reference the ADR where
schema representation and proof obligations influence architecture. Update the
changelog using its skill; update documentation examples/registries if their
contracts change.

Only after E1–E3 and all gates pass, mark roadmap task 7.3.1 and its three
subtasks done (`[x]`), using the `mapsplice` skill for structural roadmap
edits. Run documentation gates and ordinary gates after this final milestone.
Record E4, reconcile every upstream discrepancy, and mark this plan COMPLETE
only when implementation is complete. The current planning PR leaves all
feature checkboxes unchecked.

At each milestone check assigned requirements, approved design, external
assumptions, public/dependency boundaries, unchanged persisted/wire schemas,
and current evidence links. Record deviations before proceeding. Changes to
these decisions require updating both the design/ADR and this plan.

## Concrete steps and acceptance commands

Run from the repository root on `7-3-1-enumerating-missing-required-values`.
Record `git status --short` and the head before edits. Use scrutineer for
sequential gates and `set -o pipefail` with `tee` to preserve failures. Example
final gate sequence:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-7-3-1.out
make lint 2>&1 | tee /tmp/lint-ortho-config-7-3-1.out
make test 2>&1 | tee /tmp/test-ortho-config-7-3-1.out
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-7-3-1.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-7-3-1.out
```

Run commands individually and stop at failure. No Verus proof milestone is
selected for this design. If a later approved revision adds one, run its
explicit proof gate in addition to these commands. Run `make fmt` after
documentation edits, inspect its diff, and retain only relevant changes. For
implementation, run relevant baseline tests before edits; use a focused red
command before production changes, the same command after the fix, then full
gates. Suggested new targets are:

```bash
cargo test -p ortho_config --test missing_required_values
cargo test -p ortho_config --test missing_required_values_bdd
cargo test -p ortho_config --test missing_required_values_e2e
cargo test -p ortho_config_macros
```

Create those test entrypoints in M2/M3; they do not exist at the baseline.
Include property tests in the first target. Record concrete test names, red
assertion failures, green counts, property/negative-control results, and log
paths in Progress. A zero-test run, skipped proof, or incomplete compiler
invocation is not a pass.

For this documentation-only plan publication, run `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie`. No runtime code changes justify a new
full Rust compilation during planning. Implementation milestones must run the
three ordinary gates explicitly requested above.

## Idempotence and recovery

Use temporary capability-controlled fixture directories and injected sources;
rerunning tests must not alter the caller's files or environment. Revert failed
experiments and deliberate mutations before gates. Preserve meaningful red
failure evidence in logs, not failing committed tests. Commit each passing
logical change using the commit-message skill. Reverting a milestone must
revert its tests, API changes, and documentation together.

Push with the standard SSH Git client after verifying the GitHub identity.
Refresh the explicit `origin` tracking ref after URL-based pushes and confirm
the remote SHA. Publish this plan as a draft PR titled
`Plan: Enumerate missing required values (7.3.1)`, with a branch-wise plan link
and a terminal References section linking the Lody session. Approval must be
explicit before M1 starts.

## Artefacts and research notes

Wyvern evidence is available in context pack `pk_knocbild`. The durable code
paths and decisions are repeated above so execution does not depend on that
pack's lifetime. Firecrawl was used to inspect upstream Figment and GoogleTest
documentation on 2026-10-14.[^figment][^googletest]

[^figment]: [Figment 0.10.19 API](https://docs.rs/figment/0.10.19/figment/struct.Figment.html#method.find_value).

[^googletest]: [GoogleTest Rust interoperability](https://github.com/google/googletest-rust#interoperability).

Revision note (2026-10-14): initial draft replaces the obsolete Figment
preflight assumption with the live declarative merge boundary, and makes
recursive schema coverage an explicit approval decision. Expert review removed
an artificial filter-proof milestone and strengthened nested parent and repair
semantics.
