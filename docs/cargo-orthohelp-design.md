# OrthoConfig IR documentation design for cargo-orthohelp (v2)

This document defines the intermediate representation (IR) emitted by the
`OrthoConfig` derive macro and consumed by `cargo-orthohelp` to generate
localised UNIX man pages and PowerShell external help (Microsoft Assistance
Markup Language (MAML)) plus a wrapper module. It focuses on a command-line
interface (CLI) documentation pipeline that remains `clap` agnostic and keeps
documentation code out of application binaries.

- Status: Revision 2 (Windows and PowerShell amendments integrated).
- Audience: OrthoConfig maintainers and consumers.
- Goal: Generate fully localised UNIX man pages and PowerShell external help
  (MAML and wrapper module) from a robust, `clap`-agnostic IR.
- Non-goals:
  - Shipping documentation code in application binaries.
  - Hidden or dummy `clap` arguments.
  - Scraping `--help` output.
  - Depending on `clap_mangen`.

Note: document revisions track narrative changes, while compatibility is
governed by the IR schema version (`ir_version`). Generators must use the IR
schema version to determine compatibility, regardless of document revision.

## 0. Changelog (v2 vs v1)

PowerShell and Windows amendments (no change to the IR-first philosophy):

1. Wrapper module is mandatory on Windows. `Get-Help` indexes PowerShell
   artefacts only. The generator must emit a module that exports a function
   forwarding to the native executable, and the MAML `<command:name>` must
   match the exported function exactly.
2. Dual module roots: install the same module into both
   `%ProgramFiles%\WindowsPowerShell\Modules\<ModuleName>` (PowerShell 5.1) and
   `%ProgramFiles%\PowerShell\Modules\<ModuleName>` (PowerShell 7+).
3. Completions: register against the wrapper function. At import, detect
   `Register-ArgumentCompleter -Native` (PowerShell 7+) and fall back to the
   non-`-Native` form on PowerShell 5.1.
4. Locale fallback: always generate `en-US/<ModuleName>-help.xml`. If a
   target locale exists but `en-US` does not, copy the target into `en-US` so
   `Get-Help` never returns empty help.
5. CommonParameters: wrappers use
   `[CmdletBinding(PositionalBinding = $false)]` so `Get-Help -Full` lists
   common parameters. The MAML writer includes `<CommonParameters/>` unless
   explicitly disabled.
6. About topic: generate `about_<ModuleName>.help.txt` from the IR conceptual
   material (overview, discovery, precedence) per locale.
7. HelpInfoUri: optional. Only set when Update-Help payloads are published;
   otherwise omit it to avoid broken Update-Help user experience (UX).
8. Microsoft Installer (MSI) layout guidance: place the executable under
   `...\\Program Files\\<Vendor>\\<Product>\\bin\\` and add it to the machine
   PATH, drop the module into both module roots, and recommend code signing for
   the executable and MSI.
9. Wrapper robustness: resolve the executable path relative to
   `$PSScriptRoot`, forward `@Args`, and propagate `$LASTEXITCODE`.
10. IR additions (Windows-only, optional): `windows.module_name` and
    `windows.wrapper` knobs (aliases, common parameters, split subcommands)
    for the generator to consume. These have no runtime impact.

The IR schema bumps to `1.1` to include optional Windows metadata.

## 1. Architecture overview

```plaintext
User crate (uses OrthoConfig)
┌───────────────────────────────────┐
│ #[derive(OrthoConfig)]            │
│ struct AppConfig { ... }          │
│                                   │
│ OrthoConfigDocs::get_doc_metadata │
│   -> DocMetadata (IR)             │
└───────────────────────────────────┘

OrthoConfig workspace
┌───────────────────────────────────┐
│ ortho-config (runtime + macros)   │
│ - Localizer + Fluent impl         │
│ - Derive: runtime loaders         │
│ - Derive: OrthoConfigDocs (IR)    │
└───────────────────────────────────┘

cargo-orthohelp (CLI tool)
┌───────────────────────────────────┐
│ 1) Builds ephemeral bridge        │
│ 2) Calls get_doc_metadata()       │
│ 3) Resolves Fluent IDs per locale │
│ 4) Emits roff and MAML + module   │
└───────────────────────────────────┘
```

Key choices:

