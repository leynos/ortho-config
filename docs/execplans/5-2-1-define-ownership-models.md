# Define schema ownership for documentation IR, agent context, and policy reports

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This plan covers roadmap item 5.2.1 only. It was approved for implementation on
2026-05-20 when the maintainer asked Codex to proceed with the planned
functionality.

## Purpose / big picture

Roadmap phase 5 reconciles the design baseline before OrthoConfig adds new
agent-native features. Item 5.2.1 defines which contract owns three related but
different schemas:

- localized documentation intermediate representation (IR);
- compact agent-context JSON for command invocation;
- policy reports emitted by `cargo-orthohelp`.

After this plan is approved and implemented, a maintainer should be able to read
`docs/agent-native-cli-design.md`, `docs/cargo-orthohelp-design.md`,
`docs/users-guide.md`, `docs/developers-guide.md`, and any new Architecture
Decision Record (ADR) and tell exactly which crate owns each schema, how each
schema is versioned, and which future roadmap items are allowed to implement
code against those contracts.

The observable success criteria are:

- localized documentation IR remains owned by `OrthoConfigDocs` and
  `ortho_config::docs::DocMetadata`;
- agent context is specified as a compact sibling schema with its own version,
  not as localized documentation prose;
- policy reports are specified as a `cargo-orthohelp` reporting contract for
  warnings and hard failures;
- users and downstream maintainers can see the boundary without reading source
  code; and
- future implementation work has concrete tests, behavioural scenarios,
  compatibility rules, and command gates.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not workarounds.

- Do not implement the schema types, command-line flags, transforms, policy
  lints, or generated outputs until this ExecPlan is explicitly approved.
- Keep roadmap item 5.2.1 focused on ownership and contract definition. Move
  actual agent-context generation to phase 6 and policy lint execution to phase
  7 unless later approval expands this item.
- Keep localized documentation IR in the existing `OrthoConfigDocs` contract.
  Do not replace `OrthoConfigDocs::get_doc_metadata()` or rename `DocMetadata`
  as part of this item.
- Preserve existing `cargo-orthohelp --format ir`, `--format man`,
  `--format ps`, and `--format all` behaviour unless a later approved migration
  step explicitly changes them.
- Protect the hexagonal boundary: `ortho_config` owns reusable metadata
  contracts, while `cargo-orthohelp` owns bridge execution, rendering,
  command-line reporting, and generated artefact writing.
- Keep domain and contract types independent of process I/O, filesystem
  writing, Cargo metadata loading, and renderer details.
- Do not let localized long prose, Fluent message identifiers, roff output, or
  PowerShell help structures become the agent-context source of truth.
- Give documentation IR, agent context, and policy reports independent schema
  versions. A compatible change in one schema must not force a version bump in
  the others, unless the same machine contract changes.
- Use en-GB-oxendict spelling and grammar in documentation, except for external
  API names such as `color`, `JSON Schema`, and source code identifiers.
- Follow `docs/documentation-style-guide.md`, including ADR structure,
  Markdown wrapping, fenced code block languages, and link style.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests where
  implementation changes require tests.
- Add end-to-end tests when implementation changes an externally observable
  workflow, integration contract, persisted output, command-line behaviour, or
  generated schema file.
- Do not add Kani, Verus, property-test tooling, or any new dependency unless a
  later approved implementation step introduces a real invariant over a broad
  input, state, ordering, or transition space that deterministic tests cannot
  cover.
- Run validation commands sequentially and capture output with `tee` into
  `/tmp` log files. Do not run format checks, lints, and tests in parallel.
- Do not add a new public API surface without documenting its versioning,
  compatibility, and defaulting rules.
- Do not mark roadmap item 5.2.1 complete until the approved implementation,
  documentation updates, validation gates, CodeRabbit review, commit, push, and
  pull-request updates are complete.

If satisfying the objective requires violating a constraint, stop, document the
conflict in `Decision Log`, and ask for direction.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Approval: stop after drafting this plan and wait for explicit approval before
  implementation.
- Scope: during implementation, stop if the work requires more than 14 files or
  more than 650 net lines of code and documentation, excluding generated
  snapshot or golden-output churn that is reviewed separately.
- Public API: stop if an existing public type, trait, constant, or command flag
  must be renamed or removed.
