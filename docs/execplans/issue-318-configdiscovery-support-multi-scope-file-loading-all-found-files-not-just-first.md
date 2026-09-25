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
   path, while the loader stores the canonicalized path.

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
2. **Windows path assertion.** Add a shared canonicalizing assertion helper in
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
- [x] Canonicalizing path assertion helper + value assertions.
- [x] Deduplication case made non-vacuous (mutation-verified).
- [x] Lint findings cleared: private doc link, elided lifetime, `shadow_reuse`,
      and `no_std_fs_operations` in the new fixture module.
- [x] Local gates green: `check-fmt`, `lint`, `typecheck`, `test` (1144
      passed, 0 failed), `markdownlint`, `nixie`. Verified independently on the
      frozen tree; the run was fingerprinted before and after and nothing
      drifted except the predicted `typos.toml` side effect, which was
      reverted rather than committed.
- [x] Trybuild allowance raised to 960s and the filter widened to all seven
      binaries; `trybuild_tier.py` + `trybuild_tier_test.py` added so a new
      trybuild binary cannot land on the base allowance unnoticed.
- [x] Push; Windows CI reports full suite and coverage artefact.
      Pushed `69aae8de` with lease bound to `2e6cfbf7`; CI run `36070786646`.
      The failing run this replaces was `34212392891` at head `2e6cfbf7`.

      That run timed out `must_use_compile_tests` at 600.224s. The failure is
      the Windows lane's alone: Ubuntu finished 1,322/1,322 and 1,294/1,294,
      and all three packaging jobs succeeded. Within that lane the fail-fast
      left 521 of 1,316 tests unrun — 795 ran plus 521 not run is exactly the
      1,316 the green run completes — and the Windows coverage artefact went
      unproduced, so the run uploaded two Linux lcov files where the green run
      uploads four. Two independent defects were behind it: the filter named
      two binaries of seven, and the override's 120s x 5 was the same 600s
      product as the base 60s x 10, so adding a binary to it bought nothing.
      Both are now fixed and both are contracted against. Run `36068184976`,
      which finished, shows the old figure was already too small rather than
      unlucky: `crate_path_trybuild` passed at 572.5s of 600s.
- [x] Suite green on Windows at `ce8f4621`. Run `36079995528`, all five jobs
      success. Both coverage lanes ran to completion with no fail-fast: 1,316
      passed of 1,316 (18 slow) and 1,292 of 1,292 (13 slow), 10 skipped each,
      and zero `Cancelling due to test failure` lines. `must_use_compile_tests`
      passed at 455.098s, inside the new 960s but well outside the 600s that
      killed it, and its threshold cadence is now `[>120s][>240s][>360s]`,
      which is the override's 120s period rather than the base profile's 60s:
      the binary is provably on the raised tier. Package-cache lock waits fell
      from four to zero, and `Cancelling due to test failure` from one to
      none. Count either only after stripping ANSI: the log dump stores the
      escapes as literal `^[[1m` caret sequences, so a `\x1b` pattern matches
      nothing and the plain phrase is split mid-word inside the coloured
      `Blocking`. A grep that reports zero for *both* runs is reporting the
      escapes, not the lock waits. The two `##[warning]` lines in the log are
      unchanged from the failing run and are not this repository's: a Node 20
      deprecation notice emitted by GitHub's own "Complete job" step, and a
      cache-reservation race between concurrent jobs.

      Note the run id and head: the first push of this work was `69aae8de`
      (run `36070786646`, failure), and the fix above is `ce8f4621` (run
      `36079995528`, success).

- [x] Final head green on Windows. Run `36084362402` at `fd9459d1`, the head
      the plan file's own last commit produced, all five jobs success. This run
      covers more than the one above, because the three packaging jobs test as
      well: 1,316 / 1,292 / 1,322 / 1,294 tests, all passing, 10, 10, 15 and 15
      skipped, with 13, 7, 8 and 6 slow. Zero lock waits and zero cancellations
      across all four. The 96 targeted assertions named in this plan — the 24
      scoped, stacking and discovery-attribute tests times four lanes — all pass,
      including `compose_layers_remains_first_wins` four times over, which is the
      Windows assertion this work began from.

      The same two `##[warning]` lines appear, and the log names the source
      precisely enough to settle the question of whose they are: both are
      emitted by the "Complete job" step, which is the runner's own, and the
      action it complains about is reached only through
      `leynos/shared-actions`' coverage actions. This repository's `ci.yml`
      names no upload action of its own at all, so the deprecation is
      inherited rather than owned, and cannot be fixed here.

