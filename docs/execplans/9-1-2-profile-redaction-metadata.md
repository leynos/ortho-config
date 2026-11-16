# Add profile redaction metadata (9.1.2)

Status: APPROVED — implementation in progress.

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

The inspected baseline after the 2026-11-13 rebase is `origin/main` commit
`37452245bf2af9254011dd5eee103866cfff5edc`. It includes
`ddba3578fae04227e426ea3a61aa2b24aa043232`, the merge of
[PR #651](https://github.com/leynos/ortho-config/pull/651), which imports Kani
and Verus tooling, and `eaf4c028440b96eb84469a3bcddf2d9c4be4b371`, the merge of
[PR #664](https://github.com/leynos/ortho-config/pull/664),
which supplies nested Cargo test-harness fixes. The original draft used
`dc979878b5728bbb23866df9c05c2570bdb5629e`; neither its validation nor the
intermediate temporary-parent runs establish acceptance of this candidate. The
governing requirements are [roadmap](../roadmap.md) 9.1.2 and
[agent-native CLI design](../agent-native-cli-design.md) §6.7. There is no
separate terms-of-reference document for this task. Prerequisite 9.1.1 is
checked complete and implemented;
[ADR-0012](../adrs/adr-0012-profile-selection-and-layering.md) controls
selection, precedence, and static-versus-runtime metadata.
[ADR-0017](../adrs/adr-0017-consumer-side-prover-tooling.md), the
[prover import plan](import-prover-tools.md), and the developers' guide's
production proof inventory govern the existing verifier boundary. R2's M2 must
add feature-owned proofs to that boundary, not recreate the tooling.

### Merged prerequisites and implementation hand-off

The PR targets `main`. The temporary parent `issue-644-nested-cargo-timeouts`
has merged as #664, and the return to `main` is complete. The rebase replayed
only the profile-plan commit after the preserved parent boundary
`8309e3b41287f2fb9b70016194b8bfe2235db4fd`; none of the seven inherited parent
commits was replayed after the squash merge.

Preserve the merged nested Cargo serialization, bounded process-tree cleanup,
retained timeout diagnostics, shared scenario allowances, and leased reusable
identifier-fixture target. These are prerequisite test-harness fixes, not
profile-redaction implementation. Follow the developers' guide's current
Cargo-backed test conventions and load-sensitive deadline guidance. PR #681
explicitly documents that host load and cold compilation can still exhaust
those deadlines; do not interpret a timeout as proof of a semantic defect or as
a passing gate. Earlier failed or interrupted runs remain historical.

Future rebases must re-establish the exclusive child replay boundary using the
`rebase` skill, preserve recovery refs, inspect conflicts and range diffs, and
rerun the required gates. Publication of this plan, including the authorized
publication of failed validation results, does not grant feature implementation
approval.

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
BDD guide. The [scripting standards](../scripting-standards.md) now exist; use
them alongside the
[developers' guide](../developers-guide.md), [repository layout](../repository-layout.md),
and `AGENTS.md` for scripts. Follow the current
[BDD migration guide](../rstest-bdd-v0-6-0-migration-guide.md) and
[documentation provenance](../rstest-bdd-doc-provenance.md) when adapting
behavioural fixtures. Do not copy historical step signatures uncritically.

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

`ortho_config/src/docs/ir/mod.rs` defines the documentation intermediate
representation (IR), including `DocProfilesMeta` and `DocMetadata`.
`ortho_config/src/docs/ir/field.rs` owns `FieldMetadata`, `CliMetadata`, and
`ValueType`; sibling `behaviour`, `content`, and `renderer` modules hold the
remaining contracts. The macro builds these in
`ortho_config_macros/src/derive/generate/docs/fields/mod.rs` and
`ortho_config_macros/src/derive/generate/docs/mod.rs`.
`ortho_config/src/docs/mod.rs` owns exports and IR version `2.0`.

`cargo-orthohelp/src/schema/mod.rs` currently mirrors the library schema as
separate Rust types; it does not re-export the library types. Its private
`field`, `behaviour`, `content`, and `renderer` modules mirror the runtime
layout. Preserve the upstream renderer declarations and optional-value CLI
metadata when consolidating these types or projecting them. Renderer
declarations are public descriptions with application-defined string
identifiers; they are outside profile-value redaction and must remain unchanged.
`cargo-orthohelp/src/agent_context/mod.rs::bridge_ir_to_agent_context` builds
context; `build_input` in `cargo-orthohelp/src/agent_context/input.rs`
currently copies defaults and enumerated values.
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
module paths. ADR-0003 assigns ownership to that library. Consolidating the
mirror types is a new decision proposed for 9.1.2, grounded in that ownership.
Record this decision and the pre-1.0 type-identity change in the new ADR and
developers' guide, and update schema tests to test actual wire defaulting and
round trips rather than comparing a type with itself. This removes schema drift
and permits one projection without JSON conversion, fallible schema-conversion
APIs, or a generic visitor framework. This ownership consolidation is part of
M1, not a temporary compatibility layer.

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
  slots, upstream renderer declarations, optional-value CLI metadata, and
  command topology; clears exactly the specified protected slots; and P(P(d))
  equals P(d).
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
its number from the live `docs/adrs/` inventory. Reference it from
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
the documented production mapping. Reuse PR #651's existing checksum-pinned
`tools/verus/VERSION` and `tools/verus/SHA256SUMS`, shared runner pin, and
`make verus` recipe. The current Verus pin is `0.2026.01.30.44ebdee`; the
shared package is pinned to `592558f7183b554fc749696246984c5d7300d029` and runs
with Python 3.14 through `uv`. The repository script baseline remains Python
3.13. Do not introduce a second installer, runner, solver pin, or replacement
Make target.

Register `verus/profile_redaction.rs` in the existing `VERUS_PROOF_FILES`
Makefile inventory. Before registration, an explicit focused invocation is:

```sh
make verus VERUS_FLAGS='--proof-file verus/profile_redaction.rs'
```

Once registered, `make verus` without overrides must verify the production
inventory. A successful fixture-only run is not feature acceptance. Record the
owning feature (9.1.2), crate (`ortho_config`), production source paths, proof
file and exact symbols, symbolic domain, all preconditions and model
abstractions, expected nonzero verification count, and negative controls in the
feature PR and developers' guide. Refine these names as M1 settles the kernel.
An empty inventory produces `FORMAL-EMPTY` and fails; neither that result nor a
skipped integration test discharges V1 or V2.

Use `make install-verus` as the explicit provisioning step when the pinned
bundle or its embedded Rust toolchain is absent. It installs under `.verus/`
and reads the required toolchain from the release's `version.json`; do not
hardcode another toolchain pin. The existing Make boundary supplies the bundled
Z3 through `VERUS_Z3_PATH`. Do not install the shared package's standalone Z3:
its version is incompatible with this Verus release. Execution must retain both
`--no-install-missing` and `--no-ensure-toolchain`; missing tools fail rather
than being provisioned by a proof run. Pin upgrades require separate evidence
and are not part of this feature.

PR #651's `tests/fixtures/prover_tools/` and opt-in
`make test-prover-tools-integration` establish runner acceptance and rejection,
not configuration correctness. That target needs both tools installed via
`make install-kani` and `make install-verus`; it emits fixture evidence under
`target/prover-tools-fixtures/`. Do not register these fixtures as production
proofs, add a Kani harness solely because Kani is available, or add prover
installation/execution to `make all`, `make test`, or `make lint`.

Extend the existing `.github/workflows/prover-tools.yml` lane in M2 to run
registered production Verus proofs after installation and retain their separate
transcript with the uploaded evidence. Add `verus/**` and the actual production
kernel/model-mapping paths to its pull-request path filters. Update
`tests/workflow_contracts/prover_tools_workflow_test.py` to check those paths,
the production invocation, and upload coverage. Preserve the fixture checks,
read-only permissions, pinned actions, 30-minute bound, and Linux x86_64 scope.
Other platforms remain explicitly unvalidated. The existing hosted sccache
opt-out for Kani is not permission to unset local compiler admission wrappers.

Use the managed compiler admission path locally. If admission is unhealthy,
report its full status and continue non-compiling work rather than bypassing
wrappers. `make verus` must fail on failed or missing verification. Keep it a
separate milestone gate, outside the ordinary Rust gate sequence. Run
`make test-workflow-contracts` when adding the inventory and CI coverage;
contract tests prove command wiring, while the real production proof run must
independently discharge V1 and V2.

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
documents to `docs/contents.md`; update repository layout for the production
proof files. Existing tooling pins and their ownership already have
documentation from PR #651.

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

For this rebased plan-only delivery, run `make fmt`, `make check-fmt`,
`make test`, `make typecheck`, `make lint`, `make markdownlint`, and
`make nixie` sequentially. Record results for the exact candidate in the PR. No
executable feature or proof is introduced; passing existing gates does not
discharge the future profile-redaction proof obligations.

## Constraints

Explicit user approval is required before implementation. Preserve ADR-0012's
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

Medium severity, medium likelihood: the pinned tooling or required toolchain is
not installed in this checkout, or local admission is unhealthy. Use the
existing explicit provisioning boundary and report admission failures. PR #651
provides runner evidence, not installed-state or feature-proof evidence for a
new worktree. Never report a missing-input failure or fixture pass as proof of
redaction.

Medium severity, realized on 2026-11-15: the rolling Whitaker dylint suite
changed underneath the branch. Its shared object was replaced at 21:24 and
`make lint-whitaker` went from two findings to 22
`multiple_nested_conditional_logic` diagnostics, 21 of which sit in files
byte-identical to `main` (verified with `git diff --quiet main -- <path>`, 12
paths SAME-AS-MAIN; the one changed path carries a finding that predates this
branch's arms). Mitigation: certify M1 with `lint-clippy` and record the
byte-identity evidence, leave `lint-whitaker` excluded, and escalate to the
operator. Never modify the installation or `dylint.toml` to buy a green gate.

## Progress

- [x] (2026-10-14) Inspect prerequisite, branch, and projection paths with two
  Wyvern reconnaissance agents; exchange source evidence through context packs.
- [x] (2026-10-14) Draft behavioural contracts, proof sketches, milestones, and
  test strategy without implementing the feature.
- [x] (2026-10-14) Complete six-lens community-of-experts review and revise
  the proof relation, schema ownership, names invariants, and flattening scope.
- [x] (2026-10-14) Validate the plan with formatting, Markdown/spelling, and
  Mermaid gates. Publication receipts belong in the draft PR.
- [x] (2026-11-08) Rebase the plan onto PR #651's merge without conflicts;
  revise M2 to reuse the existing pinned tooling, production inventory, and CI.
- [x] (2026-11-09) Rebase without conflicts onto #664 and record the temporary
  PR target and the post-merge return to `main`.
- [x] (2026-11-13) Return to `main` after #664 merged, replaying only the
  plan commit and preserving the #651 tooling integration.
- [x] (2026-11-15) Obtain explicit user approval before starting M1. The
  implementation request named this plan and instructed implementation to
  proceed, which supplies execution approval for M1–M3.
- [ ] M1: complete disclosure boundary.
  - [x] (2026-11-15) Consolidate schema ownership: `cargo-orthohelp::schema`
    re-exports `ortho_config::docs`; the four mirror modules are deleted.
  - [x] (2026-11-15) Add `ProfileFieldExposure`, the `profile_field` IR field,
    the single `redacted_for_output` projection, and its unit tests.
  - [x] (2026-11-15) Parse and emit `profile_field = "secret"|"reference_only"`,
    rejecting unknown and duplicate labels and marked flattened containers.
  - [x] (2026-11-15) Extend `ProfilesDeclaration` with validated private
    redaction records and a names-only catalogue; reject duplicates,
    unsupported metadata, and inconsistent serialization.
  - [x] (2026-11-15) Apply projection at the derive, pipeline, generated bridge,
    and agent-context boundary; derive `ProfileFieldRedaction` records and add
    the `profile-redaction-v1` cache salt.
  - [x] (2026-11-15) Add bounded differential property tests for V1, V2, and V4
    and a names-only property test for V3; all 19 redaction tests and all 14
    declaration tests pass.
  - [x] (2026-11-15) Record non-vacuity evidence by seeding three faults
    (retained default, retained nested enum member, skipped child), observing
    failures, restoring the module byte-for-byte, and rerunning green.
  - [x] (2026-11-15) Record the contract in ADR-0018 and reference it from the
    agent-native and cargo-orthohelp design documents.
  - [x] (2026-11-16) Ordinary gates and gated M1 commit: formatting, Clippy,
    docstring coverage, Markdown, Mermaid, spelling, and the workspace test
    suite except the load-sensitive boundary target, whose saturation
    attribution is recorded below. Whitaker remains escalated, not repaired.
- [ ] M2: proofs and external workflow evidence.
  - [x] (2026-11-16) Model and prove V1/V2 in `verus/profile_redaction.rs` and
    its three modules, split to stay under the 400-line cap:
    `profile_redaction_model` (types, projection, value lemmas),
    `profile_redaction_integrity` (V2), and
    `profile_redaction_noninterference` (V1). `make verus` reports
    `27 verified, 0 errors`.
  - [x] (2026-11-16) Register the proof in `VERUS_PROOF_FILES`, so a bare
    `make verus` verifies the production inventory.
  - [x] (2026-11-16) Record negative controls: a retained protected default, a
    retained nested enum member, and a skipped child each fail the corresponding
    Verus obligation. The modules were then restored byte-for-byte (checked with
    `sha256sum -c`) and rerun green.
  - [ ] Recreate the production-proof CI lane and its workflow contract test.
  - [ ] Add the `profile_redaction.feature` behavioural contract and its
    unhappy paths.
  - [ ] Extend the golden, renderer, and end-to-end sentinel suites, including
    cache-hit and pre-change-cache cases.
- [ ] M3: documentation, final gates, and roadmap completion.

## Surprises & discoveries

The current context contains selector metadata, not profile bodies or names.
The concrete exposure paths are defaults, enum members, and examples. Therefore
this work should sanitize metadata and accept explicit names, not introduce a
profile reader. The original draft found a misspelled reference and an absent
scripting guide; the guide now exists. PR #651 has since supplied the pinned
Kani/Verus setup, command contracts, fixtures, and a dedicated CI lane.
Production proof inventories remain empty at the rebased baseline. Assertion
libraries are already authorized and present in workspace testing dependencies.

M1 discoveries, 2026-11-15:

- The production-proof CI lane named by this plan no longer exists: `#678`
  deleted `prover-tools.yml`. M2 must create a production-proof lane rather
  than extend one. The pinned Verus tooling from ADR-0017 remains available.
- `cargo-orthohelp/src/schema/` was a hand-maintained mirror of the IR types,
  as ADR-0016 recorded. Consolidating it into re-exports was required to give
  the projection one type identity; the mirror's drift risk disappears.

M2 discoveries, 2026-11-16:

- Verus 0.2026.01.30.44ebdee cannot verify structural recursion through a
  sequence of child nodes. A recursive call inside a `Seq::new` closure never
  discharges its termination check, a mutual pair of spec functions is rejected
  with an ill-typed AIR panic, and an explicit `decreases` measure computed
  from a child sequence crashes the verifier the same way. The model therefore
  encodes the command tree as a preorder sequence whose `parent` index carries
  the edge, and `is_tree_like` asserts a single root plus every parent strictly
  earlier than its child. Depth stays unbounded and the parent index is what
  the topology obligation quantifies over; the recursive traversal this
  replaces remains covered by the differential property tests.
- The first focused run of the new property suite exposed three existing unit
  tests: two encoded wrong expectations (changing `ValueType::String` to
  `ValueType::Enum` is a public shape change, not a protected-slot change; an
  absent `default` serializes as an explicit `null` because the field has no
  `skip_serializing_if`), and the traversal probe used a process-global counter
  that concurrent property tests also incremented. The fixtures were corrected
  and the probe made thread-local.
- Non-vacuity evidence (logs under `/tmp/fault{1,2,3}-*-9-1-2.out`):
  - retained protected default: 9 tests failed, including
    `projection_matches_independent_model`,
    `projection_preserves_and_clears_exactly`, and
    `projected_wire_carries_no_protected_literal`;
  - retained nested enum member: 7 tests failed, including all four proptests;
  - skipped child projection: 7 tests failed, including the traversal-count and
    command-example tests.
  The module was restored from a byte-identical copy (`sha256 33cc3bf3…`) and
  all 19 redaction tests passed again.

M1 gate evidence, 2026-11-15 (candidate `7a4586f7`, worktree clean of edits
during each run):

- `make check-fmt`: pass, 100 files unchanged.
- `make lint-clippy`: pass; `cargo doc --workspace --no-deps` clean, then
  `cargo clippy --workspace --all-targets --all-features -D warnings` clean.
- `make docstring-coverage`: pass, 746 of 746 statements (100.0%).
- `make markdownlint`: pass once the index reflected the four deleted mirror
  modules and one hyphenated spelling was corrected to its closed form.
- `make nixie`: pass, all diagrams validated.
- `make spelling`: pass.
- `make lint-whitaker`: excluded, see the Whitaker risk above. This is
  infrastructure, not a verdict on the diff.
- `make test`, `cargo test --all-targets --all-features` half: 111 test
  binaries ran and 110 passed, including every binary that exercises this branch
  (`ortho_config` lib 332 passed / 4 ignored, `ortho_config_macros` 325 passed,
  `documentation_examples_rust_tests` 58 passed / 13 ignored, `compile_fail` 2
  passed, `docs_profile_redaction` 5 passed). The single red binary is
  `-p ortho_l10n --test boundary`: 35 passed, 9 failed, 10 ignored, 900.32 s.
  No file this branch touches belongs to that target (`git status --porcelain`
  matches no `ortho_l10n` path, and no manifest changed).
- `make test`, pytest half: `128 passed, 5 skipped` when run directly with the
  pinned `uv` environment. `make` never reached this step because the red
  binary above aborted the recipe.
- `-p ortho_l10n --test boundary`: attributed to host saturation, not to the
  diff. All nine failures carry one of the two documented shapes ("exceeded the
  N second deadline" and "900 second allowance was exhausted while … waited for
  a sibling nested Cargo invocation"), and the log shows
  `Blocking waiting for file lock on package cache` plus a cold
  `target/localization-boundary/packaging-target` compile. At the time of the
  run the host was at load average 17-23 with 4 of 4 compile slots held and
  more than 30 requests queued.
  [The boundary allowance finding](../BOUNDARY-ALLOWANCE-FINDING.md) records
  the same failing set, with the same failure shapes, on an unmodified
  `origin/main` control at `9be62487`, and states that validating a pass
  requires an idle host. The suite was not re-run under the same saturation,
  because that would add load without changing the outcome; the dedicated CI
  runner is the arbiter. Log retained at
  `/tmp/gate-evidence-9-1-2/test.attempt5-boundary-deadlines.out`.

Two real defects were found by these gates and repaired in the same change:

- `ortho_config/tests/trybuild/public_api_contracts.rs` is the "should pass"
  compile fixture that proves downstream crates can build a `FieldMetadata`
  literal. Adding the `profile_field` field without updating it made
  `public_api_contracts_compile_for_downstream_crates` fail with
  `E0063: missing field`. This is the fixture doing its job.
- The documented Rust probe in
  `ortho_config/tests/documentation_examples/guide_flows.rs` serialized the
  bridge result directly. Once `bridge_ir_to_agent_context` became fallible,
  that fence failed to compile with `E0277: Arc<OrthoError>: Serialize`. The
  published example now unwraps with an `.expect(...)` naming the step.

Two gate incidents were diagnosed rather than worked around:

- The spelling gate reported `tracked file could not be scanned` for a deleted
  mirror module. The deletions were unstaged, so `git ls-files` still listed
  them. Staging the intended changes fixed the gate's index view; no
  configuration was altered.
- A killed `make test` attempt left `target/orthohelp-bridge-build.in-progress`
  naming a dead pid. Three `agent_context_contract` cases then failed with
  "requires owner recovery after an interrupted build". After confirming no
  Cargo or rustc process used this worktree's `target/`, the marker was removed
  and the suite re-run; the same binary had passed 21 of 21 in the killed
  attempt. The failure log is preserved at
  `/tmp/gate-evidence-9-1-2/test.attempt2-stale-marker.out`.

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

2026-11-15, implemented: keep one projection in `ortho_config::docs` and make
`cargo-orthohelp::schema` re-export it. This supersedes the hand-maintained
mirror noted as a residual risk in ADR-0016 and gives the bridge and renderers
one type identity. Recorded in ADR-0018.

2026-11-15, implemented: decide command-example suppression once per document
before projecting, then run a single projection pass. This keeps the global
rule honest and bounds the projection to two linear traversals.

2026-11-15, implemented: reject `profile_field` on clap- or serde-flattened
containers. The emitted leaf fields have their own names, so a container label
cannot be mapped to them; authors mark the leaves instead.

2026-11-15, decided for M2: recreate a production-proof CI lane because `#678`
deleted `prover-tools.yml`. Reuse the pinned Verus invocation and command
contracts from ADR-0017 rather than inventing new tooling.

2026-11-16, decided for M2: encode the command tree as a preorder sequence with
a parent index instead of nested children. Verus rejects structural recursion
through a sequence of nodes, so the nested encoding cannot be proved there at
all, and the plan allowed the lemma boundaries to be revised while verification
conditions were discovered. The encoding is total for topology and for slot
reachability, so V1 and V2 keep their full strength; only the traversal shape
is modelled differently, and the correspondence to the production recursion
stays with the differential tests recorded in ADR-0018.

## Outcomes & retrospective

M1 implemented on 2026-11-15 under the user's execution approval. No formal
proof, external workflow evidence, or roadmap completion is claimed. The expert
review corrected a counterexample in the draft proof relation and removed an
unsupported flattened-descendant assumption. The plan-only formatting,
format-check, Markdown/spelling, and Mermaid gates passed on 2026-10-14.

M2 proof status, 2026-11-16: V1 and V2 are discharged. The proof is split into
a crate root plus three modules under the 400-line cap and verifies 27
obligations with no `assume`, `admit`, or trusted proof body. Three seeded
model faults each break a named obligation, so the obligations are not vacuous.
The external workflow lane, the behavioural contract, and the sentinel suites
remain outstanding, so M2 is not complete.

M1 retrospectively: the largest surprises were the deleted proof lane and the
hand-maintained schema mirror, both addressed above. The differential model and
the bounded generator found the three fixture/probe defects before any gate
ran, so the milestone's property suite was useful immediately rather than only
as regression cover. The remaining M1 work is the ordinary gate run and the
gated commit; M2 and M3 remain outstanding.

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

Rebase revision (2026-11-08): incorporate merged PR #651 and ADR-0017. Reuse
the installed-tool command contract, bundled solver, production inventory, and
existing CI lane; keep fixture acceptance distinct from V1/V2 proof evidence.
Refresh moved ADR and IR paths and the scripting/BDD references. All feature
implementation milestones remain unapproved; this maintenance changes the plan
only. The requested post-rebase formatting, test, typecheck, and lint results
belong to the new candidate and are recorded in the PR delivery receipt, not
inferred from the original draft's checks.

Parent revision (2026-11-09): use #664 as the temporary base to incorporate
nested Cargo timeout fixes. Preserve the #651 tooling decisions, record the
exclusive parent boundary, and require fresh validation before publication.
After #664 merges, the implementing agent may restack onto `main` and update
the PR target; profile implementation still requires explicit approval.

Validation interruption (2026-11-10): on the temporary parent, `make fmt` and
`make check-fmt` passed, but `make test` failed in
`a_second_process_cannot_clean_a_leased_artefact`. The cold identifier fixture
build exhausted its remaining 878607 ms allowance while compiler admission was
busy. The following warm-build artefact test passed; this does not replace the
failed full-suite gate. Subsequent lint and typecheck runs passed with explicit
zero exit receipts. A full-suite follow-up against the populated fixture target
passed all eight identifier-artefact tests (one ignored), including the lease
case. It then failed nine localization-boundary tests: six reported a shared
package-build timeout and three reported consumer-test timeouts, each at the
440-second nested Cargo deadline. That boundary suite finished with 29 passed,
nine failed, and seven ignored; Make exited with status 2. Its output records
compiler-admission waits and uncached compilation. These results do not
establish a passing full-suite gate.

Publication revision (2026-11-13): the user explicitly authorized committing
and publishing the plan even if validation remains red. Rebase onto the PR's
current `main` target and record fresh gate results in the PR without claiming
that historical timeout failures passed. This exception applies to delivering
the plan, not to implementing the feature or marking roadmap 9.1.2 done.
