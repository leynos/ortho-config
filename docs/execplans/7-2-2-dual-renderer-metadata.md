# Add dual-renderer metadata (7.2.2)

Status: APPROVED (2026-10-28) — implementation authorized.

This execution plan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current at each milestone boundary.

## Purpose / big picture

Consumers should be able to discover separately whether a command supports
human and machine rendering, what presentation behaviour it declares, and which
output fields may be localized. A derive declaration must survive the generated
documentation intermediate representation (IR), the cargo-orthohelp bridge, and
the compact agent-context JSON without acquiring claims that the author never
made. Protocol identifiers must remain stable when human language changes.

Success is observable by generating agent context for a fixture with different
human and machine policies, including a nested command with no declaration. The
JSON must retain each declared policy, apply documented legacy defaults to the
nested command, and preserve protocol fields across two localizers. The feature
describes downstream rendering; it does not implement terminal output.

This plan, including the deliberate pre-1.0 Rust source break described below,
was explicitly approved on 2026-10-28; implementation may proceed. Draft PR
publication, passing checks, and expert review did not by themselves constitute
that approval.

## Constraints

- Preserve the schema ownership split in ADR-003: documentation IR belongs to
  `ortho_config::docs`, agent context to `ortho_config::agent_context`, and the
  conversion belongs to `cargo-orthohelp`. Neither library schema imports the
  other. Preserve the tool's existing hand-maintained IR mirror.
- Add a sibling `renderer(...)` attribute. ADR-010 confines `behaviour(...)`
  to execution semantics. Do not infer rendering from a command name, a
  `--json` flag, an output format name, a terminal probe, or the host locale.
- Keep existing wire fields, enum spellings, default meanings, ordering, and
  omission-versus-null rules. Agent-context version `1` and docs IR `2.0` can
  accommodate the proposed optional additions; verify this with old fixtures.
- Preserve existing man-page and PowerShell artefact paths and meanings,
  command exit behaviour, policy report schema, and stdout selection rules.
- Exclude JSON-mode enforcement, exit classes, pagination, and new policy lint
  rules: these belong to later 7.2 tasks. Machine support does not assert JSON
  support or a one-document stream guarantee.
- No rendering engine, terminal/environment probing, process execution,
  dependency cache isolation, compiler admission bypass, or Whitaker changes.
- Keep changed code files within 400 lines. Put new cohesive types and tests in
  focused modules; do not introduce compatibility aliases or parallel APIs.
- Run gates sequentially, log with `tee` and `pipefail`, and commit only green
  milestones. Keep roadmap 7.2.2 unchecked until the feature is complete.

## Tolerances (exception triggers)

Changes may span the three schema/derive/adapter areas, their public exports,
construction sites, fixtures, tests, and the documentation named here.
Mechanical updates of struct literals are included. Changes to configuration
loading, localization resolution, policy enforcement, or unrelated rendering
algorithms are outside scope. Stop and propose a revised plan if they become
necessary.

The proposed new metadata fields and types are the only approved source API
changes once this plan is approved. Do not implement roadmap 13.1 incidentally.
If release policy does not permit the source break, mark this plan BLOCKED and
propose sequencing 13.1 first. A wire-version bump, change to existing
defaults, new runtime dependency, or substantive validation algorithm also
requires an approved deviation. Existing test dependencies and the explicitly
authorized `googletest` and `pretty_assertions` are permitted with caret
requirements.

After three unsuccessful repairs of the same gate failure, record the evidence
and reassess the cause before further edits. Infrastructure failures are not
source failures. There is no time limit; queued compiler work is not a hang.

## Risks

- High impact, certain without migration: public structs use exhaustive struct
  literals. Adding a field breaks downstream Rust source construction. Update
  all workspace callers atomically and publish an explicit migration note for
  the next breaking-compatible pre-1.0 release; do not call this source-safe.
- High impact, medium likelihood: documentation and the tool's mirrored schema
  diverge. Test serialization through both real schema owners and the bridge.
- High impact, medium likelihood: a machine-support declaration is mistaken for
  JSON compliance. Keep stream content descriptive and prohibit inferred
  `output_modes`, JSON support, or exit-code promises.
