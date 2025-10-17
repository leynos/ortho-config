# ADR-001: Replace `serde_yaml` with `serde-saphyr` for YAML Parsing

Date: 2025-10-17

Status: Proposed

## Context and Problem Statement

`ortho-config` is a layered configuration library for Rust that aggregates
settings from multiple sources, including YAML files, environment variables,
and command-line arguments. It currently uses the `figment` crate to manage
this aggregation, leveraging `figment`'s built-in provider for YAML, which
depends on the `serde_yaml` crate.

The `serde_yaml` crate, and its popular fork `serde_yml` are effectively
unmaintained and suffer from significant drawbacks. They rely on bindings to
the C library `libyaml` which introduces `unsafe` code into the dependency
tree. Furthermore, `serde_yml` has demonstrated unsound behaviour, including
the potential for segmentation faults. Relying on an unmaintained, potentially
unsafe dependency poses a significant risk to the stability, security, and
long-term viability of `ortho-config`.

This ADR addresses the need to replace `serde_yaml` with a modern, safe, and
actively maintained alternative for YAML deserialization, while preserving the
powerful configuration merging capabilities provided by `figment`.

## Decision Drivers

- **Safety**: Eliminating `unsafe` code and C dependencies is a primary goal
  for improving the robustness and security of the library.
- **Maintenance**: Moving to a dependency with active and trustworthy
  maintenance is critical for long-term project health.
- **YAML Specification Compliance**: Adopting a parser that is fully compliant
  with the modern YAML 1.2 specification ensures predictable and correct
  behaviour.
- **Performance**: While not a primary bottleneck, improving parsing
  performance and reducing memory allocations is a desirable outcome.
- **Integration**: The chosen solution must integrate cleanly with the existing
  `figment`-based architecture.

## Considered Options

### Option 1: Status Quo (Continue with `serde_yaml`)

- **Description**: Make no changes and continue to rely on `figment`'s default
  YAML provider.
- **Pros**: No immediate development effort required.
- **Cons**:

- Inherits the risks of an unmaintained and unsound dependency.
- Relies on an older YAML 1.1 parser.
- Fails to address the core problem of dependency rot and safety.
- **Viability**: Not a viable long-term option.

### Option 2: Migrate to a `libyaml`-based Fork (`serde_yaml_ng`, `serde_norway`)

- **Description**: Replace `serde_yaml` with a community-maintained fork that
  continues to use `libyaml`.
- **Pros**:

- Provides a temporary solution with more active maintenance.
- Likely a near drop-in replacement.
- **Cons**:

- Does not eliminate the core issue of the `unsafe` `libyaml` dependency.
- Still targets the outdated YAML 1.1 specification.
- Represents an incremental improvement rather than a definitive solution.
- **Viability**: Viable as a short-term fix, but does not align with the
  long-term goal of embracing Rust's safety guarantees.

### Option 3: Migrate to `serde-saphyr` (Recommended)

- **Description**: Replace `serde_yaml` with `serde-saphyr` a pure-Rust, YAML
  1.2 compliant parser. This requires creating a custom integration layer to
  feed the parsed data into `figment`.
- **Pros**:

- **Memory Safe**: Completely pure Rust, eliminating all `unsafe` code
  associated with YAML parsing.
- **Actively Maintained**: A modern library with a focus on correctness and
  performance.
- **YAML 1.2 Compliant**: Adheres to the current YAML specification, avoiding
  legacy quirks.
- **Performant**: Benchmarks show it is significantly faster than
  `libyaml`-based solutions due to its zero-copy, single-pass design.
- **Robust**: Includes built-in safeguards against resource exhaustion attacks
  (e.g., billion laughs).
- **Cons**:

- `figment` does not have a native `serde-saphyr` provider, requiring a small
  amount of custom integration code.
- The library is newer than `serde_yaml` though it is well-tested against the
  official YAML test suite.
- **Viability**: Highly viable and aligns perfectly with all decision drivers.
  The required integration effort is minimal and localised.

## Decision Outcome

**Chosen Option**: Option 3. We will replace `serde_yaml` with `serde-saphyr`
as the YAML parser for `ortho-config`.

This decision prioritises safety, maintainability, and correctness. The
technical analysis confirms that `ortho-config`'s architecture is sufficiently
decoupled to allow for the creation of a custom `figment` provider for
`serde-saphyr` with minimal disruption to the existing codebase. The benefits
of moving to a pure-Rust, modern, and performant library far outweigh the minor
implementation cost of the integration shim.

## Implementation Plan

This document outlines the specific steps required to execute the migration
detailed in ADR-001.

### 1. Dependency Management (`ortho_config/Cargo.toml`)

The first step is to modify the crate's dependencies. We will remove the `yaml`
feature from `figment` and add a direct dependency on `serde-saphyr` and
`serde_json`. `serde_json` will serve as a convenient, strongly-typed
intermediate representation that can be easily converted into a `figment`
provider.

#### Current `[features]` section

