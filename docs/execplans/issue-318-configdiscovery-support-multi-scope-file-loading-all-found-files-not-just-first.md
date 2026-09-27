# Plan: Complete #318 scoped discovery and fix the Windows path assertion

Branch:
`issue-318-configdiscovery-support-multi-scope-file-loading-all-found-files-not-just-first`
PR: #465 (ready for review, not a draft)

## CodeRabbit rounds on `75d9901d`: 10 findings, all verified and answered

CodeRabbit returned `CHANGES_REQUESTED` at 2026-09-27T16:28:18Z (review
`5331150192`) against `75d9901d`, with nine actionable inline comments. An
earlier round against the same commit had raised a tenth that the later review
did not repeat; see the section on it below, which also records how it came to
be missed. The findings are archived verbatim under `/tmp/cr465-75d9901d/`, one
JSON per comment, so this plan does not have to restate claim text that the
review owns.

**All nine were verified against the source before any edit** — three parallel
read-only reconnaissance agents, one per group, none editing. Every one
returned VERIFIED, each with a material nuance the comment did not state. The
repairs are committed as `59b2b2ae`; each is pinned by a test whose failure on
revert was demonstrated rather than assumed.

1. **Macro emitter** (3 findings, two stated MAJOR) — VERIFIED, and the three
   compound. `policy_impl.rs` built its selector chain from `env_vars` alone
   and never appended the injected discovery source, so *no* environment route
   worked in policy mode: the selector rung was empty, and automatic discovery
   read the real process environment. The third finding's mechanism is broader
   than stated — the default selector is dropped for every struct that does not
   write `env_vars`, not only in some case. The `project_root_from` finding is
   real codegen breakage: the generated CLI struct wraps *every* field in
   `Option` (`cli_flags.rs:33`), so a plain `PathBuf` field yields
   `Option<PathBuf>` where the emitter passed it to `impl Into<PathBuf>`.
2. **Discovery semantics** (2 findings, MINOR) — VERIFIED as facts.
   `origins()` was a constant copy of `scope_order`, wrong even under
   `FirstWins` where `compose_scoped_layers` returns before dereferencing
   `scopes`; zero consumers and zero tests existed, which is how it stayed
   wrong. Finding 5's *remedy* was refused — see below.
3. **Python workflow contracts** (4 findings, TRIVIAL) — VERIFIED but
   currently unreachable, so latent rather than live. Repaired anyway: each
   reading was correct by luck of the current configuration shape, not by
   construction, and the repairs are cheap.

### Finding 5: the code was right and the RFC sentence was wrong

`discovery/scoped.rs:168` asked for per-reference `extends` expansion, citing
RFC 0002's "expanded once per reference". The reviewer quoted a sentence this
branch itself had written. Per-reference expansion would have a parent reached
by two children contribute two layers, doubling any `append`-strategy vector it
holds — the exact harm invariant two's own rationale exists to prevent — while
leaving scalar merges identical. So the code is the contract and the reworded
sentence was the defect. The RFC now says a cycle is detected within a chain
rather than across chains, and invariant two is widened from "across scopes" to
"across the whole composition", naming the shared-parent case explicitly. The
test the reviewer asked for is added, asserting three layers rather than four,
which is what only one reading can satisfy.

### Coverage the macro fixes would otherwise have lacked

The existing policy tests drive `load_from_iter` against the real process
environment, so nothing exercised source injection on the policy path — the
first two repairs could have been reverted with every gate still green.
`ortho_config/tests/policy_sources.rs` closes that gap. Reverting the injected
`env_source` step fails both of its tests; reverting the default selector fails
one. That file also had to satisfy Whitaker's `no_std_fs_operations`, which it
does by reusing `support/scoped_fixtures.rs::write_config` — the
capability-handle helper — rather than by adding a `dylint.toml` exemption, so
the exemption list stays as narrow as the repository had it.

### Where the review stands

The review is bound to `75d9901d`. The answers land in `59b2b2ae`, and each of
the nine threads still needs its individual `@coderabbitai` reply: duplicate
reports were grouped for implementation but must be answered separately, and a
posted reply is not a resolved thread.

### The tenth finding, from an earlier round than the nine