- IR over `clap` for complete coverage across CLI, environment variables, and
  files without dummy arguments.
- Localisation at generation time: the IR stores message identifiers (IDs),
  and generators resolve per locale.
- Out-of-band tooling: `cargo orthohelp` compiles and runs a tiny ephemeral
  bridge to fetch the IR, keeping application binaries clean.

## 2. Documentation IR (schema v1.1)

### 2.1 Top-level metadata

```rust
#[derive(Debug, Serialize)]
pub struct DocMetadata {
    pub ir_version: String,            // e.g., "1.1"
    pub app_name: String,              // binary or display name
    pub bin_name: Option<String>,      // override for man page or wrapper name
    pub about_id: String,              // Fluent ID for app description
    pub synopsis_id: Option<String>,   // Fluent ID for synopsis summary
    pub sections: SectionsMetadata,    // headings, discovery, precedence, etc.
    pub fields: Vec<FieldMetadata>,    // flattened fields for this command
    pub subcommands: Vec<DocMetadata>, // recursively the same schema
    pub windows: Option<WindowsMetadata>, // Windows-only generator hints
}
```

```rust
#[derive(Debug, Serialize)]
pub struct SectionsMetadata {
    pub headings_ids: HeadingIds,        // Fluent IDs for standard headings
    pub discovery: Option<ConfigDiscoveryMeta>,
    pub precedence: Option<PrecedenceMeta>,
    pub examples: Vec<Example>,          // app-level examples
    pub links: Vec<Link>,                // app-level related links
    pub notes: Vec<Note>,                // app-level notes or disclaimers
}
```

```rust
#[derive(Debug, Serialize)]
pub struct HeadingIds {
    pub name: String,
    pub synopsis: String,
    pub description: String,
    pub options: String,
    pub environment: String,
    pub files: String,
    pub precedence: String,
    pub exit_status: String,
    pub examples: String,
    pub see_also: String,
}
```

### 2.2 Field-level metadata

```rust
#[derive(Debug, Serialize)]
pub struct FieldMetadata {
    pub name: String,                  // Rust field name
    pub help_id: String,               // Fluent ID for field help
    pub long_help_id: Option<String>,  // optional long help ID
    pub value: Option<ValueType>,      // semantic value type for rendering
    pub default: Option<DefaultValue>,
    pub required: bool,
    pub deprecated: Option<Deprecation>,
    pub cli: Option<CliMetadata>,      // if exposed via CLI
    pub env: Option<EnvMetadata>,      // if exposed via environment variable
    pub file: Option<FileMetadata>,    // if exposed via files
    pub examples: Vec<Example>,        // field-level examples
    pub links: Vec<Link>,
    pub notes: Vec<Note>,
}
```

```rust
#[derive(Debug, Serialize)]
pub struct CliMetadata {
    pub long: Option<String>,          // "port"
    pub short: Option<char>,           // 'p'
    pub value_name: Option<String>,    // e.g., "NUM"
    pub multiple: bool,                // repeats allowed
    pub takes_value: bool,             // false for switches
    pub possible_values: Vec<String>,  // for enums
    pub hide_in_help: bool,            // excluded from OPTIONS section
}

#[derive(Debug, Serialize)]
pub struct EnvMetadata {
    pub var_name: String,
}

#[derive(Debug, Serialize)]
pub struct FileMetadata {
    pub key_path: String, // e.g., "database.host"
}
```

### 2.3 Value typing and defaults

```rust
#[derive(Debug, Serialize)]
pub enum ValueType {
    String,
    Integer { bits: u8, signed: bool },
    Float { bits: u8 },
    Bool,
    Duration,
    Path,
    IpAddr,
    Hostname,
    Url,
    Enum { variants: Vec<String> },
    List { of: Box<ValueType> },
    Map { of: Box<ValueType> },
    Custom { name: String },
}

#[derive(Debug, Serialize)]
pub struct DefaultValue {
    pub display: String,
}

#[derive(Debug, Serialize)]
pub struct Deprecation {
    pub note_id: String,
}
```

### 2.4 Config discovery and precedence

