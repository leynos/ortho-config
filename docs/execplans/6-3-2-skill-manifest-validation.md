# Validate skill manifests against real commands

This ExecPlan is a living document. Keep `Constraints`, `Tolerances`, `Risks`,
`Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan`
current at every milestone.

Status: DRAFT

## Purpose / big picture

After this work, a consumer can submit the command references and structured
examples it extracted from a skill to a reusable `cargo_orthohelp` validator.
The validator reports unknown command paths, flags absent from the referenced
command, duplicate manifest identifiers, and vocabulary/global-option
violations under the configured policy. It checks skill claims against the same
complete command contract used by agent context.

The validator does not read files, parse arbitrary Markdown, or execute shell
examples. A consumer owns manifest loading, format validation, prose parsing,
and example extraction. An example arrives as a normalized command path, option
names with value presence and values, and positional values. This makes the
claims checkable without guessing shell grammar. A prose-only skill stays valid
and reports zero command and example coverage.

This is an approval-gated plan. No implementation starts until the maintainer
explicitly approves it and the prerequisites below are complete.

## Constraints

- Roadmap 6.3.2 requires 6.3.1 and step 7.1. Do not implement until 6.3.1,
  7.1.2, and 7.1.3 are complete on the implementation base. Do not implement
  those prerequisite items in this plan.
- Keep skill-specific prose, file I/O, extraction, and domain execution in
  consumer applications. The proposed reusable API belongs in
  `cargo_orthohelp`; do not move Weaver or Netsuke execution semantics into
  OrthoConfig.
- Reuse the completed 7.1.3 glossary/policy contract and an explicit
  per-command effective option surface. Roadmap 7.1.3 does not itself promise
  metadata that marks inherited options. Never infer inherited options by
  unioning root inputs into child commands. If a consumer cannot provide
  unambiguous effective options, stop and mark the plan `BLOCKED`.
- Reuse 7.1 canonical policy rules, modes, and exceptions. Do not invent a
  second vocabulary or reject arbitrary domain commands/flags as if canonical
  vocabulary were an allowlist.
- Structural findings are independent of policy mode. `off` may suppress
  vocabulary findings, but it cannot suppress unknown paths, unknown flags,
  malformed/ambiguous normalized inputs, or duplicate manifest IDs. Exceptions
  can never authorize a nonexistent command or flag.
- Validate supplied claims only. Do not promise exhaustive discovery from
  arbitrary prose, general Markdown or shell parsing, executable command
  validity, option arity, quoting, side effects, or example runtime success.
  Preserve unsupported/unchecked example status explicitly; do not silently
  count it as validated.
- Treat manifest paths as inert metadata. Do not load them automatically.
  Findings identify a stable manifest ID and source ordinal, not raw paths or
  argument values. Do not add telemetry for local coverage counts.
- Build command/option indexes once per validation call; do not scan the full
  command tree for every claim. Emit at most one finding per invalid path or
  per declared flag/option. The library has no hidden input-size cap; the
  caller owns input byte limits and the report bound follows input claim count.
- Preserve the agent-context wire contract unless review proves a change is
  needed. A schema change requires a design decision and its versioning
  consequences.
- Use the shared Cargo cache and compiler admission. Run gates sequentially;
  do not bypass wrappers or install tools implicitly. Keep Rust files below 400
  lines and document public APIs/modules.

## Tolerances (exception triggers)

- Stop if prerequisites are incomplete, explicit per-command effective
  options are missing, or ownership/extraction semantics remain ambiguous.
- Stop if a public wire schema or released API must change, a runtime
  dependency is required, or the library must own file I/O or unrestricted
  source parsing.
- Stop if implementation exceeds 30 changed files or 1,500 net non-generated
  lines, or any Rust source file would exceed 400 lines.
- Stop if review proposes materially different API or example binding
  semantics; record options and impacts before changing this design.
- Stop after two focused repairs for the same failing gate.
- If implementation reveals non-trivial contractual business logic beyond
  exact finite lookup and reuse of 7.1 policy, stop and revise the proof sketch
  and verification method before continuing.

## Risks

