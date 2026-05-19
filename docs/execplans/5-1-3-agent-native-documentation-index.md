# Add the agent-native documentation index

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This plan covers roadmap item 5.1.3 only. It was explicitly approved on
2026-05-20 before implementation began.

## Purpose / big picture

OrthoConfig already has the main design document, the `cargo-orthohelp`
intermediate representation (IR) design, the users guide, and the agent-native
CLI design. These documents now need a trustworthy index that helps readers
find the canonical agent-native boundary without mistaking future agent-native
work for implemented runtime behaviour.

After the approved implementation completes, a maintainer can verify success by
reading `docs/design.md`, `docs/cargo-orthohelp-design.md`,
`docs/users-guide.md`, `docs/agent-native-cli-design.md`, `docs/contents.md`,
and `docs/roadmap.md`. These files should agree that:

- `docs/agent-native-cli-design.md` is the canonical agent-native contract
  document;
- the human documentation IR and compact agent-context schema are sibling
  outputs with independent versioning;
- OrthoConfig models, generates, serializes, and lints reusable command
  contracts; and
- downstream applications such as Weaver and Netsuke own command execution,
  side effects, and domain-specific command-runner semantics.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not workarounds.

- Do not implement phase 6 or phase 7 agent-native functionality while
  completing roadmap item 5.1.3. This phase repairs documentation truth and
  indexability only.
- Keep `docs/agent-native-cli-design.md` as the canonical source for
  agent-native scope, contract surfaces, and the OrthoConfig-versus-consumer
  boundary.
- Keep `docs/design.md`, `docs/cargo-orthohelp-design.md`, and
  `docs/users-guide.md` as cross-linking entrypoints. They should not duplicate
  long sections of the agent-native design.
- Preserve the existing documentation IR contract. If implementation appears
  to require changing `DocMetadata`, `OrthoConfigDocs`, `cargo-orthohelp`
  output, or a public schema, stop and ask for approval.
- Preserve public API compatibility. If a public Rust API signature must
  change, stop and ask for approval.
- Use en-GB-oxendict spelling and grammar in documentation and comments,
  except for external API names such as `color`.
- Follow `docs/documentation-style-guide.md`, including fenced code block
  language identifiers and 80-column wrapping for Markdown prose.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests where
  tests are needed to validate code or externally observable command behaviour.
  Do not add vacuous tests for a documentation-only implementation.
- Protect the architecture boundary in the spirit of hexagonal architecture:
  reusable command-contract policy belongs in OrthoConfig's domain and
  application surfaces, while downstream execution engines remain outside the
  OrthoConfig boundary.
- Do not add new dependencies without explicit approval.
- Run required gates sequentially, not in parallel, and capture output with
  `tee` into `/tmp` log files.
- Commit only after the relevant gates for the change have passed or after an
  approved exception has been recorded.

If satisfying the objective requires violating a constraint, stop, document the
conflict in `Decision Log`, and ask for direction.

## Tolerances

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Scope: stop if implementation requires changes to more than 8 files or more
  than 300 net lines, excluding this ExecPlan's living updates.
- Interface: stop if the implementation requires adding, removing, or renaming
  a public Rust API, schema field, command-line option, or generated output
  format.
- Dependencies: stop if any new crate, binary tool, or network service
  dependency is required.
- Tests: stop if `make check-fmt`, `make lint`, or `make test` still fails
  after two focused fix attempts.
- Documentation: stop if the docs cannot state the boundary consistently
  without also changing planned agent-native implementation scope.
- Ambiguity: stop if "documentation index" could validly mean both a prose
  navigation section and a new machine-readable generated index in a way that
  changes implementation or test scope.
- Process: stop if branch tracking, push, or draft pull request creation fails
  because the remote branch or pull request already exists in an incompatible
  state.

## Risks

Known uncertainties that might affect the plan. Each risk records severity,
likelihood, and mitigation.

- Risk: The documentation already contains overlapping agent-native boundary
  text, and careless edits could create inconsistent terminology. Severity:
  medium. Likelihood: high. Mitigation: make `docs/agent-native-cli-design.md`
  canonical, then add short pointers from the other documents.