```rust
#[derive(Debug, Serialize)]
pub struct ConfigDiscoveryMeta {
    pub formats: Vec<ConfigFormat>,         // e.g., [Toml, Json, Yaml]
    pub search_paths: Vec<PathPattern>,     // ordered (lowest -> highest)
    pub override_flag_long: Option<String>, // e.g., "config"
    pub override_env: Option<String>,       // e.g., "MYAPP_CONFIG"
    pub xdg_compliant: bool,
}

#[derive(Debug, Serialize)]
pub enum ConfigFormat {
    Toml,
    Yaml,
    Json,
}

#[derive(Debug, Serialize)]
pub struct PathPattern {
    pub pattern: String,
    pub note_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PrecedenceMeta {
    pub order: Vec<SourceKind>,             // e.g., [File, Env, Cli]
    pub rationale_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub enum SourceKind {
    Defaults,
    File,
    Env,
    Cli,
}
```

XDG refers to the XDG Base Directory specification used for configuration
discovery and the `xdg_compliant` flag.

### 2.5 Windows metadata (optional)

```rust
#[derive(Debug, Serialize)]
pub struct WindowsMetadata {
    pub module_name: Option<String>,
    pub export_aliases: Vec<String>,
    pub include_common_parameters: bool,
    pub split_subcommands_into_functions: bool,
    pub help_info_uri: Option<String>,
}
```

Resolution order for Windows generator settings: CLI flags override
`Cargo.toml` metadata, which overrides `windows` values from the IR. Defaults
apply when no source provides a value. When multiple sources provide the same
setting, the highest-precedence source wins.

### 2.6 Extras (examples, links, and notes)

```rust
#[derive(Debug, Serialize)]
pub struct Example {
    pub title_id: Option<String>,
    pub code: String,
    pub body_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Link {
    pub text_id: Option<String>,
    pub uri: String,
}

#[derive(Debug, Serialize)]
pub struct Note {
    pub text_id: String,
}
```

Flattening rule: config file keys must be emitted as dotted `key_path` (e.g.,
`network.proxy.url`) regardless of internal nesting. Environment variable names
must be unique per field (see section 5.3). The only nested structure is
`subcommands`.

## 3. Derive macro integration

### 3.1 Trait

```rust
pub trait OrthoConfigDocs {
    /// Returns the complete documentation metadata for this config.
    fn get_doc_metadata() -> DocMetadata;
}
```

The `#[derive(OrthoConfig)]` macro emits this implementation alongside runtime
loaders, filling all IR fields from the same parsed metadata.

### 3.2 Attributes (doc-related)

Namespace: `#[ortho_config(...)]`. Selected keys:

- IDs and text: `help_id`, `long_help_id`, `about_id`, `synopsis_id`.
- Exposure and naming:
  `cli(long = "...", short = 'x', value_name = "...", hide_in_help)`,
  `env(name = "...")`, `file(key_path = "...")`.
- Semantics: `required`, `default = "..."`,
  `deprecated(note_id = "...")`, `value(type = "duration|ipaddr|url|...")`.
- Extras:
  `example(code = "...", title_id = "...", body_id = "...")*`,
  `link(uri = "...", text_id = "...")*`, `note(text_id = "...")*`.
- App or subcommand:
  `headings(name = "...", ...)`,
  `discovery(formats = [...], xdg = bool, override_flag = "...",`
  `override_env = "...")`,
  `precedence(order = ["defaults", "file", "env", "cli"],`
  `rationale_id = "...")`.
- Windows (optional, generator hints):
  `windows(module_name = "...", include_common_parameters = true,`
  `split_subcommands = false)`.

### 3.3 Diagnostics

Hard errors at macro time:

- Duplicate `env.var_name` or `file.key_path` across fields emits a hard error
  with spans on both fields and a remediation hint.
- Illegal characters in environment variable names or file key paths emit a
  hard error with suggested canonical forms.
- Ambiguous value typing suggests `value(type = ...)`.

Warnings:

- Unknown or unused locale IDs, missing heading overrides, or Windows hints on
  non-Windows targets.

### 3.4 Auto-ID generation

Deterministic IDs when omitted:

- App about: `"{crate}.about"`.
- Field help: `"{crate}.fields.{command_path}.{field}.help"`.
- Long help: `"{crate}.fields.{command_path}.{field}.long_help"`.
- Headings: library defaults such as `"ortho.headings.options"`.

`command_path` is `sub1.sub2` for nested subcommands.