- Medium impact, medium likelihood: unspecified values become reassuring false
  claims. Preserve absence in IR and use explicit documented defaults only.
- Medium impact, medium likelihood: localization changes a protocol key. Keep
  field identifiers and enum tokens outside localization and compare protocol
  projections under distinct injected localizers.
- Medium impact, low likelihood: shared build infrastructure blocks gates.
  Record exact errors and logs; do not change shared services or credentials.

## Progress

- [x] (2026-10-14) Read repository instructions and requested planning skills;
  renamed the local branch to `7-2-2-dual-renderer-metadata`.
- [x] (2026-10-14) Wyvern reconnaissance verified prerequisite 7.2.1, schema
  ownership, extension points, source compatibility, and nearby test seams.
- [x] (2026-10-14) Firecrawl checked colour-suppression and assertion-library
      prior art.
- [x] (2026-10-14) Community-of-experts review covered all six perspectives;
  revised inheritance, width encoding, nested defaults, and gate fail-fast
  behaviour in response.
- [x] (2026-10-14) Documentation gates passed: formatting, Markdown/spelling,
  and diagram validation; no unrelated tracked changes resulted.
- [ ] Commit and publish this plan as a draft PR for approval.
- [x] (2026-10-28) Obtain explicit approval, including the source compatibility
  decision.
- [ ] M1: deliver metadata types, derive, bridge, and contract tests together.
- [ ] M2: finish behavioural/end-to-end coverage, documentation, and closure.

## Surprises & discoveries

The design's renderer default rows are still labelled planned. There is no
current renderer field in `AgentCommand` or `DocMetadata`; `output_modes` is
currently emitted as an empty vector. Introducing the field realizes those
planned defaults rather than changing an existing renderer default.

The tool owns a separate `cargo-orthohelp/src/schema/mod.rs` IR mirror.
Updating only the reusable library would silently lose metadata at this
boundary.

Roadmap 13.1 and agent-native design §9 explicitly identify struct-literal
construction as an extensibility gap. JSON compatibility is not Rust source
compatibility. The next release must acknowledge the new source requirement.

## Decision log

- 2026-10-14, proposed: use a sibling renderer group and independent per-command
  declarations. This follows ADR-010 and avoids inheriting a parent's promises
  into commands with different output behaviour.
- 2026-10-14, proposed for approval: accept a deliberate pre-1.0 source break
  for the added fields, update callers atomically, and document migration.
  Deferring to roadmap 13.1 is the alternative if the release cannot accept it.
  No compatibility wrapper is justified for this pre-1.0 surface.
- 2026-10-14, proposed: keep this a descriptive metadata feature. Finite mapping
  tests and generated-input tests provide appropriate evidence. No Verus
  milestone is warranted for mere field transport; no speculative proof source
  will be added. See the proof selection decision in Verification plan.
- 2026-10-14, research: the colour-suppression environment variable disables
  colour when non-empty, not other styling. It cannot stand in for plain
  output, emoji, or accessibility policy.

## Outcomes & retrospective

The six-perspective expert review retained the ownership and descriptive scope.
It exposed ambiguous cross-renderer inheritance and width encoding, which are
now explicit per-renderer decisions and tagged width objects. Nested reader
defaults and strict IR versus tolerant agent decoding are specified. No runtime
code, schema, macro, tests, or roadmap completion state has changed.
Documentation validation passed on 2026-10-14: `make fmt`, `make check-fmt`,
`make markdownlint` (including spelling), and `make nixie`, run sequentially by
scrutineer. Only this plan and its index entry changed. Code lint/tests are
implementation-milestone requirements, not claimed planning evidence.
Publication follows the gated plan commit; implementation awaits approval.

## Context and orientation

The prerequisite 7.2.1 is checked in `docs/roadmap.md`. Its `behaviour(...)`
pattern supplies the nearest implementation precedent. Struct attributes enter
`ortho_config_macros/src/derive/parse/doc_attrs.rs` and `doc_types.rs`;
`behaviour_attrs.rs` demonstrates strict nested-key parsing. Generated metadata
is assembled in `ortho_config_macros/src/derive/generate/docs/sections.rs` with
helpers such as `generate/docs/behaviour.rs`.

