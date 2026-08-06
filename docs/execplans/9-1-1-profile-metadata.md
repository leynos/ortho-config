# Design and implement optional profile metadata (roadmap 9.1.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, and `Outcomes & retrospective` must be kept up to date as work
proceeds.

Status: DRAFT (awaiting approval; no implementation may begin before the plan
is explicitly approved)

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
- Selecting an unknown profile fails with a clear error that names the unknown
  profile and lists the available ones.
- `cargo orthohelp --format agent-context` for a profile-enabled CLI emits
  `profiles.supported = true` plus selection metadata (flag, environment
  variable, and how to list profiles), while legacy derives keep emitting
  `{ "supported": false }` unchanged.
- A downstream `context --json` command can report which profile is selected
  and why (flag, environment, or none) via a new runtime type.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` all pass.

Profile support is strictly opt-in. OrthoConfig provides the reusable
contract and merge mechanics; downstream applications own profile content,
naming, and any storage helpers (deferred to roadmap 9.1.3). Redaction
metadata is deferred to roadmap 9.1.2 but the type shapes chosen here must not
block it.

## Plain-language glossary

- Profile: a named bundle of configuration values defined under a
  `[profile.<name>]` table in a discovered configuration file. Selecting a
  profile overlays its values on the file layer.
- Selected profile: the single profile chosen for this invocation via the
  `--profile` flag or the `<PREFIX>PROFILE` environment variable. At most one
  profile is selected.
- Layer: one source of configuration values in the merge pipeline. Layers are
  merged in a fixed precedence order by `MergeComposer`
  (`ortho_config/src/declarative/composer.rs`).
- Provenance: the tag recording which kind of source a layer came from,
  modelled by the `#[non_exhaustive]` enum `MergeProvenance`
  (`ortho_config/src/declarative/layer.rs`).
- Agent context: the compact, machine-oriented JSON document describing a CLI
  to agents, produced by `cargo orthohelp --format agent-context` and
  re-served by downstream `<tool> context --json` commands. Schema types live
  in `ortho_config::agent_context` with version constant
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` (currently `"1"`).
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
2. Profile support is opt-in. Existing derives that do not opt in must compile
   unchanged, keep their current four-layer merge order, gain no new CLI
   flags, and keep emitting `profiles: { "supported": false }` in agent
   context. The schema v1 defaulting table
   (`docs/agent-native-cli-design.md` §8.1) must remain satisfied.
3. The agent-context schema may only change additively: new optional fields
   with `#[serde(default)]`; consumers that ignore unknown fields must keep
   working; `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` stays `"1"`. The existing
   wire-contract fixture may gain fields but no existing field may change
   shape or meaning.
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
   profile body (no bootstrap circularity): only the flag and the dedicated
   environment variable choose the profile.
8. All gates (`make check-fmt`, `make typecheck`, `make lint`, `make test`)
   must pass at every milestone boundary, followed by a clean
   `coderabbit review --agent` pass before the next milestone starts.

## Tolerances (exception triggers)

Stop and escalate when any threshold below is reached.

1. Dependencies: adding any dependency other than the pre-approved dev
   dependencies `googletest` and `pretty_assertions` (see decision D9)
   requires escalation.
2. Public API: removing or changing the signature of any existing public item
   requires escalation. Additive public API listed in "Interfaces and
   dependencies" is pre-approved.
3. Schema: any agent-context change that is not purely additive-with-default
   requires escalation (constraint 3).
4. Size: if a single milestone exceeds roughly 600 net new lines of
   non-test code, or the whole task exceeds roughly 2,500 net lines including
   tests and docs, stop and escalate with a slimming proposal.
5. Iterations: if a gate still fails after three fix attempts for the same
   root cause, stop and escalate with the log evidence.
6. Ambiguity: if profile semantics interact with an existing feature in a way
   this plan does not cover (for example `extends` inheritance inside a
   profile table, or subcommand-scoped profiles), stop and present options
   rather than inventing semantics.

## Approved decisions

These decisions become binding when the plan is approved. Each records the
choice, rationale, and the rejected alternative.

