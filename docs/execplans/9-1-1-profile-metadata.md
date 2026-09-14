# Design and implement optional profile metadata (roadmap 9.1.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
and `Outcomes & retrospective` must be kept up to date as work proceeds.

Status: COMPLETE (2026-08-09)

## Purpose / big picture

Downstream command-line interfaces (CLIs) built on OrthoConfig, such as Weaver
and Netsuke, need named, reusable bundles of configuration — "profiles" — so
that agents and humans can switch between prepared setups (for example
`--profile weekly-recap`) without re-supplying every flag. Today OrthoConfig
has no profile mechanism at all: the merge engine knows four layers (defaults,
files, environment, CLI), the derive macro generates no `--profile` flag, and
the agent-context schema hard-codes `"profiles": { "supported": false }`.

After this change, a maintainer can observe success directly:

- A config file may contain `[profile.<name>]` tables. Running the CLI with
  `--profile <name>` (or `<PREFIX>PROFILE=<name>` in the environment) overlays
  that profile's values on top of the file layer, below environment variables
  and flags, giving the documented precedence
  `built-in defaults < config files < selected profile < environment < flags`.
- Selecting an unknown profile fails with a structured error that names the
  unknown profile, states where the selection came from (flag or environment
  variable), and lists the available names.
- `cargo orthohelp --format agent-context` for a profile-enabled CLI emits
  `profiles.supported = true` plus selection metadata (flag and environment
  variable names), while legacy derives keep emitting `{ "supported": false }`
  byte-for-byte unchanged.
- A downstream `context --json` command can report which profile is selected
  and why (flag, environment, or none) via a new runtime type.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` all pass.

Profile support is strictly opt-in. OrthoConfig provides the reusable contract
and merge mechanics; downstream applications own profile content, naming, and
any storage helpers (deferred to roadmap 9.1.3). Redaction metadata is deferred
to roadmap 9.1.2 but the type shapes chosen here must not block it.

## Plain-language glossary

- Profile: a named bundle of configuration values defined under a
  `[profile.<name>]` table in the resolved configuration file chain. Selecting
  a profile overlays its values on the file layer.
- Selected profile: the single profile chosen for this invocation via the
  `--profile` flag or the `<PREFIX>PROFILE` environment variable. At most one
  profile is selected.
- Layer: one source of configuration values in the merge pipeline. Layers are
  merged in a fixed precedence order by `MergeComposer`
  (`ortho_config/src/declarative/composer.rs`).
- Provenance: the tag recording which kind of source a layer came from,
  modelled by the `#[non_exhaustive]` enum `MergeProvenance`
  (`ortho_config/src/declarative/layer.rs`).
- File chain: the single configuration file that discovery selects
  (first successful candidate wins — see
  `ortho_config/src/discovery/load.rs::compose_layers`) plus any base files it
  pulls in via `extends`, ordered base-first. Discovery does not merge multiple
  independently discovered files.
- Agent context: the compact, machine-oriented JSON document describing a CLI
  to agents, produced by `cargo orthohelp --format agent-context` and re-served
  by downstream `<tool> context --json` commands. Schema types live in
  `ortho_config::agent_context` with version constant
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` (currently `"1"`).
- Documentation IR: the intermediate representation (`DocMetadata` in
  `ortho_config/src/docs/ir.rs`) emitted by the derive and bridged by
  `cargo-orthohelp` into the agent context. Versioned independently
  (`ORTHO_DOCS_IR_VERSION`) per ADR-003.
- Derive macro: `#[derive(OrthoConfig)]` in `ortho_config_macros`, which
  generates the layer-composition code (`build_compose_layers_impl` in
  `ortho_config_macros/src/derive/load_impl.rs`) and therefore enforces merge
  order.
- BDD: behaviour-driven development; feature files under
  `ortho_config/tests/features/` executed by the `rstest_bdd` test binary.

## Constraints

Hard invariants. Violation requires escalation, not a workaround.

1. Precedence must be exactly
   `built-in defaults < config files < selected profile < environment < flags`
   as required by `docs/agent-native-cli-design.md` §6.7 and roadmap 9.1.1.
   This includes the case where a flag value equals the built-in default: an
   explicitly provided flag must still beat the profile (see risk 5 and
   milestone 4).
2. Profile support is opt-in. Existing derives that do not opt in must compile
   unchanged, keep their current four-layer merge order, gain no new CLI flags,
   and keep emitting `profiles: { "supported": false }` in agent context with
   unchanged bytes. The schema v1 defaulting table
   (`docs/agent-native-cli-design.md` §8.1) must remain satisfied.
3. The agent-context wire schema and the documentation IR may only change
   additively: new optional fields with `#[serde(default)]`; consumers that
   ignore unknown fields must keep working;
   `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` stays `"1"` and `ORTHO_DOCS_IR_VERSION`
   is bumped only if its own compatibility policy demands it. No existing wire
   field may change shape or meaning. (Rust API additivity is governed
   separately by tolerance 2 and decision D7.)
4. OrthoConfig owns generic mechanics only. No application-specific literals
   (profile names, app names) may enter the library. Downstream applications
   own domain behaviour, mirroring the boundary in RFC 0002
   (`docs/rfcs/0002-config-layer-resolution-policy.md`).
5. No circular dependencies between workspace crates. The dependency
   direction stays `ortho_config_macros` → (generates code against) →
   `ortho_config`; `cargo-orthohelp` depends on `ortho_config`, never the
   reverse.
6. `docs/agent-native-cli-design.md` §6.7 requires the exact merge order and
   migration impact to be documented before code is changed. Milestone 1
   (documentation and ADR) must therefore land before any behavioural code.
7. The selector must not be settable from inside a configuration file or
   profile body, and must not leak into the merged configuration value: opting
   in reserves the `profile` root key across all three projections (file tables
   are extracted, the selector environment variable is stripped from the
   environment layer, and the generated flag is excluded from the serialized
   CLI layer). A downstream field that claims the `profile` key or flag on an
   opted-in struct is a compile-time error.
8. `make check-fmt`, `make typecheck`, `make lint`, and `make test` must pass
   at every milestone boundary for the files this task changes, followed by a
   clean `coderabbit review --agent` pass before the next milestone starts. The
   full gate suite was observed green on the rebase base (2026-08-07), so a red
   gate should be treated as this branch's regression by default. If a Whitaker
   finding cites a file outside this task's diff, confirm the baseline at the
   merge base before treating it as inherited, record the finding in
   `Surprises & discoveries`, and proceed rather than fixing unrelated files.
   Consult `docs/whitaker-users-guide.md` for lint configuration and the
   `addressing-whitaker-findings` skill for the per-lint remediation playbook.
9. Profiles apply to the root configuration load only in 9.1.1. The
   subcommand loading path (`ortho_config/src/subcommand/`,
   `load_and_merge_subcommand*`) bypasses `MergeComposer` and does not see
   profiles. A profile table containing a `cmds` key is rejected with an error
   so that no configuration is silently dead (decision D11).

## Tolerances (exception triggers)

Stop and escalate when any threshold below is reached.

1. Dependencies: adding any dependency other than the pre-approved dev
   dependencies `googletest` and `pretty_assertions` (see decision D9) requires
   escalation. If either proves incompatible with the Whitaker Dylint gate or
   the workspace minimum supported Rust version (1.89), escalate rather than
   suppressing lints.
2. Public API: removing or changing the signature of any existing public item
   requires escalation, with one pre-approved exception recorded in D7: the
   retype of the public field `AgentContext.profiles`. All other public API
   listed in "Interfaces and dependencies" is additive and pre-approved.
3. Schema: any agent-context or docs-IR wire change that is not purely
   additive-with-default requires escalation (constraint 3).
4. Size: if a single milestone exceeds roughly 600 net new lines of
   non-test code, or the whole task exceeds roughly 2,500 net lines including
   tests and docs, stop and escalate with a slimming proposal. The natural
   split point is after milestone 4 (behaviour complete, agent context still
   reporting unsupported): if size pressure appears, propose shipping
   milestones 1–4 as one pull request and 5–6 as a follow-up.
5. Iterations: if a gate still fails after three fix attempts for the same
   root cause, stop and escalate with the log evidence.
6. Ambiguity: if profile semantics interact with an existing feature in a way
   this plan does not cover, stop and present options rather than inventing
   semantics. Known-covered interactions: `extends` (D12), subcommands (D11),
   the CLI-absent heuristic (milestone 4), selector leakage (constraint 7).
7. Heuristic: if the flag-equals-default fix (milestone 4) cannot be
   implemented for opted-in structs without changing behaviour of legacy
   derives, stop and escalate with options.

## Approved decisions

These decisions become binding when the plan is approved. Each records the
choice, rationale, and the rejected alternative. D1–D10 were revised and
D11–D15 added after the Logisphere design-review panel (see Decision log).

