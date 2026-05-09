# Overhaul OrthoConfig agent-native design and roadmap

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / Big Picture

This plan updates the OrthoConfig design documents and future roadmap so they
describe a coherent agent-native command-line interface (CLI) strategy rather
than a mostly completed historical feature list. After the update, a maintainer
or implementation agent should be able to read the design set, understand why
OrthoConfig is the right schema and code-generation spine for agent-native CLI
contracts, and follow `docs/roadmap.md` to deliver the next product phase in
review-sized slices.

The observable result is a documentation change, not a code feature. Success is
visible when the repository contains a ratified agent-native CLI design, the
roadmap is rewritten around future build slices, stale completed claims are
reconciled against the implementation, and documentation validation passes. The
implementation of the feature work described by the new roadmap is out of scope
until this plan is approved and completed.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not workarounds.

- Do not implement the agent-native runtime, intermediate representation (IR),
  lint, profile, delivery, feedback, or async job features in this plan. This
  plan produces design and roadmap updates only.
- Keep the final design uncompromising on the core decision from the user
  conversation: OrthoConfig must help developers enforce agent-native CLI
  behaviour mechanically through schema, generated metadata, and lint checks
  rather than relying on prose or code review.
- Preserve the existing product identity: OrthoConfig remains a Rust
  configuration, derive, documentation, and generator framework. It must not be
  redesigned into every downstream application's command runner.
- Treat `docs/` as the source of truth. Update or retire stale design notes
  instead of leaving contradictory guidance in place.
- Follow `docs/documentation-style-guide.md`: British English with Oxford
  spelling, sentence-case headings, code fence language identifiers, GFM
  checkboxes, 80-column prose wrapping, and 120-column code wrapping.
- If creating a roadmap, follow the GIST-aligned roadmap conventions from the
  `roadmap-doc` skill: phases are ideas, steps are workstreams, tasks are
  concrete execution units, and the roadmap contains no date commitments.
- Keep Markdown validation as part of the change. Run `make fmt`,
  `make markdownlint`, and `make nixie` after documentation edits.
- Do not run format, lint, or test commands in parallel. Capture long command
  output with `tee` in `/tmp`.
- Do not use Firecrawl unless a design claim depends on external material not
  already present in the user conversation. The supplied conversation contains
  the full agent-native CLI article, so external retrieval is not required for
  the initial draft.
- When collaborating with an agent team, exchange repository context through
  the `context_pack` Model Context Protocol (MCP) server.

If satisfying the objective requires violating a constraint, do not proceed.
Document the conflict in `Decision Log` and escalate.

## Tolerances (Exception Triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Scope: stop if the documentation overhaul requires changes to more than 12
  files or more than 1,500 net documentation lines.
- Interface: stop if the planned design requires a breaking public Rust API
  change without an explicit migration phase.
- Dependencies: stop if the documentation plan requires adding a new external
  dependency before any code implementation begins.
- Architecture: stop if the design cannot preserve the split between
  human-facing documentation IR and compact agent-facing invocation context.
- Ambiguity: stop if multiple valid product directions would materially change
  whether OrthoConfig implements a feature itself or only models and lints the
  downstream command contract.
- Validation: stop if Markdown validation still fails after two fix attempts.
- Agent-team drift: stop if delegated findings contradict local evidence in a
  way that changes the roadmap ordering or document update scope.

## Risks

Known uncertainties that might affect the plan. Identify these upfront and
update as work proceeds. Each risk notes severity, likelihood, and mitigation.

- Risk: Existing documents contain stale completion claims that may be partly
  true under different names. Severity: high. Likelihood: high. Mitigation:
  audit each contradiction against code before rewriting the roadmap; document
  uncertain items as "verify and reconcile" tasks instead of assuming either
  the docs or code are correct.
- Risk: Adding agent-native concepts to the existing `DocMetadata` schema could
  blur human documentation and agent invocation contracts. Severity: high.
  Likelihood: medium. Mitigation: design a sibling agent-context contract and
  explicitly state which data remains in documentation IR.
