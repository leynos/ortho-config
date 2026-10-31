# Add profile redaction metadata (9.1.2)

Status: DRAFT — approval required before implementation.

This execution plan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current at every milestone. All APIs
and files described as proposed below are future work.

## Purpose / big picture

Applications should describe their profile fields and publish profile names
without publishing sensitive values. Roadmap 9.1.2 adds explicit secret and
reference-only declarations, removes their value-bearing metadata before
context or documentation rendering, and provides a names-only context input. A
consumer can still load a secret from a profile: redaction affects disclosure,
not configuration resolution.

Success is observable when two fixtures with identical public metadata but
entirely different protected defaults, examples, and profile contents produce
identical context JSON and generated documentation. Public field descriptors
and deliberately supplied profile names remain visible. Ordinary fields retain
their current defaults and examples, subject to the conservative
command-example policy below. No profile store, secret resolver, or domain
policy is introduced.

## Conformance basis

The inspected baseline is commit `dc979878b5728bbb23866df9c05c2570bdb5629e`.
The governing requirements are [roadmap](../roadmap.md) 9.1.2 and
[agent-native CLI design](../agent-native-cli-design.md) §6.7. There is no
separate terms-of-reference document for this task. Prerequisite 9.1.1 is
checked complete and implemented;
[ADR-012](../adr-012-profile-selection-and-layering.md) controls selection,
precedence, and static-versus-runtime metadata.

Use these trace identifiers throughout implementation:

- R1: explicit secret/reference-only field classification → M1 → derive,
  serialization, and compile-fail tests.
- R2: sensitive values absent from context and generated examples → M1 and
  M2 → noninterference properties, renderer snapshots, and executable tests.
- R3: names exposed independently of contents → M1 and M2 → names-only API
  tests and context end-to-end tests.
- R4: reusable contracts, unchanged loading and precedence → every milestone
  → existing profile suites and conformance review.

Read [design](../design.md) for crate boundaries;
[CLI localization design](../cli-localization-design.md) and
[localizable Rust libraries with Fluent](../localizable-rust-libraries-with-fluent.md)
for localization ownership;
[cargo-orthohelp design](../cargo-orthohelp-design.md) for the bridge and
renderers; [clap integration](../clap-dispatch-and-ortho-config-integration.md)
for root selectors; and [error design](../improved-error-message-design.md) for
typed diagnostics.

Use [rstest fixtures](../rust-testing-with-rstest-fixtures.md) for reusable
setup and [doctest guidance](../rust-doctest-dry-guide.md) for examples.

Use the
[dependency injection guide](../reliable-testing-in-rust-via-dependency-injection.md)
for isolated configuration sources.

Use
[complexity guidance](../complexity-antipatterns-and-refactoring-strategies.md)
for module decomposition and [rstest-bdd guide](../rstest-bdd-users-guide.md)
for behavioural scenarios.