## 4. Localisation model

### 4.1 Resolver

`Localizer` (trait) with `FluentLocalizer` implementation layered (consumer
bundle -> library defaults -> English). Generators pass a `&dyn Localizer` for
the target locale.

### 4.2 Catalogues

```plaintext
locales/
  en-GB/ortho_config.ftl         # default headings or boilerplate
  en-GB/<crate>.ftl              # consumer app translations (optional)
  fr-FR/...                      # additional locales
```

PowerShell note: always emit `en-US` help XML. If generating another locale
only (for example, `en-GB`), copy it to `en-US` as a fallback because
PowerShell culture probing strongly prefers `en-US` presence.

## 5. Naming and flattening

### 5.1 File key paths (dotted)

- Derived from nested field structure; segments default to snake_case.
- Override via `#[ortho_config(file(key_path = "..."))]`.
- Validate `[A-Za-z0-9_-]+` per segment (library default). Render quoting
  guidance in docs if users need non-ASCII values.

### 5.2 Environment variable names

- Prefix from crate (uppercased; non-alphanumeric -> `_`), for example,
  `my-app` -> `MY_APP`.
- Segments: top-level -> `FIELD`; nested -> `PARENT__CHILD` (double
  underscore between segments).
- Final: `{PREFIX}_{SEGMENTS}`.

Examples:

- `database.host` -> `MY_APP_DATABASE__HOST`.
- `database_host` -> `MY_APP_DATABASE_HOST`.

### 5.3 Collision detection

- Build maps of `env.var_name` and `file.key_path`. If a duplicate maps to a
  different field, raise a hard error with remediation text.

## 6. `cargo orthohelp` CLI

### 6.1 Interface (proposed)

```bash
cargo orthohelp \
  [--package <pkg>] [--bin <name> | --lib] \
  [--root-type <path::to::Type>] \
  [--locale <lang>] [--all-locales] \
  [--format man|ps|all] \
  [--out-dir <path>] \
  [--man-section <N>] [--man-date <YYYY-MM-DD>] [--man-split-subcommands] \
  [--ps-module-name <Name>] [--ps-split-subcommands] \
  [--ps-include-common-parameters] [--ps-help-info-uri <URI>] \
  [--ensure-en-us] \
  [--cache] [--no-build]
```

`Cargo.toml` defaults:

```toml
[package.metadata.ortho_config]
root_type = "my_crate::AppConfig"
module_name = "MyApp"
locales = ["en-GB", "fr-FR"]
man_section = 1
```

### 6.2 Pipeline

1. Discover the package with `cargo metadata`.
2. Determine the root type from CLI or metadata. If missing, emit an error
   with remediation guidance.
3. Build the ephemeral bridge under `target/orthohelp/<hash>/`:
   - Dependencies: `user_crate`, `ortho_config_docs`.
   - `main.rs` invokes
     `<root_type as OrthoConfigDocs>::get_doc_metadata()` and serialises the
     IR JSON to stdout.
4. Run the bridge and capture the IR.
5. For each locale, instantiate `FluentLocalizer` and resolve IDs to strings.
6. Emit the requested outputs into `--out-dir`.
7. Summarise artefacts and exit non-zero on hard errors.

### 6.3 Caching

Cache IR at `target/orthohelp/<hash>/ir.json` keyed by the crate fingerprint
plus macro and tool versions. `--cache` reuses it when valid; `--no-build`
trusts the existing IR.

## 7. Output generators

### 7.1 Man page (roff)

Files: `man/man<section>/<name>.<section>` (or `.../<name>-<sub>.<section>`
when split). Use classic `man` macros: `.TH`, `.SH`, `.SS`, `.TP`, `.B`, `.I`.

Sections: NAME, SYNOPSIS, DESCRIPTION, OPTIONS, ENVIRONMENT, FILES, PRECEDENCE,
EXAMPLES, SEE ALSO, EXIT STATUS.

Rules mirror v1: CLI fields in OPTIONS; environment variables in ENVIRONMENT;
config keys and discovery in FILES; precedence explained; examples rendered
verbatim.

### 7.2 PowerShell help (MAML) and wrapper module

Artefacts (per locale):

