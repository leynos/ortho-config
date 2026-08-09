# Architectural decision record (ADR) 008: Profile selection and layering

## Status

Proposed.

## Date

2026-08-07.

## Context and problem statement

Downstream command-line interfaces (CLIs) built on OrthoConfig — Weaver and
Netsuke first among them — need named, reusable bundles of configuration,
called profiles, so that agents and humans can switch between prepared setups
(for example `--profile weekly-recap`) without re-supplying every flag. Today
OrthoConfig has no profile mechanism at all: the declarative merge engine knows
four layers (defaults, files, environment, CLI), the derive macro generates no
`--profile` flag, and the agent-context schema hard-codes
`"profiles": { "supported": false }`.

[Agent-native CLI assistance design](agent-native-cli-design.md) §6.7, titled
"Persistent profiles", names the contract: profile support should be optional,
must use a canonical root flag when present, and should follow the recommended
precedence
`built-in defaults < config files < selected profile < environment < flags`.
The word "persistent" there describes profiles that live in configuration files
and survive across invocations; it does not require persisted *selection*
state. §6.7 further requires that if implementation work decides profiles are
named config overlays, the exact merge order and migration impact are
documented before code is changed. Roadmap item 9.1.1 tasks OrthoConfig with
designing and implementing that optional profile metadata. This record is the
required documentation; the governing execution plan is
[9.1.1 profile metadata](execplans/9-1-1-profile-metadata.md).

Prior art surveyed before deciding: AWS CLI (named `[profile x]` sections in
shared INI files), kubectl and gcloud (named contexts plus a persisted
"current" selection), dbt (`--profiles-dir` with `--profile` selection), Cargo
(`[profile.<name>]` tables in `Cargo.toml`), Docker buildx, figment (named
profiles with silent fallback on unknown names), config-rs, mise
(per-environment file suffixes), and Spring Boot profiles.

## Decision drivers

- Hold the precedence
  `built-in defaults < config files < selected profile < environment < flags`
  exactly, including the case where a flag value equals the built-in default.
- Keep profile support strictly opt-in: derives that do not opt in must
  compile unchanged, keep their four-layer merge order, gain no new flags, and
  keep emitting `profiles: { "supported": false }` byte-for-byte.
- Keep the agent-context wire schema and documentation IR changes additive
  within schema version `"1"`.
- Keep OrthoConfig generic: no application-specific literals (profile names,
  app names, variable names) may enter the library.
- Keep provenance honest so roadmap 9.1.2 redaction diagnostics can trace a
  merged value back to the file and profile that supplied it.
- Fail loudly and structurally on unknown or invalid selections; figment's
  silent fallback on unknown profiles is a documented footgun.
- Settle the public API shape before implementation so no public surface is
  improvised mid-milestone.

## Requirements

### Functional requirements

- A configuration file may define `[profile.<name>]` tables; JSON5 and YAML
  files use the equivalent key path `profile.<name>`.
- Selecting a profile overlays that profile's values on top of the file
  layer, below environment variables and flags.
- Selection happens through a `--profile <name>` flag with a
  `<PREFIX>PROFILE` environment-variable fallback; the flag wins.
- Unknown, invalid, and reserved profile selections produce structured errors
  that name the selection source and list available profiles.
- Agent context reports whether profiles are supported and how they are
  selected.
- After loading, an application can report which profile is selected and why.

### Technical requirements

- `MergeProvenance` gains a `Profile` variant; the enum is already
  `#[non_exhaustive]`, so the addition is non-breaking.
- No existing public item changes signature, with the single pre-approved
  exception recorded under "Migration plan" (the `AgentContext.profiles`
  retype).
- No new runtime dependencies; new dev dependencies are limited to the
  pre-approved test crates recorded in the execution plan.
- The selector must never leak into the merged configuration value.
- Crate dependency directions stay unchanged: `ortho_config_macros` generates
  code against `ortho_config`; `cargo-orthohelp` depends on `ortho_config`.

## Options considered

### Option A: Named overlay tables inside existing files

Profiles are `[profile.<name>]` tables within the resolved configuration file
chain, mirroring Cargo's `[profile.<name>]`. Selection overlays the chosen
table on the file layer. This reuses existing discovery machinery, needs no new
file formats, and keeps all configuration for a tool in the files users already
manage. It is the accepted option.

### Option B: Profile-per-file suffixes

Each profile lives in its own file, for example `app.staging.toml` (the
mise/Spring pattern). Rejected: it enlarges the discovery surface, introduces a
second naming convention beside in-file keys, and gains little, since a profile
typically overrides only a handful of keys.

