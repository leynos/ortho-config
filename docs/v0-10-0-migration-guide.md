# Migration guide: v0.9.0 to v0.10.0

## Who should read this

Read this guide when adopting source-aware environment merging or the Cargo
external-subcommand helper. Existing callers can upgrade without changing their
loading code: process-backed behaviour remains the default, and applications
that do not provide Cargo external subcommands require no changes.

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

## Adopt the Cargo external-subcommand helper

Cargo invokes `cargo <name>` by executing `cargo-<name>` with `<name>` injected
as the first argument after the executable name. A hand-built parser that only
models the tool's options rejects that token before application logic runs.
Wrap the existing command at the entry-point boundary:

```rust
use ortho_config::cargo::external_subcommand;

let args_command = clap::Command::new("demo")
    .arg(
        clap::Arg::new("verbose")
            .long("verbose")
            .action(clap::ArgAction::SetTrue),
    );
let cli = external_subcommand("cargo-demo", "demo", args_command);
let matches = cli
    .try_get_matches_from(["cargo-demo", "demo", "--verbose"])
    .expect("the Cargo-injected subcommand parses");
let demo = matches
    .subcommand_matches("demo")
    .expect("the wrapped command requires the subcommand");
assert!(demo.get_flag("verbose"));
```

The returned command accepts both Cargo dispatch and direct invocation with the
same injected token. Read the wrapped options through
`subcommand_matches("demo")`; the helper does not alter OrthoConfig's merge
precedence or add another configuration-loading pathway.

The helper is for hand-built commands. Derive-based callers can keep the
single-variant `#[command(subcommand)]` wrapper used by `cargo-orthohelp`.

## No migration required for other users

The helper is additive. Existing configuration loading, derive usage, and
subcommand merging continue unchanged. Add the helper only when adopting the
Cargo external-subcommand entry-point shape.
