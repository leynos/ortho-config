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

  ```text
  /tmp/issue566-rebase-assertions-<gate>-issue-566-extract-reusable-localisation-runtime-into-ortho-l10n.out
  ```

  EP-M1 is committed as `fe5b5334`. CodeRabbit completed its four-file review
  and reported three minor documentation concerns. Commit `b50d56b0` resolves
  them; the subsequent six-file reassessment reports zero findings.

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

### Historical gate runs

These earlier failures and retries are resolved by the successful 20 October
validation above. Retain them as evidence of the original blocker and failed
approaches; they are no longer outstanding work.

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
  compiling. Fresh child artefacts confirm forward progress. The complete
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
- [x] EP-M1 test retry: `make test` stops with four documentation-example
  workspace-membership errors. Setting `TMPDIR` beneath the checkout caused
  Cargo to discover the enclosing workspace for temporary packages. Move the
  gate temporary root to the session-specific directory beneath
  `/home/leynos/.cache/ortho-config-gates/`, outside both the checkout and
  `/tmp`. Verify the affected test target, then rerun the full gate. No
  production or test-source fix is indicated by these diagnostics.
- [x] EP-M1 infrastructure wait: the external-root focused retry fixes
  workspace discovery but all four example builds time out at 600 seconds (14
  pass, four fail, five ignore). Supervisor readback is healthy with all four
  slots held, approximately 200 queued requests, and sccache unavailable.
  Preserve both red logs. Inspect supported runner settings and wait for lower
  contention; do not stretch timeouts, bypass admission, or commit with a red
  full gate. Finish documentation-only checks while Rust gates are idle.
- [x] Final formatting, Markdown, spelling, and Mermaid gates pass without
  unrelated changes. Subsequent minute-spaced admission samples fall from
  approximately 200 queued requests to 114, 93, 28, 27, 10, then 10.
- [x] Retry the focused documentation examples under the improved queue with
  command-scoped `RUST_TEST_THREADS=1`; retain the fixed 600-second limit,
  shared Cargo cache, and compiler admission. This serializes independent test
  cases while retaining the explicit concurrent-interleaving test. Run the full
  gate with the same setting only after focused success.
- [x] 2026-10-16: The serialized retry's first example fails; remaining
  cases are still running. A bounded sample shows its Cargo child waiting for
  `.package-cache`, with no compiler or fresh target artefacts. The conflicting
  writer and mutation-lock reader are unrelated to the child ancestry,
  falsifying a direct parent/child Cargo lock-cycle hypothesis. Lower compiler
  queue pressure alone has not removed package-cache contention. Await the
  exact retry summary before another action.
- [x] 2026-10-16: Serialized focused retry finishes with 15 passed, three
  failed, five ignored (2,376.04 seconds). Aliased dependency, standard Rust
  examples, and concurrent interleavings each time out while building at 600
  seconds; the external-subcommand example passes. The full test gate is still
  red. Latest readback has 23 queued compilers and 26 cache-lock waiters. Await
  reduced cache contention before any further retry. The hard-coded timeout has
  no supported operator override; do not broaden this extraction into an
  unrelated harness or service change.

### Remaining milestones

- [x] EP-M1: Commit characterization (`fe5b5334`), resolve the three
  documentation findings (`b50d56b0`), and complete fresh CodeRabbit review
  with zero findings across all six files. Publish over SSH with an explicit
  absent-branch lease; HEAD, remote, and origin tracking all match `b50d56b0`.
- [x] 2026-10-20: Begin EP-M2. Record ADR-015 before introducing the crate
  and shared fallible identifier API, as repository abstraction policy
  requires. This brings the planned boundary ADR forward from EP-M4 without
  changing ownership or scope. CodeGraph impact identifies 12 direct and 30
  total identifier references; preserve their configuration re-export paths.
- [x] EP-M2: Runtime, embedded resources, and pure tests moved into
  `ortho_l10n`; configuration now re-exports actual types and retains adapters.
  Macros use the shared fallible API, including root validation and the common
  argument-fragment separator. Structured-error/panic and source-span
  regressions plus direct/facade identity checks are present. All gates pass
  and the milestone review's only valid findings are repaired; the clean
  milestone commit is the delivery point.
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

Official [Cargo packaging documentation][cargo-package] describes multiple
package selectors and registry-aware lockfile generation. The installed Lading
pipeline packages crates individually, so the new consumer gate must
independently verify the multi-package graph. Keep any newer Cargo invocation
scoped to packaging; retain the repository toolchain pin.