- Risk: Profiles, delivery, feedback, and async job ledgers could pull
  OrthoConfig into application runtime responsibilities. Severity: medium.
  Likelihood: medium. Mitigation: describe these as optional reusable
  primitives or metadata/lint surfaces unless later implementation proves a
  runtime helper belongs in the crate.
- Risk: `cargo-orthohelp` may be treated as only a documentation generator,
  leaving OrthoConfig without a concrete dogfood CLI for the first five
  principles. Severity: medium. Likelihood: medium. Mitigation: make
  `cargo-orthohelp` itself the reference Tier 1 agent-native CLI in the roadmap.
- Risk: The roadmap could become a layer-by-layer schema project rather than a
  set of usable vertical slices. Severity: medium. Likelihood: medium.
  Mitigation: structure phases around observable developer outcomes: whole-CLI
  introspection, enforceable policy, dogfood output, and optional compounding
  primitives.

## Progress

Use this list to summarize granular steps. Every stopping point must be
documented here, even if it requires splitting a partially completed task into
two ("done" versus "remaining"). This section must always reflect the actual
current state of the work.

- [x] (2026-05-09T10:56Z) Loaded repository guidance, the `leta`,
  `execplans`, `roadmap-doc`, and en-GB Oxford style skills.
- [x] (2026-05-09T10:56Z) Confirmed the current branch is
  `feat/ortho-agent-cli-roadmap`, so this plan belongs at
  `docs/execplans/ortho-agent-cli-roadmap.md`.
- [x] (2026-05-09T10:56Z) Created context pack `pk_xivjzrji` for the wyvern
  agent team and recorded the main design, roadmap, and implementation
  references.
- [x] (2026-05-09T10:56Z) Performed a local read-only audit of the design docs,
  roadmap, `OrthoConfigDocs` IR, `cargo-orthohelp`, and error surface.
- [x] (2026-05-09T10:56Z) Drafted this ExecPlan.
- [x] (2026-05-09T11:06Z) Received the wyvern reconnaissance result. It
  reported no completed findings, so no delegated findings were merged.
- [x] (2026-05-09T11:07Z) Validated the ExecPlan with `make fmt`,
  `make markdownlint`, and `make nixie`.

## Surprises & Discoveries

Unexpected findings during implementation that were not anticipated as risks.
Document with evidence so future work benefits.

- Observation: The `roadmap-doc` skill pointed to
  `/mnt/skills/user/roadmap-doc/references/conventions.md`, but this
  environment stores the conventions at
  `/home/leynos/.codex/skills/roadmap-doc/references/conventions.md`. Evidence:
  the first `sed` read failed with "No such file or directory"; a later `find`
  located the local reference file. Impact: the correct local conventions were
  read before drafting.
- Observation: `leta workspace add` succeeded, but semantic Rust lookup could
  not start because rust-analyzer was unavailable to the LSP server. Evidence:
  `leta grep` failed with "Language server 'rust-analyzer' for rust failed to
  start". Impact: code evidence for this plan comes from targeted `rg` and file
  reads instead of semantic LSP queries.
- Observation: `docs/roadmap.md` claims `OrthoError::MissingRequiredValues`
  is complete, and `docs/users-guide.md` documents it, but the current
  `OrthoError` enum does not contain that variant. Evidence: `docs/roadmap.md`
  lines 14-20 and `docs/users-guide.md` around the error handling section
  mention the variant; `ortho_config/src/error/types.rs` lists `CliParsing`,
  `File`, `CyclicExtends`, `Gathering`, `Merge`, `Validation`, and `Aggregate`.
  Impact: the overhaul must begin with a truth audit before adding new roadmap
  promises.
- Observation: The IR design describes recursive subcommand metadata, and the
  runtime `DocMetadata` struct has `subcommands: Vec<DocMetadata>`, but the
  derive generator currently emits `subcommands: Vec::new()`. Evidence:
  `docs/cargo-orthohelp-design.md` §2.1 defines recursive subcommands;
  `ortho_config/src/docs/ir.rs` contains the field;
  `ortho_config_macros/src/derive/generate/docs/mod.rs` emits an empty vector.
  Impact: whole-CLI introspection should be the first implementation slice in
  the future roadmap.
