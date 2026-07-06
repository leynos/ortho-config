# Add skill manifest metadata to agent context

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: APPROVED

This plan covers roadmap item 6.3.1 only. It does not implement skill manifest
validation, parsing, prose loading, or any new `cargo-orthohelp` flag; those
belong to roadmap items 6.3.2 and later. The plan must be reviewed and
explicitly approved before any code changes are made.

## Purpose / big picture

Phase 6.3 of the active roadmap (see `docs/roadmap.md` §6.3) covers skill
manifest contracts. Item 6.3.1 is the first step in that subphase: model the
reusable metadata that lets a downstream application say "this command surface
ships a skill manifest at this path, declaring this manifest schema version,
and referencing these commands and flags". The reference design names the
requirements explicitly:

- `docs/agent-native-cli-design.md` §3.4 ("Long-form workflow material") states
  that OrthoConfig "should model the manifest path, schema version, command
  index, and validation rules that prove a manifest mentions real commands and
  flags. It must not own a downstream skill's domain prose, such as Weaver's
  safe Rust rename workflow or Netsuke's build workflow";
- `docs/agent-native-cli-design.md` §8.1 lists the `skill_manifest_paths`
  defaults row (default `[]`) as a compatibility-defaulting promise that
  agent-context readers must honour;
- `docs/agent-native-cli-design.md` §9 lists skill manifest validation among
  the "Current gaps to resolve" before downstream applications can rely on
  OrthoConfig as a contract anchor.

After this plan is approved and implemented, a maintainer working in a
downstream consumer crate should be able to:

1. construct an `ortho_config::agent_context::AgentContext` value and populate
   a new `skill_manifests: Vec<SkillManifest>` field with one or more
   descriptors, each carrying a stable identifier, the manifest's filesystem
   path, its declared manifest schema version, and a flat command index;
2. round-trip that `AgentContext` through `serde_json` and observe that legacy
   agent-context JSON without the new field still deserializes (the field
   defaults to the empty vector, matching the documented default);
3. read `docs/agent-native-cli-design.md` §3.4 and §8.1, `docs/users-guide.md`,
   and `docs/developers-guide.md` and tell exactly which crate owns the new
   types, which fields are wire-stable, and which behaviour is deferred to
   roadmap item 6.3.2.

Observable success is checked by:

- new `rstest` unit tests in `ortho_config/src/agent_context/tests.rs` covering
  the new types, the absent-field default, the camino path round-trip, and an
  updated inline `insta` snapshot of the full `AgentContext` wire shape;
- existing tests in `ortho_config/src/agent_context/tests.rs` continuing to
  pass after the additive change;
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all passing at the close of each milestone; and
- `coderabbit review --agent` returning clean (or with all concerns resolved)
  before each milestone is marked done.

### Dependency on roadmap item 6.2.1

Roadmap item 6.2.1 ("Add `--format agent-context` to `cargo-orthohelp`") is
listed as a prerequisite for 6.3.1. As of 2026-06-12 it has landed on `main`
via pull request 342: `cargo orthohelp --format agent-context` now emits an
`agent-context.json` document from the bridge IR, and 6.2.1 added an additive
`summary: Option<String>` field to `AgentCommand`. The prerequisite is
therefore satisfied, and the consumer that will read the new `skill_manifests`
field already exists.

This plan remains a passive-schema change. It adds reusable types to
`ortho_config::agent_context` (the module introduced by roadmap item 5.2.1 and
pull request 325) plus one additive `#[serde(default)]` field on
`AgentContext`. Because the field is defaulted, the existing 6.2.1 generator
keeps producing valid output without modification, and a later generator change
can populate the field when an application declares skill manifests. The 5.2.1
plan (`docs/execplans/5-2-1-define-ownership-models.md`) established this
passive-schema pattern when it introduced `AgentContext`, `AgentCommand`, and
the surround types ahead of the generator.

This plan does not modify the 6.2.1 generator. Wiring the new field into
`cargo orthohelp --format agent-context` output, and validating that a skill
manifest references real commands and flags, are deferred to roadmap item 6.3.2.

This plan does not change `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`,
`ORTHO_DOCS_IR_VERSION`, or `ORTHO_POLICY_REPORT_SCHEMA_VERSION`. It does not
add a new external crate dependency; `camino = "1"` is already declared in
`ortho_config/Cargo.toml:26`. It does not alter `cargo-orthohelp` behaviour or
add a CLI flag.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violating any of them requires escalation in `Decision Log`, not a
workaround.

- Do not implement code, tests, examples, or documentation in this branch
  until this ExecPlan is explicitly approved by the maintainer. A "DRAFT" plan
  must remain a planning artefact only.