- Risk: `agent-native-cli-design.md` §3.4 assigns validation to consumers,
  while current `SkillManifest` Rustdoc says `cargo-orthohelp` validates it.
  Severity: high; likelihood: high. Mitigation: reconcile ownership in design
  docs before coding: reusable validator called by consumers; consumer owns I/O
  and extraction.
- Risk: `AgentInput` lacks explicit global/inherited metadata, and current
  policy evaluation runs before bridge command inputs are built. Roadmap 7.1.3
  promises a glossary/linter, not necessarily inheritance metadata. Severity:
  high; likelihood: high. Mitigation: require an explicit downstream-resolved
  effective option surface and never synthesize inheritance.
- Risk: raw `AgentExample.command` cannot reliably preserve parsing or option
  values. Severity: high; likelihood: high. Mitigation: use normalized examples
  and test the consumer adapter against controlled fixture text.
- Risk: structured examples can drift from their Markdown source. Severity:
  high; likelihood: medium. Mitigation: integration fixture adapter extracts
  from the same source submitted to validation, with a drift negative control.
  Consumer parser internals remain outside this library's proof boundary.
- Risk: vocabulary findings could hide structural failures in `off` mode or
  exceptions could authorize invalid claims. Severity: medium; likelihood:
  medium. Mitigation: separate structural and policy result classes and test
  mode/exception cross-products.
- Risk: future work may add a substantive invariant that needs formal proof.
  Severity: medium; likelihood: low. Mitigation: current exact finite lookup
  does not warrant Verus; revise the plan first if a non-trivial pure kernel
  appears.

## Progress

- [x] (2026-10-14 17:30Z) Read roadmap, design, metadata types, policy seams,
  ExecPlan guidance, and the 6.3.1 plan.
- [x] (2026-10-14 17:30Z) Confirmed 6.3.1 is complete while 7.1.2 and 7.1.3
  remain incomplete; recorded a hard implementation gate.
- [x] (2026-10-14 17:30Z) Collected Wyvern reconnaissance and community-of-
  experts review via context pack `pk_f4rgdzgk`.
- [x] (2026-10-14 17:32Z) Incorporated three community-of-experts reviews;
  no material blocker remains in the planning design.
- [x] (2026-10-14 18:00Z) Recorded planning baseline `make lint` passed rustdoc,
  Clippy, and Whitaker. No runtime code changed.
- [x] (2026-10-14 19:38Z) Planning gates passed: `make fmt`,
  `make check-fmt`, `make lint`, `make test`, `make markdownlint` (including
  spelling), and `make nixie`. Only the plan and documentation index changed.
- [ ] Begin implementation only after explicit approval and EP-M0 passes.

## Surprises & discoveries

- Observation: `SkillManifest` and `SkillCommandRef` exist as serializable
  metadata, but there is no manifest reader or validator; current agent-context
  fixtures contain no populated skill manifests. Evidence:
  `ortho_config/src/agent_context/mod.rs` and `cargo-orthohelp` agent-context
  fixtures. Impact: plan an explicit supplied input boundary rather than
  assuming a parser exists.
- Observation: `AgentExample.command` is a raw string and cannot safely
  preserve option values. Evidence: `ortho_config/src/agent_context/mod.rs`.
  Impact: normalize example claims at the consumer boundary.
- Observation: 7.1.2 and 7.1.3 are incomplete; `PolicyInputs` is empty.
  Evidence: `docs/roadmap.md` and `cargo-orthohelp/src/policy/config/mod.rs`.
  Impact: implementation must wait for the 7.1 policy and an explicit
  per-command effective option surface from the consumer or bridge.
- Observation: `docs/scripting-standards.md` is absent. The requested
  `docs/rtest-bdd-users-guide.md` is also absent; the repository has
  `docs/rstest-bdd-users-guide.md`. Impact: signpost the existing guide and do
  not invent a missing document.
- Prior art: the Agent Skills specification defines YAML frontmatter and
  Markdown body but no standardized command/example grammar for arbitrary
  prose. It points to `skills-ref` for structure and naming validation. This
  supports downstream extraction; it does not define OrthoConfig policy.
  Source: <https://agentskills.io/specification> (Firecrawl retrieval,
  2026-10-14).

## Decision log

