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

| Criterion                                                         | Evidence                                                                                                                                                                              |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Direct runtime consumer builds and tests                          | `workspace_runtime_consumer_builds_and_tests`                                                                                                                                         |
| Existing facade consumer still builds and tests                   | `workspace_facade_consumer_builds_and_tests`                                                                                                                                          |
| Both import paths name the same types                             | `workspace_mixed_consumer_builds_and_tests`, which compiles cross-assignments                                                                                                         |
| Published graph behaves the same                                  | `every_packaged_consumer_shape_builds_and_tests`                                                                                                                                      |
| No forbidden package resolves                                     | `no_forbidden_package_appears_in_a_workspace_runtime_consumer`, `packaged_runtime_consumer_has_no_facade_closure`                                                                     |
| The closure is not over-shrunk                                    | `assert_runtime_normal_edges` and `assert_root_edges`, which both fail on a *missing* edge too                                                                                        |
| A negative fixture fails for the injected dependency and names it | `negative_controls_reject_every_injected_edge` (one `#[values]` case per injection, each asserting the diagnostic names the crate), `an_unrelated_failure_does_not_satisfy_a_control` |
| Packaged manifests carry the right edges                          | the four `manifests::` tests                                                                                                                                                          |
| The render and the resolved graph agree                           | `the_rendered_consumer_is_the_package_the_closure_reads`                                                                                                                              |
| A focused entry point exists                                      | `make boundary`, pinned by `boundary_target_test.py`                                                                                                                                  |

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