### Option C: A separate profile store file

A dedicated store file holds all profiles. Rejected for 9.1.1: storage and any
`save`/`delete` helpers are roadmap 9.1.3's question, and adopting a store now
would pre-empt that decision without a consumer needing it.

| Topic                   | Option A | Option B      | Option C   |
| ----------------------- | -------- | ------------- | ---------- |
| New discovery surface   | none     | yes           | yes        |
| New file formats        | none     | naming scheme | store file |
| Cargo precedent         | direct   | partial       | none       |
| Defers cleanly to 9.1.3 | yes      | no            | no         |

*Table 1: Profile storage options.*

Two subsidiary shape decisions were also weighed. For the layer shape,
pre-merging profile values into the file layer's value was rejected because it
destroys provenance and makes the five-tier precedence unprovable, and a single
pre-merged profile layer was rejected because it loses the per-file trail that
9.1.2 needs; first-class per-file profile layers were chosen. For selection
state, a persisted "current profile" (kubectl/gcloud style) was rejected
because it causes the classic wrong-context incident class and requires a
store, which is 9.1.3's question; stateless selection was chosen.

## Decision outcome / proposed direction

Profiles are named config overlays inside existing files (Option A), selected
statelessly, merged as first-class layers, and surfaced through additive
agent-context and documentation-IR metadata. The subsections below record each
binding decision; together they correspond to decisions D1–D8 and D11–D15 in
the execution plan.

### Five-tier merge order with per-file profile layers

The merge order is exactly:

```text
built-in defaults < config files < selected profile < environment < flags
```

The profile layer is a first-class merge layer. `MergeProvenance` gains a
`Profile` unit variant, `MergeLayer` gains a `profile(value, path)`
constructor, and `MergeComposer` gains a `push_profile` method. Profile tables
are extracted from the file chain per contributing file, producing one profile
layer per file that defines the selected profile. Those layers are pushed in
file-chain order (base first) after all file layers and before the environment
layer. Per-file granularity preserves the provenance trail that 9.1.2's
redaction diagnostics will need. The selected profile's name travels in the
selection result types below, not inside `MergeLayer`, so the layer shape is
unchanged apart from the new provenance.

### Selection is stateless

Selection uses a generated `--profile <name>` clap argument with an
`env = "<PREFIX>PROFILE"` fallback, so the flag beats the environment variable
for selection. The selector environment variable defaults to `<PREFIX>PROFILE`,
derived from the existing `prefix` attribute exactly as other environment keys
are (prefix `APP_` gives `APP_PROFILE`). An empty selector value (for example
`APP_PROFILE=""` from a leaked export) is treated as unset, not as an invalid
name. There is no persisted "current profile" state in 9.1.1.

### Unknown profiles are structured hard errors

Selecting a profile for which no `[profile.<name>]` table exists fails with
`OrthoError::UnknownProfile { selected, source, available }`. The `available`
list is sorted and capped at 16 names, with the error display appending "and N
more" beyond the cap. The error records whether the selection came from the
flag or the environment variable, so a leaked `<PREFIX>PROFILE` is
distinguishable from a typo on the command line. When no configuration file was
discovered at all, the error says so explicitly instead of reporting an empty
available list. File parse errors take precedence over unknown-profile errors
so the root cause is never masked.

### Name grammar and reserved names

Profile names are case-sensitive and validated against the grammar
`[A-Za-z0-9_-]+` (non-empty), following Cargo's validation precedent. The name
`default` is reserved: defining `[profile.default]` is an error, and selecting
`default` is equivalent to selecting no profile. The observable contract is
that the selection accessors report no selection and downstream
`context --json` commands report none; this is documented so agents are not
surprised. The key `inherits` is reserved inside profile bodies so that
Cargo-style single-parent inheritance can be added later without colliding with
a downstream field.

### Opt-in attribute and reserved selector projections

Profile support is opt-in via the struct-level derive attribute
`#[ortho_config(profiles)]`. Only structs carrying the attribute gain the
generated `--profile` flag, the selector environment variable, the profile
merge layer, and `profiles.supported = true` in agent context.

Opting in reserves the `profile` root key across all three projections:

- file tables named `profile` are extracted and never merged as ordinary
  values;
- the selector environment variable is stripped from the environment layer;
- the generated `--profile` flag is excluded from the serialized CLI layer.

