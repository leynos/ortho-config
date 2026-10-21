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

EP-M3 acceptance evidence, per criterion:

| Criterion                                                         | Evidence                                                                                                                                            |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Direct runtime consumer builds and tests                          | `workspace_runtime_consumer_builds_and_tests`                                                                                                       |
| Existing facade consumer still builds and tests                   | `workspace_facade_consumer_builds_and_tests`                                                                                                        |
| Both import paths name the same types                             | `workspace_mixed_consumer_builds_and_tests`, which compiles cross-assignments                                                                       |
| Published graph behaves the same                                  | `every_packaged_consumer_shape_builds_and_tests`                                                                                                    |
| No forbidden package resolves                                     | `no_forbidden_package_appears_in_a_workspace_runtime_consumer`, `packaged_runtime_consumer_has_no_facade_closure`                                   |
| The closure is not over-shrunk                                    | `assert_runtime_normal_edges` and `assert_root_edges`, which both fail on a *missing* edge too                                                      |
| A negative fixture fails for the injected dependency and names it | `negative_controls_reject_every_injected_edge`, `negative_controls_name_the_injected_dependency`, `an_unrelated_failure_does_not_satisfy_a_control` |
| Packaged manifests carry the right edges                          | the four `manifests::` tests                                                                                                                        |
| The render and the resolved graph agree                           | `the_rendered_consumer_is_the_package_the_closure_reads`                                                                                            |
| A focused entry point exists                                      | `make boundary`, pinned by `boundary_target_test.py`                                                                                                |

Proof inventory: not applicable. EP-M3 introduces no CrossHair, Kani, Verus, or
LemmaScript artefact. Its claims are about a resolved dependency graph and a
compiled consumer, which are checked by building and resolving the real graph
rather than by a model of it; a model checker over crate names would restate
the fixture it reads.

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

EP-M3 measured that cost: +24 crates and +13.25 s on a from-scratch macro
build, about 10 %, and nothing on an incremental one. The figure is a
warm-cache delta, not a download cost, and it was taken under a fully held
admission pool with sccache unavailable for part of the HEAD run. Treat it as
an upper bound.

The boundary suite writes 1.7 G under `target/localization-boundary`, and the
CI runner is sized for disk rather than time. `discard_build_trees.py` cannot
name that tree — it accepts one relative component directly under the target
root and the workflow's invocation is pinned — so the tree is discarded only by
`cargo clean` or by hand. On a 150 GB runner against 1.7 G this is not a disk
risk, but it is a local one on a full workstation.

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
- [x] 2026-10-21: EP-M3 boundary fixtures, package graph, and measurements
  built and green locally; the `make boundary` target and its workflow contract
  are in place. Committed as `f357813d`, after two gate repairs (14 Clippy
  findings, then one Whitaker `bumpy_road_function`) and a clean re-run of all
  seven gates on tree `7b2ce374`. The milestone CLI review has run; its
  findings are being dispositioned, so the milestone checkbox stays open until
  none is outstanding.
- [ ] EP-M3: Boundary fixtures, package graph, measurements, gates, review.
- [ ] EP-M4: Release tooling, documentation, publication checks, review, draft
      PR.

### EP-M3 reconnaissance (2026-10-21)

Read-only survey before any EP-M3 tracked edit. All probes ran under the
git-ignored `target/l10n-m3-probe` and were then deleted; no workspace file
changed.

- **CI budget is exactly consumed.** `build-test` has a 165-minute ceiling
  (9,900 seconds). Its watchdog plus outside-watchdog allowance plus ceiling
  margin already total 2×2,700 + 3,600 + 900 = 9,900, so no further
  coverage-invoking step fits in that job. Any new EP-M3 gate must be a
  Makefile target that the existing Linux leg already runs, or a new job with
  its own budget entry.
- **Closed step-condition set.**
  `tests/workflow_contracts/runner_placement_test.py` enumerates all 16
  platform-keyed or always-guarded `if:` strings and asserts set equality. A new
  `build-test` step carrying a platform condition must be registered there.
  `discard_build_trees.py` accepts only single relative path components
  directly beneath the target root, so a new build tree cannot be added to its
  prune list without a matching script change.
- **Doctest-collection obligation.** `PYTEST_FLAGS` is a closed list checked by
  `scripts/tests/test_doctest_collection.py`, which fails if any
  `scripts/**/*.py` containing `>>>` is unnamed. A new script with doctests
  must be appended to that variable.