- Decision: Propose `cargo_orthohelp::skill_validation` as a reusable API
  called by consumers. Consumers retain manifest I/O, format checks, prose
  parsing, and example extraction. Rationale: this reconciles design §3.4 with
  current Rustdoc and preserves the consumer boundary. Date/Author: 2026-10-14,
  draft.
- Decision: Represent examples as normalized claims bound to manifest ID and
  command path. Preserve each option name, value-presence, values, positionals,
  and source ordinal. Rationale: validation cannot be sound if option values
  are discarded or shell syntax is guessed. Unsupported extraction remains
  explicitly unchecked. Date/Author: 2026-10-14, draft.
- Decision: Keep structural and policy findings separate. Structural failures
  are always returned; policy mode and exceptions affect vocabulary and
  global-option findings only. Rationale: `off` cannot make a false command
  claim valid. Date/Author: 2026-10-14, draft.
- Decision: Use only explicit effective per-command option sets. Do not infer
  child globals from root inputs. Rationale: current `AgentInput` does not
  encode inheritance. Date/Author: 2026-10-14, draft.
- Decision: Coverage counts are descriptive, not completeness thresholds.
  Empty refs/examples remain valid for prose-only skills. Rationale: the body
  format has no standard command-section grammar. Date/Author: 2026-10-14,
  draft.
- Decision: Do not add a Verus milestone for exact finite path/flag lookup.
  State soundness/completeness as acceptance obligations and compare the
  implementation against an independent linear oracle with generated cases and
  negative controls. Rationale: proof would formalize list membership but would
  not cover the uncertain extraction/global-option bridge; property and
  integration tests give more relevant evidence here. If implementation adds
  non-trivial contractual logic, pause and revise the proof sketch before
  proceeding. Date/Author: 2026-10-14, draft after expert review.

## Outcomes & retrospective

Planning only. No implementation has started. Wyvern reconnaissance and all six
expert perspectives have been reconciled. Full repository checks passed for
this documentation-only change; they do not constitute feature acceptance. The
shared compiler cache was unavailable, so the admitted builds ran uncached.
Record implementation outcomes and lessons at later milestones. Do not mark the
plan complete while prerequisites, design updates, or acceptance gates remain.

## Context and orientation

The reusable agent-context schema lives in
`ortho_config/src/agent_context/mod.rs`. `AgentCommand` holds a command path,
inputs, a canonical verb, and examples. `SkillManifest` holds a stable ID,
relative path, opaque schema-version string, and command references.
`SkillCommandRef.path` is exact; flags are exact long names without `--`. An
empty command-reference list is valid for a prose-only skill.

`cargo-orthohelp` owns the bridge and policy machinery. Current `PolicyInputs`
is empty, and policy checking occurs before bridge command metadata is built.
The 7.1.2/7.1.3 work is a real contract dependency. The proposed API receives
already extracted skill claims plus a complete command surface; it does not add
automatic file loading or a new CLI command.

Fixtures should resemble an operation skill and a build-workflow skill, as
requested, using generic command shapes and opaque values. The validator tests
structure, vocabulary, and bindings; it does not encode Weaver or Netsuke
domain semantics.

## Conformance basis

- Planning base: `dc979878b5728bbb23866df9c05c2570bdb5629e` on branch
  `6-3-2-skill-manifest-validation`. Recheck the base and prerequisite state
  before implementation; this planning snapshot does not certify future heads.
- `docs/roadmap.md` §6.3.2 requires real command-path and flag validation,
  examples checked against canonical vocabulary/global options, and generic
  operation/build-workflow fixtures. Prerequisite 6.3.1 is complete;
  prerequisites 7.1.2 and 7.1.3 are not. See `EP-M0`.
- `docs/agent-native-cli-design.md` §3.4 assigns skill prose/workflows to
  downstream apps and says manifests are checked against command metadata. §5
  defines canonical verbs, flags, policy, and global-option glossary. Reconcile
  its ownership with current Rustdoc before implementation.
- `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md` governs
  agent-context schema ownership. [ADR-008][adr-008] governs opt-in policy
  modes and exceptions.
- `docs/execplans/6-3-1-skill-manifest-metadata.md` is the preceding schema
  decision; validation was explicitly deferred to 6.3.2.
