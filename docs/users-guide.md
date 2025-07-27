# OrthoConfig User's Guide

`OrthoConfig` is a Rust library that unifies command‑line arguments,
environment variables and configuration files into a single, strongly typed
configuration struct. It is inspired by tools such as `esbuild` and is designed
to minimize boiler‑plate. The library uses `serde` for deserialization and
`clap` for argument parsing, while `figment` provides layered configuration
management. This guide covers the functionality currently implemented in the
repository.

## Core concepts and motivation

Rust projects often wire together `clap` for CLI parsing, `serde` for
de/serialisation, and ad‑hoc code for loading `*.toml` files or reading
environment variables. Mapping between different naming conventions (kebab‑case
flags, `UPPER_SNAKE_CASE` environment variables, and `snake_case` struct
fields) can be tedious. `OrthoConfig` addresses these problems by letting
developers describe their configuration once and then automatically loading
values from multiple sources. The core features are:

- **Layered configuration** – Configuration values can come from application
  defaults, configuration files, environment variables and command‑line
  arguments. Later sources override earlier ones. Command‑line arguments have
  the highest precedence and defaults the lowest.

- **Orthographic naming** – A single field in a Rust struct is automatically
  mapped to a CLI flag (kebab‑case), an environment variable (upper snake case
  with a prefix), and a file key (snake case). This removes the need for manual
  aliasing.

- **Type‑safe deserialization** – Values are deserialized into strongly typed
  Rust structs using `serde`.

- **Easy adoption** – A procedural macro `#[derive(OrthoConfig)]` adds the
  necessary code. Developers only need to derive `serde` and `clap` traits on
  their configuration struct and call a generated method to load the
  configuration.

- **Customizable behaviour** – Attributes such as `default`, `cli_long`,
  `cli_short` and `merge_strategy` provide fine‑grained control over naming and
  merging behaviour.

## Installation and dependencies

Add `ortho_config` as a dependency in `Cargo.toml` along with `serde`:

```toml
[dependencies]
ortho_config = "0.3.0"            # replace with the latest version
serde = { version = "1.0", features = ["derive"] }
clap = { version = "4", features = ["derive"] }    # required for CLI support
```

By default, only TOML configuration files are supported. To enable JSON5
(`.json` and `.json5`) and YAML (`.yaml` and `.yml`) support, enable the
corresponding cargo features:

```toml
[dependencies]
ortho_config = { version = "0.3.0", features = ["json5", "yaml"] }
```

Enabling the `json5` feature causes both `.json` and `.json5` files to be
parsed using the JSON5 format. Without this feature, attempts to load JSON
files will fail. The `yaml` feature similarly enables YAML file discovery and
parsing.

## Defining configuration structures

A configuration is represented by a plain Rust struct. To take advantage of
`OrthoConfig`, derive the following traits:

- `serde::Deserialize` and `serde::Serialize` – required for deserialising
  values and merging overrides.

- `clap::Parser` – required for generating CLI parsing code. Fields may be
  annotated with standard `clap` attributes such as `#[arg(long)]` or
  `#[arg(short)]`.

- `OrthoConfig` – provided by the library. This derive macro generates the code
  to load and merge configuration from multiple sources.

Optionally, the struct can include a `#[ortho_config(prefix = "PREFIX")]`
attribute. The prefix sets a common string for environment variables and
configuration file names. Trailing underscores are trimmed and the prefix is
lower‑cased when used to form file names. For example, a prefix of `APP_`
results in environment variables like `APP_PORT` and file names such as
`.app.toml`.

### Field‑level attributes

Field attributes modify how a field is sourced or merged:

| Attribute                   | Behaviour                                                                                                                                                                     |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `default = expr`            | Supplies a default value when no source provides one. The expression can be a literal or a function path.                                                                     |
| `cli_long = "name"`         | Overrides the automatically generated long CLI flag (kebab‑case).                                                                                                             |
| `cli_short = 'c'`           | Adds a single‑letter short flag for the field.                                                                                                                                |
| `merge_strategy = "append"` | For `Vec<T>` fields, specifies that values from different sources should be concatenated. This is currently the only supported strategy and is the default for vector fields. |

