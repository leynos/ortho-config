# Extract the reusable localization runtime (#566)

## Purpose and conformance basis

Applications such as Netsuke must be able to load and format catalogues using
`ortho_l10n` without compiling configuration discovery, derives, Figment, or
Clap. The supplied issue #566 coding plan authorizes implementation. This
living plan records its four milestones and evidence. The baseline is
`f99d9234f6d72e6b1a489b82f35ff1c5eb7228a7` (workspace version 0.9.0). The
governing design is `docs/cli-localization-design.md` and ADR-006. PR #536
supersedes #420; Netsuke #779 concerns documentation IR 2.0, not the build
graph. Live readback confirms #536 is merged (3479c735), and Netsuke #779
remains open. The shared trait and documentation contract are already in this
baseline; no prerequisite branch or cross-repository change is needed.

## Constraints

Preserve the existing public root paths and nominal type identity. Keep Fluent
arguments, error variants and messages, embedded defaults, and generated macro
paths. Keep Clap adapters, `OrthoConfigLocalization`, `ArgLocalizationIds`, and
the documentation IR in `ortho_config`. No runtime dependency may point back
from `ortho_l10n` to configuration or Clap. Use version-plus-path internal
edges. Never isolate the Cargo cache or bypass compile admission. Build targets
belong outside `/tmp`; temporary logs belong in `/tmp`. Gates run sequentially.

## Compatibility contract

The `ortho_config` root exports `ArgLocalizationIds`, `FluentBundleSource`,
`FluentLocalizer`, `FluentLocalizerBuilder`, `FluentLocalizerError`,
`FormattingIssue`, `LocalizationArgs`, `LocalizeCmd`, `LocalizedParse`,
`Localizer`, `NoOpLocalizer`, `OrthoConfigLocalization`, `WithBase`,
`clap_error_formatter`, `localize_clap_error`,
`localize_clap_error_with_command`, `message_id_for`, `parse_localized_command`,
`LanguageIdentifier`, and `langid`.

`Localizer: Send + Sync` is object safe. `lookup` accepts `&str` and
`Option<&LocalizationArgs<'_>>` and returns an owned `Option<String>`; `locale`
returns `Option<&LanguageIdentifier>`. `message` supplies the caller fallback.
`LocalizationArgs<'value>` is `HashMap<&'value str, FluentValue<'value>>`.

`FormattingIssue` exposes owned `id`, `locale`, `source`, and `Vec<FluentError>`
`errors`. Its reporter is `Arc<dyn Fn(&FormattingIssue) + Send + Sync>`; the
default emits a tracing warning and installs no subscriber.
`FluentBundleSource` is `Consumer` or `Default`. `FluentLocalizerError`
variants are `UnsupportedLocale { locale }`,
`Parser { locale, catalogue, errors }`, and
`Registration { locale, catalogue, errors }`; retain their display text. The
literal display templates are:

```text
no embedded Fluent resources exist for locale {locale}
failed to parse {catalogue:?} resources for {locale}
failed to register {catalogue:?} resources for {locale}
```

Parser errors remain `Vec<fluent_syntax::parser::ParserError>`; registration
and formatting errors remain `Vec<fluent_bundle::FluentError>`. The reporter is
synchronous and receives a borrowed issue; lookup reports a failed bundle
before attempting the next bundle.

Builders load defaults before consumer resources. A supplied concurrent Fluent
bundle replaces resource strings, including invalid strings; its reporting
locale is the builder locale. Resource registration conflicts fail. Language
`en` selects English resources, `ja` Japanese resources; others fail with
`UnsupportedLocale` when defaults are enabled, even with consumer resources.
The bundle retains the requested locale: `en-GB` reports `en-GB` while using
English default copy. Disabling defaults permits unsupported locales and an
empty localizer.

Lookup tries the consumer then defaults, with exact identifier before replacing
dots with hyphens. A formatting error reports the original identifier and
advances to the next bundle without retrying another identifier in that bundle.
Exhaustion returns `None`. Catalogue preprocessing changes top-level dotted
message declarations only, preserving bodies, terms, and attributes.