```toml
[features]
default = ["clap", "toml", "yaml"]
clap = ["dep:clap", "ortho_config_macros/clap"]
toml = ["dep:toml", "figment/toml"]
yaml = ["figment/yaml"]

```

#### New `[features]` section

```toml
[features]
default = ["clap", "toml", "yaml"]
clap = ["dep:clap", "ortho_config_macros/clap"]
toml = ["dep:toml", "figment/toml"]
yaml = ["dep:serde_saphyr", "dep:serde_json"]

```

#### Current `[dependencies]` section

```toml
# (Dependencies listed)
figment = { version = "0.10.10", features = ["env"] }
# ...

```

#### New `[dependencies]` section

```toml
# (Dependencies listed)
figment = { version = "0.10.10", features = ["env"] }
serde_saphyr = { version = "0.2.0", optional = true } # Use the latest version
serde_json = { version = "1.0", optional = true }
# ...

```

### 2. Create a Custom `figment` Provider

In `ortho_config/src/file.rs` we will define a new provider that wraps
`serde-saphyr`. This provider will parse the YAML file and convert its contents
into a format that `figment` understands.

```rust
// In ortho_config/src/file.rs

#[cfg(feature = "yaml")]
use figment::{
    error::Kind,
    value::{Dict, Value},
    Metadata, Profile, Provider,
};
#[cfg(feature = "yaml")]
use std::path::PathBuf;

#[cfg(feature = "yaml")]
#[derive(Debug, Clone)]
pub struct SaphyrYaml {
    path: PathBuf,
}

#[cfg(feature = "yaml")]
impl SaphyrYaml {
    pub fn file<P: Into<PathBuf>>(path: P) -> Self {
        SaphyrYaml { path: path.into() }
    }
}

#[cfg(feature = "yaml")]
impl Provider for SaphyrYaml {
    fn metadata(&self) -> Metadata {
        Metadata::from(format!("Saphyr YAML: `{}`", self.path.display()))
    }

    fn data(&self) -> Result<Dict, figment::Error> {
        // 1. Read the file content.
        let content = std::fs::read_to_string(&self.path)
            .map_err(|e| Kind::Io(e).at(self.path.display().to_string()))?;

        // 2. Parse with serde-saphyr into a serde_json::Value.
        let value: serde_json::Value = serde_saphyr::from_str(&content)
            .map_err(|e| Kind::InvalidValue(content, format!("{}", e)))?;

        // 3. Convert serde_json::Value into figment::Value, then into a Dict.
        let figment_value: Value = value.into();
        figment_value
            .into_dict()
            .ok_or_else(|| Kind::InvalidType(figment_value.to_string(), "a dictionary".into()).into())
    }
}

```

### 3. Integrate the Custom Provider

With the custom provider defined, we update the `parse_config_by_format`
function in `ortho_config/src/file.rs` to use it for files with `.yml` or
`.yaml` extensions.

#### Current `parse_config_by_format` logic

```rust
// ...
        #[cfg(feature = "yaml")]
        "yml" | "yaml" => Ok(Box::new(figment::providers::Yaml::file(path))),
// ...

```

#### New `parse_config_by_format` logic

```rust
// ...
        #[cfg(feature = "yaml")]
        "yml" | "yaml" => Ok(Box::new(SaphyrYaml::file(path))),
// ...

```

### 4. Testing Strategy

The existing test suite provides excellent coverage, but it should be augmented
to validate the new implementation thoroughly.

1. **Run Existing Tests**: Execute the full test suite, including all
   integration and cucumber tests (`cargo test --workspace`). This will verify
   that the behaviour of configuration loading and merging remains unchanged
   from a user's perspective. Pay close attention to tests in
   `ortho_config/tests/extends.rs` and features like
   `ortho_config/tests/features/extends.feature`.
2. **YAML 1.2 Compliance Tests**: Add new unit tests specifically for YAML
   parsing in `ortho_config/src/file/file_tests.rs`. These tests should assert:

   - A file containing `key: yes` results in the string `"yes"`, not the boolean
     `true`.
   - A file containing duplicate keys returns a parsing error, as this is the
     default and correct behaviour for `serde-saphyr`.

3. **Error Handling Tests**: Add tests to ensure that malformed YAML files
   produce clear and actionable errors from the `SaphyrYaml` provider.

### 5. Documentation and Communication

1. **Changelog**: Add an entry to `CHANGELOG.md` for the new release. This
   should be flagged as a **potentially breaking change** for users who may
   have been unknowingly relying on YAML 1.1's lenient parsing rules (e.g.,
   unquoted booleans).
2. **README**: Update `README.md` and any other user guides to state that
   `ortho-config` now uses a fully compliant YAML 1.2 parser.
3. **Release Versioning**: Given the potential for a minor breaking change in
   parsing behaviour, this update should correspond to a **minor version bump**
   (e.g., from `0.5.x` to `0.6.0`), in accordance with semantic versioning.

By following this comprehensive plan, the migration to `serde-saphyr` can be
performed efficiently, safely, and with a high degree of confidence, ultimately
resulting in a more robust and future-proof `ortho-config` library.