Unrecognized keys are ignored by the derive macro for forwards compatibility.
Unknown keys will therefore silently do nothing. Developers who require
stricter validation may add manual `compile_error!` guards.

### Example configuration struct

The following example illustrates many of these features:

```rust
use ortho_config::{OrthoConfig, OrthoError};
use serde::{Deserialize, Serialize};
use clap::Parser;

#[derive(Debug, Clone, Deserialize, Serialize, OrthoConfig, Parser)]
#[ortho_config(prefix = "APP")]                // environment variables start with APP_
struct AppConfig {
    /// Logging verbosity
    #[arg(long)]
    log_level: String,

    /// Port to bind on – defaults to 8080 when unspecified
    #[arg(long)]
    #[ortho_config(default = 8080)]
    port: u16,

    /// Optional list of features.  Values from files, environment and CLI are appended.
    #[arg(long)]
    #[ortho_config(merge_strategy = "append")]
    features: Vec<String>,

    /// Nested configuration for the database.  A separate prefix is used to avoid ambiguity.
    #[serde(flatten)]
    database: DatabaseConfig,

    /// Enable verbose output; also available as -v via cli_short
    #[arg(long)]
    #[ortho_config(cli_short = 'v')]
    verbose: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, OrthoConfig, Parser)]
#[ortho_config(prefix = "DB")]               // used in conjunction with APP_ prefix to form APP_DB_URL
struct DatabaseConfig {
    #[arg(long)]
    url: String,

    #[arg(long)]
    #[ortho_config(default = 5)]
    pool_size: Option<u32>,
}

fn main() -> Result<(), OrthoError> {
    // Parse CLI arguments and merge with defaults, file and environment
    let cli_args = AppConfig::parse();
    let config = cli_args.load_and_merge()?;
    println!("Final config: {:#?}", config);
    Ok(())
}
```

In this example the `AppConfig` struct uses a prefix of `APP`. The
`DatabaseConfig` struct has its own prefix `DB`, resulting in environment
variables such as `APP_DB_URL`. The `features` field is a `Vec<String>` and
will accumulate values from multiple sources rather than overwriting them.

## Loading configuration and precedence rules

### The `load_and_merge()` method

The `OrthoConfig` derive macro generates a method
`load_and_merge(&self) -> Result<Self, OrthoError>`. It takes the struct
populated by `clap` parsing and returns a fully populated configuration
instance. Internally, it performs the following steps:

1. Builds a `figment` configuration profile. A defaults provider constructed
   from the `#[ortho_config(default = …)]` attributes is added first.

2. Attempts to load a configuration file. Candidate file paths are searched in
   the following order:

   1. A `--config-path` CLI argument. A hidden option is generated
      automatically by the derive macro; if the user defines a `config_path`
      field in their struct then that will override the hidden option.
      Alternatively the environment variable `PREFIXCONFIG_PATH` (or
      `CONFIG_PATH` if no prefix is set) can specify an explicit file.

   1. A dotfile named `.config.toml` or `.<prefix>.toml` in the current working
      directory.

   1. A dotfile of the same name in the user's home directory.

   1. On Unix‑like systems, the XDG configuration directory (e.g.
      `~/.config/app/config.toml`) is searched using the `xdg` crate; on
      Windows, the `%APPDATA%` and `%LOCALAPPDATA%` directories are checked.

   1. If the `json5` or `yaml` features are enabled, files with `.json`,
      `.json5`, `.yaml` or `.yml` extensions are also considered in these
      locations.

3. Adds an environment provider using the prefix specified on the struct. Keys
   are upper‑cased and nested fields use double underscores (`__`) to separate
   components.

4. Adds a provider containing the CLI values (captured as `Option<T>` fields)
   as the final layer.

