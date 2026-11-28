# 9.1.3 Decide whether OrthoConfig ships a profile store helper

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting user approval; no implementation has started)

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

What is missing today is any way for a program, rather than a person with a
text editor, to *manage* profiles. An agent driving a downstream command-line
interface (CLI) such as Weaver wants to run:

```console
weaver --profile weekly-recap render --json
weaver profiles save weekly-recap --since 7d --format markdown
weaver profiles list --json
weaver profiles show weekly-recap --json
weaver profiles delete weekly-recap
```

Weaver's own design names exactly these verbs as its target contract
(`docs/ui-gap-analysis.md` and roadmap 19.1.1 in the Weaver repository), and
says the command semantics are Weaver-owned while "reusable parsing, redaction,
metadata, and ledger vocabulary come from OrthoConfig". Today every such
application would have to invent its own storage format, its own atomic-write
and locking code, its own rule for which fields may be saved, its own redaction
of `show` output, and — critically — its own way of making a saved profile
selectable by `--profile`, because the generated loader only reads
`[profile.<name>]` tables from configuration files. The agent-context field
`profiles.list_command` has been reserved since 9.1.1 "for the 9.1.3 store
helpers" and is never populated.

This plan's first job is the decision the roadmap asks for. The recommended
decision (D1 below) is: **yes, ship a narrow, opt-in profile store helper**,
shaped as follows.

- A *machine-owned* JSON store file (default
  `$XDG_CONFIG_HOME/<app>/profiles.json`) holds programmatically saved
  profiles. OrthoConfig never rewrites hand-authored configuration files, so
  comments and formatting in those files are never at risk.
- A pure, persistence-agnostic core (`ProfileBody`, `ProfileStoreDocument`,
  `ProfileProtection`) carries every rule: name validation, reserved keys,
  refusal to save `secret` values, and redaction of `show` output. Applications
  that keep their own storage can use the core without the file adapter.
- A thin file adapter (`JsonProfileStore`) provides `list`, `show`, `save`, and
  `delete` with a bounded exclusive lock and atomic replacement. It returns
  typed values; it never prints, parses arguments, or depends on `clap`, so any
  command framework can call it.
- When a struct opts in with `#[ortho_config(profiles(store))]`, the generated
  loader reads the store as a second *source* of profile definitions in the
  existing "selected profile" tier. The five-tier order is unchanged. A name
  defined both by hand and in the store is a hard error, never a silent shadow.
- `#[ortho_config(profiles(list_command = "profiles list"))]` finally populates
  `profiles.list_command` in agent context.

After the change, an application author can add four subcommands in whatever
framework they use, each a few lines calling the helper, and an agent can save
a profile, see it listed (with its origin), view it with protected values
withheld, select it with `--profile`, and delete it. Applications that do not
opt in observe no change at all: no new flags, no new files read,
byte-identical agent context.

The alternative decision (do not ship; document a recipe) is recorded with its
trade-offs in `Decision log` D1 and in the proposed ADR-0028, so the approver
can choose it instead. If the approver rejects D1, the plan collapses to
`EP-M1` (record the "no" decision and populate nothing) plus the `list_command`
builder from `EP-M4`.

## Constraints

- Implementation begins only after the user explicitly approves this plan.
  Approval of the plan is approval of decision D1 and of the public API listed
  in `Interfaces and dependencies`; any further public surface needs separate
  approval.
- Hand-authored configuration files are read-only to OrthoConfig. No helper
  writes, rewrites, or deletes a file found by configuration discovery.
- The five-tier precedence in ADR-0012 must hold exactly. Store-defined
  profiles occupy the existing "selected profile" tier; nothing is inserted
  between tiers.
- Selection stays stateless (ADR-0012). No persisted "current profile", no
  `use`/`activate` verb, no `active_profile` file.
- Opt-in is strict. A struct without `profiles(store)` reads no store file,
  and a struct without `profiles(list_command = ...)` keeps `list_command`
  absent. Existing agent-context bytes for non-opted-in structs, and for
  opted-in structs that do not use the new sub-attributes, must not change
  (pinned by the existing golden and byte-identity tests).
- No command-framework coupling: nothing in `ortho_config::profile::store`
  may name `clap`, `clap_dispatch`, `ArgMatches`, or print to stdout/stderr.
  Command and flag literals stay downstream (ADR-0007).
- The store never contains a value for a field classified `secret`
  (ADR-0026). `show` output never contains a value for a field classified
  `secret` or `reference_only`.
- No new runtime dependencies in any published crate. Atomic replacement is
  implemented with `cap-std` (already a dependency) and `std`; locking uses
  `std::fs::File::lock`/`try_lock`, stable since Rust 1.89.0, which equals the
  workspace MSRV (`Cargo.toml` `rust-version = "1.89.0"`). Development
  dependencies are limited to crates already in the workspace (`rstest`,
  `rstest-bdd`, `googletest`, `pretty_assertions`, `insta`, `proptest`,
  `trybuild`, `assert_cmd`, `tempfile`).