```plaintext
<out>/powershell/<ModuleName>/
  <ModuleName>.psm1
  <ModuleName>.psd1
  <culture>/<ModuleName>-help.xml  # always include en-US
  about_<ModuleName>.help.txt      # conceptual, optional but recommended
  completions.ps1                  # optional separate script
```

Wrapper function:

```powershell
# <ModuleName>.psm1
[CmdletBinding(PositionalBinding = $false)]
param()

function <BinName> {
  [CmdletBinding(PositionalBinding = $false)]
  param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Args)
  $exe = Join-Path $PSScriptRoot '..' 'bin' '<bin>.exe'
  $exe = (Resolve-Path $exe).ProviderPath
  & $exe @Args
  $global:LASTEXITCODE = $LASTEXITCODE
}

# Completions registration
$sb = {
  param($wordToComplete, $commandAst, $cursorPosition)
  # Delegate to generated completion logic or script body
}
$hasNative = (Get-Command Register-ArgumentCompleter).Parameters.ContainsKey(
  'Native'
)
if ($hasNative) {
  Register-ArgumentCompleter -Native -CommandName '<BinName>' -ScriptBlock $sb
} else {
  Register-ArgumentCompleter -CommandName '<BinName>' -ScriptBlock $sb
}
```

Manifest (minimum):

```powershell
@{
  RootModule = '<ModuleName>.psm1'
  ModuleVersion = '0.1.0'
  CompatiblePSEditions = @('Desktop', 'Core')
  FunctionsToExport = @('<BinName>')
  # Only set if hosting Update-Help payloads.
  # HelpInfoUri = 'https://example.com/help/<ModuleName>'
  ExternalHelp = '<ModuleName>-help.xml'
}
```

MAML mapping:

- One `<command:command>` for the wrapper function. If
  `--ps-split-subcommands` is set, also export `<BinName>_<sub>` functions and
  generate separate `<command:command>` nodes.
- Parameters from `CliMetadata`:
  - Switches map to `SwitchParameter`.
  - Values map to `String`, `Int32`, `Double`, and so on inferred from
    `ValueType`.
  - Required or position heuristics: position only when unambiguous; otherwise
    named.
- Field descriptions come from `help_id` or `long_help_id`.
- Enum allowed values append to the description.
- Environment or file exposure is documented in Notes per parameter.
- App examples or links map to `<command:examples>` or
  `<maml:relatedLinks>`.
- Include `<CommonParameters/>` unless disabled via IR or CLI flag.

Culture folders: always generate `en-US`. Add additional cultures per
`--locale` or `--all-locales`.

Line endings and encoding: emit `.psm1` and `.psd1` with carriage return line
feed (CRLF) line endings. Emit MAML XML as UTF-8 with a byte order mark (BOM)
for maximum compatibility.

## 8. Templates and formatting

- Default `value_name` when absent: `STRING`, `INT`, `FLOAT`, `PATH`,
  `DURATION`, `IP`, `URL`, `CHOICE`, `LIST`, `MAP`.
- OPTIONS grouped by top-level `file.key_path` segment, then by flag name.
- ENVIRONMENT sorted by variable name; FILES grouped by table.
- Headings use Fluent IDs; library defaults apply when missing.

## 9. Error handling and diagnostics

Macro time:

- Duplicates, illegal names, and ambiguous types are hard errors with spans
  and remediation text.

Generation time:

- Missing Fluent messages emit warnings and fall back to English or
  `[missing: ...]` in development mode.
- MAML validation errors include line and column information.
- Wrapper or function name mismatches with the MAML `<command:name>` emit an
  error.
- Missing `en-US` when other cultures exist and `--ensure-en-us` is enabled
  triggers a copy and warning.

Exit non-zero on hard errors and list artefacts on success.

## 10. Testing strategy

- Macro unit tests: attribute parsing, ID generation, collision detection.
- Roff unit tests: escaping, width wrapping, enum rendering.
- MAML unit tests: schema sanity and value type mapping.
- Golden tests: generate outputs for a fixture config across locales and
  compare against goldens.
- Windows integration tests:
  - `powershell.exe` (5.1) and `pwsh` (7+) import the generated module,
    `Get-Help <BinName> -Full` works, and CommonParameters render.
  - The argument completer registers with or without `-Native`.
  - Wrapper preserves `$LASTEXITCODE`.

