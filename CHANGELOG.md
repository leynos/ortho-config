# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### Added

- Add behavioural metadata for non-interactive execution and mutation
  boundaries (roadmap 7.2.1): the struct-level
  `#[ortho_config(behaviour(interaction = …, mutation = …, bypass = …,
  dry_run = …))]`
  derive attribute, carried into documentation IR version 1.2 and the
  agent-context `interaction_mode`, `mutation_effect`, `bypass_flag`, and
  `dry_run_flag` fields without a schema version bump; and the
  `cargo_orthohelp::policy::rules::behaviour` rule set, which reports
  `agent-native.behaviour.*` findings for destructive commands that lack an
  approved bypass, interactive commands that lack a bypass, bypasses that match
  no declared input, and undeclared metadata. Undeclared behaviour stays
  `unknown`; no semantics are inferred from command names or flags.
- Add an opt-in agent-native policy configuration
  (`cargo orthohelp --check-agent-native`, roadmap 7.1.1):
  `[package.metadata.ortho_config.policy]` metadata table with `off`, `warn`,
  and `deny` modes and explicit project exceptions; a machine-stable
  `policy-report.json` (schema version 1) carrying `mode`, `results`, `summary`,
  `exceptions`, and a canonical `vocabulary` block; and the `--policy-mode`
  report override (requires `--check-agent-native`).
- Add the canonical agent-native vocabulary defaults
  (`cargo_orthohelp::policy::vocabulary`) as a single source of truth shared by
  the agent-context verb mapper and the 7.1.2 lint seam.
- Add the additive `exceptions` field to the agent-context `AgentPolicy` wire
  schema (serde-defaulted; carries kind, name, and optional command scope, not
  reasons).