- Observation: The delegated wyvern agent returned without repository
  findings. Evidence: the agent reported that it only attempted tool discovery,
  could not retrieve the context pack, read no files, and made no edits.
  Impact: this plan proceeds from the local audit and records no external
  changes from the agent team pass.

## Decision Log

Record every significant decision made while working on the plan. Include
decisions to escalate, decisions on ambiguous requirements, and design choices.

- Decision: Use this ExecPlan as the planning artefact rather than immediately
  editing the design documents and roadmap. Rationale: the `execplans` skill
  requires an approval gate before executing non-trivial implementation work,
  and the requested overhaul is broad enough that the user should approve the
  document strategy first. Date/Author: 2026-05-09 (assistant).
- Decision: Do not use Firecrawl for the initial plan.
  Rationale: the user conversation includes the complete source article and
  asks for a plan based on that conversation; no design claim depends on
  externally retrieved content. Date/Author: 2026-05-09 (assistant).
- Decision: Treat OrthoConfig as the schema, documentation, and enforcement
  spine for agent-native CLIs, not as a mandatory runtime for every downstream
  command behaviour. Rationale: this preserves the crate's existing macro/IR
  architecture while delivering the blog post's strongest point: consistency
  must be generated or linted mechanically. Date/Author: 2026-05-09 (assistant).
- Decision: Plan a compact agent-context contract as a sibling to the existing
  documentation IR rather than overloading localized documentation output.
  Rationale: human docs and agent invocation context have different size,
  stability, localization, and token-budget requirements. Date/Author:
  2026-05-09 (assistant).
- Decision: Make `cargo-orthohelp` the first dogfood target for Tier 1
  agent-native CLI behaviour. Rationale: it already consumes the IR, has a
  bounded command surface, and can demonstrate structured output, enumerating
  errors, non-interactive defaults, and atomic artefact writes before
  downstream applications adopt new metadata. Date/Author: 2026-05-09
  (assistant).

## Outcomes & Retrospective

Summarize outcomes, gaps, and lessons learned at major milestones or at
completion. Compare the result against the original purpose. Note what would be
done differently next time.

- Outcome: Not yet complete. This draft records the source audit and proposed
  document-overhaul strategy. It has passed documentation validation and is
  ready for user review.

## Context and Orientation

The current documentation set has two different jobs that now need separating.
`docs/design.md` is the broad architecture document for OrthoConfig as a derive
macro, runtime crate, discovery system, merge engine, localization layer, and
documentation IR producer. `docs/cargo-orthohelp-design.md` is the focused
design for the `OrthoConfigDocs` IR and the `cargo-orthohelp` generator that
emits localized IR JSON, roff man pages, and PowerShell help. `docs/roadmap.md`
is currently mostly retrospective: nearly every item is checked off, and its
future section contains only async loading, custom providers, and live reload.

The code confirms that OrthoConfig already has the right foundation for the new
direction. `ortho_config/src/docs/ir.rs` defines versioned documentation
metadata, including CLI flags, environment variables, file keys, value types,
defaults, enum possible values, precedence, discovery, Windows hints, and
recursive subcommands. `cargo-orthohelp/src/main.rs` consumes that metadata to
emit documentation. The derive generator in
`ortho_config_macros/src/derive/generate/docs/mod.rs` currently fills
subcommands with an empty vector, which means the documented recursive shape is
not yet a whole-CLI introspection surface.

The user conversation establishes the product rationale and the new target bar.
Agent-native CLIs need table-stakes behaviour that does not break agents:
non-interactive defaults, structured output, errors that teach and enumerate,
safe retries with explicit mutation boundaries, and bounded responses. They
also need compounding behaviour that improves repeated agent use: cross-CLI
vocabulary consistency, three-layer introspection, async-aware execution,
persistent profiles, and two-way I/O through delivery and feedback. The final
architecture note in the conversation is the most important design constraint:
these properties should be mechanically generated, linted, or validated from
one schema rather than manually enforced in review.

