# Record consumer dependency boundaries for Weaver and Netsuke

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS

This plan covers roadmap item 5.2.3 only. It must not be implemented until the
maintainer approves it.

## Purpose / big picture

Roadmap item 5.2.3 records the dependency boundary between OrthoConfig and its
first two downstream consumers, Weaver and Netsuke. The objective is normative
documentation, not new code: after this plan is approved and implemented, a
maintainer reading the design set should be able to tell, for every reusable
agent-native capability, whether OrthoConfig support is a hard prerequisite for
Weaver's generated command surface or a soft prerequisite that a consumer may
temporarily satisfy with a local adapter until OrthoConfig catches up.

Observable success looks like this. A maintainer can open
`docs/agent-native-cli-design.md` §2.1 and read three explicit lists: what
OrthoConfig owns; what Weaver owns; and what Netsuke owns. They can then read
a sibling §2.2, "Consumer dependency tier", and see a single dependency-tier
matrix (capability, tier, roadmap item, consumer adaptation rule, replacement
trigger) showing that whole-CLI introspection, strict vocabulary policy,
agent-context IR, and localized help generation are ship-time hard
dependencies for Weaver's generated command surface, while profiles, delivery,
feedback, skill manifests, and execution ledgers are ship-time soft
dependencies that may be locally adapted. Every other affected document
(`docs/design.md`, `docs/cargo-orthohelp-design.md`, `docs/users-guide.md`,
`docs/developers-guide.md`, and ADR-003) carries only a back-reference to
§2.2; none restates the matrix. Roadmap item 5.2.3 is then marked done.

The plan is documentation-only. It does not change schema ownership, runtime
code, generated artefacts, public Rust APIs, or `cargo-orthohelp` command-line
behaviour. The schema ownership decision in
[ADR-003](../adr-003-define-schema-ownership-for-agent-native-contracts.md) and
the compatibility rules recorded by roadmap item 5.2.2 remain authoritative.
This plan applies that accepted boundary by giving every capability a
dependency tier and a normative consumer-adaptation rule.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not workarounds.

- Do not implement this plan until explicit approval is received.
- Treat this as a documentation-only change. No production Rust code, no
  `cargo-orthohelp` generator, and no schema definition file changes are in
  scope.
- Preserve the schema ownership defined in ADR-003 and recorded in
  `docs/agent-native-cli-design.md` §3, `docs/cargo-orthohelp-design.md` §1,
  and `docs/developers-guide.md` "Schema ownership". OrthoConfig owns reusable
  command-contract machinery, agent context lives in
  `ortho_config::agent_context`, documentation IR lives in
  `ortho_config::docs`, and policy reports live in `cargo_orthohelp::policy`.
- Preserve the migration rules recorded by roadmap item 5.2.2. The legacy
  `cargo orthohelp --format ir`, `--format man`, `--format ps`, and
  `--format all` behaviours, their output paths, and the success/failure
  contract remain compatible. New documentation must not imply that adding a
  dependency tier changes those surfaces.
- Use the canonical OrthoConfig boundary language from
  `docs/agent-native-cli-design.md` §2.1. OrthoConfig must not be described as
  owning semantic code editing, build graph execution, sandboxing, or
  package-specific run records, even when those activities consume OrthoConfig
  metadata.
- Define "ship-time hard dependency" and "ship-time soft dependency" in plain
  language the first time the terms appear, anchored to consumer adaptation
  behaviour. A ship-time hard dependency means Weaver's generated command
  surface cannot ship without that OrthoConfig capability. A ship-time soft
  dependency means a consumer may keep shipping by carrying a temporary local
  adapter while OrthoConfig support arrives later. State explicitly that this
  is a ship-time framing, not a runtime resilience framing, so the AWS
  Well-Architected sense of "soft dependency" (a runtime fallback or circuit
  breaker) is not reused under the same words inside this project.
- Make the dependency tier address contract dependency, not delivery state.
  An entry is hard or soft based on whether Weaver's generated command surface
  can ship without the OrthoConfig contract, irrespective of whether that
  contract is already shipped, in flight, or merely planned. A reader must not
  be able to conclude that the hard tier is "mostly shipped" or that the soft
  tier is "blocked".
- Treat `docs/agent-native-cli-design.md` §2.2 as the authoritative location
  for the dependency-tier matrix. ADR-003 and every other affected document
  carry only a one-sentence back-reference. If the back-reference and §2.2
  ever disagree, §2.2 wins.
- Mark whole-CLI introspection, strict vocabulary policy, agent-context IR, and
  localized help generation as hard dependencies for Weaver's generated command
  surface, with explicit references to the roadmap items that deliver them:
  §6.1 and §6.2 for whole-CLI introspection and agent-context IR, §7.1 for
  strict vocabulary policy, and the existing OrthoConfigDocs contract together
  with §6.1.1 for localized help generation.
- Mark profiles, delivery, feedback, skill manifests, and execution ledgers as
  soft dependencies, with explicit references to roadmap items §9.1, §9.2,
  §9.3, §6.3, and §7.2, and an explicit note that consumers may carry a local
  adapter until OrthoConfig publishes the reusable contract.
- Keep the soft-dependency adaptation rule consistent with the existing scope
  boundary: a consumer may carry a temporary local adapter for the *contract*
  shape (for example, parsing `--profile`, `--deliver`, `feedback <text>`,
  or a JSONL ledger record), but must not duplicate the *domain* behaviour
  OrthoConfig does not own (semantic code editing for Weaver, build graph
  execution for Netsuke).
