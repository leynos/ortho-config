# Gap Analysis: OrthoConfig vs ddlint Configuration Requirements

This document compares OrthoConfig's current capabilities with the
command-line and configuration interface described in the [`ddlint` design
document](https://raw.githubusercontent.com/leynos/ddlint/refs/heads/main/docs/ddlint-design-and-road-map.md).

## Relevant ddlint Requirements

The design describes a `clap` based CLI and a `ddlint.toml` configuration file:

> The primary user interaction with the linter will be through its command-line
> binary, `ddlint`. The CLI will be built using the `clap` crate.
>
> The core commands will be:
>
```bash
ddlint <FILES...>
ddlint --fix <FILES...>
ddlint rules
ddlint explain <RULE_NAME>
```
>
>
> Key flags include `--format <compact|json|rich>`, `--config <PATH>` and
> `--no-ignore`.

The configuration schema includes:

> | Key | Type | Default | Description |
> | --- | --- | --- | --- |
> | extends | String | (none) | A path to a base configuration file. Settings from the current file will override settings from the extended file. |
> | ignore_patterns | Array of Strings | [".git/", "build/", "target/"] | Patterns of files and directories to exclude from linting. |
> | [rules] | Table | (empty) | Location for configuring rule severities and options. |
> | [rules].`<rule-name>` | String | (rule default) | Sets the severity for a rule (`allow`, `warn`, or `error`). |
> | [rules.consistent-casing] | Table | { level = "allow", relation_style = "PascalCase" } | Example of a rule with options. |

## Current OrthoConfig Features

OrthoConfig layers configuration sources in this order:

```text
## Configuration Sources and Precedence

OrthoConfig loads configuration from the following sources, with later sources
overriding earlier ones:

1. **Application-Defined Defaults:** Specified using
   `#[ortho_config(default =...)]` or `Option<T>` fields (which default to
   `None`).
1. **Configuration File:** Resolved in this order:
   1. `--config-path` CLI option
   1. `[PREFIX]CONFIG_PATH` environment variable
   1. `.<prefix>.toml` in the current directory
   1. `.<prefix>.toml` in the user's home directory (where `<prefix>` comes from
      `#[ortho_config(prefix = "...")]` and defaults to `config`). JSON5 and
      YAML support are feature gated.
1. **Environment Variables:** Variables prefixed with the string specified in
   `#[ortho_config(prefix = "...")]` (e.g., `APP_`). Nested struct fields are
   typically accessed using double underscores (e.g., `APP_DATABASE__URL` if
   `prefix = "APP"` on `AppConfig` and no prefix on `DatabaseConfig`, or
   `APP_DB_URL` with `#` on `DatabaseConfig`).
1. **Command-Line Arguments:** Parsed using `clap` conventions. Long flags are
   derived from field names (e.g., `my_field` becomes `--my-field`).
```

Subcommands can load defaults from a `cmds` namespace:

```text
// Reads `[cmds.add-user]` sections and `APP_CMDS_ADD_USER_*` variables then merges with CLI
let args = load_and_merge_subcommand_for::<AddUserArgs>(&cli)?;

Configuration file example:
[cmds.add-user]
username = "file_user"
admin = true

Environment variables override file values using the pattern
`<PREFIX>CMDS_<SUBCOMMAND>_`:
APP_CMDS_ADD_USER_USERNAME=env_user
APP_CMDS_ADD_USER_ADMIN=false
```

Vectors support an `append` merge strategy:

```text
#[ortho_config(merge_strategy = "append")] // Default for Vec<T> is append
features: Vec<String>,
```

## Observed Gaps

| Gap | Potential Mitigation |
| --- | --- |
| Array Environment Variables | Support comma-separated lists such as `DDLINT_RULES=A,B,C`. |
| Extends Support | Implement an `extends` mechanism for configuration inheritance. |
| Custom Option Names | Document renaming `--config-path` to `--config`. |
| Dynamic Rule Tables | Use a map type to accept arbitrary rule entries. |
| Ignore Patterns | Allow comma-separated lists for environment variables. |

Overall, OrthoConfig covers layered loading and CLI integration. It would need
enhancements for string list parsing and configuration extension to fully
satisfy ddlint's design.

## Next Steps

The following steps are ordered by impact on ddlint's user experience:

1. **Comma-Separated Lists** – add support for parsing comma-separated
   environment variables as string lists. This allows `DDLINT_RULES=A,B,C`
   without JSON syntax.
2. **Configuration Inheritance** – design an `extends` mechanism so one file can
   pull defaults from another. Leverage the existing layering logic described in
   `docs/design.md`.
3. **Flag Name Overrides** – provide examples showing how to rename
   `--config-path` to `--config` using struct field attributes.
4. **Dynamic Tables** – explore using a map type (e.g., `BTreeMap`) to handle
   arbitrary rule names under `[rules]`.
5. **Ignore Pattern Lists** – after implementing comma-separated parsing,
   document usage for `ignore_patterns` to keep CLI and environment
   configuration consistent.

These improvements will align OrthoConfig with ddlint's planned interface while
maintaining compatibility with the crate's existing architecture.
