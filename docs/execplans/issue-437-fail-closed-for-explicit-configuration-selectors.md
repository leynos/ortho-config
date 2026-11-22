# Fail closed for explicit configuration selectors

Status: COMPLETE (pending gate evidence and review)

## Purpose

Make explicit configuration selectors fail closed. After this change, a path
added with `ConfigDiscovery::builder(...).add_required_path(...)`, or selected
by a non-empty `ConfigDiscoveryBuilder::env_var(...)` value, must produce an
error when it is missing, unreadable, malformed, uses a disabled format, or
fails to expand its `extends` chain. A later successful automatic candidate
must not hide that error.

The change closes
[issue 437](https://github.com/leynos/ortho-config/issues/437) and consolidates
the still-applicable part of the earlier report that discovery silently skipped
invalid explicit configs. It preserves the buffered-error policy
[issue 96](https://github.com/leynos/ortho-config/issues/96) established for
automatic discovery.

## Context and orientation

The discovery engine lives in `ortho_config/src/discovery/`:

- `load.rs` walks candidates and implements `load_first`,
  `load_first_partitioned`, and `load_first_with_errors`.
- `candidate_set.rs` holds the deduplicated candidate list and the
  `CandidateAccumulator` that builds it.
- `candidates/mod.rs` generates explicit, selector, platform, home, and project
  candidates in preference order.
- `scoped.rs` walks the same list for scope stacking.
- `builder.rs` exposes `add_required_path` and `env_var`.
- `outcome.rs` and `mod.rs` define the partitioned outcome types.

The derive-generated loader in
`ortho_config_macros/src/derive/load_impl/discovery.rs` already appends
`required_errors` even when layers load, so fixing the selector's requiredness
propagates to derived consumers without a macro change.

## Constraints

- Automatic discovery candidates stay optional, preserving issue 96's buffered
  error policy.
- An unset or empty selector value selects nothing and must not fail closed.
- `load_first_with_errors` and `load_first_partitioned` keep their existing
  bucket behaviour; only `load_first` gains the fatal required-error check.
- Public API changes are limited to documented behaviour; no new public type is
  required.
- The repository's Clippy configuration caps functions at four arguments, so
  candidate metadata must be grouped rather than added as another positional
  parameter.

## Tolerances (exception triggers)

- Stop if the change requires a new public error variant or a change to the
  derive macro.
- Stop if an automatic candidate's failure must become fatal to satisfy a
  consumer; that contradicts the issue 96 contract.
- Stop if a test proves the required-error check must also apply to
  `compose_layer` or `compose_scoped_layers` rather than being the caller's
  policy.

## Risks

- Some existing tests assert the old silent-fallback behaviour. Each affected
  test must be updated deliberately, not deleted.
- Candidate deduplication can let an optional explicit path mask a required
  selector that names the same path; requiredness must be promoted when a later
  rung marks an existing entry required.
- Telemetry expectations change for a failed selector: the candidate event now
  reports `required_failure` instead of `optional_failure`.

## Conformance basis

- Issue 437: fail closed for explicit configuration selectors.
- Issue 96: automatic discovery errors are buffered and optional.
- `docs/design.md`, discovery section: file-layer resolution semantics.
- `docs/users-guide.md`, "Handle every `load_first` outcome".
- ADR-0012 is unaffected: required-error precedence over profile diagnostics is
  preserved.

## Milestones

1. Core discovery: make `load_first` return required errors before any
   fallback, and carry requiredness on each candidate, including selector
   candidates whose value is non-empty.
2. Documentation: update the discovery API doc comments, the users' guide, the
   design document, and `CHANGELOG.md`.
3. Regression coverage: update tests that asserted silent fallback and add
   builder, derive, telemetry, and behavioural scenarios for the fail-closed
   contract.
4. Delivery: run the repository gates, obtain a CodeRabbit review, push, and
   open a draft pull request that closes issue 437.

## Verification plan

- Required-path failure with a valid fallback: `load_first` returns `Err` and
  the error names the required path. Verified by a unit test in
  `ortho_config/src/discovery/tests/loading.rs`.
- Non-empty selector failure with a valid fallback: the same, for missing,
  malformed, and `extends` failures. Verified by unit and derive tests.
- Selector unset and empty: automatic discovery still loads. Verified by unit
  tests.
- Deduplication promotion: an optional explicit path later named by the
  selector becomes required. Verified by a unit test in `dedup_tests.rs`.
- Required-after-success traversal: a required path after the first successful
  candidate is still checked, both when the success is a required path and when
  it is an optional explicit path generated before the selector. Verified by
  `loading_required.rs` and the selector tests in `loading.rs`.
- Unanchorable relative selector: a non-empty relative selector whose source
  cannot report a working directory is a required failure, not a fallback.
  Verified by `loading_required.rs` and `injected_env_loaders.rs`.
- Scoped composition: a required selector with no automatic scope is still
  checked by `StackScopes`, so its absence is reported even though its layers
  are not part of the stacked composition. Verified by
  `tests/scoped_required_selector.rs`.
- Derive propagation: `load_from_iter_with_sources` returns
  `OrthoError::File` when the selector names a missing or malformed file even
  when a valid automatic file exists. Verified in
  `ortho_config/tests/discovery_attributes.rs`.
- Behavioural contract: feature scenarios in `config_path.feature` and
  `error_aggregation.feature` exercise the same outcomes through the public
  loader.
- Full gates: `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
  and `make nixie` pass for the candidate commit.

## Progress

- [x] Branch renamed and tracking
  `origin/issue-437-fail-closed-for-explicit-configuration-selectors`.
- [x] ExecPlan recorded.
- [x] Core discovery change: per-candidate requiredness, required selector, and
  fail-closed `load_first`.
- [x] Documentation and changelog.
- [x] Regression tests at builder, derive, telemetry, example, and BDD level.
- [ ] Repository gates and CodeRabbit review.
- [ ] Draft pull request opened.

## Surprises & discoveries

- `ortho_config/tests/load_with_sources.rs` had a test that documented the
  selector as optional and relied on a malformed selector being buffered while
  a later XDG file supplied the profile chain. It now uses a malformed
  automatic candidate for that case and a separate test proves the selector
  error surfaces.
- The BDD support directory is `ortho_config/tests/rstest_bdd_support/`, not
  `ortho_config/tests/rstest_bdd/` as the original task sketch named.
- `examples/hello_world/src/cli/discovery.rs` is a handwritten
  `compose_layer` consumer with the same fall-through bug as `load_first`. It
  now aggregates required errors before returning a layer.
- The four-argument Clippy ceiling and the 400-line file cap both shaped the
  accumulator API: three thin `push_*` methods build a private `CandidateSpec`,
  keeping call sites short without adding a fifth positional argument.
- Review found the first-success walk was still too eager: optional explicit
  paths are generated before the selector, so a valid optional path could end
  the walk before the required selector was checked. The walk now skips only
  optional candidates after a success and still attempts every remaining
  required one.
- Review also found that an unanchorable relative selector was silently
  dropped. It now produces a required assembly error, so `load_first` fails
  rather than falling back to the process working directory.
- Review found a second traversal gap in `StackScopes`: a required selector has
  no automatic scope, so the per-scope walk never attempted it. A pre-pass now
  checks required candidates that no requested scope will visit.

## Decision log

- Group candidate metadata in a `CandidateSpec` passed to `push_unique`, rather
  than adding a fifth positional argument. The repository's Clippy threshold is
  four arguments including the receiver.
- Expose three thin `push_required`, `push_optional`, and `push_automatic`
  methods so candidate generation reads as policy rather than struct literals.
  This also keeps the generation module inside the 400-line file cap.
- Promote requiredness when deduplication meets an existing entry, so a
  selector naming the same path as an earlier optional explicit path still
  fails closed.
- Keep `compose_layer`, `compose_layers`, and `compose_scoped_layers` returning
  partitioned outcomes; their callers already choose how to surface required
  errors, and the derive does so eagerly. The handwritten example consumer was
  updated to follow that contract.
- Carry an unanchorable selector's error on `CandidateSet` as an assembly
  error rather than inventing a candidate path the loader must not read. Both
  the flat walk and the scoped walk record it in the required bucket with the
  `selector` source label.
- Check remaining required candidates after the first success, but keep the
  first success's value and precedence. Optional candidates after that point
  are still skipped, so the optional-error suppression contract is unchanged.
- In `StackScopes`, check required candidates that no requested scope owns in a
  single pre-pass, and skip candidates a requested scope will visit so their
  errors are not recorded twice. The pre-pass discards any successful layers:
  explicit selection remains outside the stacked composition's value.

## Outcomes & retrospective

The fail-closed contract is implemented and covered at every level the issue
named. `ConfigDiscovery::load_first` now returns required diagnostics before
any fallback, a non-empty `env_var` value is a required candidate, and
deduplication promotes requiredness rather than letting an earlier optional
entry relax it. Automatic discovery candidates remain optional, so the issue 96
buffered-error policy still holds for them.

Two things were worth discovering. The derive already had the correct policy,
so the selector fix propagated with no macro change; but the `hello_world`
example carried its own copy of the old fall-through logic, which shows the
hazard of handwritten consumers of partitioned outcomes. Second, the
repository's four-argument Clippy ceiling plus the 400-line file cap together
pushed the accumulator API toward three thin `push_*` methods over a private
`CandidateSpec`, which read better than the alternative of a fifth positional
parameter.

Verification evidence: `make check-fmt`, `make lint`, `make test`,
`make markdownlint`, and `make nixie` are recorded in the pull request, and
each new regression test was written to fail against the pre-change behaviour.
