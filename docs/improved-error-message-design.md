# Proposed design for improved missing value error messages

Status: implemented in roadmap task 7.3.1. The schema contract, requiredness
matrix, and supply-route rules are settled in
[ADR-0024](adrs/adr-0024-enumerate-missing-required-values.md). The public
`OrthoError` enum in `ortho_config/src/error/types.rs` exposes the
`OrthoError::MissingRequiredValues` variant; this document records the design
that produced it.

Loads that finish composing their layers with absent required values now return
`OrthoError::MissingRequiredValues`. The enum variant separates that case from
the errors that remain: required command-line arguments rejected by `clap`
still surface as `OrthoError::CliParsing`; values that are present but invalid
still surface as `OrthoError::Merge` or the related gathering and
deserialization errors; and multiple source failures may still be reported
through `OrthoError::Aggregate`.

The implementation work was tracked in [roadmap.md](roadmap.md) as the phase
7.3.1 agent-native error-diagnostics task. Roadmap item 5.1.1 recorded that the
feature was not implemented under the proposed enum variant at that time.

This design focuses on replacing the generic `figment::Error` for missing
fields with a custom, helpful error message that guides the user on how to fix
the problem.

## 1. The new error message format

When one or more required configuration values are missing, the load returns a
clear, actionable error message, and the application should exit with it. The
rendered format is:

```plaintext
Missing required configuration for `my-program`

The following value(s) are missing:
  - `reference`
  - `database.host`

Please provide them using one of the following methods, in order of precedence:
  1. Command-Line Argument:
     --reference <value>
  2. Environment Variable:
     MY_APP_REFERENCE=<value>
     MY_APP_DATABASE__HOST=<value>
  3. Configuration File:
     reference = "<value>"
     [database]
     host = "<value>"
```

The library never exits. A consumer's `main` decides the process exit code and
may prefix the rendering with `error:`, as the users' guide example does.

This format addresses all the requirements:

- **Context**: It includes the program name.

- **Clarity**: It explicitly lists the missing fields.

- **Guidance**: It shows the user exactly how to provide the missing values.

- **Precedence**: It orders the methods by their precedence.

- **Consolidation**: It presents all missing values in a single, clean message.

## 2. Proposed changes to `OrthoError`

To support this new error message, a new variant should be added to the
`OrthoError` enum in `ortho_config/src/error/types.rs`. This separates the
"missing value" case from other gathering errors, allowing for custom
formatting. The payload is typed rather than a preformatted string, so callers
can inspect paths without parsing prose:

```rust
#[error("{0}")]
MissingRequiredValues(Box<MissingRequiredValues>),
```

`MissingRequiredValues` owns a non-empty ordered list of
`MissingRequiredValue { path: ConfigPath, routes: Vec<SupplyRoute>,
expects_object: bool }`.
The full public surface, including the `RequiredConfigSchema` companion trait
and the `#[ortho_config(nested)]` marker, is specified in
[ADR-0024](adrs/adr-0024-enumerate-missing-required-values.md).

### 2.1 Settled schema support matrix

Runtime requiredness follows the loader, not the documentation IR:

| Declared shape or attribute                   | Requiredness  |
| --------------------------------------------- | ------------- |
| `Option<T>` (one outer layer)                 | Optional      |
| `#[ortho_config(default = ...)]`              | Optional      |
| Inferred clap default                         | Optional      |
| `#[serde(default)]` on the field or container | Optional      |
| `#[serde(skip)]` / `skip_deserializing`       | Not inspected |
| Collections without a default                 | Required      |
| Any other scalar, map, or opaque type         | Required      |

A `#[serde(default)]` on a flattened field is the one exception to that table:
Serde always deserializes the field from the unmatched keys, so the default
applies only to a field it never saw and never rescues the child. A flattened
child therefore resolves to absence exactly when its type is `Option`.

A field's own deserializer is consulted after both defaults, and it changes one
answer in that table. Serde raises a missing-field error instead of calling the
helper that turns absence into `None`, so an `Option<T>` field read through
`#[serde(deserialize_with = "...")]` or `#[serde(with = "...")]` is required. A
Serde default at either level keeps it optional. The same replacement means the
child's declared type no longer describes what the loader reads, so a
`#[ortho_config(nested)]` field with a custom deserializer is reported as an
opaque leaf under its own key rather than as an object with composed children.

Nested enumeration is opt-in: only a field marked `#[ortho_config(nested)]`
composes the child's schema. `#[serde(flatten)]` composes at the current level
only when paired with that marker. An arbitrary custom `Deserialize`
implementation, a flatten without the marker, an enum payload, and a
selected-subcommand value extracted through Figment stay outside the coverage
contract and keep their existing error behaviour.

Supply routes are computed for reported entries only, in CLI, environment, and
configuration-file order. A route is omitted when its provider cannot consume
the spelling: a non-identifier canonical key has no CLI flag, and a path
segment containing a literal dot has no environment variable. A segment
containing `=` or a NUL has no variable at all — every process environment
treats `=` as the separator between name and value, and a NUL terminates the
name — so only the configuration-file route remains. The test is applied to the
whole path, not to each segment alone: the provider splits the joined name, so
`database_.host` becomes a variable it reads back as `database._host`, and an
edge underscore therefore also loses the environment route. That route quotes
any segment outside TOML's bare-key alphabet, which keeps the advertised
spelling one the file can actually express.