- Risk: The users guide may become too implementation-heavy if it repeats
  maintainer-facing schema and roadmap details. Severity: medium. Likelihood:
  medium. Mitigation: keep `docs/users-guide.md` focused on what a library
  consumer can rely on and link to design documents for rationale.
- Risk: Updating the roadmap to done before the implementation lands would
  create a stale completion claim. Severity: high. Likelihood: low. Mitigation:
  mark roadmap item 5.1.3 complete only during the approved implementation, not
  during this pre-implementation plan.
- Risk: Test requirements may be over-applied to a documentation-only change.
  Severity: low. Likelihood: medium. Mitigation: add Rust or behavioural tests
  only if approved implementation changes code, generated schemas, generated
  documentation output, command-line behaviour, or another externally
  observable contract.
- Risk: `make fmt` may reflow unrelated Markdown or expose existing
  repository-wide formatting debt. Severity: medium. Likelihood: medium.
  Mitigation: inspect formatter changes before committing and escalate if the
  formatter touches unrelated documents in a way that exceeds scope.
- Risk: The language server may be unavailable after the Leta workspace is
  created. Severity: low. Likelihood: medium. Mitigation: use Leta where it
  works for code navigation, then fall back to `rg` and direct file inspection
  for Markdown and configuration files.

## Progress

Use this list to summarize granular steps. Every stopping point must be
documented here, even if it requires splitting a partially completed task into
two.

- [x] (2026-05-18) Loaded the `leta`, `rust-router`, `execplans`,
  `firecrawl-mcp`, `hexagonal-architecture`, `commit-message`, `pr-creation`,
  and `en-gb-oxendict-style` skills for the planning task.
- [x] (2026-05-18) Added the repository as a Leta workspace with
  `leta workspace add`.
- [x] (2026-05-18) Verified the original branch was not a main branch and
  renamed it to `5-1-3-agent-native-documentation-index` before making plan
  changes.
- [x] (2026-05-18) Created context pack `pk_gilim3gg` with the roadmap and
  agent-native boundary excerpts for the Wyvern planning team.
- [x] (2026-05-18) Used two Wyvern agents for read-only planning review: one
  focused on documentation touchpoints, and one focused on testing,
  architecture boundaries, and validation gates.
- [x] (2026-05-18) Used Firecrawl for prior-art checks covering Model Context
  Protocol (MCP) documentation indexes, tool schemas, structured output, and
  JSON Schema identifier/versioning practice.
- [x] (2026-05-18) Drafted this pre-implementation ExecPlan for review.
- [x] (2026-05-18) Ran targeted Markdown linting for this ExecPlan; it passed.
- [x] (2026-05-18) Ran repository validation for the plan commit:
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` passed.
- [x] (2026-05-20) Reconfirmed current truth before implementation:
  `docs/roadmap.md` still listed item 5.1.3 as open, `docs/contents.md`
  remained the documentation index, and `docs/agent-native-cli-design.md`
  remained the canonical boundary document.
- [x] (2026-05-20) Updated the documentation index, design documents, user
  guide, and roadmap to make the agent-native boundary, sibling-output
  versioning, and command-runner boundary explicit.
- [x] (2026-05-20) Ran `coderabbit review --agent` after the documentation
  milestone; it completed with zero findings.
- [x] (2026-05-20) Ran `make fmt`; it failed on existing repository-wide
  Markdown line-length debt after reflowing many unrelated Markdown files.
  Restored unrelated formatter churn and kept only scoped documentation
  changes.
- [x] (2026-05-20) Ran validation gates for the implementation:
  `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`, and
  `make test` all passed.
- [x] (2026-05-20) Re-ran `coderabbit review --agent` after removing
  formatter-only churn from touched files; it completed with zero findings.

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as risks.
Document with evidence so future work benefits.

- Observation: `docs/contents.md` already acts as the repository-wide
  documentation index and already links the agent-native design beside the
  primary design and `cargo-orthohelp` design. Evidence: `docs/contents.md`
  lists those documents under "Product and architecture". Impact:
  implementation should probably refine discoverability and boundary language
  rather than create a second broad index document.
- Observation: `docs/agent-native-cli-design.md` already states that
  OrthoConfig is not becoming a mandatory application runtime and that
  downstream applications own domain side effects. Evidence: sections 2 and 2.1
  describe the product direction and consumer boundary. Impact: implementation
  should reuse this as the canonical wording.
- Observation: `docs/cargo-orthohelp-design.md` already says agent context is a
  sibling output, not localized documentation prose. Evidence: the opening
  paragraphs and section 6.3.1 state this relationship. Impact: implementation
  should make the independent-versioning point sharper without changing schema
  behaviour.
- Observation: Firecrawl found that the MCP specification exposes a
  documentation index at `https://modelcontextprotocol.io/llms.txt`, describes
  tools as discoverable metadata with `inputSchema` and optional
  `outputSchema`, and distinguishes metadata from actual tool execution.
  Impact: this supports a narrow documentation-index plan and reinforces that
  OrthoConfig should describe invocable contracts without becoming every
  application's executor.