- D1 — Profiles are named config overlays inside existing files. A profile is
  a `[profile.<name>]` table within the resolved file chain (TOML shown;
  JSON5/YAML equivalents follow the same key path `profile.<name>`). Rationale:
  mirrors Cargo's `[profile.<name>]`, avoids the AWS `[profile x]`/`[default]`
  header asymmetry, reuses existing discovery machinery, and needs no new file
  formats. Rejected: profile-per-file suffixes (`app.staging.toml`, the
  mise/Spring pattern) — more discovery surface and a second naming convention
  for little gain; a separate profile store file — deferred to roadmap 9.1.3 by
  design.
- D2 — The profile layer is a first-class merge layer. Add
  `MergeProvenance::Profile` (unit variant),
  `MergeLayer::profile(value, path)`, and `MergeComposer::push_profile(...)`.
  Profile tables are extracted from the file chain per contributing file,
  producing one profile layer per file that defines the selected profile,
  pushed in file-chain order (base first) after all file layers and before the
  environment layer. Per-file granularity preserves the provenance trail that
  9.1.2's redaction diagnostics will need. The selected profile's name travels
  in `SelectedProfile` (D14), not inside `MergeLayer`, so the layer shape is
  unchanged apart from the new provenance. Rationale: the composer is the
  single enforcement point of precedence; a first-class provenance keeps
  diagnostics honest. Rejected: pre-merging profile values into the file
  layer's value (loses provenance and makes the five-tier precedence
  unprovable); a single pre-merged profile layer (loses the per-file trail); a
  payload-bearing `Profile { name }` variant (complicates the generated
  provenance-label code for no current consumer).
- D3 — Selection is stateless: `--profile <name>` flag with
  `env = "<PREFIX>PROFILE"` fallback on the generated clap argument, so the
  flag beats the environment variable for selection. An empty selector value
  (for example `APP_PROFILE=""` from `export APP_PROFILE=`) is treated as
  unset, not as an invalid name. No persisted "current profile" state.
  Rationale: matches AWS CLI and dbt; persisted selection (kubectl, gcloud)
  causes the classic wrong-context incident and would require a store, which is
  9.1.3's question; empty-means-unset avoids the leaked-empty-export footgun.
  Rejected: a persisted current-profile file; treating the empty string as a
  grammar error.