- Keep this work focused on roadmap item 6.3.1 ("Add skill manifest
  metadata"). Skill manifest validation, prose parsing, command-reference
  resolution, and any `cargo-orthohelp` policy emission belong to roadmap item
  6.3.2 and later. If partial coverage of 6.3.2 falls out of 6.3.1 work, mark
  it clearly and stop for separate approval before extending it.
- Do not change `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`. The change is additive
  and carries an explicit `#[serde(default)]`, which is the legacy-defaulting
  rule defined in `docs/agent-native-cli-design.md` §8.1.
- Do not rename, remove, or change the wire shape of any existing field on
  `AgentContext`, `AgentCommand`, `AgentInput`, `AgentExample`,
  `SupportDeclaration`, `AgentPolicy`, `PolicyMode`, `InteractionMode`,
  `MutationEffect`, `AsyncSubmission`, `AsyncSubmissionMode`, `DeliveryRoute`,
  or `PaginationContract`.
- Preserve the boundary established by
  `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`: the new
  types belong to the agent-context contract owned by
  `ortho_config::agent_context`. They must not introduce any dependency on
  `cargo-orthohelp`, the bridge, the documentation IR, or policy reports.
- Do not add code to `cargo-orthohelp/src/policy/`, `cargo-orthohelp/src/cli/`,
  or any other surface that would emit, parse, or validate skill manifests.
  Validation is roadmap item 6.3.2.
- Do not introduce a manifest-format parser or any structured reading of a
  downstream skill manifest body. The plan models *that a manifest exists* and
  *what commands it claims to reference*; it does not read prose.
- Do not add a new external crate dependency. `camino`, `serde`, and
  `serde_json` are already declared. `semver` is not needed.
- Keep every Rust file under 400 lines. `ortho_config/src/agent_context/mod.rs`
  is 254 lines today and gains roughly 45 lines under this plan;
  `ortho_config/src/agent_context/tests.rs` is 272 lines and gains roughly 60
  lines. Both stay well below the cap.
- Every Rust module must begin with a `//!` comment, and every new public type
  must carry a Rustdoc comment explaining its purpose and pointing at the
  agent-context schema.
- Use en-GB Oxford spelling and grammar in documentation and comments, except
  for external API names such as `color` or third-party identifiers. Do not use
  em-dashes.
- Follow `docs/documentation-style-guide.md`: wrap Markdown at 80 columns,
  give every fenced code block an explicit language identifier (use `plaintext`
  for non-code text), and respect ADR and design-document structure.
- Use `rstest` for unit tests and `insta` inline snapshots for wire-shape
  assertions, mirroring the established style in
  `ortho_config/src/agent_context/tests.rs`. Do not introduce `rstest-bdd`
  scenarios, end-to-end binary tests, `proptest`, `kani`, or `verus` for this
  work. The rationale is recorded in §"Validation plan" below.
- Run validation commands sequentially and capture output with `tee` into
  `/tmp` log files. Do not run format checks, lints, and tests in parallel
  (they share the same Cargo cache and the Makefile relies on serial
  execution). Use the filename template
  `/tmp/$ACTION-ortho-config-6-3-1-skill-manifest-metadata.out`.
- Do not mark roadmap item 6.3.1 complete in `docs/roadmap.md` until every
  validation gate in §"Validation and acceptance" has passed, CodeRabbit review
  is clean, the commit history is on `6-3-1-skill-manifest-metadata`, and the
  draft pull request has been moved out of draft state.

If satisfying the objective requires violating a constraint, stop, document the
conflict in `Decision Log`, and ask the maintainer for direction.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Approval: stop after drafting this plan and wait for explicit maintainer
  approval before any milestone other than Milestone 0 is started.
- Scope: stop if the implementation requires changes to more than 9 files or
  more than 300 net lines of code and documentation (excluding the inline
  `insta` snapshot updates on the agent-context wire snapshots, which are
  reviewed as a unit).
- Public API: stop if any existing public type, trait, constant, function, or
  derived attribute must be renamed or removed. Additive items (`SkillManifest`,
  `SkillCommandRef`, and the additive `AgentContext.skill_manifests` field) do
  not trip this tolerance. The documentation-only rename of the §8.1 defaults
  row from `skill_manifest_paths` to `skill_manifests` is intentional and
  recorded in `Decision Log`.
- Schema shape: stop if a reviewer prefers an alternative shape with material
  downstream consequences. Specific alternatives that must be considered if
  raised: bare `Vec<Utf8PathBuf>` (defer the command index entirely);
  `BTreeMap` keyed by command path instead of `Vec<SkillCommandRef>`; typed enum
  `SkillManifestFormat` instead of opaque `manifest_schema_version` string;
  `semver::Version` instead of `String`. Present trade-offs in `Decision Log`
  before proceeding.
- Wire-version: stop if anyone proposes bumping
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` to `"2"`. The change is additive with a
  defaulted optional field; the §8 compatibility policy explicitly permits this
  without a version bump.
- Dependencies: stop if any new crate, build script, generated file, or
  non-standard Cargo feature is required.
- Proof tooling: stop if a proposed addition of `kani`, `verus`, or
  `proptest` would add tooling without a substantive invariant. Round-trip
  serialization is one assertion long and is fully covered by `rstest` plus
  `insta`.
- Tests: stop if `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, or `make nixie` still fails after two focused fix
  attempts.
- Documentation: stop if `docs/agent-native-cli-design.md`,
  `docs/users-guide.md`, and `docs/developers-guide.md` cannot describe the
  same metadata surface without contradiction.
- Process: stop if branch rename, push, draft pull-request creation, or
  `coderabbit review --agent` fails in a way that might hide review feedback or
  leave the repository in an inconsistent state.
- Iteration: stop if a single milestone takes more than three focused
  sessions without observable progress on its acceptance criteria. Record the
  cause in `Surprises & Discoveries`.

Adjust these values only with explicit maintainer approval recorded in
`Decision Log`.

## Risks

Known uncertainties that might affect the plan. Each risk records severity,
likelihood, and mitigation. Update this section as work proceeds and as new
risks emerge.

- Risk: the `insta` snapshot
  `agent_context_json_snapshot_covers_wire_contract` in
  `ortho_config/src/agent_context/tests.rs:49-110` locks the full
  `AgentContext` wire shape. Adding `skill_manifests` requires updating that
  inline snapshot. If the snapshot update lands separately from the field
  addition, CI fails between the two commits. Severity: low. Likelihood:
  medium. Mitigation: update the inline `insta` snapshot in the same commit
  that adds the field. Verify with `cargo insta accept --check` (or manual
  review of the inline block) that no other snapshots move.
- Risk: wire-field naming drift. The §8.1 defaults table currently names the
  field `skill_manifest_paths`. If the documentation update lands without the
  code update, or vice versa, a future generator implementer will follow the
  doc and emit the wrong field name. Severity: medium. Likelihood: medium.
  Mitigation: Milestone 4 makes the documentation update a hard prerequisite
  for closing the roadmap row; review must reject a state where the doc and the
  code disagree.
- Risk: stable-identifier omission. If `SkillManifest` ships with only `path`
  as an identifier, validator findings in 6.3.2 will quote a path that is
  brittle across platforms (Windows case folding, symlinked paths, OCI registry
  sources). The Logisphere pre-mortem identified this as the most expensive
  future regret. Severity: medium. Likelihood: medium. Mitigation: add
  `id: String` to `SkillManifest` in 6.3.1, with documented stable-opaque
  semantics. The field is cheap to add now and expensive to retrofit. See
  §"Recommended design" below.
- Risk: `camino::Utf8PathBuf` rejects legacy non-UTF-8 manifest paths during
  deserialization. Severity: low. Likelihood: low. Manifests are
  application-authored repository-relative paths and have never been observed
  as non-UTF-8 in the agent-context audience. Mitigation: document the UTF-8
  requirement in the `SkillManifest::path` Rustdoc and in
  `docs/users-guide.md`. If a real consumer needs non-UTF-8 paths, that is a
  separate ADR.
- Risk: roadmap-dependency confusion. A future contributor reads
  `docs/roadmap.md` §6.3.1 ("Requires 6.2.1") and assumes the prerequisite is
  still outstanding. Severity: low (down from medium after 6.2.1 landed on
  2026-06-12). Likelihood: low. Mitigation: the plan's "Purpose / big picture"
  states that 6.2.1 has landed and that 6.3.1 is an additive, defaulted change
  that the existing generator tolerates without modification. Milestone 4 keeps
  the `docs/roadmap.md` §6.3.1 note that only generator output, deferred to
  6.3.2, depends on wiring the field through
  `cargo orthohelp --format agent-context`.
- Risk: scope creep into validation. A reviewer asks "while you are there,
  also check that referenced flags resolve against `AgentCommand.path`".
  Severity: high. Likelihood: medium. Mitigation: the Constraints section
  forbids touching `cargo-orthohelp/src/policy/` and forbids any check that
  compares `SkillCommandRef.path` to `AgentCommand.path`. That work is 6.3.2.
  Cite roadmap 6.3.2 explicitly when declining the suggestion.
- Risk: duplicate `SkillManifest` entries. The schema permits two entries to
  share the same `path` and the same `id`. Without a documented rule, 6.3.2
  will inherit an ambiguous data shape. Severity: low. Likelihood: medium.
  Mitigation: document in the `SkillManifest::id` Rustdoc that the identifier
  should be unique within one `AgentContext`. Validation belongs to 6.3.2;
  6.3.1 records the expectation only.
- Risk: forward-compatible flag references. A manifest declaring
  `flags: ["new-flag-shipped-in-1.5"]` against a 1.4 binary will look like a
  stale reference to a naive validator. Severity: low for 6.3.1. Likelihood:
  medium for 6.3.2. Mitigation: out of scope for 6.3.1; flag this in
  `Decision Log` as a 6.3.2 design input.
- Risk: re-export bloat in `ortho_config/src/lib.rs`. The
  `pub use agent_context::{...}` block at lines 61-65 currently lists ten names
  and grows to twelve. Severity: low. Likelihood: low. Mitigation: keep the
  additions alphabetised; if the formatting changes, defer to `cargo fmt`
  rather than hand-editing.
- Risk: `markdownlint` flags pre-existing 80-column violations in the §8.1
  table when the row is renamed. Severity: low. Likelihood: medium. The row at
  `docs/agent-native-cli-design.md:574-592` has long entries already.
  Mitigation: keep the new row's width close to the existing one; record any
  unrelated failures in `Surprises & Discoveries` with exact line references
  before escalating scope.
- Risk: `leta` may fail to provide Rust symbols if `rust-analyzer` does not
  start. Severity: low. Likelihood: medium. Mitigation: ensure `rust-analyzer`
  is installed; fall back to `Grep` and direct file inspection if `leta` is
  unavailable; record the limitation in `Surprises & Discoveries`.

## Skills and source signposts

The implementation must use these skills (loaded via the `Skill` tool)
deliberately:

- `rust-router`: route Rust-specific implementation questions to the smallest
  useful skill.
- `leta`: use as the default tool for Rust symbol navigation (`leta show`,
  `leta refs`, `leta grep`, `leta calls`); fall back to `Grep` only when
  `rust-analyzer` is unavailable.
- `execplans`: keep this plan up to date as work proceeds.
- `rust-types-and-apis`: design the `SkillManifest` and `SkillCommandRef`
  shapes and the small additional re-exports from `ortho_config`.
- `arch-crate-design`: keep the new types inside `ortho_config::agent_context`
  without leaking new dependencies into or out of the agent-context module.
- `hexagonal-architecture`: protect the schema types from any awareness of
  filesystem I/O, manifest parsing, or `cargo-orthohelp` adapters.
- `rust-types-and-apis` (for `Utf8PathBuf` vs `String`): prefer a typed UTF-8
  path over a stringly-typed one when the runtime already pulls in `camino`.
- `domain-cli-and-daemons`: confirm `cargo-orthohelp` stdout, stderr, exit
  codes, and machine-readable output stay stable (nothing should change).
- `rust-unused-code`: ensure the additive types do not produce `dead_code`
  warnings in any feature configuration.
- `rust-testing-with-rstest-fixtures`
  (`docs/rust-testing-with-rstest-fixtures.md`) and
  `docs/reliable-testing-in-rust-via-dependency-injection.md` for unit test
  patterns. The new tests follow the existing `agent_context/tests.rs` style:
  `#[rstest]` cases, `serde_json` round-trip helpers, inline `insta` snapshots.
- `commit-message`: write file-based commit messages following the
  established conventions.
- `pr-creation`: draft the pull request description, including the
  `(6.3.1)` tag, the execplan reference, and the lody session link.
- `en-gb-oxendict` for documentation spelling and grammar.

The implementation must review and keep aligned with:

- `docs/roadmap.md` (especially §6.3.1, §6.3.2, and §6.2);
- `docs/agent-native-cli-design.md` (§3.4, §6.2.1 of the design's renderer
  metadata section, §8.1 defaults table, §9 "Current gaps to resolve");
- `docs/cargo-orthohelp-design.md` (skim only; this plan does not change
  `cargo-orthohelp` behaviour);
- `docs/users-guide.md` "Documentation and agent contracts" (around line 196)
  and the surround;
- `docs/developers-guide.md` "Schema ownership" (around line 51);
- `docs/documentation-style-guide.md` for Markdown wrapping, fenced code
  block languages, and en-GB Oxford spelling;
- `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`, the
  governing ownership boundary;
- `docs/execplans/5-2-1-define-ownership-models.md` for the pattern this
  plan extends (passive schema types, no CLI output, deferred behavioural
  coverage);
- `docs/execplans/6-1-1-recursive-doc-metadata-subcommands-values.md` for
  the milestone, risk, and validation patterns used here;
- `docs/contents.md` if this plan or any new ADR needs to be linked.

External prior art checked during planning (firecrawl, 2026-06-02):

- Anthropic Claude Skills (`SKILL.md` in a per-skill directory; no
  `schema_version` field; tool permissions expressed as a free-form
  `allowed-tools` list). Reference:
  <https://docs.claude.com/en/docs/agents-and-tools/agent-skills/overview>.
- Model Context Protocol tool and prompt listings, in which protocol version
  is negotiated at `initialize` and per-tool metadata uses `inputSchema` (JSON
  Schema) without a per-tool version field. References:
  <https://modelcontextprotocol.io/specification/2025-06-18/server/tools> and
  <https://modelcontextprotocol.io/specification/2025-06-18/server/prompts>.
- OpenAI plugin manifest (`ai-plugin.json`) introduced the
  `schema_version: "v1"` snake_case-and-`v`-prefix convention. The command
  index is indirect via a referenced OpenAPI document.
- Microsoft 365 Copilot plugin manifest (v2.1, v2.4) uses the same
  `schema_version` field and binds `functions[]` to OpenAPI `operationId`s via
  `runtimes[].run_for_functions`, the closest prior art for "manifest
  references commands and a validator proves the references resolve".
- VS Code chat participants use
  `package.json contributes.chatParticipants[].commands[]`; Fig autocomplete
  uses per-CLI TypeScript `Spec` objects with `subcommands[]` and `options[]`.
- Just, cargo-make, and Mise manifests do not version themselves as wire
  contracts; their tasks are name-keyed maps without agent-facing descriptions.

These sources informed the plan's shape, particularly the decision to keep
`manifest_schema_version` as an opaque `String` (OpenAI ships `"v1"`, which is
not semver; Copilot ships `"2.1"`, which is). They do not override repository
documents or require adopting any external format wholesale.

A Logisphere design review (2026-06-02) examined the proposal pre-mortem and
recommended:

- adding a stable `id: String` identifier separate from `path`, so 6.3.2's
  validator findings do not depend on path equality;
- documenting `SkillCommandRef.path` and `flags` as exact-match contracts
  against `AgentCommand.path` and `AgentInput.long`;
- reserving room in the `SkillManifest::path` Rustdoc for future
  non-filesystem manifest sources (e.g. OCI registries) without committing to a
  new variant yet.

All three recommendations are folded into §"Recommended design".

## Repository orientation

The relevant code is concentrated in a single module:

- `ortho_config/src/agent_context/mod.rs` (254 lines). This file declares
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`, `AGENT_CONTEXT_KIND_SUFFIX`, the
  `AgentContext` struct (`commands`, `profiles`, `feedback`, `policy`), and the
  surround types (`AgentCommand`, `AgentInput`, `AgentExample`,
  `AsyncSubmission`, `AsyncSubmissionMode`, `DeliveryRoute`,
  `PaginationContract`, `SupportDeclaration`, `AgentPolicy`, `PolicyMode`,
  `InteractionMode`, `MutationEffect`). Roadmap item 6.2.1 added an additive
  `summary: Option<String>` field to `AgentCommand`; the file remains a flat
  schema-types module appropriate for two more additive types.
- `ortho_config/src/agent_context/tests.rs` (272 lines). It uses `rstest`
  fixtures and inline `insta` snapshots
  (`assert_snapshot!(json, @r###"..."###)`) to lock the wire contract. The
  fixture helper `sample_agent_context` builds a fully populated `AgentContext`
  value; the inline snapshot `agent_context_json_snapshot_covers_wire_contract`
  records the canonical JSON, which now includes the 6.2.1
  `AgentCommand.summary` field. Milestone 2 and Milestone 3 must extend that
  snapshot from its current 6.2.1 shape rather than the pre-6.2.1 shape.
- `ortho_config/src/lib.rs:61-65`. The
  `pub use agent_context::{...}` block re-exports the public agent-context
  types. Two new names (`SkillManifest`, `SkillCommandRef`) join the block.
- `ortho_config/Cargo.toml:26`. `camino = "1"` is already a runtime
  dependency, so `Utf8PathBuf` is available without any change to `Cargo.toml`.

The relevant documents are:

- `docs/agent-native-cli-design.md` §3.4 (skill manifests as first-class
  contracts, lines 244-254);
- `docs/agent-native-cli-design.md` §8.1 (defaults table at lines 574-592,
  which includes the `skill_manifest_paths` row at line 588);
- `docs/agent-native-cli-design.md` §9 (gaps to resolve, lines 631-647, which
  includes "skill manifest validation");
- `docs/users-guide.md` "Documentation and agent contracts" section
  (around line 196);
- `docs/developers-guide.md` "Schema ownership" section
  (around line 51, citing ADR-003);
- `docs/roadmap.md` §6.3.1 (lines 157-162 of the roadmap as of this writing).

`cargo-orthohelp/src/policy/`, `cargo-orthohelp/src/cli/`,
`cargo-orthohelp/src/bridge.rs`, and `cargo-orthohelp/src/main.rs` are
intentionally not touched by this plan.

## Recommended design

The implementation must record these decisions, subject to plan approval. The
decisions point inward: the types live in `ortho_config::agent_context`; no
adapter, parser, or validator is added; the wire contract grows by one field
with a documented default.

### A pair of additive types

Declare in `ortho_config/src/agent_context/mod.rs`, after the existing
`PaginationContract` and before `#[cfg(test)] mod tests;`:

```rust
use camino::Utf8PathBuf;

/// Descriptor for a downstream skill manifest referenced by this command
/// surface.
///
/// Skill manifests live in the application's repository, not in OrthoConfig.
/// This descriptor records *that* a manifest exists, *which*
/// downstream-owned schema version it declares, and *which* commands and
/// flags it claims to reference. Validating those claims against the real
/// command tree is performed by `cargo-orthohelp` in a later roadmap item
/// (6.3.2); OrthoConfig only models the contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillManifest {
    /// Stable opaque identifier for the manifest within this
    /// [`AgentContext`]. Validator findings should quote this value rather
    /// than the filesystem path so that diagnostics survive path
    /// canonicalization differences between platforms. Should be unique
    /// inside one `AgentContext`; duplicate identifiers are permitted by
    /// this schema but are expected to be diagnosed by later validation
    /// work.
    pub id: String,
    /// Filesystem path to the manifest, relative to the application's
    /// repository root.
    ///
    /// This schema version (`ORTHO_AGENT_CONTEXT_SCHEMA_VERSION = "1"`)
    /// only models local filesystem manifests. Future schema versions may
    /// describe other sources (for example container registry images or
    /// inline manifests); 6.3.1 does not commit to any such variant.
    pub path: Utf8PathBuf,
    /// Version string declared by the downstream manifest format.
    ///
    /// OrthoConfig treats this value as opaque. Prior art is split between
    /// non-semver strings (such as OpenAI's `"v1"`) and semver-shaped
    /// strings (such as Microsoft Copilot's `"2.1"`); a single typed
    /// representation would reject one of these conventions. Downstream
    /// tools may parse the value to gate their own compatibility checks.
    pub manifest_schema_version: String,
    /// Command index entries: each entry declares one command path the
    /// manifest references, together with the flag names it depends on.
    ///
    /// Defaults to the empty vector so older agent-context payloads remain
    /// readable. An empty vector means "the manifest is present but
    /// declares no commands", which is a legitimate value for prose-only
    /// skills.
    #[serde(default)]
    pub commands: Vec<SkillCommandRef>,
}

/// One command-path-and-flags pair claimed by a skill manifest.
///
/// `path` must match [`AgentCommand::path`] exactly: a sequence of
/// invocation path segments such as `["cargo", "orthohelp"]`. `flags` must
/// match [`AgentInput::long`] exactly: long flag names without the leading
/// `--`. These exact-match contracts let 6.3.2's validator compare
/// references against the real command tree without normalisation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillCommandRef {
    /// Invocation path. Matches [`AgentCommand::path`] exactly.
    pub path: Vec<String>,
    /// Long flag names referenced by the manifest, without the leading
    /// `--`. Matches [`AgentInput::long`] exactly.
    #[serde(default)]
    pub flags: Vec<String>,
}
```

Extend `AgentContext` so the new field sits alongside the existing
support-declaration block:

```rust
pub struct AgentContext {
    // ... existing fields unchanged ...
    /// Skill manifests linked to this command surface.
    ///
    /// Defaults to the empty list. The defaulting rule matches the legacy
    /// compatibility table in `docs/agent-native-cli-design.md` §8.1.
    /// Validation against the real command tree is deferred to roadmap
    /// item 6.3.2.
    #[serde(default)]
    pub skill_manifests: Vec<SkillManifest>,
}
```

Update `AgentContext::new` so it initializes `skill_manifests: Vec::new()`.
Update the inline doctest on `AgentContext::new` to assert
`context.skill_manifests.is_empty()` alongside the existing assertions.

Re-export `SkillManifest` and `SkillCommandRef` from `ortho_config/src/lib.rs`
in the existing `pub use agent_context::{...}` block at lines 61-65. Keep the
block alphabetised.

### What the new types deliberately omit

- They do not own a manifest body. The downstream skill prose (Weaver's
  safe Rust rename workflow, Netsuke's build workflow) stays application-owned,
  as `docs/agent-native-cli-design.md` §3.4 requires.
- They do not contain a `kind`, `audience`, or `provider` tag. Such tags
  are application-domain metadata and belong in the manifest body, not in the
  agent-context schema.
- They do not contain inline content, only a path. Future schema versions
  may describe non-filesystem sources; this version does not.
- They do not promise that `id` is globally unique across applications.
  The contract is uniqueness within one `AgentContext` only.

### Two schema versions, disambiguated

`ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` continues to govern the agent-context wire
shape, including the new `skill_manifests` field. It stays at `"1"` because the
addition is additive and defaulted.

`SkillManifest.manifest_schema_version` is a per-entry string that describes
the *downstream* manifest format. OrthoConfig does not parse, compare, or
validate the value; it merely records what the application declared. Adding a
new `ORTHO_SKILL_MANIFEST_SCHEMA_VERSION` constant is not justified because
there is no third schema to version: there is only the agent-context schema,
into which an opaque-to-OrthoConfig version string is recorded.

### Wire-field name rename in the documentation

`docs/agent-native-cli-design.md` §8.1 currently names the row
`skill_manifest_paths`. This plan renames the row to `skill_manifests` so the
documented default matches the implemented field. No shipped consumer reads the
old name. The rename lands as part of Milestone 4 alongside the other
documentation updates and is logged in `Decision Log`.

### No new ADR (confirmed)

`docs/adr-003-define-schema-ownership-for-agent-native-contracts.md` already
authorizes additive evolution of the agent-context schema inside
`ortho_config::agent_context`. This plan cites ADR-003 in the design overview
and adds a single sentence to `docs/developers-guide.md` "Schema ownership"
noting that skill manifest descriptors are part of the agent-context contract,
not a sibling schema.

The maintainer confirmed on 2026-06-12 that referencing ADR-003 is sufficient
and that no new ADR is required for the wire-field rename. The open question is
therefore closed; Milestone 0 no longer needs to ask it.

### Out of scope

The following items are deliberately not addressed by this plan:

- skill manifest parsing or any structured reading of a manifest body;
- `cargo-orthohelp --format agent-context` (roadmap item 6.2.1);
- skill manifest validation against the real command tree (roadmap item
  6.3.2);
- emitting policy report entries for unresolved skill manifest references
  (roadmap item 6.3.2);
- non-filesystem manifest sources such as OCI registries or inline content;
- a stable identifier scheme spanning multiple `AgentContext` values.

## Planned implementation milestones

Each milestone ends with a validation gate. Do not begin the next milestone
until the previous one's gate is green and `coderabbit review --agent` is
clear. Commit at the end of each milestone (or more frequently if local
checkpoints are useful) with descriptive messages following
`docs/documentation-style-guide.md` and `AGENTS.md`.

### Milestone 0: approve plan, decide ADR question, refresh signposts

Goal: turn this plan into an approved-and-recorded design decision before
touching code; resolve the one open ADR question.

Steps:

1. Submit this ExecPlan for review and wait for explicit maintainer
   approval. Record the approval date in `Progress`. Update `Status:` to
   `APPROVED`.
2. The ADR question is closed: the maintainer confirmed on 2026-06-12 that
   referencing ADR-003 in `docs/developers-guide.md` is sufficient and that no
   new ADR is required for the wire-field rename. No ADR drafting is needed in
   this milestone.
3. Add this ExecPlan to `docs/contents.md` next to the other phase-6
   execplans so the plan is discoverable from the documentation index (already
   done as part of the draft commit).

Validation:

```sh
set -o pipefail
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-3-1-skill-manifest-metadata.out
make nixie 2>&1 \
  | tee /tmp/nixie-ortho-config-6-3-1-skill-manifest-metadata.out
```

Expected: both commands exit successfully. Unrelated pre-existing failures must
be recorded in `Surprises & Discoveries` before continuing.

Run `coderabbit review --agent` and clear concerns.

Acceptance: the plan is `APPROVED`, the ADR question is recorded in
`Decision Log`, the documentation index links the plan, and any pre-existing
documentation debt has been recorded rather than expanded.

### Milestone 1: introduce passive types

Goal: land `SkillManifest` and `SkillCommandRef` with no consumer wiring.

Steps:

1. Add `use camino::Utf8PathBuf;` at the top of
   `ortho_config/src/agent_context/mod.rs` (next to the existing
   `use serde::{Deserialize, Serialize};`).
2. Add the `SkillManifest` and `SkillCommandRef` definitions from
   §"Recommended design" between `PaginationContract` and
   `#[cfg(test)] mod tests;`. Include the full Rustdoc comments shown in
   §"Recommended design".
3. Re-export both names from `ortho_config/src/lib.rs` in the existing
   `pub use agent_context::{...}` block at lines 61-65. Keep the block
   alphabetised. Do not add a separate `pub use` line.
4. Do **not** touch `AgentContext` in this milestone. Do **not** add tests
   in this milestone. The milestone is intentionally tiny so a reviewer can
   verify "two new types added, no behaviour change, no test churn".

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-3-1-skill-manifest-metadata.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-3-1-skill-manifest-metadata.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-3-1-skill-manifest-metadata.out
```

Expected: all three commands exit successfully. Existing tests must continue to
pass because no consumer of `AgentContext` has changed.

Run `coderabbit review --agent` and clear concerns.

Acceptance: `SkillManifest` and `SkillCommandRef` exist in
`ortho_config::agent_context`, are re-exported from `ortho_config`, build
cleanly, and add no behavioural change.

### Milestone 2: link `skill_manifests` into `AgentContext`

Goal: connect the new types to the agent-context wire contract via a defaulted
optional field.

Steps:

1. Add `pub skill_manifests: Vec<SkillManifest>` to `AgentContext` with
   `#[serde(default)]`. Place it immediately after the `policy` field so the
   field order matches the wire snapshot's reading order.
2. Update `AgentContext::new`
   (`ortho_config/src/agent_context/mod.rs:50-63`) to set
   `skill_manifests: Vec::new()` alongside the existing default initializers.
3. Update the inline doctest on `AgentContext::new` (lines 42-49) to add
   `assert!(context.skill_manifests.is_empty());`. The existing assertions stay
   intact.
4. Update the inline `insta` snapshot in
   `ortho_config/src/agent_context/tests.rs::agent_context_json_snapshot_covers_wire_contract`
   so the snapshot includes `"skill_manifests": []` (or a populated list,
   depending on whether Milestone 3 also extends the fixture). Land the
   snapshot update in the same commit as the field addition.

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-3-1-skill-manifest-metadata.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-3-1-skill-manifest-metadata.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-3-1-skill-manifest-metadata.out
```

Expected: all three commands exit successfully. Specifically:

- `agent_context_json_snapshot_covers_wire_contract` passes against the
  updated inline snapshot;
- `absent_optional_metadata_deserializes_to_documented_defaults`
  (`agent_context/tests.rs:113-138`) continues to pass because the new field is
  defaulted;
- `new_context_uses_legacy_defaults`
  (`agent_context/tests.rs:18-29`) continues to pass and now also asserts the
  empty `skill_manifests` vector via the updated doctest.

Run `coderabbit review --agent` and clear concerns.

Acceptance: `AgentContext` now carries `skill_manifests`, the inline snapshot
reflects the new wire shape, and legacy payloads continue to deserialize
without errors.

### Milestone 3: add passive-schema tests

Goal: prove the new types' wire contract with `rstest` cases and `insta`
snapshots that follow the established style in
`ortho_config/src/agent_context/tests.rs`.

Steps:

1. Add `skill_manifest_default_is_empty_list` (`#[rstest]`): deserialize a
   payload that omits `skill_manifests`; assert the field reads as `Vec::new()`.
2. Add `skill_manifest_serialises_with_camino_path` (`#[rstest]`): build a
   `SkillManifest` with id `rename`, path `skills/rename.md`, manifest schema
   version `v1`, and one `SkillCommandRef` for `["weaver", "rename"]` using the
   `json` flag. Serialize it, assert that the JSON path field is the string
   `"skills/rename.md"`, and assert the inline `insta` snapshot of the full
   descriptor.
3. Add `skill_command_ref_defaults_flags_to_empty` (`#[rstest]`):
   deserialize a payload that omits `flags`; assert the field reads as
   `Vec::new()`.
4. Add `skill_manifest_required_fields_fail_deserialization`
   (`#[rstest]` with `#[case]` rows): cover missing `id`, missing `path`,
   missing `manifest_schema_version` payloads; assert each fails with a
   `serde_json::Error` whose `is_data()` or `is_syntax()` is true, in the style
   of `missing_required_top_level_fields_fail_deserialization`
   (`agent_context/tests.rs:140-153`).
5. Extend `sample_agent_context()` (`agent_context/tests.rs:182-221`) so
   the canonical fixture includes one `SkillManifest` entry with one
   `SkillCommandRef`. Update the
   `agent_context_json_snapshot_covers_wire_contract` inline snapshot
   accordingly. Keep the canonical example small (one manifest, one command
   ref, two flags at most) so the snapshot stays reviewable.

Do not add `rstest-bdd` scenarios, end-to-end tests, `proptest` cases, `kani`
harnesses, or `verus` proofs. The rationale is documented in §"Validation plan".

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-3-1-skill-manifest-metadata.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-3-1-skill-manifest-metadata.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-3-1-skill-manifest-metadata.out
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-3-1-skill-manifest-metadata.out
```

Expected: all four commands exit successfully. The new `rstest` cases pass; the
updated snapshot reflects the populated fixture; no other snapshots move.

Run `coderabbit review --agent` and clear concerns.

Acceptance: the wire contract for `SkillManifest`, `SkillCommandRef`, and the
new `AgentContext.skill_manifests` field is locked under unit tests plus inline
`insta` snapshots, matching the existing schema-test style.

### Milestone 4: documentation, changelog, roadmap close-out

Goal: bring documentation in line with the implementation and close the roadmap
entry.

Steps:

1. Update `docs/agent-native-cli-design.md` §3.4 (lines 244-254): keep the
   "first-class contracts" framing, but name the new types (`SkillManifest`,
   `SkillCommandRef`) and point at `ortho_config::agent_context`. Reiterate
   that downstream manifest prose stays application-owned.
2. Update `docs/agent-native-cli-design.md` §8.1 defaults table (lines
   574-592): rename the row from `skill_manifest_paths` to `skill_manifests`,
   keep the default `[]`, and adjust the rationale to read "Skill manifests are
   absent until declared; validation lands in roadmap item 6.3.2."
3. Update `docs/agent-native-cli-design.md` §9 ("Current gaps to resolve",
   lines 631-647): remove "skill manifest validation" from the omnibus bullet
   at line 645 and add a precise bullet noting that 6.3.1 modelled the metadata
   and that 6.3.2 still owes the validation work.
4. Update `docs/users-guide.md` "Documentation and agent contracts"
   subsection (around line 196): add a short paragraph naming `SkillManifest`,
   `SkillCommandRef`, and `AgentContext.skill_manifests` with a small JSON
   example showing the empty default and one populated entry.
5. Update `docs/developers-guide.md` "Schema ownership" section (around
   line 51): add one sentence stating that skill manifest descriptors are part
   of the agent-context contract owned by `ortho_config::agent_context`,
   governed by `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`, and that downstream
   manifest prose remains application-owned.
6. Update `CHANGELOG.md` "Unreleased / Added":
   "`SkillManifest`, `SkillCommandRef`, and the `AgentContext.skill_manifests`
   field for declaring downstream skill manifests in agent context (roadmap
   item 6.3.1)." Under "Unreleased / Changed (design)" (creating the heading if
   absent): "Renamed the agent-context defaulting-table row from
   `skill_manifest_paths` to `skill_manifests` to reflect that entries are
   structured descriptors rather than bare paths."
7. Update `docs/roadmap.md` §6.3.1 (lines 157-162): mark `[x] 6.3.1` and its
   three child bullets done; add a single sentence noting "Modelling can land
   before 6.2.1; only generator output depends on 6.2.1."
8. If a new ADR was drafted in Milestone 0, link it from
   `docs/contents.md` alongside ADR-003 and ADR-005.

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-3-1-skill-manifest-metadata.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-3-1-skill-manifest-metadata.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-3-1-skill-manifest-metadata.out
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-3-1-skill-manifest-metadata.out
make nixie 2>&1 \
  | tee /tmp/nixie-ortho-config-6-3-1-skill-manifest-metadata.out
```

Expected: all five commands exit successfully. Documentation matches the
implementation across `docs/agent-native-cli-design.md`, `docs/users-guide.md`,
`docs/developers-guide.md`, `CHANGELOG.md`, and `docs/roadmap.md`.

Run `coderabbit review --agent` and clear concerns.

Acceptance: the roadmap entry is closed, every doc references the new types
consistently, the draft PR is moved out of draft, and the change is ready to
land on `main`.

## Concrete steps

The following commands are the canonical operations a fresh agent should run.
They are deliberately idempotent: re-running them after a partial failure
recreates the same state without drift.

Repository orientation (read-only):

```sh
git fetch origin
git branch --show-current
ls docs/execplans/6-3-1-skill-manifest-metadata.md
```

Per-milestone validation (sequential, with `tee`):

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-3-1-skill-manifest-metadata.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-3-1-skill-manifest-metadata.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-3-1-skill-manifest-metadata.out
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-3-1-skill-manifest-metadata.out
make nixie 2>&1 \
  | tee /tmp/nixie-ortho-config-6-3-1-skill-manifest-metadata.out
```

Targeted schema test loop during Milestone 3:

```sh
cargo test -p ortho_config agent_context 2>&1 \
  | tee /tmp/test-agent-context-ortho-config-6-3-1-skill-manifest-metadata.out
```

Snapshot review during Milestone 2 and Milestone 3:

```sh
cargo insta accept --snapshot agent_context__fixture.json 2>&1 \
  | tee /tmp/insta-ortho-config-6-3-1-skill-manifest-metadata.out
```

The installed `cargo-insta` accepts pending snapshots through
`cargo insta accept`; it does not support the obsolete `accept --check` form.
Treat any unexpected pending snapshot as a Milestone failure and investigate
before accepting.

Post-milestone review:

```sh
coderabbit review --agent 2>&1 \
  | tee /tmp/coderabbit-ortho-config-6-3-1-skill-manifest-metadata.out
```

Clear every concern before moving to the next milestone.

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes; `agent_context_json_snapshot_covers_wire_contract`
  passes against the updated inline snapshot; new `rstest` cases pass;
  legacy-default and required-field tests still pass.
- Lint and format: `make check-fmt` and `make lint` pass with no new
  warnings.
- Documentation gates: `make markdownlint` and `make nixie` pass.
- Review: `coderabbit review --agent` is clear at the end of each
  milestone.
- Roadmap close-out: `docs/roadmap.md` §6.3.1 marked done with the three
  child bullets ticked.

Quality method (how we check):

- Run all five `make` gates sequentially as shown above, piping each into a
  separate `/tmp` log so truncated output can be inspected after the fact.
- Run `cargo insta accept --check` after Milestone 2 and Milestone 3 to
  guarantee no stray pending snapshots.
- Run `coderabbit review --agent` after each milestone and clear concerns
  before moving on.

### Test tiering rationale

- **`rstest` + `insta` unit tests**: appropriate. The new contract is a
  pair of `Serialize`/`Deserialize` types and a field on `AgentContext`.
  Round-trip, default, and snapshot tests cover the entire observable surface.
  This mirrors the existing pattern in
  `ortho_config/src/agent_context/tests.rs`.
- **`rstest-bdd` behavioural tests**: not applicable. There is no
  externally observable command behaviour to exercise until roadmap item 6.2.1
  lands `--format agent-context` and roadmap item 6.3.2 lands the validator.
  The 5.2.1 plan reached the same conclusion when adding `AgentContext` itself.
- **End-to-end tests**: not applicable. No binary output, persisted
  artefact, integration contract, stdout, stderr, or exit-code behaviour
  changes.
- **`proptest`**: not justified. Round-trip serialization is a single
  assertion that does not benefit from generated inputs; the schema does not
  introduce an invariant over a broad input domain.
- **`kani`**: not justified. There is no bounded state machine, dispatch
  table, or unsafe block in the new code.
- **`verus`**: not justified. No deductive lemma is needed for additive
  schema types.

If a reviewer requests one of the deferred tiers, treat the request as a
schema-shape escalation and present trade-offs in `Decision Log` before
proceeding.

## Idempotence and recovery

Every step in §"Concrete steps" is idempotent. Re-running a `make` target
re-validates the working tree without modifying it. Re-running
`cargo insta accept --check` does not rewrite committed snapshots.
Documentation edits use `Edit` operations that target specific lines so reruns
either no-op (when the edit has already been applied) or fail loudly (when the
underlying lines have moved). Branch operations are atomic via Git.

Recovery: if a milestone fails its validation gate, fix the smallest relevant
cause, update `Progress`, and re-run only the failed gate before re-running
later gates. If two focused fix attempts do not clear the gate, stop and
escalate per the Tolerances section.

## Interfaces and dependencies

The implementation must end with the following public surface in `ortho_config`:

```rust
pub use agent_context::{
    AGENT_CONTEXT_KIND_SUFFIX, AgentCommand, AgentContext, AgentExample,
    AgentInput, AgentPolicy, AsyncSubmission, AsyncSubmissionMode,
    DeliveryRoute, InteractionMode, MutationEffect,
    ORTHO_AGENT_CONTEXT_SCHEMA_VERSION, PaginationContract, PolicyMode,
    SkillCommandRef, SkillManifest, SupportDeclaration,
};
```

The two new names must implement
`Debug + Clone + Serialize + Deserialize + PartialEq + Eq`. The `AgentContext`
struct gains exactly one new public field,
`pub skill_manifests: Vec<SkillManifest>`, with `#[serde(default)]`. No
constructor signatures change. No other public types are modified.

The crate continues to depend on `camino = "1"` (already declared) and makes no
new crate-level additions.

## Progress

Use this list to summarize granular steps. Every stopping point must be
documented here, even if it requires splitting a partially completed task into
two.

- [x] (2026-06-02) Loaded `leta`, `rust-router`, and `execplans` skills for
  planning.
- [x] (2026-06-02) Created a leta workspace for this worktree.
- [x] (2026-06-02) Used Firecrawl to research prior art for Anthropic
  Claude Skills, Model Context Protocol tool listings, OpenAI plugin manifests,
  Microsoft 365 Copilot plugin manifests v2.1, VS Code chat participants, and
  Fig autocomplete specs.
- [x] (2026-06-02) Used the Plan agent to draft the milestone breakdown
  and Rust shape.
- [x] (2026-06-02) Ran the Logisphere design review and folded the three
  accepted recommendations (`id: String` field, exact-match contract comments on
  `SkillCommandRef`, room reserved for future non-filesystem sources) into
  §"Recommended design".
- [x] (2026-06-02) Drafted this ExecPlan for approval.
- [x] (2026-06-12) Maintainer confirmed the ADR question: referencing
  ADR-003 is sufficient, so no new ADR is required for the wire-field rename.
- [x] (2026-06-12) Received explicit maintainer approval to implement this
  ExecPlan via the request to proceed with implementation.
- [x] (2026-06-12) Confirmed this ExecPlan is already linked from
  `docs/contents.md`.
- [x] (2026-06-12) Rebased the branch onto `origin/main` after roadmap items
  6.2.1, 5.2.3, and 11.1.1 merged. Reconciled the plan with 6.2.1, which landed
  the agent-context generator and added `AgentCommand.summary`. Updated the
  dependency framing, line counts, and the wire-snapshot note.
- [x] (2026-06-12) Milestone 1: introduced `SkillManifest` and
  `SkillCommandRef`, enabled `camino`'s `serde1` feature for their typed path
  field, passed `make check-fmt`, `make lint`, `make test`, and received a
  zero-finding CodeRabbit review for the uncommitted milestone diff.
- [x] (2026-06-12) Milestone 2: linked `skill_manifests` into
  `AgentContext`, updated constructor defaults and the existing agent-context
  snapshots, passed `make check-fmt`, `make lint`, `make test`, confirmed no
  pending `insta` snapshots, and received a zero-finding CodeRabbit review.
- [x] (2026-06-12) Milestone 3: added passive-schema tests for absent-field
  defaults, camino path serialization, command-reference flag defaults, and
  required manifest fields. Extended the canonical unit fixture with one
  populated manifest, passed the focused
  `cargo test -p ortho_config agent_context` loop plus `make check-fmt`,
  `make lint`, `make test`, `make markdownlint`, and received a zero-finding
  CodeRabbit review.
- [x] (2026-06-12) Milestone 4: updated the design document, users' guide,
  developers' guide, changelog, roadmap, and this ExecPlan. Ran `make fmt`,
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie`; all deterministic gates passed before the milestone's
  zero-finding CodeRabbit review.
- [x] (2026-06-12) Moved pull request 344 out of draft after the implementation
  and documentation milestones passed their gates and CodeRabbit reviews.
- [x] (2026-06-12) Marked roadmap item 6.3.1 done in the branch after the
  implementation, documentation, and validation gates passed.

## Surprises & discoveries

Unexpected findings during planning that were not anticipated as risks.
Document with evidence so future work benefits.

- Observation: `docs/agent-native-cli-design.md` §8.1 names the default
  row `skill_manifest_paths`, but the design narrative throughout §3.4
  describes structured descriptors with paths, schema versions, and a command
  index. The two are inconsistent. Evidence:
  `docs/agent-native-cli-design.md:640` (row, after the 6.2.1 rebase) versus
  the §3.4 narrative. Impact: the plan renames the row to `skill_manifests` to
  match the structured shape and records the rename in `Decision Log`.
- Observation: roadmap item 6.2.1 merged on 2026-06-12 while this plan was a
  draft, landing the `cargo orthohelp --format agent-context` generator and
  adding an additive `summary: Option<String>` field to `AgentCommand`.
  Evidence: pull request 342 (commit `fc420c7`). Impact: 6.3.1's prerequisite
  is now satisfied; the plan's dependency framing, file line counts, and
  wire-snapshot note were updated during the rebase. The change does not
  conflict with the proposed `skill_manifests` field on `AgentContext`.
- Observation: Anthropic's Claude Skills format ships a free-form
  `allowed-tools` list rather than a structured command index. Evidence:
  Firecrawl scrape of
  <https://docs.claude.com/en/docs/agents-and-tools/agent-skills/overview>.
  Impact: borrow the directory-rooted manifest convention from Claude Skills
  but not the unstructured tool list, because OrthoConfig needs a structured
  index to support future validation.
- Observation: the OpenAI `schema_version: "v1"` convention and the
  Microsoft 365 Copilot `schema_version: "2.1"` convention disagree on whether
  the value is semver. A typed `semver::Version` would reject one of the two.
  Evidence: Firecrawl research summarized in §"Skills and source signposts".
  Impact: keep `manifest_schema_version` as an opaque `String`; defer parsing
  to 6.3.2 if comparison semantics are ever needed.
- Observation: the Logisphere pre-mortem identified that path equality is
  brittle across platforms (Windows case folding, symlinked paths) and across
  non-filesystem sources (OCI registries) that might appear in a future schema
  version. Evidence: review report 2026-06-02. Impact: the plan adds an
  `id: String` to `SkillManifest` so 6.3.2's validator findings can quote a
  stable identifier rather than a brittle path.
- Observation: Milestone 0 validation found a pre-existing Mermaid parse
  failure in `docs/rstest-bdd-v0-5-0-migration-guide.md` around the migration
  flowchart. Evidence: `make nixie` failed on diagram 1 with "Unterminated node
  label (missing `}`)" after `make markdownlint` passed. Impact: quote the
  labels that contain `#once` and `StepContext::insert_owned` so the
  documentation gate can validate before implementation proceeds.
- Observation: the first Milestone 0 `coderabbit review --agent`
  invocation reached sandbox preparation and then produced no further output
  for several minutes. Evidence:
  `/tmp/coderabbit-ortho-config-6-3-1-skill-manifest-metadata.out` contained
  only setup status. Impact: terminate only that review process and retry the
  same review with a bounded shell timeout so the milestone still receives
  CodeRabbit validation.
- Observation: Milestone 1 exposed that `camino::Utf8PathBuf` does not
  implement `Serialize` or `Deserialize` unless the existing `camino`
  dependency enables its `serde1` feature. Evidence: `make test` failed in the
  `cargo-orthohelp` golden agent-context test while rebuilding `ortho_config`
  through the bridge with trait-bound errors on `Utf8PathBuf`. Impact: enable
  the standard `serde1` feature on the existing `camino = "1"` dependency in
  `ortho_config/Cargo.toml` rather than adding a new crate or replacing the
  typed path with a string.
- Observation: Milestone 2 also moves the `cargo-orthohelp` generated
  agent-context golden snapshot because the generator serializes
  `AgentContext::new` and therefore emits the defaulted empty `skill_manifests`
  field. Evidence: `make test` produced
  `cargo-orthohelp/tests/golden/agent_context__fixture.json.snap.new` with only
  the added `"skill_manifests": []` field. Impact: update the committed golden
  snapshot in the same milestone as the field addition.
- Observation: the planned `cargo insta accept --check` command is not
  supported by the installed `cargo-insta`; it exits with "unexpected argument
  '--check'". Evidence:
  `/tmp/insta-ortho-config-6-3-1-skill-manifest-metadata.out`. Impact: replace
  the plan's snapshot command with targeted `cargo insta accept` usage and rely
  on `git diff` plus the test rerun to verify the accepted snapshot.

## Decision Log

Record every significant decision made while working on the plan. Include
decisions to escalate, decisions on ambiguous requirements, and design choices.

- Decision: treat this branch as a pre-implementation ExecPlan branch.
  Rationale: the user explicitly required plan approval before implementation.
- Decision: place the new types inside `ortho_config::agent_context` rather
  than a sibling `ortho_config::skill_manifest` module. Rationale: the audience
  for skill manifest metadata is the agent-context consumer, and `ADR-003`
  draws ownership boundaries at *contract audience*. A sibling module would
  create a redundant schema-version question and a second `pub use` block in
  `ortho_config/src/lib.rs` for what is effectively one field addition.
- Decision: keep `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` at `"1"`. Rationale:
  the §8 compatibility policy permits additive defaulted fields without a
  version bump; the 5.2.1 plan applied the same rule when introducing
  `profiles`, `feedback`, and `policy` on `AgentContext`.
- Decision: rename the §8.1 defaults row from `skill_manifest_paths` to
  `skill_manifests`. Rationale: the new field is a list of structured
  descriptors, not a list of bare paths; the name now reflects the shape. No
  shipped consumer reads the old name (the generator does not yet exist), so
  the rename is documentation-only.
- Decision: add `id: String` to `SkillManifest`. Rationale: the
  Logisphere pre-mortem showed that 6.3.2's validator findings cannot rely on
  path equality without breaking on Windows case folding, symlinks, and future
  non-filesystem sources. Adding the field now is cheap; retrofitting it later
  is a wire-shape change.
- Decision: keep `manifest_schema_version` as an opaque `String`.
  Rationale: prior art is split between non-semver strings (OpenAI's `"v1"`)
  and semver strings (Microsoft Copilot's `"2.1"`); a typed `semver::Version`
  would reject one of the two. OrthoConfig has no reason to parse the value in
  6.3.1.
- Decision: keep `SkillManifest::path` as a `camino::Utf8PathBuf`.
  Rationale: the runtime already pulls in `camino = "1"`, the project's
  filesystem code uses `camino` throughout, and `Utf8PathBuf` serializes as a
  plain JSON string. UTF-8 paths are the documented expectation.
- Decision: keep `SkillManifest::commands` as `Vec<SkillCommandRef>` with
  `#[serde(default)]`. Rationale: distinguishing absent from empty is what
  6.3.2's validator will be asked about first; an empty vector is a meaningful
  "prose-only skill" value, not malformed data. Make `commands` defaulted at
  the wire level so legacy payloads remain readable.
- Decision: do not write a new ADR for this work; reference ADR-003.
  Rationale: ADR-003 already authorizes additive evolution of the agent-context
  schema inside `ortho_config::agent_context`. Two new schema types fit that
  authorization. The maintainer confirmed on 2026-06-12 that referencing
  ADR-003 is sufficient and that no new ADR is required for the wire-field
  rename, closing the only open question.
- Decision: defer validation, parsing, and `cargo-orthohelp` integration
  to 6.3.2. Rationale: §3.4 of the design distinguishes "modelling" from
  "validation rules"; 6.3.1 is the modelling step and 6.3.2 is the validation
  step. Mixing them would breach the Constraints.
- Decision: do not add `rstest-bdd`, end-to-end, `proptest`, `kani`, or
  `verus` coverage for 6.3.1. Rationale: there is no observable command
  behaviour, no invariant over a broad input domain, and no bounded state
  machine to verify. The same conclusion was reached by the 5.2.1 plan for the
  same reason.
- Decision: proceed with implementation on 2026-06-12. Rationale: the
  maintainer explicitly requested implementation of this ExecPlan, which
  satisfies the approval gate and turns the plan from `DRAFT` to `APPROVED`.
- Decision: fix the pre-existing Mermaid syntax failure during Milestone 0
  rather than carrying it as known debt. Rationale: `make nixie` is a required
  gate for this plan, and the fix is syntax-only documentation maintenance
  outside the Rust implementation surface.
- Decision: retry CodeRabbit after a stalled invocation. Rationale: no
  review findings were emitted, no rate-limit response was reported, and the
  plan requires CodeRabbit review before moving to Milestone 1.
- Decision: enable `camino`'s standard `serde1` feature for
  `ortho_config`. Rationale: the approved schema uses `Utf8PathBuf` as a
  serialized field, `camino` already exists as a runtime dependency, and the
  feature is the crate-supported serde integration rather than a new dependency
  or custom adapter.
- Decision: update the `cargo-orthohelp` agent-context golden snapshot in
  Milestone 2. Rationale: roadmap item 6.2.1 has landed, so the passive schema
  type is now visible through the existing generator's default serialization
  path even though 6.3.1 does not add generator logic.

## Outcomes & Retrospective

The implementation matched the approved passive-schema plan. `SkillManifest` and
`SkillCommandRef` now live in `ortho_config::agent_context`, are re-exported
from `ortho_config`, and are connected through the additive defaulted
`AgentContext.skill_manifests` field. No validator, command-line behaviour, or
policy-report behaviour was added; that remains roadmap item 6.3.2.

The main divergence from the draft plan was caused by the already-landed 6.2.1
generator. Because `cargo-orthohelp` now serializes `AgentContext::new`, the
defaulted field appears in the generator's existing golden snapshot. The
snapshot was updated as a consequence of the schema addition, without adding
generator-specific logic. The other implementation discovery was that
`camino::Utf8PathBuf` needs the crate-supported `serde1` feature for the chosen
wire shape.

Validation evidence:

- Milestone 1 passed `make check-fmt`, `make lint`, and `make test`, followed
  by a zero-finding CodeRabbit review.
- Milestone 2 passed `make check-fmt`, `make lint`, `make test`, and targeted
  `cargo insta accept` verification, followed by a zero-finding CodeRabbit
  review.
- Milestone 3 passed `cargo test -p ortho_config agent_context`,
  `make check-fmt`, `make lint`, `make test`, and `make markdownlint`, followed
  by a zero-finding CodeRabbit review.
- Milestone 4 passed `make fmt`, `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie` before a zero-finding CodeRabbit review.

Lessons for 6.3.2: the `id: String` field gives the future validator a stable
diagnostic target independent of path normalization, symlinks, and future
non-filesystem manifest sources. The `manifest_schema_version: String` choice
also remains appropriate because this milestone uncovered no need for version
comparison semantics. The validator should treat `commands: []` as an explicit
prose-only skill and should reserve invalid-command diagnostics for populated
command references.

## Revision note

- 2026-06-02 (initial DRAFT): plan authored from the Firecrawl prior-art
  research and the Plan agent's milestone breakdown, with three Logisphere
  recommendations folded into §"Recommended design" (`id: String`, exact-match
  documentation on `SkillCommandRef`, forward-compatibility note on
  `SkillManifest::path`). Status: DRAFT; awaiting maintainer approval before
  Milestone 1 may begin.
- 2026-06-12 (approval): maintainer requested implementation. Status:
  APPROVED; Milestone 0 may proceed to validation and review.
- 2026-06-12 (rebase onto `origin/main`): reconciled the plan with roadmap
  item 6.2.1, which merged via pull request 342 and now provides the
  `cargo orthohelp --format agent-context` generator plus an additive
  `AgentCommand.summary` field. Updated the dependency framing (prerequisite
  now satisfied), the `mod.rs` and `tests.rs` line counts (254 and 272), the
  wire-snapshot note (now carries `summary`), and the roadmap-dependency risk
  (downgraded). The proposed `skill_manifests` field and the two new types are
  unaffected by 6.2.1. No remaining open questions.