Nine was the wrong count. An earlier round — review `5331092523` at
2026-09-27T15:54, also against `749032b3` — raised four comments, and only
three of them reappeared in the `CHANGES_REQUESTED` round. The fourth,
`candidate_set.rs:98`, survived unmentioned here, unanswered in its thread, and
unduplicated by the later review. It was found by enumerating the PR's review
threads rather than by re-reading the round the plan names, which is why it
went unnoticed: the plan tracked a round, and the defect was in a different one.

**It is real, and reachable.** Assembly de-duplicates candidates on the path
and kept whichever *scope* tag arrived first.
`$XDG_CONFIG_HOME/demo/config.toml` is a user candidate by construction, and a
project root is arbitrary, so a root of `$XDG_CONFIG_HOME/demo` with the same
file name makes the *identical* path a project candidate too. The second push
was refused whole, so the surviving entry carried only `User`, and a
`StackScopes` request naming `Project` alone matched nothing and loaded the
file that is, by construction, exactly what it asked for. Demonstrated before
the fix with a failing test: the `Project`-only request loaded zero layers.

**The repair makes the scope tags additive while the entry stays single.**
`Candidate.scope` became `scopes: Vec<DiscoveryScope>` behind an `in_scope`
predicate, and the accumulator's `seen` set became a map from key to entry
index so a later rung records its scope on the entry an earlier rung created
instead of being discarded. Asking for both scopes still contributes one layer,
which is the property that must not regress while fixing the one that must.

Mutation-checked: restoring first-tag-wins fails
`scope_stacking_keeps_a_shared_path_reachable_from_each_scope` and nothing else
in that suite. The test asserts the `Project`-only case precisely because it is
discriminating — a `User`-only request passes under either behaviour, since the
user rung is the one that creates the entry.

### The gate round that caught the tenth fix's own lint

Scrutineer ran all seven gates against the tenth-finding change set and
returned one red: `make lint` exited 2, with **three Clippy errors in the new
`push_unique` branch** — `excessive_nesting` at `candidate_set.rs:118`,
`shadow_reuse` at `:116`, and `indexing_slicing` at `:117`. All three are
`deny`-level repository policy, not upstream defaults: `clippy.toml:5` sets
`excessive-nesting-threshold = 4` (upstream leaves it off), and the workspace
`[lints.clippy]` table sets `shadow_reuse` and `indexing_slicing` to `deny`.

Because `lint-clippy` failed, make aborted and **`lint-whitaker` never ran** —
Whitaker's verdict on this change set is that round is *unknown*, not passing.
A green `make test` in the same run says nothing about it either: `cargo test`
does not run Clippy lints.

The three fixes are one refactor, not three suppressions. The scope-merge moved
into `CandidateAccumulator::record_scope(index, requested)`, which:

- takes the resolved index, so the body reaches the entry through
  `self.candidates.get_mut(index)` — clearing `indexing_slicing`, and letting
  the miss resolve to `return` rather than a panic path in a library;
- binds the later rung's scope as `requested` and moves it into a local `scope`
  only in the second `let ... else` — clearing `shadow_reuse`;
- splits the two guards into flat `let ... else` statements, dropping the block
  from nesting level 5 to level 2 — clearing `excessive_nesting`.

The caller now binds `let listed = self.seen.get(&key).copied();` before
matching, so the map borrow is over before `key` is moved into `seen` on the
insertion path. Behaviour is unchanged and that is asserted, not assumed:
`scope_stacking` passes 8/8 including
`scope_stacking_keeps_a_shared_path_reachable_from_each_scope`, and both
`cargo doc` under `RUSTDOCFLAGS="-D warnings"` and
`cargo clippy --all-targets --all-features -- -D warnings` are clean.

Scrutineer also reported `make check-fmt` red on the plan document, and
correctly labelled it stale: it failed at 20:59, the file was rewritten at
21:00, and `mdtablefix --check` on the current content exits 0 with "74 files
left unchanged". Re-run confirmed green. Its other five gates — `typecheck`,
`test` (188s, every suite `0 failed`, pytest 87 passed/5 skipped),
`markdownlint` (0 errors over 75 files, `spellcheck` reached and passed),
`test-workflow-contracts` (312 passed/1 skipped), `nixie` — all passed.

### The re-gate at `8536622b`: all seven green, and Whitaker finally ran

