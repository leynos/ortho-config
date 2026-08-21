# Migration guide: v0.9.0 to v0.10.0

## Who should read this

Read this guide when upgrading an application or library from OrthoConfig
v0.9.0 to v0.10.0. This release adds `ortho_config::cargo::external_subcommand`
for callers that build a `clap::Command` by hand and expose it through a Cargo
external subcommand. Applications that do not provide Cargo external
subcommands require no changes.

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
