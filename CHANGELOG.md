# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### Added

- Support dependency aliasing via `#[ortho_config(crate = "...")]`
  attribute on `OrthoConfig` and `SelectedSubcommandMerge` derive macros
  (closes #291).
- Re-export `figment`, `uncased`, `xdg`, and optional format parsers to
  simplify dependency graphs for consumers.
- Forward `json5`, `yaml`, and `toml` feature flags to
  `ortho_config_macros`, so macros compile with matching support.
- Introduce the `#[ortho_config(discovery(...))]` attribute to customise config
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
  parse arguments with localised command metadata and parse errors.
- Add the public `parse_localized_command` helper for applications that need
  to parse an already-localised `clap::Command` with a custom message-id base.

### Changed

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

### Changed (design)

- Rename the agent-context defaulting-table row from `skill_manifest_paths` to
  `skill_manifests` to reflect that entries are structured descriptors rather
  than bare paths.

### Fixed

- Skip YAML-specific `hello_world` `rstest-bdd` scenarios unless the `yaml`
  feature is enabled, so feature-disabled test runs no longer invoke YAML
  parsing.
- Add doc comments to generated `OrthoConfig` support structs so crates with
  strict `missing_docs` linting build without broad suppressions (closes
  #253).