- **Unaffected closed sets.** New `ortho_l10n` tests and fixtures do not touch
  `mutation_testing_test.py` (`EXPECTED_WITH` paths and exclude globs),
  `trybuild_tier.py` (configuration and `cargo-orthohelp` directories only), or
  `documentation_budget_test.py` (shared-cache group and serial names), unless
  the new target is deliberately added to the `orthohelp_cache` group.
- **Packaging recipe proved.**

  ```sh
  cargo package --offline -p ortho_l10n -p ortho_config --allow-dirty \
    --config 'patch.crates-io.ortho_config_macros.path="ortho_config_macros"'
  ```

  This succeeds in 31 seconds. `--no-verify` over all three crates succeeds in
  1 second. `-p ortho_config_macros` alone with only the `ortho_l10n` patch
  verifies in 10 seconds, and its child build compiles `ortho_l10n` from the
  workspace path. The "package collision in the lockfile" seen earlier occurs
  only when the macro crate is *verified* alongside another selected package.
- **Published manifest shape.** Packaged manifests rewrite internal edges to a
  bare `version = "0.9.0"` with no `path`, expand `[lints] workspace = true`
  inline into `[lints.clippy]`, `[lints.rust]`, and `[lints.rustdoc]`, and add
  `build = false` and `autolib = false` to `ortho_l10n`.
- **Packaged consumer pattern.** A consumer root with its own `[workspace]`,
  `ortho_l10n = "0.9.0"`, and `[patch.crates-io]` entries pointing at
  **absolute** paths into the extracted `.crate` contents resolves offline in 2
  seconds with all three `ortho_*` packages at `source = None`. Relative patch
  paths resolve against the consuming manifest and fail.
- **Standalone closure.** A fresh `ortho_l10n`-only consumer resolves to 36
  packages with no `ortho_config`, `ortho_config_macros`, `figment`, `clap`, or
  `ortho_config_test_helpers`.
- **Edge classification.** `resolve.nodes[].deps[].dep_kinds[].kind` reports
  normal edges as `[None]`, build edges as `["build"]`, and dev edges as
  `["dev"]`. `packages[].dependencies[]` omits `path` for registry dependencies
  and cannot classify edges.

### EP-M3 boundary suite (2026-10-21)

Twelve files, 2,390 lines as committed in `f357813d` (the counts below are the
committed ones). The declared `[[test]] name = "boundary"` target in
`ortho_l10n/Cargo.toml` reaches `tests/boundary_tests/mod.rs`; the fixtures
below it are rendered foreign package roots, not tests.

| File                                                 | Lines           | What it owns                                          |
| ---------------------------------------------------- | --------------- | ----------------------------------------------------- |
| `boundary/fixture_cargo/mod.rs`                      | 372             | disposable root, deadline-bounded child, TOML reading |
| `boundary/fixture_cargo/environment.rs`              | 143             | the closed child environment                          |
| `boundary/fixture_cargo/metadata.rs`                 | 229             | the `cargo metadata` document contract                |
| `boundary/consumer/mod.rs`                           | 195             | consumer manifests, workspace and packaged            |
| `boundary/consumer/library/mod.rs`                   | 289             | the three consumer bodies                             |
| `boundary/closure/mod.rs`                            | 316             | forbidden names, expected edges, negative controls    |
| `boundary/package_set/mod.rs`                        | 293             | the two-invocation packaging recipe                   |
| `boundary_tests/{closure,consumer,manifests}/mod.rs` | 210 / 145 / 155 | the 15 contracts                                      |
| `boundary/mod.rs`, `boundary_tests/mod.rs`           | 29 / 14         | module wiring                                         |

Green runs, three regimes, all 15 tests:

- `cargo test -p ortho_l10n --test boundary --offline` (default parallelism,
  warm): `15 passed … finished in 115.25s`. The two packaged-runtime contracts
  each ran past 60 s and both trigger the same `OnceLock` packaging step.
- The same suite cold, single-threaded: `finished in 71.06s`.
- `cargo nextest run -p ortho_l10n -E 'binary(boundary)'` (process per test):
  `Summary [70.144s] 15 tests run: 15 passed (2 slow)`, slowest 70.143 s.

70 s sits far inside nextest's 600 s per-test allowance, so **no nextest
override is added**. The first figure is larger than the third only because
libtest's in-process threads contend for one shared packaging `OnceLock`; that
is a scheduling property of the harness, not suite cost.

The suite is 1.7 G on disk (938 M `cargo-target`, 745 M `packaging-target`),
all under the git-ignored `target/localization-boundary`. `BOUNDARY_ROOT`
appears at three sites, all inside `ortho_l10n/tests/`.

