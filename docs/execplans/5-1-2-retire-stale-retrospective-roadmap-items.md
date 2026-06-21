# Retire stale retrospective roadmap items

This execution plan (ExecPlan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This plan covers roadmap item 5.1.2 only. It was approved for implementation on
2026-05-19.

## Purpose / big picture

OrthoConfig's active roadmap should describe future work, not reopen old
completion claims or force maintainers to infer whether a historical note is
still normative. Roadmap item 5.1.2 repairs that boundary. After the approved
implementation, a maintainer should be able to read `docs/roadmap.md`,
`docs/archive/v0-8-0-roadmap.md`, `docs/ddlint-gap-analysis.md`, and the
historical design notes and know which items are implemented, which are
deferred, which have been superseded by agent-native policy work, and which
notes are preserved only as rationale.

The observable outcome is documentation truth. The active roadmap will contain
only active future work and explicit closed-retrospective references. The
archived v0.8.0 roadmap will remain readable as history, with errata where
later review found a checked item was not actually implemented. The DDLint
gap-analysis document will no longer look like an open loading-work checklist:
it will point readers to the implemented loading features and the agent-native
policy items that replaced the remaining DDLint-specific ideas.

## Repository context

The active roadmap is `docs/roadmap.md`. It already states that completed
v0.8.0-era phases live in `docs/archive/v0-8-0-roadmap.md`, and that the active
roadmap continues numbering from that archive with forward-looking work. Its
phase 5.1 currently has item 5.1.1 checked complete and item 5.1.2 unchecked.

The archived roadmap is `docs/archive/v0-8-0-roadmap.md`. It preserves old
checked-off items and already contains an archive note that the historical
completion state is not a re-audited statement of the current codebase. It also
records the known correction for `OrthoError::MissingRequiredValues`, which is
now phase 7 work despite an old checked entry.

The DDLint gap analysis is `docs/ddlint-gap-analysis.md`. It says it is a
historical analysis and that its original loading gaps have largely been
addressed. It also says the remaining relevance is source material for
agent-native policy, especially the distinction between domain-specific
`--format <compact|json|rich>` options and the cross-CLI `--json` convention.

The primary design document is `docs/design.md`. Section 6 is titled
"Historical implementation baseline" and already says the bootstrap milestones
are retained as historical context, not the current future roadmap. Other
historical notes include `docs/feedback-from-hello-world-example.md` and
`docs/subcommand-refinements.md`; both begin with historical-proposal status
markers.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not improvisation.

- Do not implement new agent-native features in this roadmap item. This item is
  a documentation truth repair.
- Do not move or delete archived roadmap content unless the approved
  implementation explicitly records why a link or short errata note is not
  enough.
- Preserve historical context. Corrections should mark the status of old claims;
  they should not rewrite history to pretend the old roadmap always knew the
  current truth.
- Keep the active roadmap focused on future work. Completed historical
  milestones belong in the archive or in a short background reference.
- Treat DDLint-specific command semantics as prior art unless the active roadmap
  names reusable OrthoConfig work. `ddlint rules`, `ddlint explain`, and
  `--no-ignore` must not become OrthoConfig implementation requirements in this
  phase.
- Keep the OrthoConfig boundary clear: OrthoConfig owns reusable configuration,
  documentation intermediate representation (IR), agent context, and policy
  contracts; downstream applications own their domain execution engines,
  resource semantics, and command-specific payloads.
- Apply the hexagonal architecture rule as a boundary protection tool, not a
  pattern transplant. If code changes become necessary, domain or policy logic
  must not depend on adapter implementation details.
- Use en-GB-oxendict spelling and grammar in new documentation, except for
  external API names such as `color`.
- Follow `docs/documentation-style-guide.md`: ordered heading levels, fenced
  code block language identifiers, paragraph wrapping at 80 columns, table
  captions if tables are added, and clear status labels.
- Update `docs/contents.md` when adding, renaming, or removing documentation.
- Update `docs/users-guide.md` only if the approved implementation changes
  consumer-facing behaviour, application programming interface (API) promises,
  or user-facing compatibility notes. Do not add maintainer-only roadmap
  bookkeeping to the users guide.
- Update `docs/developers-guide.md` only if the approved implementation creates
  or changes an internal convention that maintainers need to follow.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests where code
  behaviour changes. If the approved implementation stays documentation-only,
  do not add vacuous tests.
- Use property tests, Kani, or Verus only if the approved implementation adds a
  real invariant over inputs, states, orderings, transitions, or contractual
  business logic. Documentation status repair alone does not justify a proof.
- Do not add new dependencies without explicit approval.
- Run gates sequentially and capture command output with `tee` into `/tmp`
  logs.
- Do not mark roadmap item 5.1.2 done until the approved implementation has
  landed and the validation gates have passed.

If satisfying the objective requires violating a constraint, stop, document the
conflict in `Decision Log`, and ask for direction.

## Tolerances

These thresholds define when implementation must stop for human direction.

- Scope: stop if implementation requires changes to more than 10 files or more
  than 500 net lines, excluding this ExecPlan's own progress updates.
- Behaviour: stop if any Rust code change appears necessary to make the
  documentation truthful. Record the gap and ask whether to expand the task or
  split a follow-up item.
- API: stop if any public API, command-line behaviour, generated IR schema, or
  documented compatibility contract would change.
- Dependencies: stop if any crate, tool, or workflow dependency must be added.
- Architecture: stop if classifying a historical note requires changing the
  documented responsibility boundary between OrthoConfig and Weaver, Netsuke,
  or other downstream applications.
- DDLint ambiguity: stop if `--no-ignore`, `rules`, or `explain` cannot be
  classified as implemented, deliberately deferred, or replaced by agent-native
  policy using documentation evidence alone.
- Validation: stop if `make check-fmt`, `make lint`, or `make test` still fails
  after two focused fix attempts. For documentation-specific gates, stop if
  `make markdownlint` or `make nixie` fails because of edited lines after two
  focused fix attempts.
- Process: stop if branch push, upstream tracking, or draft pull request
  creation fails because a remote branch or pull request already exists and the
  correct continuation path is ambiguous.

## Risks

- Risk: The archive contains checked items that were accurate at release time
  and checked items later found incomplete. Severity: high. Likelihood: high.
  Mitigation: add explicit status vocabulary and errata instead of treating all
  checkmarks as current truth.
- Risk: Moving completed items too aggressively could make the roadmap harder to
  follow. Severity: medium. Likelihood: medium. Mitigation: keep a compact
  cross-reference from the active roadmap to the archive and only retire stale
  background from the active path.
- Risk: DDLint examples could be mistaken for OrthoConfig product requirements.
  Severity: medium. Likelihood: high. Mitigation: classify loading items as
  implemented, command-shape examples as prior art, and reusable command policy
  as active agent-native work.
- Risk: Historical design notes may mix active guidance with preserved
  rationale. Severity: medium. Likelihood: high. Mitigation: add or normalize
  document-level status markers such as `Status: historical proposal`,
  `Status: proposed`, `Status: adopted`, or `Status: active guidance`.
- Risk: Documentation-only work can tempt meaningless tests. Severity: low.
  Likelihood: medium. Mitigation: validate documentation with Markdown and
  repository gates; add Rust tests only if implementation changes Rust
  behaviour.
- Risk: The repository may have pre-existing Markdown formatting debt. Severity:
  low. Likelihood: medium. Mitigation: record pre-existing failures with log
  paths and fix edited lines rather than expanding the task without approval.

## Prior art and research notes

Firecrawl was used during planning to resolve the status-labelling and
historical-note convention gap. The Architecture Decision Record (ADR)
community guidance describes ADRs as records of a decision with context and
consequences, recommends keeping rationale specific, and says that existing
records should be amended or superseded rather than silently rewritten. This
supports preserving historical roadmap entries while adding errata and status
labels.

The repository's own `docs/documentation-style-guide.md` aligns with that prior
art. It distinguishes design documents, ADRs, and requests for comments (RFCs),
and defines ADR statuses including `Proposed`, `Accepted`, `Superseded`, and
`Deprecated`. The approved implementation should reuse that local convention
instead of inventing a separate status taxonomy where possible.

## Implementation plan

This section describes the approved implementation path. Do not execute it
until this plan is explicitly approved.

### Milestone 1: Audit current truth

Start by confirming that the working tree is clean or contains only expected
plan changes:

```sh
git status --short --branch
```

Read the relevant roadmap and status-bearing documents:

```sh
pattern='historical|retrospective|DDLint|ddlint'
pattern="${pattern}|MissingRequired|Status:|superseded"
pattern="${pattern}|active guidance|preserved rationale"
rg -n "${pattern}" docs
```

Confirm the following baseline before editing:

- `docs/roadmap.md` has 5.1.1 checked and 5.1.2 unchecked.
- `docs/archive/v0-8-0-roadmap.md` preserves completed v0.8.0 entries and
  already corrects the missing-required-values claim.
- `docs/ddlint-gap-analysis.md` marks the DDLint loading checklist as
  historical and lists agent-native next steps.
- `docs/design.md` section 6 marks the bootstrap milestones as historical
  context.
- `docs/feedback-from-hello-world-example.md` and
  `docs/subcommand-refinements.md` already carry historical-proposal markers.

Record any mismatch in `Surprises & Discoveries` before changing files.

### Milestone 2: Normalize roadmap and archive status

Edit `docs/roadmap.md` so item 5.1.2 explains the intended repair in active
roadmap language. The result should say that the implementation will retire
completed historical milestones from the active path, add explicit background
references where useful, and keep active phase 6 onward as future work.

Edit `docs/archive/v0-8-0-roadmap.md` so the archive note is not limited to a
single known missing-required-values correction. Add a small errata or status
legend that tells readers how to interpret archived checkmarks:

- `archived-complete`: completed according to the historical roadmap;
- `corrected`: later review found the archived completion claim was inaccurate;
- `superseded`: later design work replaced the historical item;
- `deferred-active`: follow-up work now lives in the active roadmap.

Use the exact labels only if they read naturally in the edited document. The
important outcome is that a maintainer can distinguish historical completion
state from current implementation truth.

Do not remove the existing detailed v0.8.0 checklist unless approval is
amended. The archive is evidence; the repair is status context.

### Milestone 3: Classify DDLint gap-analysis items

Edit `docs/ddlint-gap-analysis.md` to make each DDLint-derived item's current
status explicit.

Classify these loading items as implemented, with links to current docs where
the behaviour is documented:

- comma-separated arrays and list handling;
- `extends` configuration inheritance;
- custom configuration path option naming;
- dynamic rule tables;
- ignore-pattern list handling.

Classify these command-policy ideas as replaced by, or deferred to,
agent-native policy work:

- domain-specific `--format <compact|json|rich>` versus canonical `--json`;
- compact command-tree context for commands such as `rules` and `explain`;
- bounded output policy for list-shaped commands;
- enumerating valid rule names, severities, and output modes in diagnostics.

Classify DDLint's `--no-ignore` as historical DDLint prior art unless current
documentation can prove a direct OrthoConfig feature mapping. If that mapping
cannot be proven without code or product interpretation, record it as
deliberately not an OrthoConfig requirement in this phase and leave any
reusable policy implications to phase 7.

### Milestone 4: Mark active guidance versus preserved rationale

Audit historical notes and design documents for ambiguous status. At minimum
inspect:

- `docs/design.md`;
- `docs/agent-native-cli-design.md`;
- `docs/feedback-from-hello-world-example.md`;
- `docs/subcommand-refinements.md`;
- `docs/improved-error-message-design.md`;
- `docs/behavioural-testing-in-rust-with-cucumber.md`.

For each document touched, prefer a short document-level `Status:` paragraph
near the top over scattered warnings. Use these local meanings:

- `active guidance`: current maintainers should follow it;
- `proposed`: future work, not implemented;
- `adopted`: implemented practice;
- `historical proposal`: preserved rationale or source material;
- `superseded`: replaced by another named document or roadmap item.

Only create an ADR if the implementation records a substantive new decision,
such as adopting a repository-wide status taxonomy that changes the
documentation style guide. If the work merely applies the existing style guide,
update the relevant design note instead.

### Milestone 5: Update indexes and guides

Update `docs/contents.md` for any added, renamed, or reclassified document
whose description no longer matches its status.

Update `docs/developers-guide.md` only if the implementation adds a maintainer
convention such as "all historical design notes must begin with `Status:`".

Update `docs/users-guide.md` only if a user-facing behaviour, public API, or
consumer compatibility statement changes. For a documentation truth repair, the
likely correct outcome is no users-guide change.

Update `CHANGELOG.md` only if the approved implementation changes a
user-visible documentation promise or migration note. A purely internal
roadmap/archive repair may not need a changelog entry.

### Milestone 6: Validate

Run all gates sequentially. Capture each command with `tee` so truncated
terminal output is not the only evidence. Use the exact branch name in the log
path:

```sh
set -o pipefail
branch=5-1-2-retire-stale-retrospective-roadmap-items
project=ortho-config
log="/tmp/check-fmt-${project}-${branch}.out"
make check-fmt 2>&1 | tee "${log}"
log="/tmp/lint-${project}-${branch}.out"
make lint 2>&1 | tee "${log}"
log="/tmp/test-${project}-${branch}.out"
make test 2>&1 | tee "${log}"
log="/tmp/markdownlint-${project}-${branch}.out"
make markdownlint 2>&1 | tee "${log}"
log="/tmp/nixie-${project}-${branch}.out"
make nixie 2>&1 | tee "${log}"
```

If Markdown formatting changes are needed, run `make fmt` and then repeat the
affected gates:

```sh
set -o pipefail
branch=5-1-2-retire-stale-retrospective-roadmap-items
project=ortho-config
log="/tmp/fmt-${project}-${branch}.out"
make fmt 2>&1 | tee "${log}"
```

If any Rust code changes were approved despite this plan's documentation-only
expectation, add focused `rstest` unit coverage first, add `rstest-bdd`
behavioural coverage when externally observable workflows or command behaviour
change, and then run the same full gates.

### Milestone 7: Mark roadmap item done

After the implementation is validated, update `docs/roadmap.md` to mark item
5.1.2 and its subitems complete. Do this only after the approved implementation
has landed and the validation evidence is recorded in this plan.

Update this ExecPlan's `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` sections before the implementation commit.

## Validation expectations

For the initial plan-only pull request, validation proves the plan is well
formed and the repository still passes the requested gates. The plan-only
branch does not complete roadmap item 5.1.2.

For the later implementation pull request, validation proves that:

- the active roadmap no longer contains stale retrospective items in the active
  path;
- the archive preserves historical context while marking corrections;
- DDLint gap-analysis items are classified as implemented, deferred, or
  replaced by agent-native policy;
- historical design notes have clear active-versus-preserved status;
- `docs/contents.md` matches the documentation set;
- `make check-fmt`, `make lint`, and `make test` succeed;
- documentation-specific gates succeed or any pre-existing failures are
  documented with log evidence and edited lines are clean.

## Progress

- [x] (2026-05-18) Loaded the `execplans`, `leta`, `rust-router`,
  `hexagonal-architecture`, `firecrawl-mcp`, `commit-message`, and
  `pr-creation` skills relevant to planning, architecture boundaries, web
  research, committing, and pull request creation.
- [x] (2026-05-18) Confirmed the starting branch was
  `feat/plan-stale-roadmap-items`.
- [x] (2026-05-18) Renamed the branch to
  `5-1-2-retire-stale-retrospective-roadmap-items`.
- [x] (2026-05-18) Added the repository to `leta` for semantic navigation.
- [x] (2026-05-18) Used a Wyvern agent team for read-only planning
  reconnaissance across the roadmap/archive, DDLint and design documents, and
  testing/documentation plan constraints.
- [x] (2026-05-18) Used Firecrawl to inspect open source ADR prior art for
  preserving historical records while adding status or supersession context.
- [x] (2026-05-18) Drafted this pre-implementation ExecPlan.
- [x] (2026-05-18) Ran plan-branch validation. `make check-fmt`, `make lint`,
  `make test`, `make markdownlint`, and `make nixie` passed. Direct
  `markdownlint-cli2` validation of this plan and `docs/contents.md` also
  passed.
- [x] (2026-05-18) Ran `make fmt` for the documentation workflow. It still
  fails in `mdformat-all` because the wrapper invokes `markdownlint --fix`
  against pre-existing long lines in unrelated documentation. This plan and its
  contents entry pass direct Markdown validation.
- [x] (2026-05-19) Received explicit approval to implement this plan.
- [x] (2026-05-19) Re-ran the current-truth audit. The active roadmap,
  archive, DDLint gap analysis, and historical proposal documents still match
  the planned baseline.
- [x] (2026-05-19) Implemented the documentation repair in the active roadmap,
  archived v0.8.0 roadmap, DDLint gap analysis, and historical Cucumber guide.
- [x] (2026-05-19) Ran `coderabbit review --agent` after the main
  documentation milestone. It completed with zero findings; log:
  `/tmp/coderabbit-ortho-config-5-1-2-retire-stale-retrospective-roadmap-items-milestone1.out`.
- [x] (2026-05-19) Ran implementation validation. `make check-fmt`,
  `make lint`, `make test`, `make markdownlint`, and `make nixie` passed.
- [x] (2026-05-19) Marked roadmap item 5.1.2 and its subitems done after the
  validation gates passed.
- [x] (2026-05-19) Re-ran changed-file Markdown validation and a final
  `coderabbit review --agent` pass after completion bookkeeping. Both passed;
  the final CodeRabbit log is
  `/tmp/coderabbit-ortho-config-5-1-2-retire-stale-retrospective-roadmap-items-final.out`.

## Surprises & discoveries

- Observation: The active roadmap already has a clean top-level archive
  boundary. It says completed v0.8.0 phases are retained in
  `docs/archive/v0-8-0-roadmap.md` and that the active roadmap continues with
  future work. Impact: implementation should refine and reinforce this
  boundary, not rebuild the roadmap structure from scratch.
- Observation: The archived roadmap already contains an archive correction for
  `OrthoError::MissingRequiredValues`. Impact: the implementation should
  generalize the archive-status model rather than adding a second one-off
  warning.
- Observation: The DDLint gap-analysis document already marks itself as
  historical analysis and states the loading gaps are largely addressed.
  Impact: the implementation can focus on making each item classification
  explicit and removing ambiguity around remaining command-policy ideas.
- Observation: `docs/design.md`, `docs/feedback-from-hello-world-example.md`,
  and `docs/subcommand-refinements.md` already contain historical status
  language. Impact: only ambiguous or incomplete markers should be touched.
- Observation: `docs/contents.md` lists individual ExecPlans, but did not yet
  include a 5.1.1 or 5.1.2 entry. Impact: this plan-only change should update
  the contents index for the newly added ExecPlan.
- Observation: No context packs were available for this repository through the
  `context_pack` Model Context Protocol (MCP) server. Impact: Wyvern
  coordination used direct repository paths instead.
- Observation: `make fmt` remains unsuitable as a clean gate for this
  plan-only change because its `mdformat-all` wrapper reports existing
  repository-wide MD013 line-length failures through `markdownlint --fix`.
  Impact: the branch records the failure log, keeps unrelated formatter
  rewrites out of the commit, and validates the changed Markdown through
  `markdownlint-cli2` and the repository `make markdownlint` target.
- Observation: The implementation audit found that
  `docs/behavioural-testing-in-rust-with-cucumber.md` lacks a document-level
  status marker, while `docs/rstest-bdd-users-guide.md` is now the current
  behavioural-testing guide. Impact: classify the cucumber document as
  historical reference rather than changing active testing guidance.
- Observation: The implementation stayed within the documentation-only scope.
  Impact: no Rust tests, `rstest-bdd` scenarios, Kani harnesses, Verus proofs,
  users-guide changes, developers-guide changes, or ADRs were needed.

## Decision log

- Decision: Keep this branch as a pre-implementation plan branch. Rationale:
  the user explicitly said the plan must be approved before implementation, and
  the ExecPlan skill requires an approval gate.
- Decision: Treat roadmap item 5.1.2 as documentation truth repair unless later
  approval expands the scope. Rationale: the roadmap wording is about stale
  retrospective items, DDLint classification, and historical design notes, not
  runtime behaviour.
- Decision: Use existing local documentation status conventions before adding a
  new taxonomy. Rationale: `docs/documentation-style-guide.md` already defines
  document types and ADR status concepts, and Firecrawl research supports
  preserving or superseding historical records instead of silently rewriting
  them.
- Decision: Do not plan user-guide edits as mandatory for this item. Rationale:
  a documentation roadmap/archive repair does not by itself change library
  behaviour or API usage. The implementation plan still requires a users-guide
  update if consumer-facing behaviour or promises change.
- Decision: Classify DDLint command examples as prior art or agent-native policy
  input, not current OrthoConfig feature requirements. Rationale: current docs
  mark loading gaps as implemented and move command semantics into the
  agent-native design and roadmap phases.
- Decision: Do not add Rust tests for a documentation-only implementation.
  Rationale: the repository requires `rstest` and `rstest-bdd` where behaviour
  changes; adding tests that assert no runtime behaviour would be misleading.
- Decision: Treat the user's 2026-05-19 instruction to proceed with
  implementation as the approval gate for this plan. Rationale: the request
  explicitly names the plan and asks to implement the planned functionality.
- Decision: Do not update `docs/users-guide.md`, `docs/developers-guide.md`,
  `CHANGELOG.md`, or create an ADR. Rationale: this change updates maintainer
  roadmap and historical-document truth only; it does not change user-facing
  behaviour, internal maintainer conventions, release promises, or a
  substantive architecture decision.

## Outcomes & Retrospective

The implementation completed roadmap item 5.1.2 as a documentation truth repair.
`docs/roadmap.md` now marks the item done and keeps active phase 5.1 focused
on the boundary between historical context and future work.
`docs/archive/v0-8-0-roadmap.md` now explains how to read archived checkmarks
and calls out the missing-required-values entry as corrected and deferred
active work. `docs/ddlint-gap-analysis.md` now classifies loading gaps as
implemented, treats `--no-ignore` as DDLint prior art, and routes
command-policy ideas to the relevant agent-native roadmap phases. The only
historical note that needed a new status marker was
`docs/behavioural-testing-in-rust-with-cucumber.md`, which is now labelled as a
historical reference superseded in current practice by `rstest-bdd`.

Validation passed:

- `make check-fmt`:
  `/tmp/check-fmt-ortho-config-5-1-2-retire-stale-retrospective-roadmap-items.out`
- `make lint`:
  `/tmp/lint-ortho-config-5-1-2-retire-stale-retrospective-roadmap-items.out`
- `make test`:
  `/tmp/test-ortho-config-5-1-2-retire-stale-retrospective-roadmap-items.out`
- `make markdownlint`:
  `/tmp/markdownlint-ortho-config-5-1-2-retire-stale-retrospective-roadmap-items.out`
- `make nixie`:
  `/tmp/nixie-ortho-config-5-1-2-retire-stale-retrospective-roadmap-items.out`

No follow-up roadmap items or ADRs were created.