- Trace links: `ROADMAP-6.3.2-PATH-FLAG` -> `EP-M2` -> structural tests,
  property oracle, and negative controls; `ROADMAP-6.3.2-EXAMPLES-VOCAB-GLOBAL`
  -> `EP-M3` -> normalized example and policy tests plus fixture integration;
  `ROADMAP-6.3.2-FIXTURES` -> `EP-M3` -> operation and build-workflow fixtures
  and BDD scenarios.
- No separate terms of reference exists. The roadmap and
  `agent-native-cli-design.md` are the governing requirement and design.

## Verification plan

Abstract model: let `C` be a finite set of command contracts with unique exact
paths and explicitly effective long options. Let `M` map unique manifest IDs to
declared command references `R`, and let `E` be normalized examples. A
reference resolves iff its path occurs in `C` and each flag listed on that
reference occurs in the resolved command's effective options. An example is
owned only when its `command_reference_ordinal` selects a reference in the
owning manifest, its path equals that reference's path, and each option name
occurs in that reference's declared flags. It is real only when that path
resolves in `C` and each option name belongs to the resolved command. Option
values are preserved, not tested for membership in `C`. Policy is a separate
function over resolved uses and the approved 7.1 policy.

Input-integrity predicate: a command surface is valid only when every path is
unique and each path has one unambiguous effective option set; duplicate paths
or conflicting duplicate option metadata return a typed surface error before
claims are evaluated. Manifest IDs must be unique; duplicate IDs produce a
structural finding keyed by ID and ordinal. Each example binds to the owning
manifest and a specific command-reference source ordinal, so duplicate paths in
a manifest do not cause an arbitrary reference to win. For a reference with an
unknown path, emit the path finding and do not cascade into unknown-flag
findings for that reference.

Postconditions: each structural finding corresponds to a failed integrity or
membership predicate and every failed predicate yields its defined finding;
policy mode/exceptions cannot change structural findings. Each offending claim
produces at most one finding for its path and one per declared flag or example
option. No postcondition claims exhaustive prose scanning, shell executability,
option arity, or domain-value correctness. These are contracts for tests, not a
decision to introduce formal verification.

- `INV-REF-SOUND-COMPLETE`: compare validator findings with an independent,
  deliberately simple linear reference oracle for generated command/reference
  lists. Artefact: `cargo-orthohelp/src/skill_validation/proptests.rs`.
  Evidence: `cargo test -p cargo-orthohelp skill_validation`; generate empty,
  singleton, nested, matching, missing, and reordered cases without excessive
  filtering. Include witnesses for each class and a mutation that swaps path or
  flag equality; the property must reject it. This is proportionate for finite
  exact lookup and can fail on omitted, widened, or mis-scoped checks.
- `INV-EFFECTIVE-INPUTS`: table-test that the consumer/bridge adapter passes
  explicit per-command effective inputs exactly, including an explicit global
  option and a similarly named root-only non-global option. Artefact:
  `cargo-orthohelp/src/skill_validation/tests.rs`. A root-option-union mutant
  must fail. This test covers the real adapter boundary; the oracle alone does
  not prove clap metadata.
- `INV-EXAMPLE-BINDING`: an example must bind to a specific reference ordinal
  in its owning manifest, and each example option must be in that reference's
  declared flags as well as the real effective inputs for that path. Generated
  and table tests preserve option name, value presence, values, example source
  ordinal, and referenced-command ordinal. Cover attached/separate and repeated
  values, booleans, positionals, unknown options, wrong owner, missing index,
  unsupported extraction, and malicious path/argv strings. Apply values only to
  value-sensitive 7.1 rules (for example enum values or a global option's
  canonical shape); preserve other domain values without validating them. A
  duplicate-path pair with disjoint flags must not allow an example to union
  their flags. A mutant dropping values or rebinding a child example to root
  options must fail. No process or filesystem effects may occur.
- `INV-POLICY-SEPARATION`: test modes `off`, `warn`, and `deny`, exceptions,
  noncanonical vocabulary, global-option near-misses, and structural errors.
  Structural findings remain in all modes; exceptions only affect policy
  results. Each mode must have structural and policy witnesses. A mutant hiding
  structural errors under `off` or accepting a nonexistent exception flag must
  fail.
