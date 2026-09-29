# Migration guide: v0.9.0 to v0.10.0

## Who should read this

Read this guide when adopting source-aware environment merging, parser-faithful
clap string defaults, optional profile overlays, or the Cargo
external-subcommand helper. Existing callers can upgrade without changing their
loading code: process-backed behaviour remains the default, profile support is
opt-in, and applications that use neither profiles nor the helper require no
changes.

## Adopt the opt-in agent-native policy check

Run `cargo orthohelp --check-agent-native --package <package>` to write a
policy report for a package. The `--package <package>` argument is required
when the workspace has no root package, such as a virtual workspace. Configure
the policy in the `[package.metadata.ortho_config.policy]` metadata table. The
`off`, `warn`, and `deny` modes select disabled, advisory, and failing policy
behaviour. See the
[agent-native policy section in the user's guide][users-guide-policy] for the
report shape, exceptions, and command-line override.

## Keep the default process behaviour

`load()`, `load_from_iter()`, and the existing subcommand merge methods
continue to read the process environment. No migration is required for
applications that do not need a hermetic environment boundary.

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

For an `OrthoConfig`-derived subcommand, pass a `SharedScanEnvSource` to
`load_and_merge_with_sources`:

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

## Adopt optional profile overlays

Profile support is opt-in and additive. Adding the struct-level attribute
`#[ortho_config(profiles)]` enables three things on that configuration struct:

- `[profile.<name>]` overlay tables inside the resolved configuration files,
  using the equivalent `profile.<name>` key path for JSON5 and YAML;
- a generated `--profile <name>` flag; and
- a `<PREFIX>PROFILE` environment selector, where `<PREFIX>` is the struct's
  `prefix` attribute, so `APP_` gives `APP_PROFILE`.

### Decide whether to migrate

Most applications need to do nothing. Derives without the attribute compile
unchanged, keep the four-layer merge order, gain no `--profile` flag, and treat
a `[profile.*]` table in a shared file like any other unknown key.

To adopt profile support:

1. add `profiles` to the struct's existing `#[ortho_config(...)]` attribute;
2. declare `[profile.<name>]` tables in the configuration files the
   application already loads; and
3. select a profile with `--profile <name>` or `<PREFIX>PROFILE`.

The change is reversible, with two costs to weigh. Removing the attribute
restores the four-layer merge order and removes the `--profile` flag, so any
invocation that already scripts that flag breaks, and any `[profile.*]` tables
left in configuration files become inert unknown keys.

Opting in reserves the `profile` root key across all three projections. File
tables named `profile` are extracted and never merged as ordinary values, the
selector environment variable is stripped from the environment layer, and the
generated flag is excluded from the serialized command-line layer. A downstream
field that claims the `profile` key, the `--profile` flag, or the
`<PREFIX>PROFILE` binding is a compile-time error, so add the attribute as part
of a normal build and test cycle.

### Expect five precedence tiers

```text
built-in defaults < config files < selected profile < environment < flags
```

A profile overlay is still configuration data: it is selected from the file
chain, so the environment and the command line both outrank it. The profile
tier adds one layer per contributing file that defines the selected profile, in
file-chain order with the base file first, pushed after every file layer and
before the environment layer.

### Review the selection rules

The `--profile` flag beats the `<PREFIX>PROFILE` environment variable whenever
both are present, and an empty flag value likewise suppresses the environment
fallback. An empty selector value, for example `APP_PROFILE=""` left behind by
a leaked export, means "no selection" rather than an invalid name. The reserved
name `default` also means "no selection".

### Check profile names and bodies

Profile names are case-sensitive and must match `[A-Za-z0-9_-]+` (non-empty),
following Cargo's validation precedent. `default` is reserved: defining
`[profile.default]` fails with `OrthoError::ReservedProfileName`, and selecting
`default` is equivalent to selecting no profile. The keys `cmds` and `inherits`
are forbidden inside a profile body and fail with
`OrthoError::ProfileForbiddenKey`: `cmds` because subcommand loading ignores
profiles, and `inherits` because it is reserved for future single-parent
inheritance. Subcommand configuration must therefore stay outside the profile
tables.

### Adopt the selection-aware load entry points

The existing `load_from_iter` and `load_and_merge` entry points keep their
signatures and their return types. Opted-in structs additionally gain the
generated associated functions `load_with_profile_from_iter(iter)` and
`load_with_profile()`, which return `OrthoResult<ProfileLoadOutcome<Config>>`.
Use them when the application must report which profile produced the loaded
configuration:

```rust
let outcome = Config::load_with_profile()?;
for selected in outcome.selection() {
    println!("{} selected via {}", selected.name, selected.source);
}
let config = outcome.into_config();
```

`ProfileLoadOutcome` borrows the configuration through `config()`, consumes it
through `into_config()`, and reports the resolved selection through
`selection()`. That slice is empty or holds one `SelectedProfile` today, and
each entry carries the profile `name` and whether it came from the flag or the
environment. Downstream `context --json` commands own the JSON mapping; the
[user's guide](users-guide.md) shows the recommended rendering.

### Review agent-context consumers

Retyping `AgentContext.profiles` from `SupportDeclaration` to
`ProfilesDeclaration` is a Rust-level change for consumers that construct or
match `AgentContext` by struct literal. Build those values with the
`ProfilesDeclaration::unsupported()` and `ProfilesDeclaration::supported(...)`
constructors instead. The wire contract is unchanged: a struct that did not opt
in still serializes as `{ "supported": false }`.

### Handle unknown profile errors

Selecting a profile that no file defines fails with
`OrthoError::UnknownProfile`. The error names the selection, records whether it
came from the `--profile` flag or the selector environment variable, and lists
the available names sorted and capped at 16, rendering further names as "and N
more". When the chain discovered no configuration files at all, the error says
so explicitly; when files exist but define no profile tables, it reports "no
profiles were found" instead, so a leaked `<PREFIX>PROFILE` is not mistaken for
missing files. These variants reach an application alongside the other load
failures rather than replacing them: a malformed command line and an unknown
selector are reported together, with the parse error first, so neither root
cause is masked.

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

Profile support and the Cargo helper are additive. Existing configuration
loading, derive usage, and subcommand merging continue unchanged. Add the
`profiles` attribute only when an application wants profile overlays, and add
the helper only when adopting the Cargo external-subcommand entry-point shape.
The one Rust-level change in this release is the `AgentContext.profiles`
retype, which affects only consumers that build or match `AgentContext` by
struct literal.

[users-guide-policy]: users-guide.md#agent-native-policy-checking
