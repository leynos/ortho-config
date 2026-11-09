# Architectural decision record (ADR) 0018: Pre-publication clap coverage gate

## Status

Accepted. Supersedes the build-time identity gate proposed in
[CLI localization design](../cli-localization-design.md) §6.1.1.

## Date

2026-11-09.

## Context and problem statement

`clap::error::ErrorKind` is `#[non_exhaustive]`, and stable clap 4.x may add
variants in a patch release. OrthoConfig ships an exhaustive matrix of en-US
translations and `DisplayOnly` sentinels, and needs a mechanical signal when
the resolved `ErrorKind` grows, so a new variant cannot fall through silently
untranslated.

The localization design originally required a build script to obtain the
resolved `clap_builder` package identity, verify it against a supported
manifest, emit `cargo:rustc-env=ORTHO_CONFIG_CLAP_ERROR_KIND_COUNT=<n>` and an
`OUT_DIR` constant, and fail closed for consumers whose identity it could not
establish.

Feasibility work for roadmap task 11.2.1 falsified the premise behind that
mechanism. A dependency's build script cannot observe its dependent's
dependency resolution, so no build-time check can establish which
`clap_builder` the compiling application actually resolved:

- In this workspace the enclosing `Cargo.lock` does govern the build, so the
  check would appear to work.
- For an external path consumer, `CARGO_MANIFEST_DIR` remains inside the
  dependency's checkout, so a lockfile walk finds the dependency's workspace
  lock, not the consumer's.
- For an unpacked packaged consumer, Cargo 1.84 and later ship a publish-time
  `Cargo.lock` beside the packaged manifest. That file describes the packaging
  resolution, including dev-dependencies the consumer never builds, and
  `cargo metadata` invoked from a build script honours it.

Every other route is closed as well. `clap` and `clap_builder` declare no
`links`, so there is no `DEP_*` metadata channel. At build-script time the
target directory names no `clap` package: the build-script unit's fingerprint
carries no dependencies, and the unit that does list `clap` is written only
after the build script has run. `OUT_DIR` ancestry reaches the consumer's
target directory, but that layout is not contractual. The Cargo team treats
dependency-to-dependent communication as out of scope.

A build-time gate would therefore either bind the wrong lockfile silently or
reject ordinary path and git consumers. Neither outcome is acceptable, and the
public contract does not need it.

## Decision Drivers

- Detect upstream `ErrorKind` growth before a release, not in production.
- Never claim a consumer-side guarantee that cannot be delivered.
- Introduce no build script, generated build output, or network request.
- Leave the public enum and constant unchanged.

## Decision

The coverage gate is a test-only pre-publication check inside this repository.
It does not run for consumers and makes no claim about their resolutions.

1. Read the root workspace `Cargo.lock` and select the unique locked `clap` that
   `ortho_config` uses as a normal production dependency for the configured
   target and features, recording its version, source identity, and checksum.
2. Follow the `clap` → `clap_builder` normal dependency edge to exactly one
   `clap_builder` identity.
3. Cross-check both package identities and the edge against
   `cargo metadata --locked --offline` for the same target and feature
   selection.
4. Fail closed on lock/metadata disagreement, a source override, multiple
   candidates, a missing or unexpected edge, or an unsupported identity, before
   any count is compared.
5. Locate the source defining `ErrorKind` through metadata's `manifest_path`.
   Never infer it from registry directory names.
6. Parse that source with `syn` and count every top-level variant without
   assigning meaning to any of them. Audit attributes and `cfg` conditions; a
   declaration shape whose compiled inventory cannot be established
   exhaustively fails closed.
7. Compare the parsed count with the reviewed `CLAP_ERROR_KIND_COUNT` test
   constant.
8. In the matrix test, assert the matrix length against the same constant at
   compile time with
   `static_assertions::const_assert_eq!(CLAP_ERROR_IDS.len(), CLAP_ERROR_KIND_COUNT)`.

The reviewed identity and its total are explicit test data beside the matrix
test, not generated output and not a build input. The gate owns the comparison;
`CLAP_ERROR_IDS` owns the classification.

The two comparisons answer different questions. Source growth changes the
parsed count, so the publication test fails until the total is reviewed and
updated. Once the total is updated, a matrix that omits the new variant, or
duplicates another to preserve its length, fails the const assertion or the
semantic inventory test. Neither comparison parses source on its own.

## Known risks and limitations

- **Workspace-only assurance.** A consumer that resolves a different
  `clap_builder` receives no identity assurance. It keeps the shipped matrix
  and the design's §6.3 fallback, and its build is never rejected.
- **Two-step review.** A new variant passes neither check until the reviewed
  total is updated, and the matrix is corrected only after that. The pair is a
  gate for maintainers, not a single atomic assertion.
- **Selection-specific.** The check covers the configured target and feature
  selection. Another feature combination is not validated.
- **Local source required.** The check needs the resolved registry source
  present, and runs with `--locked --offline`. Continuous integration compiles
  that source anyway, so this adds no network dependency.
- **Conditional declarations.** A `cfg`-gated or macro-generated variant fails
  closed and requires an explicit source-support decision rather than a guess.

## Consequences and validation

When clap adds a variant, the publication test fails and reports the parsed
count against the reviewed count. A maintainer updates the total and adds the
matrix row together, choosing a translated identifier or the `DisplayOnly`
sentinel. No `build.rs`, `rustc-env` value, or `OUT_DIR` artefact is introduced.

`syn`, `cargo_metadata`, and `static_assertions` are test-only dependencies
with caret requirements. Negative controls required by the ExecPlan alter the
checksum, source identity, or facade edge and require identity failure; supply
an unknown identity and require a fail-closed diagnostic; alter only the parsed
count and require the publication comparison to fail; change the total without
the matrix and require compile failure; and introduce a duplicate with an
omission at unchanged length and require the semantic inventory test to fail.

## Alternatives rejected

- **Build script (the superseded §6.1.1).** It cannot see consumer resolution,
  so it binds an unrelated lockfile or rejects valid consumers.
- **Workspace-only build script.** This is a consumer escape hatch that emits
  nothing for most builds, so the check becomes vacuous exactly where it was
  claimed.
- **A version constant or `links` key from clap.** Not available today; it would
  simplify the gate and should be revisited if upstream adds one.
- **A bridge crate declaring `links`.** Such a crate's own build script is
  equally blind to its dependent's resolution, so its metadata value would be a
  hard-coded literal.
- **Manual audit only.** Silent fall-through is the defect being fixed.

## References

- [CLI localization design](../cli-localization-design.md), §§6.1, 6.2, and 12.
- [ExecPlan 11.2.1](../execplans/11-2-1-ship-en-us-translations-for-clap-stable-error-matrix.md),
  "M0 outcome".
- [cargo#3946: workspace access from dependencies](https://github.com/rust-lang/cargo/issues/3946).
- [cargo#9661: reliable target-dir discovery](https://github.com/rust-lang/cargo/issues/9661).
- [cargo#15272: `cargo metadata` in build scripts honours the packaged lock](https://github.com/rust-lang/cargo/issues/15272).
- [Draft PR #588](https://github.com/leynos/ortho-config/pull/588).
