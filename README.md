# OrthoConfig

**OrthoConfig** is a Rust configuration management library designed for
simplicity and power, inspired by the flexible configuration mechanisms found
in tools like `esbuild`. It allows your application to seamlessly load
configuration from command-line arguments, environment variables, and
configuration files, all with a clear order of precedence and minimal
boilerplate.

The core principle is **orthographic option naming**: a single field in your
Rust configuration struct can be set through idiomatic naming conventions from
various sources (e.g., `--my-option` for CLI, `MY_APP_MY_OPTION` for
environment variables, `my_option` in a TOML file) without requiring extensive
manual aliasing.

## Core Features

- **Layered Configuration:** Sources configuration from multiple places with a
  well-defined precedence:
  1. Command-Line Arguments (Highest)
  2. Environment Variables
  3. Configuration File (e.g., `config.toml`)
  4. Application-Defined Defaults (Lowest)
- **Orthographic Option Naming:** Automatically maps diverse external naming
  conventions (kebab-case, UPPER_SNAKE_CASE, etc.) to your Rust struct's
  snake_case fields.
- **Type-Safe Deserialization:** Uses `serde` to deserialize configuration into
  your strongly-typed Rust structs.
- **Easy to Use:** A simple `#[derive(OrthoConfig)]` macro gets you started
  quickly.
- **Customizable:** Field-level attributes allow fine-grained control over
  naming, defaults, and merging behavior.
- **Nested Configuration:** Naturally supports nested structs for organized
  configuration.
- **Sensible Defaults:** Aims for intuitive behavior out-of-the-box.

## Quick Start

<!-- markdownlint-disable MD029 -->

1. **Add `OrthoConfig` to your `Cargo.toml`:**

```toml
[dependencies]
ortho_config = "0.3.0" # Replace with the latest version
serde = { version = "1.0", features = ["derive"] }
```

2. **Define your configuration struct:**

```rust
use ortho_config::{OrthoConfig, OrthoError};
use serde::{Deserialize, Serialize}; // Required for OrthoConfig derive

#[derive(Debug, Clone, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "DB")] // Nested prefix: e.g., APP_DB_URL
struct DatabaseConfig {
    // Automatically maps to:
    // CLI: --database-url <value> (if clap flattens) or via file/env
    // Env: APP_DB_URL=<value>
    // File: [database] url = <value>
    url: String,

    #[ortho_config(default = 5)]
    pool_size: Option<u32>, // Optional value, defaults to `Some(5)`
}

impl std::str::FromStr for DatabaseConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, ',');
        let url = parts
            .next()
            .ok_or_else(|| "missing url".to_string())?
            .to_string();
        let pool_size = parts
            .next()
            .and_then(|p| p.parse::<u32>().ok());
        Ok(DatabaseConfig { url, pool_size })
    }
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP")] // Prefix for environment variables (e.g., APP_LOG_LEVEL)
struct AppConfig {
    log_level: String,

    // Automatically maps to:
    // CLI: --port <value>
    // Env: APP_PORT=<value>
    // File: port = <value>
    #[ortho_config(default = 8080)]
    port: u16,

    #[ortho_config(merge_strategy = "append")] // Default for Vec<T> is append
    features: Vec<String>,

    // Nested configuration
    database: DatabaseConfig,

    #[ortho_config(cli_short = 'v')] // Enable a short flag: -v
    verbose: bool, // Defaults to false if not specified
}

fn main() -> Result<(), OrthoError> {
    let config = AppConfig::load()?; // Load configuration

    println!("Loaded configuration: {:#?}", config);

    if config.verbose {
        println!("Verbose mode enabled!");
    }
    println!("Log level: {}", config.log_level);
    println!("Listening on port: {}", config.port);
    println!("Enabled features: {:?}", config.features);
    println!("Database URL: {}", config.database.url);
    println!("Database pool size: {:?}", config.database.pool_size);

    Ok(())
}
```

