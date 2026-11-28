# Migration guide: v0.11.0 to v0.12.0

## Who should read this

Read this guide before upgrading an application or library from OrthoConfig
v0.11.0 to v0.12.0. No Rust API changes are required. The release adds one
opt-in builder method for controlling how `Fluent` isolates interpolated
values, and generated documentation now describes the clap command line that
executes.

## Impact at a glance

| Priority     | Area                      | What to do                                                                                                    |
| ------------ | ------------------------- | ------------------------------------------------------------------------------------------------------------- |
| **Required** | Dependency versions       | Update every OrthoConfig crate requirement to v0.12.0 together.                                               |
| **Optional** | `Fluent` isolation marks  | Call `with_use_isolating(false)` in terminals that must not show them.                                        |
| **Optional** | Prebuilt consumer bundles | Passing an explicit `true` or `false` now overrides the bundle's choice.                                      |
| **Optional** | Documentation metadata    | Declare `#[arg(long)]` or `#[arg(skip)]` where a clap-declared struct must keep a named flag or hide a field. |

_Table 1: Application-facing work when moving from v0.11.0 to v0.12.0._

## 1. Update dependency versions

Update the runtime crate, and any direct macro or tooling dependency, as one
unit.

Before:

```toml
[dependencies]
ortho_config = { version = "0.11.0" }
```

After:

```toml
[dependencies]
ortho_config = { version = "0.12.0" }
```

## 2. Optionally remove Unicode isolation marks from localized output

While isolation is enabled, `Fluent` wraps a placeable in an outer pair of
marks — U+2068 FIRST STRONG ISOLATE and U+2069 POP DIRECTIONAL ISOLATE — when
the message also contains literal text and the expression is a variable or
another non-literal expression. Isolation keeps right-to-left substitutions in
the intended order inside left-to-right copy, but the marks are invisible
characters that some terminals and log viewers render as boxes, and they defeat
`grep`, exact string comparison, and snapshot assertions on diagnostics.

`Fluent` adds no outer marks to a pattern that is a single placeable, such as
`name = { $name }`, nor to a direct message reference, a term reference, or a
string literal. A referenced message can still add its own marks inside its own
pattern, so code that embeds those results in its own left-to-right copy cannot
rely on the outer marks.

`FluentLocalizerBuilder::with_use_isolating` chooses the behaviour:

- Leaving the setting unset preserves each bundle's own setting, which keeps
  `Fluent`'s `true` default for resource-built bundles and keeps any supplied
  prebuilt bundle's choice. Existing output is unchanged.
- `with_use_isolating(false)` removes the marks from the consumer catalogue and
  the embedded defaults, including the built-in Clap error copy.
- `with_use_isolating(true)` restores the marks on a bundle that was built
  without them.

The value is applied once when `try_build` constructs the bundles, so setter
order does not matter and the last call wins. The option controls one `Fluent`
formatting setting; it is not bidirectional sanitization and never rewrites
catalogue text.

See "Remove Unicode isolation marks from localized output" in the
[user's guide](users-guide.md#remove-unicode-isolation-marks-from-localized-output)
for a worked example, and
[the design document](design.md#412-localization-architecture) for why a
locale-script default was rejected.

## 3. Review generated documentation for clap-declared structs

Generated documentation metadata now describes the command line that clap
parses. A clap helper attribute only compiles when a clap derive registered it,
so a struct carrying any clap helper — a struct-level `#[command(...)]`,
`#[arg(...)]`, `#[command(flatten)]`, or `#[command(subcommand)]` — is a clap
command throughout: every field without its own declaration is documented as
the positional operand clap derives for it, with clap's one-based index and
value name. A struct that declares no clap argument keeps the generated
`OrthoConfig` flag surface.

- Add `#[arg(long)]` or `#[arg(short)]` to a field that must remain a named
  option in a clap-declared struct.
- Add `#[arg(skip)]` to a field that must not appear on the command line at
  all. `#[ortho_config(skip_cli)]` only affects the generated loader parser,
  which clap does not read.
- Generated environment suffixes and file keys follow Serde's deserialization
  key, so `#[serde(rename = "...")]`, `#[serde(rename(deserialize = "..."))]`,
  and a container `rename_all` rule change the documented names. A
  serialization-only rename does not. A kebab-case key keeps its hyphen in the
  environment spelling; a camel-case, dotted, or double-underscore key keeps
  its file key but has no environment route, because the provider cannot read
  such a variable back as that key.
- A clap declaration the derive cannot read statically, such as
  `value_name = SOME_CONST`, is documented without CLI metadata. Use a literal
  value name when the placeholder must appear.

No Rust API changes are required. Rendered man pages, PowerShell help, and
agent context change only where the previous metadata disagreed with clap.