A downstream field that claims the `profile` key, the `--profile` flag, or the
`<PREFIX>PROFILE` binding on an opted-in struct is a compile-time error.
Non-opted-in derives reading a shared file that contains `[profile.*]` tables
treat the `profile` key like any other unknown key; that existing behaviour is
pinned by a test.

### Subcommand loading ignores profiles

The subcommand loading path (`load_and_merge_subcommand*`) is a separate
figment pipeline that bypasses `MergeComposer`; profiles do not reach it in
9.1.1. To prevent silently dead configuration, a `[profile.<name>]` table
containing a `cmds` key fails validation with
`OrthoError::ProfileForbiddenKey`. The users' guide states the limitation, and
lifting it is recorded as the expected follow-up once 9.1 stabilizes.

### Interaction with `extends`

Profile tables are collected from the file chain after `extends` resolution:
one profile layer per contributing file that defines the selected profile, in
chain order (base first, extending file last), matching the file layers
themselves. A milestone-1 spike confirmed the premise:
`load_config_file_as_chain` returns an ancestor-first chain in which each
file's values remain distinct, and `ConfigDiscovery::compose_layers` maps each
entry to a separate `MergeLayer::file`. Discovery semantics are otherwise
unchanged: the first successful candidate wins, so profiles never merge across
independently discovered files.

### Relationship with RFC 0002

RFC 0002 (file-layer resolution policy, status Proposed) names the seam this
feature consumes: the ordered post-`extends` file values, which RFC 0002 models
as `FileLayerOutcome`. This decision implements profile extraction against
today's discovery output rather than sequencing behind an unaccepted RFC, but
records a design obligation: if RFC 0002 lands, `FileLayerOutcome` must expose
the ordered file values profile extraction needs, and the extraction helper is
written against a minimal internal interface (ordered `(path, value)` pairs) so
it can be re-seated without semantic change.

### Post-load selection surfacing

Opted-in structs gain generated associated functions
`load_with_profile_from_iter(iter) -> OrthoResult<ProfileLoadOutcome<Self>>`
and a `load_with_profile()` convenience. `ProfileLoadOutcome<T>` has private
fields with accessors `config()`, `into_config()`, and
`selection() -> &[SelectedProfile]` (empty or singleton today; a slice so
multiple simultaneous profiles can arrive additively later).
`SelectedProfile { name, source }` and
`#[non_exhaustive] ProfileSource { Flag, Environment }` are plain runtime types
without serde implementations: downstream `context --json` commands own their
JSON mapping per ADR-003's ownership split, and the users' guide shows the
recommended snake_case rendering. The existing `load`/`load_from_iter`
signatures are untouched.

### Agent-context exposure and the documentation IR

`AgentContext.profiles` is retyped from `SupportDeclaration` to a new
`ProfilesDeclaration`. This is a deliberate, pre-approved breaking change to
the Rust API of a pre-1.0 crate (see "Migration plan"). It is not a wire-schema
break: the unsupported case serializes byte-identically to today's
`{ "supported": false }`, because the new optional fields are omitted when
absent. `ProfilesDeclaration` provides constructors
(`ProfilesDeclaration::unsupported()` and
`ProfilesDeclaration::supported(selection)`) so downstream construction
survives future field additions. The new fields are:

- `selection: Option<ProfileSelectionContract>` — the flag name following the
  `AgentInput::long` convention (no leading `--`) and the environment variable
  name;
- `list_command: Option<Vec<String>>` — a command path matching
  `AgentCommand::path` token-for-token, populated by roadmap 9.1.3 when listing
  helpers exist; carried now so the contract shape is settled.

Selected-profile semantics (which profile is active now and why) are a runtime
concern exposed through `ProfileLoadOutcome`; the static generated context
documents the mechanism, not the moment, following ADR-007's static/runtime
split.

The bridge learns about profile support through the documentation IR:
`DocMetadata` gains an additive, defaulted field
`profiles: Option<DocProfilesMeta>` where `DocProfilesMeta { flag, env_var }`
mirrors `ProfileSelectionContract`. The derive emits it for opted-in structs and
`bridge_ir_to_agent_context` maps it into `ProfilesDeclaration`. Because the
field is additive with `#[serde(default)]`, `ORTHO_DOCS_IR_VERSION` stays
unchanged per the IR compatibility policy.

### Flag-equals-default resolution

The generated loader pushes the CLI layer only when the parsed CLI differs from
defaults (`differs_from_defaults`). Left unaddressed, an explicit flag set to
the built-in default value would be dropped and the profile would win,
violating the five-tier order. For opted-in structs, `push_cli` is therefore
gated on clap's value-source information: an argument counts as provided when
clap reports a command-line or environment origin, not by comparing values.
Legacy derives keep the existing heuristic untouched.