5. Merges vector fields according to the `merge_strategy` (currently only
   `append`) so that lists of values from lower precedence sources are extended
   with values from higher precedence ones.

6. Attempts to extract the merged configuration into the concrete struct. On
   success it returns the completed configuration; otherwise an `OrthoError` is
   returned.

### Source precedence

Values are loaded from each layer in a specific order. Later layers override
earlier ones. The precedence, from lowest to highest, is:

1. **Application‑defined defaults** – values provided via `default` attributes
   or `Option<T>` fields are considered defaults.

2. **Configuration file** – values from a TOML (or JSON5/YAML) file loaded from
   one of the paths listed above.

3. **Environment variables** – variables prefixed with the struct's `prefix`
   (e.g. `APP_PORT`, `APP_DATABASE__URL`) override file values.

4. **Command‑line arguments** – values parsed by `clap` override all other
   sources.

Nested structs are flattened in the environment namespace by joining field
names with double underscores. For example, if `AppConfig` has a nested
`database` field and the prefix is `APP`, then `APP_DATABASE__URL` sets the
`database.url` field. If a nested struct has its own prefix attribute, that
prefix is used for its fields (e.g. `APP_DB_URL`).

### Using defaults and optional fields

Fields of type `Option<T>` are treated as optional values. If no source
provides a value for an `Option<T>` field then it remains `None`. To provide a
default value for a non‑`Option` field or for an `Option<T>` field that should
have an initial value, specify `#[ortho_config(default = expr)]`. This default
acts as the lowest‑precedence source and is overridden by file, environment or
CLI values.

### Environment variable naming

Environment variables are upper‑cased and use underscores. The struct‑level
prefix (if supplied) is prepended without any separator, and nested fields are
separated by double underscores. For the `AppConfig` and `DatabaseConfig`
example above, valid environment variables include `APP_LOG_LEVEL`, `APP_PORT`,
`APP_DATABASE__URL` and `APP_DATABASE__POOL_SIZE`. If the nested struct has its
own prefix (`DB`), then the environment variable becomes `APP_DB_URL`.

## Subcommand configuration

Many CLI applications use `clap` subcommands to perform different operations.
`OrthoConfig` supports per‑subcommand defaults via a dedicated `cmds`
namespace. The helper function `load_and_merge_subcommand_for` loads defaults
for a specific subcommand and merges them beneath the CLI values. The merged
struct is returned as a new instance; the original `cli` struct remains
unchanged.

### How it works

When a struct derives `OrthoConfig`, it also implements the associated
`prefix()` method. This method returns the configured prefix string.
`load_and_merge_subcommand_for(prefix, cli_struct)` uses this prefix to build a
`cmds.<subcommand>` section name for the configuration file and an
`PREFIX_CMDS_SUBCOMMAND_` prefix for environment variables. Configuration is
loaded in the same order as global configuration (defaults → file → environment
→ CLI), but only values in the `[cmds.<subcommand>]` section or environment
variables beginning with `PREFIX_CMDS_<SUBCOMMAND>_` are considered.

### Example

Suppose an application has a `pr` subcommand that accepts a `reference`
argument and a `repo` global option. With `OrthoConfig` the argument structures
might be defined as follows:

```rust
use clap::Parser;
use ortho_config::{OrthoConfig, load_and_merge_subcommand_for};
use serde::{Deserialize, Serialize};

#[derive(Parser, Deserialize, Serialize, Debug, OrthoConfig, Clone, Default)]
#[ortho_config(prefix = "VK")]               // all variables start with VK
pub struct GlobalArgs {
    #[arg(long)]
    pub repo: Option<String>,
}

#[derive(Parser, Deserialize, Serialize, Debug, OrthoConfig, Clone, Default)]
#[ortho_config(prefix = "VK")]               // subcommands share the same prefix
pub struct PrArgs {
    #[arg(required = true)]
    pub reference: Option<String>,            // optional for merging defaults but required on the CLI
}

fn main() -> Result<(), ortho_config::OrthoError> {
    let cli_pr = PrArgs::parse();
    // Merge defaults from [cmds.pr] and VK_CMDS_PR_* over CLI
    let merged_pr = load_and_merge_subcommand_for::<PrArgs>(&cli_pr)?;
    println!("PrArgs after merging: {:#?}", merged_pr);
    Ok(())
}
```