Identifier segments allow ASCII letters, digits, underscores, and hyphens;
ASCII case folds to lowercase. Dotted suffixes split into segments joined by
hyphens. Empty paths, empty segments, invalid characters, and nonletter roots
panic with the existing text. Case-only names collide. The extracted fallible
API must own this convention; runtime panic and macro diagnostics wrap it.
Preserve these literal panic templates, including the debug-formatted input:

```text
Fluent identifier must start with an ASCII letter: missing command root
invalid Fluent identifier segment: segment must not be empty
invalid Fluent identifier segment: {raw:?}
Fluent identifier must start with an ASCII letter: {id:?}
```

## Milestones and recovery

EP-M1 characterizes the contract with pure runtime tests before production
changes. Run all code gates, commit, then request `coderabbit review --agent`.
Resolve all valid concerns before EP-M2.

EP-M2 extracts the publishable crate and integrates both configuration and
macros atomically (coding tasks 2 and 3). Move pure tests and catalogues, add
one fallible identifier owner, and preserve generated tokens and snapshots. Run
focused agreement and public-path tests plus all gates, commit, and review.
Recovery is reverting the atomic commit; no wrapper traits are introduced.

EP-M3 proves standalone and packaged boundaries (coding task 4). Create direct
and mixed consumer roots, metadata closure checks with negative injections, and
Makefile/CI targets. Package all three crates together. Measure equivalent
consumers and baseline macro host timing with fresh target directories and the
shared Cargo cache. Record cache state honestly; do not claim cold downloads.
Run gates, commit, and review. A negative fixture must fail for its injected
forbidden dependency and show its path.

EP-M4 updates release order, internal dependency version discovery, ADRs,
guides, README examples, and version freshness (coding task 5). Run publication
and documentation gates as well as code gates, commit, and review. Finally push
over verified SSH, refresh origin tracking, and create a draft PR with
`Closes #566` and a terminal Lody References section.

## Verification plan

Characterization examples cover finite builder, locale, lookup, formatting, and
registration cases; compile assertions cover `Send + Sync`. Existing property
tests cover identifier grammar, idempotence, and collisions. Agreement tests
connect derive constants, runtime paths, and documentation IR. Mixed consumers
prove nominal identity by passing values across import paths. Metadata graph
traversal proves the boundary for normal/build edges and root dev edges.
Injected normal/build/dev Clap edges and a test-helper dev edge provide
negative controls. Package consumers test published manifests outside workspace
feature unification. Cargo metadata and package overlay semantics are external
tool contracts to verify against official Cargo documentation; Fluent
parser/formatter internals are external assumptions exercised through real
resources. No new business theorem or unsafe invariant is introduced.

## Tolerances (exception triggers)

Stop for an unintended public API change, snapshot change, dependency cycle,
missing prerequisite that cannot be independently scoped, full disk, or compile
admission failure. Do not silently expand to backend neutrality, a new IR,
configuration policy changes, or an identifier-only feature split. Record
material host-build regressions as a follow-up option. Review rate limits use
the requested foreground `vsleep` interval before retrying.

## Risks

A macro normal dependency can add host compilation cost. Shared package caches
and compile admission can distort timings; record those conditions. Packaged
same-version edges need Cargo's local registry overlay. Full workspace tests
may contain unrelated infrastructure failures; preserve logs and distinguish
unavailable infrastructure from source failures.

## Progress

- [x] 2026-10-20: Rebased `make check-fmt`, `make test`, and
  `make typecheck` pass. The four previously timing-out documentation examples
  pass; their suite records 50 passed, 14 ignored, and zero failures in 164.84
  seconds. Python tests record 115 passed and five skipped.
- [x] 2026-10-20: Rustdoc passes, but Rust 1.99 Clippy rejects two negated
  emptiness assertions in the characterization tests. Replace them with
  comparisons against empty vectors, retaining the same condition while
  improving failure diagnostics. The refreshed formatting, lint (including
  rustdoc, Clippy, and Whitaker), typecheck, full tests, Markdown/spelling, and
  diagram gates all pass. Canonical logs are
  `/tmp/issue566-rebase-assertions-<gate>-issue-566-extract-reusable-localisation-runtime-into-ortho-l10n.out`.
  EP-M1 commit and CodeRabbit assessment remain pending.