- Schema shape: stop if two plausible top-level schema shapes would materially
  affect downstream compatibility. Present the alternatives with trade-offs.
- Dependencies: stop if any new crate, tool, or non-standard Cargo feature is
  needed.
- Proof tooling: stop if a proposed Kani, Verus, or property-test addition
  would add tooling without a clearly stated non-trivial invariant.
- Tests: stop if `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, or `make nixie` still fails after two focused fix
  attempts.
- Documentation: stop if `docs/agent-native-cli-design.md`,
  `docs/cargo-orthohelp-design.md`, and `docs/users-guide.md` cannot describe
  the same ownership model without contradiction.
- Process: stop if branch tracking, push, draft pull request creation, or
  `coderabbit review --agent` fails in a way that might hide review feedback.

## Risks

Known uncertainties that might affect the plan. Each risk records severity,
likelihood, and mitigation.

- Risk: agent-context ownership may be ambiguous because the schema is useful
  to downstream applications, while generation initially happens in
  `cargo-orthohelp`. Severity: high. Likelihood: medium. Mitigation: record the
  approved split explicitly. The recommended default is that `ortho_config`
  owns reusable contract types and `cargo-orthohelp` owns the concrete bridge
  transform and command output.
- Risk: policy reports could become a generic static-analysis format instead
  of a narrow `cargo-orthohelp` contract. Severity: medium. Likelihood: medium.
  Mitigation: use Static Analysis Results Interchange Format (SARIF) as prior
  art for rule identifiers, severities, locations, and messages, but do not
  adopt SARIF wholesale without a separate decision.
- Risk: localized documentation metadata may be tempting to reuse directly for
  agent context. Severity: high. Likelihood: high. Mitigation: require tests
  and docs that prove localized long prose is excluded from compact context
  output unless a short command-selection summary is explicitly declared.
- Risk: existing local schema duplication in `cargo-orthohelp/src/schema` may
  expand into duplicated ownership for future schemas. Severity: medium.
  Likelihood: high. Mitigation: plan a deliberate local mirror only where the
  tool needs to parse unpublished crate output, and test version alignment.
- Risk: roadmap items 6.2 and 7.1 through 7.3 may appear to belong to this
  item because they mention agent context and policy. Severity: medium.
  Likelihood: high. Mitigation: keep this item to schema ownership and contract
  definition, with implementation sequencing recorded for later roadmap items.
- Risk: `markdownlint` may report pre-existing repository-wide line-length
  issues. Severity: low. Likelihood: medium. Mitigation: keep edited sections
  compliant and record any unrelated failures with exact line references before
  escalating.
- Risk: `leta` may not provide Rust symbols if `rust-analyzer` fails to start.
  Severity: low. Likelihood: medium. Mitigation: use `leta` where available,
  fall back to `rg` and direct file inspection, and record the limitation in
  `Surprises & Discoveries`.

## Skills and source signposts

The implementation must use these skills and documents deliberately:

- `leta`: use for Rust symbol navigation where the language server works.
- `rust-router`: route Rust-specific implementation questions.
- `arch-crate-design`: decide crate and module ownership.
- `rust-types-and-apis`: design schema types, version constants, newtypes, and
  public API boundaries.
- `rust-errors`: design policy-report error and failure classification.
- `domain-cli-and-daemons`: keep `cargo-orthohelp` stdout, stderr, exit codes,
  and machine-readable output stable.
- `hexagonal-architecture`: protect domain/policy logic from bridge, renderer,
  filesystem, and command-line adapters.
- `en-gb-oxendict-style`: keep documentation spelling and grammar consistent.

The implementation must review and keep aligned with:

- `docs/roadmap.md`, especially item 5.2.1 and later items 6.2 and 7.1;
- `docs/design.md`;
- `docs/agent-native-cli-design.md`;
- `docs/cargo-orthohelp-design.md`;
- `docs/users-guide.md`;
- `docs/developers-guide.md`;
- `docs/documentation-style-guide.md`;
- `docs/rust-testing-with-rstest-fixtures.md`;
- `docs/rust-doctest-dry-guide.md`;
- `docs/reliable-testing-in-rust-via-dependency-injection.md`;
- `docs/localizable-rust-libraries-with-fluent.md`;
- `docs/complexity-antipatterns-and-refactoring-strategies.md`; and
- `docs/rstest-bdd-users-guide.md`.

External prior art checked during planning:

- JSON Schema Draft 2020-12 provides the current metaschema and validation
  vocabulary for documenting JSON contracts.
- Model Context Protocol tool definitions use compact tool descriptions with
  `inputSchema`, optional `outputSchema`, and structured results.
- SARIF 2.1.0 is an OASIS JSON-based static-analysis results format with runs,
  tools, rules, results, severities, messages, and locations.

These sources inform the schema shape. They do not override repository design
documents or require adopting an external format wholesale.

## Repository orientation

The existing documentation IR lives in `ortho_config::docs`.
`ortho_config/src/docs/mod.rs` exports `ORTHO_DOCS_IR_VERSION`,
`OrthoConfigDocs`, and `DocMetadata`. The derive macro implements
`OrthoConfigDocs::get_doc_metadata()` for consumer configuration types.

`cargo-orthohelp` has a local copy of the documentation IR schema in
`cargo-orthohelp/src/schema/mod.rs`. The local copy lets the tool parse bridge
JSON while the workspace and publishable crates evolve. The current tests in
`cargo-orthohelp/src/schema/tests.rs` prove the local schema version matches
`ortho_config::docs` and that sample metadata round-trips through JSON.

`cargo-orthohelp/src/main.rs` currently parses CLI arguments, loads Cargo
metadata, resolves locales, builds or loads bridge IR, deserializes
`DocMetadata`, localizes docs, and writes requested outputs. The current
`OutputFormat` enum in `cargo-orthohelp/src/cli.rs` supports `ir`, `man`, `ps`,
and `all`; future roadmap work adds agent-context and policy-related outputs
after ownership is approved.

## Recommended ownership model

The implementation should record these decisions, subject to plan approval.

First, localized documentation IR remains in `ortho_config::docs` and the public
`OrthoConfigDocs` contract. `DocMetadata.ir_version` continues to govern
human-documentation IR compatibility. Add only metadata that human
documentation needs or metadata that is genuinely shared by both human docs and
agent context.

Second, compact agent context is a sibling contract. The preferred ownership is
for reusable schema types and the `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` constant
to live in `ortho_config`, while the concrete bridge transform initially lives
in `cargo-orthohelp`. This keeps the contract reusable by downstream
applications without making `ortho_config` depend on Cargo metadata, local file
writing, renderer code, or process I/O.

Third, `cargo-orthohelp` owns policy report emission because it is the
reference CLI and the planned `--check-agent-native` command surface. The
policy report schema should have its own version constant, such as
`ORTHO_POLICY_REPORT_SCHEMA_VERSION`, and must include stable fields for the
tool, mode, results, rule identifier, machine code, severity level, message,
and optional source location. If later work needs downstream libraries to
create the same report values, extract only the reusable report model into
`ortho_config` or a lower shared crate with explicit approval.

Fourth, all transforms should point inward. Domain-like contract mapping should
operate on structured `DocMetadata` and schema types. Adapters should handle
Cargo metadata, bridge execution, filesystem writes, stdout, stderr, and
process exit codes.

## Planned implementation milestones

### Milestone 1: approve and freeze ownership documents

Review this ExecPlan. If it is accepted, change `Status` to `APPROVED` and
record the approval date in `Progress`. Do not skip this step.

Create or update a design decision record if the approved ownership split is
substantive. The likely file is:

```plaintext
docs/adr-003-define-schema-ownership-for-agent-native-contracts.md
```

The ADR should use `docs/documentation-style-guide.md` and record:

- documentation IR stays in `ortho_config::docs`;
- agent context is a compact sibling schema with independent versioning;
- policy reports are initially owned by `cargo-orthohelp`;
- `cargo-orthohelp` adapters transform and emit outputs without owning the
  reusable metadata contract; and
- SARIF, JSON Schema, and Model Context Protocol are prior art, not mandatory
  compatibility targets.

Update `docs/agent-native-cli-design.md` and `docs/cargo-orthohelp-design.md`
so they state the same ownership model in one place each. Update
`docs/users-guide.md` for consumer-visible behaviour and
`docs/developers-guide.md` for internal practices. Update `docs/contents.md` if
a new ADR is added.

Run:

```sh
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-5-2-1-define-ownership-models.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-5-2-1-define-ownership-models.out
```

Expected result: both commands exit successfully. If they fail, fix edited
documentation first. If they fail only on unrelated pre-existing lines, record
the exact failure in `Surprises & Discoveries` and ask whether to expand scope.

Run `coderabbit review --agent` after this milestone. Clear all concerns before
moving to schema implementation.

### Milestone 2: introduce schema types behind the approved boundary

Add the smallest useful schema modules after approval. The expected shape is:

- documentation IR remains in `ortho_config/src/docs`;
- agent-context schema types live in a sibling module such as
  `ortho_config/src/agent_context`;
- `ortho_config/src/lib.rs` re-exports only the public agent-context types that
  downstream consumers need;
- `cargo-orthohelp` mirrors schema types only where it must parse bridge output
  or emit a tool-owned report; and
- policy report types live in `cargo-orthohelp/src/policy` or
  `cargo-orthohelp/src/report` until reusable-library ownership is approved.

Each new Rust module must start with a `//!` module comment. Public APIs must
have Rustdoc comments with examples where useful. Keep each file under 400
lines and split by cohesive feature if needed.

