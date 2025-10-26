# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### Added

- Re-export `figment`, `uncased`, `xdg`, and optional format parsers to
  simplify dependency graphs for consumers.
- Forward `json5`, `yaml`, and `toml` feature flags to
  `ortho_config_macros`, so macros compile with matching support.
- Introduce the `#[ortho_config(discovery(...))]` attribute to customise config
  discovery (filenames, environment overrides, and the generated CLI flag) and
  expose the new flag in the `hello_world` example.

### Changed

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