- `INV-FIXTURE-EXTRACTION`: `rstest-bdd` plus end-to-end fixture adapter
  verifies the extracted records match source fixtures. A drift mutation that
  edits fixture text without changing extracted claims must fail. Use generic
  operation-skill and build-workflow skill fixtures; do not embed either
  application's domain semantics. Empty coverage must be represented honestly.

No introduced contractual business logic is selected for formal verification:
exact finite membership and reuse of completed policy rules have direct,
independent executable oracles, while the uncertain integration boundary is
consumer extraction and real bridge metadata. Verus would add little assurance
for the membership helper and would not prove the integration obligations. If
implementation adds a stateful or non-trivial all-input contract, return to
proof-sketch-first design, select Verus only if warranted, pin the tool, and
add an explicit command and acceptance criterion before implementation. Do not
add `assume`, `admit`, or trusted proof shortcuts.

External assumptions: clap's generated command tree represents its configured
commands; the consumer or bridge supplies a correct explicit effective-option
surface; the consumer adapter extracts intended claims; Serde carries input
values as modelled. Do not verify third-party internals. Exercise repository-
owned bridge and adapter logic against real fixtures and record residual
extraction limitations.

## Plan of work

### EP-M0: Recheck prerequisites and reconcile design

Verify 6.3.1, 7.1.2, and 7.1.3 on the implementation base. Confirm 7.1 exports
canonical rules and exceptions. Roadmap 7.1.3 does not promise inherited-option
metadata, so also identify an explicit consumer- or bridge-resolved effective
option set for each command, including global-option classification. Reconcile
`agent-native-cli-design.md` §3.4 with current Rustdoc and policy pipeline:
define a reusable validator API invoked by consumers, with downstream I/O and
extraction. Record any required design or ADR update before code work.

Go only when prerequisites, inputs, owners, and approved design agree. If not,
mark `BLOCKED`, record evidence and options, and stop. Do not implement
prerequisites or work around missing metadata.

### EP-M1: Specify normalized input and passing contract tests

Candidate module: `cargo-orthohelp/src/skill_validation/`. Define supplied
manifest claims, structured examples, effective command inputs, separated
structural/policy findings, deterministic coverage counts, stable codes, and
finding identity based on manifest ID plus source ordinal. Keep paths inert and
diagnostics free of raw argv values. Represent unsupported/unchecked examples
explicitly. Empty reference/example lists are valid and yield zero counts.

Add passing `rstest` contract tests for construction, deterministic ordering,
empty input, and typed rejection of ambiguous command surfaces. The
missing-path regression belongs to EP-M2 so this plateau ends green. Establish
the abstract equivalence contract and independent test oracle before production
implementation.

### EP-M2: Implement structural resolution

Implement exact command-path and scoped long-flag checks. Diagnose duplicate
manifest IDs deterministically. Structural findings remain visible in `off`
mode; exceptions cannot suppress them. Do not normalize case, aliases,
punctuation, or `--` prefixes because 6.3.1 specifies exact path segments and
long names without the prefix.

Run focused `rstest`, `proptest` against the independent linear oracle, and
mutation controls for swapped predicates, omitted references, and wrong-command
lookup. This plateau provides correct structural validation without vocabulary
or example policy integration.

### EP-M3: Validate examples, apply policy, and exercise consumer fixtures

Connect to completed 7.1 policy and explicit effective command inputs. An
example path must be declared by its owning manifest, and its options must be
declared by that path's command reference and exist on the real command.
Preserve value presence and contents; inspect values only for value-sensitive
7.1 policy rules, not general option arity or domain validity. Apply canonical
vocabulary and global-option rules with configured modes and exceptions. Do not
parse raw `AgentExample.command` as shell.

Add generic operation and build-workflow skill fixtures. A narrow test adapter
extracts the deliberately small fixture convention and sends those records to
the API; it is integration evidence, not a supported general Markdown parser.
Store the fixtures at
`cargo-orthohelp/tests/fixtures/skills/operation/SKILL.md` and
`cargo-orthohelp/tests/fixtures/skills/build-workflow/SKILL.md`. Use realistic
workflow-shaped examples with opaque resource names and values; do not encode
application semantics or execute commands. In these test fixtures only, use a
`Command index` heading followed by one JSON array of `SkillCommandRef`
records, and a `Checked examples` heading followed by one JSON array of the
normalized example records defined below. Each array occupies a fenced `json`
block. The test adapter requires each heading exactly once, rejects malformed
arrays, and extracts directly from these blocks; surrounding workflow prose
remains opaque. This convention is fixture-owned, not a new public manifest
format. A source mutation replacing a known option with `missing-option` must
change the extracted record and yield an unknown-option finding. A mutant
adapter returning stale records must fail the source-to-record assertions.