**The publish dry run cannot pass while `ortho_l10n` is unpublished.** lading
does not classify Cargo's `no matching package named` diagnostic, so the
dry-run override never sees the failure and the gate aborts on the first
dependent crate. This is an upstream defect
([leynos/lading#367](https://github.com/leynos/lading/issues/367)); the gate
must not be weakened locally to hide it. It is self-clearing once `ortho_l10n`
has been published once, and it is only reachable on the `workflow_dispatch`
path, so no pull-request check is red because of it. Tracked in "EP-M4
publication check blocked upstream".

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
- [x] EP-M3: Boundary fixtures, package graph, measurements, gates, review.
  All eight gates pass against the repaired tree `3c22ba57` plus the
  Markdown-only repair `71ac7873`, and the closing review's four unique
  findings are all base-owned (see "EP-M3 closing review"), so no concern
  against `#566`'s own work is outstanding. Closed 2026-10-22 with PR #607
  recorded as an open delivery dependency.
- [ ] EP-M4: Release tooling, documentation, publication checks, review, draft
      PR. In progress: publish order and its workflow contract are done (see
      "EP-M4 publish order"), as are version discovery and the guides. The
      publication check is blocked upstream, not locally (see "EP-M4
      publication check blocked upstream"); code gates, milestone review, and
      the draft PR remain.

### EP-M4 publish order (2026-10-22)

`lading.toml` listed four crates and the workspace publishes five: `ortho_l10n`
was never in the order, so `make publish-check` would not have packaged it at
all. Red was observed first. The contract is
`tests/workflow_contracts/publish_order_test.py`, four tests, run with
`pytest tests/workflow_contracts/`.

Writing the contract found three defects in itself before it found any in the
repository. Each is recorded because each one produced a *green* result that
meant nothing:

- The published predicate read `publish != []`, which treats `publish = false`
  as publishable, because `False != []` is true. It reported the example and
  all five fixtures as missing from the order. Cargo's rule is that `false`
  means never, a list means only those registries, and a missing key means
  anywhere; the predicate now spells out all three, which also matters because
  `test_helpers` uses the list form.
- A `path` dependency is written relative to the manifest that declares it. The
  first version resolved it against the process's working directory, so no edge
  ever matched, and the ordering test passed against an empty edge set. The
  check only became meaningful once the path resolved against the declaring
  manifest's directory.
- `ortho_config` declares the helper crate under the alias `test_helpers` with
  `package = "ortho_config_test_helpers"`. The edge table is keyed by the
  *published* name, so an assertion written against the alias failed. The
  correct behaviour was already right; the assertion was not.

The ordering check also had to learn one fact about the graph. `ortho_config`
depends on `ortho_config_macros`, and `ortho_config_macros` has a dev
dependency on `ortho_config`, so the pair is cyclic and no order satisfies
both. The cycle is not an artefact of this branch: `git show v0.9.0:` of both
manifests carries the same two edges, and v0.9.0 shipped with this order. The
check therefore exempts mutually reachable pairs only.

That exemption is dangerous in exactly one direction. `ortho_config` also
dev-depends on the helper crate, and *that* edge has no return path, so
`ortho_config_test_helpers` must still precede `ortho_config`. Commit
`e1ca1a4c` ("Publish test_helpers before ortho_config") exists because that
requirement was broken once: its message cites "cargo publish verification
requirements for dev dependencies that must resolve from crates.io during
package checks". A blanket "dev dependencies are not real edges" rule would
re-break it silently, because the workspace supplies the path either way. A
dedicated test pins the non-exemption, and it asserts the edge exists first, so
deleting the dependency cannot make the guard vacuous.

Evidence, all run before the change was committed:

| Probe                                   | Observation                                                                                       |
| --------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `ortho_l10n` absent from the order      | `missing: ['ortho_l10n']`, and nothing unexpected                                                 |
| `ortho_l10n` moved last                 | both real edges fire: `ortho_config`, `ortho_config_macros` depend on it                          |
| helper crate moved after `ortho_config` | `ortho_config` depends on it; the dedicated guard also fails (`assert 3 < 2`)                     |
| `lading.toml` restored after each probe | `diff -q` against the pre-probe copy, taken with `cp` because `git checkout --` reverts the index |

The final order is `ortho_l10n`, `ortho_config_macros`,
`ortho_config_test_helpers`, `ortho_config`, `cargo-orthohelp`. Four contract
tests pass.

### EP-M4 version discovery (2026-10-22)

`scripts/bump_version.py` mapped one package name to one dependency:

```python
dep = "ortho_config_macros" if package_name == "ortho_config" else None
```

That covered `ortho_config`'s dependency on the macro crate and nothing else.
`ortho_config` now also requires `ortho_l10n`, and it dev-depends on
`ortho_config_test_helpers`, which was never bumped either; `cargo-orthohelp`
requires both `ortho_config` and the helper crate. A whole-workspace release
would therefore have shipped members whose internal requirements named the
previous version.

Red was observed first: the new test
`test_member_bump_updates_every_internal_dependency` failed with
`dependencies.ortho_l10n kept a stale requirement` before any script change.

The fix derives the list instead of naming it. `_internal_dependencies(doc)`
returns every dependency declared with a `path`, across the normal, dev, and
build tables, deduplicated in first-seen order; `_set_version` now takes that
list. A dependency's identity is the table key, not its `path` value, so an
aliased dependency is handled. External requirements have no `path` and are
untouched, which is what keeps a third-party version like `serde = "1"` from
being rewritten.

Two checks beyond the unit test, because a synthetic test can agree with a
wrong rule:

| Check                                  | Observation                                                                                                                                                                      |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Derived edges from every real manifest | `ortho_config` -> `ortho_l10n`, `ortho_config_macros`, `test_helpers`; `cargo-orthohelp` -> `ortho_config`, `test_helpers`; each fixture -> `ortho_config`; `ortho_l10n` -> none |
| Full script run in a throwaway copy    | Every `path` requirement reaches the new version, in all three tables, and every changed line is a package version or an internal requirement                                    |

The throwaway run also showed `cargo-orthohelp`'s own version being replaced
with the workspace version. That is pre-existing and correct for this script:
the developer's guide reserves it for a whole-workspace release and documents
the per-crate release as a hand edit plus a `v<version>` tag.

Documentation updated in the same milestone: the developer's guide now states
the publish-order contract and the cycle exemption, and states that the bump
script discovers requirements; `CHANGELOG.md` records the new crate under
`## Unreleased`. `docs/design.md` already described the crate, and
`ortho_l10n/README.md` already documents the standalone use.

### EP-M4 publication check blocked upstream (2026-10-22)

`make publish-check` fails, and the failure is not in this workspace. It is the
first-publication case for a brand-new crate, and lading does not recognize the
diagnostic Cargo emits for it. Filed as
[leynos/lading#367](https://github.com/leynos/lading/issues/367); the evidence
and the reasoning for not working around it are recorded here.

The run: 169 s pre-flight, then `ortho_l10n` packaged successfully (1/5, 12.7
s), then `ortho_config_macros` failed with exit 101:

```text
error: failed to prepare local package for uploading

Caused by:
  no matching package named `ortho_l10n` found
  location searched: crates.io index
  required by package `ortho_config_macros v0.9.0`
```

The dry-run override was active in the same log
(`INFO: Allowing unpublished workspace dependencies during dry-run publish`),
and the order was satisfied: `ortho_l10n` is first in `lading.toml` and had
just been packaged. Neither was the problem.

Cargo has two index-miss diagnostics, and lading matches one:

| Index state of the sibling      | Cargo wording                                                     | lading  |
| ------------------------------- | ----------------------------------------------------------------- | ------- |
| Package present, version absent | `failed to select a version for the requirement \`x = "^0.8.0"\`` | handled |
| Package absent entirely         | `no matching package named \`x\` found`                           | ignored |

`_INDEX_MISSING_VERSION_MARKERS` in `cargo_output_adapter.py` requires
`failed to select a version for the requirement`, which the second wording does
not contain, so `parse_index_lookup_failure` returns `None`, the failure is
never typed as an index miss, and neither the override nor its ordering check
is consulted. The name regex would miss it too:
`no matching package named \`x\`` has no `=` to anchor on.

The gap is against lading's own documented contract. Its user guide, §"Dry-run
limitations with unpublished workspace dependencies", describes "crates
released together for the first time" and shows only the version-mismatch
diagnostic; the override text then promises the dry run "completes end-to-end".
The absent-package case is that same scenario and is not covered. The defect is
present unchanged at lading's `origin/main` (`16b2c1a`), so re-pinning the
Makefile's `LADING_REF` would not help.

Why this is this branch's failure rather than a pre-existing one: at the stack
base `1334cc88`, `ortho_config_macros` has no `ortho_l10n` dependency at all.
`a4fc4435` (`Extract localization runtime into ortho_l10n (#566)`) added it, so
this branch is the first to require a sibling that has never been published.
Earlier releases only ever needed the version-absent shape, which lading
handles.

No override is available in the interval: `cargo package` is invoked by lading
itself (as `cargo package --allow-dirty`, with no user-supplied arguments), and
a workspace `[patch]` would be stripped anyway because `lading.toml` sets
`strip_patches = "all"`. A patch-injected `cargo package` *does* pass on
today's tree, which is what isolates the defect to the classification rather
than to the manifests:

```sh
cargo package --offline -p ortho_config_macros --allow-dirty \
  --config 'patch.crates-io.ortho_l10n.path="ortho_l10n"'
# exit 0; the child build compiles ortho_l10n from the workspace path
```

Decisions taken:

- **Do not weaken the gate in this repository to make it green.** Deleting the
  step, skipping the crate, or pinning a local lading fork would hide the one
  check that sees what a published crate exports (issue #414 is this
  repository's own instance of that). The gate stays red until upstream is
  fixed or the crate has been published once.
- **The blocking condition expires on its own.** After `ortho_l10n`'s first real
  publication the package exists in the index and Cargo emits the
  version-mismatch wording, which lading already handles. The blockage is a
  property of a *first* release, not of the crate's steady state.
- **Exposure is limited to the dispatch path.** CI runs `make publish-check`
  only under `workflow_dispatch` (`.github/workflows/ci.yml`, step "Publish dry
  run"), so no pull-request check depends on it and PR #607 is unaffected.

### EP-M4 gate certificate (2026-10-22)

Run by the scrutineer, sequentially, against the frozen candidate
`f0d1feac4018478d13f8f1234ee4a4e36bfe7a33` with a clean tree. The milestone's
changed file classes are 4 Markdown, 3 Python, 1 TOML and no Rust, so the gate
set was chosen for those classes rather than for a Rust-shaped diff.

| Gate                           | Status | Duration |
| ------------------------------ | ------ | -------- |
| `make check-fmt`               | pass   | 1 s      |
| `make markdownlint`            | pass   | 12 s     |
| `make nixie`                   | pass   | 1 s      |
| `make test-workflow-contracts` | pass   | 4 s      |
| Python script suite            | pass   | 1 s      |
| `make lint-clippy`             | pass   | 2 s      |
| `make lint` (with Whitaker)    | pass   | 80 s     |

`make markdownlint` reported `current: typos.toml`, so the generated spelling
configuration was not rewritten. The workflow-contract suite recorded 338
passed and 1 skipped; the script suite recorded 117 passed and 5 skipped. The
run emitted eight notices that the managed sccache server was not serving
compilations and compiled uncached; per the host rules that is a cache miss,
not a failure, and both lint gates still exited zero with no diagnostics.

`make publish-check` was excluded deliberately, not skipped by oversight: it is
the upstream defect above. No gate failed, so the certificate carries no
findings.

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
| `boundary/fixture_cargo/environment.rs`              | 161             | the closed child environment                          |
| `boundary/fixture_cargo/metadata.rs`                 | 229             | the `cargo metadata` document contract                |
| `boundary/consumer/mod.rs`                           | 195             | consumer manifests, workspace and packaged            |
| `boundary/consumer/library/mod.rs`                   | 289             | the three consumer bodies                             |
| `boundary/closure/mod.rs`                            | 316             | forbidden names, expected edges, negative controls    |
| `boundary/package_set/mod.rs`                        | 335             | the two-invocation packaging recipe                   |
| `boundary_tests/{closure,consumer,manifests}/mod.rs` | 208 / 145 / 156 | the 17 cases, from 15 test functions                  |
| `boundary/mod.rs`, `boundary_tests/mod.rs`           | 29 / 14         | module wiring                                         |

Green runs, three regimes. The counts are the pre-review shapes: the 15 test
functions below became 17 cases when the negative controls were parameterized in
`b82f182e`.

- `cargo test -p ortho_l10n --test boundary --offline` (default parallelism,
  warm): `15 passed … finished in 115.25s`. The two packaged-runtime contracts
  each ran past 60 s and both trigger the same `OnceLock` packaging step.
- The same suite cold, single-threaded: `finished in 71.06s`.
- `cargo nextest run -p ortho_l10n -E 'binary(boundary)'` (process per test):
  `Summary [70.144s] 15 tests run: 15 passed (2 slow)`, slowest 70.143 s.

70 s sits far inside nextest's 600 s per-test allowance. The first figure is
larger than the third only because libtest's in-process threads contend for one
shared packaging `OnceLock`; that is a scheduling property of the harness, not
suite cost.

The reassessment corrected the scheduling conclusion, not the allowance one. No
`slow-timeout` override is needed — but a `test-group` override is, because the
process-per-test regime is exactly what lets several contracts package one
shared target directory at once. See the reassessment round above. Re-measured
after that repair, all 17 cases pass in 34.98 s under `make boundary` and 32.55
s under `cargo nextest run`.

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
after each restore). The full contract suite is 330 passed, one skipped; after
the reassessment added `boundary_packaging_test.py` it is 334 passed, one
skipped.

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

### EP-M3 reassessment round (2026-10-22)

The follow-up `coderabbit review --agent` (CLI 0.9.1) ran over the tree at
`ea71e094` and returned four findings, all minor, and all four are valid.

Scope, recorded because the CLI did not attest it: the request carried
`reviewType: "all"` and `baseBranch: "main"` with no `baseCommit`. The
`complete` record names 130 reviewed files, and that set is exactly
`git diff --name-only origin/main...HEAD`, which is the same 130 paths. The
head binding is therefore inferred from the file set rather than asserted by
the CLI, and it is consistent with the tree the gates certify.

| #   | Finding                                         | Disposition                                     |
| --- | ----------------------------------------------- | ----------------------------------------------- |
| 1   | `PACKAGE_DEADLINE` can never fire under nextest | Valid; 900 s reduced to 240 s, see below        |
| 2   | `environment` module doc contradicts its list   | Valid; doc rewritten to classify the exclusions |
| 3   | `-ise` spellings in `ortho_l10n` rustdoc        | Valid; `-ize` sweep over the crate's prose      |
| 4   | The `boundary` tests are not serialized         | Valid; `boundary_packaging` nextest group added |

Finding 1 is the most consequential, and neither remedy the report offered was
taken as stated. The review proposed lowering the bound *or* adding a nextest
override for a larger allowance. Under nextest a single test carries both
packaging invocations, so the real requirement is that the pair fit inside the
per-test allowance; whether `make boundary` (one process) can reach 900 s is
irrelevant, because nextest would stop the test at 600 s first. 240 s per
invocation puts the pair at 480 s of the 600 s base allowance with 120 s for
the root checks around them, which needs no override at all. That choice also
avoids the ambiguity rule in `trybuild_tier_test.py`, which refuses to read the
filter if two overrides carry the file's largest allowance.

The figure is a measurement, not a guess. With the packaging target removed,
the pair took 244 s on this host — and that figure is an upper bound, because
the child inherits compile admission and waits in its queue. Warm, the same
test takes about 35 s: `make boundary` finished all 17 contracts in 34.98 s, and
`cargo nextest run -p ortho_l10n --test boundary` in 32.55 s.

Finding 4 was proved rather than accepted on inspection. `packaged_set()`'s
`OnceLock` is process-local and nextest runs each test in its own process, so
in a full nextest run six contracts each ran `cargo package` against one
`packaging-target`. Cargo's package-cache lock closes the package archive but
does not hold readers out while a root is removed and unpacked, so a run that
passed was evidence of scheduling luck rather than safety. Two fixes follow: a
`boundary_packaging` group with `max-threads = 1` selecting `binary(boundary)`,
and `tests/workflow_contracts/boundary_packaging_test.py`, which reads the
group, the `[[test]]` target name, the override count and the deadline out of
the sources so a later edit cannot quietly undo either half. Its two
load-bearing assertions were mutation-proved: `max-threads = 2` fails only the
seriality test, `from_secs(400)` and an invocation count of 3 each fail only
the arithmetic test.

Finding 3 was applied to `ortho_l10n` alone. The crate's identifiers
(`LocalizationArgs`, `Localizer`, `NoOpLocalizer`, `FluentLocalizer`) already
spell `-ize`, so the prose now matches them. Outside the crate the same sweep
would rename `exercises` and `raises`, which are correct English, so it stops
at the crate boundary; `examples/hello_world/tests/localised_help.rs` and the
`typos.local.toml` entries that name it are untouched. The two fixture string
literals built on the same stem in `ortho_l10n/src/tests.rs` were test data,
not a contract, and became `Consumer fallback` rather than a respelled version
of the same word. A value that no longer reads like a misspelling is easier to
keep out of a future sweep's way, and the spelling gate cannot trip over the
sentence that describes it.

Finding 2's `environment` doc now states the policy the list actually encodes:
compiler flags and wrappers are inherited *deliberately*, so the child is held
to the same `-D warnings` policy as the parent, and the exclusions are named by
kind — target-directory selectors, configuration and profile selectors, and the
two variables that name the parent crate.

### EP-M3 gate results against the repaired tree (2026-10-22)

Repairs committed as `3c22ba57`. The scrutineer ran all eight gates
sequentially against that frozen candidate
(`3c22ba573e83d87c897493bb6b0802023c1764c9`, tree
`c7927bb3abf47d3fb16dc0943871ff77201839b3`) and re-checked HEAD, tree and
`git status --porcelain` after the last gate, so the certificate applies to the
frozen tree rather than to whatever was on disk at the end.

| Gate                      | Result  | Time  | Evidence                                 |
| ------------------------- | ------- | ----- | ---------------------------------------- |
| `check-fmt`               | pass    | 1 s   | 90 files unchanged                       |
| `typecheck`               | pass    | 10 s  | `cargo check --workspace --all-targets`  |
| `lint`                    | pass    | 30 s  | rustdoc, Clippy *and* Whitaker all ran   |
| `test`                    | pass    | 209 s | 2,066 Rust tests passed, 29 ignored      |
| `boundary`                | pass    | 12 s  | 17/17, packaged and negative-control too |
| `markdownlint`            | **red** | 16 s  | one spelling finding, see below          |
| `test-workflow-contracts` | pass    | 3 s   | 334 passed, 1 skipped                    |
| `nixie`                   | pass    | 0 s   | every diagram validated                  |

`build-limits-status` reported capacity 4, held 0, healthy, caching through
sccache, with no queued requests, so nothing in this round was coloured by
admission.

The one failure was mine, and it is the gate catching a category of error no
other gate can see. The reassessment record above quoted the misspelling it was
describing, in backticks. The typos gate reads inline code inside Markdown
prose, so naming the word reintroduced exactly the fault the sentence reported.
The repair is a rewording rather than an exception: the sentence now refers to
the strings by their role, so there is no token for the gate to read and no new
`typos.local.toml` entry whose scope would have to be argued.

The repair is `71ac7873`, and its delta from the certified tree `3c22ba57` is
one Markdown file. Only the three Markdown-reading gates are re-run against it;
the other five keep their certificates, because a name-only diff of those two
commits contains no path any of them reads. This is the same transfer rule the
plan applies elsewhere: a certificate follows content identity, not ancestry.

A first attempt at that re-run was void, and the reason belongs in the record
because it is a process failure rather than a source failure. The scrutineer
froze `71ac7873` correctly and `git status --porcelain` was empty at the start,
but this section was being written into the worktree while the gates read it.
`check-fmt` then reported a `+14 -13` mdtablefix delta and the spelling gate
reported two tokens that do not exist in the frozen blob at all, while the same
file on disk kept growing. The scrutineer proved the frozen blob passes the
identical mdtablefix invocation, so the failure was the moving file and not the
candidate. It discarded all three results rather than reporting the `nixie`
pass, which is the right call: a green gate over contaminated input certifies
nothing either.

The rule this leaves behind is the one the plan already states and this round
broke: freeze HEAD *and* stop writing to it. The plan text is part of the
candidate when a gate reads Markdown, so a "living document" is not exempt from
a gate freeze. The re-run below starts only after the tree has stopped changing.

### EP-M3 closing review (2026-10-22)

The closing `coderabbit review --agent` (CLI 0.9.1) ran over the tree at
`94ac0219` and returned five findings, which the CLI's `complete` record counts
as four unique after collapsing one exact duplicate of another. The scope is
attested rather than inferred this time: `reviewedFiles` is 131 paths and
`git diff --name-only origin/main...HEAD` is the same 131 paths, with an empty
symmetric difference. The fence held at both ends — HEAD
`94ac0219497d37b59def9660d17ffab0e0b85a99`, tree
`dd02f83703eabf90ea4eb3d62e1f81c972a1041a`, `git status --porcelain` empty
before and after, `dirty=0`, `untracked=0`, exit 0, 667 seconds, empty stderr,
no rate-limit notice.

None of the four is a finding against this milestone, and that is a scope
result rather than a quality one. **This branch is stacked on PR #607**, whose
tip has advanced since the base this branch was built on. A review with
`--base main` on a stacked branch compares 131 files; only 56 of them belong to
`#566`. The other 75 are the stack's, and every finding landed in that 75:

- All four named files are `cargo-orthohelp/` sources or `docs/users-guide.md`.
  Zero of the twelve `#566` commits touch any of those paths;
  `git log 1334cc88..HEAD -- cargo-orthohelp/` is empty.
- `git blame` at `94ac0219` attributes every flagged line to the base
  (`867849ba`, `a8d70338`, `92161017`), not to a `#566` commit.
- Two of the four are already repaired at the base's current tip `579fab25`:
  `--message-format=json` replaces the `json-render-diagnostics` spelling at
  `bridge/mod.rs:288`, and `is_generated_output_name` matches the `.pdb` file at
  `bridge_build_protocol.rs:188`. Repairing them here would duplicate the
  base's own fix, which is how a stacked PR is made to conflict.

| #   | Finding                                         | Disposition                                                            |
| --- | ----------------------------------------------- | ---------------------------------------------------------------------- |
| 1   | `.pdb` and incremental artefacts escape cleanup | Base-owned; `.pdb` half already fixed at `579fab25`; not repaired here |
| 2   | Interrupted-build marker undocumented           | Base-owned; `docs/users-guide.md` untouched by `#566`                  |
| 3   | Bespoke `flock`/`LockFileEx` in BDD steps       | Base-owned; fix belongs with the code in `#607`                        |
| 4   | `--message-format=json` for the diagnostic cap  | Base-owned, and already fixed at `579fab25`                            |
| 5   | Duplicate of #3                                 | Collapsed into #3 by the CLI's own record                              |

Two conclusions follow, and they point in opposite directions, so both are
recorded. The first is that the milestone's own repair set is not re-litigated:
none of the five touches the deadline, the `environment` allow-list, the `-ize`
prose, or the `binary(boundary)` serialization. The second is that a clean
result was never available from this command on this branch — asking the same
question again would return the base's findings again, and the closure
condition for EP-M3 is about `#566`'s work, not `#607`'s.

The stack is therefore the open delivery risk, not a milestone defect. PR #607
is open with `mergeStateStatus: CLEAN` and `reviewDecision: CHANGES_REQUESTED`
at `579fab25`, which is 30 commits ahead of the base this branch was built on
and has been rebased since; `1334cc88` is no longer an ancestor of it. Until
`#607` merges, this branch carries 75 files it does not own, and any hosted
review of it will keep reporting them. The disposition is to record the
dependency rather than absorb it, which is also what the plan's tolerance for
"a missing prerequisite that cannot be independently scoped" asks for: it is
recorded here as a delivery dependency and handed to `pr-babysitting` with the
PR, rather than repaired across a stack boundary.

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

EP-M4: "a crate needs the same treatment as its predecessors" held for the
publish order but not for the first publication. Every earlier release of this
workspace bumped an existing crate, so Cargo always found the package in the
index and reported a *version* mismatch, which lading downgrades. A brand-new
crate produces a different diagnostic — the package is absent, not the version
— and that one lading does not classify, so the dry run aborts even with the
override on and the order correct. A new member changes the *shape* of the
release, not just its contents. The `publish_order_test.py` contract could not
catch this, because it checks the order against the graph and the order was
right.

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

2026-10-21 (EP-M3): Add no CI step and no nextest `slow-timeout` override. The
`build-test` ceiling is exactly consumed by its two coverage watchdogs and the
surrounding allowances, the platform-keyed step set is closed, and
`DISCARD_COMMAND` is pinned as an exact string. The suite's 70 s worst case
sits far inside the 600 s per-test allowance, so a timeout override would
encode a contention figure as policy.

2026-10-22 (EP-M3, revised): Add a nextest `test-group` override even though no
timeout override is warranted. The two decisions are about different things.
The allowance question is answered by measurement and stays closed; the safety
question is answered by the process model and was open. nextest gives each test
its own process, so a cache that is correct within a process is not correct
across a run, and six packaged contracts were each repackaging one shared
target directory. A single-threaded group is the smallest fix that does not
touch the fixture's design, and it leaves `make boundary` and `make test`
unchanged because each starts one process for the binary.

2026-10-22 (EP-M3): Size the packaging deadline from the allowance rather than
from comfort. `2 x 240 s` fits inside the 600 s base allowance with 120 s for
the root checks; 900 s per invocation, the figure inherited from an earlier
draft, could never fire because nextest would stop the test first. The
invocations are now a `const` array so the workflow contract can count them
instead of restating the number, and its arithmetic assertion is the thing that
fails if either figure drifts.

2026-10-22 (process): Stop editing the plan before a gate round starts. A gate
that reads Markdown reads the working tree, so an edit made while it runs is an
edit to the candidate, and it silently voids the round in a way that looks like
a source failure. The first re-run after `3c22ba57` was discarded for exactly
this reason. Plan maintenance and gate execution are therefore sequenced, not
interleaved: settle the text, commit it, then freeze and gate.

2026-10-22 (EP-M3 close): Scope a stacked branch's review findings by
authorship, not by file path. A `--base main` review of this branch compares
131 files, but 75 of them belong to the stack beneath it, so "the file is in
the diff" does not mean "the milestone introduced it". The discriminator
applied here was whether any `#566` commit touches the flagged path and what
`git blame` says at the flagged line; both put all four closing findings on the
base. The findings were recorded and left to `#607` rather than repaired across
the stack boundary, because a duplicate repair of a line the base already fixes
is how a stacked PR becomes conflicting.

2026-10-22 (EP-M4): Report the publish-check failure upstream rather than
working around it here. The gate is the only step that builds each crate from
its packaged sources, and issue #414 is this repository's own evidence of what
that catches, so relabelling the gate, excluding the crate, or pinning a local
lading fork would trade a real signal for a green tick. lading's own user guide
already promises the behaviour that fails, which makes this a defect rather
than a missing feature, and it is self-clearing after `ortho_l10n`'s first
publication. Filed as leynos/lading#367 and recorded as a risk; the milestone
does not claim this gate passed.

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

EP-M3 implementation is complete and its evidence is recorded above: 17
boundary cases green under three scheduling regimes, the negative controls
proved to fail for their injected dependency by name, the packaged set built
from two `cargo package` invocations, the macro host cost measured against
`f99d9234`, and the focused `make boundary` target pinned by a workflow
contract. The sequential gate run passed on tree `7b2ce374`, the milestone CLI
review returned seventeen findings, and the follow-up round returned four more.
All twenty-one were dispositioned and every valid one repaired. What remains
before the checkbox closes is the gate run against the repaired tree and the
assessment that confirms it. The measured +24-crate macro closure is recorded
as a risk with an upper-bound caveat rather than as a settled performance claim.

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
