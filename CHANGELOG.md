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