## 11. Packaging and MSI guidance

- Install the executable to `C:\Program Files\<Vendor>\<Product>\bin\` and
  add that folder to PATH (machine scope).
- Install the PowerShell module to both module roots:
  - `C:\Program Files\WindowsPowerShell\Modules\<ModuleName>\`.
  - `C:\Program Files\PowerShell\Modules\<ModuleName>\`.
- Place culture subfolders (`en-US`, `en-GB`, and so on) under the module
  directory.
- Code sign the executable and MSI; module scripts are optional but recommended
  in locked-down environments.

These are packaging recommendations; the generator writes only to `--out-dir`.

## 12. Versioning and compatibility

- IR: `ir_version = "1.1"` (Windows metadata added). Future breaking schema
  changes bump the major version.
- Tooling: `cargo-orthohelp` tracks the IR major.
- Runtime: `clap` v4.x unchanged; PowerShell targets 5.1+ and 7+.

## 13. Worked example (abridged)

### 13.1 IR JSON (excerpt, 1.1)

```json
{
  "ir_version": "1.1",
  "app_name": "myapp",
  "bin_name": "myapp",
  "about_id": "myapp.about",
  "sections": {
    "headings_ids": {
      "name": "ortho.headings.name",
      "synopsis": "ortho.headings.synopsis",
      "description": "ortho.headings.description",
      "options": "ortho.headings.options",
      "environment": "ortho.headings.environment",
      "files": "ortho.headings.files",
      "precedence": "ortho.headings.precedence",
      "exit_status": "ortho.headings.exit_status",
      "examples": "ortho.headings.examples",
      "see_also": "ortho.headings.see_also"
    }
  },
  "fields": [
    {
      "name": "port",
      "help_id": "myapp.fields.port.help",
      "value": {"Integer": {"bits": 16, "signed": false}},
      "default": {"display": "8080"},
      "required": false,
      "cli": {
        "long": "port",
        "short": "p",
        "value_name": "NUM",
        "multiple": false,
        "takes_value": true,
        "possible_values": [],
        "hide_in_help": false
      },
      "env": {"var_name": "MY_APP_PORT"},
      "file": {"key_path": "port"}
    }
  ],
  "windows": {
    "module_name": "MyApp",
    "export_aliases": [],
    "include_common_parameters": true,
    "split_subcommands_into_functions": false,
    "help_info_uri": null
  },
  "subcommands": []
}
```

### 13.2 Wrapper (psm1) excerpt

```powershell
[CmdletBinding(PositionalBinding = $false)]
param()
function myapp {
  [CmdletBinding(PositionalBinding = $false)]
  param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Args)
  $exe = Join-Path $PSScriptRoot '..' 'bin' 'myapp.exe'
  $exe = (Resolve-Path $exe).ProviderPath
  & $exe @Args
  $global:LASTEXITCODE = $LASTEXITCODE
}
$sb = { param($wordToComplete, $commandAst, $cursorPosition) # ... }
$hasNative = (Get-Command Register-ArgumentCompleter).Parameters.ContainsKey(
  'Native'
)
if ($hasNative) {
  Register-ArgumentCompleter -Native -CommandName 'myapp' -ScriptBlock $sb
} else {
  Register-ArgumentCompleter -CommandName 'myapp' -ScriptBlock $sb
}
```

### 13.3 Manifest (psd1) excerpt

```powershell
@{
  RootModule = 'MyApp.psm1'
  ModuleVersion = '0.1.0'
  CompatiblePSEditions = @('Desktop', 'Core')
  FunctionsToExport = @('myapp')
  ExternalHelp = 'MyApp-help.xml'
}
```

## 14. Implementation plan (delta)

1. Bump the IR schema to 1.1 and add `WindowsMetadata` plus CLI flags.
2. Make the PowerShell wrapper mandatory when `--format ps|all` is selected
   and default to generating PowerShell artefacts on Windows unless
   `--format man` is set.
3. Enforce `en-US` emission and implement `--ensure-en-us` (on by default).
4. Add `-Native` detection logic to the module template and ensure
   `$LASTEXITCODE` propagation.
5. Extend the MAML writer to emit `<CommonParameters/>` and generate the
   about topic file.
6. Add Windows integration tests (PowerShell 5.1 and 7+).