Add an `insta` report snapshot only if multivariant report output is part of
the approved contract. Add end-to-end tests because this is a consumer
integration boundary.

The feature specification lives at
`cargo-orthohelp/tests/features/skill_manifest_validation.feature`; step glue
lives in `cargo-orthohelp/tests/skill_validation_bdd.rs`; consumer extraction
end-to-end coverage lives in `cargo-orthohelp/tests/skill_validation_e2e.rs`.
Run `cargo test -p cargo-orthohelp --test skill_validation_bdd` and
`cargo test -p cargo-orthohelp --test skill_validation_e2e`. The feature is:

```gherkin
Feature: Validate skill claims against an agent command contract

  Scenario: Report a command reference with an unknown flag
    Given a command contract with a nested command and effective options
    And an operation skill fixture with a valid reference and an unknown flag
    When the consumer adapter submits the extracted claims
    Then the report resolves the valid reference
    And reports the unknown flag for the referenced command
    And identifies the manifest and source ordinal

  Scenario: Check normalized example values and global options
    Given a command contract with explicit effective global options
    And a build workflow example with an option name and value
    When the adapter submits the normalized example
    Then the option is checked on that command
    And value presence and contents are preserved

  Scenario: Report a noncanonical option under deny policy
    Given a real command accepting the declared option "colour-policy"
    And an example using "colour-policy" with value "always"
    And deny policy without an exception for that option
    When the consumer submits the normalized example
    Then structural validation succeeds
    And the policy finding recommends "--color"

  Scenario: A root-only option is unavailable on a child
    Given a root command with a non-global option "root-only"
    And a child whose explicit effective options omit "root-only"
    And a child example declaring and using "root-only"
    When the consumer submits it for validation
    Then an unknown-option structural finding is returned

  Scenario: Accept a prose-only skill without implying coverage
    Given a manifest with no command references or examples
    When the consumer submits it for validation
    Then no coverage error is reported
    And checked command and example counts are zero
```

### EP-M4: Document and close acceptance

Update `docs/agent-native-cli-design.md` with ownership, normalized example
contract, structural-versus-policy results, and extraction limits. Update
`docs/developers-guide.md` for internal interfaces/practice and
`docs/users-guide.md` for the consumer API and observable validation behaviour.
Create/update an ADR if ownership or result separation is a substantive
architecture decision. Update `docs/contents.md`. Mark roadmap 6.3.2 done only
after all accepted behaviour and gates pass.

At every major milestone run `make check-fmt`, `make lint`, and `make test`
sequentially. Final acceptance also runs `make markdownlint` and `make nixie`.
Use `/tmp` logs per repository convention, inspect terminal summaries, and do
not parallelize gates.

## Milestones and plateaus

### EP-M0 — prerequisite and design gate

Requirements: 6.3.1, 7.1.2, 7.1.3 and ownership reconciliation. End state:
verified prerequisites, explicit API source, and accepted design. Evidence:
roadmap readback and API/design references. Recovery: resume when upstream work
lands. Remaining gaps: all implementation. Compatibility decision: none.
Conformance: requirements complete; ownership explicit; assumptions valid; no
API/dependency invented; no trust or persisted format changed; links current.

### EP-M1 — normalized contract and passing specification

Requirements: reference, example, policy-separation, and prose-only contracts.
End state: input/result contract, empty-input success, and deterministic-order
tests pass. Evidence: ambiguous-surface cases fail with typed errors and
contract tests pass. Recovery: retain tests/spec and revise only through
approved design. Remaining gaps: reference validation. Compatibility decision:
none absent a named consumer. Conformance: exact scoping and values
represented; no file/shell boundary added; no wire schema changed without
approval; links current.

### EP-M2 — structural validator