- [x] 2026-10-20: At the user's direction, advance the branch from
  `f99d9234f6d72e6b1a489b82f35ff1c5eb7228a7` to supervision PR #607's
  `b2fbf0fffc1fe0e8fbabc6d51f83067783c43771`. The verified remote branch is
  `origin/harden-documentation-build-supervision-20261015`; the supplied
  `onharden-…` spelling does not exist. There are no committed child changes to
  replay. Verified recovery copies and a retained stash preserve all four
  outstanding files. Rebase and patch restoration complete without conflicts;
  target lockfile and documentation improvements remain intact.
- [x] 2026-10-20: Validate the rebased candidate with sequential formatting,
  tests, typechecking, lint, Markdown, and diagram gates. Previous gate results
  below describe the old baseline only. Current instructions require submitting
  builds to healthy shared admission even when other work is queued; historical
  idle-host waits below are superseded. Commit only after required gates pass,
  review EP-M1, then publish with an explicit lease. The remote issue branch
  was absent at the pre-rebase lease readback.
- [x] 2026-10-15: Read instructions and requested skills; rename branch/session.
- [x] 2026-10-15: Record baseline contract before production edits.
- [x] 2026-10-15 18:06 UTC: Characterization tests written; formatting and
  refreshed full workspace typecheck pass.
- [x] 2026-10-15 18:40 UTC: Rustdoc and Clippy pass. Whitaker is running;
  test and documentation gates remain pending. No commit or CodeRabbit review
  has been requested.
- [x] 2026-10-15 19:16 UTC: Full `make lint` passes with exit status zero,
  including rustdoc (9 minutes 7 seconds), Clippy (22 minutes 58 seconds), and
  Whitaker (36 minutes 18 seconds). `make test` is running.
- [x] 2026-10-15 20:02 UTC: The first 258 unit tests pass. Integration tests
  are building isolated fixture targets under `target/gate-tmp`; the full test
  result is pending. Child builds are present and compile admission remains
  active. No timeout or source failure has been reported.
- [x] 2026-10-15 20:40 UTC: All three agent-context contracts and all nine CLI
  dispatch tests pass. Compile-time fixture builds continue through shared
  cache and admission waits; no failure has been reported. The full test gate
  and EP-M1 review remain pending.
- [x] 2026-10-15 21:34 UTC: All four compile-time cases pass without snapshot
  changes (suite elapsed 4,406.92 seconds). Rendering and policy golden tests
  pass; five agent-context golden cases are building fixtures. Full gate
  completion remains pending.
- [x] 2026-10-15: All 18 golden tests pass (944.86 seconds), and all three
  nested-subcommand end-to-end cases pass (430.10 seconds). Five of six
  policy-report contracts pass; the final combined-output fixture is still
  compiling. Fresh child artifacts confirm forward progress. The complete
  `make test` result is still pending.
- [x] 2026-10-15: All six policy-report contracts pass (898.11 seconds), and
  all 25 `cargo-orthohelp` BDD scenarios pass (1,820.05 seconds). The gate has
  advanced into example and remaining workspace tests without a failure.
- [x] 2026-10-15: All eight new characterization cases pass. The
  `ortho_config` unit suite passes 293 tests with four existing ignored tests.
  The full gate is now checking configuration compile-fail fixtures after a
  successful 15-minute 13-second nested check.
- [x] 2026-10-15: All 33 configuration UI diagnostic cases pass without
  snapshot changes. A second nested fixture build passes in 14 minutes 2
  seconds; the public API contract fixture is still compiling. The full gate
  remains pending with no reported failure.
- [ ] EP-M1 test retry: `make test` stops with four documentation-example
  workspace-membership errors. Setting `TMPDIR` beneath the checkout caused
  Cargo to discover the enclosing workspace for temporary packages. Move the
  gate temporary root to the session-specific directory beneath
  `/home/leynos/.cache/ortho-config-gates/`, outside both the checkout and
  `/tmp`. Verify the affected test target, then rerun the full gate. No
  production or test-source fix is indicated by these diagnostics.
- [ ] EP-M1 infrastructure wait: the external-root focused retry fixes
  workspace discovery but all four example builds time out at 600 seconds (14
  pass, four fail, five ignore). Supervisor readback is healthy with all four
  slots held, approximately 200 queued requests, and sccache unavailable.
  Preserve both red logs. Inspect supported runner settings and wait for lower
  contention; do not stretch timeouts, bypass admission, or commit with a red
  full gate. Finish documentation-only checks while Rust gates are idle.