## 3. Implementation design

The derive macro records a required-value schema alongside the loader it
generates. `#[derive(OrthoConfig)]` emits `RequiredConfigSchema` for the
configuration root, and the standalone `#[derive(RequiredConfigSchema)]` serves
nested child types that do not implement `OrthoConfig`. The schema is static:
it records deserialization keys, aliases, requiredness, and the container's
environment prefix and CLI flags. It never consults the environment, the
process arguments, or a configuration file at macro time.

### 3.1. Identifying missing values

The generated `load_from_iter` pipeline composes the layers, merges them, and
then inspects the merged value rather than interrogating the intermediate
`Figment`. A pure collector walks the schema against that value and reports
every absent required path, so enumerating leaves never depends on which layer
supplied a sibling. Section 2.1's support matrix decides requiredness, and
nested enumeration happens only through `#[ortho_config(nested)]`.

The collector also records whether an absent path expects an object, which lets
the presentation render `[database]` instead of `database = "<value>"`.

A node the merged value spells two accepted ways at once is skipped rather than
inspected. A canonical key and an accepted alias name the same value, so Serde
rejects the second spelling as a duplicate field, or reports the first
spelling's missing field when that value is incomplete. Both diagnostics are
the deserializer's, and a missing-value report written beneath the node would
describe an absence the loader never reaches while advertising a route that
cannot repair the duplication.

### 3.2. Generating the error message

The collector produces `MissingRequiredValues`, which owns a non-empty ordered
list of paths together with the supply routes computed for each. Presentation
belongs to `ortho_config`, not the generated loader:

- The program label is the one the merge step received, so a caller that passes
  an explicit program name sees that name.
- Routes are emitted only when the provider can consume the spelling, and they
  appear in command-line, environment, and configuration-file order.
- Values never appear in the rendering; only keys, flags, and variable names
  do.

The full public surface is specified in
[ADR-0024](adrs/adr-0024-enumerate-missing-required-values.md), and the
developers' guide records the ownership boundaries.

## 4. Historical implementation outline

This outline is retained as design rationale. It is not a completed task list.
The phase 5 truth audit in [roadmap.md](roadmap.md) scheduled the
implementation as phase 7.3.1, which is now delivered; the outline below
records the original sequencing, not the final module layout.

### Task 1: Introduce the New `OrthoError` Variant

- **Objective**: Add the `MissingRequiredValues(String)` variant to the
  `OrthoError` enum.

- **File to Modify**: `ortho_config/src/error/types.rs`.

- **Acceptance Criteria**: The new variant exists, and the crate compiles.

### Task 2: Implement Pre-flight Check for a Single Missing Field

- **Objective**: Modify the `load_from_iter` implementation in the derive macro
  to check for missing fields before calling `extract()`. For this task, only
  handle the first missing field found.

- **Files to Modify**: `ortho_config_macros/src/derive/load_impl.rs`.

- **Details**:

  - After the `merge_section` but before `fig.extract()`, iterate through the
    struct's fields.

  - For each required field, use `fig.find_value(&path)` to check if the key
    exists.

  - If a key is missing, format a basic error message and return
    `Err(OrthoError::MissingRequiredValues(…))`.

  - This will likely require adding `serde::Deserialize` as a trait bound on
    the user's struct to inspect its fields.

### Task 3: Aggregate All Missing Fields

- **Objective**: Extend the pre-flight check to identify *all* missing required
  fields, not just the first one.

- **File to Modify**: `ortho_config_macros/src/derive/load_impl.rs`.

- **Details**:

  - Create a `Vec<String>` to store the paths of all missing fields.

  - Iterate through all required fields and populate this vector.

  - If the vector is not empty after checking all fields, then generate the
    error.

### Task 4: Generate the Full, User-Friendly Error Message

- **Objective**: Implement the logic to format the detailed error message as
  designed in section 1.

- **File to Modify**: `ortho_config_macros/src/derive/load_impl.rs`.

- **Details**:

  - Create a new helper function that takes the list of missing fields, the
    program name, and the configuration prefix as input.

  - This function will generate the complete error string, including the CLI,
    environment, and file examples for each missing field.

  - The main `load_from_iter` function will call this helper to construct the
    error message.

### Task 5: Add Comprehensive Tests

- **Objective**: Write tests to verify the new error handling behaviour.

- **File to Create**: A new test file in `ortho_config/tests/`, perhaps
  `missing_fields.rs`.

- **Details**:

  - Add a test case where a single required field is missing.

  - Add a test case where multiple required fields are missing, including one
    in a nested struct.

  - Add a `trybuild` compile-fail test to ensure that the error reporting
    integrates correctly with the macro.

  - Verify that the error message content is correctly formatted and contains
    the right information.

By following this design and roadmap, `ortho-config` can be enhanced to provide
a significantly better developer and end-user experience, turning a frustrating
dead-end error into a helpful guide.