The reusable documentation model is `ortho_config/src/docs/ir.rs`, exported by
`ortho_config/src/docs/mod.rs`. The equivalent tool model is
`cargo-orthohelp/src/schema/mod.rs`. The adapter's
`cargo-orthohelp/src/agent_context/mod.rs::walk` recursively constructs one
`AgentCommand` per metadata node. It resolves a cosmetic summary through
`resolve_summary`; protocol fields use their original values. The wire types
live in `ortho_config/src/agent_context/mod.rs` and are exported by the library.

Nearby coverage includes `ortho_config/tests/docs_ir_behaviour.rs`,
`cargo-orthohelp/src/agent_context/behaviour_tests.rs`, the reusable schema's
`tests_json.rs` and `tests_round_trip.rs`, the tool's `tests/golden/` fixtures,
`cargo-orthohelp/tests/features/orthohelp_agent_context.feature`, and
`examples/hello_world/tests/agent_context_e2e.rs`. Reuse existing fixture
builders, localizer injection, and temporary directory helpers after a search
for equivalents. New helpers must have documented ownership and allowed uses.

## Conformance basis

The baseline is commit `dc979878b5728bbb23866df9c05c2570bdb5629e`. No separate
terms of reference exists for this task. The following documents at that
revision and the user's request supply the governing requirements.

- R1: `docs/roadmap.md` 7.2.2 and
  `docs/agent-native-cli-design.md` §§6.2–6.2.1: independent support and
  complete presentation vocabulary. M1 delivers the model and E1 mapping
  evidence; M2 delivers E4 observable generation evidence.
- R2: the same design §§6.2, 8.1–8.2 and
  `docs/cli-localization-design.md`: protocol fields stay non-localized, with
  explicit legacy defaults and additive wire compatibility. M1 supplies E2/E3;
  M2 supplies E4 and migration documentation.
- R3: `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`,
  `docs/adr-010-behavioural-metadata-attribute-surface.md`, `docs/design.md`,
  and `docs/cargo-orthohelp-design.md`: retain schema boundaries and sibling
  declaration scope. M1 architecture review and M2 documentation discharge it.
- R4: `docs/agent-native-cli-design.md` §9 and roadmap 13.1: source additions
  require a breaking release until the extensibility work lands. Approval and
  M2 migration/release notes explicitly discharge that constraint.
- R5: `docs/clap-dispatch-and-ortho-config-integration.md` and
  `docs/improved-error-message-design.md`: do not introduce dispatch, loading,
  or improved-error runtime changes as incidental metadata work.