3. **Run your application:**

   - With CLI arguments:
     `cargo run -- --log-level debug --port 3000 -v --features
     extra_cli_feature`
   - With environment variables:
     `APP_LOG_LEVEL=warn APP_PORT=4000`
     `APP_DB_URL="postgres://localhost/mydb"`
     `APP_FEATURES="env_feat1,env_feat2" cargo run`

<!-- markdownlint-enable MD029 -->
- With a `.app.toml` file (assuming `#[ortho_config(prefix = "APP_")]`;
     adjust for your prefix):

     ```toml
     # .app.toml
     log_level = "file_level" port = 5000 features = ["file_feat_a",
     "file_feat_b"]

     [database] url = "mysql://localhost/prod_db" pool_size = 10 ```

## Configuration Sources and Precedence

OrthoConfig loads configuration from the following sources, with later sources
overriding earlier ones:

1. **Application-Defined Defaults:** Specified using
   `#[ortho_config(default =...)]` or `Option<T>` fields (which default to
   `None`).
2. **Configuration File:** Resolved in this order:
   1. `--config-path` CLI option
   2. `[PREFIX]CONFIG_PATH` environment variable
   3. `.<prefix>.toml` in the current directory
   4. `.<prefix>.toml` in the user's home directory (where `<prefix>` comes from
      `#[ortho_config(prefix = "...")]` and defaults to `config`). JSON5 and
      YAML support are feature gated.
3. **Environment Variables:** Variables prefixed with the string specified in
   `#[ortho_config(prefix = "...")]` (e.g., `APP_`). Nested struct fields are
   typically accessed using double underscores (e.g., `APP_DATABASE__URL` if
   `prefix = "APP"` on `AppConfig` and no prefix on `DatabaseConfig`, or
   `APP_DB_URL` with `#` on `DatabaseConfig`).
4. **Command-Line Arguments:** Parsed using `clap` conventions. Long flags are
   derived from field names (e.g., `my_field` becomes `--my-field`).

### File Format Support

TOML parsing is enabled by default. Enable the `json5` and `yaml` features to
support additional formats:

```toml
[dependencies]
ortho_config = { version = "0.3.0", features = ["json5", "yaml"] }
```

The file loader selects the parser based on the extension (`.toml`, `.json`,
`.json5`, `.yaml`, `.yml`). When the `json5` feature is active, both `.json`
and `.json5` files are parsed using the JSON5 format. Standard JSON conforms to
JSON5 syntax, so existing `.json` files continue to work. Without this feature
enabled, attempting to load a `.json` or `.json5` file will result in an error.
When the `yaml` feature is enabled, `.yaml` and `.yml` files are also
discovered and parsed. Without this feature, those extensions are ignored
during path discovery.

JSON5 extends JSON with conveniences such as comments, trailing commas,
single-quoted strings, and unquoted keys.

## Orthographic Naming

A key goal of OrthoConfig is to make configuration natural from any source. A
field like `max_connections: u32` in your Rust struct will, by default, be
configurable via:

- CLI: `--max-connections <value>`
- Environment (assuming `#[ortho_config(prefix = "MYAPP")]`):
  `MYAPP_MAX_CONNECTIONS=<value>`
- TOML file: `max_connections = <value>`
- JSON5 file: `max_connections` or `maxConnections` (configurable)

You can customize these mappings using `#[ortho_config(...)]` attributes.

## Field Attributes `#[ortho_config(...)]`

Customize behavior for each field:

- `#[ortho_config(default =...)]`: Sets a default value. Can be a literal (e.g.,
  `"debug"`, `123`, `true`) or a path to a function (e.g.,
  `default = "my_default_fn"`).
- `#[ortho_config(cli_long = "custom-name")]`: Specifies a custom long CLI flag
  (e.g., `--custom-name`).
- `#[ortho_config(cli_short = 'c')]`: Specifies a short CLI flag (e.g., `-c`).
- `#`: Specifies a custom environment variable suffix (appended to the
  struct-level prefix).
- `#[ortho_config(file_key = "customKey")]`: Specifies a custom key name for
  configuration files.
- `#[ortho_config(merge_strategy = "append")]`: For `Vec<T>` fields, defines how
  values from different sources are combined. Defaults to `"append"`.
