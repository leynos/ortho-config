# OrthoConfig

*One Rust struct keeps every configuration source on the straight and narrow.*

[![Ask DeepWiki][dw]][dw-url] [![Crates.io Version][cr]][cr-url]

> **TL;DR:** Derive `OrthoConfig`, call `load()`, and let your users choose
> defaults, configuration files, environment variables, or command-line
> options. OrthoConfig handles the naming, discovery, and precedence.

______________________________________________________________________

## Why OrthoConfig?

Configuration plumbing starts small, then quietly takes over the kitchen.
Every new setting needs a CLI flag, an environment variable, a file key, merge
rules, and useful errors when something goes wrong.

OrthoConfig lets you describe that setting once, in the Rust struct your
application already needs. From there it gives you:

- **less glue:** derive the interface instead of hand-wiring `clap`, Serde, and
  file discovery;
- **familiar choices:** users can reach for a flag, an environment variable, or
  a configuration file;
- **unsurprising overrides:** command-line values beat environment values,
  which beat files and defaults; and
- **one source of truth:** the same metadata can generate human help, agent
  context, man pages, and Windows PowerShell help.

You get to spend more time on what your application does—and less time teaching
four configuration systems to agree.

______________________________________________________________________

## Quick start

From an empty Rust binary to a layered CLI takes one derive and one call.

### Installation

Add OrthoConfig and Serde to `Cargo.toml`:

<!-- tested-example: readme-install -->
```toml
[dependencies]
ortho_config = "0.9.0"
serde = { version = "1.0", features = ["derive"] }
```

The `cargo-orthohelp` documentation tool ships prebuilt binaries for Linux,
macOS, and Windows on x86-64, and for Linux and macOS on aarch64. Install one
without compiling:

<!-- tested-example: readme-binstall -->
```console
cargo binstall --disable-strategies compile cargo-orthohelp
```

Each release archive is published with a `.sha256` sidecar, so a download can
be verified with `sha256sum --check`. `cargo install cargo-orthohelp` remains
available where a source build is preferred.

### Basic usage

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

That is the whole integration. Your application can grow into files,
subcommands, localization, and generated help when it needs them; it does not
have to start there.

______________________________________________________________________

## Features

- **Layered configuration:** combine typed defaults, configuration files,
  environment variables, and command-line arguments predictably.
- **Convention without confinement:** get idiomatic names such as
  `--log-level`, `APP_LOG_LEVEL`, and `log_level`, then customize the public
  names that matter.
- **Practical file support:** discover configuration across platforms, extend
  base files, and choose how collections merge.
- **CLI-shaped configuration:** support subcommands, localized help, and rich
  source-aware errors without building a second settings model.
- **Documentation from code:** generate man pages, Windows PowerShell help,
  human documentation, and compact agent context with `cargo-orthohelp`.
- **Production-friendly instrumentation:** opt into structured tracing and
  low-cardinality metrics while keeping global setup in the application.

______________________________________________________________________

## Now and next

**Now:** the configuration foundation is in place, from layered loading and
file inheritance to subcommands, Fluent localization, generated help, recursive
command metadata, and compact agent context.

**Next:** make agent-driven CLIs harder to misunderstand and safer to operate.
Skill manifests will be checked against the real command tree, while opt-in
policy flags inconsistent vocabulary, missing machine-readable results, unsafe
mutation surfaces, and unbounded list commands. `cargo-orthohelp` will dogfood
those contracts with structured results, actionable errors, and atomic output;
later metadata will describe profiles, delivery targets, and long-running jobs
so agents can reuse predictable workflows instead of inventing integration
glue.

See the [completed v0.8.0 roadmap][archived-roadmap] for the foundation and the
[active roadmap][roadmap] for the detailed sequence.

______________________________________________________________________

## Learn more

- [User's guide][users-guide] — build a real CLI one practical task at a time,
  with worked examples.
- [v0.9.0 migration guide][migration-guide] — separate required migrations
  from improvements that can be adopted when useful.
- [Hello World application][hello-world] — explore a complete,
  multi-module example with localization and generated help.
- [API documentation](https://docs.rs/ortho_config) — look up individual
  traits, attributes, and types.
- [Developer's guide][developers-guide] — build, test, and contribute to
  OrthoConfig.
- [Changelog][changelog] — review released features, fixes, and compatibility
  notes.
- [Design document][design] — understand the architecture and guiding
  decisions.
- [Roadmap][roadmap] — see what has landed and what comes next.

______________________________________________________________________

## Licence

OrthoConfig is distributed under the [ISC licence][licence].

______________________________________________________________________

## Contributing

Found a rough edge, a missing example, or an idea that would make configuration
less of a chore? Contributions are welcome. Start with the
[developer's guide][developers-guide] and the repository's
[contributor guidance][contributor-guidance].

[archived-roadmap]: https://github.com/leynos/ortho-config/blob/main/docs/archive/v0-8-0-roadmap.md
[changelog]: https://github.com/leynos/ortho-config/blob/main/CHANGELOG.md
[contributor-guidance]: https://github.com/leynos/ortho-config/blob/main/AGENTS.md
[cr]: https://img.shields.io/crates/v/ortho_config "crates.io package"
[cr-url]: https://crates.io/crates/ortho_config
[design]: https://github.com/leynos/ortho-config/blob/main/docs/design.md
[developers-guide]: https://github.com/leynos/ortho-config/blob/main/docs/developers-guide.md
[dw]: https://deepwiki.com/badge.svg
[dw-url]: https://deepwiki.com/leynos/ortho-config
[hello-world]: https://github.com/leynos/ortho-config/tree/main/examples/hello_world
[licence]: https://github.com/leynos/ortho-config/blob/main/LICENSE
[migration-guide]: https://github.com/leynos/ortho-config/blob/main/docs/v0-9-0-migration-guide.md
[roadmap]: https://github.com/leynos/ortho-config/blob/main/docs/roadmap.md
[users-guide]: https://github.com/leynos/ortho-config/blob/main/docs/users-guide.md
