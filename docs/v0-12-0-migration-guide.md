# Migration guide: v0.11.0 to v0.12.0

## Who should read this

Read this guide before upgrading an application or library from OrthoConfig
v0.11.0 to v0.12.0. No code changes are required. The release adds one opt-in
builder method for controlling how `Fluent` isolates interpolated values, whose
default behaviour is unchanged, and it publishes the clap error translation
matrix as data. One clap behaviour does change for overrides: the help `clap`
prints when a required input is missing is no longer translated.

## Impact at a glance

| Priority     | Area                      | What to do                                                               |
| ------------ | ------------------------- | ------------------------------------------------------------------------ |
| **Required** | Dependency versions       | Update every OrthoConfig crate requirement to v0.12.0 together.          |
| **Optional** | `Fluent` isolation marks  | Call `with_use_isolating(false)` in terminals that must not show them.   |
| **Optional** | Prebuilt consumer bundles | Passing an explicit `true` or `false` now overrides the bundle's choice. |
| **Review if matched** | Missing-input help override | A `clap-error-missing-subcommand` override no longer replaces that help. |
| **Optional** | Translated clap inventory | Enumerate `CLAP_ERROR_IDS` rather than hard-coding identifiers. |

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
||||||| parent of 2ad3d66a (Re-version the clap notes and renumber the gate ADR)

## 3. Re-check an override for missing-input help

v0.12.0 classifies three `clap` error kinds as display-only: `DisplayHelp`,
`DisplayVersion`, and `DisplayHelpOnMissingArgumentOrSubcommand`. The
localization pipeline never substitutes a translated body for a display-only
kind, so missing-input help — the help `clap` prints when a required argument
or subcommand is absent — stays `clap`'s own text and keeps `clap`'s error exit
status.

An application that overrode `clap-error-missing-subcommand` to restyle that
help no longer sees the override applied there. The identifier still translates
the `MissingSubcommand` kind, so keep the override if that diagnostic still
needs consumer copy. Missing-input help has no translated identifier of its own.

No other catalogue entry changes meaning: the three display identifiers are
never looked up, and every translated identifier in the
[user's guide](users-guide.md#translate-or-override-clap-parse-errors) keeps
its earlier behaviour.

## 4. Enumerate the translated clap error kinds

v0.12.0 publishes its clap error inventory as data.
`ortho_config::CLAP_ERROR_IDS` lists every `clap::error::ErrorKind` this
release classifies, paired with an `ortho_config::ClapErrorTranslation` that is
either `Translated(identifier)` — the Fluent identifier a consumer override
targets — or `DisplayOnly`, the three kinds `clap` renders itself. Both are
exported from `ortho_config` and from `ortho_config::localizer`, so either path
works.

`ClapErrorTranslation` is `#[non_exhaustive]`: a match over it needs a wildcard
arm, so a later release can add an outcome without breaking your build.
Enumerate `CLAP_ERROR_IDS` rather than hard-coding identifiers when you build a
coverage test or a translator brief.

The inventory describes `clap_builder` 4.6.7, the version this workspace
resolves, and ships translated en-US text for 14 of its 17 kinds:
`InvalidValue`, `UnknownArgument`, `InvalidSubcommand`, `NoEquals`,
`ValueValidation`, `TooManyValues`, `TooFewValues`, `WrongNumberOfValues`,
`ArgumentConflict`, `MissingRequiredArgument`, `MissingSubcommand`,
`InvalidUtf8`, `Io`, and `Format`. The other three — `DisplayHelp`,
`DisplayVersion`, and `DisplayHelpOnMissingArgumentOrSubcommand` — are
display-only and are never looked up.

Four identifiers predate this matrix and keep their earlier spelling, so an
override catalogue written against an older release keeps resolving:
`clap-error-invalid-value`, `clap-error-unknown-argument`,
`clap-error-missing-argument` (for `MissingRequiredArgument`), and
`clap-error-missing-subcommand`. Every other translated row uses the spelling
that `clap-error-<kebab-cased kind>` derives.

A consumer that resolves a different `clap` release keeps this matrix. When
`clap` reports a kind the matrix does not list, localization first derives
`clap-error-<kebab-cased kind>` and looks that up; only when no catalogue
defines it — neither the embedded one nor an override — does the error keep
`clap`'s own text, with a warning event. See the
[pre-publication coverage gate](developers-guide.md#the-pre-publication-clap-coverage-gate)
and [ADR-0028](adrs/adr-0028-pre-publication-clap-coverage-gate.md) for how
the table stays bound to the resolved version.