- Spell out the conflict-resolution rule. When OrthoConfig publishes the
  reusable contract for a soft-dependency capability, the consumer must
  replace its local adapter within the next consumer release; if the consumer
  adapter and the published OrthoConfig shape disagree, the published shape
  wins. The local adapter must declare which roadmap item it shadows so the
  replacement can be tracked.
- Use British English Oxford spelling throughout, except for code identifiers,
  external API names, and direct quotations.
- Follow `docs/documentation-style-guide.md` for headings, link style, ADR
  shape, and Compatibility and Migration framing.
- Use repository Make targets for all gates and capture output with `tee` under
  `/tmp`. Run gates sequentially, never in parallel.
- Avoid public API signature changes. None are anticipated; if one becomes
  required, stop and escalate.
- Avoid new crate, tooling, or external service dependencies.
- Avoid circular crate dependencies at all costs. The dependency-tier
  vocabulary must flow downwards (OrthoConfig publishes contracts that
  consumers depend on) and must never describe `ortho_config` as depending on
  Weaver or Netsuke crates.
- Do not mark roadmap item 5.2.3 done until the approved implementation,
  documentation updates, validation gates, CodeRabbit review, commit, push,
  and pull-request updates are complete.

If satisfying the objective requires violating a constraint, stop, document
the conflict in `Decision Log`, and ask for direction.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Approval: stop after this draft is complete and wait for explicit approval
  before implementation.
- Scope: stop if implementation requires touching more than 10 files or more
  than 700 net lines of documentation. The expected surface is ADR-003 and
  five design or guide documents.
- Public API: stop if any public Rust type, trait, constant, command-line
  spelling, or generated file path must change.
- Ownership: stop if writing the dependency-tier list shows that ADR-003 is
  ambiguous about who owns a capability. A new ownership decision is out of
  scope for 5.2.3 and requires a 5.2.x extension or a new ADR.
- Vocabulary: stop if the hard or soft dependency framing would force a
  competing definition that contradicts the AWS Well-Architected hard/soft
  dependency convention or the existing Compatibility and Migration framing.
- Tests: stop if `make check-fmt`, `make lint`, `make test`,
  `make typecheck`, `make markdownlint`, or `make nixie` still fails after two
  focused fix attempts on the same gate.
- Documentation: stop if the same dependency tier cannot be described
  identically in `docs/agent-native-cli-design.md`, `docs/design.md`,
  `docs/cargo-orthohelp-design.md`, `docs/users-guide.md`,
  `docs/developers-guide.md`, and ADR-003.
- Roadmap: stop if a hard or soft classification would require changing the
  scope of an existing roadmap item rather than referencing it. The plan
  describes dependency tiers; it does not redesign roadmap phases.
- Roadmap drift: cite every roadmap item by both number and title in the
  dependency-tier matrix so a later renumbering surfaces in review. If a
  citation would need to change because a section moved, update the matrix in
  the same change rather than relying on the old number.
- Review: stop if `coderabbit review --agent` reports a concern that would
  require violating a constraint.
- Process: stop if branch tracking, push, or draft pull-request creation fails
  in a way that might hide review feedback.

## Risks

Known uncertainties that might affect the plan. Identify these upfront and
update as work proceeds.

- Risk: the hard or soft classification could be mistaken because a roadmap
  capability is more entangled with Weaver's generated command surface than
  the design currently records. Severity: high. Likelihood: medium.
  Mitigation: anchor every classification to a concrete consumer behaviour
  (what fails if the capability is missing), and require the agent-native
  design document to spell out that behaviour beside the classification.

- Risk: the documentation could imply that consumers are allowed to fork
  OrthoConfig domains (for example, to ship their own agent-context schema)
  permanently. Severity: high. Likelihood: medium. Mitigation: state that soft
  dependencies permit a *temporary* local adapter shaped like the reusable
  contract, and require the adapter to be replaced once OrthoConfig publishes
  the reusable surface; record the deprecation expectation in the same
  paragraph and add the shape-conflict rule that the published OrthoConfig
  shape wins.