- D4 — Unknown profile names are a hard error with structured diagnostics.
  If a profile is selected but no `[profile.<name>]` table exists in the file
  chain, loading fails with
  `OrthoError::UnknownProfile { selected, source, available }` where
  `available` is sorted and capped at 16 names (the error display appends "and
  N more" beyond the cap). The error names the selection source so a leaked
  `<PREFIX>PROFILE` in the environment is distinguishable from a typo on the
  flag. When no configuration file was discovered at all, the error says so
  explicitly instead of reporting an empty available list. File parse errors
  take precedence over unknown-profile errors so the root cause is never
  masked. Rationale: figment's silent fallback on unknown profiles is a
  documented footgun; every operational tool surveyed errors loudly; source
  attribution and error ordering are the difference between a five-minute and a
  two-hour incident. Rejected: silent fallback to base values; an uncapped name
  listing.
- D5 — Profile names are case-sensitive and validated against the grammar
  `[A-Za-z0-9_-]+` (non-empty). The name `default` is reserved: defining
  `[profile.default]` is an error, and selecting `default` is equivalent to
  selecting no profile (the observable contract: `SelectedProfile` reports no
  selection, and downstream `context --json` reports none — this is documented
  so agents are not surprised). Additionally, the key `inherits` is reserved
  inside profile bodies (an error if present) so that Cargo-style single-parent
  inheritance can be added later without colliding with a downstream field.
  Rationale: Cargo's validation grammar is the clearest precedent;
  case-sensitivity matches AWS/kubectl and avoids locale-dependent folding;
  reserving `default` prevents two spellings of the base configuration;
  reserving `inherits` is one line now versus a breaking change later.
  Rejected: figment-style case-insensitive matching (surprising duplicates such
  as `Dev` vs `dev`); reserving `global` (OrthoConfig has no global-override
  tier, so the name stays free).
- D6 — Opt-in via a struct-level derive attribute `#[ortho_config(profiles)]`.
  Only structs carrying the attribute gain the generated `--profile` flag, the
  selector environment variable, the profile merge layer, and
  `profiles.supported = true` in agent context. Opting in reserves the
  `profile` root key in every projection (constraint 7); the derive emits a
  compile-time error if a field claims the `profile` key, the `--profile` flag,
  or the `<PREFIX>PROFILE` environment binding. Non-opted-in derives reading a
  shared file that contains `[profile.*]` tables treat the `profile` key like
  any other unknown key (existing behaviour, now pinned by a test). Rationale:
  constraint 2 requires legacy derives untouched; an attribute is the
  established opt-in mechanism (`post_merge_hook`, `discovery(...)`); the flag
  surface must be static for agent context. Rejected: auto-enabling when a
  `[profile.*]` table is present (spooky action at a distance).
- D7 — Agent-context exposure retypes `AgentContext.profiles` from
  `SupportDeclaration` to a new `ProfilesDeclaration` type. This is a
  deliberate, pre-approved breaking change to the Rust API of a pre-1.0 crate
  (any consumer constructing or matching `AgentContext` by struct literal must
  adjust), carried on the next 0.x minor release and recorded in the changelog
  and migration notes. It is not a wire-schema break: the unsupported case
  serializes byte-identically to today's `{ "supported": false }`, because the
  new optional fields are omitted when absent. `ProfilesDeclaration` provides
  constructors (`ProfilesDeclaration::unsupported()` and
  `ProfilesDeclaration::supported(selection)`) so downstream construction
  survives future field additions. The new fields:
  `selection: Option<ProfileSelectionContract>` (the flag name following the
  `AgentInput::long` convention — no leading `--` — and the environment
  variable name) and `list_command: Option<Vec<String>>` (a command path
  matching `AgentCommand::path` token-for-token, populated by 9.1.3 when
  listing helpers exist; carried now so the contract shape is settled).
  Selected-profile semantics (which profile is active now and why) are a
  runtime concern, exposed through `SelectedProfile` (D14), which downstream
  `context --json` commands may embed; the static generated context documents
  the mechanism, not the moment. Rationale: keeps the wire change additive
  while satisfying roadmap 9.1.1; the static/runtime split follows ADR-007;
  honest labelling of the Rust-level break replaces the earlier, incorrect "all
  additive" claim. Rejected: a parallel sibling field on `AgentContext` leaving
  `profiles` untouched (permanently confusing vocabulary for a one-time pre-1.0
  adjustment); a stringly `list_command: Option<String>` (forces consumers to
  invent shell tokenization, contra the schema's exact-match path convention).
- D8 — The selector environment variable defaults to `<PREFIX>PROFILE`,
  derived from the existing `prefix` attribute exactly as other environment
  keys are (for example prefix `APP_` gives `APP_PROFILE`). Rationale: the
  de-facto standard and consistent with OrthoConfig's environment naming.
  Rejected: a configurable variable name in 9.1.1 — additive later if a
  consumer needs it.
- D9 — Add `googletest` and `pretty_assertions` as workspace dev
  dependencies, used only in the new profile test modules; the convention is
  recorded in `docs/developers-guide.md` in milestone 6. This is a project
  requirement for this feature (richer matcher output for the new merge-order
  and error-path assertions), not an invitation to migrate existing tests. The
  first milestone-2 commit verifies both crates build cleanly under `make lint`
  (Whitaker at `-D warnings`) and rustc 1.89 before any test depends on them;
  failure escalates per tolerance 1. Whitaker's assertion-related lints (notably
  `no_expect_outside_tests`) do not always recognize helper functions as test
  code, so consult `docs/whitaker-users-guide.md` for the configuration options
  before suppressing anything. Rejected: continuing with bare `assert_eq!` for
  new tests.
- D10 — Out of scope, recorded to prevent drift: profile inheritance
  (`inherits =` — the key is reserved by D5 but the semantics are future work),
  multiple simultaneous profiles (the selection accessor returns a slice so
  this can arrive additively — D14), secret redaction (9.1.2), any profile
  store helper (9.1.3), and profile-aware subcommand loading (D11 defines
  today's behaviour; lifting it is future work).
- D11 — Subcommand loading ignores profiles in 9.1.1, and profile tables
  must not contain a `cmds` key. The subcommand path
  (`load_and_merge_subcommand*` in `ortho_config/src/subcommand/mod.rs`) is a
  separate figment pipeline that bypasses `MergeComposer`; pretending profiles
  reach it would make agent context lie. To prevent silently dead
  configuration, a `[profile.<name>]` table containing `cmds` fails validation
  with `OrthoError::ProfileForbiddenKey`. The users' guide states the
  limitation; the ADR records lifting it as the expected follow-up once 9.1
  stabilizes. Rationale: Weaver and Netsuke are subcommand CLIs and will hit
  this immediately; an explicit error beats a silent no-op. Rejected: silently
  ignoring `cmds` inside profiles; extending profile merging into the
  subcommand path in this task (a second pipeline's worth of scope).
- D12 — `extends` interaction: profile tables are collected from the file
  chain after `extends` resolution, one layer per contributing file that
  defines the selected profile, in chain order (base first, extending file
  last, matching the file layers themselves). Discovery semantics are
  unchanged: the first successful candidate wins, so profiles never merge
  across independently discovered files. Milestone 1 includes a short spike
  confirming the generated code still sees per-file values post-`extends`; if
  `extends` pre-merges values before layering, escalate per tolerance 6 before
  the ADR is finalized.
- D13 — Relationship to RFC 0002 (file-layer resolution policy, status
  Proposed): profile extraction is specified as a consumer of "the ordered
  post-`extends` file values", which is exactly the seam RFC 0002 names
  `FileLayerOutcome`. This plan implements extraction against today's discovery
  output; ADR-008 records as a design obligation that if RFC 0002 lands,
  `FileLayerOutcome` must expose the ordered file values profile extraction
  needs, and the extraction helper is written against a minimal internal
  interface (ordered `(path, value)` pairs) so it can be re-seated without
  semantic change. Rationale: sequencing 9.1.1 behind an unaccepted RFC is
  unacceptable schedule risk, but ignoring the collision would force the RFC to
  work around shipped code. Rejected: building profiles as the first RFC 0002
  policy now (blocked on RFC acceptance); ignoring the overlap (rework trap).
- D14 — Post-load selection surfacing: opted-in structs gain a generated
  associated function
  `load_with_profile_from_iter(iter) -> OrthoResult<ProfileLoadOutcome<Self>>`
  (plus a `load_with_profile()` convenience). `ProfileLoadOutcome<T>` has
  private fields and accessors `config()`, `into_config()`, and
  `selection() -> &[SelectedProfile]` (empty or singleton today; a slice so
  multiple simultaneous profiles can arrive additively). The existing `load`/
  `load_from_iter` signatures are untouched.
  `SelectedProfile { name: ProfileName, source: ProfileSource }` and
  `#[non_exhaustive] ProfileSource { Flag, Environment }` are plain runtime
  types without serde implementations: downstream `context --json` commands own
  their JSON mapping per ADR-003's ownership split, and the users' guide shows
  the recommended snake_case rendering. Rationale: the original plan asserted
  selection would be "available after load" without designing the surface;
  deciding it now prevents an improvised public API mid-milestone. Rejected:
  changing `load_from_iter`'s return type (breaking); serde derives on
  `SelectedProfile` (creates an unversioned de facto wire contract with no
  fixture).
- D15 — Documentation IR carries profile metadata. `DocMetadata`
  (`ortho_config/src/docs/ir.rs`) gains an additive, defaulted field
  `profiles: Option<DocProfilesMeta>` where `DocProfilesMeta { flag, env_var }`
  mirrors `ProfileSelectionContract`; the derive emits it for opted-in structs;
  `bridge_ir_to_agent_context` maps it into `ProfilesDeclaration`. The field is
  additive-with-default, so `ORTHO_DOCS_IR_VERSION` stays unchanged per the IR
  compatibility policy; IR golden fixtures are re-baselined in the same
  milestone. Rationale: the bridge cannot learn about profile support any other
  way — without this the agent-context milestone is unimplementable; the panel
  identified this as a missing workstream. Rejected: having `cargo-orthohelp`
  re-parse source attributes (violates the bridge architecture).

## Risks

- Risk: the generated `--profile` flag, `profile` key, or `<PREFIX>PROFILE`
  binding collides with an existing downstream field on an opted-in struct.
  Severity: medium. Likelihood: medium. Mitigation: compile-time error covering
  all three projections (D6); documented in the migration notes.
  `docs/agent-native-cli-design.md` §2.2 already declares that on shape
  conflict the OrthoConfig shape wins.
- Risk: retyping `AgentContext.profiles` breaks downstream Rust consumers.
  Severity: medium. Likelihood: high (certain for struct-literal construction).
  Mitigation: acknowledged as a deliberate pre-1.0 breaking change (D7) on a
  minor bump with migration notes and constructors; the wire contract is
  unaffected, which the byte-identity test proves.
- Risk: the flag-equals-default heuristic (`differs_from_defaults` gating
  the CLI layer push in `build_compose_layers_impl`) silently drops an explicit
  flag, letting the profile win. Severity: high. Likelihood: high if
  unaddressed. Mitigation: milestone 4 fixes the heuristic for opted-in structs
  using clap's value-source information (an argument counts as provided when
  clap reports a command-line or environment origin, not by comparing values);
  a dedicated red test pins "flag equal to default still beats profile"; the
  precedence property test generates equal-to-default values on purpose;
  tolerance 7 escalates if the fix would disturb legacy derives.
- Risk: profile tables interact badly with `extends`.
  Severity: medium. Likelihood: medium. Mitigation: D12 defines the rule and
  schedules a milestone-1 spike before the ADR is finalized; a BDD scenario
  pins base-file profile tables being overridden by the extending file's.
- Risk: RFC 0002 later restructures file-layer assembly underneath profile
  extraction. Severity: medium. Likelihood: medium. Mitigation: D13 —
  extraction written against a minimal ordered `(path, value)` interface;
  ADR-008 records the `FileLayerOutcome` obligation.
- Risk: fixture sprawl — one schema field touches the wire-contract JSON,
  contract-support helpers, round-trip property strategy, the agent-context
  insta snapshot, three `cargo-orthohelp` goldens, and the three
  `examples/hello_world` agent-context surfaces. Severity: medium. Likelihood:
  high (by construction). Mitigation: milestone 5 enumerates all artefacts up
  front and lands them atomically; shared fixture-builder helpers keep future
  additions linear; the wire-contract fixture's line-ending pinning
  (`.gitattributes`) is respected.
- Risk: `googletest` or `pretty_assertions` trips the Whitaker gate or the
  1.89 MSRV. Severity: low. Likelihood: low. Mitigation: verified in the first
  milestone-2 commit before dependence (D9); tolerance 1 escalates on failure.

## Progress

- [x] (2026-08-06) Recon: roadmap/design docs, agent-context implementation,
      docs conventions, and external prior art surveyed.
- [x] (2026-08-06) Initial draft written.
- [x] (2026-08-06) Logisphere design-review panel run; five of six lenses
      reported (scaling and operational cost did not complete). Findings
      folded into this revision (see Decision log and revision note).
- [x] (2026-08-07) Rebased onto `origin/main`; upstream Whitaker guide,
      toolchain, and formatter changes absorbed. Gates green.
- [x] (2026-08-07) Stage A: plan submitted as draft pull request #418 and
      approved by the maintainer; implementation started.
- [ ] Optional before approval: run the scaling and operational-cost review
      lens that did not complete.
- [x] (2026-08-09) Milestone 1: ADR-008 and design documentation. ADR-008
      drafted and accepted; `design.md` precedence statements (§3, §4.3,
      §4.10, §4.17), `agent-native-cli-design.md` §6.7/§8.1/§8.2,
      `contents.md`, and `roadmap.md` 9.1.1 updated; the D12 `extends` spike
      confirmed; docs gates and CodeRabbit review clean.
      - [x] (2026-08-07) D12 spike (read-only): confirmed. See Surprises &
            discoveries.
- [x] (2026-08-09) Milestone 2: profile merge layer in the composer.
      `MergeProvenance::Profile`, `MergeLayer::profile(value, path)`, and
      `MergeComposer::push_profile` landed with the generated provenance-label
      update and its fixture; the red→green→refactor cycle and the D9 dev-dep
      verification are recorded below; full gates and CodeRabbit review clean.
      - [x] (2026-08-09) D9 dev-dep verification commit: googletest 0.14.3
            and pretty_assertions 1.4.1 build clean under `-D warnings` on
            the MSRV 1.89 and Whitaker toolchains; the profile test-module
            skeleton (four files) passes.
      - [x] (2026-08-09) Red: focused tests fail to compile with the
            expected missing `push_profile`/`MergeProvenance::Profile`/
            `MergeLayer::profile` errors; BDD feature extended with a
            profile-layer scenario. See Artefacts and notes.
      - [x] (2026-08-09) Green: `MergeProvenance::Profile`,
            `MergeLayer::profile(value, path)`, `MergeComposer::push_profile`,
            guards.rs label + fixture updated; all focused tests pass.
      - [x] (2026-08-09) Refactor: `MergeLayer::value()` borrowing accessor
            finalized as the extraction value-access mechanism (see decision
            log); constructor plumbing already minimal.
- [x] (2026-08-09) Milestone 3: profile extraction, selection resolution,
      validation, and structured error paths in `ortho_config`. The
      `profile/` module (name, selection, extract submodules), the four
      `OrthoError` variants plus localizer IDs, the unit test modules, and
      `profiles.feature` land together; full gates and CodeRabbit review
      clean (verified 2026-08-09). See Artefacts and notes.
- [x] (2026-08-09) Milestone 4: derive-macro opt-in, generated `--profile`
      flag, selector leakage stripping, the flag-equals-default fix, docs-IR
      emission (D15), and end-to-end precedence behaviour. The
      `#[ortho_config(profiles)]` opt-in, the collision guards, the
      ProfileLoadOutcome entry points, the docs-IR field, the trybuild
      fixtures, the bounded precedence proptest, and the BDD scenarios
      through the real derived CLI all land together; full gates and
      CodeRabbit review clean. See Artefacts and notes.
- [x] (2026-08-09) Milestone 5: agent-context schema retype (D7),
      `ProfileLoadOutcome` surfacing (D14, landed with milestone 4),
      `cargo-orthohelp` bridge, and the full fixture set. `ProfilesDeclaration`
      and `ProfileSelectionContract` replace the `SupportDeclaration` field
      with byte-identical unsupported serialization; the bridge maps
      `DocMetadata.profiles` into the declaration; a profile-enabled golden
      fixture and byte-identity + round-trip coverage land. Full gates and
      CodeRabbit review clean. See Artefacts and notes.
- [x] (2026-08-09) Milestone 6: user-facing and contributor documentation,
      roadmap tick, and retrospective. `users-guide.md` gains the profiles
      subsection and the agent-contract profile metadata; `developers-guide.md`
      records the schema ownership and the D9 testing convention; the
      changelog records the D7 break; the roadmap ticks 9.1.1; the
      retrospective is below. Final gates and CodeRabbit review clean.
- [x] (2026-08-12) Post-completion review follow-up: behaviour-preserving
      private helpers now share the profile-selection, profile-key recording,
      forbidden-key assertion, and unknown-profile assertion seams without
      merging the distinct BDD entry points or named reserved-key tests. The
      full commit gates, Markdown gates, and focused profile BDD suite are
      green at the current pull-request head; a refreshed CodeScene analysis
      remains requested for its stale duplication and nesting findings.
- [x] (2026-08-12) D4 error-payload correction: `AvailableProfileNames` now
      retains only the first 16 sorted names and their omitted-name count, so
      `UnknownProfile.available.as_slice()` cannot expose an unbounded list.
      The 20-profile extraction test verifies both the 16-name payload and the
      unchanged `and 4 more` display suffix.

Progress entries from milestone 1 onward must carry timestamps.

## Surprises & discoveries

- Observation: `googletest` and `pretty_assertions` are named in the task
  brief and several ExecPlans but are not yet dependencies of any workspace
  crate. Evidence: no matches in any `Cargo.toml` at planning time. Impact:
  decision D9 adds them, scoped to new profile test modules, with an up-front
  lint/MSRV verification step.
- Observation: config discovery is first-match-wins, not multi-file: only
  the first successful candidate (plus its `extends` chain) produces file
  layers. Evidence: `ortho_config/src/discovery/load.rs::compose_layers`
  returns on the first successful candidate. Impact: the profile-collection
  rule (D12) is defined over the file chain, not over "every discovered file"
  as an earlier draft claimed.
- Observation: the generated code pushes the CLI layer only when the parsed
  CLI differs from defaults, which breaks "flags beat profile" for flags
  explicitly set to the default value. Evidence: `build_compose_layers_impl`
  (`ortho_config_macros/src/derive/load_impl.rs`) guards `push_cli` behind a
  `differs_from_defaults` check. Impact: risk 5, milestone 4 work item, and
  tolerance 7 added.
- Observation: the documentation IR (`DocMetadata`) carries no profile
  metadata, so the agent-context bridge had no input for `profiles` until D15
  added the IR field. Evidence: `ortho_config/src/docs/ir.rs` field list.
  Impact: D15 and a milestone-4 work item added.
- Observation: the generated provenance-label code uses a wildcard match, so
  a new `MergeProvenance` variant compiles silently while labelling profile
  layers "unknown". Evidence:
  `ortho_config_macros/src/derive/generate/declarative/guards.rs` maps
  provenance with a `_ => "unknown"` arm; the fixture
  `expected_merge_impl_empty.rs.txt` encodes the current labels. Impact:
  milestone 2 updates both alongside the enum.
- Observation: the Whitaker lint baseline on `main` was repaired between
  2026-07-30 and 2026-08-07, and `main` gained `docs/whitaker-users-guide.md`.
  Evidence: a full gate run at the rebase base returned green for `cargo doc`,
  clippy at `-D warnings`, and Whitaker (nightly-2026-05-28), where the same
  gate previously failed with five `no_expect_outside_tests` findings. Impact:
  constraint 8 now treats a red gate as this branch's regression by default
  rather than as inherited noise, and the new guide is signposted for D9's
  dev-dependency verification.
- Observation: `mdformat` output for this plan diverged from the repository
  baseline until `main` adopted the current formatter. Evidence: running
  `mdformat-all` initially rewrote `main`'s freshly formatted
  `docs/execplans/6-2-2-...md`; upstream commit `796c2bd` then applied exactly
  that reformatting. Impact: this plan was reformatted to match, and the
  `Concrete steps` section now tells implementers to run `make fmt` before
  committing documentation.
- Observation (2026-08-07, D12 spike): per-file values survive `extends`
  resolution as distinct, ordered layers. Evidence: `load_config_file_as_chain`
  (`ortho_config/src/file/loader.rs`) returns a `FileLayerChain` whose `values`
  are documented as ancestor-first ("The first entry is the root ancestor; the
  last is the directly-loaded file"), and `ConfigDiscovery::compose_layers`
  (`ortho_config/src/discovery/load.rs`) maps each `(value, path)` entry to its
  own `MergeLayer::file` with the source path attached. Impact: D12's rule is
  implementable directly against `DiscoveryLayersOutcome.value`; no escalation
  per tolerance 6 was needed, and ADR-008 records the confirmed chain order.
- Observation (2026-08-09, milestone 1): the `markdownlint` gate embeds the
      `typos` spelling gate, whose first run over the milestone-1 diff flagged
      `recognize` in this plan (Oxford spelling requires `-ize`). Evidence:
      `make markdownlint` failed with `error: 'recognize' should be 'recognize'`
      until the plan was reworded. Impact: subsequent docs commits must run
      `make spellcheck` (or the full `make markdownlint`) locally before
      review; the `typos.local.toml` inline-code exclusion remains pinned as a
      temporary hold pending an identifier sweep (see commits 27260d3, 41d2554).
- Observation (2026-08-12, post-completion review): CodeScene findings for
      nested selection logic and duplicated profile-test setup targeted an
      earlier revision. Evidence: `SelectedProfile::resolve` selects the flag
      pair before the environment pair and delegates to `from_value`; the BDD
      steps and extraction tests use their respective private helpers while
      retaining their separate framework contracts. Impact: no suppression is
      justified; CodeScene must reanalyse the current pull-request head.
- Observation (2026-08-12, D4 review): `AvailableProfileNames::new` sorted
      every name, while only `Display` applied the 16-name cap. Evidence: a
      new 20-profile extraction assertion observed `available.as_slice().len()`
      as 20 before the correction. Impact: programmatic consumers of
      `OrthoError::UnknownProfile` could receive unbounded configuration
      metadata despite D4's stated cap.

## Decision log

- Decision: plan drafted with decisions D1–D10; profile-as-overlay design
  chosen after prior-art survey (AWS, kubectl, gcloud, dbt, Cargo, docker,
  figment, config-rs, mise, Spring). Rationale: recorded per decision in
  "Approved decisions". Date/Author: 2026-08-06, planning agent.
- Decision: revised after the Logisphere design-review panel. D2 pinned to
  per-file profile layers with an unchanged `MergeLayer` shape; D3 gained
  empty-selector-means-unset; D4 gained structured payloads, source
  attribution, capped listings, and error-ordering rules; D5 gained the reserved
  `inherits` key and the documented `--profile default` contract; D6 widened
  the collision check to all three projections; D7 re-labelled the
  `AgentContext.profiles` retype as a deliberate Rust-level breaking change,
  added constructors, and made `list_command` a path vector; D9 scoped the new
  dev dependencies and added a lint/MSRV verification step; D11–D15 added
  (subcommand boundary, `extends` rule, RFC 0002 seam, post-load surfacing API,
  docs-IR field). Constraints 7–9, tolerances 6–7, and risks 3, 5–7 added or
  reworked accordingly. Rationale: panel findings — selector leakage,
  flag-equals-default suppression, subcommand split-brain, docs-IR gap, semver
  mislabelling, and first-match discovery — were all boundary-of-record
  failures that are cheap to fix pre-code and expensive after. Date/Author:
  2026-08-06, planning agent with the Logisphere panel.
- Decision: rebase onto `origin/main` and absorb the upstream changes that
  bear on this plan — signpost the new `docs/whitaker-users-guide.md`, tighten
  constraint 8 now that the lint baseline is green, note the `rust-analyzer`
  toolchain requirement and the `mdformat` step in `Concrete steps`, and
  reformat this document with the adopted formatter. Rationale: the plan must
  stay executable against `main` as it actually is; a stale lint caveat would
  invite a future implementer to dismiss a genuine regression as inherited
  noise. Date/Author: 2026-08-07, leynos.
- Decision: plan approved by the maintainer and implementation started. The
  D12 `extends` spike (read-only) confirmed per-file values survive `extends`
  resolution as distinct, ancestor-first layers, so no tolerance-6 escalation
  was needed. Status moved to APPROVED; milestone work proceeds per the plan.
  Date/Author: 2026-08-07, implementing agent.
- Decision: milestone 1 (ADR-008 and design documentation) completed and the
  ADR flipped to Accepted. ADR-008 was verified against the approved plan —
  decisions D1–D8 and D11–D15, the five-tier merge order, the reserved selector
  projections, the migration and rollback stories — and the docs gates
  (`make markdownlint`, `make nixie`) plus a CodeRabbit review pass clean.
  Rationale: constraint 6's documentation-first requirement is now satisfied;
  behavioural work (milestone 2) may begin. Date/Author: 2026-08-09,
  implementing agent.
- Decision: the milestone-2 refactor finalizes the `MergeLayer` value-access
  mechanism as a borrowing `value()` accessor rather than the `into_parts()`/
  `map_value` candidates named in the plan. Rationale: the milestone-3/4
  extraction helper must read a file layer's value while the layer is still
  pushed as a file layer, so a non-consuming accessor is the minimal seam;
  `value()` is the borrowing counterpart to the existing `into_value()`, and
  `map_value` would add a combinator no call site needs yet. The composer's
  `push_*` wrappers were already one-liners over `push_layer`, so no
  constructor plumbing needed deduplication. Date/Author: 2026-08-09,
  implementing agent.
- Decision: the `UnknownProfile` error field is named `selection_source`
  rather than the plan's `source`. Rationale: thiserror reserves the field name
  `source` for the error source chain and requires it to implement
  `std::error::Error`; `ProfileSource` is a plain runtime type (D14) that must
  not become an error type. The codebase's other `source` fields are all real
  error sources, so the rename is consistent with existing semantics.
  Date/Author: 2026-08-09, implementing agent.
- Decision: the `AvailableProfileNames` display renders "no configuration
  files were found" when the list is empty. The rare "files discovered but none
  define profile tables" case renders the same message; this is a documented
  edge case, not a correctness issue. Date/Author: 2026-08-09, implementing
  agent.
- Decision: milestone 4 attributes the selection source via clap's
  `ArgMatches::value_source` rather than reading the CLI struct's `profile`
  field directly. Rationale: clap fills the field from the `env` fallback too,
  so a naive read would mislabel an environment selection as a flag; the
  value-source gate keeps the D3 attribution honest. When clap parsing fails,
  the environment is read directly so selection errors never mask parse errors
  (D4). Date/Author: 2026-08-09, implementing agent.
- Decision: the flag-equals-default fix gates the CLI push on
  `differs_from_defaults || explicitly_provided` rather than value-source
  alone. Rationale: a clap `default_value_t` that differs from the ortho
  default currently pushes via `differs_from_defaults`; a pure replacement
  would silently change that opted-in behaviour. The OR changes behaviour only
  in the target case (sanitized CLI equals defaults but a field was explicitly
  provided). The clap `env` feature was added to ortho_config's dependency to
  support the generated `env = "<PREFIX>PROFILE"` attribute (D3). Date/Author:
  2026-08-09, implementing agent.
- Decision: milestone 6 completes the plan and the status moves to COMPLETE.
      The users' guide, developers' guide, changelog, and roadmap are updated; the
      retrospective records the deferrals (fixture-builder extraction riding along
      with 9.1.2, the unfinished scaling lens, D11/D5/9.1.3) and the friction
      points (thiserror `source` reservation, Whitaker module caps, rstest-bdd slot
      semantics). Date/Author: 2026-08-09, implementing agent.
- Decision: retain separate rstest-bdd entry points and the two named
      reserved-key tests while extracting only private assertion and fixture
      helpers. Rationale: the framework attributes and test names are part of
      the behavioural and diagnostic contracts; the helpers remove repeated
      setup without reducing coverage or changing public behaviour. Date/Author:
      2026-08-12, implementing agent.
- Decision: apply D4's 16-name limit when constructing
      `AvailableProfileNames`, retaining the omitted-name count alongside the
      capped, sorted payload. Rationale: enforcing the bound at the structured
      error boundary protects programmatic consumers as well as display output,
      while the count preserves the existing `and N more` diagnostic. Date/Author:
      2026-08-12, implementing agent.

## Outcomes & retrospective

All six milestones completed on 2026-08-09. The feature ships: opt-in
`#[ortho_config(profiles)]` with the generated `--profile` flag and
`<PREFIX>PROFILE` selector, the first-class profile merge layer, the
extraction/validation/error machinery, the flag-equals-default fix, the
`ProfileLoadOutcome` runtime surfacing, the additive agent-context and docs-IR
exposure, and the users'/developers' documentation. Every milestone boundary
passed the four commit gates and a clean CodeRabbit review.

What went well: the milestone-1 documentation-first order (constraint 6) made
later decisions cheap; the BDD scenarios built on the library helpers in
milestone 3 transferred cleanly to the real derived CLI in milestone 4; the
byte-identity property of the agent-context retype meant the wire-contract
fixtures needed no re-baselining.

What cost time: thiserror's `source` field-name reservation forced the
`selection_source` rename; Whitaker's `module_max_lines` and
`no_expect_outside_tests` required several mid-milestone module splits and
helper-signature changes; `rstest-bdd`'s positional placeholder binding and
`Slot::with_mut` no-op-on-empty semantics each hid a bug that took a debug
print to surface.

Deferred (recorded, not accidental): the fixture-builder extraction across
agent-context test families (ride along with 9.1.2's `redaction` field); the
scaling and operational-cost design-review lens; profile-aware subcommand
loading (D11); `inherits` semantics (D5); the 9.1.3 store helpers.

Post-completion review left the delivered profile contract unchanged while
consolidating repeated test setup and assertions. The current pull-request head
still satisfies the completed roadmap task, with the focused profile BDD suite,
full commit gates, and Markdown gates green. CodeScene's reported duplication
and nesting findings require reanalysis against this revision rather than a
diagnostic suppression.

The D4 review correction now enforces the documented cap for the structured
unknown-profile payload as well as its display text. The 20-profile extraction
test keeps both the 16-name payload bound and the omitted-count diagnostic
observable.

## Context and orientation

The workspace (`/` is the repository root) contains:

- `ortho_config/` — the core library. Relevant modules:
  `src/declarative/layer.rs` (`MergeProvenance`, `MergeLayer`),
  `src/declarative/composer.rs` (`MergeComposer`, `push_defaults`, `push_file`,
  `push_environment`, `push_cli`, generic `push_layer`), `src/discovery/`
  (config-file discovery; `load.rs::compose_layers` is first-match-wins),
  `src/subcommand/` (the separate subcommand loading path — profiles do not
  apply there, D11), `src/error/types.rs` (`OrthoError`, `#[non_exhaustive]`),
  `src/localizer/` (message IDs for new error variants), `src/docs/ir.rs`
  (`DocMetadata`, `ORTHO_DOCS_IR_VERSION`), and `src/agent_context/` (schema
  types, JSON serialization, wire-contract fixture at
  `src/agent_context/fixtures/agent_context_wire_contract.json`, tests in
  `src/agent_context/tests*.rs`, insta snapshots in
  `src/agent_context/snapshots/`).
- `ortho_config_macros/` — the derive. `src/derive/parse/mod.rs` parses
  struct attributes (`StructAttrs`) and field attributes;
  `src/derive/load_impl.rs::build_compose_layers_impl` emits the canonical
  layer order (`push_defaults` → file layers → `push_environment` → conditional
  `push_cli`), parses the CLI before composing (so the selector is available in
  time), and guards `push_cli` behind `differs_from_defaults`;
  `src/derive/generate/declarative/guards.rs` generates provenance labels
  (wildcard arm — see Surprises); fixture `expected_merge_impl_empty.rs.txt`
  pins the generated output.
- `cargo-orthohelp/` — documentation and agent-context generator;
  `src/agent_context/mod.rs::bridge_ir_to_agent_context` builds the
  `AgentContext` from the docs IR; golden outputs in
  `tests/golden/agent_context__*.json.snap`.
- `examples/hello_world/` — dogfood binary with three agent-context test
  surfaces (`agent_context_snapshot.rs`, `agent_context_e2e.rs`,
  `agent_context_bdd.rs` plus its feature file).
- Behavioural tests: feature files in `ortho_config/tests/features/`
  (`cli_precedence.feature`, `merge_composer.feature`, and so on) run by the
  `rstest_bdd` test target.
- Governing documents: `docs/agent-native-cli-design.md` §6.7 (persistent
  profiles), §8.1 (schema v1 defaulting table), and §8.2 (compatibility policy,
  including the null-versus-omitted asymmetry paragraph that milestone 1
  amends), `docs/design.md` §3, §4.10 and §4.17 (current four-tier precedence
  statements), `docs/roadmap.md` §9.1, ADR-003 (schema ownership), ADR-007
  (`context --json` naming), RFC 0002 (file-layer resolution policy, Proposed).

Environment-variable-dependent tests must use the guards in `test_helpers`
(`ortho_config_test_helpers`) — raw environment mutation in tests is forbidden
by `AGENTS.md`. AGENTS.md also caps source files at 400 lines; the new test
modules are laid out as `profile/tests_names.rs`, `profile/tests_selection.rs`,
`profile/tests_extraction.rs`, and `profile/tests_errors.rs` from the start to
avoid a mid-flight split.

## Plan of work

### Stage A — approval (no code changes)

Draft this plan, run the community-of-experts design review (done — see
Decision log), and submit for approval as a draft pull request. Implementation
starts only after explicit approval. The remainder of this section is the
approved route.

### Milestone 1 — documentation before code (ADR + design updates)

Constraint 6 requires the merge order and migration impact documented before
behavioural change.

First, run the D12 spike (read-only): confirm in `ortho_config/src/discovery/`
and the generated layer code that per-file values survive `extends` resolution
as distinct layers. If not, stop (tolerance 6).

Then write `docs/adr-008-profile-selection-and-layering.md` following the ADR
template in `docs/documentation-style-guide.md`. Author it as Proposed in the
first commit; flip to Accepted in the same milestone once its text is verified
against the approved plan. It must capture D1–D8 and D11–D15, including: the
exact five-tier merge order; per-file profile layers (D2); the reserved
`profile` root key and selector-stripping rule (constraint 7); the subcommand
boundary and `cmds` rejection (D11); the `extends` rule (D12); the RFC 0002
`FileLayerOutcome` obligation (D13); the flag-equals-default resolution (risk
3); the Rust-level break on `AgentContext.profiles` with its migration note for
§2.2 soft-tier consumers (Weaver/Netsuke adapters), including the rollback
story: removing `#[ortho_config(profiles)]` restores pre-profile behaviour at
the cost of deleting the `--profile` flag users may have scripted against, and
flips agent context back to unsupported.

Update `docs/design.md` precedence statements (§3 provider list, §4.10
`extends` ordering, §4.17) to insert the selected-profile tier, marked as
opt-in. Update `docs/agent-native-cli-design.md`: §6.7 records the resolved
merge order; §8.1's table gains the new defaulted fields; §8.2's
null-versus-omitted asymmetry paragraph is amended to state that
`AgentCommand.summary` and the new `ProfilesDeclaration` optional fields are
omitted when absent, and the schema v1 history list gains an entry. Register
the ADR and this plan in `docs/contents.md`. Update `docs/roadmap.md` 9.1.1
notes to cite the ADR and to note that the `list_command` population is
deferred to 9.1.3 (the 9.1.1 sub-item is satisfied by the contract field and
documented semantics).

Validation: `make markdownlint` and `make nixie` pass; scrutineer runs the docs
gates; CodeRabbit review of the docs commit is clean.

### Milestone 2 — profile layer in the merge engine (red → green → refactor)

First commit: add `googletest` and `pretty_assertions` as workspace dev
dependencies with one trivial usage each in the new test module skeleton, and
run the full gates to verify Whitaker/MSRV compatibility (D9).

Red: add rstest unit tests in `ortho_config/src/declarative/` asserting that a
layer pushed via the new `push_profile` merges above files and below
environment, and that `MergeProvenance::Profile` is labelled correctly in
diagnostics. Extend `ortho_config/tests/features/merge_composer.feature` with a
profile-layer scenario. Run the focused tests and record the expected failures
(missing variant/method).

Green: add `MergeProvenance::Profile` (additive; the enum is
`#[non_exhaustive]`), `MergeLayer::profile(value, path)`, and
`MergeComposer::push_profile`. Update the generated provenance-label match in
`ortho_config_macros/src/derive/generate/declarative/guards.rs` and its fixture
`expected_merge_impl_empty.rs.txt` in the same commit (see Surprises). Make the
red tests pass.

Refactor: deduplicate constructor plumbing; add the minimal `MergeLayer` access
the extraction helper needs (an `into_parts()`/rebuild pair or a `map_value`
combinator — named in "Interfaces and dependencies"), so milestone 3 does not
improvise API surface.

Validation: focused tests pass; full gate run (`make check-fmt`,
`make typecheck`, `make lint`, `make test`) via scrutineer; commit; CodeRabbit
review clean.

### Milestone 3 — extraction, selection, validation, errors

Red: unit tests (rstest, googletest matchers, pretty_assertions) in the
pre-planned `profile/tests_*.rs` modules for: extraction of `[profile.<name>]`
tables from an ordered `(path, value)` file chain (per-file layers, chain
order, D2/D12); base-layer stripping (profile keys never leak into unselected
loads, and stripping runs for opted-in structs whether or not a profile is
selected); selection resolution (flag beats environment; empty selector means
unset, D3); name-grammar acceptance and rejection (property test over the
grammar); reserved-name rules (`[profile.default]` errors; selecting `default`
reports no selection; `inherits` inside a body errors; `cmds` inside a body
errors, D11); unknown-profile errors carrying structured `selected`/`source`/
`available` payloads with sorted, capped listings; the no-files-discovered
error text; parse-error precedence over unknown-profile; and unknown keys
inside a profile table behaving exactly as base-config unknown keys (pinned,
not assumed), with an empty profile table as a valid no-op. Add a new
`ortho_config/tests/features/profiles.feature` covering the pinned scenarios
(the feature text is embedded in "Validation and acceptance" below).

Green: implement in a new module `ortho_config/src/profile/` (split per the
400-line rule): `ProfileName` (validated newtype), selection resolution
(`SelectedProfile::resolve(...)` — no separate helper type, keeping the
vocabulary to three names), `SelectedProfile`/`ProfileSource` (D14, serde-
free), the extraction helper over ordered `(path, value)` pairs (D13), and new
`#[non_exhaustive]`-friendly variants on `OrthoError`:
`UnknownProfile { selected, source, available }`, `InvalidProfileName { name }`,
`ReservedProfileName { name }`, and `ProfileForbiddenKey { profile, key }`,
each with localizer message IDs.

Refactor: consolidate with discovery types; keep RFC 0002's boundary — no
application literals; extraction stays a library helper so
`build_compose_layers_impl` does not accrete logic.

Validation: as milestone 2 (focused red/green evidence, full gates, commit,
CodeRabbit).

### Milestone 4 — derive opt-in, leakage stripping, heuristic fix, IR

Red: macro-level tests asserting `#[ortho_config(profiles)]` generates a global
`--profile` argument with the `<PREFIX>PROFILE` environment fallback; that
legacy derives are byte-for-byte unaffected; compile-failure tests for the
three collision projections (field claiming the `profile` key, the `--profile`
flag, or the `<PREFIX>PROFILE` binding, D6); tests that the selector never
appears in the merged value (environment layer stripped of the selector key,
generated flag excluded from the serialized CLI layer, constraint 7); the
flag-equals-default red test ("`--profile ci --retries 3` with default 3 and
profile 7 yields 3"); and docs-IR tests for the new `DocMetadata.profiles`
field (D15) with re-baselined IR goldens. Behavioural scenarios in
`profiles.feature` for the five-tier precedence, including one where the flag
value equals the built-in default. A bounded property test asserts precedence
at the composer level (not through a full CLI): a small fixed key alphabet,
scalar values, default case count, and committed regression files; the strategy
deliberately generates equal-to-default values.

Green: parse the `profiles` struct attribute into `StructAttrs`; in
`build_compose_layers_impl`, when enabled: resolve the selection from the
already-parsed CLI (or directly from the environment when clap parsing failed,
so selection errors do not mask parse errors), interpose profile extraction on
the same discovered layers (single pass — no second discovery call), splice
`push_profile` between the file loop and `push_environment`, strip the selector
from the environment and CLI layers, adjust the `MergeComposer::with_capacity`
hint, and gate `push_cli` on clap value-source information instead of
`differs_from_defaults` for opted-in structs (tolerance 7 if this cannot be
contained); emit `DocMetadata.profiles` (D15); generate
`load_with_profile_from_iter` returning `ProfileLoadOutcome` (D14).

Refactor: keep the generated code readable; the generated body calls library
helpers rather than open-coding extraction.

Validation: as before; this milestone also adds an end-to-end behavioural test
exercising a real derived CLI through the new entry point with a temp config
file, environment guard, and flags.

### Milestone 5 — agent context and runtime exposure

Red: update the wire-contract expectations first. The complete artefact list,
landed atomically: `agent_context_wire_contract.json` (respecting its
line-ending pinning), the contract-support assertion helpers
(`tests_contract_support.rs`), the round-trip property strategy
(`tests_round_trip.rs`), the agent-context insta snapshot, the three
`cargo-orthohelp` goldens plus a new profile-enabled fixture variant, and the
three `examples/hello_world` agent-context surfaces. A byte-identity test
proves the legacy unsupported serialization is unchanged.

Green: introduce `ProfilesDeclaration` and `ProfileSelectionContract` with
constructors (D7); retype `AgentContext.profiles` (the pre-approved breaking
change); wire `bridge_ir_to_agent_context` to map `DocMetadata.profiles` (D15)
into the declaration; update `cargo-orthohelp` and `examples/hello_world`.

Refactor: extract a shared fixture-builder for agent-context test construction
so future field additions (9.1.2's `redaction`, 9.1.3's `store`/`list_command`
population) touch one helper, not six fixture families.

Validation: as before, plus explicit evidence that the legacy fixture bytes for
`profiles` are unchanged.

### Milestone 6 — documentation, roadmap, retrospective

Update `docs/users-guide.md`: a new subsection under "Loading configuration and
precedence rules" documenting profiles, the five-tier precedence, the selector
flag and environment variable, reserved names and keys, the subcommand
limitation (D11), the `--profile default` contract, and the structured errors;
and an update under "Documentation and agent contracts" showing the widened
`profiles` JSON with a `json` example, the recommended `SelectedProfile`
rendering for `context --json`, and compatibility caveats, following the
agent-context precedent. Update `docs/developers-guide.md` (schema ownership:
the new fields and their omitted-when-absent rule; testing conventions: the
scoped googletest/pretty_assertions usage, D9). Record the changelog entry for
the D7 Rust-level break. Mark roadmap 9.1.1 and its three sub-items done (with
the 9.1.3 deferral note from milestone 1). Complete this plan's retrospective
and set Status: COMPLETE.

Validation: full gates plus docs gates via scrutineer; final CodeRabbit review;
final commit.

## Concrete steps

All commands run at the repository root. Gates are delegated to the
`scrutineer` subagent, which runs them sequentially and logs to
`/tmp/<action>-ortho-config-9-1-1-profile-metadata.out`; on failure, read the
cited log rather than re-running.

Environment prerequisites: `rust-toolchain.toml` requires the `rust-analyzer`
component (added upstream on 2026-08-07), so `rustup` must have it installed
for the pinned toolchain before the gates will run. Markdown is formatted with
`mdformat` via `make fmt`; run it before committing documentation so the
rewrapping matches the repository baseline rather than appearing as churn in a
later commit.

```console
$ git branch --show-current
9-1-1-profile-metadata
$ make check-fmt && make typecheck && make lint && make test  # via scrutineer
$ make markdownlint && make nixie                             # docs gates
$ coderabbit review --agent                                   # after green gates
```

Focused test commands during red/green cycles:

```text
cargo test -p ortho_config profile            # unit tests for the module
cargo test -p ortho_config --test rstest_bdd  # behavioural scenarios
cargo insta review                            # snapshot changes, if intentional
```

Commit after every green milestone with an imperative, ≤50-character subject
and a wrapped Markdown body, per `AGENTS.md`.

## Validation and acceptance

The feature specification driving milestones 3 and 4
(`ortho_config/tests/features/profiles.feature`, abridged to the pinned
scenarios; the file may add more):

```gherkin
Feature: Profile selection and precedence

  Scenario: Selected profile overlays file values
    Given a config file with key "retries" set to "3"
    And the same file defines profile "ci" with "retries" set to "7"
    When the CLI loads with "--profile ci"
    Then the merged value of "retries" is "7"

  Scenario: Environment beats the selected profile
    Given a config file defining profile "ci" with "retries" set to "7"
    And the environment sets the "retries" key to "9"
    When the CLI loads with "--profile ci"
    Then the merged value of "retries" is "9"

  Scenario: An explicit flag equal to the default beats the profile
    Given a struct default of "3" for "retries"
    And a config file defining profile "ci" with "retries" set to "7"
    When the CLI loads with "--profile ci --retries 3"
    Then the merged value of "retries" is "3"

  Scenario: The profile flag beats the selector environment variable
    Given a config file defining profiles "ci" and "local"
    And the selector environment variable names profile "local"
    When the CLI loads with "--profile ci"
    Then the selected profile is "ci" with source "flag"

  Scenario: Selecting an unknown profile fails with the available names
    Given a config file defining profiles "ci" and "local"
    When the CLI loads with "--profile staging"
    Then loading fails naming "staging" from source "flag"
    And the error lists available profiles "ci" and "local"

  Scenario: An env-selected profile with no config files fails clearly
    Given no configuration files are discoverable
    And the selector environment variable names profile "ci"
    When the CLI loads
    Then loading fails naming "ci" from the selector environment variable
    And the error states that no configuration files were found

  Scenario: A profile table must not configure subcommands
    Given a config file defining profile "ci" containing a "cmds" table
    When the CLI loads with "--profile ci"
    Then loading fails identifying the forbidden "cmds" key in "ci"
```

Red-Green-Refactor evidence is recorded per milestone in "Progress" and
"Artefacts and notes": each red command with its expected failure, the green
command passing, and the post-refactor full-gate pass.

Quality criteria:

- Tests: `make test` passes; new unit (rstest + googletest +
  pretty_assertions), behavioural (rstest-bdd), snapshot (insta), and property
  (proptest, bounded per milestone 4) tests all present and passing; the legacy
  agent-context bytes for the unsupported case are proven unchanged.
- Lint/typecheck: `make check-fmt`, `make typecheck`, `make lint` clean for
  the files this task changes (constraint 8 scoping).
- Docs: `make markdownlint` and `make nixie` clean.
- Review: `coderabbit review --agent` raised concerns cleared at every
  milestone.

## Idempotence and recovery

Every milestone is an ordinary commit on `9-1-1-profile-metadata`; recovery is
`git revert` or resetting to the previous milestone commit. Snapshot updates go
through `cargo insta review` so accidental acceptance is visible in the diff.
No step mutates state outside the worktree except `/tmp` logs. The downstream
rollback story (removing the opt-in attribute) is recorded in ADR-008
(milestone 1).

## Artefacts and notes

Populated during implementation with focused transcripts (red failures, green
passes, gate summaries, fixture diffs).

### Milestone 2, red (2026-08-09)

Focused tests fail to compile exactly as expected — the new API does not exist
yet:

```text
$ cargo test -p ortho_config --lib tests_profile_layer
error[E0599]: no method named `push_profile` found for struct `MergeComposer`
error[E0599]: no variant or associated item named `Profile` found for enum
             `MergeProvenance`
error[E0599]: no function or associated item named `profile` found for struct
             `MergeLayer`

$ cargo test -p ortho_config --test rstest_bdd
error[E0599]: no method named `push_profile` found for struct `MergeComposer`
error[E0599]: no variant or associated item named `Profile` found for enum
             `MergeProvenance`
```

Test placement notes: the derive macro cannot be invoked inside the library
crate (its generated code names the consumer crate), so the declarative unit
tests use a handwritten `DeclarativeMerge` state machine (as in the trait's doc
example) and the generated-label diagnostics test lives in
`ortho_config/tests/declarative_merge.rs` where a derived struct exists.

### Milestone 2, green (2026-08-09)

Focused tests pass after adding the API:

```text
$ cargo test -p ortho_config --lib tests_profile_layer
test result: ok. 5 passed   # ordering, merge precedence, accessors, borrow

$ cargo test -p ortho_config --test declarative_merge
test result: ok. 22 passed  # incl. merge_layers_label_profile_provenance_in_diagnostics

$ cargo test -p ortho_config --test rstest_bdd
test result: ok. 57 passed  # incl. the profile-layer composer scenario

$ cargo test -p ortho_config_macros
test result: ok. 123 passed  # guards.rs + expected_merge_impl_empty.rs.txt updated
```

### Milestone 3, red (2026-08-09)

The pre-planned unit test modules fail to compile — the profile types and
`OrthoError` variants do not exist yet:

```text
$ cargo test -p ortho_config --lib profile
error[E0432]: unresolved imports `crate::profile::ProfileName`,
             `crate::profile::ProfileSource`, `crate::profile::SelectedProfile`,
             `crate::profile::extract_profile_layers`
error[E0599]: no variant named `UnknownProfile` / `InvalidProfileName` /
             `ReservedProfileName` / `ProfileForbiddenKey` found for
             `error::types::OrthoError`
```

### Milestone 3, green (2026-08-09)

Focused tests pass after implementing the module:

```text
$ cargo test -p ortho_config --lib profile
test result: ok. 37 passed   # grammar + property, selection, extraction, errors

$ cargo test -p ortho_config --test rstest_bdd
test result: ok. 64 passed   # incl. seven profiles.feature scenarios

$ cargo test -p ortho_config --lib
test result: ok. 187 passed
```

BDD step notes: `rstest-bdd` binds step placeholder arguments positionally, and
the `{flags}` placeholder captures surrounding quotes, so the step layer
normalizes the raw flag string before parsing. The `Slot::with_mut` helper is a
no-op on an empty slot, so accumulation steps use `get_or_insert_with`.

### Milestone 4, green (2026-08-09)

The derive opt-in works end to end — the BDD scenarios now run through the real
derived CLI (`ProfilesConfig::load_with_profile_from_iter`) against a jailed
`.app.toml` and `APP_` environment:

```text
$ cargo test -p ortho_config --test rstest_bdd
test result: ok. 66 passed  # nine profiles.feature scenarios incl.
                            # flag-equals-default and selector-never-leaks

$ cargo test -p ortho_config --test compile_fail
test result: ok. 1 passed   # three profile collision trybuild fixtures

$ cargo test -p ortho_config --lib
test result: ok. 188 passed # incl. the bounded five-tier precedence proptest
$ cargo test -p ortho_config_macros --lib
test result: ok. 132 passed # incl. profiles parse + profile-flag token tests
```

Design notes: the selector source is attributed via clap's
`ArgMatches::value_source` (a flag counts only on a command-line origin, so an
env-filled value stays attributed to the environment); the flag-equals-default
fix gates the CLI push on `differs_from_defaults || explicitly_provided` so a
clap `default_value_t` that differs from the ortho default keeps its current
behaviour; the clap `env` feature was added to the ortho_config dependency to
support the generated `env = "<PREFIX>PROFILE"` attribute (D3).

### Milestone 5, green (2026-08-09)

The `AgentContext.profiles` retype lands without disturbing the wire contract:

```text
$ cargo test -p ortho_config --lib agent_context
test result: ok. 48 passed  # incl. the byte-identity test proving the
                            # unsupported serialization is unchanged

$ cargo test -p cargo-orthohelp --test golden_tests
test result: ok. 16 passed  # new agent_context__profile_fixture.json.snap
                            # shows supported + selection; legacy goldens
                            # byte-identical

$ cargo test -p hello_world
test result: ok. 67 passed  # all three agent-context surfaces unchanged
```

Design notes: the retype is a pre-approved Rust-level break (D7) with
constructors; the wire contract is additive because the optional fields are
omitted when absent. The fixture-builder refactor (extract one helper to build
agent-context fixtures across crates) is deferred: the wire change is additive
so no fixture family gained fields, and the profile golden reuses the existing
`doc()`/`sample_agent_context()` builders. The cross-crate extraction can ride
along with 9.1.2's `redaction` field if fixture sprawl materializes.

## Interfaces and dependencies

New and changed public API in `ortho_config` (additive unless marked):

```rust
// ortho_config/src/declarative/layer.rs
#[non_exhaustive]
pub enum MergeProvenance { Defaults, File, Profile, Environment, Cli }
// plus MergeLayer::profile(value, path) and the minimal value-access
// mechanism for extraction (an into_parts()/rebuild pair or map_value),
// finalized in milestone 2's refactor step.

// ortho_config/src/profile/ (new module, serde-free runtime types)
pub struct ProfileName(/* validated: [A-Za-z0-9_-]+, not "default" */);
#[non_exhaustive]
pub enum ProfileSource { Flag, Environment }
pub struct SelectedProfile { pub name: ProfileName, pub source: ProfileSource }
pub struct ProfileLoadOutcome<T> { /* private fields */ }
// accessors: config(), into_config(), selection() -> &[SelectedProfile]

// ortho_config/src/error/types.rs (variants on #[non_exhaustive] OrthoError)
// UnknownProfile { selected, source, available /* sorted, capped at 16 */ },
// InvalidProfileName { name },
// ReservedProfileName { name },
// ProfileForbiddenKey { profile, key }

// ortho_config/src/agent_context/mod.rs
pub struct ProfilesDeclaration {
    pub supported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<ProfileSelectionContract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_command: Option<Vec<String>>, // AgentCommand::path convention
}
// constructors: ProfilesDeclaration::unsupported(), ::supported(selection)
pub struct ProfileSelectionContract {
    pub flag: String,    // AgentInput::long convention: no leading "--"
    pub env_var: String, // literal, e.g. "APP_PROFILE"
}
// BREAKING (pre-approved, D7): AgentContext.profiles retyped from
// SupportDeclaration to ProfilesDeclaration.

// ortho_config/src/docs/ir.rs (D15, additive with #[serde(default)])
// DocMetadata.profiles: Option<DocProfilesMeta>
// DocProfilesMeta { flag: String, env_var: String }
```

Generated for opted-in structs only:
`load_with_profile_from_iter(iter) -> OrthoResult<ProfileLoadOutcome<Self>>`
and a `load_with_profile()` convenience; the `--profile` argument also appears
as a normal `AgentInput` on each command so the contract's `flag` field
references an input that exists.

`ProfilesDeclaration` deliberately leaves room for 9.1.2 (a future `redaction`
field) and 9.1.3 (a future `store` field and the `list_command` population).
Derive attribute surface gains the struct-level bare `profiles` key. New
workspace dev dependencies: `googletest`, `pretty_assertions` (D9, scoped). No
new runtime dependencies.

## Signposts: documentation and skills

Read before implementing:

- `docs/agent-native-cli-design.md` §6.7, §8.1, §8.2 — the governing
  contract and its compatibility policy.
- `docs/design.md` §3, §4.3, §4.10, §4.11, §4.17 — merge architecture and
  the subcommand path.
- `docs/rfcs/0002-config-layer-resolution-policy.md` — ownership boundary
  and the D13 seam.
- `docs/documentation-style-guide.md` — ADR template and Markdown rules.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rstest-bdd-users-guide.md`, `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md` — test
  conventions.
- `docs/localizable-rust-libraries-with-fluent.md` — message IDs for the new
  error variants.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — refactor
  stages.
- `docs/whitaker-users-guide.md` — Whitaker Dylint configuration, needed for
  constraint 8's lint gate and D9's dev-dependency verification.

Skills to load during implementation: `leta` (navigation/refactoring),
`rust-router` then `rust-types-and-apis` (newtype and schema shapes),
`rust-errors` (new error variants), `rust-unit-testing` (rstest/googletest/
insta discipline), `proptest` (precedence invariant), `arch-crate-design`
(boundary checks), `arch-decision-records` (ADR-008),
`addressing-whitaker-findings` (if the lint gate reports findings),
`commit-message`, `comenq-coderabbit` (review loop), and `rebase` if `main`
moves.

## Revision note

2026-08-06: revised after the Logisphere design-review panel. Five lenses
reported: structural, alternatives, contracts, failure-mode, and viability. The
scaling and operational-cost lens did not complete, so its concerns —
compile-time cost of the generated code, fixture-maintenance burden as snapshot
families multiply, and continuous-integration cost of the new property tests —
are only partially covered here, via the bounded-proptest requirement in
milestone 4 and the fixture-sprawl risk. A reviewer may wish to run that lens
before approval. The panel confirmed the core architecture and found the gaps
at the seams. This revision: re-labelled the `AgentContext.profiles` retype as
a deliberate Rust-level breaking change with constructors (D7); added
selector-leakage stripping across file, environment, and CLI projections
(constraint 7); added the flag-equals-default heuristic fix (risk 3, milestone
4, tolerance 7); defined the subcommand boundary with `cmds` rejection (D11,
constraint 9); corrected the profile-collection rule to the first-match file
chain (D12); recorded the RFC 0002 seam obligation (D13); designed the
post-load selection surface (`ProfileLoadOutcome`, D14); added the docs-IR
workstream (D15); made `list_command` a path vector deferred to 9.1.3; bounded
the property tests and the error name listing; scoped the new dev dependencies
with a lint/MSRV verification step (D9); amended the §8.2 asymmetry handling
(milestone 1); enumerated the full fixture set (milestone 5); and pre-planned
the test-module layout for the 400-line file cap. Remaining work is unchanged
in shape: six milestones, docs first.

2026-08-07: rebased onto `origin/main` and absorbed the upstream changes that
bear on this plan. `main` added `docs/whitaker-users-guide.md` (now signposted
and cited from D9), required the `rust-analyzer` toolchain component, and
adopted an `mdformat` baseline this document has been reformatted to match.
Constraint 8 was tightened: the full gate suite was observed green at the
rebase base, so a red gate is this branch's regression by default rather than
inherited noise. No design decision changed; the plan's shape and status are
unaffected.

2026-08-12: recorded the completed post-implementation review follow-up. The
review retained the profile contract and its distinct BDD and named-test
surfaces while extracting private helpers for repeated setup and assertions.
The progress, discoveries, decision log, and retrospective now cite the current
validation state and request a CodeScene reanalysis rather than a suppression.
The roadmap already marks task 9.1.1 complete, so it required no change.

2026-08-12: corrected D4's implementation record after review found the 16-name
cap applied only to display text. `AvailableProfileNames` now stores the capped
sorted payload and an omitted-name count, and the extraction test observes both
that payload bound and the existing `and 4 more` suffix.
