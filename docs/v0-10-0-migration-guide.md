# Migration guide: v0.9.0 to v0.10.0

## Who should read this

Read this guide when adopting source-aware environment merging and layer
composition, parser-faithful clap string defaults, agent-native behaviour
metadata, the optional-value boolean flags, optional profile overlays, or the
Cargo external-subcommand helper. Existing callers can upgrade without changing
their loading code: process-backed behaviour remains the default, profile
support is opt-in, and applications that do not use the new opt-in metadata,
profiles, or the Cargo helper require no changes.

## Impact at a glance

| Change                                  | Affects                                | Required?        |
| --------------------------------------- | -------------------------------------- | ---------------- |
| Optional-value boolean flags            | Commands with `bool` CLI fields        | No; additive     |
| Explicit CLI values always win          | Commands that restate a struct default | No; a defect fix |
| Documentation IR version `1.1` to `2.0` | Consumers pinning the IR version       | No; accept `2.0` |
| Injected environment sources            | Tests needing hermetic environments    | No; opt-in       |
| Parser-faithful clap string defaults    | Fields using `cli_default_as_absent`   | No; opt-in       |
| Cargo external-subcommand helper        | Hand-built Cargo subcommands           | No; additive     |

## Adopt the opt-in agent-native policy check

Run `cargo orthohelp --check-agent-native --package <package>` to write a
policy report for a package. The `--package <package>` argument is required
when the workspace has no root package, such as a virtual workspace. Configure
the policy in the `[package.metadata.ortho_config.policy]` metadata table. The
`off`, `warn`, and `deny` modes select disabled, advisory, and failing policy
behaviour. See the
[agent-native policy section in the user's guide][users-guide-policy] for the
report shape, exceptions, and command-line override.

The policy check writes `policy-report.json` and emits one compact JSON
document followed by a newline on standard output. With `--check-agent-native`
alone, this is a report-only operation. When combined with an explicit
generator format, the report remains the only JSON document on standard output;
successful checks continue generation and preserve the selected format's
artefacts, including `agent-context.json` for `--format agent-context` and
`--format all`. A deny-mode violation emits and retains the policy report, then
fails before generator artefacts are created. Without the policy check,
`--format agent-context` and `--format all` continue to emit AgentContext JSON
on standard output as well as writing `agent-context.json`.

## Load files from more than one scope

Nothing changes by default: discovery still stops at the first file that loads,
which is what `AutomaticMode::FirstWins` means, and that remains the default.

Opting into `AutomaticMode::StackScopes` makes every applicable file that loads
in each requested scope contribute a layer, rather than only the first. Scopes
apply in the order named, so a later scope overrides an earlier one. Within a
scope the most-preferred location still wins, preserving the first-wins result.

Two ways to opt in: the `automatic_mode = "stack_scopes"` derive key, and
`ConfigFilePolicy::from_builder(...).automatic_mode(...)` on a hand-built
policy. Both live under the same `discovery(...)` attribute and policy API
described in [the user's guide][users-guide-scoped-discovery].

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

## Compose layers from injected sources

`Config::compose_layers_from_iter_with_sources` accepts separate
`SharedEnvSource` and `SharedScanEnvSource` arguments for discovery lookups and
merge-variable scanning. It returns the composed layers without deserializing a
configuration. Use `load_from_iter_with_sources` when the resolved
configuration value is required. Existing composition entry points remain
process-backed.

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

## Inject a subcommand file discovery source

The process-backed loaders are the default and require no migration: they take
their named lookups from the live process environment and use the current
directory as the local base. Use `SubcommandFileContext` with the
`_with_sources_at` loaders when the search inputs must be supplied rather than
inherited.

A context pairs an explicit local base path with a lookup-only `EnvSource`; the
source answers named lookups for `HOME`, `USERPROFILE` (non-Unix),
`XDG_CONFIG_HOME`, and `XDG_CONFIG_DIRS`, and on non-Unix targets it may also
supply the native platform configuration directory. The merge source is
deliberately separate: `SharedScanEnvSource` enumerates the environment layer,
while the `EnvSource` inside the context only performs lookups. See
[Choose the file-discovery source][users-guide-discovery] in the user's guide
for the candidate order and a complete example.

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

## Declare agent-native command behaviour

Add a struct-level `behaviour(...)` declaration when an agent needs explicit
interaction, mutation, bypass, or dry-run metadata:

```rust
#[derive(ortho_config::OrthoConfig)]
#[ortho_config(
    prefix = "APP",
    behaviour(
        interaction = "interactive",
        mutation = "delete",
        bypass = "--force",
        dry_run = "--dry-run"
    )
)]
struct PurgeArgs {
    // ...
}
```

The declaration is carried into documentation IR version 1.2 and then into
agent-context schema v1. The IR change is additive: a 1.1 reader ignores the new
`behaviour` block, while a 1.2 reader accepts a 1.1 document with the block
absent. `bypass_flag` and `dry_run_flag` remain nullable agent-context fields;
when interaction or mutation is not declared, the corresponding values remain
`unknown`. No interaction or mutation semantics are inferred from command names
or flags.

Opt into policy checking with `--check-agent-native`. The enforcement mode
comes from `[package.metadata.ortho_config.policy]` and defaults to `off`; a
`--policy-mode <off|warn|deny>` override applies to the report. The check writes
`policy-report.json` atomically to the output directory and a human-readable
summary to stderr. Warnings are non-fatal, while deny findings report a policy
violation and exit non-zero after the report has been written.

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

### Review documentation-metadata consumers

The bridge IR's `DocMetadata` gains a `profiles` field, and it is an additive
change to the *wire* contract but a breaking one to the *Rust* contract. Both
copies of the type gain it:

- `ortho_config::docs::DocMetadata`, the library-side type.
- `cargo_orthohelp::schema::DocMetadata`, the bridge-side mirror.

The field is `#[serde(default)]`, so JSON written against the previous release
still deserializes and a struct that did not opt in still omits it. An external
crate that builds the type by struct literal is different: a defaulted field is
still required in a literal, so every literal needs the field named. Adding
`profiles: None` preserves the previous behaviour, and the derive only ever
produces `Some` for a struct declared with `#[ortho_config(profiles)]`.

This release also adds the `DocProfilesMeta` type that the field carries. It
holds the selector's `flag` and `env_var` as plain strings, so a consumer
reading the IR needs no new dependency to interpret it.

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

## Review boolean CLI flag spellings

Generated boolean flags accept an optional `=<BOOL>` value. The bare flag and
omitted flag keep their previous meanings, so the change is additive for every
caller. Commands that pass a space-separated value are the exception:
`--flag false` is still rejected, with the same
`unexpected argument 'false' found` error as before, because the value must
follow an `=`. That spelling was never a way to express `false` and still is
not; use `--flag=false`.

Before, a boolean flag was a presence-only switch. It could only express
`true`, so a `true` from a configuration file or environment variable could not
be cleared from the command line. `--excited=false` was rejected outright
rather than silently ignored:

```plaintext
# config.toml sets excited = true
$ app --excited=false
error: unexpected value 'false' for '--excited' found; no more were expected

Usage: app [OPTIONS]

For more information, try '--help'.
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

Nothing needs changing to keep the old behaviour. Adopt the explicit form where
a user needs to override a `true` that originates below the command line, and
prefer omission when the intent is to defer. The same spelling applies to
`Option<bool>` fields, which continue to distinguish "supplied nowhere" from an
explicit `false`.

Generated man pages now print the `--flag[=BOOL]` form, with the bracketed
placeholder italicized and joined to the flag so it cannot be misread as the
invalid `--flag =BOOL` spelling. PowerShell help explains the optional value in
prose instead. The documentation IR reports `CliMetadata.value_optional = true`
alongside the `BOOL` value name and the `true`/`false` possible values. The IR
version advanced to 2.0 alongside the identifier-value change; consumers that
pin the version should read `ortho_config::docs::ORTHO_DOCS_IR_VERSION` rather
than hard-code either value. Older documents remain readable, because the field
carries a serde default.

## Expect explicit CLI values to win

An explicit command-line value now wins even when it equals the struct default.
This corrects a layering defect: the generated guard compared the whole parsed
CLI object against the whole defaults object and skipped the CLI layer when the
two matched. A configuration whose command line restated its own default
therefore discarded that layer, and a lower-precedence file or environment
value silently won instead.

The defect was invisible in most applications because any other differing field
made the two objects unequal. It surfaced whenever the command line covered
every defaulted field with a value equal to that field's default; a
single-field configuration reaches that state as soon as the user restates the
one default:

```plaintext
# ACME_PORT=9000, struct default port = 8080
$ app --port 8080
port = 9000        # before: the explicit value was discarded
$ app --port 8080
port = 8080        # after: the command line wins
```

No migration is required. Applications that came to rely on the discarded value
can restore the previous outcome by omitting the argument, which still defers
to the file or environment layer and is the supported way to express that
intent.

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

## Adopt derived CLI localization

`#[derive(OrthoConfig)]` now implements `OrthoConfigLocalization` for the
derived configuration. Use `CommandLine::LOCALIZATION_BASE` and the generated
command and argument identifiers when referring to Fluent messages. Set
`#[ortho_config(localization_base = "acme.cli")]` to choose the dotted
catalogue base; when omitted, the application name supplies the default.

The derive normalizes dotted base and argument segments to Fluent-safe,
hyphen-joined identifiers. Normalized argument-ID collisions within one derived
struct are compile-time errors. Update catalogues to use the generated IDs and
retain any hand-built command-tree validation because those trees keep the
runtime panic contract.

Re-spell catalogues carefully: the separators are load-bearing. Roots and clap
flag-style names are hyphen-joined, but an argument's own segment keeps the raw
Rust field name, so an underscored field yields `is_dry_run`, not `is-dry-run`.
The fixed suffixes `long_help` and `value_name` are underscore-separated for
the same reason. `normalize_segment` preserves both `-` and `_`, so the two
spellings normalize to *distinct* keys and a respelled entry becomes
unreachable — it translates cleanly and is simply never looked up.

Documentation IR version rises from `1.1` to `2.0` because the identifier
*values* change meaning for consumers keyed on the old shapes. The old dotted
`{app}.fields.{field}.help` form becomes the hyphenated
`{mounted-path}-args-{field}-help`, and nested command defaults now extend the
mounted command path rather than a standalone base. Read
`ortho_config::docs::ORTHO_DOCS_IR_VERSION` instead of hard-coding the version,
and re-key any Fluent catalogue whose entries used the dotted forms.

Identifier artefact emission is opt-in. Run
`cargo clean -p <package> && ORTHO_CONFIG_EMIT_IDENTIFIERS=1 cargo build -p <package>`
to force a fresh expansion, then inspect
`${OUT_DIR}/ortho-config/cli-identifiers.json`. Cargo does not fingerprint
proc-macro environment reads, so a warm build can leave an existing artefact
unchanged. The artefact is a standalone declaration inventory; mounted command
tree identifiers remain owned by the path-aware documentation IR. If output
exceeds 1 MiB, read the `cli-identifiers.<n>.json` parts listed by the index.

## No migration required for other users

Profile support and the Cargo helper are additive. Existing configuration
loading, derive usage, and subcommand merging continue unchanged. Add the
`profiles` attribute only when an application wants profile overlays, and add
the helper only when adopting the Cargo external-subcommand entry-point shape.

Two Rust-level changes affect consumers that build the affected types by struct
literal; neither changes the serialized form. `AgentContext.profiles` is
retyped, and `DocMetadata` gains a `profiles` field in both its library and
bridge copies. Build the first with the `ProfilesDeclaration` constructors and
name the second as `profiles: None`. Both are described above under
[Review agent-context consumers](#review-agent-context-consumers) and
[Review documentation-metadata consumers](#review-documentation-metadata-consumers).

## Upgrade checklist

- [ ] Update every OrthoConfig crate requirement to v0.10.0.
- [ ] Run tests with the feature combinations shipped by the application.
- [ ] Accept documentation IR version `2.0` in any consumer that pins it.
- [ ] Review generated help text that asserted a bare `--flag` spelling.
- [ ] Add `--flag=false` to any workflow that needs to clear a configured
  `true`; leave other invocations unchanged.
- [ ] Audit command lines that restate defaults across every defaulted field,
  which now take effect instead of being discarded. A single-field
  configuration reaches this state as soon as its one field is restated.
- [ ] Adopt injected environment sources and the Cargo helper only where the
  application needs them.

[users-guide-policy]: users-guide.md#agent-native-policy-checking
[users-guide-discovery]: users-guide.md#choose-the-file-discovery-source
[users-guide-scoped-discovery]: users-guide.md#control-which-configuration-files-become-layers