## Final Design Decisions to Capture

The document overhaul must state these decisions plainly and consistently.

1. OrthoConfig will target agent-native CLI assistance as a first-class product
   direction. It will help downstream developers declare, generate, inspect,
   and lint their command contracts from the same source as configuration
   metadata.
2. The existing documentation IR remains the human documentation contract.
   It should be extended where human docs need more truth, but it should not
   become the only agent contract.
3. A new compact agent-context contract should describe invocation shape for
   agents: command paths, canonical verbs, flags, value types, required values,
   enum values, output contracts, pagination, mutation boundaries, async job
   metadata, profiles, delivery, and feedback availability.
4. Whole-CLI introspection requires real subcommand metadata. The future build
   must close the gap between recursive `DocMetadata` and the generator's
   current `subcommands: Vec::new()`.
5. Cross-CLI vocabulary should be enforceable through an opt-in strict policy
   before it becomes a default. The policy should reserve canonical names such
   as `get`, `list`, `create`, `update`, `delete`, `--json`, `--force`,
   `--dry-run`, `--limit`, `--cursor`, `--wait`, `--profile`, and `--deliver`,
   and it should reject or warn on off-policy names such as `info`, `ls`,
   `--format=json`, `--output json`, and `--skip-confirmations`.
6. OrthoConfig should model and lint command semantics for non-interactive
   prompts, destructive operations, idempotency, bounded list output, and async
   jobs. It should not pretend it can implement every downstream application's
   side effects.
7. Profiles, delivery sinks, feedback stores, and async job ledgers should be
   designed as optional primitives or metadata-backed conventions. The roadmap
   may include reusable helpers, but must keep them clearly opt-in.
8. `cargo-orthohelp` should demonstrate the table-stakes contract itself:
   non-interactive by default, structured `--json` command summaries, clean
   stdout/stderr separation, enumerating errors, stable exit classes, and
   atomic file output.
9. Documentation truth must be repaired before new work is planned. Stale
   claims about `MissingRequiredValues`, completed gaps in
   `docs/ddlint-gap-analysis.md`, and historical implementation-roadmap text in
   `docs/design.md` must either be reconciled, archived, or rewritten.

## Document Update Scope

The implementation of this plan should update these documents unless the truth
audit shows a narrower edit is sufficient.

- `docs/design.md`: add a product direction section explaining why
  agent-native CLI assistance belongs in OrthoConfig; trim or clearly mark the
  old v0.1-v0.5 implementation roadmap as historical; update future work so it
  distinguishes async configuration loading from async application jobs.
- `docs/cargo-orthohelp-design.md`: revise the IR design to introduce the
  compact agent-context contract, subcommand population requirements, output
  metadata, policy metadata, and generator/dogfood obligations.
- `docs/roadmap.md`: replace the mostly completed retrospective roadmap with a
  future-looking GIST roadmap for the agent-native work. Preserve historical
  completion context only where it helps explain dependencies.
- `docs/users-guide.md`: correct stale error claims and add consumer-facing
  guidance for generated agent context and strict agent-native policy once the
  design is accepted.
- `cargo-orthohelp/README.md`: add the intended agent-native contract for
  `cargo-orthohelp`, including future `--json` summaries, stdout/stderr
  behaviour, exit classes, and result artefact reporting.
- `docs/improved-error-message-design.md`: reconcile the proposed
  `MissingRequiredValues` design with the current error enum. Either mark the
  design as pending, update it to the implemented shape, or add a follow-up
  roadmap task to implement it.
- `docs/ddlint-gap-analysis.md`: update or retire stale unchecked gaps that
  have since been completed according to `docs/roadmap.md` and the current
  codebase.
- `docs/feedback-from-hello-world-example.md` and
  `docs/subcommand-refinements.md`: preserve them as historical rationale, but
  add short status notes if the current design supersedes earlier proposals.
- `README.md` and `ortho_config/README.md`: update only if the accepted
  product direction needs top-level positioning. Avoid duplicating detailed
  design text from `docs/`.