Initial agent-context schema version should be independent of
`ORTHO_DOCS_IR_VERSION`, for example:

```rust
pub const ORTHO_AGENT_CONTEXT_SCHEMA_VERSION: &str = "1";
```

Initial policy-report schema version should also be independent:

```rust
pub const ORTHO_POLICY_REPORT_SCHEMA_VERSION: &str = "1";
```

Do not add command-line flags in this milestone unless approval explicitly
combines schema definition with output implementation.

### Milestone 3: add unit tests for contract invariants

Use `rstest` for schema unit tests. Extend the current pattern in
`cargo-orthohelp/src/schema/tests.rs` rather than inventing a separate style.

Unit tests should prove:

- documentation IR versioning remains unchanged when agent-context versioning
  changes;
- agent-context serialization uses the compact top-level shape and independent
  `schema_version`;
- localized documentation fields or Fluent identifiers do not leak into compact
  agent context unless an explicit short summary field exists;
- legacy or absent metadata defaults match the defaults in
  `docs/agent-native-cli-design.md`;
- policy reports serialize and deserialize with stable `version`, `tool`,
  `mode`, `results`, `rule_id`, `code`, `severity`, and `message` fields;
- report severity maps warnings and hard failures without parsing human prose;
  and
- missing required machine-stable fields fail deserialization or validation
  intentionally.

