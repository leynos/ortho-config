# OrthoConfig

[![Ask DeepWiki][dw]][dw-url] [![Crates.io Version][cr]][cr-url]

[cr]: https://img.shields.io/crates/v/ortho_config "crates.io package"
[cr-url]: https://crates.io/crates/ortho_config
[dw]: https://deepwiki.com/badge.svg
[dw-url]: https://deepwiki.com/leynos/ortho-config

OrthoConfig turns one Rust struct into a complete configuration interface for
your command-line application. Define each setting once, then accept it from
command-line arguments, environment variables, or configuration files with a
predictable precedence order.

It gives an application:

- idiomatic names for every source, such as `--log-level`, `APP_LOG_LEVEL`, and
  `log_level`;
- typed parsing and validation through `clap` and Serde;
- configuration-file discovery without application-specific glue;
- man page and Windows PowerShell help generation with `cargo-orthohelp`;
- generated human and agent-oriented configuration documentation; and
- opt-in localization, tracing, and metrics for production CLIs.

## Get to Hello World

Add OrthoConfig and Serde:

<!-- tested-example: readme-install -->
```toml
[dependencies]
ortho_config = "0.9.0"
serde = { version = "1.0", features = ["derive"] }
```

Define the settings your application needs and call `load()`:

<!-- tested-example: readme-main -->
```rust
use ortho_config::{OrthoConfig, OrthoResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "HELLO_")]
struct Config {
    #[ortho_config(default = String::from("127.0.0.1"))]
    host: String,

    #[ortho_config(default = 8080)]
    port: u16,
}

fn main() -> OrthoResult<()> {
    let config = Config::load()?;
    println!("Listening on {}:{}", config.host, config.port);
    Ok(())
}
```

The same fields are now available as CLI options and `HELLO_HOST` or
`HELLO_PORT` environment variables. Command-line values take precedence:

<!-- tested-example: readme-run -->
```console
$ cargo run -- --host 0.0.0.0 --port 3000
Listening on 0.0.0.0:3000
```

That is the whole integration. OrthoConfig adds the source-specific naming and
merging behaviour around the struct you already use in your application.

## Now and next

**Now:** OrthoConfig provides typed, layered configuration with file
inheritance, collection merging, cross-platform discovery, subcommand support,
Fluent localization, and generated human documentation. More recently, it has
added recursive command metadata, compact agent-context output, and skill
manifest metadata.

**Next:** The immediate work is to validate skill manifests against real
commands. The active roadmap then develops agent-native policy, structured and
atomic `cargo-orthohelp` output, reusable profile and workflow contracts, and a
broader localization lifecycle and derive surface.

See the [completed v0.8.0 roadmap](docs/archive/v0-8-0-roadmap.md) for the
foundation and the [active roadmap](docs/roadmap.md) for the detailed sequence.

## Where to go next

- Follow the [user's guide](docs/users-guide.md) for worked examples covering
  files, custom discovery, subcommands, errors, testing, localization,
  observability, and generated help.
- Upgrading from v0.8? Read the
  [v0.9.0 migration guide](docs/v0-9-0-migration-guide.md) before changing the
  dependency version. It identifies required migrations separately from
  optional improvements.
- Explore the complete [Hello World application](examples/hello_world/) when
  you want a multi-module example with localization and `cargo-orthohelp`.
- Consult the [API documentation](https://docs.rs/ortho_config) for individual
  traits, attributes, and types.
- Use the [design](docs/design.md), [changelog](CHANGELOG.md), and
  [roadmap](docs/roadmap.md) for architecture, released changes, and planned
  work.
- See the [developer's guide](docs/developers-guide.md) to build, test, or
  propose a change.

OrthoConfig is distributed under the [ISC licence](LICENSE).