The implementation may create one new focused design document, tentatively
`docs/agent-native-cli-design.md`, if adding the new material to
`docs/design.md` and `docs/cargo-orthohelp-design.md` would make those
documents harder to use. If created, it must be the authoritative design for
agent-native contracts and the existing documents must link to it.

## Future Roadmap Shape to Produce

The rewritten `docs/roadmap.md` should follow this phase order.

## 1. Foundation: truthful documents and agent-native contracts

Idea: if OrthoConfig first reconciles its documentation with implementation
truth and records the agent-native contract boundary, later work can extend the
IR without compounding stale claims or accidental runtime scope creep.

This phase should include tasks to audit stale claims, write or update the
agent-native design, decide the schema split between documentation IR and
agent-context IR, and add an architectural decision record if the maintainers
want a durable decision record.

## 2. Vertical slice: whole-CLI introspection for real command trees

Idea: if the existing IR can describe complete command trees, OrthoConfig can
deliver immediate agent value through accurate `agent-context` output before
profiles, delivery, or async helpers exist.

This phase should include tasks to populate subcommand metadata from
`SelectedSubcommandMerge` or a companion docs trait, define the compact
agent-context schema, add `cargo orthohelp --format agent-context` or an
equivalent command, and validate token-bounded output against fixtures.

## 3. Vertical slice: mechanically enforced vocabulary and invocation policy

Idea: if developers can opt into a strict policy and receive compile-time or
tool-time failures for off-convention verbs, flags, output modes, and mutation
metadata, OrthoConfig turns agent-native guidance into enforceable build
feedback.

This phase should include tasks for policy metadata, banned vocabulary checks,
canonical flag checks, structured output metadata, enum value coverage,
pagination metadata, destructive-operation metadata, non-interactive metadata,
and a `cargo orthohelp` lint/check command.

## 4. Vertical slice: `cargo-orthohelp` as the reference agent-native CLI

Idea: if OrthoConfig's own generator satisfies the table-stakes agent-native
rules, downstream projects get both tooling and a working example of the
contract they are being asked to adopt.

This phase should include tasks to add `--json`, structured success summaries,
JSON error rendering or stable exit classification, better enumerating errors,
stdout/stderr separation, generated artefact reporting, and atomic output
writes.

## 5. Vertical slice: compounding primitives for repeated agent workflows

Idea: if the core introspection and policy model is already trustworthy,
optional primitives for profiles, delivery, feedback, and async job metadata
can reduce repeated-agent friction without destabilizing the configuration
loader.

This phase should include tasks to design profile metadata and precedence,
delivery target parsing, feedback storage, async job metadata, and optional
runtime helper crates or feature flags. Each task must state whether
OrthoConfig implements the helper or only models and lints the downstream
contract.

## 6. Deferred extensions after the core agent-native promise

Idea: if the core agent-native promise is already useful and boring to operate,
the project can evaluate broader extensions on product value instead of letting
them distort the main architecture.

This phase should collect lower-priority work such as full MCP surface
generation, runtime OpenAPI-style explorers, live configuration reloading,
remote configuration providers, and fully managed async job ledgers.

## Plan of Work

Stage A: Finish reconnaissance and reconcile sources. Wait for the wyvern
brief, then compare its findings against the local audit. Re-read any cited
files where the findings materially affect document scope. Update this
ExecPlan's `Surprises & Discoveries` and `Decision Log` before editing any
source documents.

Stage B: Perform a truth audit of stale documentation. Check `docs/roadmap.md`,
`docs/users-guide.md`, `docs/improved-error-message-design.md`,
`docs/ddlint-gap-analysis.md`, `docs/design.md`, and
`docs/cargo-orthohelp-design.md` against the current implementation. Record
contradictions in the new design or roadmap so future implementers know whether
each item is implemented, pending, or superseded.

Stage C: Draft the agent-native design. Either add a focused
`docs/agent-native-cli-design.md` or add a clearly bounded section to the
existing design documents. The design must explain the product rationale, the
schema/codegen approach, the split between documentation IR and agent context,
strict policy mode, subcommand introspection, output contracts, mutation
metadata, profiles, delivery, feedback, and async job metadata.