## Decision log

2026-10-15: Treat the user's detailed coding plan and explicit implementation
request as execution authorization. Retain compatibility resource data below
configuration rather than changing builder defaults. Retain Fluent arguments
and documentation IR ownership as explicitly required.

## Outcomes & retrospective

EP-M1 is complete, reviewed with zero outstanding findings, and published as a
remote branch at `b50d56b0`; no pull request exists yet, so "published" here
means branch-pushed over SSH with the origin tracking ref matching. EP-M2 is
complete: the runtime, resources, and pure tests now belong to `ortho_l10n`,
with actual-type configuration re-exports and shared macro normalization.
Focused runtime, macro, identity, and documentation-agreement tests pass, every
sequential gate passes on the corrected tree, and the milestone review's three
findings are dispositioned and repaired. A mid-implementation disk exhaustion
and a 312-character table-wrap defect both caused red gates; both are repaired
and recorded above. Standalone/package closure proof, measurements, release
tooling, and the draft PR remain EP-M3/EP-M4 work.

Decision log note (2026-10-21): The earlier guidance to use explicit result
branches at the Clap collision panic boundaries is superseded. Those sites now
delegate to `message_id_for`, matching the base branch and satisfying the
review. The rule that survives is narrower and is what the gate actually
enforced: do not introduce `unwrap_or_else` panic closures in this API. Panics
documented in public `# Panics` sections and routed through one fallible owner
remain the intended programmer-error behaviour.

Revision note (2026-10-20): The user explicitly selected the open supervision
PR as the new base, replacing the earlier wait for its merge into main. The
base advance preserves the characterization work and requires fresh gate
results before the first milestone can be committed or reviewed.

Revision note (2026-10-20, validation): Record the successful rebased gate run
and the two Clippy assertion corrections. The documentation-example blocker is
resolved on PR #607's base. At that point EP-M1 still needed its gated commit
and milestone assessment; both subsequently completed as recorded above.

[cargo-package]:
  https://doc.rust-lang.org/cargo/commands/cargo-package.html

Revision note (2026-10-20, review): Close the superseded infrastructure
checklist items in a labelled historical record and wrap long references. These
changes address the three documentation findings from the completed EP-M1
CodeRabbit assessment; production code remains unchanged.

Revision note (2026-10-20, EP-M2): Record accepted EP-M1 review and verified
publication. Bring the boundary ADR into the extraction milestone so the new
shared helper's ownership is documented before implementation.

Revision note (2026-10-20, extraction candidate): Move implementation and tests
with the compatibility catalogues, replace the paired normalizers with one
fallible owner, and update affected catalogue paths and architecture guidance.
The workspace lockfile changes only internal dependency edges and the new
package. Standalone/package closure proof and measurements remain EP-M3 work.

EP-M2 focused evidence: `ortho_l10n` passes 48 unit tests and ten doctests;
macros pass 276 tests. The attempted configuration command names fixture and
support files as Cargo targets, so Cargo rejects it before running tests. Use
their owning harnesses: `localization_trybuild`, `crate_path_trybuild`,
`localized_parse`, and `docs_ir`, alongside the direct identity and nested IR
targets. This is an invocation correction, rather than a source failure.

EP-M2 corrected focused configuration verification passes: crate aliases,
localization public-path and parser trybuild fixtures, two nominal-identity
checks, 12 localized-parse cases, eight docs IR cases, and nine nested IR
cases. No diagnostic snapshots changed. Full candidate gates and review remain
pending.

EP-M2 full-gate attempt: formatting and rustdoc pass. Clippy identifies two
unused imports left in the retained Clap test module after its pure tests
moved. Remove those imports and restart the sequential gates; this is test
support cleanup, with no runtime behaviour change.

EP-M2 cleanup retry: rustdoc passes after shared-cache waits. Clippy rejects
assertions in the result-returning identity test and two discarded `must_use`
values. Make the test infallible with explicit fixture construction and
exercise the typed arguments and reporter through a real formatted lookup.
Retain all cross-path assignments without lint suppressions; repeat the
sequential gates.

EP-M2 identity-test retry: rustdoc and Clippy pass. Whitaker rejects the
`unwrap_or_else` panic closure in the compatibility API. The API deliberately
retains programmer-error panics, while dynamic callers have the fallible API.
Use explicit result branches at that documented boundary and the two existing
Clap collision panic boundaries. Preserve exact error text and validation
order, without expects, exclusions, or lint suppressions. Repeat all gates.