Run targeted tests first, then the full test gate:

```sh
cargo test -p cargo-orthohelp schema 2>&1 | tee /tmp/test-schema-ortho-config-5-2-1-define-ownership-models.out
make test 2>&1 | tee /tmp/test-ortho-config-5-2-1-define-ownership-models.out
```

Expected result: targeted and full tests pass.

### Milestone 4: add behavioural and end-to-end coverage where observable

Use `rstest-bdd` where command behaviour or generated artefacts become
observable. Reuse existing fixture and feature structure in:

```plaintext
cargo-orthohelp/tests/features/orthohelp_ir.feature
cargo-orthohelp/tests/rstest_bdd/behaviour/
```

If this implementation only records schema ownership and adds passive schema
types, behavioural tests may be limited to parseable fixture outputs. If it adds
`--format agent-context`, `--check-agent-native`, `--json`, or output files,
add end-to-end tests that invoke the real binary and assert:

- stdout contains machine-readable JSON only when JSON output is requested;
- human diagnostics stay on stderr;
- generated JSON parses into the intended schema;
- `warn` mode exits successfully while carrying findings;
- `deny` mode exits with the documented failure class; and
- existing `ir`, `man`, `ps`, and `all` output still works.

Run:

```sh
make test 2>&1 | tee /tmp/test-bdd-ortho-config-5-2-1-define-ownership-models.out
```

Expected result: behavioural and full workspace tests pass.

Run `coderabbit review --agent` after schema and behavioural tests are in
place. Clear all concerns before final validation.

### Milestone 5: update guides and roadmap

Update `docs/users-guide.md` so consumers understand that:

- `OrthoConfigDocs` remains the human documentation metadata contract;
- agent context is a separate compact schema;
- policy reports are emitted by `cargo-orthohelp`; and
- existing documentation outputs remain compatible until a migration is
  explicitly approved.

Update `docs/developers-guide.md` so maintainers know:

- where to add future metadata fields;
- where to add schema version constants;
- how to decide whether a field belongs in documentation IR, agent context, or
  policy reports;
- when to add `rstest`, `rstest-bdd`, end-to-end, property, Kani, or Verus
  coverage; and
- how `coderabbit review --agent` gates major milestones.

Update `docs/roadmap.md` only after the approved implementation has landed and
all gates pass. Mark item 5.2.1 and its three child bullets done, leaving later
items 5.2.2, 5.2.3, 6.2, and 7.x open unless they are separately implemented.

Run documentation gates:

```sh
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-5-2-1-define-ownership-models.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-5-2-1-define-ownership-models.out
```

Expected result: both pass.

### Milestone 6: final validation, review, commit, and pull request update

Run all required gates sequentially:

```sh
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-5-2-1-define-ownership-models.out
make lint 2>&1 | tee /tmp/lint-ortho-config-5-2-1-define-ownership-models.out
make test 2>&1 | tee /tmp/test-ortho-config-5-2-1-define-ownership-models.out
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-5-2-1-define-ownership-models.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-5-2-1-define-ownership-models.out
```

Expected result: every command exits successfully. If any gate fails, inspect
the matching `/tmp` log, fix the smallest relevant cause, update this plan, and
rerun the failed gate before rerunning later gates.

Run `coderabbit review --agent` and clear all concerns.

Commit using a file-based commit message. Do not use `git commit -m`. Push the
branch and update the draft pull request summary with:

- roadmap item `(5.2.1)`;
- the ExecPlan link;
- validation evidence;
- CodeRabbit results; and
- the Lody session link.

## Validation plan

The future implementation must validate both the code and the documentation.

Use `rstest` for unit-level schema and conversion checks. Use `rstest-bdd` for
observable command behaviour and generated artefacts. Add end-to-end tests when
a user can observe a new command flag, output file, JSON stream, exit code, or
integration contract.

Do not add Kani, Verus, or property tests for the ownership-only portion. Add
one of them only if implementation introduces a substantive invariant over a
range of inputs, states, orderings, or transitions. Examples that could justify
extra proof later include deterministic schema migration across all version
pairs, exhaustive policy severity classification, or ordering guarantees for
recursive command trees. A test that merely restates a type definition is not
acceptable proof.

The required command gates are:

```sh
make check-fmt
make lint
make test
make markdownlint
make nixie
```

Run all gates sequentially and capture logs with `tee` as shown in the
milestones.

## Progress

Use this list to summarize granular steps. Every stopping point must be
documented here, even if it requires splitting a partially completed task into
two.

- [x] (2026-05-18) Loaded `leta`, `rust-router`, `execplans`,
  `hexagonal-architecture`, and related Rust/API/documentation skills for
  planning.
- [x] (2026-05-18) Created a Leta workspace for this worktree.
- [x] (2026-05-18) Renamed the local branch to
  `5-2-1-define-ownership-models`.
- [x] (2026-05-18) Used Firecrawl to check external prior art for JSON Schema
  Draft 2020-12, Model Context Protocol tool schemas, and SARIF policy-report
  structure.
- [x] (2026-05-18) Created context pack `pk_c4ztjt22` for the Wyvern agent
  team.
- [x] (2026-05-18) Asked a Wyvern agent to review schema ownership,
  hexagonal boundaries, crate/API ownership, and ADR needs.
- [x] (2026-05-18) Asked a Wyvern agent to review validation, behavioural test,
  proof-tooling, documentation, and CodeRabbit obligations.
- [x] (2026-05-18) Drafted this ExecPlan for approval.
- [x] (2026-05-18) Added this ExecPlan to `docs/contents.md` so it is
  discoverable from the documentation index.
- [x] (2026-05-18) Ran validation for the draft-plan milestone:
  `markdownlint-cli2` on the edited Markdown files, `make check-fmt`,
  `make lint`, `make test`, and `make nixie` all passed.
- [x] (2026-05-18) Ran `coderabbit review --agent`, addressed two minor
  documentation findings, and reran it with zero findings.
- [x] (2026-05-20) Received explicit maintainer approval to implement this
  ExecPlan.
- [x] (2026-05-20) Reloaded `leta`, `rust-router`, `execplans`,
  `arch-crate-design`, `rust-types-and-apis`, `rust-errors`,
  `domain-cli-and-daemons`, `hexagonal-architecture`, and `commit-message`
  guidance for implementation.