The three repairs were committed as `5e406f7d` (Clippy), `037d2676` (CodeScene)
and `8536622b` (this document), and scrutineer re-ran every gate on the frozen,
clean head. **All seven passed, with no retries and no skips, and HEAD did not
move during the run** — the certificate is valid for `8536622b` without re-run.

The result that matters is Whitaker. `lint-clippy` exited 0 this time, so make
did not abort and `lint-whitaker` **executed for the very first time against
this change set**, completing in 5.02s under `nightly-2026-05-28` with **zero
dylint findings** of any severity. Its invocation is recorded verbatim in the
lint log, so "it ran" is a read fact rather than an inference from a green
`make`. The previous round's three Clippy errors are confirmed repaired by
`5e406f7d`, and the `excluded_crates` concern recorded in memory did not
surface. Totals: `test` 1347 passed / 0 failed / 15 ignored across 80 suites
plus pytest 87 passed / 5 skipped; `test-workflow-contracts` 313 passed / 1
skipped — one more than the previous round, which is the new helper's doctest.

## The pre-merge table: a review surface this plan never reconciled

The subclass matters because it was found late. CodeRabbit's pre-merge table is
a *separate surface* from the inline findings — it lives in the top-level
walkthrough comment (`5398461696`), is edited in place rather than posted, and
carries **2 errors and 7 warnings** that no section here had recorded. The
inline threads were checked and answered; this was not looked at at all. It is
the `comenq-coderabbit` workflow's step 7, and skipping it left two error rows
unexamined.

The table is bound to `75d9901d` and predates every repair: `59b2b2ae` is
**after** that commit, so three of its rows describe defects this branch has
already fixed — `Domain Architecture` is the `source_tokens` finding,
`Observability` is the origins finding, and `Testing (Overall)` is partly the
`upload`/`origins` gaps the new suites close. Those rows are stale in the
ordinary sense and need a follow-up rather than code changes.

**`Unit Architecture` is not stale, and that is the finding worth keeping.** It
is an *error* row, and it points at code that is still exactly as described at
`8536622b`. The row is reproduced in substance rather than word for word: it
names the helper directly, and that identifier is one this repository's
spellcheck gate rejects wherever it appears in Markdown.

    `record_first_canonical_path` resolves each layer path through the
    fallible helper in `file/path.rs` (which returns `OrthoResult<PathBuf>`)
    and converts any error to `true` with `map_or(true, ...)`

That is `scoped.rs:211-213` — a fallible path resolution feeding
`map_or(true, ...)` on the same expression, so a filesystem error is silently
reported as "not a duplicate" and the layer is kept. The row's own diagnosis is
right: a failed resolution is swallowed rather than surfaced.

Its proposed remedy is right too, and better than it looks. `MergeLayer::file`
stores the path the loader built from `canonical` (`loader.rs:188,198`), and
`to_utf8_path` is `Utf8PathBuf::from_path_buf(canonical)`, so **the layer
already carries a canonical path** and the second lookup is redundant work as
well as a swallowed error. Removing it and keying the deduplication set on
`layer.path()` directly would fix the error, delete a syscall per layer, and
retire the issue in one move.

**Why it is recorded rather than fixed here.** The remedy changes the
deduplication key from "resolve whatever path the layer reports" to "trust the
layer", and `assert_layer_path` exists precisely because the loader's
resolution and the test's can differ in *representation* on Windows
(`dunce::canonicalize` rewrites an otherwise identical path). Proving the two
keys agree is a Windows-shaped question — the one platform this worktree cannot
run. Landing it here would be a change whose evidence cannot be gathered
locally, on a head that has just been certified green, to fix a silent-error
path rather than a wrong answer: the fallible helper failing yields "keep the
layer" which is the non-deduplicating answer, not a corrupt one. It needs its
own round with Windows CI in the loop.

The warnings are recorded with their dispositions rather than dismissed:
`Docstring Coverage` (70.16% against an 80% threshold, scoped to touched
functions) and `Testing (Property / Proof)` are genuine gaps this plan already
names as outstanding work; `Testing (Compile-Time / Ui)` and the two
documentation warnings concern coverage of the policy API by trybuild fixtures
and the user/developer guides, which the plan's work items 4 and 5 partly cover.

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

## Second rebase onto `c043980d` (done, 2026-09-27)

