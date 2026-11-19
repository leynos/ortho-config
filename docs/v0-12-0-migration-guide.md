# Migration guide: v0.11.0 to v0.12.0

## Who should read this

Read this guide before upgrading an application or library from OrthoConfig
v0.11.0 to v0.12.0. Applications that derive their configuration need no code
changes: every addition is opt-in, and the wire contract stays additive.

One behaviour changes for an application that overrides `clap` error text, and
it deserves a review before release:

- an override of `clap-error-missing-subcommand` no longer reaches the help
  `clap` prints when a required argument or subcommand is absent.

JSON consumers need no change, and neither do Fluent catalogues: the
identifiers this release adds are new, and every identifier that predates it
keeps its spelling and its meaning.

## Impact at a glance

| Priority             | Area                 | What to do                                                           |
| -------------------- | -------------------- | -------------------------------------------------------------------- |
| **Required**         | Dependency versions  | Update every OrthoConfig crate requirement to v0.12.0 together.      |
| **Required if used** | Clap error overrides | Re-check an override of `clap-error-missing-subcommand`; see §2.     |
| **Optional**         | Clap error inventory | Iterate `CLAP_ERROR_IDS` instead of hard-coding identifiers; see §3. |

_Table 1: Application-facing work when moving from v0.11.0 to v0.12.0._

## 1. Update dependency versions

Update the runtime crate, and any direct macro or tooling dependency, as one
unit.

Before:

```toml
[dependencies]
ortho_config = { version = "0.11.0", features = ["yaml"] }
```

After:

```toml
[dependencies]
ortho_config = { version = "0.12.0", features = ["yaml"] }
```

## 2. Re-check an override for missing-input help

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

## 3. Enumerate the translated clap error kinds

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
and [ADR-0024](adrs/adr-0024-pre-publication-clap-coverage-gate.md) for how
the table stays bound to the resolved version.