- Observation: Firecrawl found JSON Schema guidance for declaring `$schema`
  and `$id` identifiers, and the JSON Schema core specification defines
  vocabularies and dialects as independently identifiable schema concerns.
  Impact: this supports the plan's independent-versioning language for human
  documentation IR versus compact agent context.
- Observation: The Wyvern agents could not read the created context pack
  directly in their forked sessions, but they still inspected the repository
  files read-only and returned useful planning notes. Impact: context-pack
  exchange was attempted and the plan incorporates the agent findings without
  depending on sub-agent pack access.
- Observation: `make fmt` failed during plan validation because `mdformat-all`
  invokes `markdownlint --fix`, which reports existing repository-wide
  line-length violations in unrelated documents. Evidence: the new ExecPlan
  passes `markdownlint-cli2` directly and `make markdownlint` passes for the
  repository. Impact: the plan commit is validated with the required Rust
  gates, repository Markdown lint, and diagram validation, but the formatter
  debt remains outside this task.
- Observation: The first implementation CodeRabbit review returned zero
  findings for the documentation milestone. Evidence:
  `/tmp/coderabbit-ortho-config-5-1-3-agent-native-documentation-index-milestone1.out`
  ends with `{"type":"complete","status":"review_completed","findings":0}`.
  Impact: no review concerns needed remediation before validation.
- Observation: `make fmt` still fails in the approved implementation for the
  same repository-wide Markdown line-length debt discovered during planning,
  plus one long command line in this ExecPlan that has now been split.
  Evidence: `/tmp/fmt-ortho-config-5-1-3-agent-native-documentation-index.out`
  contains unrelated MD013 reports in files such as `cargo-orthohelp/README.md`
  and historical guides. Impact: the implementation keeps unrelated formatter
  churn out of the commit and relies on `make markdownlint` plus targeted
  Markdown linting for documentation validation.
- Observation: The implementation validation gates passed after unrelated
  formatter churn was removed. Evidence: branch-scoped logs under `/tmp` for
  `markdownlint`, `nixie`, `check-fmt`, `lint`, and `test` all end
  successfully. Impact: the documentation-only implementation is ready to
  commit, with the `make fmt` exception explicitly recorded.
- Observation: The final CodeRabbit review returned zero findings after the
  diff was narrowed back to the intended documentation changes. Evidence:
  `/tmp/coderabbit-ortho-config-5-1-3-agent-native-documentation-index-final.out`
  ends with `{"type":"complete","status":"review_completed","findings":0}`.
  Impact: there are no outstanding review concerns before commit.

## Decision log

Record every significant decision made while working on the plan. Include
decisions to escalate, decisions on ambiguous requirements, and design choices.

- Decision: Treat `docs/agent-native-cli-design.md` as the canonical
  agent-native boundary document. Rationale: it already owns the product
  rationale, contract surfaces, and consumer application boundary, while the
  other target documents have narrower audiences.
- Decision: Treat `docs/contents.md` as the documentation index entrypoint
  unless approved implementation discovers that the roadmap intends a new,
  separate document. Rationale: the documentation style guide defines the
  contents file as the canonical index for the documentation set.
- Decision: Do not add Rust tests for a documentation-only implementation.
  Rationale: tests should prove code or externally observable behaviour. A
  prose-only index update is better validated with Markdown linting, diagram
  validation, and the standard gates.
- Decision: Require `rstest`, `rstest-bdd`, schema, golden, or end-to-end
  tests if approved implementation introduces a generated index field, schema
  version contract, `cargo-orthohelp` output, or CLI-visible behaviour.
  Rationale: those changes affect machine contracts or user-observable
  workflows and need executable regression coverage.