Replayed `c144641e..90f1a359` (13 commits) onto `c043980d`. One conflict, in
`.config/nextest.toml`, and one commit dropped.

**The conflict is two fixes for one failure.** Main had independently found the
same Windows trybuild timeouts and fixed them a different way: a Windows-gated
override reserving every nextest slot for four binaries (`crate_path_trybuild`,
`declarative_merge_trybuild`, `compile_fail`, `compile_time`) and keeping 600
s, because their cold child `cargo` builds were competing with the rest of the
suite. This branch's commit `b3ea1e5a` widened the filter from two binaries to
all seven and raised the allowance 600 s to 960 s. Both are measured remedies,
so neither was discarded.

**Resolution: keep both entries, main's first.** nextest resolves each override
field from the *first* matching entry that sets it — established here by probe,
not by assumption (`cargo nextest show-config test-groups` against two entries
with identical filters: the first matched, the second received nothing; two
entries setting *disjoint* fields both applied). So the four named binaries
keep 600 s *and* run alone, the other three (`env_source_trybuild`,
`generated_lint_trybuild`, `localized_parse_trybuild`) take 960 s, and Linux
takes 960 s for all seven. Merging into one entry would have had to pick one
remedy for all seven binaries; keeping both applies each where it was measured.

Both sides' contracts are satisfied simultaneously:
`windows_trybuild_isolation_test.py` (main's, added by the rebase) reads the
first entry and `trybuild_tier_test.py` (this branch's) reads the entry at the
largest allowance, which is uniquely the second. The independence claim is
mutation-checked both ways: widening the filter leaves the isolation contract
green while the tier contract fails and names the uncovered binary.

**`bc869e61` became empty and was skipped.** Main's `41e54346` had produced the
*byte-identical* `typos.toml` (`bfdda131` on both sides), so the commit that
"committed the regenerated fixed point" had nothing left to add. This is the
earlier fixed-point conclusion corroborated from the other direction: the
regeneration is deterministic enough that an unrelated branch arrived at the
same bytes. Confirmed before skipping — the commit's 13 added lines are all
present in the rebased tree, and `git hash-object typos.toml` equals
`origin/main:typos.toml`.

**Audit of the replay** (all three checks):

1. 84 target-only paths checked, 0 differ — no main-side work was lost.
2. Every deletion hunk against `TARGET` lies in a branch-owned file, and each
   deleted symbol was relocated by a branch commit rather than dropped
   (`budget_of`, `is_positive_integer`, `terminate_after` and `TypeGuard` moved
   to `nextest_allowances.py`; `DiscoveryAttrs`, `MergeStrategy` and
   `parse_prefix` to `discovery_attrs.rs`; `discard_unknown` is still present
   and only changed visibility).
3. No newly-repeated multi-line blocks; the two candidates are a
   `tempfile::tempdir()` idiom and a docstring boilerplate heading.

One post-rebase commit, `cc9f302e`, reconciles the guide: the automated merge
had kept both sides' prose, leaving main's "The 600 s ceiling stays" reading as
a claim about the whole file when it is now about one entry.

`range-diff` after the replay: 11 of 12 commits identical, `b3ea1e5a` shown
only as the intended conflict resolution, `bc869e61` as the dropped no-op.

### A false PASS in the audit tooling

The first run of audit 1 reported **zero target-only paths**, which is
impossible: main changed 87 files and the branch touched 30. Cause:
`comm -z -13` exited 0 and printed nothing. The manifests had been sorted
*without* `LC_ALL=C` but compared *under* it, so `sort` and `comm` disagreed on
collation and `comm` silently produced an empty set. Rebuilt with `LC_ALL=C`
pinned across both steps and cross-checked with a known-answer probe
(`grep -Fxv -f`) before trusting it: 84 paths, 0 differ.

This is the same shape as the ANSI-in-logs trap already recorded in this plan:
a tool reporting "nothing found" for *both* a known-good and a known-bad input
is reporting its own configuration, not the data. Probe a set-difference or a
search against a known answer before believing a zero.

## Third rebase onto 5732adf9 (done, 2026-09-27)

Replayed `c043980d..749032b3` (16 commits) onto `5732adf9`. No conflicts. All
16 commits are `=` identical under `range-diff`, so the series replayed
verbatim rather than being reconstructed.

`5732adf9` (#508) splits oversized configuration test modules: `env_source.rs`
gains a sibling `env_source_tests.rs`, and `cli_default_as_absent.rs` drops 668
→ 347 lines by moving its parser-default material into
`tests/support/parser_defaults.rs`.

**That is this branch's own pattern, arrived at independently.** Main declares
its new module as
`#[path = "support/parser_defaults.rs"] mod parser_defaults;` — exactly how
this branch splits its fixtures and assertions (`support/layer_assertions.rs`,
`support/scoped_fixtures.rs`), and how it already declared
`support/to_anyhow.rs` and `support/discovery_builder.rs` before the rebase.
Both sides place shared test code in `tests/support/` and include it with
`#[path]`, so the two efforts are the same convention and the files coexist in
one directory with no rename or reconciliation needed. Nothing in the branch
had to change to adopt it; no `tests/support/mod.rs` exists on either side,
confirming the include-per-binary idiom is the repository's intent rather than
an accident of either branch.

Audit of the replay, all three checks:

1. 4 target-only paths checked, 0 differ.
2. No deletions relative to the target, and the branch-vs-target change set is
   byte-identical before and after (`29 files, +2885 -225` both sides). The
   name set grows 29 → 33 because the merge base moved and now includes main's
   four files.
3. 26 sliding-window repeats, all classified with no reconstruction artefact:
   NumPy-style docstring scaffolding (`Parameters`/`Returns`, one more
   docstring per added function), the `#[rstest]`/`#[expect]` attribute idiom
   appearing once more as a suite grew, `tempfile::tempdir()` setup lines, and
   a struct-init field pair shared by two conversion functions that return
   different types. The window inflates each count by its own width, so these
   are one extra genuine occurrence each rather than duplication.

`ortho_config/tests/extends.rs` is 405 lines, one file over the 400-line ceiling
`AGENTS.md` sets — but it is 405 at `origin/main` and untouched by this
branch, so it is inherited, not caused here. Main is already splitting
oversized test modules one per change (#507, #508), so this is that series in
progress rather than a defect to fix from this branch; fixing it here would
collide with the next such change.

## Fourth rebase onto f6a406fc (done, 2026-09-27)

Replayed `5732adf9..38bb41f6` (21 commits) onto `f6a406fc`. No conflicts; all
21 are `=` identical under `range-diff`, and the change set is byte-identical
before and after (`30 files, +3442 -226`), so the series replayed verbatim.

`origin/main` moved `5732adf9` → `f6a406fc`, two commits. Neither touches the
scoped or policy API — checked with `git grep` over both new commits — so
nothing had to be adopted, and no main-side pattern needed reconciling this
time. The one thing worth recording is that they are *pertinent* even though
non-overlapping: they independently validate the injected-source approach this
branch's repairs commit to, and describe the duplication they address as "a
correctness hazard rather than only a tidy-up". Main reaching the same
conclusion from its own direction is corroboration, not a merge obligation.

**Weave did not participate.** `git check-attr merge` reports `unspecified` for
every sampled path and no `.gitattributes` carries a merge rule, so the driver
is registered globally but selected by nothing. Plain `zdiff3` is therefore the
correct rebase mode and no `-c` override was needed.

Recovery refs: `refs/recovery/pre-rebase-20260927-201138` (→ `38bb41f6`) and
`refs/recovery/target-f6a406fc`.

Two gates failed on the rebased head, both in the branch-added plan document
rather than being rebase artefacts: `check-fmt` wanted the coverage-gap
paragraph rewrapped, and `markdownlint` raised four MD049 emphasis-style errors
where the plan had quoted the RFC's underscore emphasis in a file whose
convention is asterisk. Fixed in `0bf59c86`; both gates then re-ran green, and
`markdownlint` reached its `spellcheck` sub-target again — the MD049 failure
had been aborting before it.

## Code health: the CodeScene finding this plan failed to record

CodeScene reports one finding against this branch, in a file this branch adds:

    ❌ New issue: Bumpy Road Ahead
    trybuild_binaries has 2 blocks with nested conditional logic. Any nesting of
    2 or deeper is considered. Threshold is 2 blocks per function
    tests/workflow_contracts/trybuild_tier.py  line 122

The function is `trybuild_binaries` (`trybuild_tier.py:84`), whose loop body
carries three nested levels: `for crate` → `if not tests.is_dir(): continue` →
`for source` → `if TRYBUILD_CALL not in ...: continue`. The tool counts the two
loop-and-guard pairs as the two blocks. Its `# suppression` link is a CodeScene
hosted action, not a thing to use from here.

Three facts matter, and only the first is comfortable:

1. **It does not block the merge.** The `main-required-checks` ruleset
   (`18427801`) requires exactly `build-test (ubuntu-latest)` and
   `build-test (windows-latest)`. CodeScene is absent from that list. Proof by
   example: PR #532 sits at `mergeStateStatus: UNSTABLE` — GitHub's word for
   "mergeable, but a *non-required* check is failing" — with this same
   CodeScene check as its only failure.
   `gh api repos/…/branches/main/protection` returns 404 "Branch not protected"
   here even though merges are gated, so that endpoint proves nothing either
   way; the ruleset is the authority.
2. **It is genuinely ours.** `trybuild_tier.py` is added by this branch (+227
   lines), not inherited. It is not dismissible as pre-existing.
3. **This plan never recorded it.** The finding has been continuously valid
   since `ce8f4621`, surviving two rebases, and no section here mentioned
   CodeScene at all. The comment's `original_commit_id` is `ce8f4621` while its
   `commit_id` tracks the new head, which is why the first reading looked like
   a stale artefact — the anchor is old, the finding is current.

The deferred-refactor reading is that `trybuild_binaries` is a three-level
comprehension-shaped walk that could be flattened into helpers, which would
both clear the rule and read better. That is a real improvement and it was not
taken in that round: it would have been a late change to a file whose bytes a
queued CodeRabbit review and a running CI were both inspecting, and it would
have cost a full re-gate of a head that was otherwise certified.

**The deferral was superseded, and the work is now done.** Every clause of that
basis had expired: the Clippy round described above made a full re-gate
mandatory anyway, so the cost argument was void; `comenq hist` showed no queued
review (the last was 3h earlier, against `15b09f3d`); GitHub Actions had
settled; and the head was not certified after all — it had a red `make lint`.
Deferring behind a cost that is no longer being avoided is how a recorded
finding turns into a permanent one.

**It was verified rather than assumed.** `cs check` is installed locally and
reproduces the hosted finding exactly before the change:

    warn: tests/workflow_contracts/trybuild_tier.py:91: Bumpy Road Ahead (bumps = 2)

The per-crate walk moved to `_crate_trybuild_binaries(base, crate)`, whose two
guards — the crate may have no `tests/`, and most files there carry no trybuild
call — now sit at the depth they belong to, with the binary-name set
comprehension replacing the loop-and-`continue`. The finding is gone and the
file scores a clean 10.00. Behaviour is asserted, not assumed: an inline
reimplementation of the old walk and the new function return the same
`frozenset` on the real workspace, both naming the same seven binaries
(`compile_fail`, `compile_time`, `crate_path_trybuild`,
`declarative_merge_trybuild`, `env_source_trybuild`, `generated_lint_trybuild`,
`localized_parse_trybuild`). `make test-workflow-contracts` passes 313/1
skipped — one more than before, the new helper's doctest — so no contract
regressed.

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
- `make check-fmt` covers `mdtablefix --wrap` as well as `cargo fmt`, so a
  hand-wrapped prose edit fails a *formatting* gate while the Rust tree is
  untouched. Both post-rebase commits here (`cc9f302e`, `004761d1`) tripped it
  that way — the second because the first's paragraphs were re-wrapped to a few
  words off 80 columns. Run `make fmt` before committing prose, not after: the
  gate reports only "+N -M" and which files, so the cause is invisible in the
  log and has to be recovered with `mdtablefix --diff`.

  The fix is verifiable rather than assertable. `mdtablefix --in-place` on a
  *copy* first, then compare the multiset of non-whitespace tokens against the
  original: an empty difference proves nothing but line breaks moved. That
  check matters here because workflow contracts parse the guide's prose
  (`sccache_wiring_test.py`, `timeout_budgets.py`, `trybuild_tier.py`), so a
  reflow that silently altered a token would fail a contract rather than the
  formatter. Also confirm the file holds no Mermaid block before reflowing, or
  a diagram `make nixie` validates could be reshaped.
