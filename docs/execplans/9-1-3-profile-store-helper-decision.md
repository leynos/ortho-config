# 9.1.3 Decide whether OrthoConfig ships a profile store helper

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (expert design review applied; awaiting user approval; no
implementation has started)

Branch: `9-1-3-profile-store-helper-decision`

Roadmap item: `9.1.3` in `docs/roadmap.md` ("Decide whether OrthoConfig ships a
profile store helper").

## Purpose / big picture

OrthoConfig is a Rust configuration library. An application derives
`OrthoConfig` on a struct and gets a loader that merges built-in defaults,
configuration files, environment variables, and command-line flags. Roadmap
item 9.1.1 added *profiles*: named bundles of configuration that a user selects
with `--profile <name>` (or the `<PREFIX>PROFILE` environment variable). Under
[ADR-0012](../adrs/adr-0012-profile-selection-and-layering.md) a profile is a
`[profile.<name>]` table written by hand inside the application's ordinary
configuration file, and the merge order is:

```plaintext
built-in defaults < config files < selected profile < environment < flags
```

Roadmap item 9.1.2 added *redaction metadata*: a field marked
`#[ortho_config(profile_field = "secret")]` or `"reference_only"` never has its
value published in generated documentation or agent context.

Nothing today lets a *program*, rather than a person with a text editor, manage
profiles. Both downstream consumers plan to:

- Weaver's roadmap 19.1.1 and `docs/ui-gap-analysis.md` target
  `weaver profiles save|list|show|delete` and assign "reusable parsing,
  redaction, metadata" to OrthoConfig.
- Netsuke's roadmap 5.3.3 targets `netsuke profile save|list|get|delete`
  (with `--force` on delete) and "depends on OrthoConfig 9.1.3; if unavailable,
  implement only the Netsuke-local adapter".

Without a shared helper each application must invent a storage format,
atomic-write and locking code, a rule for which fields may be saved, redaction
of `show` output, and — critically — a way for `--profile` to select a saved
profile, because the generated loader reads profiles only from configuration
files. The agent-context field `profiles.list_command`, reserved since 9.1.1
"for the 9.1.3 store helpers", is never populated.

This plan's first job is the decision the roadmap asks for. The recommended
decision (D1) is **yes: ship a narrow, opt-in profile store helper**, split so
that `ortho_config` itself never writes files:

- `ortho_config` gains the *read side and the rules*: a versioned,
  persistence-agnostic store document; a derive-generated key schema that knows
  every spelling serde accepts for each field and which fields are protected;
  the checks that refuse `secret` values and unknown keys; the projection that
  withholds protected values from `show`; a catalogue that lists profiles with
  their origins; and loader integration so `--profile` can select a stored
  profile. All of this is usable by an application that keeps its own storage.
- A new workspace crate, `ortho_config_store`, holds the *write side*: a
  JSON file adapter with `list`, `show`, `save`, and `delete`, a bounded
  exclusive lock, and atomic replacement. It returns typed values; it never
  prints, parses arguments, or depends on `clap`.
- The store is a separate, machine-owned file (default
  `$XDG_CONFIG_HOME/<app>/profiles.json`). Hand-authored configuration files
  are never written, so their comments and formatting are never at risk.
- A struct opts in with `#[ortho_config(profiles(store))]`; agent context
  advertises the listing command through
  `#[ortho_config(profiles(list_command = ["profiles", "list"]))]`.

After the change, an application author adds four subcommands in any framework,
each a few lines calling the helper, and an agent can save a profile from the
flags it passes, see it listed with its origin, view it with protected values
withheld, select it with `--profile`, and delete it. Applications that do not
opt in observe no change: no new flags, no new files read, byte-identical agent
context.

The strongest alternative — do not ship; document a recipe — is recorded in D1
and in the proposed ADR-0028. If the approver chooses it, the plan collapses to
`EP-M1` (record the "no" decision) plus the `list_command` attribute from
`EP-M4`.

### Questions the approver must answer

These are recorded as decisions below but change the plan materially, so they
are surfaced here for explicit approval:

1. D1: ship the helper (recommended) or record "do not ship".
2. D11: put the writer in a new published crate `ortho_config_store`
   (recommended) or behind an off-by-default `profile-store` feature of
   `ortho_config` with an ADR-0003 amendment.
3. D5: refuse `secret` values absolutely in version 1 (recommended), or allow
   an explicit opt-out as Netsuke's "avoid storing secrets *by default*"
   wording might suggest.
4. Design §2.2 makes a published OrthoConfig shape binding on Weaver and
   Netsuke once it ships. Approval of this plan is taken as agreement that the
   store format, default location, and conflict rules in ADR-0028 become that
   binding shape; if the downstream owners must be consulted first, say so.

## Constraints

- Implementation begins only after the user explicitly approves this plan,
  including the four questions above.
- Hand-authored configuration files are read-only to OrthoConfig. No helper
  writes, rewrites, or deletes a file found by configuration discovery.
- `ortho_config` gains no filesystem *write* code (ADR-0003 decision driver:
  avoid making `ortho_config` depend on filesystem writing). Reading the store
  file is permitted; `ortho_config` already reads configuration files.
- The five-tier precedence in ADR-0012 holds exactly. Store-defined profiles
  occupy the existing "selected profile" tier.
- Selection stays stateless (ADR-0012): no persisted current profile, no
  `use`/`activate` verb.
- Opt-in is strict. A struct without `profiles(store)` never reads a store,
  even with `--profile` naming a stored profile; a struct without
  `profiles(list_command = ...)` keeps `list_command` absent. Existing golden
  agent-context bytes for structs that do not use the new sub-attributes must
  not change.
- No command-framework coupling: `ortho_config::profile::store` and
  `ortho_config_store` must not name `clap`, `clap_dispatch`, or `ArgMatches`,
  and must not write to stdout or stderr. Command and flag literals stay
  downstream (ADR-0007). (Derive-generated code may use `clap` as it already
  does for the loader.)
- The store never contains any value — including `null` — for any spelling of
  a field classified `secret` (ADR-0026). `show` output never contains a value
  for any spelling of a field classified `secret` or `reference_only`. No error
  `Display` or `Debug`, tracing event, or snapshot contains a profile value.
- No new third-party dependencies, runtime or development. `ortho_config_store`
  depends only on workspace crates and on crates already in the workspace
  (`cap-std`, `camino`, `serde`, `serde_json`, `thiserror`, `tracing`, optional
  `metrics`). Locking uses `std::fs::File::lock`/`try_lock`, stable since Rust
  1.89.0, which equals the workspace MSRV.
- Environment access goes through `ortho_config::EnvSource` (`MapEnv` in
  tests); no new `std::env::var`; no process-wide locks or `serial_test`.
- Filesystem access uses `cap_std::fs_utf8` and `camino` (`AGENTS.md`);
  `cap_std` types do not appear in public signatures.
- No code file exceeds 400 lines. At the base revision
  `ortho_config_macros/src/derive/parse/struct_attrs.rs` is 397 lines and
  `ortho_config/src/env_source.rs` is 398 lines, so new parsing and location
  code goes in new modules.
- Every module begins with `//!`; every public item has a Rustdoc example;
  every `assert!`-family call has a diagnostic message; no `.expect()` outside
  tests; errors are semantic `thiserror` enums with large payloads boxed.
- Public API changes are pre-1.0 and need no shims. Breaking changes are
  limited to those listed under `Interfaces and dependencies` and are recorded
  in `docs/v0-12-0-migration-guide.md`.
- Completed ExecPlans (`docs/execplans/9-1-1-profile-metadata.md`,
  `docs/execplans/9-1-2-profile-redaction-metadata.md`) are not edited.
  ADR-0003, ADR-0012, and ADR-0026 change only through the amendment process
  (banner plus `## Amendments`).
- Spelling is en-GB-oxendict; Markdown wraps at 80 columns; `typos.toml` is
  generated and never hand-edited.

## Tolerances (exception triggers)

Comparison base: `de2e7ae1d0ac42dfe8adf02cd9dad944283970d3` (`main` at planning
time). This ExecPlan file is excluded from every count, including when it is
updated in the same commit.

Classification is by whole file:

- Code: `*.rs` anywhere (production, tests, trybuild fixtures, example),
  `*.feature`, and `Makefile` recipes.
- Configuration: `Cargo.toml`, `Cargo.lock`, `lading.toml`, `dylint.toml`,
  `.config/**`, `.github/**`, `typos.local.toml`.
- Documentation: `*.md` outside `docs/execplans/`, and `*.ftl`.
- Excluded generated files (reviewed, not counted): `typos.toml`, `*.snap`,
  `*.stderr`, and golden JSON under `**/tests/golden/**`.

Counting command (net lines = additions minus deletions):

```sh
git diff --numstat de2e7ae1d0ac42dfe8adf02cd9dad944283970d3 -- . \
  ':(exclude)docs/execplans/**' ':(exclude)typos.toml' \
  ':(exclude)**/*.snap' ':(exclude)**/*.stderr' \
  ':(exclude)**/tests/golden/**'
```

Budgets, each enforced independently:

- Files: more than 80 changed non-ExecPlan files.
- Code: more than 4,800 net lines in total, or more than 1,700 net lines of
  production source (excluding tests and the example).
- Configuration: more than 150 net lines (a new crate manifest, publish order,
  workflow path triggers, `Cargo.lock`).
- Documentation: more than 1,800 net lines.
- Interface: any public item, or any change to an existing public item, not
  listed in `Interfaces and dependencies`.
- Dependencies: any new third-party crate, runtime or development.
- Architecture: any need to write a discovered configuration file, persist
  selection state, change the five-tier order, or add write code to
  `ortho_config`.
- Iterations: a focused test still failing after five fix attempts.
- Time: a milestone exceeding two working days of agent effort.
- Ambiguity: two readings of ADR-0012, ADR-0026, or ADR-0028 leading to
  different observable behaviour.

Breaching any trigger means: stop, record the situation and options in
`Decision log`, set the status to `BLOCKED`, and wait for direction.

Budget evidence. Roadmap 9.1.1 budgeted about 600 net non-test lines per
milestone and 2,500 overall, and finished at 8,695 net lines (3,319 non-test
source, 2,114 tests, 3,245 Markdown, 17 manifests) under an exception granted
on 2026-10-12. That overrun was missed scope: fixture families touched by one
schema field (wire contract, round-trip strategies, snapshots,
`cargo-orthohelp` goldens, `hello_world`) were not counted, and Markdown had no
separate limit. Roadmap 9.1.2 used qualitative triggers only. This plan
therefore counts tests and the example inside the code budget and gives
documentation its own limit. The expert review cut speculative scope (custom
store file names, `from_serialize`, a tuple-returning generated helper, and a
Verus model; see D9 and the revision note), which lowered the estimate.
Estimate: production source about 1,450 lines (store core and schema 450,
catalogue 200, loader and derive integration 450, writer crate 350); tests
about 2,300; example about 400 — about 4,150, with 15% contingency.
Documentation about 1,350 (ADR-0028 400, three ADR amendments 90, users' guide
350, developers' guide 220, design documents 150, changelog and migration guide
120, roadmap and contents 20), with 30% contingency because 9.1.1
underestimated prose.

## Risks

- Risk: a broken store blocks every command, including the ones that would
  repair it. Severity: high. Likelihood: medium. Mitigation: the store is read
  only when a profile is selected, listed, shown, saved, or deleted (D12); the
  write-side verbs never resolve the selection or need a successful
  configuration load; `delete` never consults hand-authored names; malformed
  and oversized store errors say "move `<path>` aside to recover"; a
  behavioural scenario pins this.
- Risk: a secret reaches the store through an alias or an unknown key.
  Severity: high. Likelihood: medium without mitigation. Mitigation: save is
  closed-world over the derive-generated key schema (canonical serde keys
  only); the schema is generated from serde's real key and alias spellings,
  never from documentation key paths (D13); a schema-completeness test covers
  aliases, renames, `rename_all`, and documentation-only overrides.
- Risk: a field is reclassified as `secret` after values were saved.
  Severity: medium. Likelihood: low. Mitigation: loading and showing re-check
  stored bodies against the current schema; a stored value for a now-secret
  field fails with `SecretValue` naming the key and the recovery (delete and
  re-save), and is never served.
- Risk: lost updates under concurrent saves. Severity: high. Likelihood: low.
  Mitigation: read-modify-write under an exclusive lock on a sidecar
  `<store>.lock` that is created once and never deleted (deleting it could let
  two holders lock different inodes); stale locks are impossible because the
  kernel releases advisory locks on process exit; deterministic pause-seam test.
- Risk: network or FUSE filesystems where advisory locks are unsupported or
  thread-local. Severity: medium. Likelihood: low. Mitigation: map lock errors
  other than `WouldBlock` to `LockUnavailable`; document that serialization is
  not guaranteed there.
- Risk: a crash mid-save truncates the store (the AWS CLI defect
  aws/aws-cli#10648). Severity: high. Likelihood: low. Mitigation: temporary
  file in the same directory, `sync_all`, rename, Unix directory sync; orphaned
  temporary files removed under the lock; fault-injection test.
- Risk: Windows sharing violations when antivirus or an indexer holds the
  target open. Severity: medium. Likelihood: medium. Mitigation: bounded retry
  (5 attempts, 20 ms apart, within the lock budget) for
  `ERROR_SHARING_VIOLATION` and `ERROR_ACCESS_DENIED` only; Windows CI leg.
- Risk: a symlinked store (dotfile managers) silently becomes a regular file
  on save. Severity: medium. Likelihood: low. Mitigation: reads follow the link;
  `save` and `delete` refuse a symlink at the store path with `StoreIsSymlink`.
- Risk: the set of hand-authored names depends on the working directory and
  `--config-path`, so a profile saved in one directory can collide with a
  project file elsewhere. Severity: medium. Likelihood: medium. Mitigation: the
  save-time check is documented as advisory; the authoritative check runs only
  when the colliding name is selected or shown; `list` marks the conflict
  instead of failing (D4).
- Risk: CI loses hermeticity because a personal store is read even when an
  explicit configuration file is given. Severity: medium. Likelihood: medium.
  Mitigation: `<PREFIX>PROFILE_STORE` redirects the store path (D14); a path
  that does not exist reads as an empty store.
- Risk: the `profiles(...)` list form collides with the bare `profiles`
  parser. Severity: medium. Likelihood: low. Mitigation: probe P3; trybuild
  fixtures for both forms.
- Risk: local `make test` cannot finish under compile-admission load (as
  recorded for 9.1.2). Severity: medium. Likelihood: medium. Mitigation:
  separate load from defects using the cold-build deadline evidence process in
  `docs/developers-guide.md`; never raise deadlines to hide a defect.
- Risk: the approver prefers "do not ship". Severity: low. Likelihood:
  medium. Mitigation: the collapse path in `Purpose / big picture`.

## Progress

- [x] (2026-11-28 13:40Z) Branch renamed, Lody session titled.
- [x] (2026-11-28 14:10Z) Reconnaissance: code surface, documentation and
  conventions, downstream consumers (Netsuke and Weaver `main` on 2026-11-27),
  and web prior art.
- [x] (2026-11-28 14:40Z) Initial draft written and committed.
- [x] (2026-11-28 16:30Z) Expert design review (structure and viability,
  contracts, failure modes and security, alternatives and verification rigour)
  applied; see `Decision log` D11–D16 and the revision note.
- [ ] Plan approved by the user.
- [ ] `EP-M0` feasibility probes.
- [ ] `EP-M1` decision recorded (ADR-0028 and amendments).
- [ ] `EP-M2` store core, key schema, and projection in `ortho_config`.
- [ ] `EP-M3` `ortho_config_store` writer crate.
- [ ] `EP-M4` loader, catalogue, derive, and agent-context integration.
- [ ] `EP-M5` end-to-end example workflow.
- [ ] `EP-M6` documentation and delivery closure; roadmap 9.1.3 marked done.

## Surprises & discoveries

- Observation: neither downstream consumer uses OrthoConfig profiles yet, but
  both name a dependency on 9.1.3. Evidence: no `#[ortho_config(profiles)]`,
  `--profile`, or `load_with_profile*` in Netsuke `main` `c8c344ca` or Weaver
  `main` `6d701715` (both on `ortho_config` 0.10.0); Netsuke `docs/roadmap.md`
  5.3.3 and Weaver `docs/roadmap.md` 19.1.1. Impact: no deployed store format
  constrains the design; demand is real but planned. The two applications use
  different nouns and verbs (`profile get` versus `profiles show`), confirming
  that command literals must stay downstream.
- Observation: `ProfilesDeclaration::list_command` and `with_available_names`
  are never populated in production. Impact: both are populated here.
- Observation: `AgentCommand::path` includes the root binary token
  (`cargo-orthohelp/src/agent_context/mod.rs:319-322`), so a `list_command`
  must too. Impact: the attribute holds a root-relative path; the bridge
  prefixes the root token and validates that the command exists.
- Observation: the documentation-IR `FileMetadata.key_path` is not the serde
  key when an explicit documentation key overrides it, and it never lists
  `#[serde(alias)]` spellings. Evidence:
  `ortho_config_macros/src/derive/generate/docs/fields/mod.rs:290-306`;
  `tests/fixtures/orthohelp_fixture/src/lib.rs:64`. Impact: protection for
  stored values must be generated from serde keys and aliases (D13), not
  derived from documentation metadata.
- Observation: roadmap 9.1.2 (`cbf58bd4`, 2026-11-25) is not an ancestor of
  `v0.11.0`; the released IR `"2.0"` lacks `FieldMetadata.profile_field`, yet
  `CHANGELOG.md` files 9.1.2 under `[0.11.0]`. Impact: the IR bump to `"2.1"`
  covers both `profile_field` and `list_command`, and the misfiled changelog
  entry is corrected in `EP-M6`.
- Observation: no general-purpose layered-configuration library surveyed
  (figment 0.10.19, config-rs 0.15.27, confy 2.0.0) owns profile
  create/read/update/delete; tools that write back into human-edited files lose
  comments (kubernetes/kubectl#1262) or truncate on interruption
  (aws/aws-cli#10648); Docker contexts show the machine-owned, atomically
  written JSON pattern. Impact: D2, D3.
- Observation: `ProfileName` derives `Eq` and `Hash` but not `Ord`
  (`ortho_config/src/profile/name.rs:17`). Impact: it gains `PartialOrd` and
  `Ord` to key a `BTreeMap`.

## Decision log

- D1. Decision: ship a narrow, opt-in profile store helper, recorded in a new
  ADR-0028. Rationale: the strongest alternative is "do not ship; document a
  recipe", which keeps file I/O out of OrthoConfig and matches the phase-9
  pattern in design §6.4, §6.6, and §6.8 ("OrthoConfig models and lints the
  contract; the application owns the store"). It is rejected for profiles
  because (a) a saved profile is useless unless `--profile` can select it, and
  only the generated loader can add a definition source without every
  application forking the loader; (b) the safety rules — refuse secrets,
  withhold protected values, validate names and keys, keep the five-tier order,
  fail on ambiguous definitions — depend on derive-generated knowledge of serde
  keys that applications cannot easily reproduce, and are the reusable
  contracts design §2.1 assigns to OrthoConfig ("optional primitives for
  profiles, delivery targets, feedback stores"); (c) both consumers' roadmaps
  depend on 9.1.3; (d) `list_command` needs a listing helper. The
  §6.4/§6.6/§6.8 pattern is still honoured: OrthoConfig owns the contract and
  an optional adapter; applications own the commands. Date/Author: 2026-11-28,
  planning agent; pending approval.
- D2. Decision: the store is a separate machine-owned file; hand-authored
  files are never written. Rejected: `toml_edit` write-back into the user's
  TOML file (one format only, a new dependency, no single "which discovered
  file" answer); generic write-back for YAML and JSON5 (no format-preserving
  editor; comment loss is the recurring defect in prior art).
- D3. Decision: one versioned JSON document per store, with a per-profile
  envelope: `{"version": 1, "profiles": {"ci": {"values": {...}}}}`, keys
  written in sorted order. Rationale: one atomic unit and one lock; profile
  names never become file names (case-insensitive filesystems cannot merge
  `Weekly` and `weekly`); the envelope leaves room for per-profile metadata
  (description, timestamps) without a version 2. Integer `version`; unknown
  top-level or envelope keys and higher versions are refused, even by read-only
  operations, so an old binary never silently drops newer data. Rejected: one
  file per profile (Docker/gcloud style).
- D4. Decision: a name defined both by hand and in the store is an error only
  when that name is *selected* (loading) or *shown*; `list` reports both
  origins and marks the entry `conflict`; `save` refuses a name currently
  defined by hand (advisory, because the hand-authored set depends on the
  working directory); `delete` acts on the store only and never consults
  hand-authored names. Rationale: a global disjointness rule let an unrelated
  `git pull` that adds `[profile.ci]` to a project file break every load and
  block the `delete` that would repair it. Rejected: store-wins or file-wins
  shadowing (silent), and per-name merging (two partial definitions of one name
  are confusing).
- D5. Decision: `save` refuses any value, including `null`, for any field
  classified `secret`; it accepts `reference_only` values. `show` withholds
  both, listing each withheld key with its exposure. Rationale: a plaintext
  store must not hold secrets; a `reference_only` value names material resolved
  elsewhere (ADR-0026) and is storable but not echoed. An opt-out is deferred
  until a consumer needs it; it can be added without a format change.
- D6. Decision: no persisted current selection and no `use`/`activate` verb
  (ADR-0012).
- D7. Decision: the read side (document, schema, projection, catalogue,
  loader integration) lives in `ortho_config::profile` with no new feature
  flag; it is compiled wherever profiles are. Rationale: it adds no dependency
  and no write code.
- D8. Decision: the atomic replacement and lock live in `ortho_config_store`
  as crate-private helpers. `cargo-orthohelp`'s `write_atomic_json` is not
  moved in this item; consolidating it (and later 9.2.1 delivery and 9.2.2
  feedback writers) onto `ortho_config_store` is a follow-up recorded in
  ADR-0028.
- D9. Decision: no Verus proof in this item. Rationale: once protection is
  generated from serde keys (D13), every protected spelling is a single
  root-level key; serde keys are never split on `.`, and ADR-0026 rejects
  annotations on flattened containers, whose children appear as root fields.
  The projection is therefore set filtering over root keys and the storability
  check is closed-world set membership. A Verus model of either would restate
  its definition (the 9.1.2 model was warranted by recursive documentation
  trees with prefix interactions; nothing comparable exists here). The
  security-relevant property that is *not* trivial — that the schema contains
  every spelling serde accepts — is a property of proc-macro generation and
  serde behaviour that Verus cannot reach; it is verified by generated-fixture
  tests and a property test (see `Verification plan`). Revisit if protected
  paths ever become nested (for example, profile-aware subcommand bodies). Kani
  is likewise not planned.
- D10. Decision: `extract_profile_layers` takes a context struct instead of a
  growing positional list:
  `extract_profile_layers(layers, ProfileExtraction { selected,
  discovery_failed, store })`.
  Changed in place with every caller (one generated call site and 21 test call
  sites). ADR-0012's consequence that 9.1.3 builds "without further breaking
  changes" is amended by ADR-0028; compatibility is not owed because the
  function is pre-1.0 and has no downstream caller.
- D11. Decision: the writer is a new published workspace crate,
  `ortho_config_store`, depending on `ortho_config`. Rationale: ADR-0003's
  driver is that `ortho_config` must not depend on filesystem writing;
  splitting modules inside one crate does not change what the crate does. A
  separate crate also gives the later 9.2.1, 9.2.2, and 9.3 writers one home.
  Rejected fallback: an off-by-default `profile-store` feature in
  `ortho_config` plus an ADR-0003 amendment (cheaper to release, but weakens
  the boundary).
- D12. Decision: the generated loader reads the store only when a profile is
  selected. `list`, `show`, `save`, and `delete` read it on demand. A store
  read failure with a selection suppresses `UnknownProfile`, as a file parse
  error already does (`extract.rs` discovery-failure semantics). Rationale: a
  store contributes nothing to a load without a selection; reading it there
  only adds a way to fail.
- D13. Decision: the derive generates a `ProfileKeySchema` for structs with
  `profiles(store)`: for each root field, its canonical serde key (after
  `rename` and `rename_all`), every `#[serde(alias)]` spelling, and its
  `profile_field` exposure. Save accepts canonical keys only and refuses
  unknown keys and alias spellings; `show` withholds every spelling of a
  protected field. A `profile_field` annotation on a field reachable only
  through `#[serde(flatten)]` remains rejected (ADR-0026).
- D14. Decision: the store path defaults to
  `<user configuration directory>/<discovery app name>/profiles.json`, resolved
  through discovery's existing XDG/APPDATA resolver (not `env_source.rs`'s home
  fallbacks), and may be redirected by `<PREFIX>PROFILE_STORE` (absolute path;
  empty means unset; relative is an error). Opting in reserves that variable
  exactly as `<PREFIX>PROFILE` is reserved (stripped from the environment
  layer; a field binding it is a compile error).
- D15. Decision: `list_command` is written as an array relative to the root
  command, `profiles(list_command = ["profiles", "list"])`, matching the
  repository's list-attribute convention (`env_vars = [...]`). The bridge
  prefixes the root token and fails generation if the path does not resolve to
  an emitted command. `ProfilesDeclaration.list_command` becomes private with a
  `list_command()` accessor, validated in the builder and on deserialization.
- D16. Decision: the derive generates `profile_body_from_iter`, which builds a
  `ProfileBody` from parsed root flags using the loader's existing value-source
  pruning (only explicitly supplied flags), excluding the `profile` selector
  and the configuration-path flag. Subcommand flags are never part of a body
  (profiles apply to the root load only, ADR-0012).

## Outcomes & retrospective

Not started. To be completed at each milestone boundary and at completion.

## Context and orientation

Inspected base revision: `de2e7ae1d0ac42dfe8adf02cd9dad944283970d3`. Line
numbers refer to that revision; relocate symbols with `git grep -n '<symbol>'`
or the `codegraph_symbol_search` tool.

Workspace crates: `ortho_config` (runtime library), `ortho_config_macros`
(derive macro), `cargo-orthohelp` (documentation and agent-context generator),
`ortho_l10n`, `test_helpers`, `examples/hello_world`, and test fixtures.
`lading.toml` sets the publish order
`ortho_l10n, ortho_config_macros, ortho_config_test_helpers, ortho_config,
cargo-orthohelp`.

Profile runtime (`ortho_config/src/profile/`):

- `name.rs`: `ProfileName` (grammar `[A-Za-z0-9_-]+`, `default` reserved,
  derives `Clone, Debug, Eq, Hash, PartialEq`) and `AvailableProfileNames`
  (sorted, deduplicated, display cap 16; renders "no configuration files were
  found" or "no profiles were found").
- `selection.rs`: `SelectedProfile`, `ProfileSource { Flag, Environment }`.
- `extract.rs`: `extract_profile_layers(layers, selected, discovery_failed)`
  strips the reserved `profile` root key from each file layer, validates names
  and bodies (forbidden keys `cmds`, `inherits`; body must be an object), and
  returns one profile layer per defining file, or `OrthoError::UnknownProfile`.
  Its only production caller is generated code
  (`ortho_config_macros/src/derive/load_impl_profiles.rs:231`).
- `outcome.rs`: `ProfileLoadOutcome<T>`.
- `pub mod profile` at `ortho_config/src/lib.rs:61` (not feature-gated);
  re-exports at `lib.rs:180`.

Merge layers: `ortho_config/src/declarative/layer.rs` defines `MergeLayer` over
`serde_json::Value`, with `MergeLayer::file` and `MergeLayer::profile`.
`sanitize_value` (`ortho_config/src/declarative/merge.rs:99`) strips nulls from
layers.

Discovery: `ortho_config/src/discovery/candidates/mod.rs:85-160` resolves the
user configuration directory (absent, empty, or present `XDG_CONFIG_HOME`;
`APPDATA`/`LOCALAPPDATA` on Windows). Without `discovery(app_name = ...)` the
app name falls back to one derived from `prefix` (to be confirmed by probe P3).
Generated discovery honours the parsed `config_path` flag
(`ortho_config_macros/src/derive/load_impl/discovery.rs:35-46`).

Agent context: `ortho_config/src/agent_context/profiles/mod.rs` defines
`ProfilesDeclaration` (public `supported`, `selection`, `list_command`; private
`redaction`, `available_names`), `ProfileSelectionContract`, and
`ProfileFieldRedaction`. `cargo-orthohelp/src/agent_context/mod.rs:100-133`
maps the documentation IR into it.

Documentation IR:
`DocMetadata.profiles: Option<DocProfilesMeta { flag, env_var }>`
(`ortho_config/src/docs/ir/mod.rs:79-93`; struct literals in seven files);
`FieldMetadata.profile_field` and `FieldMetadata.file`
(`ortho_config/src/docs/ir/field.rs:36-52, 102-105`). `ORTHO_DOCS_IR_VERSION` is
`"2.0"` (`ortho_config/src/docs/mod.rs:24`); `cargo-orthohelp`'s cache key
hashes it.

Derive macro: bare `#[ortho_config(profiles)]` is parsed at
`ortho_config_macros/src/derive/parse/struct_attrs.rs:345-352`. Serde key
resolution is `serde_serialized_field_key` in
`ortho_config_macros/src/derive/parse/serde_attrs.rs:331-350`; aliases are
parsed near line 211. Generated profile loading is in
`ortho_config_macros/src/derive/load_impl_profiles.rs` and
`load_impl_entry.rs:127-178, 276-299`; the `_with_sources` convention is
`(discovery_source: SharedEnvSource, merge_source: SharedScanEnvSource)`. CLI
value-source pruning is in
`ortho_config_macros/src/derive/load_impl/cli.rs:20-60`.

Errors: `ortho_config/src/error/types.rs` (`OrthoError`, `#[non_exhaustive]`,
English `thiserror` messages; profile variants at lines 66-113). Telemetry
categories: `ortho_config/src/merge_telemetry.rs:71, 221-226` and
`ortho_config/src/subcommand/paths_telemetry.rs:273`; the loader's telemetry
contract never records values, keys, paths, or raw errors
(`load_impl_entry.rs:92, 124`).

Existing write and lock patterns (for reference, not reuse across the
dependency direction): `cargo-orthohelp/src/output/mod.rs:126-144, 235-363`
(`write_atomic_json`: unique temporary name with `create_new`, rename, Unix
directory sync) and
`ortho_config_macros/src/derive/generate/localization/artefact/artefact_lock.rs`
(`File::lock`/`try_lock`, poll with deadline, unlock in `Drop`, wait callback
after a threshold).

Tests: profile unit tests in `ortho_config/src/profile/tests_*.rs` (proptests
over names and precedence); BDD in
`ortho_config/tests/features/profiles.feature` with steps under
`ortho_config/tests/rstest_bdd_support/behaviour/steps/profiles_steps*.rs`;
trybuild suites `ortho_config/tests/*_trybuild.rs`; goldens in
`cargo-orthohelp/tests/golden/`; `hello_world` end-to-end tests in
`examples/hello_world/tests/` using `assert_cmd`. `tracing-subscriber` and
`metrics-util` (with `debugging`) are existing development dependencies of
`ortho_config`.

Gates (`Makefile`): `make check-fmt`, `make lint` (rustdoc, Clippy, Whitaker,
docstring coverage), `make test` (does not run doctests), `make markdownlint`
(includes `make spellcheck`), `make nixie`. `make verus` proves the 9.1.2
model; this item adds no proof (D9) and does not touch the prover workflow.

Downstream consumers (read-only, `main` on 2026-11-27): Weaver (clap derive,
`dirs` 6 for runtime paths, no `context --json` yet); Netsuke (clap derive, XDG
configuration through OrthoConfig discovery, `write_atomic` with `cap_std` for
build sidecars, no `context --json` yet).

Prior art (web, November 2026): Docker contexts (machine-owned JSON, atomic
writes); gcloud configurations and Terraform workspaces (guarded deletion);
kubectl (non-atomic writes, comments lost); AWS CLI (in-place rewrite,
truncation bug, no delete verb); `confy` 2.0.0 (non-atomic); figment and
config-rs (read-only); `cap-tempfile` 4.0.3 and `atomic-write-file` 0.3.1
(atomic-write crates, not needed).

Signposts. Documentation: `docs/agent-native-cli-design.md` §2.1, §2.2, §6.4,
§6.6, §6.7, §6.8, §8.1; `docs/design.md` profile sections; ADR-0003, ADR-0007,
ADR-0012, ADR-0026; `docs/users-guide.md` "Switch between named profiles"
(404-518) and "Advertise profile support" (2280-2312);
`docs/developers-guide.md` profile sections (from line 1880), environment
injection (3736-3760), dependency policy (3884-3899);
`docs/documentation-style-guide.md` ADR template (546-679);
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rstest-bdd-users-guide.md`
(the request names `rtest-bdd-users-guide.md`; the file is
`rstest-bdd-users-guide.md`);
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/rust-doctest-dry-guide.md`; `docs/improved-error-message-design.md`
(error shape for enumerating choices);
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/cli-localization-design.md` and
`docs/localizable-rust-libraries-with-fluent.md` (new errors follow the English
`OrthoError` convention unless probe P5 finds profile errors localized);
`docs/cargo-orthohelp-design.md` (bridge and golden conventions);
`docs/clap-dispatch-and-ortho-config-integration.md` (the helper must not assume
`clap-dispatch`); `docs/scripting-standards.md` (no scripts planned). Skills:
`execplans`, `rust-router`, `rust-types-and-apis`, `rust-errors`,
`rust-unit-testing`, `proptest`, `rust-verification`, `arch-crate-design`,
`arch-decision-records`, `hexagonal-architecture`, `domain-cli-and-daemons`,
`nextest`, `codegraph-mcp`, `en-gb-oxendict-style`.

## Conformance basis

Upstream artefacts at revision `de2e7ae1`: `docs/roadmap.md` §9 and 9.1.3;
`docs/agent-native-cli-design.md` §2.1, §2.2, §6.4, §6.6, §6.7, §6.8, §8.1;
ADR-0003, ADR-0007, ADR-0012 (Accepted 2026-08-09), ADR-0026; `AGENTS.md`.
There is no separate Terms of Reference; the roadmap and the agent-native
design serve that role.

Requirements:

- `REQ-decide`: record a decision, evaluated against applications that manage
  their own storage.
- `REQ-verbs`: if shipped, list, show, save, and delete helpers.
- `REQ-framework-neutral`: no forced command framework (roadmap; ADR-0007).
- `REQ-precedence`: the five-tier order holds (ADR-0012).
- `REQ-stateless`: selection stays stateless (ADR-0012).
- `REQ-no-leak`: protected values are never persisted (`secret`) or echoed
  (`secret`, `reference_only`) (ADR-0026).
- `REQ-list-command`: `list_command` is populated when a listing command
  exists (ADR-0012; design §8.1).
- `REQ-opt-in`: non-opted-in applications are unaffected.
- `REQ-own-storage`: applications with their own storage can adopt the rules
  and make their profiles selectable without the file adapter.
- `REQ-no-write-in-core`: `ortho_config` does not write files (ADR-0003).

Gaps: `GAP-no-writer` (no programmatic create or delete); `GAP-no-source`
(loader reads profiles only from files); `GAP-list-command`; `GAP-names`
(`available_names` never populated); `GAP-show-redaction` (no projection for
profile *values*); `GAP-key-truth` (documentation key paths are not serde's
accepted spellings).

Trace:

```plaintext
REQ-decide -> D1 -> EP-M1 -> docs/adrs/adr-0028-profile-store-helper.md
REQ-no-write-in-core -> D11 -> EP-M3 -> ortho_config has no write call (grep check in EP-M6)
REQ-verbs, GAP-no-writer -> D2, D3, D8 -> EP-M3 -> ortho_config_store/tests/json_store.rs
REQ-no-leak, GAP-show-redaction -> D5 -> EP-M2 -> ortho_config/src/profile/store/tests_projection.rs
REQ-no-leak, GAP-key-truth -> D13 -> EP-M4 -> ortho_config/tests/profile_key_schema.rs
REQ-no-leak -> EP-M2..EP-M5 -> canary tests in tests_leaks.rs and the example
REQ-precedence, GAP-no-source -> D4, D10, D12 -> EP-M4 -> profile_store.feature
REQ-precedence -> EP-M4 -> ortho_config/tests/profile_store_precedence.rs
REQ-stateless -> D6 -> EP-M1 -> ADR-0028 non-goal; no selection-writing API
REQ-list-command, GAP-list-command -> D15 -> EP-M4 -> agent_context__profile_store.json.snap
GAP-names -> EP-M5 -> hello_world context --json snapshot lists names
REQ-framework-neutral -> EP-M3, EP-M5 -> no clap reference in store sources; example plain functions
REQ-opt-in -> EP-M4 -> existing goldens unchanged; non-opted-in store-ignored scenario
REQ-own-storage -> D1, EP-M2, EP-M4 -> StoreSnapshot injection test; users' guide section
```

## Verification plan

### Abstract model

Let `N` be the valid profile names. A *body* is a finite map from root keys to
JSON values. A *key schema* `K` is a finite set of fields; each field `f` has a
canonical key `c(f)`, a finite alias set `A(f)`, and an exposure
`e(f) ∈ {public, reference_only, secret}`. Spellings are pairwise distinct
across fields (serde forbids collisions). Write `spell(f) = {c(f)} ∪ A(f)`,
`Secret = ⋃ { spell(f) | e(f) = secret }`,
`Protected = ⋃ { spell(f) | e(f) ≠ public }`, and
`Canonical = { c(f) | f ∈ K }`.

A *store document* is a finite map `D : N ⇀ Body` with format version 1. The
hand-authored catalogue `F` is the set of names defined by `[profile.<name>]`
tables in the discovered file chain (it depends on the working directory and
the configuration-path flag).

Operations:

- `storable(b)` iff `keys(b) ⊆ Canonical` and `keys(b) ∩ Secret = ∅`.
- `project(b)` is the pair `(values, withheld)` where `values` is `b`
  restricted to `keys(b) \ Protected`, and `withheld` is
  `[(k, e) | k ∈ keys(b) ∩ Protected]` sorted by `k`.
- `save(D, F, n, b)`: requires `n ∈ N`, `n ∉ F` (advisory), `storable(b)`,
  no reserved key (`profile`, `cmds`, `inherits`) in `b`, and
  `size(serialize(D[n ↦ b])) ≤ max_bytes`; yields `D[n ↦ b]` and `Created` or
  `Replaced`.
- `delete(D, n)`: requires `n ∈ dom(D)`; yields `D \ {n}`.
- `list(D, F)`: every name in `dom(D) ∪ F` with its origins, `conflict` set
  when it is in both.
- `show(D, F, n)`: fails if `n` is in both; otherwise `project` of the body
  (the store body, or the hand-authored tables merged in chain order); fails
  with `SecretValue` if a store body has a key in `Secret` (protection drift).
- `load` with selection `n`: if `n ∈ F ∩ dom(D)`, `ProfileDefinedTwice`;
  if `n ∈ F`, the per-file profile layers; if `n ∈ dom(D)`, one store layer
  (after re-checking `storable`); else `UnknownProfile` over `dom(D) ∪ F`.
  Without a selection, `D` is not read.

Every failed operation leaves the persisted document byte-identical.

### Invariants and lemmas

- `INV-names-valid`: every name in a persisted or loaded document is in `N`.
- `INV-body-shape`: every stored body is an object without reserved keys.
- `INV-no-secret-at-rest`: every body persisted by `save` is `storable` under
  the schema in force at save time; every store body served by `load` or `show`
  is re-checked against the current schema.
- `INV-schema-complete`: for every field annotated `secret` or
  `reference_only`, every spelling serde accepts for that field when
  deserializing the root struct is in `Secret` or `Protected` respectively.
- `INV-selected-unambiguous`: no successful load or `show` of name `n` occurs
  when `n ∈ F ∩ dom(D)`.
- `INV-atomic`: after any `save` or `delete`, completed or interrupted, the
  store parses as the previous or the new document.
- `INV-serialized`: concurrent `save`/`delete` on one store are linearizable.
- `INV-bounded`: no successful `save` produces a document that `load` rejects
  for size.
- `INV-precedence`: with a store-defined selection, the loaded result equals
  the five-tier merge with the store body as the only profile layer.
- `INV-opt-in`: a struct without `profiles(store)` behaves as before even when
  a store file defining the selected name exists.
- `INV-no-value-in-diagnostics`: no error `Display` or `Debug`, tracing
  event, or snapshot contains a body value.
- `L-project`: `project(b)` contains no key in `Protected`, contains every
  key of `b` outside `Protected` with an identical value, and fabricates
  nothing; `withheld` is exactly `keys(b) ∩ Protected` with exposures.
  (Presence of a protected key is deliberately public: an agent may learn that
  `token` is set, never its value, matching ADR-0026's "name the setting it
  must not echo".)
- `L-storable`: `check_storable(b)` succeeds iff `storable(b)`; on failure it
  reports the lexicographically least offending key, so errors are
  deterministic.
- `L-map-laws`: `show(save(D,n,b), n) = project(b)`; saving a new name adds
  exactly that name to `list`; `delete(save(D,n,b), n)` restores `dom(D)` when
  `n` was new; operations on distinct names commute.
- `L-round-trip`: `from_json_slice(to_json_vec(D)) = D`, and `to_json_vec` is
  canonical (sorted keys, independent of serde_json's `preserve_order`).

### Axioms (trusted, not verified here)

- `AXIOM-rename`: renaming over an existing file in the same directory is
  atomic for readers (POSIX `rename(2)`; Windows `MoveFileExW` with
  `MOVEFILE_REPLACE_EXISTING`, as used by `std` and `cap-std`), except that
  Windows may refuse while another process holds the target open; the retry
  rule bounds that case. Boundary: `ortho_config_store` replace helper.
- `AXIOM-lock`: `std::fs::File::lock`/`try_lock` give an exclusive advisory
  lock between processes, released by the kernel on process exit (`flock`,
  `LockFileEx`). Not guaranteed on some network or FUSE filesystems. Boundary:
  the store lock.
- `AXIOM-fsync`: `sync_all` plus a Unix directory sync make the rename
  durable on mainstream filesystems; durability is best effort and atomicity
  does not depend on it.
- `AXIOM-serde-aliases`: serde's derived `Deserialize` accepts exactly the
  canonical key (after `rename`/`rename_all`) and the declared `alias`
  spellings for a non-flattened field. Boundary: `INV-schema-complete`,
  exercised against real derived structs, not re-implemented.
- `AXIOM-serde-json`: `serde_json` round-trips `Value`, quotes offending
  values in some error messages (hence `INV-no-value-in-diagnostics`), and
  limits recursion depth to 128.

### Methods per obligation

- `INV-names-valid`, `INV-body-shape`, `L-round-trip`. Method: `rstest`
  tables over constructors and the deserializer (valid, invalid grammar,
  reserved `default`, empty, non-object, each reserved key, higher version,
  unknown top-level key, unknown envelope key) plus a `proptest` round trip
  over documents of up to 8 profiles with values of depth 4, with keys
  generated in random insertion order. Artefact:
  `ortho_config/src/profile/store/tests_document.rs`. Evidence:
  `cargo test -p ortho_config profile::store::tests_document`; red is an
  unresolved import. Non-vacuity: generator classification shows valid and
  invalid documents; a seeded fault that skips `ProfileName::new` during
  deserialization must fail `reserved_default_is_rejected`; a seeded fault that
  serializes without sorting must fail the canonical-bytes property. Assertions:
  `pretty_assertions::assert_eq!` for `Value` equality, `googletest`
  `matches_pattern!` for error variants.
- `L-project`, `L-storable`. Method: differential `proptest` against an
  independent oracle plus an exhaustive `rstest` table. The oracle is written
  separately in the test module over plain `BTreeSet<String>` and
  `BTreeMap<String, Value>` (it never calls the production schema). Domain:
  schemas of up to 6 fields with up to 2 aliases each and all three exposures;
  bodies whose keys are drawn from the schema's spellings plus unknown keys,
  including `null` values. Artefact:
  `ortho_config/src/profile/store/tests_projection.rs`. Evidence: 1,024 cases
  pass; the table covers secret present, secret `null`, secret via alias,
  reference-only present, unknown key, alias of a public field, empty body.
  Non-vacuity: generator statistics must show bodies containing secret,
  reference-only, alias, and unknown keys; seeded faults (projection keeps
  alias spellings; `check_storable` ignores `null`; `withheld` omits exposure)
  must each fail.
- `INV-schema-complete`. Method: generated-fixture tests over real derived
  structs: for each fixture (canonical key, `rename`, `rename_all`, `alias`,
  multiple aliases, documentation-only `file(key_path = ...)` override, raw
  identifier), assert that every spelling for which
  `serde_json::from_value::<Fixture>` populates the protected field is in the
  generated schema's protected set, and that no other key is. A `proptest` over
  generated key spellings for one fixture confirms that any spelling outside
  the schema does not populate the field. Artefact:
  `ortho_config/tests/profile_key_schema.rs`. Non-vacuity: a seeded fault that
  derives the schema from documentation `key_path` must fail the override
  fixture; a seeded fault that omits aliases must fail the alias fixture.
- `L-map-laws`, operation sequences. Method: `proptest` state-machine test of
  `ProfileStoreDocument` against a `BTreeMap` model whose refusal rules are
  written independently, then the same sequences replayed through
  `ortho_config_store::JsonProfileStore` on a temporary directory. Domain: up
  to 24 operations over 6 names (one reserved, one invalid) and 4 bodies (one
  with a secret, one with an unknown key). Artefact:
  `ortho_config/src/profile/store/tests_sequences.rs`,
  `ortho_config_store/tests/json_store_sequences.rs`. Non-vacuity: outcome
  counters assert created, replaced, deleted, refused-secret, refused-unknown,
  refused-reserved, and not-in-store all occur; a seeded fault in which
  `delete` reports success without removing must fail.
- `INV-atomic`. Method: fault injection through a crate-private seam in
  `ortho_config_store` invoked between writing the temporary file and renaming
  it. Artefact: `ortho_config_store/src/replace/tests.rs`. Evidence: after the
  injected failure the store is byte-identical to before and no temporary file
  remains. Non-vacuity: a negative control that writes the target in place with
  the same injected failure leaves a truncated file.
- `INV-serialized`. Method: deterministic pause seam: writer A acquires the
  lock and pauses between load and replace; writer B with a 100 ms timeout must
  fail with `LockTimeout`; A resumes; B retries and succeeds; both names are
  present. Artefact: `ortho_config_store/src/lock/tests.rs`. Non-vacuity: with
  a seeded fault that skips locking, B succeeds while A is paused and A's
  resumed write loses B's name, every time.
- `INV-bounded`. Method: an `rstest` case saving a body that would push the
  serialized document over `max_bytes` must fail with `TooLarge` and leave the
  store unchanged; the sequence test also asserts that every successful save
  yields a loadable document.
- `INV-selected-unambiguous`, `INV-precedence`, `INV-opt-in`. Method:
  integration property through a derived fixture using
  `load_with_profile_from_iter_with_sources` with `MapEnv`, temporary
  configuration files, and a store (the existing per-key tier oracle from
  `ortho_config/src/profile/tests_precedence.rs` decides expected values); plus
  `rstest` cases for duplicates and the non-opted-in fixture. Artefact:
  `ortho_config/tests/profile_store_precedence.rs`. Non-vacuity: generated
  cases include store-only, file-only, and duplicate selections; a seeded fault
  that pushes the store layer after the environment layer in generated code
  must fail; a seeded fault that always reads the store must fail the
  non-opted-in case.
- `INV-no-value-in-diagnostics`. Method: canary test: bodies carry a unique
  canary string in secret, reference-only, and public values; every error path
  (malformed JSON quoting the canary, unknown key, secret refusal, duplicate,
  not found, too large) is triggered, and the canary must not appear in any
  error `Display` or `Debug`, in captured `tracing` output
  (`tracing-subscriber` test writer), in `ShownProfile` JSON for protected
  keys, or in any `insta` snapshot file. Artefact:
  `ortho_config/src/profile/store/tests_leaks.rs` and
  `ortho_config_store/tests/leaks.rs`. Non-vacuity: a seeded fault that formats
  the `serde_json::Error` text into `Malformed` must fail.
- Framework neutrality. Method: a unit test reads the store sources with
  `include_str!` and asserts no `clap` reference; the example implements the
  verbs in plain functions. Non-vacuity: the assertion is shown to fail on a
  scratch copy containing `use clap`, then the copy is deleted.
- Diagnostics and formats. Method: `insta` snapshots of every new error's
  `Display` (each variant, with and without names), of the persisted store
  bytes for a fixed document (a format contract), and of trybuild `.stderr`
  files; paired with semantic `googletest` assertions on the variant fields.

### Formal verification decision

No model checking or formal proof is planned (D9). The obligations that look
proof-shaped reduce, after D13, to set operations over root keys (`L-project`,
`L-storable`) or to finite-map laws (`L-map-laws`); proving them in Verus would
restate their definitions, and the developers' guide reserves prover tooling
for "a substantive invariant". The genuinely risky properties are not reachable
by Verus: `INV-schema-complete` depends on proc-macro output and serde's
behaviour, and `INV-atomic`/`INV-serialized` depend on operating-system axioms.
They are verified against the real interfaces as described above. A stronger
method would add little: Kani over `serde_json::Value` trees is impractical and
the bounded cases are covered by `proptest`. The existing 9.1.2 Verus proof is
unaffected; `make verus` is not part of this item's acceptance because no proof
file changes. Revisit if protected paths become nested.

### Suggested BDD features

File: `ortho_config/tests/features/profile_store.feature`, bound by the
existing rstest-bdd harness for `ortho_config/tests/features` (harness name
confirmed by probe P4). Scenarios map to `EP-M4`; the store verbs are driven
through `ortho_config_store` from step definitions.

```gherkin
Feature: Programmatically managed profiles
  Scenario: A saved profile is selectable
    Given an application that opts into the profile store
    And an empty profile store
    When the profile "weekly-recap" is saved with "format" set to "markdown"
    And the application loads with "--profile weekly-recap"
    Then the loaded "format" is "markdown"
    And the selection reports "weekly-recap" from the flag

  Scenario: Saving twice replaces the profile
    Given a profile store defining "weekly-recap" with "format" set to "markdown"
    When the profile "weekly-recap" is saved with "format" set to "json"
    Then saving reports "replaced"
    And the stored "format" for "weekly-recap" is "json"

  Scenario: The environment still beats a stored profile
    Given a profile store defining "weekly-recap" with "format" set to "markdown"
    And the environment sets "FORMAT" to "json"
    When the application loads with "--profile weekly-recap"
    Then the loaded "format" is "json"

  Scenario: Listing shows both origins and marks conflicts
    Given a configuration file defining the profiles "staging" and "ci"
    And a profile store defining the profiles "weekly-recap" and "ci"
    When the profiles are listed
    Then "staging" is listed from the configuration file
    And "weekly-recap" is listed from the store
    And "ci" is listed from both and marked as a conflict

  Scenario: Showing withholds protected values and keeps the rest
    Given a field "token" classified as reference_only
    And a stored profile "ci" with "token" set to "op://vault/ci" and "format" set to "json"
    When the profile "ci" is shown
    Then the shown "format" is "json"
    And "token" is withheld as reference_only
    And the output does not contain "op://vault/ci"

  Scenario: A hand-authored reference-only value is withheld too
    Given a field "token" classified as reference_only
    And a configuration file defining the profile "staging" with "token" set to "op://vault/staging"
    When the profile "staging" is shown
    Then "token" is withheld as reference_only

  Scenario: Saving a secret value is refused
    Given a field "password" classified as secret
    When the profile "ci" is saved with "password" set to "hunter2"
    Then saving fails naming the key "password"
    And the profile store bytes are unchanged

  Scenario: Saving an alias spelling is refused
    Given a field "password" classified as secret with the alias "pwd"
    When the profile "ci" is saved with "pwd" set to "hunter2"
    Then saving fails naming the unknown key "pwd"

  Scenario: A name defined by hand cannot be saved
    Given a configuration file defining the profile "staging"
    When the profile "staging" is saved with "format" set to "json"
    Then saving fails naming the configuration file that defines "staging"

  Scenario: Selecting a doubly defined profile fails loudly
    Given a configuration file defining the profile "ci"
    And a profile store defining the profile "ci"
    When the application loads with "--profile ci"
    Then loading fails naming both the configuration file and the store

  Scenario: An unrelated conflict does not block ordinary loads
    Given a configuration file defining the profile "ci"
    And a profile store defining the profile "ci"
    When the application loads without a profile
    Then loading succeeds

  Scenario: Deleting removes a stored profile
    Given a profile store defining the profile "weekly-recap"
    When the profile "weekly-recap" is deleted
    Then the profiles listed from the store are empty

  Scenario: Deleting an unknown profile lists what is stored
    Given a profile store defining the profile "weekly-recap"
    When the profile "weekly" is deleted
    Then deleting fails listing "weekly-recap"

  Scenario: A corrupt store blocks only store operations
    Given a profile store file containing "not json"
    When the application loads without a profile
    Then loading succeeds
    When the profiles are listed
    Then listing fails naming the store path and how to recover

  Scenario: An application that has not opted in ignores the store
    Given an application that does not opt into the profile store
    And a profile store defining the profile "weekly-recap"
    When the application loads with "--profile weekly-recap"
    Then loading fails because "weekly-recap" is unknown
```

## Plan of work

### `EP-M0` feasibility probes (prototype; scratch only)

Run before `EP-M1`; record transcripts in `Artefacts and notes`; delete all
scratch code.

- P1: `cargo +1.89.0 check` on a scratch crate using `File::try_lock` (CI
  runs only the pinned nightly, so MSRV must be checked explicitly). Whitaker's
  `no_std_fs_operations` exclusions in `dylint.toml` are keyed by crate; decide
  whether `ortho_config_store` needs an entry (with rationale) or can use
  `cap_std` exclusively.
- P2: confirm `cap_std::fs_utf8::Dir::rename` replaces an existing target on
  Windows, how its errors expose `ERROR_SHARING_VIOLATION`, and how to create
  the temporary file and directory with modes `0600`/`0700` on Unix through
  `cap_std` (`cap_fs_ext`, or `set_permissions` after creation). Proxy:
  `cargo clippy --target x86_64-pc-windows-msvc` on the scratch crate.
- P3: confirm the `syn` parser can accept bare `profiles` and
  `profiles(store, list_command = [..])`; confirm the discovery app-name
  fallback when `discovery(app_name = ...)` is absent.
- P4: confirm the rstest-bdd harness binary for `ortho_config/tests/features`.
- P5: confirm whether any profile `OrthoError` message is localized through
  Fluent.
- P6: confirm that serde's derive gives a non-flattened field no spelling
  other than its canonical key and aliases (including
  `rename(deserialize = ...)` and `deny_unknown_fields` interactions), so
  `AXIOM-serde-aliases` is accurate; extend the schema generator to read
  `rename(deserialize = ...)` if needed.

Go/no-go: if P1 or P2 shows atomic replacement or locking needs a new
dependency, stop and escalate with options (`cap-tempfile` 4.0.3,
`atomic-write-file` 0.3.1).

### `EP-M1` decision recorded

Write `docs/adrs/adr-0028-profile-store-helper.md` (status Proposed; Accepted
on approval) with the template in `docs/documentation-style-guide.md`: context
(roadmap 9.1.3, ADR-0012's Option C deferral, Weaver 19.1.1, Netsuke 5.3.3,
prior art), decision drivers, options (no helper; write-back into hand-authored
files; machine-owned single document; one file per profile; store as an extra
discovered configuration file — rejected because ADR-0012 discovery never
merges independently discovered files and per-file profile layers would
silently merge duplicates), the outcome (D1–D16), the abstract model and
invariants, the testing boundary and why no proof (D9), the residual gaps, the
`ortho_config_store` reuse policy, and non-goals (persisted selection, rename,
export/import, `inherits` across sources, profile-aware subcommand loading,
multiple simultaneous profiles). Add an `Amended by ADR-0028` banner and
`## Amendments` section to ADR-0012 (Option C realized as an additional source;
the "no further breaking changes" consequence amended for
`extract_profile_layers`), ADR-0026 (value projection and the storability rule
for profile bodies), and ADR-0003 (the writer crate as a second reference
adapter). Update `docs/agent-native-cli-design.md` §6.7 and §8.1 and
`docs/design.md`; index ADR-0028 in `docs/contents.md`.

Acceptance: `make fmt`, `make check-fmt`, `make markdownlint`, `make nixie`
pass. No code changes.

### `EP-M2` store core, key schema, and projection in `ortho_config`

Red: add `ortho_config/src/profile/store/tests_document.rs`,
`tests_projection.rs`, `tests_sequences.rs`, and `tests_leaks.rs` referencing
the not-yet-written types; `cargo test -p ortho_config profile::store` fails
with unresolved imports.

Green: add under `ortho_config/src/profile/store/`: `mod.rs` (module docs,
re-exports), `body.rs` (`ProfileBody`, handwritten `Debug` showing key count
only), `document.rs` (`ProfileStoreDocument`, `StoreSnapshot`, `SaveOutcome`;
canonical serialization), `schema.rs` (`ProfileKey`, `ProfileKeySchema`,
`check_storable`, `project`, `ShownProfile`, `WithheldValue`), `guard.rs`
(`SaveGuard`, `ConfigDefinedProfiles`), and `error.rs` (`ProfileStoreError`,
`MalformedReason`). Add `ortho_config/src/profile/validate.rs` as the single
owner of profile name and body validation (reserved keys `profile`, `cmds`,
`inherits`; object shape), returning a small `ProfileBodyError` converted at
each boundary, and switch `extract.rs` to it. Derive `PartialOrd` and `Ord` on
`ProfileName`.

Refactor: files under 400 lines; run each seeded fault named in the
`Verification plan` for these obligations, record the failure, revert.

Acceptance: focused tests pass; `make check-fmt`, `make lint`, `make test` pass.

### `EP-M3` `ortho_config_store` writer crate

Red: create the crate skeleton (`ortho_config_store/Cargo.toml`, `src/lib.rs`
with `//!` docs) and tests `tests/json_store.rs`,
`tests/json_store_sequences.rs`, `tests/leaks.rs`, `src/replace/tests.rs`, and
`src/lock/tests.rs` referencing `JsonProfileStore` and `StoreOptions`; they
fail to compile.

Green: `src/json_store.rs` (`JsonProfileStore` with `open_path`, `load`, `list`,
`show`, `save`, `delete`; `save` reloads under the lock, applies
`ProfileStoreDocument::save` with the guard, serializes, checks `max_bytes`,
and replaces; `save` and `delete` refuse a symlink at the store path; reads
follow symlinks and use `take(max_bytes + 1)`); `src/replace/mod.rs` (unique
temporary name `<file>.<pid>.<counter>.tmp` with `create_new`, mode `0600`,
`sync_all`, rename with the bounded Windows retry, Unix directory sync, removal
of the temporary file on failure, removal of orphaned temporary files under the
lock, crate-private fault seam); `src/lock/mod.rs` (sidecar `<file>.lock`
created once and never deleted; `try_lock` polled every 50 ms, clamped to the
remaining deadline; `LockUnavailable` for errors other than `WouldBlock`;
explicit unlock in `Drop`; a `debug` event when the wait exceeds 500 ms;
crate-private pause seam); and `src/error.rs`. Telemetry: `tracing` events with
a closed vocabulary (`operation`, `outcome`, `category`; no paths, names, or
values), and, behind an optional `metrics` feature, a counter
`ortho_config_store.profile.operations` with labels `operation` and `outcome`
plus a lock-wait histogram, verified non-zero through `metrics-util`'s
debugging recorder in a test. Add the crate to the workspace members, to
`lading.toml`'s publish order after `ortho_config`, and to any release workflow
list that enumerates crates.

Acceptance: crate tests pass on Linux;
`cargo clippy -p ortho_config_store --target x86_64-pc-windows-msvc --all-features`
passes; `make check-fmt`, `make lint`, `make test` pass.

### `EP-M4` loader, catalogue, derive, and agent-context integration

Red: add `ortho_config/tests/features/profile_store.feature` and steps under
`ortho_config/tests/rstest_bdd_support/behaviour/steps/profile_store_steps.rs`;
`ortho_config/tests/profile_key_schema.rs`;
`ortho_config/tests/profile_store_precedence.rs`; trybuild fixtures for bare
`profiles`, `profiles(store)`, `profiles(list_command = [..])`, an empty or
non-array `list_command`, an unknown sub-key, a field binding
`<PREFIX>PROFILE_STORE`, and `profile_field` on a flattened field with
`profiles(store)`; a `cargo-orthohelp` golden for a fixture with
`list_command`, and a bridge test for a `list_command` that names no emitted
command. All fail.

Green:

1. Replace `extract_profile_layers`' positional parameters with
   `ProfileExtraction { selected, discovery_failed, store: StoreInput }`
   (`StoreInput::{NotConfigured, Loaded(&StoreSnapshot), Unreadable}`); update
   every caller. With a store-defined selection, emit one `MergeLayer::profile`
   from the store (re-checked with the schema); with a doubly defined selection,
   `OrthoError::ProfileDefinedTwice`; extend `AvailableProfileNames` so a
   store-only setup does not report "no configuration files were found".
2. Add `ortho_config/src/profile/catalogue.rs`: `ProfileCatalogue` built by
   `ProfileCatalogue::from_layers(&[MergeLayer], Option<&StoreSnapshot>)`, with
   `entries()`, `names()`, `config_defined()`, and
   `show(name, &ProfileKeySchema)`; `CatalogueEntry` and `ProfileOrigin` derive
   `Serialize` with the wire shape
   `{"name": "…", "origins": [{"kind": "config_file", "label": "…"}],
   "conflict": false}`
   (`kind` is `config_file` or `store`).
3. Add `ortho_config/src/profile/store/location.rs` with
   `default_profile_store_path(app_name, prefix, env)` reusing discovery's user
   configuration directory resolver through a crate-private function, and the
   `<PREFIX>PROFILE_STORE` override.
4. Parse `profiles(store)` and `profiles(list_command = [..])` in a new
   module `ortho_config_macros/src/derive/parse/profiles_attrs.rs`; reserve
   `<PREFIX>PROFILE_STORE` when `store` is set.
5. Generate, for structs with `profiles(store)`: the schema
   (`profile_key_schema()`), lazy store reading inside the compose body only
   when a selection exists, and the associated functions listed in
   `Interfaces and dependencies` on both the CLI and the configuration struct,
   following the existing `_from_iter` and `_with_sources` conventions; and
   `profile_body_from_iter` (D16).
6. Add `DocProfilesMeta.list_command` (serde default, omitted when absent)
   and `DocProfilesMeta::new`; bump `ORTHO_DOCS_IR_VERSION` to `"2.1"`; emit it
   from the derive; in `cargo-orthohelp`, prefix the root token, validate the
   path against emitted commands, and call
   `ProfilesDeclaration::with_list_command`. Make
   `ProfilesDeclaration.list_command` private with an accessor and validate it
   in `from_wire`.
7. Add `OrthoError::ProfileDefinedTwice` and `OrthoError::ProfileStore`, a
   `profile_store` telemetry category with closed sub-reasons in both telemetry
   matches, and `From<ProfileStoreError> for Arc<OrthoError>`.

Refactor: keep `load_impl_profiles.rs` and parse modules under 400 lines.

Acceptance: 15 BDD scenarios, schema, precedence, and trybuild tests pass;
existing goldens unchanged; new golden reviewed; `make check-fmt`, `make lint`,
`make test` pass.

### `EP-M5` end-to-end example workflow

Red: `examples/hello_world/tests/profile_store_e2e.rs` with `assert_cmd`,
`Command::env_clear`, and `XDG_CONFIG_HOME` under a temporary directory, driving
`profiles save`, `profiles list --json`, `profiles show --json`,
`profiles delete`, `--profile <name>` on an ordinary command, saving a secret
(non-zero exit, stderr names the key, store bytes unchanged), and a corrupt
store; it fails because the commands do not exist.

Green: opt `hello_world`'s root configuration into
`profiles(store, list_command = ["profiles", "list"])`; add a `profiles`
command group whose handlers are plain functions over
`ortho_config_store::JsonProfileStore` and the generated helpers; declare
`behaviour(mutation = ...)` on `save` and `delete` (design §6.4); render JSON
through the example's existing JSON path; populate runtime `context --json`
names with `ProfilesDeclaration::with_available_names`. Add `insta` snapshots
for `profiles list --json`, `profiles show --json`, and the secret-refusal
stderr, with the temporary path normalized.

Acceptance: the end-to-end test passes; snapshots reviewed and free of the
canary; `make check-fmt`, `make lint`, `make test` pass.

### `EP-M6` documentation and delivery closure

Update `docs/users-guide.md` (new section "Manage profiles programmatically"
after "Switch between named profiles": store location and
`<PREFIX>PROFILE_STORE`, the four verbs with a framework-neutral example,
origins and conflicts, secret and reference-only rules, the advisory save-time
check, concurrency, atomicity and symlink rules, recovery from a corrupt store,
and "Use the store rules with your own storage"; update "Advertise profile
support" for `list_command`); `docs/developers-guide.md` (the
`ortho_config_store` crate and its reuse policy, `profile::validate` as the
single owner of validation, the seams and how to run seeded faults, telemetry
vocabulary); `docs/v0-12-0-migration-guide.md` (`extract_profile_layers`, the
`ProfilesDeclaration.list_command` accessor, `DocProfilesMeta::new`, IR `"2.1"`,
`ProfileName: Ord`, new opt-in attributes); `CHANGELOG.md` under Unreleased,
moving the misfiled 9.1.2 entry from `[0.11.0]` to Unreleased;
`docs/roadmap.md` (mark 9.1.3 and both sub-items done, citing ADR-0028). Search
for stale claims with
`git grep -n "9.1.3\|list_command\|store helper" -- docs ':!docs/execplans'`.
Confirm `REQ-no-write-in-core` with
`git grep -nE 'fs::write|File::create|OpenOptions|rename\(' -- ortho_config/src`
showing no new production write. Reconcile `Conformance basis`; set ADR-0028
to Accepted.

Acceptance: `make fmt`, `make check-fmt`, `make lint`, `make test`,
`make markdownlint`, `make nixie` all pass on a frozen `HEAD`.

## Milestones and plateaus

- `EP-M1`: decision recorded; no document claims implementation.
  Requirements: `REQ-decide`. Evidence: ADR-0028 and amendments; Markdown
  gates. Conformance: no code; amendment process followed. Recovery: revert.
  Compatibility: none.
- `EP-M2`: store rules usable by applications with their own storage.
  Requirements: `REQ-no-leak` (partial), `REQ-own-storage` (partial),
  `GAP-show-redaction`. Evidence: focused tests and seeded faults. Conformance:
  no I/O; no new dependency; no write code. Recovery: revert. Remaining:
  writer, loader, schema generation. Compatibility: none.
- `EP-M3`: framework-neutral file verbs. Requirements: `REQ-verbs`,
  `REQ-framework-neutral`, `REQ-no-write-in-core`, `GAP-no-writer`. Evidence:
  crate tests. Conformance: writes only the store, its lock, and temporary
  files. Recovery: revert the crate. Compatibility: none (no deployed store).
- `EP-M4`: stored profiles selectable, listed, shown; listing advertised.
  Requirements: `REQ-precedence`, `REQ-list-command`, `REQ-opt-in`,
  `REQ-no-leak`, `REQ-own-storage`, `GAP-no-source`, `GAP-list-command`,
  `GAP-key-truth`. Evidence: BDD, schema, precedence, goldens. Conformance:
  five-tier order unchanged; non-opted-in bytes unchanged. Recovery: revert
  this milestone's commits; `EP-M3` stays coherent. Compatibility: none
  (pre-1.0; the IR version records the additive field).
- `EP-M5`: observable workflow in a real binary. Requirements: `GAP-names`,
  `REQ-framework-neutral`. Evidence: end-to-end test and snapshots. Recovery:
  revert example changes. Compatibility: none.
- `EP-M6`: documentation current; roadmap done. Evidence: all gates on a
  frozen `HEAD`. Compatibility: none.

## Concrete steps

Working directory: the repository root. Run gates sequentially, each through
`tee` with `pipefail`:

```sh
set -o pipefail
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-ortho-config-$BRANCH.out"
make lint 2>&1 | tee "/tmp/lint-ortho-config-$BRANCH.out"
make test 2>&1 | tee "/tmp/test-ortho-config-$BRANCH.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-ortho-config-$BRANCH.out"
make nixie 2>&1 | tee "/tmp/nixie-ortho-config-$BRANCH.out"
```

Focused commands:

```plaintext
EP-M2 red:   cargo test -p ortho_config profile::store -> error[E0432]: unresolved import
EP-M2 green: cargo test -p ortho_config profile::store -> store core tests pass
EP-M3 red:   cargo test -p ortho_config_store -> unresolved import
EP-M3 green: cargo test -p ortho_config_store -> passes
EP-M3 win:   cargo clippy -p ortho_config_store --target x86_64-pc-windows-msvc --all-features -> no errors
EP-M4 red:   cargo test -p ortho_config --test <bdd harness> profile_store -> scenarios fail
EP-M4 green: same command -> 15 scenarios pass
EP-M5 green: cargo test -p hello_world --test profile_store_e2e -> passes
```

## Validation and acceptance

Behavioural acceptance in the example after `EP-M5` (option names are fixed
against the example's real flags in `EP-M5` and recorded here):

```console
$ XDG_CONFIG_HOME=$PWD/tmp hello-world profiles save loud --is-excited
saved profile "loud" (created)
$ XDG_CONFIG_HOME=$PWD/tmp hello-world profiles list --json
{"profiles":[{"name":"loud","origins":[{"kind":"store","label":".../hello_world/profiles.json"}],"conflict":false}]}
$ XDG_CONFIG_HOME=$PWD/tmp hello-world --profile loud greet
HELLO, WORLD!
$ XDG_CONFIG_HOME=$PWD/tmp hello-world profiles delete loud
deleted profile "loud"
$ XDG_CONFIG_HOME=$PWD/tmp hello-world --profile loud greet
error: unknown profile 'loud' selected via --profile: <message fixed in EP-M4>
```

Quality criteria:

- Tests: `make test` passes, including 15 BDD scenarios and the unit,
  property, schema, precedence, trybuild, golden, leak, and end-to-end tests.
- Verification: every seeded fault in `Verification plan` observed failing
  and reverted, recorded in `Artefacts and notes`.
- Lint and format: `make check-fmt` and `make lint` pass.
- Documentation: `make markdownlint` and `make nixie` pass.
- Security: the canary appears in no diagnostic, trace, snapshot, or `show`
  output for protected keys; no test-written store contains a secret value.

## Idempotence and recovery

Every step is re-runnable. Tests use temporary directories and injected
environments, never the user's configuration directory. Seeded faults live in
scratch commits that are dropped, or in the working tree, reverted with
`git restore` after evidence is copied out. If a gate fails, read its log under
`/tmp` before re-running. Each milestone is one or more commits that can be
reverted in reverse order.

## Artefacts and notes

Reconnaissance observations (already obtained):

```plaintext
$ git merge-base --is-ancestor cbf58bd4 v0.11.0 || echo "9.1.2 NOT in v0.11.0"
9.1.2 NOT in v0.11.0
$ git show v0.11.0:ortho_config/src/docs/ir/field.rs | grep -c profile_field
0
$ git grep -n "list_command" -- '*.rs'
(definition, constructors, wire, serializer, and one omission test only)
```

Probe transcripts, red and green evidence, and seeded-fault results are added
here as milestones run.

## Interfaces and dependencies

New public items in `ortho_config` live under `ortho_config::profile::store` and
`ortho_config::profile` (re-exported from `ortho_config::profile`). Each needs
a Rustdoc example.

```rust
// ortho_config/src/profile/name.rs (changed: derives gain PartialOrd, Ord)
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct ProfileName(/* unchanged */);

// ortho_config/src/profile/store/body.rs
/// A validated profile body: a JSON object without reserved keys.
pub struct ProfileBody(/* serde_json::Map<String, Value> */);
impl ProfileBody {
    pub fn try_from_value(value: serde_json::Value) -> Result<Self, ProfileStoreError>;
    pub fn as_map(&self) -> &serde_json::Map<String, serde_json::Value>;
}

// ortho_config/src/profile/store/document.rs
pub struct ProfileStoreDocument { /* version, BTreeMap<ProfileName, StoredProfile> */ }
impl ProfileStoreDocument {
    pub fn empty() -> Self;
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, ProfileStoreError>;
    pub fn to_json_vec(&self) -> Vec<u8>; // canonical, sorted keys
    pub fn names(&self) -> impl Iterator<Item = &ProfileName>;
    pub fn get(&self, name: &ProfileName) -> Option<&ProfileBody>;
    pub fn save(&mut self, name: ProfileName, body: ProfileBody, guard: &SaveGuard<'_>)
        -> Result<SaveOutcome, ProfileStoreError>;
    pub fn delete(&mut self, name: &ProfileName) -> Result<(), ProfileStoreError>;
}
#[non_exhaustive]
pub enum SaveOutcome { Created, Replaced }
/// A loaded document plus a label naming where it came from.
pub struct StoreSnapshot { /* label: String, document */ }
impl StoreSnapshot {
    pub fn new(label: impl Into<String>, document: ProfileStoreDocument) -> Self;
    pub fn read_file(path: &camino::Utf8Path, max_bytes: u64)
        -> Result<Self, ProfileStoreError>; // read-only; missing file = empty
}

// ortho_config/src/profile/store/schema.rs
/// A canonical or alias root key spelling.
pub struct ProfileKey(/* String */);
/// Generated by the derive for structs with `profiles(store)`.
pub struct ProfileKeySchema { /* fields: canonical, aliases, exposure */ }
impl ProfileKeySchema {
    pub fn builder() -> ProfileKeySchemaBuilder; // used by generated code and tests
    pub fn check_storable(&self, body: &ProfileBody) -> Result<(), ProfileStoreError>;
    pub fn project(&self, body: &ProfileBody) -> ShownProfile;
}
#[derive(serde::Serialize)]
pub struct ShownProfile { /* values: Map, withheld: Vec<WithheldValue> */ }
#[derive(serde::Serialize)]
pub struct WithheldValue { pub key: ProfileKey,
    pub exposure: crate::docs::ProfileFieldExposure }

// ortho_config/src/profile/store/guard.rs
pub struct ConfigDefinedProfiles { /* name -> first defining file */ }
pub struct SaveGuard<'a> { /* private */ }
impl<'a> SaveGuard<'a> {
    pub fn new(schema: &'a ProfileKeySchema, hand_authored: &'a ConfigDefinedProfiles) -> Self;
}

// ortho_config/src/profile/store/location.rs
pub fn default_profile_store_path(app_name: &str, prefix: &str, env: &dyn crate::EnvSource)
    -> crate::OrthoResult<camino::Utf8PathBuf>;

// ortho_config/src/profile/store/error.rs
#[derive(thiserror::Error)] // Debug handwritten; never includes values
#[non_exhaustive]
pub enum ProfileStoreError {
    Malformed { label: String, line: usize, column: usize, reason: MalformedReason },
    UnsupportedVersion { label: String, found: u64, supported: u64 },
    TooLarge { label: String, limit: u64 },
    Io { label: String, operation: StoreOperation, source: std::io::Error },
    NotInStore { name: ProfileName, stored: Vec<ProfileName> /* capped at 16 */ },
    DefinedInConfigFile { name: ProfileName, path: camino::Utf8PathBuf },
    SecretValue { name: ProfileName, key: ProfileKey },
    UnknownKey { name: ProfileName, key: ProfileKey },
    InvalidBody(ProfileBodyError),
}
#[non_exhaustive]
pub enum MalformedReason { Syntax, WrongType, UnknownField, InvalidName, BodyNotObject,
    ReservedKey, Eof }

// ortho_config/src/profile/catalogue.rs
#[derive(serde::Serialize)]
pub struct ProfileCatalogue { /* entries */ }
#[derive(serde::Serialize)]
pub struct CatalogueEntry { pub name: ProfileName, pub origins: Vec<ProfileOrigin>,
    pub conflict: bool }
#[derive(serde::Serialize)]
#[non_exhaustive]
pub enum ProfileOrigin { ConfigFile { label: String }, Store { label: String } }
impl ProfileCatalogue {
    pub fn from_layers(layers: &[MergeLayer<'_>], store: Option<&StoreSnapshot>)
        -> crate::OrthoResult<Self>;
    pub fn entries(&self) -> &[CatalogueEntry];
    pub fn names(&self) -> impl Iterator<Item = &ProfileName>;
    pub fn config_defined(&self) -> ConfigDefinedProfiles;
    pub fn show(&self, name: &ProfileName, schema: &ProfileKeySchema)
        -> crate::OrthoResult<ShownProfile>;
}

// ortho_config/src/profile/extract.rs (changed)
pub struct ProfileExtraction<'a> { pub selected: Option<&'a SelectedProfile>,
    pub discovery_failed: bool, pub store: StoreInput<'a> }
#[non_exhaustive]
pub enum StoreInput<'a> { NotConfigured, Loaded(&'a StoreSnapshot), Unreadable }
pub fn extract_profile_layers(layers: Vec<MergeLayer<'static>>, ctx: ProfileExtraction<'_>)
    -> OrthoResult<ExtractionOutcome>;

// ortho_config/src/agent_context/profiles/mod.rs (changed)
// `list_command` becomes a private field.
impl ProfilesDeclaration {
    pub fn with_list_command(self, path: Vec<String>) -> OrthoResult<Self>;
    pub fn list_command(&self) -> Option<&[String]>;
}

// ortho_config/src/docs/ir/mod.rs (changed)
pub struct DocProfilesMeta { pub flag: String, pub env_var: String,
    pub list_command: Option<Vec<String>> }
impl DocProfilesMeta { pub fn new(flag: impl Into<String>, env_var: impl Into<String>) -> Self; }
// ORTHO_DOCS_IR_VERSION = "2.1"

// ortho_config/src/error/types.rs (new variants)
ProfileDefinedTwice { name: String, config_file: std::path::PathBuf, store: String },
ProfileStore(Box<ProfileStoreError>),
// plus: impl From<ProfileStoreError> for Arc<OrthoError>
```

Generated for structs with `#[ortho_config(profiles(store))]`, on both the CLI
struct and the configuration struct (exact spellings confirmed against
`load_impl_entry.rs` conventions in `EP-M4` and recorded in `Decision log`):

```rust
fn profile_key_schema() -> ProfileKeySchema;
fn profile_body_from_iter<I, T>(iter: I) -> OrthoResult<ProfileBody>;
fn profile_catalogue_from_iter<I, T>(iter: I) -> OrthoResult<ProfileCatalogue>;
fn profile_catalogue_from_iter_with_discovery<I, T>(iter: I, discovery_source: SharedEnvSource)
    -> OrthoResult<ProfileCatalogue>;
fn profile_store_handle_from_iter_with_discovery<I, T>(iter: I, discovery_source: SharedEnvSource)
    -> OrthoResult<ProfileStoreHandle>;
fn load_with_profile_from_iter_with_store<I, T>(iter: I, store: StoreSnapshot)
    -> OrthoResult<ProfileLoadOutcome<Self>>; // injected storage (REQ-own-storage)
```

`ProfileStoreHandle` (in `ortho_config::profile::store`) bundles the resolved
store path, the key schema, and the hand-authored `ConfigDefinedProfiles`; it
performs no I/O itself.

New crate `ortho_config_store` (version 0.11.0 in lockstep; publish order after
`ortho_config`):

```rust
pub struct StoreOptions { pub lock_timeout: std::time::Duration, pub max_bytes: u64 }
// #[non_exhaustive], Default: 5 s, 1 MiB
pub struct JsonProfileStore { /* private */ }
impl JsonProfileStore {
    pub fn open_path(path: &camino::Utf8Path, options: StoreOptions) -> Result<Self, StoreError>;
    pub fn from_handle(handle: &ortho_config::profile::store::ProfileStoreHandle,
        options: StoreOptions) -> Result<Self, StoreError>;
    pub fn load(&self) -> Result<StoreSnapshot, StoreError>;
    pub fn list(&self) -> Result<Vec<ProfileName>, StoreError>;
    pub fn show(&self, name: &ProfileName, schema: &ProfileKeySchema)
        -> Result<ShownProfile, StoreError>;
    pub fn save(&self, name: ProfileName, body: ProfileBody, guard: &SaveGuard<'_>)
        -> Result<SaveOutcome, StoreError>;
    pub fn delete(&self, name: &ProfileName) -> Result<(), StoreError>;
}
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    Rules(Box<ProfileStoreError>),
    LockTimeout { path: camino::Utf8PathBuf, waited: std::time::Duration },
    LockUnavailable { path: camino::Utf8PathBuf, source: std::io::Error },
    StoreIsSymlink { path: camino::Utf8PathBuf },
    Write { path: camino::Utf8PathBuf, source: std::io::Error },
}
```

Reuse policy: `ortho_config::profile::validate` is the single owner of profile
name and body validation; `ProfileKeySchema` is the single owner of value-level
protection for profile bodies, while documentation keeps using
`DocMetadata::redacted_for_output`; `ortho_config_store` is the only crate that
writes profile data, and its replace and lock helpers stay crate-private until
a later decision publishes them for 9.2 and 9.3.

Dependencies: none added. `ortho_config_store` uses `ortho_config` (path,
lockstep version), `cap-std` (`fs_utf8`), `camino`, `serde_json`, `thiserror`,
`tracing`, and optional `metrics`, all already in the workspace; development
dependencies are existing workspace crates (`rstest`, `rstest-bdd`,
`googletest`, `pretty_assertions`, `insta`, `proptest`, `tempfile`,
`tracing-subscriber`, `metrics-util`).

## Revision note

- 2026-11-28: initial draft from reconnaissance (code surface, documentation,
  Netsuke and Weaver `main` on 2026-11-27, web prior art).
- 2026-11-28: expert design review applied (structure and viability;
  contracts; failure modes and security; alternatives and verification rigour).
  Changes: the writer moves to a new crate `ortho_config_store` (D11) because
  module splitting did not honour ADR-0003; disjointness is enforced only for
  selected or shown names, with conflicts marked in `list` (D4); the loader
  reads the store only when a profile is selected (D12); protection is
  generated from serde keys and aliases rather than documentation key paths,
  and save is closed-world over canonical keys (D13); the Verus milestone is
  withdrawn because, after D13, the projection is root-key set filtering and
  the risky properties are outside Verus's reach (D9); `list_command` becomes a
  root-relative array validated by the bridge (D15); a generated body builder
  (D16), a store-path override (D14), an injected-snapshot loader for
  applications with their own storage, a per-profile envelope, canonical bytes,
  a closed `MalformedReason`, handwritten `Debug`, a closed telemetry
  vocabulary, symlink refusal, a write-side size bound, a never-deleted lock
  file, deterministic pause-seam concurrency tests, a canary leak test, and
  seven more BDD scenarios were added; custom store file names,
  `from_serialize`, and the tuple-returning generated function were cut.
  Milestone order and the approval gate are otherwise unchanged; the plan still
  awaits approval.