- Support dependency aliasing via `#[ortho_config(crate = "...")]`
  attribute on `OrthoConfig` and `SelectedSubcommandMerge` derive macros
  (closes #291).
- Re-export `figment`, `uncased`, `xdg`, and optional format parsers to
  simplify dependency graphs for consumers.
- Forward `json5`, `yaml`, and `toml` feature flags to
  `ortho_config_macros`, so macros compile with matching support.
- Introduce the `#[ortho_config(discovery(...))]` attribute to customize config
  discovery (filenames, environment overrides, and the generated CLI flag) and
  expose the new flag in the `hello_world` example.
- Add the `OrthoConfigSubcommandDocs` trait and derive so subcommand enums can
  emit per-variant documentation metadata.
- Populate recursive `DocMetadata.subcommands` values from `OrthoConfig`
  structs that hold a `#[command(subcommand)]` field.
- Add behavioural fixtures and step definitions covering nested subcommand
  trees (`ortho_config/tests/features/docs_ir_nested.feature`).
- Add renderer compatibility tests and `insta` golden snapshots for populated
  nested-subcommand `DocMetadata`
  (`cargo-orthohelp/tests/golden/nested_subcommand_snapshots.rs`).
- Add `SkillManifest`, `SkillCommandRef`, and the
  `AgentContext.skill_manifests` field for declaring downstream skill manifests
  in agent context (roadmap item 6.3.1).
- Add the public `LocalizedParse` blanket trait so any `clap::Parser` can
  parse arguments with localized command metadata and parse errors.
- Add `OrthoConfigLocalization` derive constants, compile-time normalized
  argument-id collision diagnostics, and opt-in identifier artefact emission.
- Add the public `parse_localized_command` helper for applications that need
  to parse an already-localized `clap::Command` with a custom message-id base.
- Introduce the `EnvSource` trait so configuration discovery reads the
  environment through an injectable seam instead of ambient `std::env` calls;
  add `MapEnv` and `process_env_source()`, and the
  `ConfigDiscoveryBuilder::env_source(...)` method to inject a source. The
  default remains the live process environment, so existing callers are
  unaffected (closes #410).
- Emit structured `tracing` events at each configuration-discovery decision
  point (source selection, selector resolution, XDG/Windows/home resolution,
  attempt/candidate/outcome), drawn from a closed vocabulary that never carries
  environment values, paths, or file contents. A successful `discovery.load`
  event also carries `source`, naming the rung that produced the winning
  candidate.
- Add the optional `metrics` feature, emitting
  `ortho_config.discovery.attempts`, `ortho_config.discovery.outcomes`, and
  `ortho_config.discovery.candidate_failures` counters through the `metrics`
  facade. The feature is off by default and installs no recorder.
- Add the [v0.9.0 migration guide](docs/v0-9-0-migration-guide.md) covering
  the injectable environment source, discovery telemetry, and the `metrics`
  feature.
- Bump `cargo-orthohelp` to 0.9.1, the first version whose crates.io release
  carries binstall metadata and prebuilt archives. The rest of the workspace
  stays at 0.9.0.
- Publish prebuilt `cargo-orthohelp` release archives with `.sha256` sidecars
  for `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
  `x86_64-apple-darwin`, `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`,
  and declare `[package.metadata.binstall]` so
  `cargo binstall --disable-strategies compile cargo-orthohelp` installs them
  instead of compiling from source (closes #479).
- Add `scripts/release_archive.py` and `scripts/verify_release_archives.py`,
  covered by `scripts/tests/test_release_archive.py`,
  `tests/workflow_contracts/release_workflow_test.py`, and a pull-request
  packaging dry run on Linux, macOS, and Windows.

- Add opt-in profile support (`#[ortho_config(profiles)]`, roadmap 9.1.1):
  `[profile.<name>]` config tables, a first-class profile merge layer between
  the file and environment tiers, stateless `--profile`/`<PREFIX>PROFILE`
  selection, structured unknown/invalid/reserved profile errors, and the
  `load_with_profile()` entry point returning `ProfileLoadOutcome`.
- Add `ProfilesDeclaration` and `ProfileSelectionContract` to the
  agent-context schema and the matching `DocMetadata.profiles` IR field.
- Add an integration test pinning that a non-Unicode `<PREFIX>PROFILE` selector
  fails a process-backed profile load with an encoding diagnostic, rather than
  silently loading the unprofiled configuration.

### Breaking

- Retype `AgentContext.profiles` from `SupportDeclaration` to
  `ProfilesDeclaration` (decision D7 of execplan 9-1-1). A struct literal or
  pattern that named the old type no longer type-checks, so consumers must
  update the `profiles` value itself — for example by constructing it with
  `ProfilesDeclaration::unsupported()` or `ProfilesDeclaration::supported()`.
  Those constructors protect *future* field additions, not the retype. The wire
  JSON for non-opted-in applications is unchanged: the unsupported case still
  serializes as `{ "supported": false }`.

### Changed

- Remove the remaining fixture `#[path]` attributes by moving the
  `cargo-orthohelp` PowerShell fixture under `tests/golden/` and restructuring
  the BDD support modules around top-level integration-test entry points (closes
  [#292](https://github.com/leynos/ortho-config/issues/292)).
- Emit compact agent-context JSON to stdout for `--format agent-context` and
  `--format all`, and compact policy-report JSON for `--check-agent-native`,
  while retaining both JSON artefacts. When policy checking and an explicit
  generator format are combined, the policy report is the sole JSON document on
  stdout (closes #329).
- Bump documentation IR to 2.0: generated default identifiers now use mounted
  command paths; see the localization migration guidance in the user's guide.
- Adopt `sha2` 0.11 in `cargo-orthohelp` and render cache digests through a new
  crate-internal lowercase hexadecimal encoder, because `sha2` 0.11 returns
  `hybrid_array::Array<u8, _>` from `finalize`, which no longer implements
  `core::fmt::LowerHex`. Rendered cache keys are unchanged, so existing cache
  directories remain valid (closes #400).
- Clarify that `OrthoError::MissingRequiredValues` is proposed future work, not
  part of the current public error surface, and keep the implementation tracked
  in the phase 7 roadmap.
- Report missing `extends` targets with a clear not-found error that names the
  resolved absolute path and the referencing file (closes #110).
- Clarify `ConfigDiscovery::load_first` semantics: the helper now returns
  `Err` when every candidate fails and any discovery errors were recorded, and
  `Ok(None)` only when no candidates exist without errors. Update error
  handling in consumers accordingly.
- Replace the legacy `serde_yaml` integration with a feature-gated
  `SaphyrYaml` provider backed by `serde-saphyr`, enabling YAML 1.2 semantics
  (strict boolean parsing keeps unquoted `yes` as a string and duplicate
  mapping keys raise errors) and removing the transitive `figment/yaml`
  dependency.
- Migrate the `hello_world` behavioural suite from the bespoke `cucumber-rs`
  runner to `rstest-bdd`, introducing a reusable harness fixture, compile-time
  tag filters, and removing the `cucumber`, `gherkin`, and Tokio
  dev-dependencies.
- Generated boolean CLI flags now accept an optional `=<BOOL>` value, so
  `--flag=false` supplies an explicit `false` that clears a `true` from a
  configuration file or environment variable. The bare `--flag` spelling still
  means `true`, and omitting the flag still leaves the field absent so lower
  layers win. The documentation IR reports the new surface through
  `CliMetadata.value_optional`, included in documentation IR version 2.0. The
  roff renderer prints the `--flag[=BOOL]` form, joining the placeholder to the
  flag so it cannot be misread as the invalid `--flag =BOOL` spelling, and
  PowerShell help explains the optional value in prose (closes #444).

### Changed (design)

- Rename the agent-context defaulting-table row from `skill_manifest_paths` to
  `skill_manifests` to reflect that entries are structured descriptors rather
  than bare paths.

### Fixed

- Keep the generated CLI layer when an explicit command-line value happens to
  equal the struct default. The layering guard compared the whole sanitized CLI
  object against the whole defaults object and skipped the CLI layer when they
  matched, so an explicit `--flag=false` or `--port 8080` that restated its own
  default was discarded whenever the command line covered every defaulted
  field, and a lower-precedence file or environment value silently won. The
  guard now asks clap's per-argument `value_source`, which reports the command
  line independently of the parsed value (closes #444).
- Merge a layer list that supplies no values without failing on a `Null`
  accumulator. The generated declarative state seeds its accumulator with
  `serde_json::Value::default()`, which is `Null`, and `merge_layer` skips
  empty maps, so `merge_from_layers([])` and a prefixed all-optional
  configuration with no file or environment values reached `finish` with the
  accumulator still `Null`, and failed with
  `invalid type: null, expected struct`. Those cases now yield all-`None`
  fields. Required fields are unaffected: a struct with a required field still
  reports `missing field` rather than silently defaulting (closes #444).
- Generate `compose_layers` and `compose_layers_from_iter` with
  `#[allow(dead_code, ...)]` rather than `#[expect(dead_code, ...)]`, so
  downstream `build.rs` files no longer need to allow
  `unfulfilled_lint_expectations`. See
  [the clap-mangen to cargo-orthohelp migration guide](docs/clap-mangen-cargo-orthohelp-migration-guide.md)
  for removing the remaining build-script suppressions (closes #273).
- Skip YAML-specific `hello_world` `rstest-bdd` scenarios unless the `yaml`
  feature is enabled, so feature-disabled test runs no longer invoke YAML
  parsing.
- Add doc comments to generated `OrthoConfig` support structs so crates with
  strict `missing_docs` linting build without broad suppressions (closes #253).
- Resolve the Rust 1.99 beta Clippy lints across the workspace —
  `assert_is_empty`, `trivially_copy_pass_by_ref`, `byte_char_slices`,
  `string_lit_as_bytes`, and the byte-order lints — by changing the code rather
  than suppressing the lints, and extend the advisory `clippy-beta` job to
  Windows, where `cfg(windows)` code a Linux-only leg never compiles had been
  reaching the beta channel unseen.
- Raise the documentation tests' subprocess bound from 600 s to 1,200 s. The
  child is a nested `cargo run` paying a cold build, and on a developer host
  that build shares the machine's compiler pool; three local runs of unchanged
  code took 588.0 s, 593.6 s and 600.5 s, the last killed at the old bound
  while the same test takes 56.5 s on a `build-test` runner.