Requirements: `ROADMAP-6.3.2-PATH-FLAG`. End state: exact structural
resolution, deterministic findings, empty prose-only support. Evidence: the
missing-path red test fails before code and passes after; unit/property tests
pass and seeded wrong-path/flag mutations fail. Recovery: roll back
implementation while retaining approved specification. Remaining gaps: policy
and examples. Compatibility decision: none. Conformance: result corresponds to
exact metadata; consumer ownership preserved; effective-input assumptions hold;
no unapproved API/dependency or I/O; links current.

### EP-M3 — examples, policy, and integration

Requirements: `ROADMAP-6.3.2-EXAMPLES-VOCAB-GLOBAL` and
`ROADMAP-6.3.2-FIXTURES`. End state: examples and policy checked with generic
operation/build-workflow fixtures. Evidence: unit, property, BDD, E2E, and
required snapshots pass; drift and policy-separation mutants fail. Recovery:
retain EP-M2 and roll back integration. Remaining gaps: docs and final gates.
Compatibility decision: none. Conformance: values and paths preserved; policy
reused from 7.1; no domain semantics or file loading; no unapproved
schema/dependency; links current.

### EP-M4 — docs and acceptance

Requirements: all roadmap bullets and documentation obligations. End state:
docs match implementation and all sequential gates pass. Evidence:
`make check-fmt`, `make lint`, `make test`, `make markdownlint`, `make nixie`,
and BDD/E2E targets pass. Mark roadmap item done only now. Recovery: repair
from cited logs and rerun affected plus downstream gates in sequence. Remaining
gaps: none except recorded follow-up. Compatibility decision: none.
Conformance: requirements/docs agree; deviations accepted; assumptions and
residual gaps recorded; dependencies authorized; trust/persistence unchanged;
trace and evidence current.

## Concrete steps

Run commands from the repository root. Before each code milestone, read the
current roadmap and this plan's milestone; record branch and base commit. Keep
gate output in `/tmp` with the action/project/branch template in `AGENTS.md`.

At EP-M0 inspect the exact 7.1.2/7.1.3 APIs. The BDD and E2E commands are
specified in EP-M3; confirm their integration targets exist when implemented.
At each implementation milestone follow Red-Green-Refactor: run the focused
regression before production changes and record the expected failure; make the
smallest fix; rerun focused tests; then refactor and rerun focused plus wider
checks. Keep the red interval inside its milestone and commit only after the
milestone is green. Run format, lint, and test gates sequentially. No Verus
command is planned because no formal proof milestone is selected.

## Validation and acceptance

A real `AgentContext` command tree plus consumer-supplied claims must show a
valid reference resolves; an unknown path and a flag absent on the exact
command produce findings; normalized example names resolve against the owning
reference and effective options, while values are preserved and checked only by
applicable 7.1 value-sensitive policy rules; policy applies canonical
vocabulary/global-option rules. Structural findings remain visible in all
policy modes. Exceptions cannot authorize absent paths or flags. A prose-only
skill succeeds with zero checked references and examples.

Use `rstest` for finite cases, `googletest` and `pretty_assertions` for clear
assertions, and `proptest` for generated lists and normalization-preservation
relations. Use `rstest-bdd` for the feature in EP-M3. Add end-to-end coverage
for the consumer adapter and actual validator API. Use `insta` where stable,
multivariant serialized output is part of the approved report contract. Kani is
unnecessary: there is no bounded machine/state invariant. The independent
oracle and negative controls explain why the property suite detects wrong
membership behaviour; generator cases must cover empty, matching, missing,
nested, and reordered values without excessive filtering.

Red: focused unknown-path test fails because no finding is produced. Green: the
same test passes after structural resolution. Refactor: focused tests, BDD/E2E,
property tests, and sequential repository gates still pass. Record actual test
names, counts, and output in this section during implementation.

## Idempotence and recovery

The validator is pure and can be rerun safely. Fixture extraction reads only
repository-controlled fixtures. Do not run it against arbitrary manifest paths.
On a gate failure, retain the log, repair the smallest affected milestone, and
rerun that gate followed by dependent gates sequentially. If a prerequisite or
design assumption fails, mark the plan `BLOCKED` and preserve the last
validated plateau.