## Goals and non-goals

- Goals:
  - Provide the five-tier merge order with a first-class profile layer.
  - Keep profile support strictly opt-in with a compile-time collision guard.
  - Expose profile support and selection metadata in agent context and the
    documentation IR additively.
  - Provide structured, source-attributed errors for unknown, invalid, and
    reserved selections.
  - Provide a post-load selection surface for downstream `context --json`.
- Non-goals (recorded to prevent drift):
  - Profile inheritance (`inherits =` semantics; the key is reserved only).
  - Multiple simultaneous profiles (the accessor shape allows it later).
  - Secret redaction (roadmap 9.1.2).
  - Any profile store helper (roadmap 9.1.3).
  - Profile-aware subcommand loading (the boundary above defines today's
    behaviour; lifting it is future work).

## Migration plan

Implementation proceeds in six milestones under the execution plan:
documentation first (this record), then the composer layer, extraction and
errors, derive opt-in, agent-context exposure, and finally user-facing
documentation. Two migration concerns deserve explicit notes.

### Rust-level break: `AgentContext.profiles` retype

Retyping `AgentContext.profiles` from `SupportDeclaration` to
`ProfilesDeclaration` breaks any consumer that constructs or matches
`AgentContext` by struct literal. This is accepted as a deliberate pre-1.0
breaking change carried on the next 0.x minor release, recorded in the
changelog and migration notes, with constructors provided so future field
additions do not repeat the break. The wire contract is unaffected; a
byte-identity test proves the unsupported serialization is unchanged.

Profiles are a soft ship-time dependency per
[agent-native-cli-design.md](agent-native-cli-design.md) §2.2: Weaver and
Netsuke may carry temporary `--profile` parsing adapters until 9.1 ships, and
must replace them within their next release once it does; on shape conflict,
the OrthoConfig shape wins. Those adapters should parse only, mirroring the
§2.2 adaptation rule, so the published contract slots in without rework.

### Rollback story

Removing `#[ortho_config(profiles)]` from a struct restores pre-profile
behaviour: the four-layer merge order returns, the `--profile` flag and the
`<PREFIX>PROFILE` binding disappear, and agent context flips back to
`{ "supported": false }`. The cost is that users who scripted against
`--profile` lose the flag; `[profile.*]` tables left in files become inert
unknown keys. Downstream rollback notes should say both things.

## Known risks and limitations

- Profiles apply to the root configuration load only; the subcommand path
  ignores them, and `cmds` inside a profile table is rejected rather than
  silently ignored.
- The `AgentContext.profiles` retype is a Rust-level break for struct-literal
  consumers, mitigated by constructors and the pre-1.0 window.
- One schema field touches many fixture families (wire contract, contract
  support helpers, round-trip strategy, insta snapshots, `cargo-orthohelp`
  goldens, and the `hello_world` example surfaces); the execution plan lands
  them atomically.
- If RFC 0002 lands with a different `FileLayerOutcome` shape, the extraction
  helper must be re-seated; the minimal-interface design bounds that work.

## Consequences

- Downstream CLIs gain a canonical, opt-in profile mechanism with a
  provable five-tier precedence and honest provenance.
- Agent context gains truthful profile metadata: `profiles.supported = true`
  plus selection contract fields for opted-in CLIs, unchanged bytes for
  everyone else.
- Roadmap 9.1.2 (redaction metadata) and 9.1.3 (profile store helpers) build
  on settled type shapes without further breaking changes.

## References

- [Agent-native CLI assistance design](agent-native-cli-design.md) §2.2,
  §6.7, §8.1, and §8.2.
- [Roadmap](roadmap.md) §9.1.
- [ExecPlan: 9.1.1 profile metadata](execplans/9-1-1-profile-metadata.md).
- [RFC 0002: Customizable configuration layering policy](rfcs/0002-config-layer-resolution-policy.md).
- [ADR-003: Define schema ownership for agent-native contracts](adr-003-define-schema-ownership-for-agent-native-contracts.md).
- [ADR-007: Downstream context command naming](adr-007-downstream-context-command-naming.md).
- [Design Document: The `OrthoConfig` Crate](design.md) §4.3 and §4.10.
- [Cargo book: profiles](https://doc.rust-lang.org/cargo/reference/profiles.html).
- [AWS CLI: named profiles](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-files.html).