- [x] Green again after this plan's own commits. Run `36087864052` at
      `4ca6483e`, all five jobs success. Four suites, all passing:
      1,316 / 1,292 on Windows and 1,322 / 1,294 on Linux, 10 and 15 skipped,
      zero lock waits and zero cancellations. All four coverage artefacts are
      present, the two Windows lcov files among them — the artefact whose
      absence the failing run is now recorded as causing. The 96 targeted
      assertions pass again, and `must_use_compile_tests` passed at 367.998s
      on the `[>120s][>240s][>360s]` cadence. That last figure is the useful
      one: it is well inside the 960s now allowed and well outside the 600s
      that killed it, but it is also well inside the 600s — so on this runner,
      with a warm cache, the old allowance would have passed. The margin was
      never the thing being bought; the seven-binary coverage was.

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

`tests/support/scoped_fixtures.rs` writes through a `cap_std::fs::Dir` handle
because Whitaker's `no_std_fs_operations` denies `std::fs`. Since the tests
need nested paths such as `user/demo/config.toml`, the helper opens the deepest
ancestor that exists, creates the remainder relative to that handle, and writes
at the *same* relative path. Writing the bare file name instead lands the file
beside the directory just created — the failure mode is silent, so the first
version of the rewrite produced "one file reachable from two scopes must
contribute one layer, got 0" rather than a write error.

Walking to the deepest existing ancestor is also what retires the lint
exemption the branch previously carried. `c5fe33aa` moved these fixtures to
`std::fs` and added `"scoped_layers"` to `dylint.toml`, because the helper it
replaced rooted its capability at `Path::new("/")` and did
`path.strip_prefix("/")` — which has no meaning for a drive-qualified Windows
path. That root cause is gone, so the exemption was withdrawn rather than left
in place: a suppression that outlives its reason hides the next regression.

Note that the exemption never covered the new suite in the first place. The
shared `#[path]` module is compiled into both test binaries, so the lint fired
on `scoped_stacking` — which had no exemption — while `scoped_layers` was
already excused. That asymmetry is why the failure looked like a missing
exemption rather than a stale one.

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

- `make spellcheck` regenerates `typos.toml` in place as a side effect, adding
  13 ignore patterns for content that does not exist in this repository. Ten of
  the thirteen match zero files here — vendor names, CSS property fragments,
  and one misspelled identifier that the pattern preserves verbatim — and the
  three that do match are a CLI colour-flag pattern, `HashiCorp`, and a test
  helper name. The regeneration is deterministic and the gate passes with or
  without it, so it is not a requirement of this change. The pinned builder
  (`v0.1.1`) evidently draws on a shared base dictionary broader than this
  repository, and the tracked copy predates that.

  **Reverting it was the wrong call, and this plan recorded that wrong call.**
  The regeneration is a fixed point (`make spellcheck` twice yields identical
  bytes), it is additive-only, and nothing in CI drift-checks it — so a
  committed copy is never compared against a fresh one. What settles the
  question is the Stop hook: it runs `make markdownlint` whenever Markdown
  changes and then blocks on the dirty tree that gate leaves behind. A revert
  therefore regenerates on the next run and blocks again, forever. The file is
  committed at `bc869e61`; `typos.local.toml`, the file to edit for a real
  exception, is untouched. Main carries the same kind of commit (`60cdb15f`).
- Quoting those patterns in this plan was itself a mistake: the misspelled
  identifier tripped `spellcheck` in prose even though it is a literal from a
  generated file. Describe such a token rather than reproducing it.
- The same trap caught this plan a second time, from the opposite direction.
  Naming an external action to say it is *not* used still puts its US spelling
  in prose, and `spellcheck` reads prose, not intent: it failed on the word for
  a build output immediately after the sentence denying any part in it. A name
  the gate rejects cannot appear in a sentence about that name, whatever the
  sentence claims. Say what the thing does instead.
- The SourceCoder review's "all applicable files" reading of #318 is correct and
  is the blocking objective, not an optional enhancement.
- No conflicts existed on the rebase because the branch's `parse/mod.rs` edit
  was a clap-import reflow that the target had independently narrowed; the
  replayed commit applied cleanly.