**The target is already in CI.** `generate-coverage` defaults to
`cargo llvm-cov nextest` with `all-targets` off; nextest's default filter still
runs every test binary in the workspace, so both coverage steps execute
`boundary` today. No workflow edit is needed, and none is made: the
`build-test` ceiling is exactly consumed (2×2,700 + 3,600 + 900 = 9,900 s), the
platform-keyed step set is closed at 16 entries, and `DISCARD_COMMAND` is
pinned as an exact string that cannot name a new tree.

Offline resolution under `target/` means the children emit their own
`LLVM_PROFILE_FILE` profiles, which the coverage step never collects. CI
coverage is therefore unchanged in expectation, not degraded.

### EP-M3 Makefile target (2026-10-21)

`make boundary` runs
`RUSTFLAGS="-D warnings" cargo test -p ortho_l10n --test boundary --offline`.
It selects one package and one test target, so a boundary failure is
attributable, and it sets the same `-D warnings` the workspace denies. It does
not join `make test`'s recipe: `--all-targets` already reaches the declared
test target, and CI's coverage lane reaches it again, so a second copy would be
a third execution of one suite.

`tests/workflow_contracts/boundary_target_test.py` (5 tests) pins the recipe
against exactly the failures that would make it silently not the gate:
`--workspace`, `--all-targets`, a missing `--offline`, and a missing
`RUSTFLAGS`. Each was proved non-vacuous by mutating the Makefile, observing
that mutation's own test fail, and restoring the file byte-for-byte (`cmp`
after each restore). The full contract suite is 330 passed, one skipped.

### EP-M3 measurements (2026-10-21)

All figures are from fresh `CARGO_TARGET_DIR` roots and the shared Cargo
package cache. **The cache was warm; no figure is a cold-download claim.**

| Measurement                                             | Wall     | Crates compiled |
| ------------------------------------------------------- | -------- | --------------- |
| Macro crate, baseline `f99d9234` (no `ortho_l10n` edge) | 129.74 s | 14              |
| Macro crate, HEAD `21ad3e3f`                            | 142.99 s | 38 (24 more)    |
| Runtime consumer, `ortho_l10n` only, packaged 0.9.0     | 24.53 s  | 31              |
| Facade consumer, `ortho_config` only, packaged 0.9.0    | 62 s     | 103             |

The 24 crates the macro build gains are `ortho_l10n` and the packages reachable
only through it: `displaydoc`, `fluent-bundle`, `fluent-langneg`,
`fluent-syntax`, `intl-memoizer`, `intl_pluralrules`, `once_cell`,
`pin-project-lite`, `proc-macro-hack`, `rustc-hash`, `self_cell`, `smallvec`,
`thiserror`, `thiserror-impl`, `tinystr`, `tracing`, `tracing-attributes`,
`tracing-core`, `type-map`, `unic-langid`, `unic-langid-impl`,
`unic-langid-macros`, and `unic-langid-macros-impl`. Nothing was removed. The
count is the same whether it is read as the measured 38 − 14 compiled crates or
as a `cargo tree --prune ortho_l10n` set difference, which is why it is
recorded as 24 rather than 25. So the macro normal dependency is a real
host-build cost: **+13.25 s and +24 crates** over a from-scratch build, about
10 %, and **zero** for an incremental rebuild (0.10 s warm).

Consumer closure, same packaged set: the runtime root resolves **36** packages
with no `ortho_config`, `ortho_config_macros`, `figment`, `clap`, or
`ortho_config_test_helpers`; the facade root resolves **139**. Nearly four
times the graph is what the extraction removes for a formatting-only
application, which is the claim issue #566 exists to make true.

Both measurements ran under `build-limits` with every slot held and the
admission queue populated; the HEAD run recorded three
`the managed sccache server is not serving compilations` lines while baseline
recorded none, so the HEAD number is if anything pessimistic. A first attempt
placed a doubled `$PWD/…` target path and compiled into a stray tree; that tree
was deleted and those figures were discarded rather than reported.

### EP-M3 gate results (2026-10-21)

Three gate rounds, each against a frozen HEAD/tree pair, all through the
scrutineer with redirection rather than pipes so the status read was make's:

| Round | Tree       | Outcome                                           |
| ----- | ---------- | ------------------------------------------------- |
| 1     | `44164c01` | red: Clippy, 14 findings in the new tree          |
| 2     | `d61be027` | red: Clippy green, Whitaker `bumpy_road_function` |
| 3     | `7b2ce374` | green: all seven gates, both lint halves          |