- Risk: a soft-dependency roadmap item slips its phase and a "temporary"
  local adapter becomes the de facto contract, with consumer shape diverging
  from the eventual OrthoConfig surface. Severity: high. Likelihood: medium.
  Mitigation: require the local adapter to declare which roadmap item it
  shadows in the developer guide's "shadowed contracts" pointer, and keep the
  conflict-resolution rule (OrthoConfig's published shape wins) prominent in
  §2.2.

- Risk: "hard" and "soft" already carry a runtime resilience meaning in the
  AWS Well-Architected framework, so a future reader could repurpose the
  terms for runtime fallback inside this project. Severity: medium.
  Likelihood: medium. Mitigation: qualify the terms as "ship-time" on first
  use in §2.2 and forbid runtime resilience reuse of the same words in the
  same paragraph.

- Risk: the back-reference paragraph in ADR-003 and the matrix in §2.2 drift
  apart after a future ADR-003 edit. Severity: medium. Likelihood: medium.
  Mitigation: keep ADR-003's amendment a single back-reference sentence with
  no normative content of its own, and record the precedence rule (§2.2
  wins) in Constraints.

- Risk: introducing hard or soft vocabulary could clash with the existing
  Compatibility and Migration wording or with ADR-003's "reusable contracts"
  language. Severity: medium. Likelihood: medium. Mitigation: define the
  terms once in `docs/agent-native-cli-design.md` §2.1, cross-link from every
  other document, and keep the Compatibility and Migration sections in 5.2.2
  documents unchanged unless a wording conflict is found in review.

- Risk: documentation could drift from code if future roadmap work renames a
  capability without updating the tier table. Severity: medium. Likelihood:
  medium. Mitigation: cite the roadmap item number beside every tier entry so
  later edits to either side fail review unless both sides are updated.

- Risk: `coderabbit review --agent` or GitHub operations may be unavailable in
  the local environment. Severity: medium. Likelihood: low. Mitigation: record
  the exact command and failure and escalate rather than claiming review or
  pull-request creation succeeded.

- Risk: the maintainer may want a standalone ADR rather than reusing ADR-003.
  Severity: low. Likelihood: low. Mitigation: keep the ADR amendment minimal
  and approval-gated so it can be lifted into a new ADR without redoing the
  classification work if review requests it.

## Progress

Use a list with checkboxes to summarise granular steps. Every stopping point
must be documented here, even if it requires splitting a partially completed
task into two. This section must always reflect the actual current state of
the work.

- [x] (2026-06-02T00:00:00Z) Loaded the `leta`, `rust-router`, and `execplans`
  skills.
- [x] (2026-06-02T00:00:00Z) Added the worktree to the leta workspace.
- [x] (2026-06-02T00:00:00Z) Read source documents:
  `docs/roadmap.md` §5.2.3,
  `docs/agent-native-cli-design.md` §2.1 and §6,
  `docs/cargo-orthohelp-design.md` §0.1 and §12,
  `docs/design.md` §2 and §8,
  `docs/users-guide.md` "Documentation metadata",
  `docs/developers-guide.md` "Schema ownership", and
  `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`.
- [x] (2026-06-02T00:00:00Z) Used an Explore reconnaissance subagent to confirm
  every existing Weaver and Netsuke reference and to verify that the terms
  "hard dependency" and "soft dependency" do not yet appear in `docs/`.
- [x] (2026-06-02T00:00:00Z) Used Firecrawl prior-art search for the hard
  versus soft dependency vocabulary (AWS Well-Architected reliability pillar
  and academic sources) and for agent skill manifest prior art (Skilldex,
  TrueFoundry, Microsoft Bot Framework skills).
- [x] (2026-06-02T00:00:00Z) Drafted this pre-implementation ExecPlan.
- [x] (2026-06-02T00:00:00Z) Ran a logisphere community-of-experts review on
  the draft (Pandalump, Wafflecat, Buzzy Bee, Telefono, Doggylump, Dinolump)
  and applied the blocking punch-list items: moved the dependency tier to a
  sibling §2.2, qualified the vocabulary as "ship-time", added the
  precedence rule that §2.2 wins over back-references, sharpened the
  soft-dependency conflict-resolution rule, separated contract dependency
  from delivery state, added the considered options to the Decision Log,
  and switched the §2.2 representation from prose to a single matrix.
- [x] (2026-06-04T00:00:00Z) Received explicit user approval to proceed with
  implementation.
- [x] (2026-06-04T00:00:00Z) Confirmed the current branch is
  `5-2-3-record-consumer-dependency-boundaries` and the worktree was clean
  before implementation edits began.
- [x] (2026-06-04T00:00:00Z) Established the implementation baseline by
  running `make check-fmt`, `make lint`, `make test`, `make typecheck`, and
  `make markdownlint` sequentially with `tee` logs under `/tmp`; all passed.
- [x] (2026-06-04T00:00:00Z) Landed the dependency-tier pointer in
  `docs/agent-native-cli-design.md` §2.1 and the authoritative dependency
  matrix in §2.2.
- [x] (2026-06-04T00:00:00Z) Cross-linked the dependency tiers from
  `docs/design.md`,
  `docs/cargo-orthohelp-design.md`, `docs/users-guide.md`,
  `docs/developers-guide.md`, and amended ADR-003 with a brief consequence
  note.
- [x] (2026-06-04T00:00:00Z) Ran the documentation milestone gates before
  CodeRabbit review: `make check-fmt`, `make lint`, `make test`,
  `make typecheck`, and `make markdownlint` all passed with `/tmp` logs.
- [x] (2026-06-04T00:00:00Z) Ran `coderabbit review --agent`; it completed
  with `findings: 0`, so no follow-up fixes were required.
- [x] (2026-06-04T00:00:00Z) Committed the dependency-tier documentation as
  `a5d4474` (`Record consumer dependency tiers`).
- [x] (2026-06-04T00:00:00Z) Marked roadmap item 5.2.3 and its three
  acceptance bullets done in `docs/roadmap.md`.
- [ ] Push to
  `origin/5-2-3-record-consumer-dependency-boundaries`, and open or update the
  draft pull request whose title contains `(5.2.3)`.

## Surprises & discoveries

This section is intentionally short during draft. Populate it during
implementation.

- Observation: the words "hard dependency" and "soft dependency" do not
  currently appear anywhere in `docs/`. Evidence: ripgrep over the worktree
  found no matches. Impact: the new vocabulary can be introduced cleanly
  without colliding with prior usage, but its first occurrence must define the
  terms.

- Observation: the AWS Well-Architected reliability pillar already uses
  "hard dependency" and "soft dependency" with the same intent that this plan
  proposes (a soft dependency failure can be compensated for, a hard
  dependency failure cannot). Evidence: AWS publishes the framework at
  <https://docs.aws.amazon.com/wellarchitected/latest/reliability-pillar/>.
  Impact: the plan can adopt the same terms with a one-line definition rather
  than inventing new vocabulary.

- Observation: roadmap §9.1, §9.2, and §9.3 already explicitly describe
  profiles, delivery and feedback, and execution ledgers as optional or
  application-owned. Evidence: `docs/roadmap.md` §9 lists them under
  "Add compounding primitives". Impact: the soft-dependency classification can
  cite those phase 9 items as the future OrthoConfig surface rather than
  inventing new ones.

- Observation: implementation approval arrived on 2026-06-04 while the plan
  still carried `DRAFT` status from 2026-06-02. Impact: the plan status was
  moved directly to `IN PROGRESS` before any implementation edits, preserving
  the approval gate while avoiding a stale status during delivery.

- Observation: the baseline quality gates passed before the dependency-tier
  documentation was edited. Evidence: `make check-fmt`, `make lint`,
  `make test`, `make typecheck`, and `make markdownlint` all exited with
  status 0 with logs written under `/tmp`. Impact: later failures can be
  attributed to the implementation rather than pre-existing repository state.

- Observation: `make fmt` applied repository-wide Markdown reflow beyond the
  planned document surface and then failed on one line-length issue. Impact:
  unrelated formatter churn was reverted, and non-mutating gates proved the
  kept documentation changes are formatted and lint-clean.

- Observation: CodeRabbit reviewed both the initial documentation milestone and
  the cleaned final diff and reported zero findings each time. Evidence:
  `coderabbit review --agent` completed with
  `{"status":"review_completed","findings":0}` after the final deterministic
  gate run. Impact: there are no review concerns to clear before committing
  the milestone.

## Decision log

Record every significant decision made while working on the plan, including
decisions to escalate, decisions on ambiguous requirements, and design
choices.

- Decision: Treat this branch as a pre-implementation plan branch and leave
  roadmap item 5.2.3 unchecked until implementation completes. Rationale: the
  user explicitly said the plan must be approved before it is implemented.
  Marking the roadmap done now would misrepresent the feature state.
  Date/Author: 2026-06-02 / Codex.

- Decision: Adopt the AWS Well-Architected "hard dependency" and "soft
  dependency" vocabulary rather than coining new terms. Rationale: the
  industry vocabulary is widely understood; it captures the consumer
  adaptation rule the roadmap text requires; and it does not invent a
  competing framework. Date/Author: 2026-06-02 / Codex.

- Decision: Land the dependency-tier matrix as a sibling §2.2 of
  `docs/agent-native-cli-design.md` and cross-link from every other affected
  document, rather than creating a new standalone ADR or extending §2.1.
  Considered options were:

  - A. Amend ADR-003 with a longer consequence paragraph and embed the matrix
    in §2.1. Rejected: §2.1 already carries the ownership boundary and §8.1
    carries the legacy defaulting table; adding a third structural framework
    under the same heading risks framework collisions, and ADR-003's
    Consequences section was not written to hold normative consumer guidance.
  - B. Create a new ADR-004 "Consumer dependency tier model" alongside a
    §2.2 cross-reference. Rejected for now: the predecessor 5.2.2 plan
    established the lightweight pattern of placing migration rules into
    existing design documents, and a new ADR would expand scope without
    changing the boundary content. Recorded here so the option can be lifted
    if review later requests a standalone decision record.
  - C. Replace "hard / soft" with "blocking / adaptable" or
    "required / shadowed". Rejected: the AWS Well-Architected vocabulary is
    widely understood and is anchored by a one-sentence "ship-time" qualifier
    in §2.2 that forbids reuse of the same words for runtime resilience inside
    this project. The cost of new vocabulary is higher than the cost of the
    qualifier.
  - D. Use a single dependency-tier matrix (capability, tier, roadmap item,
    consumer adaptation rule, replacement trigger) rather than prose lists.
    Accepted alongside the §2.2 choice because a matrix scales as new
    capabilities arrive and makes drift mechanically obvious in review.

  Date/Author: 2026-06-02 / Codex.

- Decision: Map every hard and soft classification to specific roadmap items.
  Rationale: a dependency tier without an implementation reference would be a
  documentation claim with no enforcement. The roadmap citations make
  drift visible. Date/Author: 2026-06-02 / Codex.

- Decision: Treat the user's 2026-06-04 instruction to proceed as explicit
  implementation approval for this ExecPlan. Rationale: the message names the
  exact plan path and asks for implementation according to it. Date/Author:
  2026-06-04 / Codex.

## Outcomes & retrospective

The implementation recorded the consumer dependency tier in
`docs/agent-native-cli-design.md` §2.2 as the single authoritative matrix and
kept every other affected document to a back-reference. ADR-003 needed only a
brief consequence note, not a new decision record. CodeRabbit reported zero
findings on both the initial documentation milestone and the cleaned final
diff. The final wording preserved the planned hard/soft tier split and the
replacement rule that the published OrthoConfig contract wins over temporary
consumer adapters.

## Context and orientation

OrthoConfig is a Rust workspace. The `ortho_config` crate owns runtime
configuration loading, merge behaviour, localization, documentation IR types,
and the reusable compact agent-context contract. The `ortho_config_macros`
crate derives metadata and loading code. The `cargo-orthohelp` binary consumes
documentation IR and emits human documentation artefacts; it also owns the
policy-report contract for `--check-agent-native` reporting.

The boundary between OrthoConfig and downstream consumers is canonical in
`docs/agent-native-cli-design.md` §2.1. OrthoConfig owns schemas and command
metadata, documentation IR and compact agent-context IR, vocabulary and
global-option policy, renderer metadata, generated help, man pages,
completions, reference artefacts, policy linting and drift checks, and
optional primitives for profiles, delivery targets, feedback stores, skill
manifests, and execution ledgers. Weaver owns semantic execution: capability
routing, Rope, rust-analyzer, Language Server Protocol providers, Tree-sitter
parsing, Sempai providers, sandboxing, Double-Lock safety, actual edits,
semantic refusal logic, and provider-specific idempotency. Netsuke owns build
and package semantics: manifest interpretation, subprocess execution, build
graph logic, and package-specific run records.

Roadmap item 5.2.1 recorded that compact agent context lives in
`ortho_config::agent_context` with `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`, the
documentation IR remains owned by `ortho_config::docs` with
`ORTHO_DOCS_IR_VERSION`, and policy reports live in `cargo_orthohelp::policy`
with `ORTHO_POLICY_REPORT_SCHEMA_VERSION`. Roadmap item 5.2.2 recorded that
the existing `cargo-orthohelp` `ir`, `man`, `ps`, and `all` formats stay
compatible until a versioned migration is approved. This plan applies both of
those baselines without renegotiating them.

Relevant documentation sources are:

- `docs/roadmap.md` for item 5.2.3 and its dependencies on 5.2.1 and 5.2.2.
- `docs/agent-native-cli-design.md` for the canonical agent-native boundary
  (§2.1), the contract surfaces (§3), and the legacy defaulting table (§8.1).
- `docs/cargo-orthohelp-design.md` for the documentation IR pipeline (§0.1,
  §1, §2) and versioning (§12).
- `docs/design.md` for the consumer alignment goal (§2 "Reusable consumer
  contracts") and the future-work scope (§8).
- `docs/users-guide.md` "Documentation metadata (OrthoConfigDocs)" for the
  downstream consumer compatibility wording.
- `docs/developers-guide.md` "Schema ownership" for the internal practice
  framing.
- `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md` for
  the accepted ownership decision.
- `docs/documentation-style-guide.md` for document structure, ADR shape, and
  the Compatibility and Migration section convention.
- `docs/execplans/5-2-1-define-ownership-models.md` and
  `docs/execplans/5-2-2-migration-rules-for-existing-consumers.md` for the
  template and predecessor lineage.

Supporting practice references signposted by the roadmap task description:

- `docs/rust-testing-with-rstest-fixtures.md` for `rstest` unit-test style.
- `docs/rstest-bdd-users-guide.md` for behavioural-test style. No new
  behavioural tests are expected because the change is documentation-only.
- `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`, and
  `docs/localizable-rust-libraries-with-fluent.md` for supporting design and
  testing practice, only if implementation surfaces a code change.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for refactoring
  guidance, again only if implementation surfaces a code change.

External prior art used while drafting this plan:

- AWS Well-Architected Framework, Reliability Pillar,
  <https://docs.aws.amazon.com/wellarchitected/latest/reliability-pillar/>,
  for the canonical "hard dependency" and "soft dependency" terminology and
  for the rule that a soft-dependency failure can be compensated for by the
  consuming application while a hard-dependency failure cannot.
- Christopher Meiklejohn, *Resilient Microservice Applications, by Design,
  and without the Chaos*, PhD thesis, 2024, for the formal definition of soft
  dependency in terms of consumer compensation behaviour.
- AgentSkills "Skill Package Manifest" discussion,
  <https://github.com/agentskills/agentskills/discussions/210>, and Skilldex
  (arXiv:2604.16911), as prior art for the dependency-resolution shape of
  skill manifests. These remain reference material only; OrthoConfig models
  manifest paths, schema versions, command indexes, and validation rules
  rather than adopting any specific manifest format.
- Microsoft Bot Framework Solutions, "Skill Manifest" reference, and
  TrueFoundry "agent-skill manifest" documentation, as further prior art for
  command index and capability declaration. These are also reference material
  only.

The dependency-tier classifications must trace back to specific roadmap items
so the plan and the roadmap evolve together:

- Whole-CLI introspection: §6.1 (`6.1.1`, `6.1.2`).
- Agent-context IR: §6.2 (`6.2.1`, `6.2.2`, `6.2.3`).
- Strict vocabulary policy: §7.1 (`7.1.1`, `7.1.2`, `7.1.3`).
- Localized help generation: the existing `OrthoConfigDocs` and
  `OrthoConfigSubcommandDocs` contract together with §6.1.1, which finalises
  the recursive subcommand tree.
- Profiles: §6.7 of the design and §9.1 of the roadmap.
- Delivery and feedback: §6.8 of the design and §9.2 of the roadmap.
- Skill manifests: §3.4 of the design and §6.3 of the roadmap.
- Execution ledgers: §6.6 of the design and §9.3 of the roadmap.

## Plan of work

Stage A: approval and baseline. Stop until the user approves this plan. After
approval, check `git status --short --branch`, confirm the branch name, and
run the existing gates once to establish the starting state. Use `tee` for
logs in `/tmp` and do not run format, lint, or tests in parallel.

Stage B: introduce the dependency-tier vocabulary as a sibling §2.2 of
`docs/agent-native-cli-design.md`, leaving §2.1 unchanged except for a
one-sentence pointer at the end of §2.1 that names §2.2 as the authoritative
dependency-tier location. The new §2.2 ("Consumer dependency tier") must:

- Define "ship-time hard dependency" and "ship-time soft dependency" in plain
  language, anchored to consumer adaptation behaviour. Cite the AWS
  Well-Architected reliability pillar as prior art and add an explicit
  one-sentence note that this is a ship-time framing only: the same words
  must not be reused inside the project for runtime fallback, circuit
  breaking, or any other resilience pattern.
- State that the tier records contract dependency, not delivery state. An
  entry is hard or soft based on whether Weaver's generated command surface
  can ship without the OrthoConfig contract, irrespective of whether that
  contract is already shipped, in flight, or merely planned.
- Present a single dependency-tier matrix with the columns: capability, tier,
  roadmap item (number and title), consumer adaptation rule, replacement
  trigger. The matrix replaces prose lists so that drift surfaces in review.
  At minimum it must contain rows for:

  - Whole-CLI introspection, hard, roadmap §6.1 ("Populate subcommand
    metadata"), no local adaptation permitted, replacement trigger not
    applicable.
  - Strict vocabulary policy, hard, roadmap §7.1 ("Implement vocabulary
    policy"), no local adaptation permitted, replacement trigger not
    applicable.
  - Agent-context IR, hard, roadmap §6.2 ("Add compact agent-context
    output"), no local adaptation permitted, replacement trigger not
    applicable.
  - Localized help generation, hard, existing `OrthoConfigDocs` and
    `OrthoConfigSubcommandDocs` together with roadmap §6.1.1 ("Generate
    recursive `DocMetadata.subcommands` values"), no local adaptation
    permitted, replacement trigger not applicable.
  - Profiles, soft, roadmap §9.1 ("Profile contracts") and design §6.7,
    consumer may carry a `--profile` parsing adapter only, replace within
    the next consumer release once §9.1 ships and on shape conflict the
    OrthoConfig shape wins.
  - Delivery, soft, roadmap §9.2 ("Delivery and feedback contracts") and
    design §6.8, consumer may carry a `--deliver` parsing adapter for
    `stdout`, `file:<path>`, and `webhook:<url>` only, replace within the
    next consumer release once §9.2 ships and on shape conflict the
    OrthoConfig shape wins.
  - Feedback, soft, roadmap §9.2 and design §6.8, consumer may carry a
    `feedback <text>` parsing adapter that writes local JSONL only, replace
    within the next consumer release once §9.2 ships and on shape conflict
    the OrthoConfig shape wins.
  - Skill manifests, soft, roadmap §6.3 ("Validate skill manifests against
    real commands") and design §3.4, consumer may carry a local manifest
    parser only, replace within the next consumer release once §6.3 ships
    and on shape conflict the OrthoConfig shape wins.
  - Execution ledgers, soft, roadmap §9.3 ("Execution ledger contracts") and
    design §6.6, consumer may carry a local JSONL ledger record format only,
    replace within the next consumer release once §9.3 ships and on shape
    conflict the OrthoConfig shape wins.

- Reinforce that Weaver and Netsuke continue to own semantic execution and
  build and package execution respectively, and that no dependency tier
  permits consumers to fork the agent-context schema, documentation IR, or
  policy-report schema permanently.

Stage C: propagate cross-references only. No other document restates the
matrix or duplicates the normative tier definitions. Each affected document
gets a one-sentence back-reference to `docs/agent-native-cli-design.md` §2.2
as the authoritative location, plus the smallest domain-appropriate framing
needed to make the back-reference legible.

- `docs/design.md` §2 ("Reusable consumer contracts") and §8 ("Future Work")
  acquire a one-sentence back-reference pointing to §2.2 of the agent-native
  design document. No new architectural claim is introduced and no list is
  duplicated.
- `docs/cargo-orthohelp-design.md` §0.1 or §12 acquires a one-sentence
  back-reference and a parallel one-sentence statement that the existing
  format compatibility surfaces are unchanged. No matrix is restated.
- `docs/users-guide.md` "Documentation metadata (OrthoConfigDocs)" acquires
  a one-sentence back-reference and a one-sentence downstream guidance line
  that human-facing consumers may continue to use the existing roff and
  PowerShell outputs without engaging with the dependency tier.
- `docs/developers-guide.md` "Schema ownership" gains a short subsection
  describing the dependency tier as internal practice for contributors:
  changes that affect a hard-dependency capability must update both §2.2
  and the cited roadmap item in the same change; changes that affect a
  soft-dependency capability must also record which roadmap item the local
  adapter shadows so the eventual replacement can be tracked. This is the
  "shadowed contracts" pointer the review recommended.
- `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`
  acquires a single back-reference sentence in its "Consequences" section
  that names §2.2 as the authoritative dependency-tier location. The ADR's
  decision and rationale do not change.

Stage D: review and harden. Run `make fmt` if Markdown formatting changes are
needed. Then run `make check-fmt`, `make lint`, `make test`, `make typecheck`,
and `make markdownlint` sequentially with `tee`. Run `make nixie` only if a
Mermaid diagram is added or edited (none is anticipated). Run
`coderabbit review --agent`, address every concern that fits within this
plan's constraints, and rerun affected gates. If CodeRabbit asks for work
outside tolerance, record it in this plan and escalate.

Stage E: commit and completion. Commit the implementation only after gates
and review pass. Mark roadmap item 5.2.3 in `docs/roadmap.md` done only after
the approved implementation is complete. Push the branch to
`origin/5-2-3-record-consumer-dependency-boundaries` and open or update a
draft pull request whose title includes `(5.2.3)`.

## Concrete steps

Run all commands from the repository root:

```sh
pwd
```

Expected output ends with:

```plaintext
/home/leynos/.lody/repos/github---leynos---ortho-config/worktrees/50d0a4ac-c6e3-4ee3-abad-125b5521c1e5
```

After approval, establish the baseline:

```sh
git status --short --branch
make check-fmt 2>&1 |
  tee "/tmp/check-fmt-ortho-config-$(git branch --show-current).out"
make lint 2>&1 |
  tee "/tmp/lint-ortho-config-$(git branch --show-current).out"
make test 2>&1 |
  tee "/tmp/test-ortho-config-$(git branch --show-current).out"
make typecheck 2>&1 |
  tee "/tmp/typecheck-ortho-config-$(git branch --show-current).out"
make markdownlint 2>&1 |
  tee "/tmp/markdownlint-ortho-config-$(git branch --show-current).out"
```

Expected result: each Make target exits with status 0. If a log is truncated
in the terminal, inspect the matching file under `/tmp`.

Inspect the documents that will be edited before changing them:

```sh
sed -n '40,90p' docs/agent-native-cli-design.md
sed -n '50,80p' docs/design.md
sed -n '30,70p' docs/cargo-orthohelp-design.md
sed -n '1240,1280p' docs/users-guide.md
sed -n '50,110p' docs/developers-guide.md
sed -n '60,110p' docs/adr-003-define-schema-ownership-for-agent-native-contracts.md
```

Update documentation in the order described in Stage B and Stage C. After
each substantive edit, rerun the relevant gates:

```sh
make check-fmt 2>&1 |
  tee "/tmp/check-fmt-ortho-config-$(git branch --show-current).out"
make markdownlint 2>&1 |
  tee "/tmp/markdownlint-ortho-config-$(git branch --show-current).out"
make typecheck 2>&1 |
  tee "/tmp/typecheck-ortho-config-$(git branch --show-current).out"
make lint 2>&1 |
  tee "/tmp/lint-ortho-config-$(git branch --show-current).out"
make test 2>&1 |
  tee "/tmp/test-ortho-config-$(git branch --show-current).out"
```

Run CodeRabbit only after the gates pass:

```sh
coderabbit review --agent 2>&1 |
  tee "/tmp/coderabbit-ortho-config-$(git branch --show-current).out"
```

If CodeRabbit reports concerns within scope, address them, rerun affected
gates, and repeat the review. If concerns fall outside the plan's constraints
or tolerances, record them in `Decision Log` and escalate.

Commit with a file-based message once gates and review pass:

```sh
git status --short
git diff -- docs
git add docs/agent-native-cli-design.md docs/design.md \
  docs/cargo-orthohelp-design.md docs/users-guide.md \
  docs/developers-guide.md \
  docs/adr-003-define-schema-ownership-for-agent-native-contracts.md \
  docs/roadmap.md \
  docs/execplans/5-2-3-record-consumer-dependency-boundaries.md
git diff --cached
COMMIT_MSG_DIR=$(mktemp -d)
cat > "$COMMIT_MSG_DIR/COMMIT_MSG.md" << 'ENDOFMSG'
Record consumer dependency boundaries for Weaver and Netsuke

Document the hard and soft dependency tiers between OrthoConfig and
its first downstream consumers so a maintainer can read which
agent-native capabilities Weaver's generated command surface cannot
ship without, and which capabilities a consumer may temporarily adapt
locally while OrthoConfig support lands.
ENDOFMSG
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

Before creating the pull request:

```sh
echo "${LODY_SESSION_ID}"
git push -u origin 5-2-3-record-consumer-dependency-boundaries
```

Create or update a draft pull request titled:

```plaintext
Record consumer dependency boundaries for Weaver and Netsuke (5.2.3)
```

The pull-request body must mention this ExecPlan and include:

```markdown
## References

- ExecPlan: docs/execplans/5-2-3-record-consumer-dependency-boundaries.md
- Lody session: https://lody.ai/leynos/sessions/${LODY_SESSION_ID}
```

## Validation and acceptance

The implementation is accepted when all of the following are true:

- `docs/agent-native-cli-design.md` contains a new §2.2 ("Consumer dependency
  tier"). §2.1 is unchanged except for a single sentence at its end pointing
  to §2.2 as the authoritative dependency-tier location.
- §2.2 defines "ship-time hard dependency" and "ship-time soft dependency" in
  plain language, anchors the terms in AWS Well-Architected prior art, and
  explicitly forbids reuse of the same words for runtime resilience inside
  the project. §2.2 also states that the tier records contract dependency,
  not delivery state.
- §2.2 presents a single dependency-tier matrix with the columns capability,
  tier, roadmap item (number and title), consumer adaptation rule, and
  replacement trigger. The matrix contains the four hard-dependency rows
  (whole-CLI introspection, strict vocabulary policy, agent-context IR,
  localized help generation) and the five soft-dependency rows (profiles,
  delivery, feedback, skill manifests, execution ledgers) named by the
  roadmap.
- Every matrix row references the roadmap item that delivers the capability
  by both number and title, so renumbering surfaces in review.
- §2.2 records the conflict-resolution rule: when OrthoConfig publishes the
  reusable contract for a soft-dependency capability, the consumer must
  replace its local adapter within the next consumer release, and if the
  consumer adapter and the published OrthoConfig shape disagree, the
  published shape wins.
- `docs/design.md`, `docs/cargo-orthohelp-design.md`, `docs/users-guide.md`,
  `docs/developers-guide.md`, and ADR-003 each carry only a back-reference
  to §2.2. None duplicates the matrix or restates the normative tier
  definitions. `docs/developers-guide.md` additionally records the
  "shadowed contracts" pointer for soft-dependency adapters.
- The existing schema ownership boundary in ADR-003 and the migration rules
  recorded by roadmap item 5.2.2 are unchanged.
- No production Rust code or `cargo-orthohelp` command-line behaviour
  changed.
- New tests are added only if the implementation surfaces a code change. The
  task description signposts `rstest`, `rstest-bdd`, `proptest`, `kani`, and
  `verus`; none is added unless implementation introduces a real invariant
  over a range of inputs, states, orderings, or transitions, in line with the
  predecessor 5.2.1 and 5.2.2 plans.
- `docs/roadmap.md` marks 5.2.3 done only after implementation is complete.
- `make check-fmt`, `make lint`, `make test`, `make typecheck`, and
  `make markdownlint` pass.
- `coderabbit review --agent` has no unresolved concerns within this plan's
  scope.
- The branch is renamed to
  `5-2-3-record-consumer-dependency-boundaries`, pushed with upstream
  tracking, and a draft pull request whose title contains `(5.2.3)` is
  open with the Lody session link in the References section.

## Idempotence and recovery

The documentation edits are additive and safe to rerun. If a gate fails after
an edit, use the matching `/tmp` log to identify the failure, apply the
smallest fix, and rerun the failed gate before continuing. If the same gate
fails twice, stop and escalate under the tolerances above.

If `make fmt` changes unrelated Markdown or Rust files, inspect the diff
before staging. Do not commit unrelated user changes. If unrelated files are
already dirty, leave them unstaged and record the situation in `Decision
Log`.

If the branch push fails because the remote branch already exists, inspect
the remote state with
`git ls-remote --heads origin 5-2-3-record-consumer-dependency-boundaries`
and escalate before overwriting anything.

If a CodeRabbit run produces concerns that would require violating a
constraint or tolerance, stop and escalate rather than relaxing the
constraint to clear the review.

## Artefacts and notes

Reconnaissance facts collected before drafting:

- `docs/agent-native-cli-design.md` §2.1 (lines 48 to 75) is the canonical
  boundary section and already names Weaver and Netsuke explicitly. It is
  the right location for the new "Consumer dependency tier" subsection.
- `docs/design.md` §2 lines 56 to 69 already record the reusable consumer
  contracts and the stable migration boundary that the new tiers will sit
  alongside.
- `docs/cargo-orthohelp-design.md` §0.1 lines 47 to 50 already cite Weaver
  and Netsuke as consumers of generic metadata, including renderer policy,
  JSON mode contracts, exit-code classes, skill manifests, capability
  provenance, profile redaction, delivery and feedback parsers, and
  configurable execution ledgers. This list maps cleanly onto the soft
  dependency tier.
- `docs/developers-guide.md` "Schema ownership" already records the internal
  practice for adding metadata fields without inferring defaults from
  command names. The new subsection extends that practice with the
  dependency tier and roadmap citation rule.
- `docs/users-guide.md` "Documentation metadata (OrthoConfigDocs)" already
  states that human-facing consumers can keep using existing outputs without
  adopting agent-context metadata. The new sentence reinforces this by
  pointing to the consumer dependency tier.
- The repository Makefile defines `check-fmt`, `lint`, `test`, `typecheck`,
  `markdownlint`, `nixie`, `build`, `release`, and supporting targets via a
  single `.PHONY` declaration. The 5.2.2 validation strategy applies without
  change.

Firecrawl prior-art findings:

- AWS Well-Architected reliability pillar uses "hard dependency" and "soft
  dependency" with the same intent: a soft-dependency failure can be
  compensated for by the consuming application; a hard-dependency failure
  cannot. This anchors the vocabulary used in the new subsection.
- Christopher Meiklejohn's PhD thesis (2024) provides a formal definition of
  soft dependency in terms of consumer compensation behaviour.
- Skilldex (arXiv:2604.16911), the AgentSkills "Skill Package Manifest"
  proposal, Microsoft Bot Framework skills, and TrueFoundry's agent-skill
  manifest documentation provide prior art for the skill-manifest contract.
  They remain reference material only; OrthoConfig owns its own manifest
  metadata shape.

## Interfaces and dependencies

No new public Rust API surface is planned. No external crate, tooling, or
service dependency is required.

The existing reusable contracts and their owners remain authoritative and are
not modified by this plan:

- `ortho_config::docs::OrthoConfigDocs` and `DocMetadata` with
  `ORTHO_DOCS_IR_VERSION` for the localized human documentation IR.
- `ortho_config::agent_context::AgentContext`, `AgentCommand`,
  `InteractionMode`, `MutationEffect`, and
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` for compact agent invocation
  metadata.
- `cargo_orthohelp::policy::PolicyReport`, `PolicySummary`, and
  `ORTHO_POLICY_REPORT_SCHEMA_VERSION` for warnings and hard failures
  emitted by `cargo-orthohelp`.

The dependency-tier paragraphs name these contracts by full path the first
time they are introduced so a reader can navigate from the boundary
documentation directly to the owning crate without re-reading ADR-003.

## Revision note

2026-06-02: Initial draft created from roadmap item 5.2.3, repository
reconnaissance through an Explore subagent, Firecrawl prior-art research on
the AWS hard and soft dependency vocabulary and agent skill manifest
ecosystem, and the predecessor 5.2.1 and 5.2.2 ExecPlans. The plan is
pre-implementation and requires explicit user approval before any feature
implementation begins.

2026-06-02: Revised after a logisphere community-of-experts review. The
dependency-tier content moved from a §2.1 subsection to a sibling §2.2
("Consumer dependency tier") to avoid colliding with the ownership boundary
in §2.1 and the legacy defaulting table in §8.1. The terms "hard dependency"
and "soft dependency" are now qualified as "ship-time" with an explicit
prohibition on reuse for runtime resilience. A conflict-resolution rule was
added stating that when OrthoConfig publishes the reusable contract the
published shape wins on disagreement, and that the consumer must replace its
local adapter within the next consumer release. A "shadowed contracts"
practice was added for `docs/developers-guide.md`. The §2.2 content is now
expressed as a single matrix (capability, tier, roadmap item with title,
consumer adaptation rule, replacement trigger) so renumbering and contract
drift surface mechanically in review. The Decision Log now records the
considered options A through D. ADR-003 is amended only with a single
back-reference sentence; §2.2 wins on any future disagreement.