Testing and documentation references to consult during implementation are
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/localizable-rust-libraries-with-fluent.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`,
`docs/rstest-bdd-users-guide.md` and `docs/documentation-style-guide.md`. Their
role is shared fixtures, executable API examples, injected state, Fluent
boundaries, small modules, behavioural wiring respectively. The requested
`docs/rtest-bdd-users-guide.md` is a filename mismatch; use the existing
`rstest` spelling above. The requested `docs/scripting-standards.md` is absent
in this checkout. No new script is planned; if one becomes necessary, obtain
the governing standard before authoring it rather than inventing a document or
claiming conformance.

Use the `execplans`, `codegraph-mcp`, and `rust-router` skills. Route API work
to `rust-types-and-apis`, verification selection to `rust-verification`, and
properties to `proptest`. Load `verus` only if an approved deviation introduces
a proof-worthy kernel. Use `firecrawl-mcp` for external research, `mapsplice`
for roadmap completion, and `commit-message`/`pr-creation` for delivery. Teams
exchange code evidence through `context_pack`; full gates belong to scrutineer.

## Behavioural contracts before implementation

An abstract command has two renderer descriptions, human and machine. Each has
an independently declared support value and a partial map of presentation
properties. A missing property means no claim. The documentation representation
retains the distinction between missing support and explicit false. The agent
projection applies only the documented legacy support defaults: human true,
machine false. These are compatibility values, not observed runtime capability.

Contract C1: projecting a declaration preserves every explicitly supplied
support and policy value. Changing one renderer does not change the other.
Contract C2: absent metadata supplies the support defaults and no invented
presentation claims. Children use their own declarations; nothing is inherited.
Contract C3: changing the localizer can change cosmetic summary text but cannot
change renderer policy, command paths, option names, output field identifiers,
or other protocol data. Contract C4: serialization and the mirrored IR boundary
preserve declared values, absence, and stable wire spellings.

Preconditions are a successfully parsed declaration, valid UTF-8 strings, and
an existing valid metadata tree. The bridge neither tests downstream behaviour
nor proves that a declaration is truthful. Postconditions describe the emitted
metadata only. Structural parsing rejects unknown keys, invalid enum tokens,
wrong literal types, and repeated assignments to the same leaf, even across
repeated groups. Separate groups may contribute distinct leaves.

There are no semantic cross-field rejections in this task. An unsupported
renderer may retain declared policies describing an unavailable implementation;
consumers must check support before acting. Potentially unsafe combinations
remain representable for future lint policy to diagnose. Syntax validity is not
a certificate that a command obeys its declarations.

## Proposed interfaces and representation

Add `DocMetadata.renderer: Option<RendererMetadata>` to both IR owners, with
`#[serde(default, skip_serializing_if = "Option::is_none")]`. Define cohesive
new modules `ortho_config/src/docs/renderer.rs` and
`cargo-orthohelp/src/schema/renderer.rs`, keeping corresponding types aligned.
Each renderer declaration has optional `supported: bool` and optional policy
fields. The top-level block has optional human and machine descriptions.

Add `AgentCommand.renderer: RendererContract` with `#[serde(default)]` in a new
`ortho_config/src/agent_context/renderer.rs` module. `RendererContract`
contains separate human and machine records; their support defaults are true
and false respectively. Missing policy fields are omitted, never replaced by
false or an affirmative promise. Default serialization is exactly:

```json
{"human":{"supported":true},"machine":{"supported":false}}
```

Missing `renderer`, an empty renderer object, an omitted human/machine record,
an empty record, and a missing `supported` leaf must all apply the
role-specific defaults. A present null renderer/record/support is invalid in
agent context; an optional IR renderer accepts null as absence. Optional policy
leaves accept null as absence and serialize absent values by omission. A
missing field list defaults empty and is omitted when empty. Explicit
`fields: null` is invalid. Name and test these cases separately; do not derive
a blanket false support default for the human record.

Do not reuse `SupportDeclaration::default()` for human support: it is false. Do
not change that existing default. Preserve the frozen pre-renderer fixture and
add a new renderer contract fixture rather than overwriting the only historical
evidence. New fields are additive to wire version 1; existing fields keep their
null/omission rules. Rust struct literal consumers must initialize IR
`renderer: None` or wire `renderer: Default::default()` during migration.

The initial policy vocabulary below is proposed for approval. Every token uses
snake_case on the wire. IR options use absence for unknown; agent policy enums
include an `unknown` fallback for unrecognized future tokens, with a tested
Serde fallback. Attribute input accepts only the listed known tokens.

- `tty`: `independent`, `adapts`, `requires`. A terminal (TTY) means stdout's
  terminal status here; it says nothing about stdin. `requires` reports that
  this renderer cannot operate without a terminal.
- `closed_stdin`: `ignored`, `eof`, `error`. These mean stdin is unused, closure
  is treated as end-of-file, or closure causes failure. No exit class is
  implied.
- `colour`, `emoji`, `progress`, and `pager`: separate policies with values
  `never`, `auto`, `always`. `auto` means application-selected, not a promise
  about a particular environment variable or precedence algorithm.
- `width`: `unbounded`, `terminal`, `fixed`. Use a
  `Fixed { columns: NonZeroU16 }` enum variant for a declared positive width,
  avoiding a separate optional column count with contradictory states. Derive
  syntax uses `width = 80` for that variant and `width = "terminal"` or
  `"unbounded"` for the others. Zero and overflow are syntax errors; no width
  calculation is implemented.
