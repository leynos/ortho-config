# cargo-orthohelp

**cargo-orthohelp** is the OrthoConfig documentation generator. It turns
`OrthoConfigDocs` metadata into localized documentation artefacts without
adding documentation-generation code to your application binary.

## The problem this solves

Documentation for configuration-heavy command-line interfaces (CLIs) can easily
drift out of sync. Teams often maintain help text in multiple places (CLI help,
man pages, PowerShell help, environment variable docs, file-key docs), and each
source can diverge.

`cargo-orthohelp` solves this by generating all documentation outputs from one
intermediate representation (IR) produced by `#[derive(OrthoConfig)]`.

## How it works

1. It discovers your package and root config type from Cargo metadata or CLI
   flags.
2. It builds a tiny bridge binary that calls
   `OrthoConfigDocs::get_doc_metadata()`.
3. It resolves Fluent message IDs for each requested locale.
4. It emits localized IR JSON, roff man pages, PowerShell external help,
   compact agent-context JSON, or all formats.

## Core features

- **Single source of truth:** Generate docs from OrthoConfig IR metadata.
- **Localized output:** Resolve Fluent IDs per locale from `locales/<locale>`.
- **Multiple output formats:** IR JSON (`ir`), UNIX man pages (`man`),
  PowerShell help (`ps`), compact agent-context JSON (`agent-context`), or all
  formats (`all`).
- **Cache-aware pipeline:** `--cache` reuses bridge IR; `--no-build` enforces
  cache-only execution.
- **Cargo-native workflow:** Run as `cargo orthohelp` from your workspace.

## Agent-native roadmap status

`cargo-orthohelp` is the planned reference CLI for OrthoConfig's agent-native
work. The current released surface generates human documentation artefacts and
compact `agent-context` output. The roadmap adds `--json` command summaries,
agent-native lint checks, enumerating errors, stable exit classes, and atomic
artefact writes. See
[Agent-native CLI assistance design](../docs/agent-native-cli-design.md) and
[OrthoConfig roadmap](../docs/roadmap.md).

The planned exit classes are the public contract downstream tools must rely on:

| Class                   | Exit code | Semantics                                                                                            |
| ----------------------- | --------- | ---------------------------------------------------------------------------------------------------- |
| `success`               | `0`       | Command completed successfully and all requested artefacts were produced.                            |
| `usage`                 | `2`       | Invocation syntax, flags, format names, package names, or binary selection were invalid.             |
| `validation`            | `3`       | Input metadata, localization resources, policy checks, or generated documentation failed validation. |
| `external_tool_failure` | `5`       | Cargo, Rust compilation, bridge execution, filesystem, or other external tooling failed.             |

## Quick start

1. **Declare metadata in your application crate `Cargo.toml`:**
   Use the canonical example in
   [Cargo metadata defaults](#cargo-metadata-defaults).

2. **Generate localized IR JSON:**

   ```bash
   cargo orthohelp --package my_app --locale en-US --out-dir target/orthohelp
   ```

   This writes localized IR to `target/orthohelp/ir/en-US.json`.

3. **Generate man pages or PowerShell help when needed:**

   ```bash
   cargo orthohelp --package my_app --locale en-US --format man --out-dir target/docs
   cargo orthohelp --package my_app --locale en-US --format ps --out-dir target/docs
   ```

## Usage

```bash
cargo orthohelp \
  [--package <pkg>] [--bin <name> | --lib] \
  [--root-type <path::Type>] \
  [--locale <locale>] [--all-locales] \
  [--format ir|man|ps|agent-context|all] \
  [--out-dir <path>] \
  [--cache] [--no-build]
```

Common format-specific options:

- Man pages:
  `--man-section <1-8> --man-date <DATE> --man-split-subcommands`
- PowerShell:
  `--ps-module-name <NAME> --ps-split-subcommands <BOOL>`
  `--ps-include-common-parameters <BOOL> --ps-help-info-uri <URI>`
  `--ensure-en-us <BOOL>`

Boolean PowerShell options take explicit values and are not bare switches. Use
`--ps-split-subcommands true`, `--ps-include-common-parameters false`, or
`--ensure-en-us false`.

## Examples

Generate IR for two locales and reuse the bridge cache:

```bash
cargo orthohelp \
  --package my_app \
  --locale en-US --locale fr-FR \
  --format ir \
  --cache \
  --out-dir target/orthohelp
```

Generate section 5 man pages and split subcommands:

```bash
cargo orthohelp \
  --package my_app \
  --locale en-US \
  --format man \
  --man-section 5 \
  --man-split-subcommands \
  --out-dir target/man
```

Generate PowerShell module help with explicit module name:

```bash
cargo orthohelp \
  --package my_app \
  --locale en-US \
  --format ps \
  --ps-module-name MyApp \
  --ps-split-subcommands true \
  --ps-include-common-parameters true \
  --out-dir target/orthohelp
```

Generate every output format in one run:

```bash
cargo orthohelp --package my_app --all-locales --format all --out-dir target/docs
```

Generate compact agent-context JSON:

```bash
cargo orthohelp \
  --package my_app \
  --format agent-context \
  --out-dir target/orthohelp
```

## Output layout

For `--out-dir target/docs`:

- IR JSON: `target/docs/ir/<locale>.json`
- Man pages (single locale):
  `target/docs/man/man<section>/<name>.<section>`
- Man pages (multiple locales via `--all-locales` or repeated `--locale`):
  `target/docs/<locale>/man/man<section>/<name>.<section>`
- PowerShell:
  - `target/docs/powershell/<ModuleName>/<ModuleName>.psm1`
  - `target/docs/powershell/<ModuleName>/<ModuleName>.psd1`
  - `target/docs/powershell/<ModuleName>/<locale>/<ModuleName>-help.xml`
  - `target/docs/powershell/<ModuleName>/<locale>/about_<ModuleName>.help.txt`
- Agent context: `target/docs/agent-context.json`

## Cargo metadata defaults

`cargo-orthohelp` reads defaults from `package.metadata.ortho_config`:

```toml
[package.metadata.ortho_config]
root_type = "my_app::AppConfig"
locales = ["en-US", "fr-FR"]

[package.metadata.ortho_config.windows]
module_name = "MyApp"
include_common_parameters = true
split_subcommands_into_functions = false
help_info_uri = "https://example.com/help/MyApp"
```

If `root_type` is omitted from both CLI and metadata, generation fails with a
clear remediation message.