The requested `docs/rtest-bdd-users-guide.md` is a misspelling of that existing
BDD guide. The requested `docs/scripting-standards.md` is absent; use
[developers' guide](../developers-guide.md), [repository layout](../repository-layout.md),
and `AGENTS.md` for existing script conventions. Do not invent a replacement
standard or claim it was read.

Load the `codegraph-mcp`, `rust-router`, and `execplans` skills first. Route
verification through `rust-verification` and `verus`, tests through
`rust-unit-testing` and `proptest`, and publication through `commit-message` and
`pr-creation`. Use `firecrawl-mcp` for external documentation. Use the
`logisphere-experts` skill for design review. Resolve skill paths from the
active skill catalogue rather than embedding workstation-specific paths here.

## Context and orientation

`ortho_config/src/profile/` owns profile names, selection, extraction, and load
outcomes. `ProfileName` validates non-empty ASCII letters, digits, underscores,
and hyphens, rejecting the reserved name `default`. `AvailableProfileNames`
sorts, deduplicates, and caps diagnostic names at 16; it is not a complete
profile catalogue and accepts raw strings.

`ortho_config/src/docs/ir.rs` defines the documentation intermediate
representation (IR), including `FieldMetadata`, `DocProfilesMeta`, and
`DocMetadata`. The macro builds these in
`ortho_config_macros/src/derive/generate/docs/fields/mod.rs` and
`ortho_config_macros/src/derive/generate/docs/mod.rs`.
`ortho_config/src/docs/mod.rs` owns exports and IR version `2.0`.

`cargo-orthohelp/src/schema/mod.rs` currently mirrors the library schema as
separate Rust types; it does not re-export them.
`cargo-orthohelp/src/agent_context/mod.rs::bridge_ir_to_agent_context` builds
context; `build_input` currently copies defaults and enumerated values.
`cargo-orthohelp/src/ir.rs::localize_doc` and `localize_field` copy defaults
and examples for human renderers. `roff/` and `powershell/maml/` under
`cargo-orthohelp/src/` render those structures. Inspect `bridge.rs`,
`generation.rs`, `cache.rs`, and `output.rs` there to cover intermediate and
cached output, not only final files.

`ortho_config/src/agent_context/profiles.rs::ProfilesDeclaration` currently
contains support, selector, and optional listing-command metadata. Static
context contains no active selection or profile bodies. `list_command`
population and discovery/store helpers remain roadmap 9.1.3 work.

## Behavioural contracts and proposed interfaces

### Classification and disclosure boundary

Propose field syntax `#[ortho_config(profile_field = "secret")]` and
`#[ortho_config(profile_field = "reference_only")]`. Introduce the shared
`ProfileFieldExposure` enum with `Secret` and `ReferenceOnly` variants in a
focused `ortho_config/src/docs/profile_redaction.rs` module. Add optional
`profile_field: Option<ProfileFieldExposure>` to `FieldMetadata`; absence means
ordinary existing disclosure. Reject unknown labels and duplicate annotations
with span-local macro diagnostics. Labels are mutually exclusive.

A secret field contains confidential material. A reference-only field names
material the application resolves elsewhere; OrthoConfig neither resolves nor
publishes the actual reference value. Both labels suppress literals, including
credential-bearing locator strings. Only their semantic labels differ.
Annotations apply to every generated representation of that field, regardless
of whether profiles are enabled on its containing derive. This permits a nested
command or reusable configuration type to declare sensitivity without changing
loader opt-in rules.

Protected fields retain their identifier, CLI spelling, environment-variable
name, configuration key, requiredness, help, and structural type. Their entire
`default` becomes `None`, examples become empty, CLI `possible_values` become
empty, and all nested `ValueType::Enum` members become empty recursively.
Protect list/map inner types too. Do not evaluate default expressions to obtain
redaction text. Omission is preferable to inserting a fake value that consumers
might execute. Help, notes, links, identifiers, and custom type names are
explicitly public author-controlled descriptions; authors must not embed
secrets there. This is a structural disclosure contract, not a scanner for
arbitrary confidential strings or a guarantee about arbitrary application logs.

Command examples are unstructured strings with no dependable field mapping. If
any field anywhere in a document tree is protected, remove all command-level
examples throughout that tree before localization. Also remove protected field
examples in full, including their titles and descriptions. This broader removal
is intentional: a parent or sibling example may exercise a protected child. Do
not add substring replacement, heuristic parsing, or a raw bypass. Ordinary
field examples are authored public data and remain outside that cross-command
suppression; document that authors must not place another field's protected
values in these public channels.

### Shared projection and context representation

Replace the generator schema mirror in `cargo-orthohelp/src/schema/mod.rs` with
explicit public re-exports from `ortho_config::docs`, preserving existing
module paths. ADR-003 assigns ownership to that library and does not require
distinct Rust mirror types. Record the pre-1.0 type-identity change in the new
ADR and developers' guide, and update schema tests to test actual wire
defaulting and round trips rather than comparing a type with itself. This
removes schema drift and permits one projection without JSON conversion,
fallible schema-conversion APIs, or a generic visitor framework. This ownership
consolidation is part of M1, not a temporary compatibility layer.

Propose `DocMetadata::redacted_for_output(&self) -> DocMetadata`, backed by one
library-owned recursive projection. It returns an owned sanitized copy and
never mutates runtime configuration. Before extracting a helper, repeat the
repository search for an equivalent; the planning sweep found none. Its only
responsibility is the above disclosure policy. Macro-generated metadata,
context conversion, and documentation generation use this policy; renderers
must not grow their own competing classification rules. Compute whether any
protected field exists once, then project the complete tree once per public
conversion boundary. Private localization recursion consumes the sanitized tree
without calling the public entry point again. The algorithm performs a constant
number of linear traversals, not one projection per subtree. Verify this
structure with a deep-chain traversal-count regression.

Sanitize derive-emitted metadata before bridge serialization, and sanitize
handwritten or deserialized metadata at public conversion entry points too.
Ensure the bridge cache stores only the sanitized representation and rejects
old cached output by changing the relevant cache identity. Audit policy reports
and error/debug output on these routes for accidental formatting of the raw
metadata. Preserve existing public entry points where feasible, without
creating aliases or compatibility wrappers. This is a pre-1.0 source API:
update struct literals and all in-tree consumers together.

Extend `ProfilesDeclaration` with an omitted-when-empty `redaction` list of
`ProfileFieldRedaction { command_path, field_name, key_path, exposure }`.
`command_path` is the existing tokenized command path; `field_name` is the
emitted metadata name; `key_path` is the optional existing file key spelling.
Use the pair `(command_path, field_name)` as identity, retaining `key_path`
without reparsing dotted strings. Include config-only and hidden fields in this
list even though they do not become invocable `AgentInput` entries. Sort
records deterministically by identity; reject duplicate identities in
handwritten/deserialized redaction lists rather than selecting an arbitrary
label. The current docs builder does not expand flattened containers into
descendant metadata. Reject `profile_field` on clap or serde flattened
containers with a compile-time diagnostic; do not invent a field graph. Support
marked emitted leaf fields and nested subcommand metadata using the existing
identities. Annotations on separately derived child types do not imply
protection in a parent that does not expose those child fields: document that
limitation and require explicitly emitted marked metadata for a protected
flattened use case. A general flatten expansion feature is outside this plan.

For supported profiles, expose that list at `profiles.redaction`. Store the new
list privately with a read-only accessor and
`with_redaction(self, records: Vec<ProfileFieldRedaction>) -> OrthoResult<Self>`
that canonicalizes records and returns a typed error for duplicates or
nonempty records on an unsupported declaration. New error payloads contain only
public identities, never value-bearing metadata.

The generator's `bridge_ir_to_agent_context` therefore returns
`OrthoResult<AgentContext>` instead of `AgentContext`, propagating
invalid-record errors through its callers to the existing application error
boundary. Update every in-tree caller and document this pre-1.0 signature
change; do not unwrap or discard builder failures. Localization remains
infallible because omission itself requires no record-identity validation.

For unsupported profiles, retain exactly `{ "supported": false }`; redaction
still applies to marked field output but no profile declaration is invented.
Keep agent-context schema `1` and documentation IR `2.0` as additive wire
extensions with missing fields defaulting to empty/absent. Invalid recognized
classification strings must fail deserialization, never become ordinary fields.
Old producers without annotations retain their existing disclosure behaviour;
old consumers cannot be assumed to enforce new labels. Hence new producers must
remove sensitive values before serialization, rather than relying on consumer
awareness of the added enum.

### Names without contents

Propose
`ProfilesDeclaration::with_available_names(self,
names: impl IntoIterator<Item = ProfileName>) -> Self`,
adding `available_names` string-array wire data with default/empty omission.
Store validated names privately with a read-only accessor; do not add a public
raw-string vector. For an unsupported declaration return it unchanged; for a
supported declaration, replace the list with sorted, deduplicated validated
labels. This is an explicit consumer-supplied catalogue, not implicit
discovery, active selection, or a capped diagnostic list. Do not reuse the
diagnostic cap for it.

The builder has no profile-body parameter and performs no file, environment,
network, or resolver access. Validate incoming serialized names with the same
`ProfileName` grammar, sort/deduplicate them, and reject nonempty names or
redaction records on an unsupported declaration. Apply the same invariants on
serialization, since the existing public `supported` flag remains mutable. If
it was flipped after adding metadata, fail serialization with a value-free
error rather than emitting an inconsistent declaration. Do not tighten legacy
selection/list-command combinations as an incidental change. Update in-tree
struct literals to constructors because the new fields are private. Consumers
deliberately designate names as public labels: grammar validation cannot detect
a password deliberately used as a syntactically valid name. Static
`cargo-orthohelp` output supplies no names. A downstream runtime context
adapter may explicitly attach names; it must not load resolved configuration
merely to build context.

## Verification plan

Define the model before implementation. A document is a finite rooted tree of
commands. Each field has a public descriptor, optional exposure label, and
value-bearing slots: default, example sequence, CLI possible values, and
recursively nested enum members. Command nodes have an example sequence. A
profile catalogue is a finite map from validated public names to arbitrary
bodies. Two documents are low-equivalent when their public descriptors,
topology, and labels agree; ordinary field slots agree; protected slots may
differ arbitrarily. Command examples must agree when no protected field exists;
they may differ arbitrarily only when a protected field exists and global
suppression applies. Low-equivalence expresses exactly what the output is
allowed to reveal.

Let P be the projection defined above, and N the sorted distinct name
projection. Preconditions are finite well-formed metadata and correctly
classified public descriptions, not an assumption that inputs are already
redacted. The postcondition is absence of protected value slots, preservation
of permitted descriptions, and independence from discarded slots.

Sketch these semantic obligations before choosing executable helpers:

- V1, noninterference: low-equivalent documents have equal P results, for
  arbitrary tree sizes, depths, and protected value contents.
- V2, projection integrity: P preserves identifiers, labels, ordinary field
  slots, and command topology; clears exactly the specified protected slots;
  and P(P(d)) equals P(d).
- V3, names independence: catalogues with identical public key sets produce
  equal N results even when every body differs. Output is deterministic and
  contains no body fragments. The implemented builder takes only validated
  names, making body access unavailable by construction.
- V4, executable correspondence: macro emission, runtime schema, bridge cache,
  localization, and final serializers implement the same projection. Every
  value-bearing slot in the actual types has an explicit model counterpart.
- V5, loading preservation: marking a field changes no load result, selection,
  precedence, CLI acceptance, or source provenance.

Verus is warranted for V1 and V2 over an unbounded recursive metadata tree,
including propagation of the global command-example suppression condition. A
single Boolean classification truth table alone would not justify it. Prove
V1/V2 in proposed `verus/profile_redaction.rs`; prove any supporting semantic
statements there without `assume`, `admit`, external/trusted proof bodies, or
an axiom equivalent to the goal. Potential proof structure is preservation
under field projection and composition across command children; these are
hypotheses, not mandatory lemma boundaries. Defer collection algebra, solver
triggers, and induction helper selection until verification conditions exist.
No speculative executable proof scaffold belongs in this planning PR.

Prefer sharing the small executable projection kernel where Verus supports it.
If a specification mirror is needed, document the abstraction mapping from every
`DocMetadata`, `FieldMetadata`, and `ValueType` variant and review it
alongside production edits. A proof of the mirror is not a proof of serde,
macros, localization, or renderers. Discharge V4 with differential property
tests comparing the production projection against an independently written
model, plus output integration tests. Execute the proved model as the oracle
where practical. If the Rust oracle is a separate implementation, record a
slot-by-slot model-to-oracle correspondence review and shared witness corpus;
that correspondence remains tested/reviewed, not formally proved. Record this
residual testing boundary honestly in the design documentation; do not claim
fully verified binaries.

Use `rstest` tables for R1 syntax, both classifications, absent annotations,
invalid JSON labels, missing optional fields, and unsupported names. Use
`googletest::assert_that!` for structural assertions and
`pretty_assertions::assert_eq!` for equality diagnostics. Existing workspace
dependencies authorize both; avoid non-fatal assertions requiring extra test
context unless the fixture return type supports them.

Use `proptest` for V1–V4 integration: construct paired documents with different
protected sentinels, nested list/map/enum types, duplicate leaf names under
different commands, renames, and reordered input names. Test rejection of
marked flattened containers separately with compile-fail fixtures. Bound
runtime generators to depth 4, 0–8 children/fields per node, and strings up to
128 characters, with a total budget of 32 command nodes and 64 fields per case;
include empty, protected-only, ordinary-only, and mixed trees as explicit
cases. These bounds are tests, not the Verus domain. Avoid heavy filtering,
record class coverage, and persist reduced failures. Test equality of complete
outputs rather than naive substring absence: public text may legitimately equal
a secret value. In dedicated sentinel fixtures choose protected tokens absent
from public data and assert their absence in all artefacts, stdout, stderr, and
captured tracing.

Non-vacuity evidence must include a mixed tree with a protected child and
parent/sibling command examples; a public field whose default stays visible; a
secret and a reference-only field; and two bodies behind the same valid profile
name. Temporarily seed faults that retain one protected default, retain a
nested enum member, and skip a child during projection. Each must fail the
relevant tests; the model-side faults must fail the corresponding Verus
obligation. Remove faults, rerun the affected checks, and record evidence. No
proof may obtain correctness by assuming low-equivalence of its outputs.

V3 uses type/API inspection plus parameterized and property tests; no separate
proof of sorting or name grammar is warranted. V5 uses existing precedence
properties and profile behavioural suites with newly marked fields. Diagnostics
formatting, Fluent resolution, filesystem/cache plumbing, and JSON/roff/XML
escaping use conventional integration tests and snapshots, not artificial proof
obligations. Kani adds no distinct assurance here and is not planned.

Trusted premises are the documented serde serialization behaviour, Fluent
resolution, Rust memory safety, and the verifier/toolchain soundness. Public
metadata is author-approved, and applications classify sensitive fields.
Exercise actual serde and renderer boundaries; do not attempt to prove those
third-party implementations. Timing, memory allocation, arbitrary application
logging, and secrets embedded deliberately in public labels are out of scope.

## Milestones and plan of work

### M1 — Deliver the complete disclosure boundary

After explicit approval, record the contract decisions in a new ADR, choosing
its number from the live ADR inventory. Reference it from
`docs/agent-native-cli-design.md` §6.7 and `docs/cargo-orthohelp-design.md`.
Record ownership and reuse policy in the latter before introducing helpers.

Use Red-Green-Refactor: first add focused tests that demonstrate the missing
annotation/projection and a sentinel leaking through the current adapters. A
compile failure for an unknown proposed attribute proves only the parser gap;
retain a behavioural red test against hand-built metadata as well. Capture the
expected failure before changing production code; never commit failing gates.

Implement the schema ownership consolidation, IR enum and projection, macro
parsing in `ortho_config_macros/src/derive/parse/field_attrs.rs` and its
`FieldAttrs` representation, field emission, context records, and names
builder. Update exports, all construction sites, and direct deserialization
tests. Integrate both output paths and cache handling in the same milestone so
a marker never ships while a supported renderer still leaks its values. Split
growing modules by responsibility to keep source files below 400 lines.

Acceptance is R1–R4 with V1/V2 provisionally established by tests, V3/V4/V5
covered by their test suites, and all ordinary gates passing. It is an
internally coherent but not feature-complete plateau: the formal discharge and
full external workflow evidence remain M2. Commit the gated change; if a
failure occurs, resume from this milestone's red/green logs, or revert its
complete commit without changing profile files. No compatibility layer is
needed for these pre-1.0 source additions.

### M2 — Discharge proofs and external workflow contracts

Implement the substantive Verus model and proofs described above, along with
the documented production mapping. Introduce checksum-pinned
`tools/verus/VERSION` and `tools/verus/SHA256SUMS` using the current
`rust-prover-tools` convention, and a `make verus` target invoking:

```sh
prover-tools verus run --repo-root . --proof-file verus/profile_redaction.rs
```

Read current official tooling documentation through Firecrawl before choosing
pins; do not guess versions or checksums. Use the managed compiler admission
path. If the runner cannot honour it, report that blocker rather than bypassing
wrappers. `make verus` must fail on failed or missing verification and may not
claim a skip as acceptance. Keep it a separately invoked gate; make it explicit
in milestone and release evidence rather than hiding it inside `make test`.

Add `cargo-orthohelp/tests/features/profile_redaction.feature`, registering
steps through its existing `tests/rstest_bdd.rs` support modules. The required
behaviour specification is:

```gherkin
Feature: Profile metadata keeps sensitive values private
  Scenario: Protected metadata is omitted from every output format
    Given a profile-enabled command with secret and reference-only fields
    And their defaults, enum members and examples contain distinct sentinels
    When context JSON and human documentation are generated
    Then public field descriptors remain visible
    And context JSON declares both classifications
    And no protected sentinel appears in any generated artefact

  Scenario: Parent examples cannot disclose a protected child
    Given a nested command tree with a secret child field
    And parent and sibling command examples contain its sentinel
    When documentation is generated
    Then every command-level example collection is empty

  Scenario: Public profile names do not expose profile contents
    Given two catalogues with the same validated public names
    And every profile body differs between the catalogues
    When each consumer attaches only the names to its context
    Then both serialized contexts are identical
    And neither context contains profile bodies

  Scenario: Ordinary metadata retains its behaviour
    Given a command with no marked fields
    When context and documentation are generated
    Then its defaults and examples retain their existing representation
```

Add parameterized unhappy paths for invalid annotations/names, unsupported
profiles, malformed labels from JSON, and hidden/config-only marked fields.
Extend `cargo-orthohelp/tests/golden/agent_context_tests.rs`, its renderer
snapshot suites, and `examples/hello_world/tests/agent_context_e2e.rs` or a
minimal fixture in `tests/fixtures/` to exercise real subprocess output. Human
documentation must omit protected values; it need not add new localized
classification badges. Classifications remain in sanitized IR and context JSON.
Cover JSON, roff, and PowerShell MAML across at least two available locales,
with nested commands. Use `insta` for output shape and explicit semantic
assertions for redaction; inspect snapshot diffs rather than accepting them
blindly. Repeat generation with cache hits and a pre-change cache fixture to
show that stale unredacted data is not reused.

Acceptance is all V1–V5 evidence, successful `make verus` with zero failed
obligations, the mutation controls, all BDD scenarios, and ordinary gates.
Record verifier version, trusted premises, generator bounds, commands, and
logs. Commit only the verified state. A proof failure leaves M2 incomplete;
reduce the failing case or record an architectural deviation, never weaken
premises to assert the desired result. M3 documentation closeout remains.

### M3 — Document and close the feature

Update `docs/users-guide.md` with annotation syntax, both label meanings,
omission rules, safe profile labels, and examples showing that actual loading
still returns original values. Update the component architecture in
`docs/cargo-orthohelp-design.md` and `docs/agent-native-cli-design.md`,
including the projection, model-to-production relationship, wire additions, and
public text limitation. Update `docs/design.md` if its interface map needs the
new shared module. Record test/proof commands, cache invalidation, and
assertion conventions in `docs/developers-guide.md`. Add the ADR and new
documents to `docs/contents.md`; update repository layout for proof files and
pins.

At every milestone review assigned requirements, architecture, assumptions,
public interfaces, dependencies, trust boundaries, wire formats, and trace
links. If a discovery changes the approved contract, set this plan to BLOCKED,
record options and impacts in Decision log, and obtain explicit acceptance. Do
not silently redesign the disclosure boundary.

After all requirements are demonstrated, mark roadmap 9.1.2 and its three
subitems done using checked boxes and an explicit completion note. Do not mark
9.1.3 done. Reconcile the ADR and design documents before setting this plan to
COMPLETE. Run all ordinary gates and `make verus`, commit the documentation
closeout, and publish validation evidence with the feature PR. Recovery is a
normal revert of the focused commit; no persisted profile format changed.

## Concrete validation commands

Run from the repository root. Use the shared Cargo cache, keep build targets
out of `/tmp`, and respect compiler-admission queue messages. Delegate full
gates to `scrutineer`; never overlap formatting, linting, tests, or proof
gates. All commands below require exit status zero for acceptance. Use
`set -o pipefail` so a successful `tee` does not hide a failed command.

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/fmt-ortho-config-9-1-2.out
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-9-1-2.out
make lint 2>&1 | tee /tmp/lint-ortho-config-9-1-2.out
make test 2>&1 | tee /tmp/test-ortho-config-9-1-2.out
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-9-1-2.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-9-1-2.out
make verus 2>&1 | tee /tmp/verus-ortho-config-9-1-2.out
```

Run `make check-fmt`, `make lint`, and `make test` after every major
implementation milestone and before each code commit. The proof command is
mandatory from M2 onward. Review broad formatter changes and preserve unrelated
files. Generated spelling configuration must be updated only through its owning
gate; unrelated dictionary churn belongs on a separate base branch.

Focused commands supplement the Makefile gates during Red-Green-Refactor:

```sh
cargo test -p ortho_config --test docs_ir_profiles
cargo test -p ortho_config --test rstest_bdd
cargo test -p cargo-orthohelp --test rstest_bdd
cargo test -p cargo-orthohelp --test golden_tests
cargo test -p hello_world --test agent_context_e2e
cargo test --workspace --doc
```

Register new directory-style test targets in the owning manifest. Ensure any
new test filter reports a nonzero executed count; a successful zero-test run is
not evidence. Add exact new unit/property target commands and red/green
transcripts to this section as implementation names settle. Before code edits,
run the affected existing suites; after edits, run focused tests and full gates.

For this plan-only delivery, run `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie` sequentially. No executable code or proof
is introduced. Code gates and proof discharge are future milestone acceptance,
not claims made by publishing this draft.

## Constraints

Explicit user approval is required before implementation. Preserve ADR-012's
five-tier precedence, stateless selection, source provenance, and opt-in
loading. Keep dependency direction from macros to generated runtime APIs and
from the generator to the library schema. No new runtime dependency, profile
store, secret resolution, or ambient discovery is authorized. Existing and
user-authorized test dependencies may be used with caret requirements.

Never log or serialize protected values on newly introduced paths. Do not claim
that grammar validation makes arbitrary names semantically non-secret. Do not
silence lint failures or alter Whitaker, compiler admission, shared services,
or other agents' processes. Do not add proof shortcuts. Keep docs within the
project spelling/wrapping conventions and Rust source files below 400 lines.

## Tolerances (exception triggers)

Stop for approval if this work needs loader/merge semantic changes, store
behaviour, raw-value output, new runtime dependencies, weakening of the stated
redaction guarantee, wire-version replacement, or a different public annotation
or names contract. Mechanical private helper names and test organization may
change with recorded rationale. If three attempted fixes fail to resolve the
same substantive design or proof issue, record the evidence and options before
proceeding. There is no elapsed-time limit; compiler queues are not failures.
Report disk exhaustion or admission refusal and preserve the current work.

## Risks

High severity, medium likelihood: a secondary serializer or cached bridge
retains raw defaults. Mitigate with early projection, cache invalidation,
direct-entrypoint tests, and full artefact sentinel inspection.

High severity, medium likelihood: flattened containers or recursive enum types
lose classification. Mitigate with explicit path identity, container
diagnostics, recursive model coverage, and nested property/mutation cases.

Medium severity, high likelihood: suppressing all command examples surprises
consumers. Document it prominently and snapshot both protected and ordinary
outputs; do not substitute an unreliable text scrubber.

High severity, medium likelihood: a correct Verus mirror diverges from runtime
projection. Maintain the mapping, differential tests, and explicit residual
gap; re-review every slot added to either representation.

Medium severity, medium likelihood: new proof tooling is unavailable or cannot
use managed admission. Verify runner capability before proof implementation;
record a blocker instead of reporting a skip as success.

## Progress

- [x] (2026-10-14) Inspect prerequisite, branch, and projection paths with two
  Wyvern reconnaissance agents; exchange source evidence through context packs.
- [x] (2026-10-14) Draft behavioural contracts, proof sketches, milestones, and
  test strategy without implementing the feature.
- [x] (2026-10-14) Complete six-lens community-of-experts review and revise
  the proof relation, schema ownership, names invariants, and flattening scope.
- [x] (2026-10-14) Validate the plan with formatting, Markdown/spelling, and
  Mermaid gates. Publication receipts belong in the draft PR.
- [ ] Obtain explicit user approval before starting M1.
- [ ] M1: complete disclosure boundary.
- [ ] M2: proofs and external workflow evidence.
- [ ] M3: documentation, final gates, and roadmap completion.

## Surprises & discoveries

The current context contains selector metadata, not profile bodies or names.
The concrete exposure paths are defaults, enum members, and examples. Therefore
this work should sanitize metadata and accept explicit names, not introduce a
profile reader. Required reference filenames include one typo and one absent
file, recorded above. No existing Verus setup was found. Assertion libraries
are already authorized and present in workspace testing dependencies.

## Decision log

2026-10-14, proposed for approval: classify both secret and reference-only
values as non-public; retain only public descriptors and distinct labels. This
avoids assuming locators are safe or resolving downstream secrets.

2026-10-14, proposed for approval: omit protected slots and conservatively
remove command examples across the document tree. Arbitrary strings cannot
safely be mapped back to fields by substring replacement.

2026-10-14, proposed for approval: keep profile discovery out of context
building; accept validated public labels explicitly. Names cannot be inferred
from secret bodies, and no store implementation is needed.

2026-10-14, proposed for approval: use Verus for recursive noninterference and
projection integrity, conventional tests for adapters, names, and diagnostics.
The reconnaissance recommendation against proving a simple selector is
retained: the proof is justified only by the whole recursive projection, not by
the enum.

## Outcomes & retrospective

Planning only. Implementation is unapproved and no feature acceptance or proof
completion is claimed. The expert review corrected a counterexample in the
draft proof relation and removed an unsupported flattened-descendant
assumption. The plan-only formatting, format-check, Markdown/spelling, and
Mermaid gates passed on 2026-10-14. No unrelated formatter or spelling changes
remain. Publication records the final commit and branch in the draft PR;
implementation approval remains outstanding.

## Research and review evidence

Firecrawl research on 2026-10-14 confirmed
[GoogleTest Rust interoperability](https://github.com/google/googletest-rust)
with rstest and the [Verus project](https://github.com/verus-lang/verus)
restriction to a subset of Rust, supporting an explicit model/runtime boundary.
[Kubernetes config view](https://kubernetes.io/docs/reference/kubectl/generated/kubectl_config/kubectl_config_view/)
documents a sensitive-data raw mode; this proposal deliberately offers no such
bypass in metadata output. These are prior art and tooling references, not
additional OrthoConfig requirements.

Wyvern source evidence packs: `pk_fthbde2u` (contracts and profile names) and
`pk_5gxybilp` (documentation paths and tests). They are transient review aids;
the durable context, decisions, and commands are included in this plan.

Revision note (2026-10-14): initial draft records the current source baseline,
proposed contracts, proof sketches, and approval-gated implementation sequence.

Review revision (2026-10-14): Pandalump and Wafflecat identified the schema
mirror and unsupported flatten expansion; consolidate the schema owner and
reject marked flattened containers. Telefono corrected low-equivalence and
required private new metadata plus serialization validation. Doggylump required
unsupported-payload checks. Buzzy Bee required linear traversal and bounded
generator size. Dinolump clarified that classification labels are required in
context and sanitized IR, not human-renderer badges. All findings are addressed
in the proposed design; implementation remains subject to approval.

Validation revision (2026-10-14): record passing documentation gates and
separate publication receipts from implementation approval. No executable
feature or proof has been added.