- `accessibility`: `unspecified`, `text_equivalent`. The latter declares that
  meaning does not depend solely on colour, emoji, animation, or layout. It is
  an application claim, not an accessibility certification.
- `plain`: `unsupported`, `available`, `always`. Plain output means no ANSI
  decoration, emoji decoration, animated progress, or pager, with textual
  equivalents for meaningful cues. `available` asserts a mode exists; it does
  not add a flag or resolve option precedence.
- `stdout` and `stderr`: independently `none`, `text`, `structured`, `mixed`.
  These describe content categories, not framing or document counts. Structured
  output need not be JSON. Concrete JSON mode is reserved for 7.2.3–7.2.4.
- `progress_inheritance` and `subprocess_inheritance`: `allowed`, `forbidden`
  within each renderer. Inheritance means downstream output can pass through
  directly. Human-only inheritance is human allowed plus machine forbidden;
  neither renderer can promise behaviour for the other. Progress display policy
  remains a separate property.

Width uses an internally tagged object in both IR and agent context:
`{"kind":"terminal"}`, `{"kind":"unbounded"}`, or
`{"kind":"fixed","columns":80}`. In agent context an unrecognized kind becomes
`{"kind":"unknown"}`; its extra fields are ignored. A known fixed kind with
missing, zero, negative, non-integer, or overflowing columns is rejected. The
IR rejects unknown kinds/tokens rather than silently discarding declarations.
Record the same distinction for all new enums: authoring/IR are strict; agent
readers tolerate future tokens as unknown. Round trips preserve known values;
future-token tests assert documented normalization to unknown, not equality to
the original unknown token.

Record output field localization as a list of typed entries on each renderer:
`fields: Vec<RendererField>`, with stable `id: String` and
`localization: localized | non_localized`. Agent readers additionally accept
unknown future classifications as `unknown`, which makes no localization
guarantee; authoring/IR reject them. An identifier is an opaque
application-defined field name, not a JSON pointer or executable lookup.
Non-empty and unique identifiers are derive-input guarantees only: public
records and JSON retain permissive String/Vec shapes. The bridge preserves
hand-built entries without validating or certifying them. The classification
describes the field's value, never its identifier. An empty list makes no claim
about undeclared fields. Preserve order. The derive parser rejects duplicate
IDs within one renderer declaration, using exact comparison; hand-built
duplicate/empty IDs remain transportable and get explicit tests so this
boundary is not mistaken for universal validation. Protocol identifiers and
error classes must be classified non-localized by consumers; this task cannot
identify arbitrary downstream protocol fields. Tests also retain unknown
localization classification as no guarantee. Human-facing text remains
Fluent-owned; the new block carries neither translated values nor Fluent
identifiers into agent context. There is no new translator.

The proposed attribute shape is:

```rust,ignore
#[ortho_config(renderer(
    human(supported = true, tty = "adapts", closed_stdin = "ignored",
          colour = "auto", emoji = "never", progress = "auto",
          pager = "auto", width = "terminal", accessibility = "text_equivalent",
          plain = "available", stdout = "text", stderr = "text",
          progress_inheritance = "allowed", subprocess_inheritance = "allowed",
          field(id = "message", localization = "localized")),
    machine(supported = true, tty = "independent", closed_stdin = "ignored",
            colour = "never", emoji = "never", progress = "never",
            pager = "never", width = "unbounded", plain = "always",
            stdout = "structured", stderr = "structured",
            progress_inheritance = "forbidden", subprocess_inheritance = "forbidden",
            field(id = "error_code", localization = "non_localized"))
))]
```

Wire objects use the same property names, with `fields` for repeated `field`
entries. The bridge explicitly maps each IR enum to its wire counterpart in
`cargo-orthohelp/src/agent_context/renderer.rs`. It does not localize, infer,
sort field declarations, or populate `output_modes`. Document helper ownership:
only the agent-context bridge calls this mapping; public schema modules contain
no terminal probing or adapter dependencies.

## Verification plan