- Decision: Defer property tests, Kani, and Verus unless implementation
  introduces an invariant over generated index ordering, canonicalization,
  schema compatibility, or reachability. Rationale: formal tooling should prove
  substantive invariants, not restate documentation assertions.
- Decision: Keep this implementation documentation-only and do not add Rust,
  behavioural, property, Kani, or Verus tests. Rationale: the approved change
  updates prose and roadmap status only; it does not change code, generated
  output, command-line behaviour, schemas, persistence, network boundaries, or
  user-interface flows.

## Prior art notes

Firecrawl was used to resolve open-source protocol and schema prior-art gaps.
The implementation does not need to adopt these protocols; they inform wording
and risk control only.

- MCP exposes a documentation index and models tools as discoverable metadata
  with schemas and structured results:
  <https://modelcontextprotocol.io/specification/2025-11-25>
- MCP tool metadata distinguishes tool descriptions and schemas from execution
  and safety policy:
  <https://modelcontextprotocol.io/specification/2025-11-25/server/tools>
- JSON Schema guidance recommends declaring `$schema` and `$id` so schemas and
  dialects are identifiable:
  <https://json-schema.org/understanding-json-schema/basics>
- JSON Schema core defines schemas, vocabularies, and dialects as independent
  schema concerns: <https://json-schema.org/draft/2020-12/json-schema-core>

## Implementation plan

This section describes the approved implementation sequence. Do not execute
these milestones until the user explicitly approves this ExecPlan.

### Milestone 1: Reconfirm current truth

Inspect the current documents before editing:

```bash
rg -n \
  "agent-native|agent context|documentation IR|OrthoConfig models" \
  docs
rg -n \
  "command runner|sibling" \
  docs
```

Confirm these facts:

- `docs/roadmap.md` still lists item 5.1.3 as open;
- `docs/contents.md` is still the documentation index;
- `docs/agent-native-cli-design.md` remains the canonical boundary document;
- `docs/design.md`, `docs/cargo-orthohelp-design.md`, and
  `docs/users-guide.md` still link or should link to the agent-native design;
  and
- no approved implementation since this plan has already added the desired
  index text.

If any fact is no longer true, update this ExecPlan before editing other files.

### Milestone 2: Update the documentation index and cross-links

Edit `docs/contents.md` so the "Product and architecture" entry for
`docs/agent-native-cli-design.md` clearly identifies it as the canonical
agent-native contract and boundary document. Keep the entry concise and do not
list the same document twice.

Add or refine short cross-links in:

- `docs/design.md`;
- `docs/cargo-orthohelp-design.md`; and
- `docs/users-guide.md`.

The design and `cargo-orthohelp` documents may use maintainer-facing wording.
The users guide should use consumer-facing wording: it should explain what a
library consumer can rely on and point to the design document for details.

### Milestone 3: State sibling outputs and versioning

In `docs/agent-native-cli-design.md`, ensure the contract surfaces section
states that the documentation IR and agent-context schema are sibling outputs
from the same metadata spine with independent versioning. Use direct language:
the documentation IR remains localized and human-documentation-oriented, while
the agent-context schema is compact, machine-oriented, and versioned
independently.

In `docs/cargo-orthohelp-design.md`, sharpen the existing agent-context
pipeline text so generator compatibility is tied to the relevant schema:
`DocMetadata.ir_version` for human documentation IR and the future
agent-context schema version for agent context. Do not introduce the schema
field itself unless the approved implementation explicitly expands beyond
documentation.

### Milestone 4: State the command-runner boundary

In `docs/agent-native-cli-design.md`, preserve or refine the consumer
application boundary:

```plaintext
OrthoConfig models, generates, serializes, and lints reusable command
contracts. Downstream applications own command execution, side effects,
domain-specific safety policy, and long-running job semantics.
```

Make any corresponding references in `docs/design.md` and `docs/users-guide.md`
short and subordinate to the canonical design text. Avoid wording that implies
OrthoConfig will execute Weaver or Netsuke commands.

### Milestone 5: Decide whether executable tests apply

If the implementation is documentation-only, do not add Rust tests. Record in
this ExecPlan that executable tests are not applicable because no code,
generated output, command behaviour, schema, persistence, network boundary, or
user-interface flow changed.

