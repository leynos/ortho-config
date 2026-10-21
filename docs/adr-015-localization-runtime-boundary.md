# Architectural decision record (ADR) 015: Reusable localization runtime

## Status

Accepted for issue #566.

## Date

2026-10-20.

## Context and problem statement

Applications need catalogue lookup and formatting without configuration
discovery, merging, derives, or Clap. Disabling `ortho_config` default features
does not remove those dependencies. Netsuke's component decomposition needs a
direct localization dependency; its issue #779 concerns documentation
intermediate representation (IR) 2.0, rather than the build graph.

## Decision

Extract `ortho_l10n` as a publishable workspace crate. It owns `Localizer`,
`NoOpLocalizer`, Fluent arguments, builders, formatting reports and errors,
catalogue composition, embedded default resources, and identifier
normalization. It must never depend on `ortho_config`, its macros or test
helpers, Figment, or Clap, including in its focused test graph.

`ortho_config` re-exports the actual runtime types. It retains configuration
policy, Clap command and error adapters, `OrthoConfigLocalization`,
`ArgLocalizationIds`, and the documentation IR. No bridge traits or wrapper
localizers are introduced. Existing macro-generated configuration paths remain
valid through these re-exports.

Keep the small English and Japanese resource files below the configuration
layer. `FluentLocalizer::builder` continues to enable embedded defaults and
select them by language; other languages require `disable_defaults()`.
Catalogue loading never calls configuration discovery. The legacy CLI, Clap
error, and profile-error copy is compatibility data, rather than a mandate for
consumer product catalogues or locale policy.

Keep the Fluent argument representation unchanged. Consumer crates own product
catalogues, diagnostic mappings, requested locale, and application fallback
policy. A backend-neutral argument interface is a separate proposal.

## Shared identifier contract

`ortho_l10n` owns the fallible segment normalizer, identifier join, dotted
suffix composition, and separator. These helpers are reusable by runtimes and
code generators; they carry no documentation IR or configuration metadata. The
existing panicking `message_id_for` and macro spanned diagnostics wrap the same
fallible implementation, preserving error text and validation order. Argument
fragments permit digit-leading segments; complete identifiers require an
ASCII-letter root. Configuration adapters retain collision detection.

The macro crate has a normal version-plus-path dependency on `ortho_l10n`. The
dependency graph remains acyclic: configuration depends on macros and the
runtime, while macros depend on the runtime. Measure the additional host-build
critical path before considering an identifier-only feature.

This supersedes Decision D-8's paired normalizers and version-marker tests. The
shared derive/documentation identifier contract belongs to PR 536, which
supersedes PR 420. Its traits and IR stay in `ortho_config`; neither a complete
documentation-IR move nor Netsuke's IR programme is a prerequisite for
extracting the runtime. Consumers of documentation metadata read
`ortho_config::docs::ORTHO_DOCS_IR_VERSION` instead of hard-coding its version.

## Consequences and validation

Standalone consumers must build and test outside workspace feature unification.
Dependency-closure checks cover normal/build edges and focused root dev edges,
with injected forbidden dependencies as negative controls. Mixed consumers
prove nominal type identity, and agreement tests connect derive constants,
runtime identifiers, and documentation metadata. Move pure runtime tests with
the implementation; its test support must not depend back on configuration.

Publish `ortho_l10n` before macros and configuration. Package consumers must
exercise the publishable version-plus-path graph. Record check/test-build
times, closure counts, target sizes, toolchain, and cache state without
assuming a percentage improvement. Full applications may still compile
configuration machinery separately.

## Alternatives rejected

- Optionalizing all configuration dependencies would retain the wrong
  ownership boundary and require extensive feature combinations.
- Duplicate traits or bridge wrappers would break nominal type identity.
- Removing embedded defaults would change the existing builder contract.
- Moving Clap adapters or the complete documentation IR would broaden scope.
- Retaining two normalization implementations would preserve avoidable drift.

## References

- [CLI localization design](cli-localization-design.md).
- [ADR-006: Identifier panics](adr-006-identifier-derivation-panics.md).
- [Issue #566](https://github.com/leynos/ortho-config/issues/566).
- [Identifier contract PR #536](https://github.com/leynos/ortho-config/pull/536).
- [Netsuke documentation IR #779](https://github.com/leynos/netsuke/issues/779).
