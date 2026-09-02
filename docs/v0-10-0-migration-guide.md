# Migration guide: v0.9.0 to v0.10.0

## Who should read this

Read this guide when adopting source-aware environment merging. Existing
callers can upgrade without changing their loading code: process-backed
behaviour remains the default.

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