If implementation introduces a code or schema change, add tests before the
implementation or in the same atomic change:

- use `rstest` unit tests for schema shape, deterministic ordering, version
  fields, or policy helper behaviour;
- use `rstest-bdd` behavioural tests for externally observable
  `cargo-orthohelp` or command-line behaviour;
- add golden or round-trip tests when serialized JSON shape changes; and
- add end-to-end tests when a generated output format or CLI workflow changes.

Use property tests, Kani, or Verus only if implementation introduces a
substantive invariant over a range of inputs, states, orderings, or
transitions. Examples include deterministic index ordering for arbitrary
command graphs, schema compatibility across version transitions, or
reachability-preserving command-tree rewrites.

### Milestone 6: Update the roadmap after implementation

After the documentation changes and any required tests are complete, mark
roadmap item 5.1.3 as done in `docs/roadmap.md`. Mark only this item and its
three child bullets. Do not mark phase 5.1 or neighbouring items complete.

### Milestone 7: Validate and commit

Run formatting and validation sequentially. Use branch-scoped `/tmp` logs:

```bash
set -o pipefail; make fmt 2>&1 | tee /tmp/fmt-ortho-config-5-1-3-agent-native-documentation-index.out
set -o pipefail; make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-5-1-3-agent-native-documentation-index.out
set -o pipefail; make nixie 2>&1 | tee /tmp/nixie-ortho-config-5-1-3-agent-native-documentation-index.out
set -o pipefail; make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-5-1-3-agent-native-documentation-index.out
set -o pipefail; make lint 2>&1 | tee /tmp/lint-ortho-config-5-1-3-agent-native-documentation-index.out
set -o pipefail; make test 2>&1 | tee /tmp/test-ortho-config-5-1-3-agent-native-documentation-index.out
```

If `make fmt` modifies unrelated files, inspect the diff. If unrelated
formatter churn exceeds the tolerances, stop and ask for direction.

Commit the approved implementation with a file-based commit message using
`git commit -F`. Push to `origin/5-1-3-agent-native-documentation-index` and
update the draft pull request.

## Acceptance criteria

The implementation is complete when all of the following are true:

- `docs/contents.md` makes the agent-native design discoverable as the
  canonical agent-native contract and boundary document.
- `docs/design.md`, `docs/cargo-orthohelp-design.md`, and
  `docs/users-guide.md` link to `docs/agent-native-cli-design.md` in the right
  audience context.
- The documentation states that documentation IR and agent-context schema are
  sibling outputs with independent versioning.
- The documentation states that OrthoConfig models, generates, serializes, and
  lints reusable contracts, and that downstream applications own command
  execution and side effects.
- Roadmap item 5.1.3 and its three child bullets are marked complete.
- Any code, schema, generated-output, or CLI-visible change is covered by
  appropriate `rstest`, `rstest-bdd`, golden, or end-to-end tests.
- `make check-fmt`, `make lint`, and `make test` pass.
- Documentation validation with `make markdownlint` passes, and `make nixie`
  passes if diagrams are present or touched.

## Outcomes & retrospective

The approved implementation completed roadmap item 5.1.3 as a
documentation-only change. `docs/contents.md` now identifies
`docs/agent-native-cli-design.md` as the canonical agent-native
command-contract and boundary document. `docs/design.md`,
`docs/cargo-orthohelp-design.md`, and `docs/users-guide.md` now link to that
boundary in the right audience context. `docs/agent-native-cli-design.md` now
states the sibling-output relationship and command-runner boundary directly,
and `docs/cargo-orthohelp-design.md` ties compatibility to
`DocMetadata.ir_version` for human documentation IR and the future
agent-context schema version for agent-facing output.

No Rust, behavioural, property, Kani, or Verus tests were added because no
code, schema, generated output, command-line behaviour, persistence, network
boundary, or user-interface flow changed. Validation passed for
`make markdownlint`, `make nixie`, `make check-fmt`, `make lint`, and
`make test`. `make fmt` still fails on pre-existing repository-wide Markdown
line-length debt after attempting to reflow unrelated files; unrelated
formatter churn was removed from the final diff. CodeRabbit reviewed the
documentation milestone and final cleaned diff with zero findings.
