# Plan: Complete #318 scoped discovery and fix the Windows path assertion

Branch:
`issue-318-configdiscovery-support-multi-scope-file-loading-all-found-files-not-just-first`
PR: #465 (draft)

## Big picture

PR #465 resolves issue #318 by adding scoped configuration-file discovery plus
an opt-in policy loader. Review established that two objectives are still unmet:

1. `StackScopes` composition stops at the **first** successful candidate in each
   scope, so it does not load "all applicable files". #318 stays open.
2. `ortho_config/tests/scoped_layers.rs::compose_layers_remains_first_wins`
   fails on Windows: it compares `MergeLayer::path` with the raw `TempDir`
   path, while the loader stores the canonicalised path.

## Rebase (done)

Rebased onto `origin/main` (`c144641e`). Series `66b60d38..2e6cfbf7` replayed
on top of the merge-base `f803dea9`; no conflicts. Audited:

- `range-diff` clean apart from the expected `parse/mod.rs` reconciliation.
- All 106 target-only paths byte-identical to `TARGET`.
- `diff TARGET..NEW_HEAD` is exactly the branch-owned 18-file change set.

## Design decision: same-scope precedence

The candidate list within a scope is a **preference order**: index 0 is what
legacy `FirstWins` selects. `MergeComposer` is last-pushed-wins. Therefore a
scope must contribute its candidates in **reverse preference order**, so the
historically-winning location is applied last and still wins, and every
lower-preference location contributes a base layer.

This keeps the global rule uniformly "later applied = higher precedence" while
mapping the legacy preference order onto it by reversal. Choosing plain
candidate order instead would let a low-preference fallback such as
`~/.demo.toml` silently override `~/.config/demo/config.toml`.

Canonical-path deduplication is unchanged in mechanism: a file reachable twice
contributes once, at the lowest-precedence position it occupies.

## Work items

1. **Scope stacking loads every applicable candidate.** New module
   `ortho_config/src/discovery/scoped.rs` (the code no longer fits `load.rs`
   under the 400-line ceiling). `compose_scope` visits candidates in reverse,
   accumulates every successful chain, and keeps one per-scope terminal
   telemetry event naming the effective (highest-preference) winner.
2. **Windows path assertion.** Add a shared canonicalising assertion helper in
   `scoped_layers.rs`, apply it to both raw-path comparisons, and add fixture
   `value` assertions so first-wins and suppression stay independently verified.
3. **Tests.** Same-scope multi-file regressions, plus the audited path helper.
4. **Docs.** `docs/design.md`,
   `docs/rfcs/0002-config-layer-resolution-policy.md`, `discovery/scope.rs`,
   `discovery/load.rs` doc comments.
5. **Gates.** `make check-fmt`, `make test`, `make typecheck`, `make lint`.
   Windows coverage comes from CI.

## Progress

- [x] Rebase onto `origin/main`, audited clean.
- [x] Scope stacking loads every applicable candidate + tests + docs.
- [x] Canonicalising path assertion helper + value assertions.
- [x] Deduplication case made non-vacuous (mutation-verified).
- [ ] Local gates green.
- [ ] Push; Windows CI reports full suite and coverage artefact.

## Implementation notes

The scoped-composition code moved out of `load.rs` into
`ortho_config/src/discovery/scoped.rs`, because adding the all-candidates walk
and its documentation pushed `load.rs` past the 400-line ceiling. Shared items
(`PartitionedErrors`, `CandidateFailure`, `chain_layers`,
`is_required_candidate`) are now `pub(super)`.

`scoped_layers.rs` reached 479 lines, so it was split: fixtures in
`tests/support/scoped_fixtures.rs`, layer assertions in
`tests/support/layer_assertions.rs`, and the six new stacking regressions in
`tests/scoped_stacking.rs`. Each support module is used by both suites — an
`#[path]` include compiles a private copy per test binary, so a helper only one
suite needs would warn as dead code in the other.

Both new properties were verified against deliberate mutations. Replacing
`.rev()` with ascending order failed
`scope_stacking_keeps_the_most_preferred_location_winning` and
`scope_stacking_layers_lower_preference_keys_under_the_winner`; reinstating the
`break` on first success failed
`scope_stacking_loads_every_applicable_location` and two others. The
implementation was then restored from a byte copy.

That audit later caught a vacuous test of my own. The first version of the
deduplication case gave both scopes byte-identical path spellings, so it was
candidate *assembly* (which keys on the literal `OsString`) that collapsed them;
`record_first_canonical_path` never ran, and deleting it still passed. The
case now reaches the same file through `user/.demo.toml` and
`user/nested/../.demo.toml`, which assembly keeps distinct, so only
canonical-path filtering can collapse them. Disabling that filter makes it fail
with "got 2" while the other five pass — the mutation evidence this case
previously lacked. A test that passes for a different reason than the one it
names is worse than no test, because it retires the question.


## Lessons

- The SourceCoder review's "all applicable files" reading of #318 is correct and
  is the blocking objective, not an optional enhancement.
- No conflicts existed on the rebase because the branch's `parse/mod.rs` edit
  was a clap-import reflow that the target had independently narrowed; the
  replayed commit applied cleanly.