- `#[ortho_config(flatten)]`: Similar to `serde(flatten)`, useful for inlining
  fields from a nested struct into the parent's namespace for CLI or
  environment variables.

## Subcommand Configuration

Applications using `clap` subcommands can keep per-command defaults in a
dedicated `cmds` namespace. The helper `load_and_merge_subcommand_for` reads
these values from configuration files and environment variables using the
struct's `prefix()` function (which defaults to an empty string) and merges
them underneath the CLI arguments.

```rust
use clap::Parser;
use serde::Deserialize;
use ortho_config::{load_and_merge_subcommand_for, OrthoConfig};

#[derive(Parser, Deserialize, Default, Debug, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
pub struct AddUserArgs {
    #[arg(long)]
    username: Option<String>,
    #[arg(long)]
    admin: Option<bool>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = AddUserArgs::parse();

    // Reads `[cmds.add-user]` sections and `APP_CMDS_ADD_USER_*` variables then merges with CLI
    let args = load_and_merge_subcommand_for::<AddUserArgs>(&cli)?;

    println!("Final args: {args:?}");
    Ok(())
}
```

Configuration file example:

```toml
[cmds.add-user]
username = "file_user"
admin = true
```

Environment variables override file values using the pattern
`<PREFIX>CMDS_<SUBCOMMAND>_`:

```bash
APP_CMDS_ADD_USER_USERNAME=env_user
APP_CMDS_ADD_USER_ADMIN=false
```

### Dispatching Subcommands

Subcommands can be executed with defaults applied using
[`clap-dispatch`](https://docs.rs/clap-dispatch):

```rust
use clap::Parser;
use clap_dispatch::clap_dispatch;
use serde::Deserialize;
use ortho_config::load_and_merge_subcommand_for;

#[derive(Parser, Deserialize, Default, Debug)]
pub struct ListItemsArgs {
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    all: Option<bool>,
}

trait Run {
    fn run(&self, db_url: &str) -> Result<(), String>;
}

impl Run for AddUserArgs { /* your logic here */ }
impl Run for ListItemsArgs { /* your logic here */ }

#[derive(Parser)]
#[command(name = "registry-ctl")]
#[clap_dispatch(fn run(self, db_url: &str) -> Result<(), String>)]
enum Commands {
    AddUser(AddUserArgs),
    ListItems(ListItemsArgs),
}

fn main() -> Result<(), String> {
    let cli = Commands::parse();
    let db_url = "postgres://user:pass@localhost/registry";

    // merge per-command defaults
    let cmd = match cli {
        Commands::AddUser(args) => {
            Commands::AddUser(load_and_merge_subcommand_for::<AddUserArgs>(&args)?)
        }
        Commands::ListItems(args) => {
            Commands::ListItems(load_and_merge_subcommand_for::<ListItemsArgs>(&args)?)
        }
    };

    cmd.run(db_url)
}
```

## Why OrthoConfig?

- **Reduced Boilerplate:** Define your configuration schema once and let
  OrthoConfig handle the multi-source loading and mapping.
- **Developer Ergonomics:** Intuitive mapping from external sources to your Rust
  code.
- **Flexibility:** Users of your application can configure it in the way that
  best suits their environment (CLI for quick overrides, env vars for CI/CD,
  files for persistent settings).
- **Clear Precedence:** Predictable configuration resolution.

## Migrating from 0.1 to 0.2

Version 0.2 introduces a small API refinement:

- `load_subcommand_config_for` now only loads default values from files and
  environment variables. Use
  [`load_and_merge_subcommand_for`](#subcommand-configuration) to merge these
  defaults with CLI arguments.
- Types deriving `OrthoConfig` expose an associated `prefix()` function. Use
  this if you need the configured prefix directly.

Update your `Cargo.toml` to depend on `ortho_config = "0.2"` and adjust code to
call `load_and_merge_subcommand_for` instead of manually merging defaults.

## Contributing

Contributions are welcome! Please feel free to submit issues, fork the
repository, and send pull requests.

## License

OrthoConfig is distributed under the terms of both the ISC license.

See LICENSE for details.
