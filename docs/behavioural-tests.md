# Behavioural Testing Design for `ortho-config`

This document outlines high-level behavioural tests for the `ortho-config`
crate using the [^cucumber] framework. Scenarios follow the **Given/When/Then**
pattern and reference [`docs/design.md`](design.md) and
[`docs/ddlint-gap-analysis.md`](ddlint-gap-analysis.md).

## 1. Goals

- Validate that configuration layers are merged with the documented precedence.
- Ensure naming conventions map fields to CLI, environment, and file keys.
- Verify subcommand configuration via the `cmds` namespace.
- Test behaviours such as the `append` merge strategy on arrays.
- Cover gaps like comma-separated list parsing and `extends` support.

## 2. Cucumber Setup

Scenarios live under `tests/features/`. Step implementations in `tests` share a
common `World` struct that uses `figment::Jail` for isolation. Each scenario
executes asynchronously with `tokio`.

```rust
#[derive(Debug, Default, cucumber::World)]
struct World {
  jail: Option<figment::Jail>,
  result: Option<Result<MyConfig, ortho_config::OrthoError>>,
}
```

## 3. Scenarios

### 3.1 Loading Order

**Given** a default configuration, a file, environment variables, and CLI flags
**When** `MyConfig::load()` is called **Then** CLI values override environment
variables, which override file values, which override defaults

### 3.2 Automatic Naming

**Given** a struct field `listen_port` **When** configuration comes from CLI,
environment, or a file **Then** the CLI flag is `--listen-port`, the env var is
`PREFIX_LISTEN_PORT`, and the file key is `listen_port`

### 3.3 File Discovery

**Given** config files in the current directory and user home **When** no
`--config-path` or env override is provided **Then** the loader prefers the
local file and falls back to the home file

### 3.4 XDG Support

**Given** `XDG_CONFIG_HOME` contains `<prefix>/config.toml` **When**
`MyConfig::load()` is called **Then** the file from that directory is loaded

### 3.5 Subcommand Namespace

**Given** `[cmds.test]` values in a file and matching environment variables
**When** loading the configuration for the `test` subcommand **Then** values
from CLI, env, and file merge into the `CmdCfg` struct

### 3.6 Append Merge Strategy

**Given** vector fields with `merge_strategy = "append"` in several sources
**When** configuration is loaded **Then** the vectors are concatenated in file
→ env → CLI order

### 3.7 Comma-Separated Lists (Gap)

**Given** `DDLINT_RULES=A,B,C` in the environment **When** loading a vector
field **Then** it is parsed as `["A", "B", "C"]`

### 3.8 Configuration Inheritance (Gap)

**Given** a file with `extends = "base.toml"` **When** `MyConfig::load()` is
called **Then** `base.toml` is loaded first and overridden by the current file

### 3.9 Custom Option Names (Gap)

**Given** a field `config_path` with `cli_long = "config"` **When** the user
specifies `--config` **Then** the file is loaded from the provided path

### 3.10 Dynamic Rule Tables (Gap)

**Given** arbitrary `[rules.*]` entries in a file **When** they deserialize
into a `BTreeMap<String, RuleCfg>` **Then** unknown keys are preserved

### 3.11 Ignore Pattern Lists (Gap)

**Given** `DDLINT_IGNORE_PATTERNS=.git/,build/` in the environment **When**
loading `ignore_patterns` as a vector **Then** it becomes `[".git/", "build/"]`

## 4. Future Scenarios

The design document lists potential future work such as async loading and
custom providers. Add scenarios as those features land.

[^cucumber]: <https://github.com/cucumber-rs/cucumber>