The abstract preservation statements C1–C4 precede the executable design above.
Candidate semantic arguments are pointwise preservation of each declared leaf,
independence of sibling renderers/commands, and commutation of protocol
projection with a change of localizer. They are test specifications here, not
claims of machine-checked mathematical theorems.

Verus is not selected: this change introduces declarative records, finite enum
mapping, and parser validation, with no unbounded algorithm or contractual
business decision kernel. Proving a detached copy of Serde or the bridge would
not verify the shipped boundary. No solver scaffolding, `assume`, `admit`, or
trusted proof bodies are permitted. If implementation adds policy resolution,
precedence, normalization, or substantive cross-field enforcement, stop and
revise the abstract model and proof sketch first. A resulting Verus milestone
must name the actual proof artefact, representation relation, satisfiable
premises, semantic lemmas, negative controls, and an explicit working Verus
verification command alongside the ordinary gates before approval.

E1, C1/C2: add `rstest` tables in proposed
`cargo-orthohelp/src/agent_context/renderer_tests.rs` for every enum variant,
all four support combinations, absent blocks, partial blocks, explicit false,
and independent child/sibling declarations. Use `googletest::assert_that!` for
structural assertions and `pretty_assertions::assert_eq!` for exact records.
Add named tests in `ortho_config/tests/docs_ir_renderer.rs` and parser tests
under `ortho_config_macros/src/derive/parse/tests/renderer_attrs.rs`; cover
unknown keys, wrong types, duplicates across groups, empty field IDs, zero
width, overflow, and accepted maximum width. A deliberate swapped human/machine
mapping must fail E1; a missing declaration and explicit false witness ensure
that default tests cannot pass vacuously.

E2, C4: retain the old wire fixture, deserialize it with the new reader, and
assert legacy renderer defaults. Serialize/deserialize every new shape through
the library IR, tool IR mirror, agent context, and the existing JSON validation
entry point. Inject unknown object keys and future policy tokens; unknown
fields are ignored in both schemas, new agent policy tokens yield unknown, and
IR policy tokens are rejected. Existing enum semantics remain unchanged.
Exercise all nested default/null cases above, the four width object forms, and
invalid known fixed widths. Known-value round trips and unknown-token
normalization are separate assertions. Add `insta` snapshots for absent,
human-only, machine-only, and dual declarations in the tool's golden suite.
Snapshots of old man and PowerShell output must remain unchanged. Removing the
mirror field or changing a wire token is a negative control that must fail
these tests.

E3, C3/C4: extend `cargo-orthohelp/src/agent_context/proptests.rs` with
generated renderer combinations, non-empty Unicode field IDs and distinct
localizer responses. Use pure in-process transformations for 256 cases, strings
of 1–64 characters, and 0–16 field entries; construct unique IDs without
rejection-heavy filtering. Compare the protocol projection after removing only
cosmetic summary, and compare round trips. Include empty and populated lists
and both renderer support values as named cases. Record class coverage and
seed; a localizer that returns a visibly different summary must demonstrate the
human change. A temporary mutation that localizes a field ID must make the
property fail. These bounded generated tests support integration confidence,
not an unbounded proof.

E4: extend `cargo-orthohelp/tests/features/orthohelp_agent_context.feature` and
its existing rstest-bdd steps/scenario registrations. Include these scenarios:

```gherkin
Feature: Declared dual-renderer metadata
  Scenario: Independent support survives generation
    Given a fixture declaring human support and no machine support
    When agent-context output is generated
    Then human support is true and machine support is false
    And declared human presentation policies are preserved

  Scenario: Child declarations are independent
    Given a dual-renderer root with an undeclared child
    When agent-context output is generated
    Then the child has legacy support defaults and no presentation claims

  Scenario: Human language does not rename protocol fields
    Given a renderer fixture with localized message values and stable error codes
    When the fixture is transformed with two distinct localizers
    Then renderer field identifiers and policy tokens are identical
    And the cosmetic summaries differ
```