- Environment access goes through `ortho_config::EnvSource` (`MapEnv` in
  tests). No new `std::env::var` call outside `ProcessEnv`. No process-wide
  locks or `serial_test` for environment isolation (`AGENTS.md`, "Inject the
  environment instead").
- Filesystem access in production code uses `cap_std::fs_utf8` and `camino`
  (`AGENTS.md`). Any new `std::fs` use must satisfy the Whitaker
  `no_std_fs_operations` lint without a new blanket exclusion in `dylint.toml`.
- No code file may exceed 400 lines.
  `ortho_config_macros/src/derive/parse/struct_attrs.rs` is already 397 lines
  at the base revision, so the `profiles(...)` sub-attribute parser must live
  in a new module.
- Every module begins with a `//!` comment; every public item has a Rustdoc
  example; every `assert!`-family call has a diagnostic message; no `.expect()`
  outside tests; errors are semantic `thiserror` enums, boxed when large.
- Public API changes are pre-1.0 and need no compatibility shims (workspace
  version 0.11.0, released 2026-11-15). The one breaking signature change,
  `ortho_config::profile::extract_profile_layers`, is updated together with its
  only production caller (derive-generated code) and recorded in
  `docs/v0-12-0-migration-guide.md`.
- The documentation IR version moves from `"2.0"` (released in v0.11.0) to
  `"2.1"` for the additive `DocProfilesMeta.list_command` field, following the
  precedent recorded in ADR-0012. `cargo-orthohelp`'s cache key already hashes
  the IR version, so stale caches are invalidated without further work.
- Completed ExecPlans (`docs/execplans/9-1-1-profile-metadata.md`,
  `docs/execplans/9-1-2-profile-redaction-metadata.md`) are historical and must
  not be edited. ADR-0012 and ADR-0026 are amended only through the ADR
  amendment process (banner plus `## Amendments` section), never rewritten.
- Spelling is en-GB-oxendict; Markdown wraps at 80 columns; `typos.toml` is
  generated and never hand-edited.

## Tolerances (exception triggers)

Comparison base: `de2e7ae1d0ac42dfe8adf02cd9dad944283970d3` (`main` at planning
time). This ExecPlan file is excluded from every count below, including when it
is updated in the same commit.

Classification is by whole file:

- Code: `*.rs` anywhere (production, tests, trybuild fixtures, Verus models,
  example application), `*.feature` files, and `Makefile` recipe changes.
- Configuration: `Cargo.toml`, `Cargo.lock`, `dylint.toml`, `.config/**`,
  `.github/**`, `typos.local.toml`.
- Documentation: `*.md` other than files under `docs/execplans/`, plus
  `*.ftl` Fluent catalogues.
- Generated and excluded from counts: `typos.toml`, `*.snap` snapshot files,
  `*.stderr` trybuild expectations, and golden JSON under `tests/golden/` (each
  is still reviewed, but generated by tooling).

Counting command (net lines = additions minus deletions):

```sh
git diff --numstat de2e7ae1d0ac42dfe8adf02cd9dad944283970d3 -- . \
  ':(exclude)docs/execplans/**' ':(exclude)typos.toml' \
  ':(exclude)**/*.snap' ':(exclude)**/*.stderr' \
  ':(exclude)**/tests/golden/**'
```

Budgets, each enforced independently:

- Files: more than 75 changed non-ExecPlan files, stop and escalate.
- Code: more than 5,500 net lines, stop and escalate. Of these, production
  source (excluding tests, Verus, and the example) above 2,000 net lines is a
  separate trigger.
- Configuration: more than 100 net lines, stop and escalate.
- Documentation: more than 1,800 net lines, stop and escalate.
- Interface: any public item not listed in `Interfaces and dependencies`, or
  any signature change to an existing public item other than
  `extract_profile_layers`, stop and escalate.
- Dependencies: any new runtime dependency, or any new development dependency
  not already in the workspace, stop and escalate.
- Architecture: any need to write to a discovered configuration file, to add
  persisted selection state, or to change the five-tier order, stop and
  escalate.
- Iterations: a focused test or proof still failing after five fix attempts,
  stop and escalate.
- Time: a milestone exceeding two working days of agent effort, stop and
  escalate.
- Ambiguity: any point where two readings of ADR-0012, ADR-0026, or ADR-0028
  lead to different observable behaviour, stop and present options.

Budget evidence. Roadmap 9.1.1 set roughly 600 net non-test lines per milestone
and 2,500 net lines overall, and finished at 8,695 net lines (3,319 non-test
source, 2,114 tests, 3,245 Markdown, 17 manifests) under an exception granted
on 2026-10-12. The overrun was missed scope, not approved growth: fixture
families touched by one schema field (wire contract, round-trip strategies,
snapshots, `cargo-orthohelp` goldens, `hello_world`) were not counted, and
Markdown was not budgeted separately. Roadmap 9.1.2 used qualitative triggers
only (stop on "store behaviour, raw-value output, new runtime dependencies").
This plan therefore budgets tests, Verus, and the example inside the code
category explicitly, and gives documentation its own limit sized from 9.1.1's
actual Markdown share scaled down for a smaller schema surface. Estimated code:
production core and adapter about 900 lines, derive and catalogue integration
about 600, errors and IR about 200, tests about 2,300, Verus about 600, example
about 350 (about 4,950 total; 10% contingency). Estimated documentation:
ADR-0028 about 380, users' guide about 350, developers' guide about 220, design
documents about 250, changelog and migration guide about 150, roadmap and
contents about 30 (about 1,380; 30% contingency because 9.1.1 underestimated
prose).

## Risks

- Risk: the store adds a second definition source, and users may be confused
  when a profile they saved is not in their configuration file. Severity:
  medium. Likelihood: medium. Mitigation: `list` and `show` report every
  definition's origin (`config_file` with path, or `store` with path); the
  users' guide states the two-source model; duplicate names are a hard error
  naming both locations.
- Risk: lost updates when two processes save concurrently. Severity: high.
  Likelihood: low. Mitigation: read-modify-write happens entirely under an
  exclusive lock on a sidecar `<store>.lock` file with a bounded wait (default
  5 s) and a typed `LockTimeout` error; covered by a deterministic contention
  test.
- Risk: a crash mid-save truncates the store (the AWS CLI failure in issue
  aws/aws-cli#10648). Severity: high. Likelihood: low. Mitigation: write to a
  uniquely named temporary file in the same directory, `sync_all`, rename over
  the target, then sync the directory on Unix; a seeded-fault test proves an
  interrupted save leaves the previous document intact.
- Risk: case-insensitive filesystems. Severity: low. Likelihood: low.
  Mitigation: a single-document store keys profiles inside JSON, so profile
  names never become file names; `Weekly` and `weekly` remain distinct as
  ADR-0012 requires.
- Risk: Windows rename-over-existing and lock semantics differ from Unix.
  Severity: medium. Likelihood: medium. Mitigation: `EP-M0` probe P2 confirms
  `cap_std::fs_utf8::Dir::rename` replaces an existing file on Windows and that
  `File::try_lock` behaves as documented; the Windows CI leg runs the adapter
  tests; the local proxy is `cargo clippy --target x86_64-pc-windows-msvc`.
- Risk: the `profiles(...)` list form collides with the existing bare
  `profiles` flag parser. Severity: medium. Likelihood: low. Mitigation:
  `EP-M0` probe P3; trybuild fixtures pin both forms and the unknown-key
  diagnostic.
- Risk: the Verus model diverges from the executable projection. Severity:
  medium. Likelihood: medium. Mitigation: a differential property test compares
  the Rust projection against an independent flatten-filter oracle; the
  residual correspondence gap is recorded, as in 9.1.2.
- Risk: local `make test` cannot finish under compile-admission load, as
  recorded for 9.1.2. Severity: medium. Likelihood: medium. Mitigation:
  distinguish load from defects using the cold-build deadline evidence process
  in `docs/developers-guide.md`; do not raise deadlines to hide a defect.
- Risk: the approver prefers the "do not ship" alternative. Severity: low.
  Likelihood: medium. Mitigation: the plan's collapse path is described in
  `Purpose / big picture`; `EP-M1` is useful under either decision.

## Progress

- [x] (2026-11-28 13:40Z) Branch renamed, Lody session titled.
- [x] (2026-11-28 14:10Z) Reconnaissance: code surface, documentation and
  conventions, downstream consumers (Netsuke, Weaver at 2026-11-27 `main`), and
  web prior art.
- [x] (2026-11-28 14:40Z) Initial draft written.
- [ ] Expert design review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M0` feasibility probes.
- [ ] `EP-M1` decision recorded (ADR-0028, design-document amendments).
- [ ] `EP-M2` pure store core with proofs.
- [ ] `EP-M3` JSON file adapter.
- [ ] `EP-M4` loader, catalogue, and agent-context integration.
- [ ] `EP-M5` end-to-end example workflow.
- [ ] `EP-M6` documentation and delivery closure; roadmap 9.1.3 marked done.

## Surprises & discoveries

- Observation: neither downstream consumer uses OrthoConfig profiles yet.
  Evidence: no `#[ortho_config(profiles)]`, `--profile`, or
  `load_with_profile*` in Netsuke `main` `c8c344ca` or Weaver `main`
  `6d701715`; both depend on `ortho_config` 0.10.0. Impact: there is no
  deployed store format or consumer API to stay compatible with; the plan can
  choose the cleanest shape. Demand is roadmap-level (Weaver 19.1.1 and its
  `profiles save/list/show/delete` target; Netsuke 5.3.1 names in
  `context --json`), not code-level.
- Observation: `ProfilesDeclaration::list_command` and
  `ProfilesDeclaration::with_available_names` are never populated in production
  code. Evidence: the only `with_available_names` caller is
  `cargo-orthohelp/tests/rstest_bdd_support/behaviour/profile_redaction_support.rs`;
  `list_command` appears only in its definition and an omission test. Impact:
  populating both is in scope here.
- Observation: no general-purpose layered-configuration library surveyed
  (figment, config-rs, confy) owns profile create/read/update/delete, and tools
  that write back into human-edited files have a poor record (kubectl drops
  comments, issue kubernetes/kubectl#1262; AWS CLI and `gh` rewrite in place
  without atomic replacement). Machine-owned JSON stores with atomic writes
  (Docker contexts) are the well-behaved precedent. Impact: decisions D2 and D3.
- Observation: `ortho_config` has no write, atomic-replace, or locking code;
  `cargo-orthohelp` has `write_atomic_json`
  (`cargo-orthohelp/src/output/mod.rs`) and `ortho_config_macros` has a `std`
  file-lock pattern
  (`ortho_config_macros/src/derive/generate/localization/artefact/artefact_lock.rs`).
  Neither can be called from `ortho_config` because of dependency direction.
  Impact: decision D8.

## Decision log

- D1. Decision: ship a narrow, opt-in profile store helper (recommended), and
  record the decision in a new ADR-0028 that amends ADR-0012's deferral of
  Option C. Rationale: the strongest alternative is "do not ship; document a
  recipe for applications to store profiles themselves". It costs nothing now
  and keeps file I/O out of the library, consistent with ADR-0003's preference
  for writes at the adapter boundary. It is rejected because (a) a saved
  profile is useless unless `--profile` can select it, and only OrthoConfig's
  generated loader can add a definition source without each application forking
  the loader; (b) the rules that make a store safe — refuse `secret` values,
  redact `show`, reject reserved keys and names, keep the five-tier order, fail
  on duplicate definitions — are exactly the reusable contracts
  agent-native-cli-design.md §2.1 assigns to OrthoConfig ("optional primitives
  for profiles, delivery targets, feedback stores"); (c) Weaver names the four
  verbs as a target and assigns their reusable parts to OrthoConfig; (d) the
  reserved `list_command` field cannot be meaningfully populated without a
  listing helper. ADR-0003 is honoured by splitting a pure core from a thin
  adapter. Date/Author: 2026-11-28, planning agent; pending user approval.
- D2. Decision: the store is a separate machine-owned file; hand-authored
  configuration files are never written. Rationale: rejected alternative is
  editing `[profile.<name>]` tables in the discovered file. That needs a
  format-preserving editor per format; one exists for TOML (`toml_edit`) but
  none comparable for YAML or JSON5, it is a new runtime dependency, and the
  "which discovered file?" question has no single answer across discovery
  scopes. Prior art shows comment loss and truncation as the recurring defects
  of that approach.
- D3. Decision: a single versioned JSON document
  (`{"version": 1, "profiles": {...}}`) rather than one file per profile.
  Rationale: one file gives one atomic unit, one lock, and a finite-map model
  that is easy to verify; profile names never become file names, so
  case-insensitive filesystems cannot merge `Weekly` and `weekly`. Rejected
  alternative: Docker/gcloud-style one file per profile, which gives per-name
  write granularity that profile volumes (tens, not thousands) do not need and
  introduces multi-file consistency and name-to-path hazards.
- D4. Decision: store and hand-authored definitions must be disjoint by name.
  `save` refuses a name defined in a configuration file; loading, listing, and
  showing fail with `OrthoError::ProfileDefinedTwice` when both sources define
  a name. Rationale: rejected alternative is "store wins" or "file wins",
  either of which silently shadows a definition the user can see; ADR-0012's
  "fail loudly" driver applies.
- D5. Decision: `save` refuses a body containing any value at a `secret` key
  path; it accepts `reference_only` values. `show` withholds both, reporting
  their key paths in a `withheld` list. Rationale: a plaintext store must not
  persist secrets; a `reference_only` value names material resolved elsewhere
  (ADR-0026) and is therefore storable, but it is still not echoed, matching
  ADR-0026's "withhold value-bearing metadata from generated artefacts".
- D6. Decision: no persisted current selection and no `use`/`activate` verb.
  Rationale: ADR-0012 rejected persisted selection; prior art (kubectl, gcloud)
  shows the wrong-context incident class.
- D7. Decision: no new Cargo feature. The store module compiles wherever
  profiles compile (behind the existing `serde_json` feature); opt-in is per
  derive attribute. Rationale: no new dependencies means a feature buys almost
  nothing and doubles the configuration matrix.
- D8. Decision: implement atomic replacement and locking as crate-private
  helpers in `ortho_config` rather than moving `cargo-orthohelp`'s helper.
  Rationale: dependency direction forbids reuse; consolidating
  `cargo-orthohelp` onto the new helper is a separate refactor recorded as a
  follow-up, not done here.
- D9. Decision: Verus is used for the projection and storability kernel only
  (see `Verification plan`); the finite-map laws, locking, atomicity, and
  loader integration use property, fault-injection, and behavioural tests.
  Rationale: the projection is the security-relevant logic with prefix
  interactions over unbounded trees; map laws proved in Verus would restate
  `vstd` map axioms.
- D10. Decision: the store-source integration changes
  `extract_profile_layers` in place (new `store` parameter) instead of adding a
  parallel function. Rationale: pre-1.0 API whose only production caller is
  generated code updated in the same change; a second function would be
  compatibility theatre.

## Outcomes & retrospective

Not started. To be completed at each milestone boundary and at completion.

## Context and orientation

Inspected base revision: `de2e7ae1d0ac42dfe8adf02cd9dad944283970d3`. Line
numbers below refer to that revision; relocate symbols with
`git grep -n '<symbol>'` or the `codegraph_symbol_search` tool.

Workspace crates: `ortho_config` (runtime library), `ortho_config_macros`
(derive macro), `cargo-orthohelp` (documentation and agent-context generator),
`ortho_l10n`, `test_helpers`, and `examples/hello_world`.

Profile runtime (`ortho_config/src/profile/`, about 1,276 lines including
tests):

- `name.rs`: `ProfileName` (grammar `[A-Za-z0-9_-]+`, `default` reserved) and
  `AvailableProfileNames` (sorted, deduplicated, display cap 16).
- `selection.rs`: `SelectedProfile`, `ProfileSource { Flag, Environment }`.
- `extract.rs`:
  `extract_profile_layers(layers, selected, discovery_failed) -> OrthoResult<ExtractionOutcome>`,
  which strips the reserved `profile` root key from each file layer, validates
  names and bodies (forbidden keys `cmds`, `inherits`), and returns one profile
  layer per file that defines the selected profile, or
  `OrthoError::UnknownProfile`.
- `outcome.rs`: `ProfileLoadOutcome<T>`.
- Re-exports at `ortho_config/src/lib.rs:180`; module `pub mod profile` at
  `lib.rs:61`.

Merge layers: `ortho_config/src/declarative/layer.rs` defines
`MergeLayer { provenance, value: Cow<Value>, path: Option<Utf8PathBuf> }` over
`serde_json::Value`, with constructors `MergeLayer::file` and
`MergeLayer::profile`.

Agent context: `ortho_config/src/agent_context/profiles/mod.rs` defines
`ProfilesDeclaration` (public `supported`, `selection`, `list_command`; private
`redaction` and `available_names` set through validating builders),
`ProfileSelectionContract { flag, env_var }`, and `ProfileFieldRedaction`.
`cargo-orthohelp/src/agent_context/mod.rs:100-133`
(`bridge_ir_to_agent_context`, `profiles_declaration`) maps the documentation
IR into the declaration.

Documentation IR: `ortho_config/src/docs/ir/mod.rs:79-93` defines
`DocMetadata.profiles: Option<DocProfilesMeta { flag, env_var }>`;
`ortho_config/src/docs/ir/field.rs:52` defines
`FieldMetadata.profile_field: Option<ProfileFieldExposure>` alongside
`FieldMetadata.file: Option<FileMetadata>` (the file key).
`ORTHO_DOCS_IR_VERSION` is `"2.0"` in `ortho_config/src/docs/mod.rs:24`.

Derive macro: `#[ortho_config(profiles)]` is parsed at
`ortho_config_macros/src/derive/parse/struct_attrs.rs:345-352` (file is 397
lines). The generated compose body is in
`ortho_config_macros/src/derive/load_impl_profiles.rs` (345 lines);
`build_profile_file_layers` (about line 225) calls `extract_profile_layers`.
Generated entry points (`load_with_profile_from_iter`, `load_with_profile`, and
the `_with_sources` variants) are in
`ortho_config_macros/src/derive/load_impl_entry.rs:127-178, 276-299`. The
`discovery(app_name = ...)` attribute is parsed into
`ortho_config_macros/src/derive/parse/discovery_attrs.rs`.

Environment seam: `ortho_config/src/env_source.rs` defines `EnvSource`,
`ScanEnvSource`, `SharedEnvSource`, `ProcessEnv`, and `MapEnv`; env-derived
home and configuration directories are at lines 169-181.

Existing write and lock patterns: `cargo-orthohelp/src/output/mod.rs:126-144`
(`write_atomic_json`, temp file plus rename plus parent `fsync` via
`cap_std::fs_utf8`), `cargo-orthohelp/src/bridge/ir_cache.rs:246-271`
(`tempfile::NamedTempFile::persist`), and
`ortho_config_macros/src/derive/generate/localization/artefact/artefact_lock.rs`
(`std::fs::File::lock`/`try_lock` with a 50 ms poll and a deadline).

Errors: `ortho_config/src/error/types.rs` (`OrthoError`, `#[non_exhaustive]`,
English `thiserror` messages; profile variants at lines 66-113).

Verus: `verus/profile_redaction*.rs` (770 lines) prove the 9.1.2 redaction
projection over `DocMetadata`; `Makefile:162` sets
`VERUS_PROOF_FILES ?= verus/profile_redaction.rs`; `make verus` (lines 293-308)
runs `rust-prover-tools verus run` without installing anything.
`make install-verus` installs the pinned version from `tools/verus/VERSION`.

Tests for profiles: unit tests in `ortho_config/src/profile/tests_*.rs`
(including proptests over names and precedence); BDD in
`ortho_config/tests/features/profiles.feature` (16 scenarios) with steps under
`ortho_config/tests/rstest_bdd_support/behaviour/steps/profiles_steps*.rs`;
trybuild suites in `ortho_config/tests/*_trybuild.rs` with fixtures under
`ortho_config/tests/trybuild/` and `ortho_config/tests/ui/`; golden agent
context snapshots in `cargo-orthohelp/tests/golden/`; `hello_world` end-to-end
tests in `examples/hello_world/tests/` (`agent_context_e2e.rs` uses
`assert_cmd`).

Gates (`Makefile`): `make check-fmt`, `make lint` (rustdoc, Clippy, Whitaker,
docstring coverage), `make test` (`cargo fetch --locked`, then
`cargo test --all-targets --all-features`, then pytest; doctests are not run by
`make test`), `make markdownlint` (includes `make spellcheck`), `make nixie`,
and `make verus`.

Downstream consumers (read-only evidence, current `main` on 2026-11-27): Weaver
(`ortho_config` 0.10.0; clap derive; no profile code; roadmap 19.1.1 and
`docs/ui-gap-analysis.md` target `weaver profiles save|list|show|delete` and
root `--profile`); Netsuke (`ortho_config` 0.10.0; clap derive; no profile
code; roadmap 5.3.1 wants `--profile` and profile names in `context --json`;
user configuration under XDG via OrthoConfig discovery). Neither has a
`context --json` command yet.

Prior art (web, November 2026): Docker contexts (machine-owned JSON per
context, atomic writes, persisted current context); gcloud configurations (one
file per configuration, persisted active configuration, cannot delete the
active one); kubectl (one YAML file, locked but non-atomic writes, comments
lost); AWS CLI (INI, in-place rewrite, truncation bug aws/aws-cli#10648, no
delete verb); Terraform workspaces and Pulumi stacks (one file or directory
each, guarded deletion); `confy` 2.0.0 (non-atomic whole-struct rewrite);
figment 0.10.19 and config-rs 0.15.27 (read-only).

Signposts. Documentation: `docs/agent-native-cli-design.md` §2.1, §2.2, §6.4,
§6.7, §8.1; `docs/design.md` profile sections; ADR-0003, ADR-0007, ADR-0012,
ADR-0017, ADR-0026; `docs/users-guide.md` "Switch between named profiles"
(lines 404-518) and "Advertise profile support" (2280-2312);
`docs/developers-guide.md` profile sections (from line 1880), prover tooling
(61-135), environment injection (3736-3760), dependency policy (3884-3899);
`docs/documentation-style-guide.md` ADR template (546-679);
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rstest-bdd-users-guide.md`
(the user's list says `rtest-bdd-users-guide.md`; the file is
`rstest-bdd-users-guide.md`);
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/rust-doctest-dry-guide.md`; `docs/improved-error-message-design.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/cli-localization-design.md` and
`docs/localizable-rust-libraries-with-fluent.md` (new errors follow the
existing English `OrthoError` convention; no Fluent catalogue entries are added
unless `EP-M0` probe P5 finds profile errors are localized);
`docs/cargo-orthohelp-design.md`;
`docs/clap-dispatch-and-ortho-config-integration.md` (the helper must not assume
`clap-dispatch`); `docs/scripting-standards.md` (only if a helper script is
added; none is planned). Skills: `execplans`, `rust-router`,
`rust-types-and-apis`, `rust-errors`, `rust-unit-testing`, `proptest`, `verus`,
`rust-verification`, `arch-decision-records`, `hexagonal-architecture`,
`domain-cli-and-daemons`, `codegraph-mcp`, `en-gb-oxendict-style`, `nextest`.

## Conformance basis

Upstream artefacts at revision `de2e7ae1`: `docs/roadmap.md` §9.1.3;
`docs/agent-native-cli-design.md` §2.1, §6.7, §8.1; ADR-0003, ADR-0007,
ADR-0012 (Accepted 2026-08-09), ADR-0026; `AGENTS.md`. There is no separate
Terms of Reference document; the roadmap and the agent-native design document
serve that role.

Requirements:

- `REQ-decide`: record a decision on shipping a store helper, evaluated
  against applications that manage their own storage (roadmap 9.1.3 first
  sub-item).
- `REQ-verbs`: if shipped, provide list, show, save, and delete helpers
  (roadmap 9.1.3 second sub-item).
- `REQ-framework-neutral`: helpers do not force a command framework (roadmap
  9.1.3; ADR-0007).
- `REQ-precedence`: the five-tier order holds (ADR-0012).
- `REQ-stateless`: selection stays stateless (ADR-0012).
- `REQ-no-leak`: protected values are never persisted (`secret`) or echoed
  (`secret`, `reference_only`) (ADR-0026).
- `REQ-list-command`: `profiles.list_command` is populated when a listing
  command exists (ADR-0012; agent-native §8.1).
- `REQ-opt-in`: non-opted-in applications are unaffected (ADR-0012 driver).
- `REQ-own-storage`: applications with their own storage can ignore or
  partially adopt the helper (roadmap 9.1.3 first sub-item).

Gaps:

- `GAP-no-writer`: no programmatic way to create or delete a profile.
- `GAP-no-source`: the loader reads profiles only from configuration files.
- `GAP-list-command`: `list_command` is never populated.
- `GAP-names`: `available_names` is never populated in production.
- `GAP-show-redaction`: no projection exists for profile *bodies* (9.1.2
  projects documentation metadata, not values).

Trace:

```plaintext
REQ-decide -> D1, ADR-0028 -> EP-M1 -> docs/adrs/adr-0028-profile-store-helper.md (Accepted)
REQ-verbs, GAP-no-writer -> D2, D3 -> EP-M3 -> ortho_config/tests/profile_store_adapter.rs
REQ-no-leak, GAP-show-redaction -> D5 -> EP-M2 -> verus/profile_store_projection.rs; profile/store/tests_projection.rs
REQ-precedence, GAP-no-source -> D4, D10 -> EP-M4 -> profile_store.feature "A saved profile is selectable"
REQ-precedence -> EP-M4 -> tests_precedence.rs store-source property
REQ-stateless -> D6 -> EP-M1, EP-M4 -> trybuild/absence check: no selection persistence API
REQ-list-command, GAP-list-command -> EP-M4 -> cargo-orthohelp golden agent_context__profile_store.json.snap
GAP-names -> EP-M4, EP-M5 -> hello_world context --json snapshot lists names
REQ-framework-neutral -> EP-M3, EP-M5 -> store module has no clap import (lint test); example uses plain functions
REQ-opt-in -> EP-M4 -> existing golden and byte-identity tests unchanged
REQ-own-storage -> D1, EP-M2 -> users' guide "Use the store core with your own storage"; core API has no I/O
```

## Verification plan

### Abstract model

Let `N` be the set of valid profile names (grammar `[A-Za-z0-9_-]+`, minus
`default`). A *body* is a finite tree: a table maps string keys to nodes; a
node is a leaf (any JSON scalar or array) or a table. A *key path* is a
non-empty sequence of keys. `lookup(b, p)` follows `p` through tables and
returns the node or nothing.

A *protection* `P` is a finite set of pairs `(p, e)` where `p` is a key path and
`e ∈ {secret, reference_only}`. Write `S(P)` for the secret paths and `W(P)`
for all protected paths.

A *store document* is a finite map `D : N ⇀ Body` with a format version `1`.
The hand-authored catalogue is a finite set `F ⊆ N` of names defined by
`[profile.<name>]` tables in the discovered file chain.

Operations:

- `list(D, F)` returns the sorted union `dom(D) ∪ F` with origins, provided
  `dom(D) ∩ F = ∅`; otherwise `ProfileDefinedTwice`.
- `show(D, F, n)` returns `project(body(n), W(P))` and the withheld paths that
  exist in the body; `body(n)` comes from `D` or from the merged file tables.
- `project(b, Q)` removes every node whose path equals or extends a path in
  `Q`, and leaves every other node unchanged; tables left empty by removal are
  kept (they still name structure, not values).
- `save(D, F, n, b)` requires `n ∈ N`, `n ∉ F`, `b` a table with no reserved
  root key (`profile`, `cmds`, `inherits`), and `storable(b, S(P))`, meaning
  `lookup(b, s)` is nothing for every `s ∈ S(P)`; it returns `D[n ↦ b]` and
  whether `n` was new.
- `delete(D, F, n)` requires `n ∈ dom(D)`; it returns `D \ {n}`. A name in `F`
  only yields `ProfileNotInStore` naming the file; an absent name yields
  `ProfileNotInStore` listing available store names.
- Loading with selection `n`: the profile tier is the per-file profile layers
  if `n ∈ F`, the single store layer if `n ∈ dom(D)`, nothing if no selection,
  else `UnknownProfile` over `dom(D) ∪ F`.

Malformed inputs (invalid JSON, wrong version, unknown top-level keys, invalid
names, non-table bodies, reserved keys, oversized file) are rejected with typed
errors before any transition; a failed operation leaves the persisted document
unchanged.

### Invariants and lemmas

- `INV-names-valid`: every key of a persisted or loaded document is in `N`.
  Establishes `REQ-verbs`.
- `INV-body-shape`: every stored body is a table without reserved root keys.
- `INV-no-secret-at-rest`: for every persisted body `b`, `storable(b, S(P))`
  holds for the protection in force at save time. Establishes `REQ-no-leak`.
- `INV-disjoint`: no successful operation or load observes
  `dom(D) ∩ F ≠ ∅`. Establishes `REQ-precedence`.
- `INV-atomic`: after any save or delete, whether or not it completes, the
  store file parses as either the previous or the new document.
- `INV-serialized`: concurrent read-modify-write operations on one store are
  linearizable (no lost update).
- `INV-precedence`: with a store-defined selection, the merged result equals
  the five-tier merge with the store body as the sole profile layer.
- `INV-opt-in`: without `profiles(store)` and `profiles(list_command)`,
  generated code and agent-context bytes are unchanged.
- `L-project-noninterference`: if `b1` and `b2` agree at every path not
  equal to or extending a path in `Q`, then `project(b1, Q) = project(b2, Q)`.
- `L-project-integrity`: for every path `p` that neither equals nor extends a
  path in `Q`, `lookup(project(b, Q), p) = lookup(b, p)` whenever
  `lookup(b, p)` is a leaf; and `lookup(project(b, Q), q)` is nothing for every
  `q ∈ Q`.
- `L-storable-sound-complete`: `check_storable(b, S)` succeeds if and only if
  `lookup(b, s)` is nothing for every `s ∈ S`; on failure it reports a path in
  `S` that resolves in `b`.
- `L-map-laws`: `show(save(D, n, b), n) = project(b)`; `list` after `save` of a
  new name grows by exactly that name; `delete(save(D, n, b), n)` restores
  `dom(D)` when `n` was new; `save` and `delete` on distinct names commute.

### Axioms (trusted, not verified here)

- `AXIOM-rename`: renaming a file over an existing file in the same directory
  is atomic with respect to readers on supported platforms (POSIX `rename(2)`;
  Windows `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` as used by
  `std::fs::rename` and `cap_std`). Boundary: `JsonProfileStore` replace
  helper. Exercised on Linux and Windows CI.
- `AXIOM-lock`: `std::fs::File::lock` and `try_lock` provide an exclusive
  advisory lock between processes that use them (Rust standard library
  documentation; `flock`/`LockFileEx`). Boundary: the store lock. Processes
  that ignore the lock are outside the guarantee and documented as such.
- `AXIOM-fsync`: `File::sync_all` plus a directory sync on Unix make the
  rename durable across power loss on mainstream filesystems. Durability is
  best effort; atomicity does not depend on it.
- `AXIOM-serde-json`: `serde_json` round-trips `Value` without loss for the
  JSON data model (documented behaviour, arbitrary-precision not required).
- `AXIOM-vstd`: Verus's `vstd` map and sequence axioms are sound.

### Methods per obligation

- Obligation: `INV-names-valid`, `INV-body-shape`. Method: `rstest` tables
  over the deserializer and constructors, plus a round-trip property.
  Rationale: finite partitions (valid, invalid grammar, reserved, empty,
  non-table, each reserved key). Domain: named cases; generated documents of up
  to 8 profiles with bodies of depth 4. Artefact:
  `ortho_config/src/profile/store/tests_document.rs`. Evidence:
  `cargo test -p ortho_config profile::store::tests_document`; red before the
  module exists (unresolved import), green after. Non-vacuity: the generator
  classifies cases as valid/invalid and asserts both classes occur; a seeded
  fault that skips `ProfileName::new` during deserialization must fail the
  `reserved_default_is_rejected` case.
- Obligation: `L-project-noninterference`, `L-project-integrity`,
  `L-storable-sound-complete`. Method: Verus proof over an independent tree
  model, plus a differential property test tying the executable projection to
  an independent oracle. Rationale: these are the security-relevant contracts;
  prefix interactions (`auth` protected while `auth.token` is also protected,
  sibling keys sharing prefixes, empty tables) over unbounded trees are where
  sampled tests miss cases. Domain: all finite trees and finite path sets
  (Verus); trees of depth up to 5, up to 6 keys per table, up to 6 protected
  paths drawn so that at least half resolve in the body (proptest). Artefact:
  `verus/profile_store_projection.rs` (entry),
  `verus/profile_store_projection_model.rs`,
  `verus/profile_store_projection_proofs.rs`;
  `ortho_config/src/profile/store/tests_projection.rs`. Evidence: `make verus`
  reports every obligation verified with no `assume`, `admit`, or
  `external_body` in the new files (checked by
  `git grep -nE 'assume|admit|external_body' verus/profile_store_projection*`
  returning nothing); the proptest passes 1,024 cases. Non-vacuity: Verus
  lemmas are stated with satisfiable antecedents and each has a witness lemma
  constructing a concrete tree where the antecedent holds and a protected path
  resolves; three seeded model faults (projection keeps a node at an exact
  protected path; projection removes an unprotected sibling sharing a key
  prefix; `check_storable` ignores nested paths) must each make `make verus`
  fail, and are reverted. The proptest oracle flattens both bodies to
  `path -> leaf` maps, filters by prefix with a separately written `is_under`
  predicate, and compares; generator statistics must show cases where a
  protected path resolves, where it does not, and where two protected paths are
  nested.
- Obligation: `L-map-laws`, `INV-names-valid` across operation sequences.
  Method: `proptest` state-machine test of `ProfileStoreDocument` operations
  against a `BTreeMap<String, Value>` model, and the same sequence replayed
  through `JsonProfileStore` on a temporary directory. Rationale: operation
  order matters; the model is independent and small. Domain: sequences of up to
  24 operations over a pool of 6 names (including one reserved and one invalid
  name) and 4 bodies (one containing a secret path). Artefact:
  `ortho_config/src/profile/store/tests_sequences.rs`,
  `ortho_config/tests/profile_store_adapter.rs`. Evidence: 256 cases pass.
  Non-vacuity: transition counters assert that created, replaced, deleted,
  refused-secret, refused-reserved, and not-found outcomes all occur; a seeded
  fault in which `delete` returns success without removing must fail.
- Obligation: `INV-atomic`. Method: fault injection through a crate-private
  `ReplaceSeam` (a narrow closure invoked between writing the temporary file
  and renaming it) that returns an error, plus a check that no temporary file
  is left behind. Rationale: crash consistency cannot be sampled; the injection
  point is the only window. Artefact:
  `ortho_config/src/profile/store/replace/tests.rs`. Evidence: the store still
  parses as the previous document and lists the previous names. Non-vacuity: a
  negative control writes the target in place (no temporary file) with the same
  injected failure and must leave a truncated file, so the test can tell the
  difference.
- Obligation: `INV-serialized`. Method: deterministic contention test. One
  handle acquires the store lock through the crate-private lock helper; a second
  `save` with a 100 ms timeout must fail with
  `ProfileStoreError::LockTimeout`; after release, the save succeeds and the
  document contains both writers' names when two saves are interleaved under a
  `std::sync::Barrier`. Artefact:
  `ortho_config/src/profile/store/lock/tests.rs`. Non-vacuity: a seeded fault
  that skips locking must make the interleaved test lose one name within 64
  iterations (recorded once, then reverted).
- Obligation: `INV-disjoint`, `INV-precedence`. Method: extend the existing
  precedence proptest in `ortho_config/src/profile/tests_precedence.rs` with a
  store source, plus `rstest` cases for duplicate definitions, plus BDD.
  Rationale: the precedence oracle already exists and is independent. Artefact:
  `tests_precedence.rs`, `ortho_config/tests/features/profile_store.feature`.
  Non-vacuity: generated cases include store-only, file-only, and duplicate
  names; a seeded fault pushing the store layer after the environment layer
  must fail the property.
- Obligation: `INV-opt-in`. Method: existing golden and byte-identity tests
  (`cargo-orthohelp/tests/golden/*.snap`, the unsupported-declaration
  byte-identity test) must pass unchanged; a new trybuild pass fixture shows
  bare `#[ortho_config(profiles)]` still compiles. Non-vacuity: the new
  `profile_store` golden differs from the existing `profile_fixture` golden
  only in `list_command`.
- Obligation: framework neutrality. Method: a unit test reads the store
  module sources with `include_str!` and asserts none mentions `clap`; the
  example implements the verbs in plain functions called from its existing clap
  commands. Non-vacuity: the assertion is shown to fail against a scratch copy
  containing `use clap` during `EP-M3`, then the scratch copy is deleted.

### Formal verification decision

Verus is warranted for the projection and storability kernel because it decides
what a store may hold and what an agent may see, over unbounded nested tables
with prefix interactions — the same class of contract 9.1.2 proved for
documentation metadata. The proof shows the property from the model's
definitions; it does not assume it. It does not cover the Rust implementation
directly; the differential property test carries that correspondence, and the
residual gap (Rust code versus Verus model) is recorded in ADR-0028 as in
ADR-0026.

Verus is not used for the finite-map laws (they would restate `vstd` map
axioms), file atomicity and locking (they depend on operating-system axioms
that a proof cannot discharge), or loader integration (straightforward plumbing
already covered by the precedence property). Kani adds no distinct assurance
here and is not planned.

### Suggested BDD features

Proposed file: `ortho_config/tests/features/profile_store.feature`, run by
`cargo test -p ortho_config --test rstest_bdd` (the existing harness that binds
`ortho_config/tests/features/*.feature` through `scenarios!`; confirm the
harness name in `EP-M0` probe P4). Scenarios map to `EP-M3` and `EP-M4`.

```gherkin
Feature: Programmatically managed profiles
  Scenario: A saved profile is selectable
    Given an application that opts into the profile store
    And an empty profile store
    When the profile "weekly-recap" is saved with "format" set to "markdown"
    And the application loads with "--profile weekly-recap"
    Then the loaded "format" is "markdown"
    And the selection reports "weekly-recap" from the flag

  Scenario: Listing shows both origins
    Given a configuration file defining the profile "staging"
    And a profile store defining the profile "weekly-recap"
    When the profiles are listed
    Then the list is "staging" from the configuration file and "weekly-recap" from the store

  Scenario: Showing withholds protected values
    Given a field "token" classified as reference_only
    And a stored profile "ci" with "token" set to "op://vault/ci"
    When the profile "ci" is shown
    Then the shown body has no "token" value
    And "token" is listed as withheld

  Scenario: Saving a secret value is refused
    Given a field "password" classified as secret
    When the profile "ci" is saved with "password" set to "hunter2"
    Then saving fails naming the key path "password"
    And the profile store is unchanged

  Scenario: A name defined by hand cannot be saved
    Given a configuration file defining the profile "staging"
    When the profile "staging" is saved with "format" set to "json"
    Then saving fails naming the configuration file that defines "staging"

  Scenario: Duplicate definitions fail loudly
    Given a configuration file defining the profile "staging"
    And a profile store defining the profile "staging"
    When the application loads with "--profile staging"
    Then loading fails naming both the configuration file and the store

  Scenario: Deleting an unknown profile lists what exists
    Given a profile store defining the profile "weekly-recap"
    When the profile "weekly" is deleted
    Then deleting fails listing "weekly-recap"

  Scenario: A corrupt store is reported, not ignored
    Given a profile store file containing "not json"
    When the profiles are listed
    Then listing fails naming the store path
```

## Plan of work

### `EP-M0` feasibility probes (prototype; scratch only)

Run before `EP-M1`; record transcripts in `Artefacts and notes`; delete all
scratch code.

- P1: confirm `std::fs::File::try_lock` compiles at the workspace MSRV and
  that the `artefact_lock.rs` polling pattern needs no `std::fs` exclusion
  beyond those already in `dylint.toml`; if a lint exclusion is needed, record
  its rationale before `EP-M3`.
- P2: confirm `cap_std::fs_utf8::Dir::rename` replaces an existing target on
  Windows (read the `cap-std` source for the Windows path and run
  `cargo clippy --target x86_64-pc-windows-msvc -p ortho_config` on the scratch
  helper) and how to create the temporary file with owner-only permissions on
  Unix through `cap_std` (`cap_fs_ext` or `set_permissions` after creation).
- P3: confirm the `syn` attribute parser accepts both bare `profiles` and
  `profiles(store, list_command = "...")` without ambiguity, and choose the new
  module path for the sub-attribute parser.
- P4: confirm the BDD harness binary name for `ortho_config/tests/features`.
- P5: confirm whether any profile `OrthoError` message is localized through
  Fluent; if so, add catalogue entries for new variants in `EP-M4`.

Go/no-go: if P1 or P2 shows atomic replace or locking is impossible without a
new dependency, stop and escalate with the options (`cap-tempfile` 4.0.3 or
`atomic-write-file` 0.3.1).

### `EP-M1` decision recorded

Write `docs/adrs/adr-0028-profile-store-helper.md` (status Proposed, becoming
Accepted on plan approval) using the template in
`docs/documentation-style-guide.md`: context (roadmap 9.1.3, ADR-0012 Option C
deferral, downstream demand, prior art), decision drivers, options (no helper;
edit hand-authored files; machine-owned single JSON document; one file per
profile), decision outcome D1–D7, the abstract model and invariants from this
plan, the Verus/testing boundary, consequences, and non-goals (persisted
selection, export/import, rename, inheritance, profiles for subcommand
loading). Add an `Amended by ADR-0028 (date)` banner and an `## Amendments`
section to ADR-0012 (Option C is now realized as an additional source, not a
replacement) and to ADR-0026 (bodies gain a value projection and a storability
rule). Update `docs/agent-native-cli-design.md` §6.7 and §8.1 to describe the
helper and point at ADR-0028; update `docs/design.md`'s profile section; index
ADR-0028 in `docs/contents.md`.

Acceptance: `make fmt`, `make check-fmt`, `make markdownlint`, `make nixie`
pass. No code changes.

### `EP-M2` pure store core with proofs

Red: add `ortho_config/src/profile/store/tests_document.rs`,
`tests_projection.rs`, and `tests_sequences.rs` referencing the not-yet-written
types; `cargo test -p ortho_config profile::store` fails to compile with
unresolved imports. Add the Verus model files with the lemma statements and
proof bodies; before the model functions are complete, `make verus` fails.

Green: add `ortho_config/src/profile/store/mod.rs` (module docs, re-exports),
`body.rs` (`ProfileBody`), `document.rs` (`ProfileStoreDocument`, serde with
version check and `deny_unknown_fields`), `protection.rs` (`ProfileProtection`,
`ProfileProtection::from_doc_metadata`, `project`, `check_storable`),
`outcome.rs` (`SaveOutcome`, `ShownProfile`), and `error.rs`
(`ProfileStoreError`). Reuse the forbidden-key validation from
`ortho_config/src/profile/extract.rs` by extracting it into a shared
crate-private function in `ortho_config/src/profile/validate.rs` (record in
`docs/developers-guide.md` as the single owner of body validation). Add
`verus/profile_store_projection*.rs` and extend `VERUS_PROOF_FILES` in the
`Makefile`.

Refactor: keep each file under 400 lines; run the seeded Verus faults and the
seeded test faults listed in `Verification plan`, record their failures, and
revert them.

Acceptance: focused tests pass; `make verus` passes with both proof files;
`make check-fmt`, `make lint`, `make test` pass.

### `EP-M3` JSON file adapter

Red: `ortho_config/tests/profile_store_adapter.rs` and the lock and replace
unit tests reference `JsonProfileStore`, `StoreOptions`, and
`ProfileStoreLocation`; they fail to compile.

Green: add `ortho_config/src/profile/store/file.rs` (`JsonProfileStore`),
`replace/mod.rs` (crate-private atomic replace with `ReplaceSeam`),
`lock/mod.rs` (crate-private bounded lock on `<file>.lock`), and `location.rs`
(`ProfileStoreLocation::user_default(app_name, &dyn EnvSource)` resolving
`$XDG_CONFIG_HOME/<app>/profiles.json`, falling back to
`$HOME/.config/<app>/profiles.json` on Unix and the platform configuration
directory elsewhere, reusing the existing helpers in
`ortho_config/src/env_source.rs`). Reads are bounded by
`StoreOptions::max_bytes` (default 1 MiB); a missing file reads as an empty
document; a missing parent directory is created on first save. Emit
`tracing::debug!` events for each operation with the store path, operation, and
outcome (profile names are public; bodies are never logged).

Acceptance: adapter, lock, replace, and sequence tests pass on Linux; the
Windows target check passes; `make check-fmt`, `make lint`, `make test` pass.

### `EP-M4` loader, catalogue, and agent-context integration

Red: add `ortho_config/tests/features/profile_store.feature` and step
definitions under
`ortho_config/tests/rstest_bdd_support/behaviour/steps/profile_store_steps.rs`;
add trybuild fixtures for `profiles(store)`, `profiles(list_command = "...")`,
an empty `list_command`, an unknown sub-key, and `profiles(store)` without
`discovery(app_name = ...)`; add a `cargo-orthohelp` golden for a fixture with
`list_command`. All fail.

Green:

1. Change `extract_profile_layers` to take `store: Option<&StoreSnapshot>`
   (a loaded document plus its path), merge the store's names into the
   `available` list, fail with `OrthoError::ProfileDefinedTwice` on overlap,
   and emit one `MergeLayer::profile` with the store path when the selection is
   store-defined. A store read error counts as discovery failure for the
   purpose of suppressing `UnknownProfile`.
2. Add `ProfileCatalogue` (`ortho_config/src/profile/catalogue.rs`): entries
   of `(ProfileName, Vec<ProfileOrigin>)`, built from file layers and an
   optional store snapshot without merging, with `names()`, `get(name)`, and
   `show(name, &ProfileProtection) -> OrthoResult<ShownProfile>` (merging a
   file-defined profile's per-file tables in chain order).
3. Parse `profiles(store)`, `profiles(store = "file-name.json")`, and
   `profiles(list_command = "space separated tokens")` in a new module
   `ortho_config_macros/src/derive/parse/profiles_attrs.rs`; the bare form is
   unchanged. `store` requires `discovery(app_name = ...)`.
4. Generate, for structs with `profiles(store)`, store loading inside the
   compose body (through `ProfileStoreLocation` and the injected `EnvSource`),
   plus `profile_catalogue()` and `profile_catalogue_with_sources(env)`
   associated functions and a `profile_store_with_sources(env)` constructor
   returning the `JsonProfileStore` and the `ProfileProtection` derived from
   `get_doc_metadata()`.
5. Add `DocProfilesMeta.list_command: Option<Vec<String>>` with
   `#[serde(default, skip_serializing_if = "Option::is_none")]`, bump
   `ORTHO_DOCS_IR_VERSION` to `"2.1"`, emit it from the derive, map it in
   `cargo-orthohelp/src/agent_context/mod.rs::profiles_declaration`, and add
   `ProfilesDeclaration::with_list_command(path) -> OrthoResult<Self>` (rejects
   empty paths and empty tokens, and unsupported declarations).
6. Add `OrthoError::ProfileDefinedTwice { name, config_file, store }` and
   `OrthoError::ProfileStore(Box<ProfileStoreError>)`, with telemetry
   classification beside the existing profile variants.

Refactor: keep `load_impl_profiles.rs` and the parse modules under 400 lines.

Acceptance: BDD scenarios pass; extended precedence property passes; all
existing goldens and byte-identity tests pass unchanged; new golden reviewed;
`make check-fmt`, `make lint`, `make test`, `make verus` pass.

### `EP-M5` end-to-end example workflow

Red: add `examples/hello_world/tests/profile_store_e2e.rs` using `assert_cmd`
with `Command::env_clear` and an `XDG_CONFIG_HOME` under a temporary directory,
driving `hello-world profiles save`, `list --json`, `show --json`, `delete`, and
`--profile <name> greet`; it fails because the commands do not exist.

Green: opt `hello_world`'s root configuration into
`profiles(store, list_command = "profiles list")`, add a `profiles` subcommand
group whose handlers are plain functions over the helper (no helper type knows
about the example's clap types), render JSON output with the example's existing
JSON path, and populate runtime `context --json` names via
`ProfilesDeclaration::with_available_names`. Add `insta` snapshots for
`profiles list --json` and `profiles show --json` (with the temporary path
normalized).

Acceptance: the end-to-end test passes; snapshots reviewed; `make check-fmt`,
`make lint`, `make test` pass.

### `EP-M6` documentation and delivery closure

Update `docs/users-guide.md` (new section "Manage profiles programmatically"
after "Switch between named profiles", covering the store location, the four
helpers, origins, the secret and reference-only rules, duplicate-name errors,
concurrency and atomicity guarantees, and "Use the store core with your own
storage"; update "Advertise profile support" for `list_command`);
`docs/developers-guide.md` (store module layout, the single owner of body
validation, the crate-private replace and lock helpers and their reuse policy,
the Verus model and how to run its seeded faults, nextest grouping if any);
`docs/v0-12-0-migration-guide.md` (`extract_profile_layers` signature, IR
`"2.1"`, new opt-in attributes); `CHANGELOG.md` under Unreleased; and
`docs/roadmap.md` (mark 9.1.3 and both sub-items done, citing ADR-0028). Search
for stale claims with
`git grep -n "9.1.3\|list_command\|store helper" -- docs ':!docs/execplans'`
and update every live document that still describes the helper as future work.
Reconcile `Conformance basis`; set ADR-0028 to Accepted if not already.

Acceptance: `make fmt`, `make check-fmt`, `make lint`, `make test`,
`make markdownlint`, `make nixie`, `make verus` all pass on a frozen `HEAD`.

## Milestones and plateaus

- `EP-M1`: decision recorded; repository documents describe the approved
  design and nothing claims it is implemented. Requirements: `REQ-decide`.
  Evidence: ADR-0028 committed; Markdown gates pass. Conformance: no code
  changed; ADR amendments follow the style guide. Recovery: revert the commit.
  Remaining: everything else. Compatibility: none.
- `EP-M2`: pure core usable by applications with their own storage.
  Requirements: `REQ-no-leak`, `REQ-own-storage`, `GAP-show-redaction`.
  Evidence: proofs and focused tests. Conformance: no I/O in the core; no new
  dependency. Recovery: revert; nothing else depends on it yet. Remaining: file
  adapter, loader. Compatibility: none.
- `EP-M3`: file-backed verbs usable from any framework. Requirements:
  `REQ-verbs`, `REQ-framework-neutral`, `GAP-no-writer`. Evidence: adapter,
  lock, replace tests. Conformance: writes only the store and its lock and
  temporary files; never a discovered file. Recovery: revert. Remaining:
  selection of saved profiles. Compatibility: none (no deployed store exists).
- `EP-M4`: saved profiles selectable; listing advertised. Requirements:
  `REQ-precedence`, `REQ-list-command`, `REQ-opt-in`, `GAP-no-source`,
  `GAP-list-command`. Evidence: BDD, precedence property, goldens. Conformance:
  five-tier order unchanged; non-opted-in bytes unchanged. Recovery: revert the
  milestone's commits; `EP-M3` remains coherent. Compatibility: none (pre-1.0;
  the IR version records the additive field).
- `EP-M5`: observable workflow in a real binary. Requirements: `GAP-names`,
  `REQ-framework-neutral`. Evidence: end-to-end test and snapshots. Recovery:
  revert example changes only. Compatibility: none.
- `EP-M6`: documentation current; roadmap done. Evidence: all gates on frozen
  `HEAD`. Compatibility: none.

## Concrete steps

Working directory: the repository root. Run gates sequentially, never in
parallel, each through `tee` with `pipefail`:

```sh
set -o pipefail
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-$BRANCH.out
make lint 2>&1 | tee /tmp/lint-ortho-config-$BRANCH.out
make test 2>&1 | tee /tmp/test-ortho-config-$BRANCH.out
make verus 2>&1 | tee /tmp/verus-ortho-config-$BRANCH.out
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-$BRANCH.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-$BRANCH.out
```

Focused commands per milestone:

```plaintext
EP-M2 red:   cargo test -p ortho_config profile::store  -> error[E0432]: unresolved import
EP-M2 green: cargo test -p ortho_config profile::store  -> all store core tests pass
EP-M2 proof: make verus                                  -> every obligation verified, 0 errors
EP-M3 red:   cargo test -p ortho_config --test profile_store_adapter -> unresolved import
EP-M3 green: cargo test -p ortho_config --test profile_store_adapter -> passes
EP-M3 win:   cargo clippy -p ortho_config --target x86_64-pc-windows-msvc --all-features -> no errors
EP-M4 red:   cargo test -p ortho_config --test rstest_bdd profile_store -> scenarios fail
EP-M4 green: same command -> 8 scenarios pass
EP-M5 green: cargo test -p hello_world --test profile_store_e2e -> passes
```

`make verus` requires `make install-verus` to have been run once; it never
installs on its own. If it reports `FORMAL-EMPTY`, `VERUS_PROOF_FILES` was not
updated.

## Validation and acceptance

Behavioural acceptance, observable in the example after `EP-M5`:

```console
$ XDG_CONFIG_HOME=$PWD/tmp hello-world profiles save loud --shout true
saved profile "loud" (created)
$ XDG_CONFIG_HOME=$PWD/tmp hello-world profiles list --json
{"profiles":[{"name":"loud","origins":[{"kind":"store","path":".../hello_world/profiles.json"}]}]}
$ XDG_CONFIG_HOME=$PWD/tmp hello-world --profile loud greet
HELLO, WORLD!
$ XDG_CONFIG_HOME=$PWD/tmp hello-world profiles delete loud
deleted profile "loud"
$ XDG_CONFIG_HOME=$PWD/tmp hello-world --profile loud greet
error: unknown profile 'loud' selected via --profile: no profiles are defined
```

The exact field names (`--shout`) and messages are fixed in `EP-M5` against the
example's real options and recorded here.

Quality criteria:

- Tests: `make test` passes, including the new BDD feature (8 scenarios),
  unit, property, trybuild, golden, and end-to-end tests.
- Verification: `make verus` verifies the 9.1.2 and 9.1.3 proof files with no
  trusted shortcuts; every seeded fault was observed failing and reverted.
- Lint and format: `make check-fmt` and `make lint` pass.
- Documentation: `make markdownlint` and `make nixie` pass.
- Security: no `secret` value in any store written by tests (asserted);
  `show` output contains no protected value (asserted and proved).

## Idempotence and recovery

Every step is re-runnable. Tests use temporary directories and injected
environments, never the user's real configuration directory. Seeded faults are
applied in scratch commits that are dropped, or in the working tree and
reverted with `git restore`, after copying any evidence out first. If a gate
fails, read its log under `/tmp` before re-running. Each milestone is one or
more commits that can be reverted independently in reverse order.

## Artefacts and notes

Reconnaissance observations (already obtained):

```plaintext
$ git grep -n "list_command" -- '*.rs'
ortho_config/src/agent_context/profiles/mod.rs:36:    pub list_command: Option<Vec<String>>,
... (definition, constructors, wire, serializer only)
ortho_config/src/agent_context/tests_json.rs:222:        profiles.get("list_command").is_none(),
$ git grep -n "ORTHO_DOCS_IR_VERSION.*=" v0.11.0 -- ortho_config/src/docs
v0.11.0:ortho_config/src/docs/mod.rs:21:pub const ORTHO_DOCS_IR_VERSION: &str = "2.0";
```

Probe transcripts, red/green evidence, and seeded-fault results are added here
as milestones run.

## Interfaces and dependencies

All new public items live under `ortho_config::profile::store` (re-exported from
`ortho_config::profile`), except the error variants and the agent-context
builder. Signatures are prescriptive; Rustdoc examples are required for each.

```rust
// ortho_config/src/profile/store/body.rs
/// A validated profile body: a JSON object without reserved root keys.
pub struct ProfileBody(serde_json::Map<String, serde_json::Value>);
impl ProfileBody {
    pub fn try_from_value(value: serde_json::Value) -> Result<Self, ProfileStoreError>;
    pub fn from_serialize<T: serde::Serialize>(value: &T) -> Result<Self, ProfileStoreError>;
    pub fn as_map(&self) -> &serde_json::Map<String, serde_json::Value>;
}

// ortho_config/src/profile/store/document.rs
/// The versioned, persistence-agnostic store document.
pub struct ProfileStoreDocument { /* version, BTreeMap<ProfileName, ProfileBody> */ }
impl ProfileStoreDocument {
    pub fn empty() -> Self;
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, ProfileStoreError>;
    pub fn to_json_vec(&self) -> Vec<u8>;
    pub fn names(&self) -> impl Iterator<Item = &ProfileName>;
    pub fn get(&self, name: &ProfileName) -> Option<&ProfileBody>;
    pub fn save(&mut self, name: ProfileName, body: ProfileBody,
                protection: &ProfileProtection) -> Result<SaveOutcome, ProfileStoreError>;
    pub fn delete(&mut self, name: &ProfileName) -> Result<(), ProfileStoreError>;
}
pub enum SaveOutcome { Created, Replaced }

// ortho_config/src/profile/store/protection.rs
/// Protected key paths derived from documentation metadata.
pub struct ProfileProtection { /* BTreeMap<Vec<String>, ProfileFieldExposure> */ }
impl ProfileProtection {
    pub fn none() -> Self;
    pub fn from_doc_metadata(metadata: &crate::docs::DocMetadata) -> Self;
    pub fn check_storable(&self, body: &ProfileBody) -> Result<(), ProfileStoreError>;
    pub fn project(&self, body: &ProfileBody) -> ShownProfile;
}
/// A body with protected values removed, plus the withheld key paths present.
pub struct ShownProfile { /* body: serde_json::Map, withheld: Vec<String> */ }

// ortho_config/src/profile/store/file.rs
pub struct JsonProfileStore { /* cap_std Dir, file name, StoreOptions */ }
pub struct StoreOptions { pub lock_timeout: std::time::Duration, pub max_bytes: u64 }
impl JsonProfileStore {
    pub fn open(dir: cap_std::fs_utf8::Dir, file_name: &camino::Utf8Path,
                options: StoreOptions) -> Self;
    pub fn open_path(path: &camino::Utf8Path, options: StoreOptions)
        -> Result<Self, ProfileStoreError>;
    pub fn load(&self) -> Result<ProfileStoreDocument, ProfileStoreError>;
    pub fn list(&self) -> Result<Vec<ProfileName>, ProfileStoreError>;
    pub fn show(&self, name: &ProfileName, protection: &ProfileProtection)
        -> Result<ShownProfile, ProfileStoreError>;
    pub fn save(&self, name: ProfileName, body: ProfileBody, guard: &SaveGuard<'_>)
        -> Result<SaveOutcome, ProfileStoreError>;
    pub fn delete(&self, name: &ProfileName) -> Result<(), ProfileStoreError>;
}
/// The protection and hand-authored names a save must respect.
pub struct SaveGuard<'a> { pub protection: &'a ProfileProtection,
                           pub hand_authored: &'a ProfileCatalogue }

// ortho_config/src/profile/store/location.rs
pub struct ProfileStoreLocation;
impl ProfileStoreLocation {
    pub fn user_default(app_name: &str, file_name: &str, env: &dyn crate::EnvSource)
        -> crate::OrthoResult<camino::Utf8PathBuf>;
}

// ortho_config/src/profile/store/error.rs
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProfileStoreError {
    Io { operation: StoreOperation, path: camino::Utf8PathBuf, source: std::io::Error },
    Malformed { path: Option<camino::Utf8PathBuf>, reason: String },
    UnsupportedVersion { found: u64, supported: u64 },
    TooLarge { path: camino::Utf8PathBuf, limit: u64 },
    LockTimeout { path: camino::Utf8PathBuf, waited: std::time::Duration },
    NotInStore { name: ProfileName, available: AvailableProfileNames },
    DefinedInConfigFile { name: ProfileName, path: camino::Utf8PathBuf },
    SecretValue { name: ProfileName, key_path: String },
    ReservedKey { key: String },
    NotATable,
}

// ortho_config/src/profile/catalogue.rs
pub struct ProfileCatalogue { /* Vec<CatalogueEntry> */ }
pub struct CatalogueEntry { pub name: ProfileName, pub origins: Vec<ProfileOrigin> }
#[non_exhaustive]
pub enum ProfileOrigin { ConfigFile(camino::Utf8PathBuf), Store(camino::Utf8PathBuf) }
impl ProfileCatalogue {
    pub fn names(&self) -> impl Iterator<Item = &ProfileName>;
    pub fn get(&self, name: &ProfileName) -> Option<&CatalogueEntry>;
    pub fn show(&self, name: &ProfileName, protection: &ProfileProtection)
        -> crate::OrthoResult<ShownProfile>;
}

// ortho_config/src/profile/extract.rs (changed signature)
pub fn extract_profile_layers(layers: Vec<MergeLayer<'static>>,
    selected: Option<&SelectedProfile>, discovery_failed: bool,
    store: Option<&StoreSnapshot>) -> OrthoResult<ExtractionOutcome>;

// ortho_config/src/agent_context/profiles/mod.rs
impl ProfilesDeclaration {
    pub fn with_list_command(self, path: Vec<String>) -> OrthoResult<Self>;
}

// ortho_config/src/docs/ir/mod.rs
pub struct DocProfilesMeta { pub flag: String, pub env_var: String,
    pub list_command: Option<Vec<String>> }

// ortho_config/src/error/types.rs (new variants)
ProfileDefinedTwice { name: String, config_file: camino::Utf8PathBuf, store: camino::Utf8PathBuf },
ProfileStore(Box<ProfileStoreError>),
```

Generated for structs with `#[ortho_config(profiles(store))]`:

```rust
fn profile_catalogue() -> OrthoResult<ProfileCatalogue>;
fn profile_catalogue_with_sources(env: SharedEnvSource) -> OrthoResult<ProfileCatalogue>;
fn profile_store_with_sources(env: SharedEnvSource)
    -> OrthoResult<(JsonProfileStore, ProfileProtection)>;
```

Exact names are fixed in `EP-M4` against the existing generated naming
conventions and recorded in `Decision log`.

Reuse policy: `ortho_config::profile::validate` is the single owner of profile
name and body validation; extraction, the store, and the catalogue call it and
must not re-implement it. The replace and lock helpers are crate-private to
`ortho_config` and may be used only by the store adapter until a separate
decision publishes them. `ProfileProtection` is the only component that
interprets `ProfileFieldExposure` for values; generated documentation keeps
using `DocMetadata::redacted_for_output`.

Dependencies: none added. Existing runtime dependencies used: `serde`,
`serde_json` (existing optional feature, already required by profiles),
`cap-std` (`fs_utf8`), `camino`, `thiserror`, `tracing`. Development
dependencies used are already in the workspace.

## Revision note

- 2026-11-28: initial draft from reconnaissance (code surface, documentation,
  Netsuke and Weaver `main` on 2026-11-27, and web prior art). Status DRAFT;
  awaiting expert review and user approval.