- [x] Final formatting, Markdown, spelling, and Mermaid gates pass without
  unrelated changes. Subsequent minute-spaced admission samples fall from
  approximately 200 queued requests to 114, 93, 28, 27, 10, then 10.
- [ ] Retry the focused documentation examples under the improved queue with
  command-scoped `RUST_TEST_THREADS=1`; retain the fixed 600-second limit,
  shared Cargo cache, and compiler admission. This serializes independent test
  cases while retaining the explicit concurrent-interleaving test. Run the full
  gate with the same setting only after focused success.
- [ ] 2026-10-16: The serialized retry's first example fails; remaining
  cases are still running. A bounded sample shows its Cargo child waiting for
  `.package-cache`, with no compiler or fresh target artefacts. The conflicting
  writer and mutation-lock reader are unrelated to the child ancestry,
  falsifying a direct parent/child Cargo lock-cycle hypothesis. Lower compiler
  queue pressure alone has not removed package-cache contention. Await the
  exact retry summary before another action.
- [ ] 2026-10-16: Serialized focused retry finishes with 15 passed, three
  failed, five ignored (2,376.04 seconds). Aliased dependency, standard Rust
  examples, and concurrent interleavings each time out while building at 600
  seconds; the external-subcommand example passes. The full test gate is still
  red. Latest readback has 23 queued compilers and 26 cache-lock waiters. Await
  reduced cache contention before any further retry. The hard-coded timeout has
  no supported operator override; do not broaden this extraction into an
  unrelated harness or service change.
- [ ] EP-M1: Complete remaining gates, commit, and review.
- [ ] EP-M2: Extraction and integration, gates, commit, review.
- [ ] EP-M3: Boundary fixtures, package graph, measurements, gates, review.
- [ ] EP-M4: Release tooling, documentation, publication checks, review, draft
      PR.

## Surprises & discoveries

The requested locale determines bundle metadata even when embedded resources
are selected by language. Preserve this distinction during extraction.

The initial full typecheck passed in 44 minutes 36 seconds. Shared
package-cache locks and compile-admission queues dominated wall time; the
managed compiler cache reported itself unreachable and fell back to uncached
compilation. The refreshed check passed with exit status zero in 4 minutes 21
seconds. Later gates use six Cargo jobs and retain `RUSTFLAGS="-D warnings"`.
Internal throwaway builds must use the session-specific external cache
directory through command-scoped `TMPDIR`, while logs remain in `/tmp`. The
original `target/gate-tmp` choice incorrectly exposed temporary example
packages to the enclosing Cargo workspace; use an external root for subsequent
runs. These are infrastructure observations, not localization performance
measurements.

Official
[Cargo packaging documentation](https://doc.rust-lang.org/cargo/commands/cargo-package.html)
describes multiple package selectors and registry-aware lockfile generation.
The installed Lading pipeline packages crates individually, so the new consumer
gate must independently verify the multi-package graph. Keep any newer Cargo
invocation scoped to packaging; retain the repository toolchain pin.

## Decision log

2026-10-15: Treat the user's detailed coding plan and explicit implementation
request as execution authorization. Retain compatibility resource data below
configuration rather than changing builder defaults. Retain Fluent arguments
and documentation IR ownership as explicitly required.

## Outcomes & retrospective

The compatibility contract and eight new characterization cases are written;
all eight pass. Formatting, typechecking, rustdoc, Clippy, Whitaker, Markdown,
spelling, and diagram validation pass. The rebased full test gate passes,
resolving the earlier documentation-example subprocess timeouts. All required
gates pass after two assertion-only Clippy corrections for the target's newer
Rust toolchain. No production extraction, commit, CodeRabbit review, push, or
PR has occurred. No acceptance item is yet claimed complete.

Revision note (2026-10-20): The user explicitly selected the open supervision
PR as the new base, replacing the earlier wait for its merge into main. The
base advance preserves the characterization work and requires fresh gate
results before the first milestone can be committed or reviewed.

Revision note (2026-10-20, validation): Record the successful rebased gate run
and the two Clippy assertion corrections. The documentation-example blocker is
resolved on PR #607's base. EP-M1 still needs its gated commit and milestone
assessment before extraction begins.