Revision note (2026-10-20, capacity recovery): Preserve the failed EP-M2
integration log at `/tmp/issue566-m2-panic-boundary-test.out`; its two failing
agent-context cases report `No space left on device`, rather than a runtime
assertion regression. Record the user's capacity-restoration instruction and
restart validation without changing production code. The prerequisite PR #607
advanced to `680efaed` after the earlier rebase; inspect and incorporate its
build-supervision improvements before the next publication. Measurements use
`b50d56b0`, the immediate pre-extraction checkpoint with the same supervision
base and toolchain, rather than the historical initial baseline.

EP-M2 full-gate results: the candidate `81fa98d` passed typecheck (47 s), lint
(rustdoc, Clippy, and Whitaker, 88 s), full tests (2,049 passed, 0 failed, 29
ignored, 418 s; pytest 115 passed and five skipped), diagrams, and workflow
contracts. Formatting and Markdown both failed on the same defect: two
`docs/repository-layout.md` table rows measured 313 and 311 characters against
the 312-character column width that `mdtablefix` enforces. Re-running the
repository's own `mdtablefix --in-place` repaired exactly those two rows;
`make check-fmt` then reported 90 files unchanged, `make markdownlint` reported
zero issues with `current: typos.toml` (no rewrite), and the repaired candidate
`1eae3da2` carried the earlier green gates forward. The delta was whitespace in
one documentation file, and no test, `include_str!`, or documentation-example
registry reads it, so the carried results remain valid for the file classes the
delta reaches.

EP-M2 milestone review: CodeRabbit CLI 0.9.0 ran through the scrutineer against
`--base-commit 1334cc88` for 316 s, covered all 40 changed files, and returned
three trivial findings. Preserved evidence is
`/tmp/coderabbit-ep-m2-gjHPyN/coderabbit-agent.out`, with head-before and
reviewed-file listings beside it. Findings one and three are the same defect
reported twice: both ask the two `assert_unique_*_ids` helpers in
`ortho_config/src/localizer/clap_command/mod.rs` to call `message_id_for`
rather than reproducing its body. Finding two is a title/body contradiction:
its title says to remove the empty-path guard in
`ortho_l10n::try_message_id_for`, while its body says to keep it and document
why.

Both dispositions are valid. The branch itself introduced the duplicated
blocks; the base `1334cc88` called `message_id_for` at both sites, so the
repair restores the base form. This supersedes the earlier plan guidance to
place explicit result branches at the Clap collision boundaries. That guidance
generalized from a Whitaker rejection of an `unwrap_or_else` panic closure in
the compatibility API, which is a narrower rule than "no panic at these
boundaries": `message_id_for` contains the same `match`/`panic!` internally and
Whitaker accepts it. The empty-path guard stays, with the requested comment
stating that it preserves the legacy error ordering.

Repair gates on the corrected tree, run sequentially: `make check-fmt`,
`make typecheck`, `make lint` (rustdoc, Clippy, and Whitaker all clean),
`make test` (2,049 passed, 0 failed, 29 ignored; pytest 115 passed and five
skipped), `make markdownlint`, `make nixie`, and `make test-workflow-contracts`
(325 passed, one skipped) all pass. Canonical logs follow the
`/tmp/issue566-m2-cr-repair-<gate>.out` pattern. The tree did not move during
any of these gates.

EP-M2 milestone close: the repairs were amended into the milestone commit,
which replaced the working subject with a descriptive one. The scrutineer then
re-ran all seven gates sequentially against the frozen candidate `a4fc4435`,
with each gate's own exit status captured by direct redirection rather than
through a pipe. All seven pass, the tree stayed empty throughout, and the full
test gate again recorded 2,049 passed, 0 failed, and 29 ignored, with pytest at
115 passed and five skipped. A fresh CodeRabbit review against
`--base-commit 1334cc88` then completed in 229 s with zero findings over all 40
changed files. That coverage is set-equal to
`git diff --name-only 1334cc88..HEAD`, and it includes all three files changed
by the repair, so the earlier findings are confirmed resolved rather than
merely unreported. Evidence lives at `/tmp/coderabbit-ep-m2-final-U2Py6hNO/`
and under the `/tmp/issue566-m2-final-<gate>-…out` log names; the earlier
review bundle is preserved unmodified.