- D1 — Profiles are named config overlays inside existing files. A profile is
  a `[profile.<name>]` table within any discovered configuration file (TOML
  shown; JSON5/YAML equivalents follow the same key path `profile.<name>`).
  Rationale: mirrors Cargo's `[profile.<name>]`, avoids the AWS
  `[profile x]`/`[default]` header asymmetry, reuses existing discovery
  machinery, and needs no new file formats. Rejected: profile-per-file
  suffixes (`app.staging.toml`, the mise/Spring pattern) — more discovery
  surface and a second naming convention for little gain; a separate profile
  store file — deferred to roadmap 9.1.3 by design.
- D2 — The profile layer is a first-class merge layer. Add
  `MergeProvenance::Profile`, `MergeLayer::profile(...)`, and
  `MergeComposer::push_profile(...)`. The generated composition splices the
  profile layer after all file layers and before the environment layer.
  Rationale: the composer is the single enforcement point of precedence;
  a first-class provenance keeps diagnostics and future redaction (9.1.2)
  honest. Rejected: pre-merging profile values into the file layer's value —
  loses provenance and makes the documented five-tier precedence unprovable.
- D3 — Selection is stateless: `--profile <name>` flag with
  `env = "<PREFIX>PROFILE"` fallback on the generated clap argument, so the
  flag beats the environment variable for selection. No persisted
  "current profile" state. Rationale: matches AWS CLI and dbt; persisted
  selection (kubectl, gcloud) causes the classic wrong-context incident and
  would require a store, which is 9.1.3's question. Rejected: a persisted
  current-profile file.
- D4 — Unknown profile names are a hard error. If a profile is selected but
  no `[profile.<name>]` table exists in any discovered file, loading fails
  with a new semantic error variant that names the unknown profile and lists
  the available names in deterministic order. Rationale: figment's silent
  fallback on unknown profiles is a documented footgun; every operational
  tool surveyed (AWS, kubectl, Cargo, dbt, docker) errors loudly. Rejected:
  silent fallback to base values.
- D5 — Profile names are case-sensitive and validated against the grammar
  `[A-Za-z0-9_-]+` (non-empty). The name `default` is reserved: defining
  `[profile.default]` is an error, and selecting `default` is equivalent to
  selecting no profile. Rationale: Cargo's validation grammar is the
  clearest precedent; case-sensitivity matches AWS/kubectl and avoids
  locale-dependent folding; reserving `default` prevents two spellings of
  the base configuration. Rejected: figment-style case-insensitive matching
  (surprising duplicates such as `Dev` vs `dev`); reserving `global`
  (OrthoConfig has no global-override tier, so the name stays free).
- D6 — Opt-in via a struct-level derive attribute `#[ortho_config(profiles)]`.
  Only structs carrying the attribute gain the generated `--profile` flag,
  the selector environment variable, the profile merge layer, and
  `profiles.supported = true` in agent context. Rationale: constraint 2
  requires legacy derives to be untouched; an attribute is the established
  opt-in mechanism (`post_merge_hook`, `discovery(...)`). Rejected:
  auto-enabling when a `[profile.*]` table is present (spooky action at a
  distance, and the flag surface must be static for agent context).
- D7 — Agent-context exposure widens `AgentContext.profiles` from
  `SupportDeclaration` to a new `ProfilesDeclaration` type that serializes
  identically for the unsupported case (`{ "supported": false }`) and adds
  optional, defaulted fields for the supported case: the selector flag, the
  selector environment variable name, and an optional listing-command path.
  Selected-profile semantics (which profile is active now and why) are a
  runtime concern, exposed through a new runtime type `SelectedProfile`
  that downstream `context --json` commands may embed; the static generated
  context documents the mechanism, not the moment. Rationale: keeps the
  schema change additive (constraint 3) while satisfying roadmap 9.1.1's
  "profile support, profile listing commands, and selected-profile
  semantics"; splitting static contract from runtime state follows
  ADR-007's `context --json` model. Rejected: embedding live selection
  state in the build-time artefact (it cannot know it).
- D8 — The selector environment variable defaults to `<PREFIX>PROFILE`,
  derived from the existing `prefix` attribute exactly as other environment
  keys are (for example prefix `APP_` gives `APP_PROFILE`). Rationale: the
  de-facto standard (`AWS_PROFILE`, `DBT_TARGET` analogues) and consistent
  with OrthoConfig's environment naming. Rejected: a configurable variable
  name in 9.1.1 — additive later if a consumer needs it.
