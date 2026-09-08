# Migration guide: v0.9.0 to v0.10.0

## Who should read this

Read this guide when adopting source-aware environment merging or
parser-faithful clap string defaults. Existing callers can upgrade without
changing their loading code: process-backed behaviour remains the default.

## Keep the default process behaviour

`load()`, `load_from_iter()`, and the existing subcommand merge methods continue
to read the process environment. No migration is required for applications
that do not need a hermetic environment boundary.

## Opt into injected environment sources

Use `MapEnv` when tests or an embedding application must supply all environment
values explicitly. It implements both source traits, so one map can drive
discovery lookups and merge-layer enumeration:

```rust
use ortho_config::{
    MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource,
};
use std::sync::Arc;

let environment = Arc::new(MapEnv::new().with_var("ACME_PORT", "9000"));
let discovery: SharedEnvSource = environment.clone();
let merge: SharedScanEnvSource = environment;
let config = Config::load_from_iter_with_sources(["acme"], discovery, merge)?;
```

The discovery source performs named lookups. The separate scan source gives the
merge layer permission to enumerate the supplied variables. This separation
preserves the `EnvSource` safety boundary while allowing a complete
configuration resolution without process mutation.

## Review `CsvEnv` transforms

`CsvEnv::with_source` replays the declarative environment options against the
injected source. Prefix matching is case-insensitive; configured case
conversion and split operations are replayed in builder order. Splits replace
their separators with dotted key components. CSV parsing remains enabled by
default, and `csv(false)` keeps comma-containing values scalar.

Providers that use `map()` or `filter_map()` cannot be injected. Those methods
store arbitrary closures, which cannot be replayed against a
`SharedScanEnvSource`; `CsvEnv::with_source` therefore rejects that provider
rather than silently changing its key mapping.

## Inject a subcommand merge source

For an `OrthoConfig`-derived subcommand, pass a
`SharedScanEnvSource` to `load_and_merge_with_sources`:

```rust
let environment = Arc::new(
    MapEnv::new().with_var("ACME_SERVE_CMDS_SERVE_PORT", "9000"),
);
let config = cli.load_and_merge_with_sources(environment)?;
```

This injects the subcommand environment layer while retaining the existing
command-line precedence. A clap-only argument type that does not derive
`OrthoConfig` remains parse-only and does not support source-aware merge APIs.

## Infer parser-faithful string defaults

Enable `cli_default_as_absent` on a field whose clap default should remain
below configuration-file and environment values:

```rust
#[derive(clap::Parser, serde::Deserialize, ortho_config::OrthoConfig)]
struct Args {
    #[arg(long, default_value = "8080")]
    #[ortho_config(cli_default_as_absent)]
    port: u16,
}
```

When no explicit CLI value is supplied, the default is placed in the generated
defaults layer. An explicit CLI value still wins according to the existing
merge precedence. `default_value_t` and `default_values_t` keep their existing
inference paths.

String `default_value` is parsed by a synthetic clap argument using the same
field parser metadata captured from the derive input. This preserves behaviour
for:

- scalar primitive and standard-library types;
- `Option<T>` and `Vec<T>` fields;
- `ValueEnum` fields; and
- fields with a custom `#[arg(value_parser = ...)]` parser.

Parser settings that affect the result, such as a value delimiter or
case-insensitive enum parsing, are replayed with the generated argument. An
explicit `#[ortho_config(default = ...)]` always takes precedence over an
inferred clap default.

## Review unsupported shapes and errors

Nested `Option`/`Vec` wrappers and map fields are rejected at compile time when
combined with inferred `default_value`, because their shape cannot be
reconstructed faithfully by the generated loader. Use an explicit
`#[ortho_config(default = ...)]` for those fields.

If clap cannot parse a supported field's default, loading returns
`OrthoError::DefaultValueConversion` through the normal accumulated error path.
The generated code does not panic while resolving the default. Applications
that inspect `OrthoError` should handle this variant alongside other load
failures when they need field-specific diagnostics.

No migration is required for fields using only typed defaults. For fields that
duplicated a string default in both clap and `#[ortho_config(default = ...)]`,
the duplicate can be removed after confirming that the field shape and parser
are supported by this guide.