Stage D: Rewrite the roadmap. Replace the current retrospective list with a
future-looking GIST roadmap using the phase shape above. Each task must cite
the relevant design document section, carry dependencies where needed, and
include success criteria for observable behaviour. Unit and behavioural tests
belong inside implementation task success criteria; whole-surface combinatorial
tests may be first-class tasks.

Stage E: Update consumer-facing docs and reference READMEs. Correct stale
claims in `docs/users-guide.md`, `docs/improved-error-message-design.md`, and
`docs/ddlint-gap-analysis.md`. Update `cargo-orthohelp/README.md` to describe
the intended reference CLI behaviour. Update root README files only for
top-level positioning and links.

Stage F: Validate and commit. Run formatting and Markdown validation
sequentially with logs in `/tmp`, inspect failures, fix documentation issues,
and commit only after the gates pass.

## Concrete Steps

1. Review the wyvern brief and update this plan if it identifies a missing
   source document, contradiction, or stronger roadmap ordering.
2. Run a focused staleness audit:

   ```bash
   rg -n \
     -e "MissingRequiredValues" \
     -e "Future enhancements" \
     -e "Implementation Roadmap" \
     -e "Observed Gaps" \
     -e "agent-context" \
     -e "profile" \
     -e "deliver" \
     -e "feedback" \
     -e "--json" \
     docs README.md cargo-orthohelp/README.md
   ```

3. Decide whether to create `docs/agent-native-cli-design.md` or distribute
   the design across `docs/design.md` and `docs/cargo-orthohelp-design.md`.
   Escalate if this choice would materially change the review scope.
4. Update the design documents with the final decisions listed in this plan.
5. Rewrite `docs/roadmap.md` using the six-phase future roadmap shape and the
   GIST roadmap conventions.
6. Update stale or dependent guide documents:
   `docs/users-guide.md`, `docs/improved-error-message-design.md`,
   `docs/ddlint-gap-analysis.md`, and `cargo-orthohelp/README.md`.
7. Run validation sequentially, logging to `/tmp`:

   ```bash
   set -o pipefail
   make fmt 2>&1 | tee /tmp/fmt-ortho-agent-cli-roadmap.out
   make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-agent-cli-roadmap.out
   make nixie 2>&1 | tee /tmp/nixie-ortho-agent-cli-roadmap.out
   ```

8. Inspect each log if a command fails, make the smallest documentation fix
   that addresses the failure, and rerun the failed gate.
9. Commit the completed documentation overhaul with a file-based commit
   message after all gates pass.

## Validation and Acceptance

Acceptance for this plan is met when the documentation overhaul, once approved
and executed, satisfies all of the following:

- The design documents clearly state why OrthoConfig is adopting
  agent-native CLI assistance as a product direction and why schema-backed
  enforcement is the chosen implementation shape.
- The design set names every final decision from the user conversation and
  translates each of the ten agent-native principles into an OrthoConfig
  responsibility, downstream application responsibility, or deferred item.
- The distinction between documentation IR and agent-context IR is explicit,
  and the future roadmap contains implementation tasks for both where needed.
- The current contradictions around `MissingRequiredValues`, empty generated
  subcommands, historical roadmap status, and stale gap-analysis checkboxes are
  resolved or turned into explicit future tasks.
- `docs/roadmap.md` is future-looking, GIST-aligned, free of date promises,
  and organized around vertical slices rather than technical layers.
- `cargo-orthohelp` has a documented path to become the reference
  agent-native CLI for the first five principles.
- Markdown formatting and validation pass with `make fmt`,
  `make markdownlint`, and `make nixie`.

## Recovery and Re-run Notes

All planned edits are documentation-only and should be safe to rerun. If
formatting changes more files than expected, inspect `git diff --stat` before
continuing. If `make nixie` reports Mermaid issues introduced by the update,
fix the diagrams rather than disabling validation. If a stale documentation
claim cannot be resolved from the current implementation, keep the claim out of
the authoritative design and add a roadmap task to investigate it.