- D9 — Add `googletest` and `pretty_assertions` as workspace dev
  dependencies, per the task brief's testing requirements. They are
  test-only, so the runtime dependency surface is unchanged. Rationale:
  richer matcher output for the new merge-order and error-path assertions.
  Rejected: continuing with bare `assert_eq!` for new tests.
- D10 — Out of scope, recorded to prevent drift: profile inheritance
  (`inherits =`), multiple simultaneous profiles, secret redaction (9.1.2),
  any profile store helper (9.1.3), and subcommand-scoped profiles. Type
  shapes must not preclude 9.1.2/9.1.3 (see "Interfaces and dependencies").

## Risks

- Risk: the generated `--profile` flag collides with an existing downstream
  field named `profile` or an existing `-p`/`--profile` argument.
  Severity: medium. Likelihood: medium.
  Mitigation: the derive emits a compile-time error when the opt-in attribute
  is present and a field already claims `--profile`; documented in the
  migration notes. `docs/agent-native-cli-design.md` §2.2 already declares
  that on shape conflict the OrthoConfig shape wins.
- Risk: widening `AgentContext.profiles` breaks the wire contract fixture,
  round-trip property tests, or downstream consumers.
  Severity: high. Likelihood: medium.
  Mitigation: `ProfilesDeclaration` serializes byte-identically to
  `SupportDeclaration` when unsupported; fixtures updated in the same
  milestone as the type; the existing unknown-field forward-compatibility
  tests guard consumers; constraint 3 forbids non-additive change.
- Risk: profile tables interact badly with file-level `extends` inheritance
  (does a base file's `[profile.x]` merge with the extending file's?).
  Severity: medium. Likelihood: medium.
  Mitigation: define the rule up front — profile tables are collected from
  every discovered file after `extends` resolution, in the same order as the
  file layers themselves, and merge in that order. A BDD scenario pins this.
  If implementation reveals the `extends` machinery cannot support this
  cleanly, tolerance 6 triggers escalation.
- Risk: RFC 0002 (file-layer resolution policy, still Proposed) later
  restructures file-layer assembly underneath the profile layer.
  Severity: low. Likelihood: medium.
  Mitigation: the profile layer only consumes the ordered file values the
  composer already holds; the ADR records that RFC 0002's `FileLayerOutcome`
  must expose profile tables if it lands.
- Risk: the derive-macro splice in `build_compose_layers_impl` is subtle and
  regressions would silently reorder precedence.
  Severity: high. Likelihood: low.
  Mitigation: a proptest invariant asserts the five-tier precedence for
  arbitrary value assignments across layers; BDD scenarios pin flag-beats-env
  and env-beats-profile explicitly.

## Progress

- [ ] Stage A: plan drafted, expert-reviewed, and submitted for approval.
- [ ] Milestone 1: ADR and design documentation (merge order documented
      before code, per constraint 6).
- [ ] Milestone 2: profile merge layer in the composer (red → green →
      refactor).
- [ ] Milestone 3: profile extraction, selection resolution, validation, and
      error paths in `ortho_config`.
- [ ] Milestone 4: derive-macro opt-in, generated `--profile` flag, and
      end-to-end precedence behaviour.
- [ ] Milestone 5: agent-context schema widening, `SelectedProfile` runtime
      type, `cargo-orthohelp` bridge, fixtures and snapshots.
- [ ] Milestone 6: user-facing and contributor documentation, roadmap
      tick, retrospective.

## Surprises & discoveries

- Observation: `googletest` and `pretty_assertions` are named in the task
  brief and several ExecPlans but are not yet dependencies of any workspace
  crate.
  Evidence: no matches in any `Cargo.toml` at planning time.
  Impact: decision D9 adds them as dev dependencies; tolerance 1 pre-approves
  exactly these two.

## Decision log

- Decision: plan drafted with decisions D1–D10 above; profile-as-overlay
  design chosen after prior-art survey (AWS, kubectl, gcloud, dbt, Cargo,
  docker, figment, config-rs, mise, Spring).
  Rationale: recorded per decision in "Approved decisions".
  Date/Author: 2026-08-06, planning agent.

## Outcomes & retrospective

To be completed at milestone boundaries and on completion.

## Context and orientation

The workspace (`/` is the repository root) contains:

- `ortho_config/` — the core library. Relevant modules:
  `src/declarative/layer.rs` (`MergeProvenance`, `MergeLayer`),
  `src/declarative/composer.rs` (`MergeComposer`, `push_defaults`,
  `push_file`, `push_environment`, `push_cli`, generic `push_layer`),
  `src/discovery/` (config-file discovery), `src/agent_context/`
  (schema types, JSON serialization, wire-contract fixture at
  `src/agent_context/fixtures/agent_context_wire_contract.json`, tests in
  `src/agent_context/tests*.rs`, insta snapshots in
  `src/agent_context/snapshots/`).
- `ortho_config_macros/` — the derive. `src/derive/parse/mod.rs` parses
  struct attributes (`StructAttrs`) and field attributes;
  `src/derive/load_impl.rs::build_compose_layers_impl` emits the canonical
  layer order: `push_defaults` → file layers → `push_environment` →
  `push_cli`.
- `cargo-orthohelp/` — documentation and agent-context generator;
  `src/agent_context/mod.rs::bridge_ir_to_agent_context` builds the
  `AgentContext`; golden outputs in `tests/golden/agent_context__*.json.snap`.
- `examples/hello_world/` — dogfood binary with an agent-context snapshot
  test.
- Behavioural tests: feature files in `ortho_config/tests/features/`
  (`cli_precedence.feature`, `merge_composer.feature`, and so on) run by the
  `rstest_bdd` test target.
- Governing documents: `docs/agent-native-cli-design.md` §6.7 (persistent
  profiles) and §8.1 (schema v1 defaulting table), `docs/design.md` §3, §4.10
  and §4.17 (current four-tier precedence statements), `docs/roadmap.md`
  §9.1, ADR-003 (schema ownership), ADR-007 (`context --json` naming),
  RFC 0002 (file-layer resolution policy, Proposed).

Environment-variable-dependent tests must use the guards in
`test_helpers` (`ortho_config_test_helpers`) — raw environment mutation in
tests is forbidden by `AGENTS.md`.

## Plan of work

### Stage A — approval (no code changes)

Draft this plan, run the community-of-experts design review, revise, and
submit for approval as a draft pull request. Implementation starts only after
explicit approval. The remainder of this section is the approved route.

### Milestone 1 — documentation before code (ADR + design updates)

Constraint 6 requires the merge order and migration impact documented before
behavioural change. Write `docs/adr-008-profile-selection-and-layering.md`
following the ADR template in `docs/documentation-style-guide.md` (Status:
Accepted on plan approval; Context; Decision Drivers; Options Considered;
Decision Outcome capturing D1–D8; Migration Plan covering the §2.2 soft-tier
consumer adapters; Known Risks). Update `docs/design.md` precedence
statements (§3 provider list, §4.10 `extends` ordering, §4.17) to insert the
selected-profile tier, marked as opt-in. Update
`docs/agent-native-cli-design.md` §6.7 to record the resolved merge order and
§8.1's table with the new defaulted fields. Register the ADR and this plan in
`docs/contents.md`. Update `docs/roadmap.md` 9.1.1 notes to cite the ADR.

Validation: `make markdownlint` and `make nixie` pass; scrutineer runs the
docs gates; CodeRabbit review of the docs commit is clean.

### Milestone 2 — profile layer in the merge engine (red → green → refactor)

Red: add rstest unit tests in `ortho_config/src/declarative/` asserting that
a layer pushed via the new `push_profile` merges above files and below
environment, and that `MergeProvenance::Profile` round-trips through
diagnostics. Extend `ortho_config/tests/features/merge_composer.feature` with
a profile-layer scenario. Run the focused tests and record the expected
failures (missing variant/method).

Green: add `MergeProvenance::Profile` (additive; the enum is
`#[non_exhaustive]`), `MergeLayer::profile(value, path, name)` carrying the
profile name for diagnostics, and `MergeComposer::push_profile`. Make the
red tests pass.

Refactor: deduplicate constructor plumbing; ensure exhaustive-match sites on
`MergeProvenance` across the crate handle the new variant deliberately.

Validation: focused tests pass; full gate run
(`make check-fmt`, `make typecheck`, `make lint`, `make test`) via
scrutineer; commit; CodeRabbit review clean.

### Milestone 3 — extraction, selection, validation, errors

