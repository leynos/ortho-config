# Migration guide: v0.9.0 to v0.10.0

## Who should read this

Read this guide when adopting source-aware environment merging, parser-faithful
clap string defaults, the optional-value boolean flags, or the Cargo
external-subcommand helper. Existing callers can upgrade without changing
their loading code: process-backed behaviour remains the default, and
applications that do not use the Cargo helper require no changes.

## Impact at a glance

| Change                                    | Affects                            | Required?          |
| ----------------------------------------- | ---------------------------------- | ------------------ |
| Optional-value boolean flags              | Commands with `bool` CLI fields     | No; additive       |
| Explicit CLI values always win            | Single-field configuration structs  | No; a defect fix   |
| Documentation IR version `1.1` to `1.2`   | Consumers pinning the IR version    | No; accept `1.2`   |
| Injected environment sources              | Tests needing hermetic environments | No; opt-in         |
| Parser-faithful clap string defaults      | Fields using `cli_default_as_absent`| No; opt-in         |
| Cargo external-subcommand helper          | Hand-built Cargo subcommands        | No; additive       |

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

## Review boolean CLI flag spellings

Generated boolean flags accept an optional `=<BOOL>` value. The bare flag and
omitted flag keep their previous meanings, so the change is additive for every
caller. Commands that pass a space-separated value are the exception: `--flag
false` was never accepted and now fails with a clearer error, because the
value must follow an `=`.

Before, a boolean flag could only express `true`, and a `true` from a
configuration file or environment variable could not be cleared from the
command line:

```plaintext
# config.toml sets excited = true
$ app --excited=false
excited = true     # the explicit false was not representable
```

After, `--flag=false` supplies an explicit `false` that overrides any lower
layer, while omitting the flag still defers to it:

```plaintext
# config.toml sets excited = true
$ app --excited=false
excited = false    # the command line clears the lower layer
$ app
excited = true     # omission still defers
```

Nothing needs changing to keep the old behaviour. Adopt the explicit form
where a user needs to override a `true` that originates below the command
line, and prefer omission when the intent is to defer. The same spelling
applies to `Option<bool>` fields, which continue to distinguish "supplied
nowhere" from an explicit `false`.

Generated man pages and PowerShell help now print the `--flag[=<BOOL>]`
form, and the documentation IR reports `CliMetadata.value_optional = true`
alongside the `BOOL` value name and the `true`/`false` possible values. The
IR version advances to `1.2`; consumers that pin the version should accept
the new value. Older documents remain readable, because the field carries a
serde default.

## Expect explicit CLI values to win

An explicit command-line value now wins even when it equals the struct
default. This corrects a layering defect: the generated guard compared the
whole parsed CLI object against the whole defaults object and skipped the CLI
layer when the two matched. A configuration whose command line restated its
own default therefore discarded that layer, and a lower-precedence file or
environment value silently won instead.

The defect was invisible in most applications because any other differing
field made the two objects unequal. It surfaced with single-field
configurations, and generally whenever the user restated a default:

```plaintext
# ACME_PORT=9000, struct default port = 8080
$ app --port 8080
port = 9000        # before: the explicit value was discarded
$ app --port 8080
port = 8080        # after: the command line wins
```

No migration is required. Applications that came to rely on the discarded
value can restore the previous outcome by omitting the argument, which still
defers to the file or environment layer and is the supported way to express
that intent.

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

## Upgrade checklist

- [ ] Update every OrthoConfig crate requirement to v0.10.0.
- [ ] Run tests with the feature combinations shipped by the application.
- [ ] Accept documentation IR version `1.2` in any consumer that pins it.
- [ ] Review generated help text that asserted a bare `--flag` spelling.
- [ ] Add `--flag=false` to any workflow that needs to clear a configured
  `true`; leave other invocations unchanged.
- [ ] Audit single-field configurations for command lines that restate their
  defaults, which now take effect instead of being discarded.
- [ ] Adopt injected environment sources and the Cargo helper only where the
  application needs them.

[users-guide-policy]: users-guide.md#agent-native-policy-checking
