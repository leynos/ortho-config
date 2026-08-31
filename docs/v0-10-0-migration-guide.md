# Migration guide: v0.9.0 to v0.10.0

## Who should read this

Read this guide when upgrading an application or library from OrthoConfig
v0.9.0 to v0.10.0. The release adds parser-faithful inference for clap string
defaults when `cli_default_as_absent` is enabled. Existing typed clap defaults
and explicit OrthoConfig defaults continue to work as before.

## Impact at a glance

| Area          | Impact                                                                            |
| ------------- | --------------------------------------------------------------------------------- |
| CLI defaults  | `default_value` can be inferred without a duplicate OrthoConfig default.          |
| Precedence    | Explicit `#[ortho_config(default = ...)]` still overrides inferred clap values.   |
| Diagnostics   | Unsupported shapes fail at compile time; conversion failures are load errors.     |

_Table 1: Application-facing changes when moving from v0.9.0 to v0.10.0._

## 1. Enable parser-faithful string-default inference

Add `cli_default_as_absent` to a field whose clap default should not override a
value supplied by a file or environment variable:

```rust
#[derive(clap::Parser, serde::Deserialize, ortho_config::OrthoConfig)]
struct Args {
    #[arg(long, default_value = "8080")]
    #[ortho_config(cli_default_as_absent)]
    port: u16,
}
```

When no explicit CLI value is supplied, the default is placed in the generated
defaults layer and remains available to file and environment layers. An
explicit CLI value still wins according to the existing merge precedence.
`default_value_t` and `default_values_t` keep their existing inference paths.

String `default_value` is parsed by a synthetic clap argument using the same
field parser metadata captured from the derive input. This preserves parser
behaviour for:

- scalar primitive and standard-library types;
- `Option<T>` and `Vec<T>` fields;
- `ValueEnum` fields; and
- fields with a custom `#[arg(value_parser = ...)]` parser.

Parser settings that affect the result, such as a value delimiter or
case-insensitive enum parsing, are replayed with the generated argument. An
explicit `#[ortho_config(default = ...)]` always takes precedence over an
inferred clap default, so existing declarations do not need to be removed.

## 2. Review unsupported shapes and errors

Nested `Option`/`Vec` wrappers and map fields are rejected at compile time when
combined with inferred `default_value`, because their shape cannot be
reconstructed faithfully by the generated loader. Use an explicit
`#[ortho_config(default = ...)]` for those fields.

If clap cannot parse a supported field's default, loading returns
`OrthoError::DefaultValueConversion` through the normal accumulated error path.
The generated code does not panic while resolving the default. Applications
that already inspect `OrthoError` should handle this variant alongside other
load failures when they need field-specific diagnostics.

No migration is required for fields using only typed defaults. For fields that
duplicated a string default in both clap and `#[ortho_config(default = ...)]`,
the duplicate can be removed after confirming that the field shape and parser
are supported by this guide.