- [x] (2026-05-20) Froze ownership decisions in design documentation and
  ADR-003.
- [x] (2026-05-20) Added passive `ortho_config::agent_context` schema types
  and `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` without adding generator flags or
  command output.
- [x] (2026-05-20) Added passive `cargo_orthohelp::policy` report schema
  types and `ORTHO_POLICY_REPORT_SCHEMA_VERSION` for tool-owned warnings and
  hard failures.
- [x] (2026-05-20) Added ADR-003 and updated agent-native design,
  `cargo-orthohelp` design, user's guide, developer's guide, and contents
  documentation with the accepted ownership split.
- [x] (2026-05-20) Ran targeted schema validation:
  `cargo test -p ortho_config agent_context` passed with 7 tests, and
  `cargo test -p cargo-orthohelp policy` passed with 9 library tests.
- [x] (2026-05-20) Ran implementation CodeRabbit review; fixed its one
  trivial ADR wording finding.
- [x] (2026-05-23) Merged pull request #325 with the schema ownership
  implementation, including the final CodeRabbit-cleared review state recorded
  in that pull request.
- [x] (2026-05-23) Validated schema types and version constants within the
  approved boundaries before merge.
- [x] (2026-05-20) Re-ran commit gates after lint-driven test fixes:
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all passed.
- [x] (2026-05-21) Used Wyvern agents to verify current schema findings and a
  scribe agent to verify documentation findings before applying minimal fixes.
- [x] (2026-05-21) Removed optional deserialization for
  `PolicyReport.summary`, added agent-context `kind`, async submission,
  delivery route, and canonical mutation-effect wire values.
- [x] (2026-05-22) Added inline `insta` snapshot assertions for complete
  serialized `PolicyReport` and `AgentContext` JSON wire contracts.
- [x] (2026-05-23) Added `rstest` coverage for the passive schema contracts and
  confirmed that `rstest-bdd` and end-to-end coverage are not applicable until
  later roadmap items add command output surfaces.
- [x] (2026-05-22) Updated user, developer, and roadmap documentation.
- [x] (2026-05-23) Ran required gates, completed review, committed, pushed, and
  merged pull request #325.
- [x] (2026-05-25) Revalidated completion after commit #327 regressed the
  roadmap checkboxes for item 5.2.1, then restored the completed roadmap state.

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as risks.
Document with evidence so future work benefits.

- Observation: `leta workspace add` succeeded, but Rust symbol queries failed
  because `rust-analyzer` closed the Language Server Protocol (LSP) connection.
  Impact: planning used `leta` where possible, then fell back to direct file
  inspection and `rg` for Rust context.
- Observation: `cargo-orthohelp` already mirrors the documentation IR schema
  locally and has tests proving version alignment and JSON round-trip
  compatibility. Impact: future schema work should extend that pattern instead
  of inventing a separate compatibility strategy.
- Observation: existing design documents already describe documentation IR and
  agent context as sibling outputs. Impact: the implementation should sharpen
  and formalize that ownership rather than reversing the design direction.
- Observation: SARIF is useful prior art for static-analysis-style reports,
  but adopting it wholesale would add compatibility expectations beyond the
  roadmap item. Impact: use its concepts only where they improve the
  `cargo-orthohelp` policy report contract.
- Observation: Repository-wide `make fmt` still reaches pre-existing Markdown
  line-length failures in unrelated documents after its formatting steps.
  Impact: this branch validated the edited Markdown files directly and left
  unrelated repository-wide Markdown debt untouched.
- Observation: `leta` still registers the workspace, but `rust-analyzer` closes
  the LSP connection during Rust symbol searches on 2026-05-20. Impact:
  implementation uses targeted file reads and `rg` after recording the tool
  limitation.
- Observation: registering `cargo-orthohelp/src/policy` in both the library
  and binary targets made the passive schema functions dead code in the binary
  target. Impact: policy remains a library module until a future CLI milestone
  wires it into command execution.
- Observation: the first implementation CodeRabbit review returned one
  trivial ADR wording finding, which was fixed. Two follow-up attempts then
  returned recoverable rate-limit responses with multi-minute waits. Impact:
  implementation is paused at the review gate until CodeRabbit can confirm the
  concern is cleared.