Red: unit tests (rstest, googletest matchers, pretty_assertions) for: a
`[profile.<name>]` table extracted from a file value in discovery order;
selection resolution (flag beats environment variable); name-grammar
rejection; reserved-`default` rules (defining it errors, selecting it is a
no-op); unknown-profile error listing available names deterministically; and
profile tables collected across multiple discovered files including after
`extends` resolution. Add a new
`ortho_config/tests/features/profiles.feature` covering the happy path,
unknown profile, reserved name, and env-selected profile scenarios (the
feature text is embedded in "Validation and acceptance" below). Add proptest
strategies generating arbitrary profile names to pin the validation grammar
(valid grammar accepted, anything else rejected).

Green: implement in a new module `ortho_config/src/profile/mod.rs`:
`ProfileName` (validated newtype), `ProfileSelection` resolution helper,
`SelectedProfile { name, source: ProfileSource }` with
`ProfileSource::{Flag, Environment}`, extraction of profile tables from the
ordered file values, and new error variants on the existing semantic error
enum (unknown profile, invalid name, reserved name). Profile keys are
stripped from the base file layer so `[profile.*]` content never leaks into
unselected loads.

Refactor: consolidate with discovery types; keep RFC 0002's boundary — no
application literals.

Validation: as milestone 2 (focused red/green evidence, full gates,
commit, CodeRabbit).

### Milestone 4 — derive opt-in and end-to-end precedence

Red: trybuild-style or macro-level tests asserting `#[ortho_config(profiles)]`
generates a global `--profile` argument with the `<PREFIX>PROFILE`
environment fallback, that legacy derives are byte-for-byte unaffected, and a
compile-failure test for the flag-collision case. Behavioural scenarios in
`profiles.feature` for the five-tier precedence: a value set in defaults,
file, profile, environment, and flag resolves to the flag; removing the flag
resolves to environment; and so on down the chain. A proptest invariant
asserts precedence for arbitrary assignments of a key across layers.

Green: parse the `profiles` struct attribute into `StructAttrs`; in
`build_compose_layers_impl`, when enabled, resolve the selection, then splice
`push_profile` between the file loop and `push_environment`; surface the
selection so `SelectedProfile` is available to callers after load.

Refactor: keep the generated code readable; extend
`docs/developers-guide.md` notes if generation conventions change.

Validation: as before; this milestone also adds an end-to-end behavioural
test exercising a real derived CLI through `load_from_iter` with a temp
config file, environment guard, and flags.

### Milestone 5 — agent context and runtime exposure

Red: update the wire-contract expectations first: extend
`agent_context_wire_contract.json`, the contract-support assertions, the
round-trip proptest strategy, and add insta snapshot expectations for a
profile-enabled context; failing tests demonstrate the missing fields.
Golden tests in `cargo-orthohelp/tests/golden/` gain a profile-enabled
fixture variant.

Green: introduce `ProfilesDeclaration { supported, selection:
Option<ProfileSelectionContract>, list_command: Option<String> }` with
`ProfileSelectionContract { flag, env_var }`, all new fields
`#[serde(default)]` and omitted when `None`; change
`AgentContext.profiles` to the new type (serialization for the unsupported
case is unchanged, satisfying constraint 3); wire
`bridge_ir_to_agent_context` to populate it from derive metadata; update the
`examples/hello_world` snapshot.

Refactor: share shape assertions in `tests_contract_support.rs`.

Validation: as before, plus explicit evidence that the legacy fixture bytes
for `profiles` are unchanged.

### Milestone 6 — documentation, roadmap, retrospective

Update `docs/users-guide.md`: a new subsection under "Loading configuration
and precedence rules" documenting profiles, the five-tier precedence, the
selector flag and environment variable, reserved names and errors; and an
update under "Documentation and agent contracts" showing the widened
`profiles` JSON with a `json` example and compatibility caveats, following
the agent-context precedent. Update `docs/developers-guide.md` (schema
ownership section: the new fields and their defaulting rules; testing
conventions for the new googletest/pretty_assertions dev dependencies).
Mark roadmap 9.1.1 and its three sub-items done. Complete this plan's
retrospective and set Status: COMPLETE.

Validation: full gates plus docs gates via scrutineer; final CodeRabbit
review; final commit.

## Concrete steps

All commands run at the repository root. Gates are delegated to the
`scrutineer` subagent, which runs them sequentially and logs to
`/tmp/<action>-ortho-config-9-1-1-profile-metadata.out`; on failure, read the
cited log rather than re-running.