A configuration file might include:

```toml
[cmds.pr]
reference = "https://github.com/leynos/mxd/pull/31"

[cmds.issue]
reference = "https://github.com/leynos/mxd/issues/7"
```

and environment variables could override these defaults:

```bash
VK_CMDS_PR_REFERENCE=https://github.com/owner/repo/pull/42
VK_CMDS_ISSUE_REFERENCE=https://github.com/owner/repo/issues/101
```

Within the `vk` example repository, the global `--repo` option is provided via
the `GlobalArgs` struct. A developer can set this globally using the
environment variable `VK_REPO` without passing `--repo` on every invocation.
Subcommands `pr` and `issue` load their defaults from the `cmds` namespace and
environment variables. If the `reference` field is missing in the defaults, the
tool continues using the CLI value instead of exiting with an error.

### Dispatching with `clap‑dispatch`

The `clap‑dispatch` crate can be combined with `OrthoConfig` to simplify
subcommand execution. Each subcommand struct implements a trait defining the
action to perform. An enum of subcommands is annotated with
`#[clap_dispatch(fn run(...))]`, and the `load_and_merge_subcommand_for`
function can be called on each variant before dispatching. See the
`Subcommand Configuration` section of the `OrthoConfig` [README](../README.md)
for a complete example.

## Error handling

`load_and_merge` and `load_and_merge_subcommand_for` return a
`Result<T, OrthoError>`. `OrthoError` wraps errors from `clap`, file I/O and
`figment`. When configuration cannot be gathered or deserialized, the error
propagates up to the caller. Consumers should handle these errors
appropriately, for example by printing them to stderr and exiting. Future
releases may include improved missing‑value error messages, but currently the
crate simply returns the underlying error.

## Additional notes

- **Vector merging** – For `Vec<T>` fields the default merge strategy is
  `append`, meaning that values from the configuration file appear first, then
  environment variables and finally CLI arguments. The
  `merge_strategy = "append"` attribute can be used for clarity, though it is
  implied.

- **Option&lt;T&gt; fields** – Fields of type `Option<T>` are not treated as
  required. They default to `None` and can be set via any source. Required CLI
  arguments can be represented as `Option<T>` to allow configuration defaults
  while still requiring the CLI to provide a value when defaults are absent;
  see the `vk` example above.

- **Hidden** `--config-path` **argument** – The derive macro inserts a hidden
  `--config-path` option into the CLI to override the configuration file path.
  This option does not appear in help output unless explicitly defined in the
  user struct. The environment variable `PREFIXCONFIG_PATH` provides the same
  functionality[GitHub](https://github.com/leynos/ortho-config/blob/58c8e0bf82d5a69182824d32e9aff8944eb435c1/README.md#L148-L161).

- **Changing naming conventions** – Currently, only the default
  snake/kebab/upper snake mappings are supported. Future versions may introduce
  attributes such as `file_key` or `env` to customise names further.

- **Testing** – Because the CLI and environment variables are merged at
  runtime, integration tests should set environment variables and construct CLI
  argument vectors to exercise the merge logic. The `figment` crate makes it
  easy to inject additional providers when writing unit tests.

## Conclusion

`OrthoConfig` streamlines configuration management in Rust applications. By
defining a single struct and annotating it with a small number of attributes,
developers obtain a full configuration parser that respects CLI arguments,
environment variables and configuration files with predictable precedence.
Subcommand support and integration with `clap‑dispatch` further reduce
boiler‑plate in complex CLI tools. The example `vk` repository demonstrates how
a real application can adopt `OrthoConfig` to handle global options and
subcommand defaults. Contributions to the project are welcome, and the design
documents outline planned improvements such as richer error messages and
support for additional naming strategies.