- Observation: a third follow-up CodeRabbit attempt returned another
  recoverable rate-limit response after waiting the requested interval. Impact:
  the implementation paused at the review gate until pull request #325 could
  carry the final review and merge evidence.
- Observation: `make lint` caught that `SupportDeclaration` could derive
  `Default` instead of using a manual implementation. Impact: the type now
  derives `Default`, preserving the documented `supported: false` behaviour
  while satisfying Clippy.
- Observation: `make lint` also enforces `indexing_slicing` in tests. Impact:
  JSON assertion helpers now use `.get(...)` and `.first()` with explicit
  failure diagnostics instead of indexing serialized values directly.
- Observation: the 2026-05-21 verification pass found four still-valid schema
  issues: optional policy-summary deserialization, missing agent-context
  `kind`, missing async and delivery command metadata, and non-canonical
  mutation-effect wire values. Impact: these are fixed as schema contract
  corrections without adding CLI output surfaces.

## Decision Log

Record every significant decision made while working on the plan. Include
decisions to escalate, decisions on ambiguous requirements, and design choices.

- Decision: Treat this branch as a pre-implementation ExecPlan branch.
  Rationale: the user explicitly required plan approval before implementation.
- Decision: Recommend `ortho_config::docs` as the continuing owner of localized
  documentation IR. Rationale: this preserves the existing public
  `OrthoConfigDocs` contract and avoids breaking current `cargo-orthohelp`
  output formats.
- Decision: Recommend agent context as a compact sibling schema with its own
  version. Rationale: the design requires machine-oriented invocation context
  without localized prose or renderer-specific output.
- Decision: Recommend initial policy-report ownership in `cargo-orthohelp`.
  Rationale: the planned report is emitted by `cargo-orthohelp` policy checks,
  and command/reporting concerns belong at the CLI adapter boundary.
- Decision: Do not require Kani, Verus, or property-test tooling for the
  ownership-only plan. Rationale: no broad invariant is introduced until
  implementation defines concrete transforms or migration semantics.
- Decision: Do not mark roadmap item 5.2.1 done in this pre-implementation
  branch. Rationale: the plan still requires explicit approval before the
  implementation can complete the roadmap item.
- Decision: Treat the 2026-05-20 request to proceed as explicit approval for
  implementation. Rationale: the maintainer named this ExecPlan and instructed
  Codex to implement the planned functionality while keeping the plan current.
- Decision: Keep this item to passive schemas and documentation, with no new
  `cargo-orthohelp` flags or generated outputs. Rationale: the approved plan
  keeps agent-context generation in phase 6 and policy lint execution in phase
  7 unless explicitly expanded.
- Decision: Stop at the CodeRabbit milestone gate after repeated recoverable
  rate-limit failures. Rationale: the plan and maintainer instructions require
  CodeRabbit concerns to be cleared before moving to the next milestone, and
  repeated rate limits can hide fresh review feedback.
- Decision: Skip new behavioural or end-to-end tests for the 2026-05-21
  comment pass. Rationale: no `cargo-orthohelp` agent-context or policy-report
  command output exists in this phase, so there is no externally observable
  workflow to exercise yet.
- Decision: Treat pull request #325 as the completion evidence for roadmap item
  5.2.1. Rationale: it merged the passive schema contracts, ADR, guide updates,
  validation evidence, and roadmap completion update before later roadmap edits
  accidentally restored unchecked boxes.

## Outcomes & Retrospective

Roadmap item 5.2.1 is complete. The accepted ownership decisions keep localized
documentation IR in `ortho_config::docs`, compact agent context in
`ortho_config::agent_context`, and policy reports in `cargo_orthohelp::policy`.

The implementation added `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` and
`ORTHO_POLICY_REPORT_SCHEMA_VERSION` beside the existing
`ORTHO_DOCS_IR_VERSION`. Unit tests and inline `insta` snapshots cover the
passive JSON contracts. Behavioural and end-to-end coverage remain scoped to
later roadmap items because this item deliberately added no new command output
surface.

ADR-003 records the ownership split. The user guide, developer guide,
agent-native design, `cargo-orthohelp` design, documentation contents, and
roadmap were updated. Pull request #325 merged the implementation after the
required gates and review, and the 2026-05-25 validation restored roadmap item
5.2.1 after a later roadmap edit regressed its checkbox state.