## Artefacts and notes

The report shape must follow the completed policy/report contract. It should
include stable finding code/category, manifest ID, source ordinal, exact
command path, and option/example identity. It must omit raw paths and argv
values. Coverage counts deterministically distinguish claims seen from claims
resolved. Capture concise positive and negative report examples during
implementation; do not freeze output shape before EP-M0.

Illustrative finding fields (not a wire-schema commitment):

```json
{
  "category": "structural",
  "code": "skill.unknown_flag",
  "manifest_id": "operation-skill",
  "source_ordinal": 2,
  "command_path": ["tool", "records", "get"],
  "option": "format"
}
```

A report with unsupported examples must expose a partial-validation status and
separate unchecked counts; absence of findings alone is not a complete
validation verdict. A prose-only manifest with no supplied examples remains a
valid zero-coverage case. Duplicate manifest IDs prevent examples referring to
that ID from choosing an arbitrary owner; report the ambiguity and skip those
dependent checks.

Report order is stable by manifest input order, source ordinal, then finding
code. Do not include manifest filesystem paths or example argument values.

## Interfaces and dependencies

Proposed module: `cargo-orthohelp/src/skill_validation/`. Candidate entry
point, subject to EP-M0 API reconciliation:

```rust
pub fn validate(
    input: &SkillValidationInput,
    commands: &EffectiveCommandSurface,
    policy: &ResolvedAgentPolicy,
) -> Result<SkillValidationReport, CommandSurfaceError>;
```

`SkillValidationInput` carries manifest IDs, exact `SkillCommandRef` values,
and normalized examples. Each normalized example carries manifest ID,
`command_reference_ordinal`, path, option uses (name, value presence, values),
positionals, example source ordinal, and option source ordinals.
`EffectiveCommandSurface` explicitly associates each path with the effective
inputs resolved by its consumer or bridge adapter. It rejects duplicate paths
and ambiguous option metadata. It does not infer inheritance. The completed
7.1.3 policy/glossary supplies canonical vocabulary and global-option rules;
`ResolvedAgentPolicy` contains the exact mode, rule levels, and exceptions.
Results separate structural from policy findings. Names and visibility may
change during approved reconciliation; ownership, scoping, and non-I/O
boundaries may not.

No runtime dependency is proposed. Verify existing workspace test dependencies
at EP-M1; add the authorized `googletest` and `pretty_assertions` as
dev-dependencies if missing, using caret requirements. `rstest-bdd` is already
used and follows the repository's existing runner. No proof tool or runtime
crate is added.

## Documentation and skill signposts

Consult/update the relevant portions of `docs/design.md`,
`docs/cli-localization-design.md`, `docs/cargo-orthohelp-design.md`,
`docs/clap-dispatch-and-ortho-config-integration.md`,
`docs/agent-native-cli-design.md`, `docs/improved-error-message-design.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/localizable-rust-libraries-with-fluent.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`, and
`docs/rstest-bdd-users-guide.md`. The request's `docs/rtest-bdd-users-guide.md`
spelling is not present; the correctly named repository guide is
`docs/rstest-bdd-users-guide.md`. `docs/scripting-standards.md` is absent and
no script is proposed.

Read `$execplans`, `$rust-router`, `$rust-verification`, `$rust-unit-testing`,
`$proptest`, and `$domain-cli-and-daemons` before their implementation facets.
Use `$codegraph-mcp` for structural navigation and `$firecrawl-mcp` for
external prior art. The Agent Skills source above is prior art only, not an
OrthoConfig format mandate.

## Revision note

- 2026-10-14: Initial DRAFT from roadmap, current context/policy code, Wyvern
  reconnaissance, Firecrawl prior art, and architecture/contracts/operations
  review. No implementation has started; explicit approval and EP-M0 remain
  mandatory.
- 2026-10-14: Reconciled final expert findings, specified fixture extraction
  and partial-validation reporting, and recorded passing planning gates.
  Implementation scope and prerequisite/approval gates remain unchanged.

- 2026-10-14: Reworded two policy-exception statements after the publication
  spelling check rejected their wording. Meaning and remaining work are
  unchanged.

[adr-008]: ../adr-008-opt-in-agent-native-policy-configuration.md