Extend `cargo-orthohelp/tests/nested_subcommand_end_to_end.rs` using its
existing subprocess/fixture harness for `--format agent-context` and
`--format all`. Parse stdout and the generated `agent-context.json`, compare
the declared renderer data, and verify existing artefacts and combined
policy-report stdout selection. This is the actual external workflow; it does
not purport to test downstream TTY or pager behaviour. Run children with closed
stdin and a bounded harness timeout to ensure this metadata-only path does not
start prompting. Keep ambient environment changes inside existing guarded
helpers, preferring injected localizers and inputs.

External assumptions: Serde follows its documented default and unknown-field
behaviour; the derive parser reports spans; Fluent/localizer implementations
return supplied text; the existing bridge harness invokes the selected local
crate. Exercise these real boundaries in E2/E4 rather than proving third-party
internals. Truthfulness of downstream declarations and actual terminal
accessibility remain outside the guarantee.

## Plan of work and milestones

### M1: Deliver the complete declaration-to-wire contract

After explicit approval, first record the approved representation and source
migration in a new numbered ADR following the repository template; choose the
next free ADR number. Link it from agent-native design §6.2.1 and document the
vocabulary/default table there. Search for existing helpers before extraction.

Write red tests for E1/E2 and the E3 locale property before production changes.
Missing-type compile failures establish only an initial API red stage; once
minimal types exist, obtain failing behavioural assertions before filling in
mapping logic. Record the commands and failure reason, then implement the
smallest complete path through types, parser, token emission, both IR schemas,
public exports, and the bridge. Update every affected struct literal, including
examples and doctests, in this same atomic change. Use focused new modules to
keep existing files below the size ceiling. Do not commit a disconnected schema
whose derive or adapter silently drops declarations.

Acceptance: E1, E2, and E3 pass; source constructors compile everywhere; new
agent context carries renderer metadata; unchanged human snapshots remain
stable. Run the full ordinary gates below sequentially, then commit the green
plateau. Check R1–R4 traceability, schema ownership, all explicit defaults, and
absence of unrelated API or dependency changes. Remaining work is expanded
workflow evidence and final documentation. Recover by reverting the whole
milestone commit, not one side of the schema mirror.

### M2: Demonstrate workflows and finish consumer documentation

Add the E4 scenarios and subprocess tests red-first, using new nested fixture
declarations. Implement only missing integration wiring revealed by those
failures; revisit the plan if substantive new semantics are required. Review
snapshots individually, especially stdout, legacy defaults, and localized
summaries. Complete user examples and migration guidance in
`docs/users-guide.md`; update `docs/developers-guide.md` with mapping
ownership, fixture discipline, and test commands. Update component architecture
in `docs/cargo-orthohelp-design.md`, the main `docs/design.md`, the relevant
localization design boundary, and `docs/contents.md` for any new ADR or guide.
Document any proof-relevant architecture decision there if scope has changed.

Acceptance: E1–E4 and all gates pass; no outstanding deviation or unproved
selected obligation remains; the next release's migration note states the
source break. Mark 7.2.2 and its three child checkboxes done using `mapsplice`,
leaving future tasks unchecked. Reconcile this plan and all governing design
references before setting implementation status COMPLETE. Run documentation
gates after the closure edits, commit the green plateau, and inspect the final
diff for unrelated changes. A revert of M2 retains the coherent M1 feature but
withdraws the completion claim. Publish implementation only under its separate
authorization; this planning task ends at the draft plan PR.

## Concrete steps and acceptance commands

Run from the repository root on the requested branch. Establish the baseline
and repeat the relevant suites before and after changes. The current Makefile,
not older prose describing its recipes, is authoritative: `make test` includes
all-target/all-feature Rust tests and Python tests. Preserve compiler wrappers
and the shared Cargo cache. A scrutineer agent runs full gates sequentially.

```bash
set -euo pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-7-2-2-dual-renderer-metadata.out
make lint 2>&1 | tee /tmp/lint-ortho-config-7-2-2-dual-renderer-metadata.out
make test 2>&1 | tee /tmp/test-ortho-config-7-2-2-dual-renderer-metadata.out
```

Stop at a failing gate; do not claim success from a truncated log. Each major
implementation milestone requires all three commands to exit zero. Run tests
for public documentation explicitly because the all-targets recipe does not
replace doctests. The focused commands below are run sequentially and logged
with the same `pipefail`/`tee` discipline, using distinct descriptive filenames:

```bash
cargo test -p ortho_config --test docs_ir_renderer
cargo test -p ortho_config_macros renderer
cargo test -p ortho_config --all-features agent_context
cargo test -p cargo-orthohelp --lib renderer
cargo test -p cargo-orthohelp --test golden_tests
cargo test -p cargo-orthohelp --test rstest_bdd
cargo test -p cargo-orthohelp --test nested_subcommand_end_to_end
cargo test --workspace --doc --all-features
```

The renderer tests and modules are proposed additions, not existing passing
commands. The selected E4 integration target is registered already; extend it
with the new agent-context cases. Expected evidence is collected tests passing,
not a zero-test filter. Preserve red-stage logs, green-stage logs, snapshots,
property seed/class coverage, and negative-control failures in the milestone
record. Restore each negative control before gates.

For every documentation change run `make fmt`, inspect the entire diff, then
`make check-fmt`, `make markdownlint`, and `make nixie` sequentially. Capture
outputs under `/tmp`. Formatting may touch unrelated tracked Markdown: restore
only tool-produced unrelated changes after inspecting them. Never edit
`typos.toml` by hand; generated unrelated dictionary drift requires its own
base change under repository instructions. Keep every gate's status explicit.

Planning-only acceptance is a reviewed plan, correct index link, passing
documentation gates, and a draft PR. The implementation gates above are future
acceptance criteria; do not report them as having passed during planning unless
actually executed and recorded.

## Idempotence and recovery

Before resuming, read this plan, `AGENTS.md`, branch/status, and the milestone
logs. Do not recreate already committed types or regenerate fixtures blindly.
Keep failed commands and infrastructure diagnostics; after a source fix rerun
the affected gate and remaining gates sequentially. Do not change shared
services, kill other agents' processes, or bypass compiler admission.

Use atomic commits with a gated rollback point at each milestone. Keep frozen
old JSON fixtures, and do not approve changed snapshots solely because they
were regenerated. If the source compatibility decision or a wire-contract
assumption fails, record alternatives and set BLOCKED pending approval.

## Artefacts and notes

Wyvern code evidence is captured in context pack `pk_yuerydym` and document
evidence in `pk_ks47tdca`, including exact paths and line references. The
repository paths above remain the durable source of truth if the temporary pack
expires.

Firecrawl research on 2026-10-14 checked the
[colour-suppression convention][no-colour]: a non-empty variable suppresses
colour, not bold/underline/italics; explicit user configuration or command-line
choices may override it. This plan records metadata only and does not implement
that precedence. It also checked [googletest documentation][googletest]. Use
fatal `assert_that!` assertions with `rstest`; non-fatal assertions need the
framework's test context and should not be introduced casually.

[no-colour]: https://no-color.org/
[googletest]: https://docs.rs/googletest/latest/googletest/

## Revision note

2026-10-14: Initial plan based on current code and two Wyvern investigations.
The draft separates wire compatibility from source migration, preserves schema
ownership, and selects tests for declarative contracts. Implementation awaits
approval.

2026-10-14 review revision: Pandalump/Wafflecat identified a conflict between
global inheritance labels and independent renderer records; replaced them with
per-renderer allowed/forbidden. Telefono/Buzzy Bee requested explicit width
wire forms and nested defaults, plus distinct strict-IR and tolerant-agent
decoding contracts. Doggylump/Dinolump requested fail-fast gate commands. These
clarifications constrain M1 and its E1/E2 cases without expanding scope.

2026-10-14 validation revision: Recorded successful documentation gates and
expert re-review. Final publication remains the next action; no implementation
work is authorized by these checks.

2026-10-14 spelling revision: Reworded the external colour-suppression
reference and its link label after the post-turn spelling gate rejected the
unquoted upstream spelling. The source URL and planned behaviour are unchanged.

2026-10-28 approval: The user explicitly approved this plan, including the
deliberate pre-1.0 Rust source break for the new metadata fields. The status is
now APPROVED and the approval progress item is checked. M1 may begin; no
requirement, tolerance, or design decision changed.