Round 3, the certificate for the committed tree: `check-fmt` 1 s, `typecheck` 1
s, `lint` 32 s (rustdoc, Clippy, and Whitaker all ran and passed), `test` 425 s
with the boundary target green, `markdownlint` 14 s, `test-workflow-contracts`
5 s (330 passed, 1 skipped), `nixie` 1 s.

The failing logs are preserved beside the round-3 certificates, under `/tmp`, as
`lint-FAILED-21ad3e3f-tree-44164c01-…out` and
`lint-FAILED-whitaker-tree-d61be027-…out`, so each repair can be read against
the diagnostic that prompted it.

### EP-M3 milestone CLI review (2026-10-21)

`coderabbit review --agent` (CLI 0.9.1) ran over the committed tree at
`f357813d` and returned 17 findings across 11 files: 13 minor and 4 trivial.
Every finding was checked against the committed source rather than the
worktree. Two findings in the same round contradict each other, and the
contradiction was resolved first.

| #   | Finding                                          | Disposition                                      |
| --- | ------------------------------------------------ | ------------------------------------------------ |
| 1   | Workflow contract docstring spelling             | Valid; repaired in `b82f182e`                    |
| 2   | `closure` module prose spelling                  | Valid; repaired                                  |
| 3   | `package_set` module prose spelling              | Valid; repaired                                  |
| 4   | Workflow contract asserted a bare `-p` token     | Valid; now reads `_flag_values("-p")`            |
| 5   | `manifests` prose spelling                       | Valid; repaired                                  |
| 6   | `metadata` prose and error text spelling         | Valid; repaired                                  |
| 7   | Packaged set hard-codes the release version      | Valid; now `CARGO_PKG_VERSION`                   |
| 8   | Harness uses `std` paths, not `fs_utf8`/`camino` | Valid; migrated (settles #17)                    |
| 9   | Consumer hard-codes the release version          | Duplicate of #7; same constant                   |
| 10  | Test name and doc claim wider than the check     | Valid; renamed, doc narrowed                     |
| 11  | ExecPlan status statements out of date           | Valid; repaired                                  |
| 12  | ExecPlan crate delta inconsistent (25 vs 24)     | Valid; both derivations now recorded as 24       |
| 13  | Parameterize the negative controls               | Valid; merged into one `#[rstest]` case per edge |
| 14  | `fixture_cargo` prose spelling                   | Valid; repaired                                  |
| 15  | ExecPlan CI budget arithmetic                    | Valid; `2×2,700 + 3,600 + 900` at both sites     |
| 16  | `consumer/library` prose spelling                | Valid; repaired                                  |
| 17  | Remove the "unused" `fs_utf8` feature            | Rejected; premise falsified by #8                |

Two dispositions need their reasoning recorded because the suggestion was not
adopted verbatim.

Finding 17 asked for the `fs_utf8` feature to be dropped, on the premise that
the boundary tests use `cap_std::fs::Dir`. That premise was about to become
false: finding 8 asks for the opposite migration, and `cargo-orthohelp` already
uses `cap_std::fs_utf8` with `camino` across roughly twenty files. The
repository therefore has a precedent for the UTF-8 API, the paths in these
fixtures are rendered into manifests and compared as crate-root names, and the
feature is load-bearing after #8. Rejecting #17 and accepting #8 is the only
pairing under which both the rule and the precedent hold.

Finding 13 proposed deleting the redundant test rather than merging behaviour
into it. The two loops did duplicate work, but the second loop's assertion
subsumes the first's: a diagnostic that names the injected crate cannot be
empty. The repair keeps one `#[rstest]` function with a `#[values]` case per
injection, which drops the duplicated nested Cargo run, attributes a failure to
one injection instead of stopping at the first in enum order, and lets nextest
schedule the four remaining runs independently. `Injection::ALL` became dead
and was removed; `Dependency::ALL` is still read and stays.

Repairs are committed as `b82f182e`. The sequential gate run against that tree
and the reassessment follow, and the milestone checkbox stays open until the
reassessment reports no valid concern.

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

EP-M3: `cargo metadata` names a path package by `source#<version>` with no
name, while a registry package ends `#<name>@<version>`. A contract that read
the identity from the id fragment would work against crates.io and fail against
exactly the unpacked packaged roots these tests resolve, so `Metadata::edges`
resolves every `pkg` id through the package list instead.

EP-M3: the child Cargo invocation must put the subcommand first.
`--manifest-path` and `--offline` are per-command flags, not global ones, so
`cargo --manifest-path X metadata` is a usage error rather than a stricter form
of the same command.

EP-M3: both child pipes must be drained on their own threads before the
deadline wait. A nested `cargo test` prints far more than a pipe buffer holds,
so a runner that waits for the exit status first deadlocks and then reports a
deadline. Draining concurrently is what makes a reported deadline mean the
child itself stalled.

EP-M3: `cargo metadata` resolves a `[patch.crates-io]` entry that the graph
does not use and warns rather than failing. A runtime-only consumer therefore
carries three patch entries while only one applies. That is intended: the same
rendered manifest is reused by every shape, and a patch that silently failed to
apply shows up as a resolved `source` rather than as silence.

EP-M3: `packaged_manifests_carry_no_path_dependencies` reads the manifest as
TOML rather than as text. `[lib] path` and `[[test]] path` are keys in other
tables, and a textual search for `path =` reports them as dependencies — the
test was written that way first and failed on its own file before the
structured walk replaced it.

EP-M3: the coverage lane already executes the boundary target. This is the
reason no CI step was added for it, and it is worth restating because the
default reading of `all-targets` is off: that option narrows *feature*
coverage, not the set of test binaries nextest selects.

EP-M3: `make test` passed on this suite while `make lint` failed on it with 14
Clippy errors. `cargo test` does not run Clippy lints, so a green test run says
nothing about Clippy — the same false green is recorded in the issue-318 plan
for `shadow_reuse`. Because `lint` runs `lint-clippy` and `lint-whitaker` as
sibling prerequisites, Clippy's failure also left the Whitaker verdict
*unknown*, not passing.

EP-M3: four of the 14 findings came from one helper rather than from the
design. The runner bound the drain thread to `stdout`, then rebound the same
name to the joined output, and the `shadow_*` lints reject exactly that; the
readers are now `stdout_reader`/`stderr_reader`, and `drain` takes `&str`
rather than an owned `String` it immediately cloned.

EP-M3: repairing Clippy exposed a Whitaker finding that Clippy could not have
reached, because `lint` runs the two halves as sibling prerequisites and the
first failure aborts make. `bumpy_road_function` rejected two sibling `if`
blocks that each spelled one dependency line. Both writers now iterate
`Dependency::crates()`, a table of the internal crates each shape needs, so the
branch disappeared instead of being split into helpers: a fourth shape adds a
row, not a conditional. The general lesson is that a repaired lint half does
not grade the other half until the whole `make lint` target is re-run.

## Decision log

2026-10-15: Treat the user's detailed coding plan and explicit implementation
request as execution authorization. Retain compatibility resource data below
configuration rather than changing builder defaults. Retain Fluent arguments
and documentation IR ownership as explicitly required.

2026-10-21 (EP-M3): Add `make boundary` as a focused entry point and leave
`make test`'s recipe unchanged. `--all-targets` already reaches the declared
`[[test]] name = "boundary"` target, and the CI coverage lane reaches it
through nextest's default filter, so adding it to `make test` would be a third
execution of one suite and would hide which contract failed. The target is what
makes a boundary failure attributable; it is not a second gate.

2026-10-21 (EP-M3): Add no CI step and no nextest override. The `build-test`
ceiling is exactly consumed by its two coverage watchdogs and the surrounding
allowances, the platform-keyed step set is closed, and `DISCARD_COMMAND` is
pinned as an exact string. The suite's 70 s worst case sits far inside the 600
s per-test allowance, so an override would encode a contention figure as policy.

2026-10-21 (EP-M3): Keep the macro's `ortho_l10n` dependency as a normal
dependency and record the measured host cost rather than hiding it behind a
build-script or feature gate. The 24 extra crates are the price of one fallible
identifier owner shared by macro-expansion time and runtime; the alternative is
a second copy of the normalizer, which is the defect the extraction exists to
remove.

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

EP-M3 implementation is complete and its evidence is recorded above: 15
boundary contracts green under three scheduling regimes, the negative controls
proved to fail for their injected dependency by name, the packaged set built
from two `cargo package` invocations, the macro host cost measured against
`f99d9234`, and the focused `make boundary` target pinned by a workflow
contract. The sequential gate run passed on tree `7b2ce374` and the milestone
CLI review has since run, so what remains before the checkbox closes is that
review round's disposition and repair. The measured +24-crate macro closure is
recorded as a risk with an upper-bound caveat rather than as a settled
performance claim.

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