```console
$ git branch --show-current
9-1-1-profile-metadata
$ make check-fmt && make typecheck && make lint && make test  # via scrutineer
$ make markdownlint && make nixie                             # docs gates
$ coderabbit review --agent                                   # after green gates
```

Focused test commands during red/green cycles:

```console
cargo test -p ortho_config profile            # unit tests for the module
cargo test -p ortho_config --test rstest_bdd  # behavioural scenarios
cargo insta review                            # snapshot changes, if intentional
```

Commit after every green milestone with an imperative, ≤50-character subject
and a wrapped Markdown body, per `AGENTS.md`.

## Validation and acceptance

The feature specification driving milestones 3 and 4
(`ortho_config/tests/features/profiles.feature`, abridged to the four pinned
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

  Scenario: The profile flag beats the selector environment variable
    Given a config file defining profiles "ci" and "local"
    And the selector environment variable names profile "local"
    When the CLI loads with "--profile ci"
    Then the selected profile is "ci" with source "flag"

  Scenario: Selecting an unknown profile fails with the available names
    Given a config file defining profiles "ci" and "local"
    When the CLI loads with "--profile staging"
    Then loading fails naming "staging" and listing "ci" and "local"
```

Red-Green-Refactor evidence is recorded per milestone in "Progress" and
"Artefacts and notes": each red command with its expected failure, the green
command passing, and the post-refactor full-gate pass.

Quality criteria:

- Tests: `make test` passes; new unit (rstest + googletest +
  pretty_assertions), behavioural (rstest-bdd), snapshot (insta), and
  property (proptest) tests all present and passing; legacy agent-context
  fixtures unchanged for the unsupported case.
- Lint/typecheck: `make check-fmt`, `make typecheck`, `make lint` clean.
- Docs: `make markdownlint` and `make nixie` clean.
- Review: `coderabbit review --agent` raised concerns cleared at every
  milestone.

## Idempotence and recovery

Every milestone is an ordinary commit on `9-1-1-profile-metadata`; recovery
is `git revert` or resetting to the previous milestone commit. Snapshot
updates go through `cargo insta review` so accidental acceptance is visible
in the diff. No step mutates state outside the worktree except `/tmp` logs.

## Artefacts and notes

Populated during implementation with focused transcripts (red failures,
green passes, gate summaries, fixture diffs).

## Interfaces and dependencies

New public API in `ortho_config` (all additive):

```rust
// ortho_config/src/declarative/layer.rs
#[non_exhaustive]
pub enum MergeProvenance { Defaults, File, Profile, Environment, Cli }

// ortho_config/src/profile/mod.rs
pub struct ProfileName(/* validated, [A-Za-z0-9_-]+, not "default" */);
pub enum ProfileSource { Flag, Environment }
pub struct SelectedProfile { pub name: ProfileName, pub source: ProfileSource }

// ortho_config/src/agent_context/mod.rs
pub struct ProfilesDeclaration {
    pub supported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<ProfileSelectionContract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_command: Option<String>,
}
pub struct ProfileSelectionContract { pub flag: String, pub env_var: String }
```

`ProfilesDeclaration` deliberately leaves room for 9.1.2 (a future
`redaction` field) and 9.1.3 (a future `store` field) without reshaping.
Derive attribute surface gains the struct-level bare `profiles` key. New
workspace dev dependencies: `googletest`, `pretty_assertions` (D9). No new
runtime dependencies.

## Signposts: documentation and skills

Read before implementing:

- `docs/agent-native-cli-design.md` §6.7, §8.1 — the governing contract.
- `docs/design.md` §3, §4.3, §4.10, §4.11, §4.17 — merge architecture.
- `docs/rfcs/0002-config-layer-resolution-policy.md` — ownership boundary.
- `docs/documentation-style-guide.md` — ADR template and Markdown rules.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rtest-bdd-users-guide.md`, `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md` — test
  conventions.
- `docs/localizable-rust-libraries-with-fluent.md` — if any new user-facing
  message needs localizing.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — refactor
  stages.

Skills to load during implementation: `leta` (navigation/refactoring),
`rust-router` then `rust-types-and-apis` (newtype and schema shapes),
`rust-errors` (new error variants), `rust-unit-testing` (rstest/googletest/
insta discipline), `proptest` (precedence invariant), `arch-crate-design`
(boundary checks), `arch-decision-records` (ADR-008), `commit-message`,
`comenq-coderabbit` (review loop), and `rebase` if `main` moves.
