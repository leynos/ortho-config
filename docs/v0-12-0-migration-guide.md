# Migration guide: v0.11.0 to v0.12.0

## Who should read this

Read this guide before upgrading an application or library from OrthoConfig
v0.11.0 to v0.12.0. No code changes are required. The release adds one opt-in
builder method for controlling how `Fluent` isolates interpolated values, and
the default behaviour is unchanged.

## Impact at a glance

| Priority     | Area                      | What to do                                                               |
| ------------ | ------------------------- | ------------------------------------------------------------------------ |
| **Required** | Dependency versions       | Update every OrthoConfig crate requirement to v0.12.0 together.          |
| **Optional** | `Fluent` isolation marks  | Call `with_use_isolating(false)` in terminals that must not show them.   |
| **Optional** | Prebuilt consumer bundles | Passing an explicit `true` or `false` now overrides the bundle's choice. |

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

When a message interleaves literal text with a variable or another non-literal
expression, `Fluent` wraps that value in U+2068 FIRST STRONG ISOLATE and U+2069
POP DIRECTIONAL ISOLATE. Isolation keeps right-to-left substitutions in the
intended order inside left-to-right copy, but the marks are invisible
characters that some terminals and log viewers render as boxes, and they defeat
`grep`, exact string comparison, and snapshot assertions on diagnostics.

`Fluent` does not isolate a pattern that is a single placeable, such as
`name = { $name }`, nor a message reference, a term reference, or a string
literal. Code that embeds those results in its own left-to-right copy cannot
rely on the marks.

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
