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

## The pre-merge table, re-read on `9e9ecc37`: nine rows, every one adjudicated

The table was re-read live rather than from this plan. `5398461696` was last
edited **2026-09-28T00:20:45Z** and is still bound to `9e9ecc37`, so it is the
current surface. The heading now reads **"3 errors, 6 warnings"** — it has
grown an error row since the earlier reading, and this section replaces the
stale count rather than repeating it.

**Why the count moved is itself the finding.** `Testing (Unit And Behavioural)`
was a warning and is now an error. Nothing regressed: the PR only gained tests
between the two readings. A growing severity on an unchanged-code row means the
row is a *judgement about coverage breadth*, not a defect report, and it is
re-evaluated against the diff each time the walkthrough is regenerated.

Each row, with the evidence for its disposition:

| Row                            | Sev     | Disposition                                                                                                                                                                               | Evidence                                                                                                                                     |
| ------------------------------ | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Testing (Overall)              | Error   | **Partially addressed** — closed for `selected_error`, `reportable_errors`, `merged_file_value`, `push_into`; the "CLI/environment selector precedence" ask is closed by the Round 6 fold | Each of the four named items has assertions at `scoped_layers.rs:271`, `:305`, `:348`, `:359`; ordered precedence at `:187`                  |
| Testing (Unit And Behavioural) | Error   | **Partially addressed** — both bracketed and parenthesized list forms now tested, plus all four `project_root_from` failures                                                              | `discovery_validation.rs` (6 tests); valid `Option<PathBuf>` at `policy_sources.rs:90`                                                       |
| Unit Architecture              | Error   | **Repaired**                                                                                                                                                                              | `ScanError(OSError)` at `source_scan.py:30`; `os.walk(..., onerror=refuse)` at `:65`; all six named functions exist and route through `scan` |
| Docstring Coverage             | Warning | **Partially addressed by constraint** — see Round 7                                                                                                                                       | Delta private items 100% except two files at the 400-line cap                                                                                |
| User-Facing Documentation      | Warning | **Addressed** — the guide work landed at `e8adc959`; the migration-guide half is Round 8                                                                                                  | `developers-guide.md:732`; `documentation_examples_tests.rs:42`                                                                              |
| Developer Documentation        | Warning | **Already satisfied on this head; the walkthrough text is stale**                                                                                                                         | `developers-guide.md:732`, `:750`, `:778`, `:836` name every type the row lists                                                              |
| Testing (Property / Proof)     | Warning | **Addressed**                                                                                                                                                                             | `scoped_stacking_proptest.rs` is a reference-model property suite over generated candidate sets, scope orders, and canonical aliases         |
| Testing (Compile-Time / Ui)    | Warning | **Declined, with a reason**                                                                                                                                                               | See below                                                                                                                                    |
| Observability                  | Warning | **Addressed**                                                                                                                                                                             | `load_outcome` ends in `count_outcome` (`telemetry.rs:295`); `discovery.policy` event added; both documented                                 |

### The compile-time row is the one row this branch declines

The row asks for trybuild cases in the discovery-attribute vocabulary. A
trybuild harness **does** exist — `ortho_config/tests/ui/` carries nine cases
with committed `.stderr` fixtures, driven by six `*_trybuild.rs` suites — but
those nine cover CLI flags and merge strategies, not discovery attributes.

The ask was declined for a reason that was established before the code was
written, and it is not a preference: **the errors this row wants to see are
raised during token generation, not during parsing.** `mode_tokens` and
`scope_order_tokens` are pure functions, so their rejections are tested
directly and exhaustively in `policy_impl.rs`'s `#[cfg(test)]` module — six
cases covering both defaults, all four legal spellings, both unknown-value
errors, and all three scope errors. A trybuild case for any of those would
compile the identical error message through a second, slower, more fragile
instrument and assert strictly less: a UI fixture pins one message, while the
unit test pins the message *and* the variant it maps to.

The parse-stage errors are a different matter and a fair ask. They live in
`discovery_validation.rs`, which is ten lines cheaper than a UI fixture for the
same guarantee and runs without a compiler subprocess. Adding `.stderr`
fixtures would also mean committing files that only the compiler can emit — the
`trybuild` constraint this plan already records above.

If the maintainer prefers the UI spelling for contract stability, the four
parse cases convert directly; the token-generation cases do not, and should
stay as unit tests.

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

**Why it was deferred, and why that reasoning did not survive.** The deferral
argued that the change moves the deduplication key from "resolve whatever path
the layer reports" to "trust the layer", and that proving the two keys agree is
a Windows-shaped question because `assert_layer_path` exists precisely because
two resolutions of one file can differ in *representation* there.

That argument contains its own refutation once the call graph is traced. The
two keys are not produced by two different resolvers:

- `load_config_file_as_chain` (`loader.rs:181`) calls `with_cycle_detection`,
  which resolves the file **once** at `loader.rs:119` and passes that
  `canonical` into the operation it invokes.
- `load_chain_for_file` (`loader.rs:188`) takes that same value — its parameter
  is literally named `canonical` — and stamps it into the layer at
  `loader.rs:198` via `to_utf8_path`.
- Every one of the four `MergeLayer::file` construction sites
  (`load.rs:232`, `load.rs:272`, `policy.rs:138`, `composer.rs:86`) receives
  its path from that chain, not from a caller's spelling.

So `layer.path()` *is* the loader's resolution, and the old second lookup was
feeding the loader's own function into itself — idempotent by construction, and
therefore incapable of disagreeing. `unique_layers` has exactly one caller,
`scoped.rs:148`, fed directly by `chain_layers` (`scoped.rs:146`), so no other
path reaches this filter. The representation risk is real for
`assert_layer_path`, which compares a *fixture* path against a stored one; it
does not exist here, where both sides are the same value.

The remaining argument — that it is a silent-error path rather than a wrong
answer — was accurate, and is exactly why it is worth fixing: the swallowed
error is the only thing the second lookup contributes. Removing it deletes a
syscall per layer and retires the silent fallback together, because there is no
longer a fallible call whose error could be dropped. Fixed in this round,
against the Windows job that now passes on this head.

The generalizable lesson: *a deferral justified by "the two may differ" is
dissolved by showing they are the same call site*, and that is a local
question, not a platform one. It was recorded as needing Windows CI; the real
reason it looked Windows-shaped was an unexamined assumption that the layer
path and the lookup path came from different sources. Tracing four construction
sites took less time than the deferral had already cost.

The warnings are recorded with their dispositions rather than dismissed:
`Docstring Coverage` (70.16% against an 80% threshold, scoped to touched
functions) and `Testing (Property / Proof)` are genuine gaps this plan already
names as outstanding work; `Testing (Compile-Time / Ui)` and the two
documentation warnings concern coverage of the policy API by trybuild fixtures
and the user/developer guides, which the plan's work items 4 and 5 partly cover.

## Big picture

PR #465 resolves issue #318 by adding scoped configuration-file discovery plus
an opt-in policy loader. Two objectives were open when the review opened, and
**both have since been closed** — the entries are kept with their disposition
so an early reader does not take the problem statement for current state:

1. ~~`StackScopes` composition stops at the **first** successful candidate in
   each scope, so it does not load "all applicable files". #318 stays open.~~
   **Resolved.** `ConfigDiscovery::scope_candidates` now walks every in-scope
   candidate and `unique_layers` collapses only genuine repeats, so a scope
   contributes all applicable files. `#318` is answered by this PR; see
   `compose_scoped_layers` in `ortho_config/src/discovery/scoped.rs` and the
   `scoped_stacking.rs` suite.
2. ~~`ortho_config/tests/scoped_layers.rs::compose_layers_remains_first_wins`
   fails on Windows: it compares `MergeLayer::path` with the raw `TempDir`
   path, while the loader stores the canonicalized path.~~ **Resolved.** The
   comparison moved into the shared
   `tests/support/layer_assertions.rs::assert_layer_path`, which canonicalizes
   both sides before comparing, so the assertion is about which file won rather
   than how each side spelt it. The Windows job passes.

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
2. **It is introduced by this branch.** `trybuild_tier.py` is added here (+227
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

The per-crate walk moved to `_crate_test_binaries(base, crate)`, whose two
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

### The same defect was waiting in a sibling file

Fixing the one finding did not clear the check. CodeScene re-analysed the
pushed head `8c8234d1` at 19:48Z and failed it again, so the second reading was
not a stale verdict — the tool had looked at the new commit and found something
else. A local sweep of every changed file `cs check` can read found it:
`tests/workflow_contracts/nextest_budgets.py:108`, the same **Bumpy Road Ahead
(bumps = 2)**, scoring 9.84.

That function is `override_allowances`, added by this branch in the same shape
as the one just repaired — a loop body carrying a guard (`continue` on a table
that is not an override, or that declares no `slow-timeout`) and, beneath it, a
nested conditional that falls back from `filter` to `platform` and raises when
neither is present. Two blocks with nested conditional logic, exactly the rule.

**`main` is clean, which is what makes it this branch's.** `cs check` on
`git show origin/main:tests/workflow_contracts/nextest_budgets.py` scores
10.00, so the finding is introduced by this branch rather than inherited, and
the plan's earlier "introduced by this branch" reasoning applies here too.

The fallback moved to `_override_selector(path, table)`, which answers a
question worth naming — which selector does this allowance apply to? — and the
loop body is now a guard, one call, and one append. The finding is gone and the
file scores 10.00.

Two things this round learned that the previous one did not have to consider:

- **The 400-line cap is live here.** `nextest_budgets.py` was 394 lines against
  the `AGENTS.md:33` limit, so an inline helper would have breached a hard rule
  to satisfy a style rule. The first draft of the helper moved it to 399, which
  is why its docstring was trimmed to the one-line form the module's other
  private helpers (`_table`, `_parsed`, `_budget_tables`) already use, with the
  rationale kept as a comment. That is 397 lines with room to edit.
- **Behaviour was proved, not assumed.** An inline reimplementation of the old
  body and the new function agree on all seven shapes that matter: `filter`,
  `platform`, both together, neither, no `slow-timeout`, no overrides at all,
  and multiple overrides — including the raised message text, character for
  character. They also agree on the real `.config/nextest.toml`.
  `make test-workflow-contracts` passes 313 with 1 skipped.

## Round 3: seven findings, four repairs

CodeRabbit returned a second `CHANGES_REQUESTED` on `d43a5629` (review
`5331986332`, 2026-09-27T20:42:06Z) with seven inline comments, all
`🟡 Minor | ⚡ Quick win`. Each was verified against the current source before
any edit. Four were real defects and are repaired; three are recorded below
with their dispositions.

**`nextest_budgets.py` refused a legal configuration — the significant one.**
The `platform` key is an override **gate**, not a filter, and the previous
round's helper (added by this branch) string-interpolated it into a
`platform(...)` filterset term and returned that as the override's selector.
Two things are wrong with that, and both were settled by probing the installed
`cargo-nextest` rather than by reading its documentation:

1. **It refuses a valid file.** A probe crate with
   `platform = { host = "cfg(unix)" }` and no `filter` runs fine under nextest,
   while `_override_selector` raised `NextestConfigurationError` — whose
   message asserts "nextest refuses such a file", which is false for that
   shape. The acceptance rule is in the nextest binary: *"at least one of
   `platform` and `filter` must be specified for override"*.
2. **`platform(...)` is not nextest's platform field.** A filterset
   `platform(...)` takes `host` or `target`; probing `platform(unix)`,
   `platform(/unix/)` and `platform(cfg(unix))` as `filter` values each failed
   with *"expected `target` or `host`"*. So the old code fed nextest a
   predicate nextest rejects, and passed it to a function that evaluates
   `binary(...)` terms — a category error the reviewer named exactly.

The repair returns `None` for a platform-gated override rather than inventing a
filter for it: such an entry applies to every test under that platform, which
no filterset describes. The one consumer (`trybuild_tier_test.py`) now refuses a
`None` from the entry carrying the largest allowance instead of returning it
as a filter expression, and a doctest pins the new shape. `platform` in
*either* form (string or table) selects `None`, and an override with neither
key still raises.

**A deferral reversed.** The `scoped.rs` finding was previously deferred on the
grounds that it needed Windows CI; tracing the call graph showed the premise
was wrong, and it is repaired in this round. See *Big picture* and the
reasoning recorded with it. Mutation-checked: restoring the old swallowed-error
body, and separately neutering the filter, each fail the deduplication tests.

**Two documentation repairs.** The guide's completion claim about the whole-run
budget overstated what `test_a_whole_run_budget_would_sit_inside_each_watchdog`
asserts — the test compares the global timeout against the per-test allowance
and contains no scheduling term — so the sentence now states what is actually
established. The plan's "Big picture" no longer presents two closed objectives
as open, and one first-person pronoun is gone.

**The trybuild inventory, and two defects the repair itself introduced.** Both
`trybuild_tier.py` findings — nested declared test paths, and automatic
discovery — came from one root cause: the inventory enumerated `tests/*.rs` and
keyed each binary by its file stem, so a `[[test]]` target rooted at
`tests/foo/mod.rs` was never visited, and a binary cargo discovers without any
`[[test]]` entry was never named. Both are fixed by one rewrite that enumerates
*both* kinds from a single shared reader, so the class and the negative control
cannot disagree about which binaries exist. The negative control grew from 3
names to 53 while the class stayed at 7 — no false positive.

The two halves are now a single `_workspace_test_binaries`, and that is where
the first self-inflicted defect appeared. Merging the per-crate inventories with
`dict.update` is **last-write-wins over bare names**, but a nextest
`binary(...)` name is workspace-wide: `rstest_bdd` is a declared target in
*both* audited crates, so `cargo-orthohelp`'s plain copy overwrote
`ortho_config`'s trybuild one and dropped the name from the class. The coverage
assertion would then have certified `rstest_bdd` as correctly uncorrected — the
same masking the finding names, reached through the merge rather than the walk.
The union is now an OR (`found.get(name, False) or carries`), verified by the
probe that exposed it: appending a trybuild call to `tests/rstest_bdd/mod.rs`
moves the name into the class, and the clean tree still reads 7 in / 53 out.

This is worth recording because the probe that found it *looked* like a broken
test rather than a broken fix. `_carries_trybuild` returned `True` when the
roots list was built by hand and `False` through `_workspace_test_binaries`, in
one process, on one file state. The instinct to re-run the probe would have
been wrong; the two calls disagreeing in the same process is the signal.

The second defect was a rule breach the repair created: `trybuild_tier.py` grew
to 424 lines and `nextest_budgets.py` to 419, both over the **400-line hard
limit** (`AGENTS.md:33`). HEAD was clean at 334 and 397, so this branch broke
it. Splitting to two new modules — `nextest_filterset.py` (the filterset
language) and `nextest_document.py` (locating the tables a budget sits in) —
brings all four to 320 / 167 / 88 / 357. The first is a seam the module already
had: `trybuild_tier` knows which binaries are trybuild ones but not how to read
a filter, and the matcher reader never needed to know about binaries. The split
also deleted `GLOB_METACHARACTERS`, a constant referenced nowhere in the
repository.

`cs check` scores all five touched modules 10.00 and
`make test-workflow-contracts` passes 316 with 1 skipped.

**One finding with a disposition but no edit:**

- `Docstring Coverage` remains an outstanding repository-wide policy gap this PR
  does not meet on its own new code, as recorded in the pre-merge
  reconciliation.

## Round 4: the pre-merge table re-read on the current head, and one row that is a real defect

The window validation closed the question the previous round left open, and the
re-read of the table on `9e9ecc37` found a different table from the one the
earlier reconciliation examined. That round's rows were bound to `75d9901d`;
these are bound to the current head, so nothing here is stale by construction.
**3 errors and 6 warnings**, and four independent read-only verifications were
run against the code before any edit was authorized.

**The window question is closed.** `build-test (windows-latest)` is green on
`9e9ecc37` — run `36353886506`, all five jobs green, `head=9e9ecc37`. Both legs
passed with zero failures: 1326 tests (9 slow) and 1302 (5 slow), 10 skipped in
each, coverage 85.59%, artefact archived. The specific test the platform
requirement names,
`ortho_config::scoped_layers::compose_layers_remains_first_wins`, **passes in
both legs**. The earlier fail-fast run that left 225 tests unrun is superseded,
and the `4ca6483e` evidence — green, but not an ancestor of this head — is
retired rather than reused.

**Three of the nine rows are unambiguously real and were already known.** Both
documentation warnings are confirmed by direct count: `ConfigFilePolicy`,
`ConfigPathSelector`, `StackScopes`, and `scope_order` each occur **zero**
times in `docs/users-guide.md` and zero times in `docs/developers-guide.md`.
The compile-time warning is confirmed the same way: `project_root_from` occurs
in zero test files, fixtures, or `.stderr` files, and `env_vars` occurs in no
test input. The property-test warning is cheap to satisfy because
`proptest = "1.11.0"` is already a dependency.

**Verification sharpened the remaining rows rather than accepting them.**

- *`Unit Architecture`* named five functions that supposedly swallow
  filesystem errors. They do not swallow: the module contains **no** `try`/
  `except`/`suppress` at all — the `try` grep hits are substrings of "trybuild"
  — and stdlib `pathlib` re-raises `PermissionError` because `_IGNORED_ERRNOS`
  excludes `EACCES`. One genuinely silent path exists: `rglob` delegates to
  `Path.walk`, which does `except OSError: continue` with no `on_error`
  supplied, so an unreadable **sub**directory beneath a `mod.rs` root is
  skipped silently and the binary leaves the trybuild class unseen by the
  coverage assertion. The narrow fix is taken; the requested injectable
  filesystem interface is not, because its severity premise is false. Ownership
  was settled by direct `git` inspection, which the reconnaissance could not
  run: `git cat-file -e origin/main:tests/workflow_contracts/trybuild_tier.py`
  fails, so the file is branch-added and in scope.
- *`Observability`* is **larger** than the row states and is the round's real
  defect. The row describes a partial gap; the explicit-selection branch
  actually emits **nothing at all**. `policy.rs` contains zero `telemetry::`
  calls, so an operator running a policy-enabled struct with a bad explicit
  `--config bad.toml` sees no attempt, no candidate failure, and no terminal
  outcome, where the same failure through the legacy path yields
  `discovery.attempt`, `discovery.candidate`, and
  `discovery.load{outcome=not_found}`. Nothing in `docs/design.md:353-358` or
  RFC 0002 exempts the policy path from the contract, and no test pins its
  silence, so this reads as ungoverned rather than deliberately out of scope.
- *`Docstring Coverage`* measures something narrower than it appears to.
  `missing_docs = "deny"` is enforced workspace-wide (`Cargo.toml:99`), so
  every genuine public item is already documented; the 74.65% counts private
  helpers and test functions. That makes the row a real but low-value gap
  rather than the API-documentation hole the number suggests.

Two constraints decided how the compile-time tests are written, and both were
established before any code was committed. First, a `compile_fail` case needs a
**committed `.stderr`** that only the compiler can emit, so no unreviewed
fixture is added under `ortho_config/tests/ui/`. Second, the mode and scope
errors are raised in token generation (`mode_tokens`, `scope_order_tokens`),
not in parsing — which makes them **pure functions** testable directly in a
`#[cfg(test)]` module, a cheaper and stronger instrument than a UI case. The
parse-stage errors do have an established test idiom in
`parse/tests/ortho_attrs.rs`.

### The two documentation findings, verified and repaired

Both are real, both are on this plan rather than on code, and both are
confirmed against the file rather than against the comment's own anchor.

- **The method's owner (line 266).** The claim is true and the repair is the
  proposed one. The name it replaces, `ScopeOutcome`, exists **nowhere in the
  repository** — `git grep` over all tracked files returns nothing, so the plan
  cited a type that was never written. The method is defined at
  `ortho_config/src/discovery/scoped.rs:177`, inside `impl ConfigDiscovery`
  (`scoped.rs:44`), so the reviewer's proposed owner is the correct one and the
  line now reads `ConfigDiscovery::scope_candidates`.
- **The second-person pronoun (line 913).** Also true, and also verbatim. The
  sentence was a probe-guidance bullet reading "A probe that contradicts *your
  fix* may be reporting the fix…"; it now reads "the fix". The bullet's meaning
  is unchanged, which is the test that the repair is minimal rather than a
  rewrite.

Neither finding needed a code change, and neither contradicts anything else in
the plan, so both were applied as stated.

### Round 5: the `Observability` repair, and a plan note that was wrong

The `Observability` row from the table above was taken as the round's real
defect, and its repair is now in the working tree. `policy.rs` went from zero
`telemetry::` calls to ten, `telemetry.rs` gained `OPERATION_POLICY_RESOLVE`
(`policy_resolve`), the closed `SELECTOR_CLASS_{CLI,ENVIRONMENT}` pair, and
`policy_resolution`, and a new support module
`ortho_config/tests/support/policy_telemetry.rs` pins the emitted vocabulary.

Three details of that repair are worth recording, because each corrects a claim
made earlier in this plan.

- **The `discovery.attempt` vocabulary is unchanged.** An earlier note in this
  plan proposed adding `policy_resolve` to the `operation` list that
  `docs/design.md:415` documents. That note was **wrong**: `telemetry::attempt`
  has exactly two call sites, `load.rs:120` and `scoped.rs:90`, and neither is
  on the policy path. Adding the value there would have documented a value the
  event can never carry. The row as written was already correct and is left
  alone.
- **`discovery.load` is the event that actually gained a value.** `policy.rs`
  calls the existing `load_outcome` with `operation = policy_resolve` and
  `source = None`, because the policy path deliberately bypasses the candidate
  machinery and so has no winning rung to name. Both `design.md`'s
  `discovery.load` bullet and the developers-guide's `discovery.load` row
  therefore needed `policy_resolve` added, and its "emitted from" cell widened
  to name `policy`.
- **`selector_class` is omitted, not empty.** The automatic branch emits
  `discovery.policy` without the field at all, so an absent field cannot be
  mistaken for a class. The test support helper renders a missing field as the
  empty string, so the assertion at `policy_telemetry.rs:107` reads
  `.field("selector_class") == ""` — which is the helper's convention for
  "absent", not a claim that the event carries an empty value. The assertion
  has since moved to `policy_telemetry.rs:146`, when Round 12 split the
  mode-outcome case in two.

**A second, unrelated blocker surfaced.** The new proptest file
`ortho_config/tests/scoped_stacking_proptest.rs` had grown to **414 lines**
against `AGENTS.md:33`'s hard 400-line cap. Its five-row `VOCABULARY` table had
been written as struct literals plus a `const fn candidate(...)` constructor,
and `rustfmt` expands each struct literal to seven lines. The fix replaces the
`Candidate` struct and its constructor with a five-field **tuple type alias**
`Rung = (usize, usize, i32, &'static str, bool)`, which `rustfmt` leaves on one
line per row. Every doc comment on the old struct was moved onto the alias, so
no documentation was dropped, and the rename also removes a collision with the
unrelated production type `discovery::candidate_set::Candidate`. A small
`is_alias(Rung)` helper replaced the one predicate that had read a named field;
it takes the rung **by value**, since `trivially_copy_pass_by_ref` is a
pedantic lint and clippy runs with `-D warnings`. The file is now 397 lines and
`rustfmt --check` clean.

### Round 6: two test gaps a wyvern adjudication turned up

A read-only wyvern was given the three testing rows and the two architecture
rows and asked to verify each claim against the current tree rather than the
comment anchors. Its verdict on the testing rows was that almost every named
behaviour is already covered — ordered selector precedence, the selection
metadata (`label`, `legacy`, `path`), required and optional malformed files,
`selected_error`, `reportable_errors`, `merged_file_value`, `push_into`, and
`into_layers_and_errors` all have assertions — and it named **two** genuinely
thin spots, which were then confirmed by reading the source directly:

- **A list attribute's parenthesized spelling had no test.**
  `assign_string_list` (`ortho_config_macros/src/derive/parse/mod.rs:169`)
  reads a list two ways: the `= [...]` array, and a bare `( ... )` form with no
  `=`. Only the array branch was exercised anywhere in the tree. The missing
  form is now pinned by `parses_parenthesized_env_vars_and_scope_order`, and
  the two forms really are distinct — `peek(Token![=])` is what selects the
  branch, so writing the parenthesized list *with* an `=` would take the array
  branch and fail to parse a `(...)` as a `syn::ExprArray`. The first draft of
  this test had exactly that bug and was corrected before commit.
- **A populated CLI rung had no policy-path test.** `three_rung_policy` builds
  `ConfigPathSelector::cli(None)` and never populates it, so no test proved
  that a CLI rung carrying a path outranks a populated environment rung on the
  `ConfigFilePolicy` path; the equivalent assertion exists only on the legacy
  path (`clap_integration/config_path.rs`, `#[case::cli_overrides_env]`). A
  third scenario now sets both rungs and asserts the CLI one wins. It required a
  `three_rung_policy_with(source, cli)` spelling, because the original
  helper's hard-coded `None` would have made the new scenario vacuous — it
  would have passed whether or not the chain were ordered at all.

The same file-size cap bit again: adding the scenario took
`ortho_config/tests/scoped_layers.rs` from 372 to **410** lines. Rather than
trim assertions, the scenario was folded into the existing precedence test and
the helper was parameterized, leaving the file at exactly **400**.

The other rows were adjudicated from the source rather than from the
walkthrough text, and two of them are **already satisfied** on this head:

- **Developer Documentation** asks for a developer-guide section covering the
  new public types, the two automatic modes, scope ordering and precedence,
  canonical-path deduplication, `extends` ordering, replay through
  `FileLayerOutcome`, `project_root_from`, and the derive keys. That section
  exists: `docs/developers-guide.md:732` "Scoped and policy-based configuration
  discovery", with `### Type roles and how they compose`,
  `### First-wins and scope stacking are two code paths`, and
  `### Scope order, and the two opposite orderings`, pointing at RFC 0002 for
  the semantics tables. Every type the row names appears in the file —
  `ConfigFilePolicy` (×4), `FileLayerOutcome` (×2), `ConfigPathSelector` (×2),
  `AutomaticMode` (×3), `ExplicitMode` (×3), `StackScopes` (×2), `FirstWins`
  (×3), `env_vars` (×3), `project_root_from` (×2). Only `DiscoveryScope` is
  absent by name. The row's "searches find no …" describes an older head.
- **User-Facing Documentation** asks for a user's-guide section with working
  examples plus a migration-guide signpost. The guide work landed at `e8adc959`:
  `### Select a file explicitly`, `### Root the project scope`, and
  `### Configure the policy from the derive`, with a
  `<!-- tested-example: guide-scoped-discovery -->` example. **Correction
  (Round 14, and acted on in Round 15):** the claim originally written here —
  that `ortho_config/tests/documentation_examples_tests.rs:42` "actually
  compiles and runs" the fence — was **false**. That line is only the
  `EXPECTED_EXAMPLE_IDS` registry, which proves the fence exists and has a
  unique identifier and nothing more. The fence was absent from
  `STANDARD_RUST_EXAMPLES`, so it was never built. Round 15 added it to that
  list and to `assert_env_alias_chain`
  (`documentation_examples/env_alias_chain.rs`), which is what makes the
  sentence true; before that commit the only honest statement was that the
  fence was registered, not executed. The migration-guide half is the one
  genuinely open piece and is handled in the reconciliation below.

The scribe then found a defect **in the brief it was given**, which is worth
recording because the delegation had originated here. It was told to add
`policy_resolve` to `discovery.load`'s `operation` list, and did; but it
pointed out that naming `policy_resolve` there makes the row's `outcome` list
wrong. The legacy path reaches `load_outcome` with only `success` or
`not_found` — a failed candidate is reported through `discovery.candidate`
instead — whereas the policy path has no candidate event at all and so carries
all four terminals itself: `success` (`policy.rs:149`), `not_found` (`163`),
`required_failure` (`170`, `178`), and `optional_failure` (`186`). Both
documents therefore under-described the operation. The `outcome` list in
`docs/design.md` and the `outcome` cell in `docs/developers-guide.md` were
widened to say so, and the reason is stated inline rather than left as
unexplained vocabulary.

Applying the guide edit pushed its telemetry row past the table's existing
column width, so `mdtablefix --in-place` re-padded all twelve rows to a uniform
343 columns. That is the tool `make check-fmt` runs, so the reformat is what
the gate would have demanded anyway; doing it here rather than discovering it
in the gate keeps the gate run clean.

### Round 7: the Docstring Coverage row, closed as far as the cap allows

The architecture wyvern put this row's largest single contributor at
`policy_impl.rs` (12 of 14 items undocumented) and confirmed by arithmetic that
the 74.65% figure counts **private helpers and `#[test]` functions**, not
public API — the same reading Round 4 reached, now independently reproduced.

The figure was reproduced locally rather than taken on trust: a scanner over
the **418 tracked `.rs` files** reports 998 of 2899 functions documented,
**34.4%**, against the published 74.65%. The two are not in conflict, and the
gap between them is the useful part — a broad scanner counts `#[cfg(test)]`
modules, macro bodies, and trait-impl methods that the hosted metric evidently
excludes or weights differently. So the hosted percentage is **not**
reproducible by local inspection, and no local run can be used to claim the row
is closed. What local inspection *can* do is establish that the delta's own
private items are documented, which is the part this PR owns.

That was done, on the ten files in the delta:

| File                                             | Before | After           |
| ------------------------------------------------ | ------ | --------------- |
| `ortho_config_macros/src/derive/policy_impl.rs`  | 8/14   | 14/14           |
| `.../derive/parse/tests/discovery_validation.rs` | 1/6    | 6/6             |
| `ortho_config/src/discovery/telemetry.rs`        | 12/19  | 19/19           |
| `ortho_config/tests/discovery_telemetry.rs`      | 8/16   | 16/16           |
| `ortho_config/tests/support/policy_telemetry.rs` | 4/5    | 5/5             |
| `ortho_config/src/discovery/policy.rs`           | 23/27  | 23/27 (no room) |
| `ortho_config/tests/scoped_layers.rs`            | 6/14   | 6/14 (no room)  |

The last two are at **exactly 400 lines** — AGENTS.md's hard cap — so neither
can accept another doc line without splitting the file, and splitting a test
suite to satisfy a percentage is the tail wagging the dog. Both are recorded as
**partially addressed by constraint**, which is a disposition rather than a
silence. The row therefore stays open against the hosted threshold, and the
plan's Round 4 reasoning for why that is acceptable (every genuine public item
is already documented under `missing_docs = "deny"`) stands unchanged.

One self-inflicted defect is worth recording, because it would have been a hard
compile error and it was caught only by re-reading the diff: a scripted edit
inserted the `count_attempt` doc comment **above the existing function** as
well as the new one, leaving two `#[cfg(feature = "metrics")] fn count_attempt`
at the same scope. `make typecheck` builds `--all-features`, so this is E0428,
a duplicate definition, not a warning. The duplicate was removed and the file
re-checked. The lesson is the one this plan keeps relearning: scripted edits
need the resulting diff read, not just the tool's success message.

### Round 8: the migration guide gets its signpost

The User-Facing Documentation row's last open sub-ask was a v0.10.0 migration
signpost, and it was the one item the guide work did not cover. The migration
guide is the right home: `Cargo.toml:17` still reads `version = "0.9.0"`, the
newest tag is `v0.9.1`, so v0.10.0 is the release this lands in and the guide
for it is the one a user upgrades through. A new
`## Load files from more than one scope` section sits between the policy-check
and default-behaviour sections, states that nothing changes by default, names
both opt-in spellings, and links to the guide section that documents them.

**The delegated draft needed three corrections, and each is a lesson.**

1. A grammar slip — a missing infinitive marker in "Two ways opt in" — which
   the agent did not flag and which only reading the diff caught. Delegation
   moves the typing, not the proofreading.
2. A wrong anchor. The draft linked
   `users-guide.md#configure-the-policy-from-the-derive` (`:571`), which is the
   derive-attribute subsection. The right target is the parent section,
   `users-guide.md#control-which-configuration-files-become-layers` (`:492`),
   whose opening paragraph *is* the `FirstWins`/`StackScopes` distinction. A
   subsection anchor would have worked; the parent is better and the agent had
   no way to know which the brief meant.

   **The wrong anchor came back.** A later delegation, handed a brief that
   specified the derive-attribute anchor, found the definition already pointing
   at the parent, and — correctly refusing to overrule its brief silently —
   restored the anchor it had been given and flagged the disagreement. It was
   reinstated again, and the hierarchy is what settles it: the section names
   **two** opt-in routes, the derive key and
   `ConfigFilePolicy::from_builder(...).automatic_mode(...)` on a hand-built
   policy, and only the parent heading contains both. The derive heading is a
   `###` beneath it, and the hand-built API is documented above it at `:527` and
   `:564`. Both anchors resolve, so no amount of static checking would have
   caught this; the heading levels had to be read. A brief that names one of
   two targets is the defect to fix, not the edit that follows the brief.
3. **A line the gate would have failed, that does not fail this gate.** The
   reference definition was 84 columns. `.markdownlint-cli2.jsonc` sets MD013 to
   `line_length: 80` with only `tables` and `headings` exempt, so a reference
   definition looks exposed — but running `markdownlint-cli2` on the file
   reports **0 issues**, because MD013 does not apply to link-reference
   definitions at all. The fix was therefore unnecessary and was **reverted**
   rather than kept: the descriptive label `users-guide-scoped-discovery` is 94
   columns and stays, because the gate that would object does not.

   The lesson is the sharper one: the constraint this plan has been applying
   all along — "wrap prose at 80" — is a `mdtablefix` and MD013 rule about
   *prose*, and inferring its scope from the config file rather than from the
   tool's behaviour invented a problem. Running the actual linter took ten
   seconds and settled it. **Read the config to know the rules; run the tool to
   know the verdict.**

### Round 9: the round-4 findings, and a lock nobody owns

Three of the four findings scrutineer returned were still live and are repaired
here; the fourth is resolved.

1. **The Python scan guard, which is a version difference rather than a bug in
   the change.** `test_an_unreadable_crate_root_is_refused_not_read_as_empty`
   failed on the gate's interpreter and passed on the local one. The cause is
   `Path.iterdir`: on 3.12 it is a generator, so the `PermissionError` surfaced
   inside `list()` and inside `_scan`'s guard; on 3.14 it is a plain function
   calling `os.scandir`, so `_scan(tests, list, tests.iterdir())` evaluated its
   argument *outside* the `try` and the raise escaped. A new `_listing` helper
   defers the whole read — enumerate and materialize together — so both sit
   behind the guard on either version. **The first draft of the helper returned
   `directory.iterdir` itself, which is still a lazy generator on 3.12 and
   would have deferred the raise past `sorted()` instead, breaking the other
   interpreter.** A fix for a version difference that is only tested on one
   version is not tested. Confirmed on 3.14.4, the exact interpreter that
   failed: 319 passed, 1 skipped, 0 failed.
2. **Five spellings in the execplan, three of them in an inline code span.** The
   house rule is en-GB-oxendict (`AGENTS.md:24`), so the `-ised` forms are
   genuinely faults. The test the plan named was renamed to the `-ize` spelling
   and the sentence reworded to match; the prose forms were corrected too.
   Renaming defeated the point of the previous round's decision. The sibling
   `-ised` identifier at `clap_attrs.rs:300` predates this branch on
   `origin/main`, so it is not this branch's to rename; the precedent now
   followed is that the overlay already uses for quoted identifiers. The gate
   scans prose and inline code spans and does **not** flag `.rs` files —
   proven, not assumed: the same spelling at `clap_attrs.rs:300` is tracked and
   unflagged, while the plain-text rewrite of it in the plan had failed the
   gate minutes earlier.
3. **A newly written paragraph tripped `check-fmt`.** `mdtablefix --wrap`
   wanted a line joined at 80 columns. Fixed with `make fmt` (the tool the gate
   runs) rather than by hand-wrapping, which is what produced the discrepancy
   in the first place: `mdtablefix` reflows with its own fragment model, so a
   hand-wrapped paragraph is only accidentally its fixed point.
4. **The migration-guide anchor, reinstated a second time.** See item 2 of
   Round 8 above; the parent heading is the right target and the brief was
   wrong.

**The three Rust gates have no verdict, and this time the cause is named: a
closed lock cycle, not load.** `make typecheck`, `make lint`, and `make test`
all blocked on the machine-wide Cargo package cache. `/proc/locks` shows one
exclusive flock held by PID 1832225 — a foreign `cargo test` in the podbot
worktree, parked ~4.5 hours at 4 seconds of CPU — with 67 queued readers. The
holder is blocked in `do_wait` on its own descendant, and that descendant is
itself queued for the lock the holder owns. **No amount of waiting clears it,
and the constraint is explicit: other agents' processes are not this branch's
to kill.** The earlier reading of this same process as merely slow was wrong;
it is self-deadlocked and needs its owner. Recorded so the next attempt
escalates against a diagnosis instead of re-deriving it.

Both mistakes this round belonged to the delegating side, and both were the
same shape: a brief asserting a fact that had not been checked — a script that
does not exist, and an anchor that was one of two candidates. Confirmed after
the fact by reading `AGENTS.md:138` (`make check-fmt`, `make lint`,
`make test`, then commit; there is no gated-commit wrapper) and by reading the
heading levels.

### Round 10: the commit carried a compile error CI found and the local gate could not

The commit above was pushed with three Rust gates unproven — the deadlock left
them unrunnable — and CI went red on it. The hosted `Lint` step reported
`error[E0597]`, a borrowed value that does not live long enough, at
`ortho_config/tests/policy_sources.rs:130:28`.

`load_root_from_map` was declared
`args: impl IntoIterator<Item = &'static str>`, but the test feeds it
`--project-root` with a path built from a `TempDir` at run time, so the borrowed
`&str` cannot satisfy `'static`. The helper has no reason to narrow the bound:
the trait method it forwards to already accepts
`Item = T where T: Into<OsString> + Clone`, and a generic `S` with that same
bound passes the argument through untouched.

**None of the seven local gates would have caught this, and that is the point.**
`check-fmt` runs `cargo fmt`, which parses but does not typecheck.
`markdownlint`, `test-workflow-contracts`, and `nixie` do not read Rust.
`make typecheck`, `make lint`, and `make test` are the three that would, and
those are exactly the three the foreign deadlock blocked. A green local board
on this branch was never evidence that the Rust tree compiled, and the plan is
the place that says so: **a gate that cannot run is not a gate that passed, and
the three that could not run were the only three that would have caught this.**

**Both CI legs failed on this one error, and it is worth saying so
explicitly.** Reading only the ubuntu leg invites the guess that Windows failed
for a platform-specific reason of its own. It did not.
`build-test (windows-latest)` stopped at step 20, `Lint (Clippy only)`, on the
same `error[E0597]` and the same `policy_sources.rs:130:28`; the logs of the
two legs are the same failure on two runners. The branch's net diff touches no
`#[cfg]`-gated code, so one bound widening answers both. The run's Markdown
lint and spelling steps passed on Windows as well, which is a second
confirmation of the earlier reading of the spelling gate: it inspects Markdown
and inline code spans, not `.rs` sources.

The deeper cause is the commit's own shape. `51e2e974` swept up a large body of
previously-uncommitted work along with the round-4 repairs — 16 files, ~1900
insertions — so the compile error arrived in a commit whose message described
only the scan guard, the spellings, and the anchor. Small, focused commits are
what `AGENTS.md:65` asks for and what makes a failure like this attributable to
one change rather than to sixteen.

### Round 11: the fix cleared one CI error and exposed the one behind it

The bound widening worked, and CI went red on a *different* error that the
first had been hiding. `build-test` failed on both legs at the `Lint` step with
`function write_config is never used` at
`ortho_config/tests/support/scoped_fixtures.rs:56`, reported against the
`scoped_stacking_proptest` binary. Reading only the failing step's first error
would have suggested the fix had not worked; the two errors are unrelated and
the second could not surface until the first was gone.

**The cause is `#[path]` inclusion, not a stray definition.** Four test
binaries include `support/scoped_fixtures.rs` through `#[path]`, and each
compiles it as its own module. `write_config` is called by `scoped_layers`,
`scoped_stacking` and `policy_sources`, but not by `scoped_stacking_proptest` —
which writes bodies carrying an extra unique key, so it needs `write_body` and
cannot use `write_config` at all. In that one binary the function is dead, and
CI's `-D warnings` promotes the warning to an error. A grep for an `allow`/
`expect` on any support module returns nothing, which is why the obvious
reading — "the repo suppresses this somewhere" — is wrong; the suppression is
per-includer and there was none on this one.

**The attribution is exact, and it points here again.** `git log` on
`scoped_stacking_proptest.rs` shows a single commit, `905f55f1`, the rebased
form of `51e2e974`. The suite did not exist at `9e9ecc37`, which is precisely
why that head ran green: this defect arrived with the same oversized commit as
the E0597, and one error masked the other. Two defects, one commit, and the
commit's message named neither.

**The repair follows a precedent already on `main`.** `merge_telemetry.rs` and
`subcommand_paths_telemetry.rs` both carry `#[expect(dead_code, reason = ...)]`
over their include of `tracing_capture.rs`, whose `write_fixture` helpers are
unused by two of that module's three includers — the same shape exactly. The
attribute goes on the includer that does not use everything, never on the
shared file: on the shared file it would be unfulfilled in the other three
binaries, and an unfulfilled `#[expect]` is itself an error. `#[expect]` rather
than `#[allow]` for the same reason — it fails loudly if the last user goes
away.

Two mechanical constraints shaped the final form, and both were measured rather
than guessed. rustfmt's `fn_call_width` of 60 governs the attribute's
*arguments*, so a reason long enough to read well is reflowed onto four lines;
the four-line form took the file to 401 against `AGENTS.md:33`'s 400-line cap.
The accepted reason is short enough to stay on one line, which lands
`scoped_stacking_proptest.rs` at 398 lines — the count verified by `wc -l` on
the current tree. Three attempts at the attribute came first — a 97-character
reason, a shorter one, and then one short enough to fit the arguments — and
each failed `cargo fmt --check` until the width that actually applied was
identified.

**Nothing here was locally proven, and that is stated rather than glossed.**
The package-cache lock was still held by the foreign process throughout (74
queued readers, up from 69), and even an offline
`cargo check --offline --locked` of the single test binary blocked on the same
lock, so no local compiler ever saw this change. CI is the gate of record for
it, and the reasoning for the fix is static: `scoped_fixtures.rs` exposes
exactly two public functions, the suite imports one and references the other
nowhere but the `reason` string, so the `#[expect]` is fulfilled by exactly one
diagnostic.

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

That audit later caught a vacuous test in this branch. The first version of the
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
- **A rebase silently invalidates commit SHAs cited in posted review replies.**
  Thirteen replies here named `59b2b2ae`; the rebase that followed rewrote it,
  and GitHub answered 422 "No commit found for SHA" for the old one. The
  replies had been accurate when posted, so nothing looked wrong — the link
  rots underneath them. Fixing this needs the *weak* form of proof:
  `git patch-id --stable` on both commits, which was byte-identical
  (`90e92d58…`), establishing they are the same change and that pointing at the
  new SHA is truthful rather than a guess. The 13 replies were then edited in
  place, each carrying an explicit note, rather than leaving dead links or
  adding 13 more comments. Two operational traps go with that:

  - **Back up before editing, and verify the backup is real.** The first backup
    pass wrote thirteen 108-byte files — suspiciously uniform, and they were:
    each held a `401 Bad credentials` body. The subprocess had not been given
    the token-unset environment that `gh` needs on this host, so every "backup"
    was an error response. Check that the captured length varies and that the
    content is the comment, not a JSON error.
  - **Prefer editing a reply to stacking another.** A correction that appends
    is one more comment for a reviewer to reconcile; the statement it corrects
    stays on the page either way.
- **Clearing one CodeScene finding does not clear the check, and the second
  verdict is not a stale artefact.** After the `trybuild_tier.py` repair the
  hosted check failed again. It was read as a delayed re-analysis of the old
  head, and that reading was wrong: the check run recorded `head_sha` equal to
  the *new* commit and a completion timestamp after it. CodeScene had genuinely
  re-analysed and found a *second* instance of the same defect class in a
  sibling file. The generalizable move is to sweep every changed file
  `cs check` can read rather than checking only the file a finding named, and
  to compare the check run's `head_sha` against the head being claimed about
  before calling any verdict stale.
- **The 400-line limit shapes where a fix can go.** `nextest_budgets.py` was
  394 lines against the `AGENTS.md:33` cap, so the usual remedy — extract the
  tricky part into a helper in the same module — very nearly breached a hard
  rule to satisfy a style rule. Measure the file after the edit, not before;
  the first draft landed at 399, one line of headroom, and only trimming the
  helper's docstring to the module's existing one-line form brought it to 397.
- **A repair can reintroduce the defect class it repairs.** The trybuild
  inventory fix unified two readers into one, and the merge that united them was
  `dict.update` — last-write-wins over names that are workspace-wide rather
  than per-crate. That silently dropped a class member, which is the *same*
  masking both original findings described, reached through the new code rather
  than the old. When a fix's whole purpose is "these two sets must not
  disagree", the merge that unites them is the highest-risk line in the change,
  not boilerplate.
- **A probe that contradicts the fix may be reporting the fix, not the
  probe.** The clobbering showed up as `_carries_trybuild` returning `True`
  when the roots were built by hand and `False` through the workspace reader —
  one process, one file state. The reflexive move is to distrust the probe and
  re-run it; the informative move is that two calls in one process disagreeing
  localizes the fault to the code between them. Re-running would have produced
  the same output and cost a round.
- **The last lesson was not applied to the sibling file.** The previous round
  learned to measure a file after an edit, then this round still let two
  modules reach 424 and 419. The lesson had been recorded and read; what it
  lacked was a step in the routine. Splitting is now the first response to a
  module approaching the cap rather than the remedy after a breach.
- **Newly written British-English prose is a spellcheck hazard, and the gate
  reads code spans too.** The verdict rule was right and the text was wrong:
  the flagged token was a British spelling of a word whose American form the
  rule demands, and it was one this round's prose introduced rather than
  inherited. Two consequences. First, the American form is the correct one here
  — the rule is a verdict, not a suggestion to silence. Second, a note
  *describing* such a fault must not quote it, not even inside backticks,
  because the checker reads code spans. Name the fault categorically and let
  the reader supply the word.
- **And it fired again in round 4, in text written while quoting that lesson.**
  Two tokens were flagged: one in a delegated agent's new prose, and one in
  this plan's own new section — in the sentence recording that the
  verifications ran before any edit was approved, written minutes after the
  lesson above was read. Knowing the rule did not prevent the fault, because
  the fault is produced by fluent British prose, not by ignorance. The step
  that would have caught it is mechanical and cheap: run `make markdownlint`
  *before* committing new prose, not after, and treat its verdict as a required
  input to the write rather than a checkpoint on it. A rule that must be
  remembered at the moment of writing is weaker than a command that can be run
  over the result.

### Round 12: a round-5 test that had never run, and asserted the wrong answer

The `#[expect]` cleared the dead-code error and CI advanced past `Lint` for the
first time — into `Test and Measure Coverage (with serde_saphyr)`, where it
failed on **both** legs on one test:

    policy_telemetry::a_failing_selected_file_reports_the_mode_outcome::case_2_optional
    left: "not_found"
    right: "optional_failure"

Identical failures on `ubuntu-latest` and `windows-latest` rule out flake and
platform asymmetry, and the sibling `case_1_required` passed. **The test's
expectation was wrong, not the implementation.** Five independent authorities
agree, and the layer the test contradicts is the loader:

- **The fixture is absent, and absence is `Ok(None)`.** The case selects
  `missing.toml`, which nothing writes. `file_exists_and_is_regular` returns
  `Ok(None)` for a path that is not there (`loader.rs:50`, `:59`), and only an
  `Err` reaches the arm that emits `optional_failure`.
- **The repo already asserted the correct mapping.** `scoped_layers.rs:96-103`
  is `optional_selected_path_does_not_report_a_missing_file`, and its sibling
  comment states outright that a missing file "takes the `Ok(None)` arm" while
  the malformed file takes the `Err` arm.
- **RFC 0002's Table 1 is explicit.** `Optional` + "rung wins, missing" is "no
  layers, stop"; `Optional` + "rung wins, malformed" is "error, stop". Two
  different rows, so two different outcomes are the faithful encoding.
- **The enum's own doc says absence is ignored.** `policy.rs:90`: "A missing
  selected path is ignored". A tolerated absence labelled a failure would put
  the event stream at odds with the contract it reports on.
- **The plan had already enumerated the arms.** Round 5's own note lists
  `not_found` (`policy.rs:163`) among the four terminals this path carries. The
  `#[case::optional]` expectation reads as a drafting slip against that list.

The honest framing is that the test was *never executed* before this run. Every
earlier CI attempt died at `Lint` — first on `E0597`, then on the dead code —
so the suite stopped before reaching it, and the local gate could not run at
all behind the package-cache lock. A green-looking `check-fmt` parses the file
without typechecking it, so no local signal could have caught a wrong expected
string. **The defect was found by the first run that got far enough to try
it**, which is the argument for treating "CI never reached this step" as
unverified rather than as passing.

Fixing only the expectation would have left the review's actual concern
uncovered. The row that prompted this suite was a policy run that was *silent*
where the legacy path reports; `optional_failure` is emitted from exactly one
arm, and that arm needs a file that exists and cannot be read. So the single
`#[rstest]` was split in two:

- `an_absent_selected_file_reports_the_mode_outcome` — `required_failure` and
  now `not_found`, the corrected expectation, with the reasoning in the doc
  comment so the next reader does not "fix" it back;
- `an_unreadable_selected_file_reports_the_mode_outcome` — a malformed fixture
  through `write_fixture_with`, giving `required_failure` and
  `optional_failure`. This is the arm no test in the tree pinned.

The pair now reads as the design intends: the two modes **agree** about a
malformed file and **disagree** about a missing one, which is the distinction
an operator needs from the event stream. The file is 196 lines, well inside the
cap, so the addition needed no splitting.

One gesture was checked and rejected rather than assumed. Widening
`ExplicitMode::Optional` to report `optional_failure` for absence would have
made the original assertion pass, and it is wrong on all five counts above; the
test was corrected to the implementation, and the implementation was not bent
to the test. The plan's `policy_telemetry.rs:107` reference was also updated —
the split moved that assertion to `:146` — because a stale line anchor in a
living plan is the same class of defect as a stale expectation in a test.

**Evidence and limits.** Four of the seven gates ran green locally before the
push — `check-fmt`, `markdownlint`, `test-workflow-contracts` (319 passed, 1
skipped) and `nixie`. The three that would typecheck or execute Rust —
`typecheck`, `lint`, `test` — could not run: the package-cache lock was still
held (holder `1832225`, 6h40m, `do_wait`, ~95 waiters) and `flock -n` confirmed
it. So **the new test's compilation and both its cases remain unverified
locally**, and the malformed fixture is the one genuinely new mechanism this
round introduces. Its unparseability is not assumed: `value = ???` is the same
content two existing tests already prove fails to parse (`scoped_layers.rs:267`,
`discovery_attributes.rs:383`), so the fixture is known-good rather than
novel. The push is `399627d8`, from `453a01d8`, with the lease bound to that
observed remote head, and CI run `36392314109` is the first execution of the
corrected expectation.

A note on what this round cost and why. Three consecutive CI rounds were
consumed by defects that only a compiler could see — `E0597`, then the dead
`write_config`, now a wrong expected string — each hidden behind the one before
it, because fail-fast stops the suite at the first failure and the local gates
were lock-blocked for all three. The pattern is worth naming: **when the
compiler-driven gates are unavailable, CI stops being a confirmation step and
becomes the primary feedback loop, at ~28 minutes per iteration.** Nothing in
this round changes that; it is an environment fact (the foreign lock) plus a
workflow fact (fail-fast), and the only cheap mitigation available was the one
taken — checking the new expectation against the code, the existing tests, the
RFC table and the enum doc before spending a round on it, rather than pushing
the obvious edit and seeing what CI said.

**One Rust gate did run, and the record misread it.** The three lock-blocked
gates were reported as wholly unverified above. That was too pessimistic about
`lint`, and the correction is worth stating because it retired the last
locally-checkable risk in the round. The new test adds two `.expect()` calls
into an `#[rstest]`-generated function, and the workspace denies `expect_used`;
`clippy.toml` sets `allow-expect-in-tests = true`, but whether that exemption
reaches a macro-expanded test body was unproven. It does, and this is
observable rather than inferred.
`ortho_config/tests/localized_parse.rs:231-250` is an `#[rstest]` whose body
calls `.expect("translated fixture args should parse")`. `make lint-clippy` runs
`cargo clippy --all-targets --all-features -- -D warnings` (Makefile:8,103),
so integration tests under `ortho_config/tests/` are linted with warnings
denied. And in run `36387995236` — the round whose *tests* failed — the
`build-test` job passed its `Lint` step and only failed later, at the coverage
step: `ci.yml:208` (`make lint`) precedes `ci.yml:222` (the test step) in the
same job, so reaching the test step *is* evidence that clippy accepted the
construct. Two further suites in the same workspace
(`selected_subcommand_merge.rs`, `clap_integration/error_cases.rs`) carry the
same shape, so this is an established pattern here, not an accident. The
remaining exposure is therefore compilation alone, not lint.

### Round 13: every pre-merge row re-verified against the current tree

The section above reconciled the pre-merge table against `75d9901d` and five
repairs ago. The table has since been re-rated — it now reports **3 errors and
6 warnings**, bound to `9e9ecc37`, and its "reviews paused" banner means
CodeRabbit is no longer re-rating automatically. Each row was therefore checked
against the *current* tree rather than trusted, and the dispositions below
replace the earlier estimates with evidence.

The three error rows all resolve to *already repaired*:

- **Unit Architecture** was exactly right when written. At `9e9ecc37` the file
  is 320 lines using `root.parent.rglob`, `tests.glob("*.rs")`, bare `is_dir` /
  `is_file` / `read_text`, and no `ScanError` at all — every named function
  returning a silent boolean. Commit `905f55f1` ("Repair round-4 findings: scan
  guard") replaced them with `os.walk(onerror=refuse)` and the `_scan` funnel
  (`trybuild_tier.py:99`, `:103-133`), and `grep` now finds no `rglob` or
  `.glob(` outside the docstring that narrates the old defect. **Repaired.**
- **Testing (Overall)** was closed for its four named items
  (`selected_error`, `reportable_errors`, `merged_file_value`, `push_into`, all
  with real assertions) plus ordered selector precedence at
  `scoped_layers.rs:187`. The plan's own evidence line cited `:121` for that —
  which is the *deduplication* test, not the precedence one — and has been
  corrected to `:187`.
- **Testing (Unit And Behavioural)** is largely closed: both bracketed and
  parenthesized list spellings, the `env_var`/`env_vars` conflict, invalid mode
  and scope values, and all three `project_root_from` failures are asserted in
  `discovery_validation.rs`. What genuinely remains is **runtime coverage of
  `env_vars`**: no test under `ortho_config/tests` references it, and no derive
  in the repo uses it, so `env_selector_tokens`'s non-empty branch
  (`policy_impl.rs:94-98`) — the alias-chain path that exists *for* the policy
  loader — is exercised only as a macro-parse unit.

The warnings separate cleanly. **Observability, User-Facing Documentation and
Developer Documentation are all already addressed**: policy telemetry emits
both a resolution event and a terminal outcome with a fixed `cli`/`environment`
class (`policy.rs:194`, `:381`, `:392`; `telemetry.rs:186-201`, `:274-296`),
and all five documentation sub-points for the users-guide and the
developers-guide section exist at the cited lines. **Testing (Property /
Proof)** is partly addressed: `scoped_stacking_proptest.rs` is a genuine
reference-model suite over generated candidate sets, scope orders and canonical
aliases, but it does not cover ordered *selector* chains or `extends` ordering,
neither of which appears in any property test in the workspace. **Testing
(Compile-Time / Ui)** stays declined on the reasoning already recorded above;
the row is accurate that no `tests/ui/` case covers the discovery vocabulary,
but the `ui/` pairs are all intact (9 `.rs` + 9 committed `.stderr`, no
orphans), so the failure mode that matters — an unpaired `compile_fail` — does
not exist here.

**Docstring Coverage** is re-confirmed still valid, and still bounded in the
way Round 7 described. The metric counts private helpers and `#[test]` bodies,
not public API: `missing_docs = "deny"` (`Cargo.toml:98`) means every genuine
public item is already documented, so the 74.65% figure is not an
API-documentation hole. Re-measuring at this head reproduces Round 7's table
exactly and adds three delta files that audit did not cover —
`discovery_attributes.rs` (8 undocumented helpers, all private), `load.rs` (4),
`candidate_set.rs` (1). The deficit is dominated by two files:
`scoped_layers.rs` (8) and `discovery_attributes.rs` (8) account for 16 of
roughly 25, and the Python side contributes none — every `def` and `class` under
`tests/workflow_contracts/` carries a docstring. Of the two worst files,
`scoped_layers.rs` is at the cap at exactly 400 lines and cannot take another
doc line without being split first; `discovery_attributes.rs` was previously
described here as also being at the cap, and that was **wrong** — a Round 15
re-count puts it at 397, so three lines of headroom existed. The correction
matters because it was the stated reason for not documenting that file's eight
private helpers, and this plan has already been caught once using an unverified
count to justify an exemption.

Two side-gaps neither row names were found while checking the above, and both
are documentation rather than behaviour:

1. `docs/design.md` documents `ConfigFilePolicy` selectors and both explicit
   modes (§4.5, `:353-358`) but does not state policy-level project-root
   rooting via `ConfigFilePolicy::project_root` / the `project_root_from`
   attribute, which the users-guide does cover (`users-guide.md:559-569`). The
   design doc is organized around the resolver boundary here, so this is a gap
   by omission rather than contradiction.
2. `docs/rfcs/0002-config-layer-resolution-policy.md:998-1001` still carries a
   #411-era note that scope partitioning and that rewire "edit the same code and
   should not be built independently". Both changes have long since landed, so
   the note now reads as a live constraint on work already complete.

Neither is load-bearing for #318, and this round does not change them: they are
recorded so the next reader does not rediscover them, and so a maintainer who
wants them closed can do so deliberately rather than by accident.

The shape worth keeping: **four of the nine rows were stale in the "already
fixed, table not yet re-rated" sense, and one was stale in the opposite sense —
an evidence anchor pointing at the wrong test.** Both directions cost the same
to check and only one of them is visible from the row's own severity.

### Round 14: the reconciliation, and a claim in this plan that was false

The Round 13 dispositions were local-only, so a reconciliation comment was
posted to the walkthrough (comment `5866262487`, 2026-09-28 08:25Z). CodeRabbit
answered it at 08:27Z (`5866290750`) and **accepted most of it**: it agrees the
silent-filesystem-error defect in `trybuild_tier.py` is closed, that the named
parser-validation gaps are covered, and that the policy tests and documentation
address much of the table. It holds six items open, and its reading matches
Round 13's on every one where the two overlap — including the two the plan
declined rather than closed:

- Runtime validation of `env_vars` (no test asserts generated-loader behaviour).
- Property coverage of ordered selector chains and `extends` ordering.
- Compile-time UI coverage for the policy attributes.
- Docstring coverage below 80%, with `scoped_layers.rs` already at the cap.
- `trybuild_tier.py` at 432 lines.
- `docs/design.md` root-policy prose, and the RFC 0002 #411 note.

**How that list resolved.** Written as the Round 14 record, it was the set of
items this plan declined rather than closed. Four of the six were subsequently
closed by construction rather than by argument — runtime validation at
`e83be802`, the two property gaps across `e83be802` and `e136fcc7`, the
`design.md` prose and the #411 note at `e83be802`, and the file size in Round
16 — so a reader who stops at this list will understate the head. It is left as
written because it is the accurate record of the disposition *at that point*,
which is what the round it belongs to was reconciling.

It also stated plainly that the Linux and Windows `build-test` checks were
**still running** at the time it queried, so its reply is not evidence about
them. Two of its own claims need care: "the nine existing UI fixture pairs are
intact" agrees with Round 13's check, but its `Domain Architecture`-style
reasoning about what the fixtures cover is the same reading this plan already
recorded and declined.

**A claim in this plan was false, and Round 14 found it.** Round 5 asserted
that the `<!-- tested-example: guide-scoped-discovery -->` fence in
`docs/users-guide.md` is one that
`ortho_config/tests/documentation_examples_tests.rs:42` "actually compiles and
runs". That line is only the **id registry** (`EXPECTED_EXAMPLE_IDS`); it
proves the fence exists and has a unique identifier. The fence is absent from
`STANDARD_RUST_EXAMPLES`
(`ortho_config/tests/documentation_examples_rust_tests.rs:14-26`), which is the
only list fed to `workspace.add_binary` (`:31-32`), so it is never compiled or
run. The Round 13 text at `:1619-1623` — "no test under `ortho_config/tests`
references it" — was the accurate one, and the two sections contradicted each
other within this same document.

That matters more than a single example: the fence is the *only* place in the
repository that writes `env_vars = [...]` in a derive, so the exact code path
the runtime-coverage gap describes is sitting in published documentation marked
as tested when it is not. Wiring it in looks cheap — one added identifier to
`STANDARD_RUST_EXAMPLES` plus one `assert_run`, the pattern every sibling
`guide-*` example already follows — but it needs `make test` to verify, and the
local Cargo gates have been unable to run since 2026-09-28 01:00Z. It is
recorded here as the concrete way to close the `env_vars` gap, not claimed as
done.

### CI run `36399549939` on `bd745cfd`: all five jobs green on both legs

This is the current head's certificate. The push that created it also
**cancelled** the previous run on `2cb3470a`, whose Windows leg was mid-flight
— so that head has no Windows result, and the green `2cb3470a` evidence cited
elsewhere in this plan is superseded rather than reused.

| Job                                   | Conclusion |
| ------------------------------------- | ---------- |
| `build-test (ubuntu-latest)`          | success    |
| `build-test (windows-latest)`         | success    |
| Packaging dry run (all three targets) | success    |

The four nextest invocations all passed with **no fail-fast truncation**:
1381/1381 and 1356/1356 on Linux, 1360/1360 and 1336/1336 on Windows, each with
15/15/10/10 skipped. Coverage 86.55% and 86.54% on Windows, against an 86.18%
baseline. The summaries are the clean `N tests run` form, not the
`896/1358 tests run` truncation signature Round 12 left behind.

`Lint` ran on ubuntu and `Lint (Clippy only)` on Windows; both succeeded.
`make lint` has two prerequisites, so the ubuntu `Lint` step covers
`cargo doc --workspace --no-deps`, the full clippy run, *and* Whitaker — the
step's log shows `whitaker --all -- --all-targets --all-features` at line 400
of its region and then exits success, with no dylint finding emitted before it.
(The step's tail is the compiler's `Finished` line, so the verdict is read from
the step's conclusion rather than from a Whitaker summary line — there is no
such line to quote.)

The round-12 repair is confirmed against this head rather than the older one:
the obsolete `a_failing_selected_file_reports_the_mode_outcome` appears
**zero** times in the logs, and its replacement's two cases each PASS on all
four leg/config combinations — eight PASS records, two cases across two legs
and two `serde_saphyr` configurations.

Read the certificate with its scope: it covers `bd745cfd`, whose diff against
the previously reviewed code is **documentation only**. It therefore
re-validates the code and says nothing new about it.

### Round 15: two gaps closed with tests, two of this plan's claims corrected

The six items CodeRabbit held open were re-verified against the current tree
rather than against their comment anchors. Two of them had a concrete address
and are now closed; three were already adequately covered or correctly
declined; and three claims in this plan did not survive the audit.

**`env_vars` runtime coverage — CLOSED.** This was CodeRabbit's first numbered
instruction and the one item the plan explicitly did not claim closed. The
cause was found in Round 14 and was a defect in this plan rather than in the
code: `guide-scoped-discovery` was registered in `EXPECTED_EXAMPLE_IDS`
(`documentation_examples_tests.rs:42`) but absent from
`STANDARD_RUST_EXAMPLES`, so the only derive in the tree writing `env_vars` was
never compiled. It is now in both, and `assert_env_alias_chain`
(`documentation_examples/env_alias_chain.rs`) asserts the three behaviours the
attribute promises: the first declared variable wins when both are set, a later
one is reached when the earlier is unset or blank, and a populated alias
suppresses automatic discovery rather than merging with it. The third is the
one that distinguishes an explicit selection from the stacked scopes beneath
it, so the fixture stages a file the automatic scopes *would* find and gives it
a third value (`3333` against `1111`/`2222`): a fall-through is then
unmistakable rather than coincidentally matching.

The blank case is deliberate, not incidental. `ConfigPathSelector::resolve`
filters `!value.is_empty()`, and the empty string resolves to an empty
`PathBuf`, which is a *relative* path rather than an absent one — the exact
hazard the candidate generators guard against elsewhere. The middle case
therefore leaves `ACME_CONFIG_PATH` set to the empty string and asserts the
later alias wins. Selector values are absolute paths under the workspace root,
via a new `ExampleWorkspace::path_in_root`; a bare filename would have happened
to work, because the child's working directory is its run directory, and
relying on that coincidence is precisely what the rest of discovery refuses to
do.

**Ordered selector chains — CLOSED.** `scoped_stacking_proptest.rs` (398 lines)
is a genuine reference-model suite, but it never imports `ConfigPathSelector`,
and its one three-rung example chain (`scoped_layers.rs:176-178`) only ever
populates the *first two* rungs. `ConfigPathSelector::resolve` decides
precedence with a `find_map` over a filtered iterator, so a reordering there
would have been invisible to every existing test. `selector_chain_proptest.rs`
adds two properties over generated three-rung chains — a length at which a wrong
`rev()`, a wrong sort, or an off-by-one bound changes the answer in more than
one way — and pins both the winning value and the layer count, because a
value-only assertion cannot distinguish "first rung wins" from "every readable
rung contributes".

**`extends` ordering — CLOSED in the round after this one.** This audit found
no property test generating a parent/child/grandparent `extends` graph and
asserting parent-first layer order, and recorded it as a bounded gap. That
reading was correct when written and was closed at `e136fcc7`:
`extends_chain_proptest.rs` generates chains of two to six files whose values
encode their own position, so a reordering shows up as a *value* mismatch
rather than only a path mismatch. Three example tests pin the order too
(`compose_layers.rs:61-87`, a two-file chain; `extends.rs:118-161`, three files;
`scoped_stacking.rs:160-209`), and the new suite was verified to have teeth —
reversing the assembled chain in `load_chain_for_file` fails it with "layer 0
should come from file-0.toml, got …/file-1.toml", and the loader was restored
byte-identical afterwards.

**Compile-time UI coverage — declined, unchanged.** Re-verified: all nine
`ortho_config/tests/ui/` pairs are intact, 9 `.rs` against 9 `.stderr`. The one
apparent orphan elsewhere (`cargo-orthohelp/tests/ui/policy_public_api.rs` with
no `.stderr`) is not one — `compile_time.rs:11` declares it `t.pass(...)`. The
failure mode the row names does not exist.

**Corrections to this plan.** Three claims did not survive re-verification, and
all three were load-bearing for an exemption this plan had granted itself:

1. The Round 5 claim that `documentation_examples_tests.rs:42` "actually
   compiles and runs" the `guide-scoped-discovery` fence was **false**. That
   line is only the id registry. Corrected in place above, and the sentence is
   now true rather than deleted, because Round 15 is what makes it true.
2. `discovery_attributes.rs` was described as being at the 400-line cap; a
   re-count puts it at **397**. Corrected in place. The stated reason for not
   documenting its eight private helpers therefore did not hold, and the
   exemption now rests on the narrower ground that those helpers are private.
3. `docs/design.md` was said not to document policy-level project-root rooting.
   That was true at the head the audit read, and the scribe's edit for this
   round is what closes it — so the finding was valid when made. Noting the
   ordering here because a later reader comparing the finding against the
   current file would otherwise conclude the finding was wrong.

### Round 16: the file-size item, and three stale claims it dislodged

CodeRabbit's last open item with a concrete address was the file size:
`tests/workflow_contracts/trybuild_tier.py` at 432 lines against
`AGENTS.md:33`'s 400-line cap. It is now **319 lines**, and the split it needed
is `source_scan.py` at 150.

**It is this branch's own file.**
`git diff --stat origin/main…HEAD -- tests/workflow_contracts/trybuild_tier.py`
shows 432 insertions and the path absent from `origin/main`, so the cap breach
is not inherited debt.

**The split is along a module boundary that already existed.** The file held
two concerns: reading Rust sources and manifests safely, and computing which
test binaries carry a trybuild call. The first moves wholesale — the three
`[[test]]` target regexes, `ScanError`, `sources_under`, `scan`, `listing` and
`declared_test_targets` — and the second keeps everything that names a binary.
`listing`'s inner function is renamed `enumerate_once` so it neither shadows
its name nor collides with the module-level `scan`.

**Two mechanical constraints shaped where the cut goes, and both were
measured.** The pytest invocation runs `--doctest-modules` against the
directory with no `conftest.py` and no module enumeration, so *every* doctest
is collected and renaming one changes the count. All six functions carrying
doctests therefore stay in `trybuild_tier.py`, and only the un-doctested
primitives move. The `__all__` re-exports `ScanError` because
`trybuild_tier_test.py` imports it from here and reads it at two
`pytest.raises` sites; the split is a move, not a rename of the public surface.
Re-exported rather than repointed so that contract keeps one import site. Count
after: **319 passed, 1 skipped** — the baseline exactly.

**Ruff findings were declined, with reason.** `ruff check` and
`ruff format --check` both report on the split file, but ruff is not a
repository gate: it appears nowhere in the Makefile, `AGENTS.md`, `docs/`, or
`.github/`, and there is no configuration file in this directory or any parent
of it. The one ruff complaint that is *not* cosmetic — an import-merge report —
is equally present in the file at `HEAD`, so it is pre-existing style rather
than a regression this split introduced. Fixing it would put this branch's
formatting at odds with every sibling module in the directory.

**Two stale claims were dislodged by the audit, and both are corrected in place
rather than deleted.** This is the same class of error Round 15 caught three
times, so the corrections carry their evidence:

1. The `Unit Architecture` row's evidence anchor pointed at
   `trybuild_tier.py:64` and `:99` for `ScanError` and the `onerror` walk. The
   split moves both. Now `source_scan.py:30` and `:65`.
2. Round 15's `extends`-ordering disposition says **"NOT closed, and
   honestly so"** — and `e136fcc7`, committed *after* that text was written, is
   the commit that closes it. The paragraph now says so, keeps the original
   finding as the record of what was true when written, and names the mutation
   that proves the new suite has teeth.

One further note, recorded because it will otherwise be rediscovered: the Round
14 declined list reads as a live task list and is not one. Four of its six
entries were closed by construction in the two rounds that followed, so the
list now carries a paragraph saying which and at which commits.

**What is not claimed.** The suite is a Python move validated by pytest and by
line count. It was not gated on a Rust build, because the change touches no
Rust file, and the last full gate pass predates it. The next full pass on this
head is the certificate for it.

### Round 17: a false mode spelling in RFC 0002, and its delivery plan

This round answers the `Developer Documentation` pre-merge row — **the same row
Round 13 recorded as already addressed**, which the review still holds open
against RFC 0002.

**The falsehood.** `docs/rfcs/0002-config-layer-resolution-policy.md` named
`fallthrough` as the default `explicit_mode`, at two sites: the attribute
bullet in the "Proposed design" section and the stability-surface grammar list.
No such mode exists. Verified three ways at this head:

1. The macro default is `required_exclusive`, not `fallthrough` —
   `ortho_config_macros/src/derive/policy_impl.rs:116-119` reads
   `mode.unwrap_or(if explicit { "required_exclusive" } else { "first_wins" })`.
2. `fallthrough` is not an accepted value at all. `policy_impl.rs:122-128`
   matches exactly `required_exclusive` and `optional` for explicit mode, and
   the reject arm at `:128` is literally
   `"explicit_mode must be required_exclusive or optional"`.
3. The runtime agrees: `ortho_config/src/discovery/policy.rs:88-89` carries
   `#[default]` on `RequiredExclusive`.

So the sentence's premise had expired. It argued the macro default needed a
name distinct from the runtime `ExplicitMode::Optional` to avoid contradicting
it; the macro default and the runtime default are in fact the same variant.

**What changed.** The attribute bullet now states `required_exclusive` as the
default and keeps `first_wins` for automatic mode, and describes `optional` as
the distinct mode it is — `RequiredExclusive` requires a selected path and
suppresses automatic discovery, while `Optional` ignores a missing selected
path while still suppressing automatic discovery, per `policy.rs:83-92`. The
rename rationale is removed rather than replaced, since it no longer applies.
The stability-surface list now reads `optional` where it read `fallthrough`, so
the grammar matches what the derive accepts.

**The delivery plan is annotated, not rewritten.** Each of the five steps now
carries its disposition in place, in the RFC's annotate-don't-remove style.
Steps 1, 2, 3 and 5 are delivered with #318. Step 4 is delivered *except*
`policy_hook`, which is left named as the one outstanding deliverable:
`grep -rn "policy_hook" --include=*.rs .` returns no hits, so it appears only
in the RFC.

One caveat, recorded rather than smoothed over. Step 5's acceptance lists a
legacy alias "emitting the default deprecation signal", and the RFC promises at
its `legacy_alias` entry that a legacy rung winning is never silent. What the
tree has is the *metadata*: `ResolvedSelection::legacy()` (`policy.rs:118-120`)
carries the flag and tests assert it (`scoped_layers.rs:206`, `:220`). No
non-test caller reads `legacy()`, and no `warn` or deprecation event exists on
this path, so the emission itself is not visible to this read. The step is
annotated as delivered on the strength of the test matrix; the signal's
emission is **not** claimed.

**No gate has been run since this edit.** It is documentation only, touching no
`.rs` file, so the next full pass on this head is its certificate.

### Round 18: the walkthrough re-bound, and the three rows it then produced

**The walkthrough moved its binding, so the rows below are fresh rather than
stale.** Round 13 recorded `change_assessment_commit = 2cb3470a` and seven
commits of drift. It now reads `69be5cbeed71884a693f72922eb4155b82b1161f` at
`:85`, written `2026-09-28T12:31:19Z` — so the queued review `0f7aa908` did
run, and its three open rows (1 error, 2 warnings) describe a head only two
commits back. That matters for this head: `git diff --name-only 69be5cbe..HEAD`
lists five paths and **no `.rs` file**, and the three files the rows cite
(`discovery/policy.rs`, `discovery/scoped.rs`, `derive/policy_impl.rs`) hash
identically at both commits. Rust is byte-identical to what was reviewed, so
each row describes the current code and none can be dismissed as a stale anchor.

**`Unit Architecture` — refuted, and the plan had already half-said so.** The
row claims `ConfigFilePolicy::resolve_layers(&self)` is "a read operation"
whose telemetry "the PR introduces". Verified against `origin/main`
(`0c498068`) this round: `resolve_layers` and `ConfigFilePolicy` appear in
**zero** `.rs` files there, so the policy API is indeed new; but
`compose_layers(&self)` at `load.rs:235` already emits `telemetry::attempt` and
`telemetry::load_outcome` through the same global counters, and
`candidate_set.rs:101-106` already documents the boundary — "Assembly records
its decisions instead of emitting them so that `candidates` stays a silent
query; discovery operations call `CandidateDecisions::emit` at their own
boundary". The PR adds one more caller of an established pattern; it does not
introduce telemetry from a resolution path. The row also calls the function a
read, and it is not: it performs filesystem I/O on both branches
(`load_config_file_as_chain`, `loader.rs:181`; `chain_layers`, `load.rs:262`),
which is the irreversible side effect the complaint is about, and the crate
reads *resolution* as its operation boundary. One clause is worth keeping as an
observation rather than a defect: the `outcome` value is computed at
`policy.rs:149-186` and emitted, but is not among the returned fields, so a
caller cannot publish it itself. Nothing in `AGENTS.md:295-317` requires the
change — it instructs the opposite — so this is a design suggestion, not a
repair task.

**`Testing (Compile-Time / Ui)` — declined again, on the same reason, now with
the code read rather than asserted.** The row asks for trybuild fixtures in the
discovery-attribute vocabulary. Confirmed this round: nine fixtures under
`ortho_config/tests/ui/`, driven by five `*_trybuild.rs` harnesses, and
`grep -rln 'scope_order\|explicit_mode\|automatic_mode\|project_root_from\|env_vars'`
over that directory returns **no** file. So the gap the row names is real. It
stays declined because the errors are raised by pure token-generation
functions, and a UI fixture would compile the identical message through a
slower, more fragile instrument while pinning strictly less: it pins one
message, where the unit test pins the message *and* the variant it maps to.

**A counting error in this plan, corrected while citing it.** Round 13 wrote
that `mode_tokens` and `scope_order_tokens` are covered by "six cases". The
module at `policy_impl.rs:227-345` holds **six test functions** — five
`#[test]` and one `#[rstest]` — and the `#[rstest]` carries six `#[case]` rows,
so eleven cases execute. The distinction is not pedantry: the row asks for
fixtures covering "valid policy discovery", and the honest count of what
already covers it is what decides whether a fixture would add anything. The
substantive claim survives — both defaults, all four legal spellings, both
unknown-value errors and all three scope behaviours are each asserted — but the
number now matches the file.

**`Observability` — valid, unrepaired, and larger than the row states.** The
row is accurate on both counts it makes, verified at this head: the failure
record at `scoped.rs:151-158` passes `candidate.source` only, while `scope` is
in hand in `compose_scope`'s signature, and `CandidateFailure`
(`load.rs:37-41`) has no field to carry it; and the terminal event at
`scoped.rs:111-119` passes `None` for the winner. Round 5's disposition claimed
this row "Addressed" on the strength of `load_outcome` ending in
`count_outcome` — which is true of the *mechanism* and does not touch the
scoped context the row asks for. **No repair is attempted in this round.**

**What is not claimed.** No gate has been run since this record. The
disposition of all three rows is to be posted as a focused walkthrough
reconciliation rather than a full re-request, and the follow-up work the
`Observability` row implies — a bounded `scope` label on the scoped failure and
terminal events — is named here as outstanding rather than silently dropped.

### Round 19: the `Observability` row is repaired, not deferred

**Round 18 named the work outstanding; this round does it.** The row asked for
a bounded scope on the scoped path's failure and terminal events. Both halves
are now emitted, and the scope vocabulary is a closed three-value set (`system`,
`user`, `project`) declared in `telemetry.rs`, so the module's
no-values-in-events property — every field a `&'static str` from a closed set,
or a `bool` — still holds. No path, variable value, or file content reaches an
event.

**The fix was smaller than the row implied, because the data was already
there.** `Candidate.scopes: Vec<DiscoveryScope>` has existed on the candidate
entry (`candidate_set.rs:80`) since the stacking work, and `compose_scope`
already had the `DiscoveryScope` in hand as a parameter. What was missing was
only the emission path: `CandidateFailure` carried no field to put a scope in,
and the terminal event passed `None`. So this is a label plumbing change, not a
change to which events fire or when.

**Why the same `source` label needed it.** `xdg` is reachable from both the
machine-wide `XDG_CONFIG_DIRS` walk and the per-user `XDG_CONFIG_HOME` walk, so
before this change the two failure events were byte-identical and an operator
could not tell which walk produced one. That is a defect the stacking mode
introduced, which is why the row is in scope for this PR rather than a
follow-up.

**One half of the row is argued rather than implemented: `scope` as a *metric*
label.** The row also asks for the scope on metric labels. `telemetry.rs`
documents the opposite decision: "the label set is part of the contract a
consumer's dashboards are built against", and `count_outcome` keeps its
`operation`/`outcome` pair deliberately. Adding a third label multiplies every
outcome series by scope to record a fact the event already carries, and the
event is where a per-walk fact belongs. The same reasoning already excludes
`source` from that label set. So this half is declined on a recorded contract,
not on effort.

**The counters the row also names are a metrics programme, not a repair.** It
asks for counters and histograms over candidates, layers, and latency. Nothing
in this PR's own change needs them, no existing issue covers them (#484, #476
and #413 are each adjacent and none is this), and inventing a label-and-metric
surface here would freeze a contract before its consumer exists. This is named
as a separate proposal rather than done, and the row's disposition says so.

**Where the new test lives, and why.** Five cases pin the field in
`ortho_config/tests/scoped_telemetry.rs`. It is a top-level test file rather
than a module of `scoped_stacking` for the reason `discovery_telemetry` already
records: the capture harness is shared, and a second suite including it would
compile a second copy. It declines dead-code warnings on that module
explicitly, in the same form `merge_telemetry.rs:15-19` uses.

**The suite was proved to have teeth rather than assumed to.** `scope_label`
was mutated to return a constant `system` and the suite re-run: four of the
five cases failed and one passed. The survivor is
`a_system_scope_failure_reports_the_system_scope` — precisely the case a
constant label satisfies, which is why the other four exist. The mutation was
reverted and the suite re-run green. Without this check the suite would have
been a set of assertions that could not distinguish the field from a literal.

**Fixture staging goes through capability handles.** Each case needs a
*directory* where a configuration file is expected, since the loader refuses
one with "configuration path is not a regular file" and that is what produces a
failure inside the walk. `std::fs::create_dir_all` would trip the repository's
`no_std_fs_operations` lint (`scoped_stacking` is not in `dylint.toml`'s
`excluded_crates`), so the directory is created through a `cap_std::fs::Dir`
opened on the caller's temporary directory.

**What is not claimed.** No gate has yet been run against this round's tree.

### Round 20: the row's fourth clause, which Round 19 never disposed

**Round 19 answered three of the `Observability` row's four asks and left one
untouched.** The row's resolution names four things: the `scope` on the scoped
events, the same context on metric labels, counters/histograms for volume and
latency, and — separately, and easily lost between the other three — "Emit a
consistent policy attempt/outcome metric for explicit and automatic policy
paths". The first is done, the second and third are argued or deferred, and the
fourth had no disposition at all until now. This round is that clause.

**It is a real defect, and it was verified twice.** A wyvern reconnaissance
confirmed the claim against the current tree, and the same reading was then
repeated by hand. `ConfigFilePolicy::resolve_layers` has two branches. The
automatic branch delegates to `compose_scoped_layers_with_origins`, which
reaches `telemetry::attempt` by one of two routes — `scoped.rs:90` for the
stacking mode, or `load.rs:126` through the `FirstWins` early return. The
explicit branch reached none: it called `policy_resolution` (which counts
nothing) and then `FileLayerOutcome::selected`, whose `load_outcome` ends in
`count_outcome`. So a policy that resolved an explicit path incremented
`ortho_config.discovery.outcomes` with `operation = policy_resolve` while
incrementing `ortho_config.discovery.attempts` not at all. The outcome counter
could climb while the attempt counter stayed flat.

**The fix is one line, and the line budget decided its shape.** The explicit
branch now calls `telemetry::attempt(telemetry::OPERATION_POLICY_RESOLVE)`
before `policy_resolution`. `policy.rs` sits at the 400-line cap exactly, so
the accompanying doc change had to pay for the new line rather than add one:
the paragraph that read "The two `discovery.policy` resolutions below are
therefore the whole of this path's visibility" was both now false *and* the
obvious place to recover the line, since an attempt, a resolution and an
outcome are now this path's visibility. The file is back at exactly 400.

**Two tests pin it, in the two channels the row named.** A `tracing`-channel
case in `support/policy_telemetry.rs` asserts the attempt's operation is
`policy_resolve` and that the terminal outcome names the same operation. A
counter-channel case in `discovery_metrics.rs` asserts the `attempts` and
`outcomes` counters for `policy_resolve` are both exactly 1 — that suite
previously drove no policy at all, so the metric half of this path was
unpinned. Each was proved non-vacuous by deleting the new line: the first fails
with "expected exactly one `discovery.attempt` event, got []", the second at its
`attempts` assertion. Restoring the line turns both green.

**What the automatic branch does, and why it is not a second defect.** The
automatic branch reports under the legacy `compose_layers` operation label
rather than a third one of its own, because it delegates the whole walk to the
legacy path; the row asks for *consistency* between the branches, not for one
label. In `StackScopes` a single call still yields one attempt against one
outcome per scope that found a winner plus a terminal one, which is the
recorded multiplicity that mode is built on. Neither is changed here.

**What is not claimed.** No repository gate has been run against this round's
tree either. The focused suites are green — `discovery_telemetry` 39/39,
`discovery_metrics` 3/3, and the scoped and proptest suites unchanged and
passing — but the commit gateways are still owed.

### Round 21: the documentation edits, and the formatter's opinion of them

The `scope` field reached the source and the tests in Round 19; it reached the
documentation only here. That was a real gap rather than a cosmetic one: both
event tables are the published vocabulary, and a field an operator cannot look
up is a field they will not read out of a log.

**Five edits, in two files.** `docs/design.md` gains `policy_resolve` on the
`discovery.attempt` bullet and a qualified `scope` on both the
`discovery.candidate` and `discovery.load` bullets. `docs/developers-guide.md`
gains `policy_resolve` in the `discovery.attempt` row's `operation` cell,
widens that row's "emitted from" cell to `load`, `policy` (the explicit branch
emits the attempt from `policy.rs:380`), and adds a qualified `scope` cell to
the `discovery.candidate` row.

**The qualification is "scoped walks only", and it is load-bearing.** The field
is structurally optional (`scope: Option<&'static str>`) and the module's
convention is that an absent field is omitted rather than rendered empty, so
the tables must say *when* it appears, not merely that it can. Verified against
the call sites rather than assumed: `load_outcome` has five, and `scope` is
`Some` at exactly one — `scoped.rs:169`, the per-scope terminal event of a
successful scoped walk. `scoped.rs:115` (the whole composition's terminal
event), `load.rs:134` and `load.rs:154` (the flat walk), and `policy.rs:194` (a
policy) all pass `None`. The `discovery.candidate` field is `Some` only from
`scoped.rs:161`. So the `discovery.load` `source` field is *not* scoped-only:
the flat walk sets it at `load.rs:137` and a policy omits it. An earlier draft
of the guide's `candidate` row over-claimed by leaving the scope cell
unqualified, which is why that cell now reads `scope` (scoped walks only) in
the same form as its `load` neighbour.

**The table is column-padded, which decides the edit order.** The guide's
telemetry table is padded to a uniform cell width, so lengthening one cell
takes every row in the table out of alignment with it — and `make check-fmt`
runs `mdtablefix` over it (`--wrap --renumber --breaks --ellipsis --fences`),
which would immediately fail a hand-padded attempt. Content was therefore
edited first and `mdtablefix --in-place` run after; the follow-up `--check`
reports 74 files unchanged, so the table is at a fixed point of the
repository's own formatter rather than at one hand-matched to it.

**The scribe stalled and was stopped.** The documentation was delegated, but
the agent applied only two of the five edits and then went silent — its
transcript output file's mtime stood 49 minutes behind the wall clock with the
last entry being the delivery of a correction message rather than a reply. It
was stopped before it could wake and edit the same lines; the remaining three
edits were applied directly, which is also why the count above is stated as
five rather than as the two that were observed to land.

### Round 22: the gate run, and the one word it rejected

The tree was committed and handed to `scrutineer` for all seven gateways. Six
passed on the first candidate and one failed, and the failure lay in the
documentation rather than in the code.

**Six green, one red.** `check-fmt` (2s), `typecheck` (20s), `lint` (29s),
`test` (282s: 1392 passed, 0 failed, 15 ignored, plus 87 pytest passed, 5
skipped), `test-workflow-contracts` (320 passed, 1 skipped), and `nixie` (all
diagrams validated). All three halves of `lint` ran — `cargo doc`,
`cargo clippy -D warnings`, and whitaker — so the sibling-prerequisite hazard
of clippy aborting `make` before whitaker runs did not bite. `policy.rs` was
confirmed at exactly 400 lines. No gate log contained a warning line; the only
textual matches for "warning" were the `-D warnings` flag in the echoed
commands and one test name. The candidate was re-read after every gate and
never moved.

**The failure: `make markdownlint`, in its `spellcheck` half.** The
`markdownlint-cli2` half was clean (75 files, 0 errors). The `spellcheck` half
rejected one word written into this very document: a verb whose suffix was the
non-Oxford variant, used in the preceding version of this section to describe
what lengthening one table cell does to every other row's alignment. The house
style is en-GB-oxendict, which takes the `-ize` suffix, and the rejected
variant is not in the generated `typos.toml` allowlist.

The fault is named categorically here rather than quoted, and the reason is
worth recording as a rule for anyone editing this document: **a description of
a misspelling re-trips the same gate.** The gate reads inline code spans too,
so writing the offending form into the prose that explains the failure fails
`spellcheck` a second time. The repair therefore avoided the word entirely,
rewriting the sentence as "takes every row in the table out of alignment with
it"; the same rule is why the offending form appears nowhere in this document,
and an earlier draft of this very paragraph was itself rejected for violating
it.

The two `-ised` forms that *do* survive, at lines 1044 and 1048, are quoted
deliberately as the thing the house style forbids, and they pass — the gate
reaches a later line only after passing them, so they are demonstrably
acceptable. Do not "correct" them.

**Fail-fast, so one error proves one error and no more.** `spellcheck` stops at
the first offender, so the failing log establishes exactly one bad line and
cannot establish that no others exist. The correction was therefore re-gated
rather than assumed sufficient: the candidate was amended (the commit had never
been pushed, so no red commit is left in history) and `make markdownlint`
re-run against the new tree to answer the question the first run could not.

**Two runs raced an edit, and both results were void.** The first re-gate was
started against a candidate and this document was then edited while it ran, so
its `make markdownlint` pass and its `make check-fmt` failure were both
unbound: the pass described a tree that no longer existed, and the failure —
`mdtablefix --check` reporting the execplan needed reformatting,
`1 file would be reformatted, 73 files left unchanged` — was merely the
difference between this document as that gate read it and as it had since
become. The second re-gate was stopped for the same reason, the section being
written here having again been edited underneath it.

The lesson is the one the freshness rule already states, sharpened by the
specific way it bit twice: **freeze the tree before summoning a gate-runner,
and edit nothing while a gate holds the candidate.** What makes this document
unusual is that its subject *is* the gate run. **The record of a run is itself
prose that must be reformatted and re-gated**, so it cannot be written
concurrently with the run it describes — the narrative and the certificate
compete for the same bytes. The resolution is structural rather than a matter
of care: the prose is finished, formatted and frozen **first**, and the gates
are run against that frozen commit as the last action. Nothing is written back
afterwards.

**Where the certificate lives.** Because of that ordering, this document does
not and cannot record its own final gate results: writing them here would edit
the very bytes the gates had just approved, invalidating the run. The result of
the last gate pass is therefore held in its own artefacts — the `tee` logs under
`/tmp`, named in the hand-off — and in the pull request's check rollup, not in
this prose. Treat a claim about gate results found in this document as a claim
about the *earlier* candidate it names, never about the current tree.

**Re-gating was narrowed deliberately.** The only delta between the first gated
candidate and the frozen one is prose in this file, verified with
`git diff --name-only <first>..<final>` rather than assumed, so the Rust and
workflow gates were not re-run against it: a delta is re-gated only for the
file classes it reaches, and their six green results stand for the unchanged
tree.

**What is not claimed here.** This section names the six gates that passed, the
one that failed, and the exact cause of the failure, because those are facts
about candidates now superseded. It does not assert the outcome of the final
gate pass, for the reason given above: that pass is the last action taken
against the frozen commit, and its evidence is the gate logs and the PR check
rollup rather than any sentence here. A reader wanting the current verdict
should read those, not this.

### Round 23: three findings repaired, two rows argued, and the generator run twice

A review bound to `ce16468d` raised three inline findings. All three were
verified against the tree before any repair, and all three were valid — no
skips this round. One turned out to be the most consequential finding of the
whole PR, and it is worth stating why: it was reported as a Minor on a file
already under repair, and it was silently discarding binaries from the trybuild
coverage class on the only interpreter that ever ran the gate.

**The Python 3.14 asymmetry, falsified by probe rather than by changelog.**
`scan` was handed bound `Path.is_file`/`Path.is_dir` predicates. Since 3.14
those answer `False` for *any* `OSError`, not only for a path that is missing.
A probe confirmed the asymmetry directly: with a `chmod 000` parent, 3.12 raises
`PermissionError` from `child.is_file()` and 3.14 returns `False`. The gate
runs `uv`'s 3.14, so a `[[test]]` target behind an unsearchable directory read
as absent and its binary left the class in silence — the exact false negative
that `source_scan` exists to refuse, written into the module whose docstring
claimed to prevent it. The module's own stated contract was false on the
interpreter the gate uses, and had been since it was written.

**The remedy differed from the suggestion in two ways, and both mattered.**
Placement: the suggested helpers belonged in `trybuild_tier.py`, but the
predicates are the guard *for* `scan`, which lives in `source_scan.py`, so the
explanation belongs beside the contract it protects — otherwise the next caller
is free to reintroduce the bug. Shape: a module-level `_is_file(path)` cannot
be passed to `scan` at all, because `read(*args)` runs before the `try` block
and a bare function name fails with a missing-argument `TypeError`; the first
attempt did exactly that and all twelve callers failed. `bool` also has no
channel to distinguish "returned `False`" from "raised", so `exists(path)` is a
**factory** returning a zero-argument callable, mirroring the existing
`listing(directory)` pattern.

**Non-vacuity was proven, not assumed.** The new regression test was checked by
reverting the four predicates to bare `is_file`/`is_dir`: it then failed with
`DID NOT RAISE ScanError`. The files were backed up with `cp` before the
mutation probe and restored by `diff` against that copy — never by
`git checkout --`, which reverts to the index and would have destroyed the
unstaged repair.

**The typing finding was taken on its merits, not because it was gated.** The
finding quoted Ruff ANN401 warnings as support. There is no `ruff.toml`, no
`[tool.ruff]` in `pyproject.toml`, and no ruff target in the `Makefile`, so
ANN401 cannot fail CI — confirmed rather than asserted, and CodeRabbit now
records the same as a learning. The change was still made: `scan` erased its
read's type to `Any`, and its four reads return three different types, so the
contract was discarded at exactly the boundary the module polices. A
`ParamSpec` /`TypeVar` pair now preserves both ends. `basedpyright` under the
Makefile's pinned deviation introduced no new diagnostics.

**A reply was corrected within minutes of being posted.** The first version of
the doc-style reply asserted that `make check-fmt` and `make markdownlint` were
both green. `make fmt` does run markdownlint-cli2, but not the fail-fast
`spellcheck` half, which was still running under the gate pass. The reply was
patched to claim only the three things actually verified and to say the fourth
was pending. Re-reading an outbound claim before it is relied upon is cheap;
the alternative is a public record that overstates its evidence.

**Both open pre-merge rows were argued rather than re-filed.**
`Unit Architecture` was refuted on its premise by reading the *base* tree
directly: at `0c498068` the `compose_layers(&self)` method already called
`walk_candidates(…, Self::chain_layers)`, which reaches `read_file_to_string` at
`file/loader.rs:192`, and neither `policy.rs` nor `scoped.rs` existed there.
The row's evidence is fact; its conclusion — that this PR introduces a
query-like API with hidden side effects — does not follow. The requested
"narrow injectable file-loading dependency" has no seam to attach to: all 18
traits declared in the crate were enumerated and none is a file loader, and the
nearest candidate is an internal dispatch helper rather than a caller-supplied
dependency. The purity half is a genuine design question and is recorded as
one, with the alternative on the record if the maintainer prefers it.

`Testing (Compile-Time / Ui)` was **half right, and the half that was wrong
mattered.** The row claimed the PR "adds no trybuild or equivalent downstream
compilation tests". It does: `discovery_attributes.rs:38-68` derives
`OrthoConfig` with valid `automatic_mode`/`scope_order` and `explicit_mode`
blocks, and `policy_sources.rs:47,88` add more. What is genuinely missing is a
`.stderr` snapshot pinning the rendered form of the derive-time discovery
diagnostics — the five rejection unit tests call the validators *directly* and
assert message text with `==`, so none exercises the `to_compile_error()` hop.
That is a snapshot of text already asserted, not an untested mechanism, and it
is declined for this commit with the reasoning recorded rather than the ask
dismissed. If it is wanted later, `compile_fail.rs:11` globs `tests/ui/*.rs`,
so new fixtures are self-registering — but a fixture without a committed
`.stderr` does not skip, it fails and writes a `wip/` directory, so the
snapshot must be emitted by the compiler itself.

**The generator was run twice before it was committed.** `make markdownlint`
regenerates the tracked `typos.toml` from the shared dictionary, leaving the
tree dirty and the trunk unsettled. The gate had already done this once for
this branch, and a prior session had *reverted* it three times on the authority
of a plan that recorded reverting as the decision. Reverting loops: the next
gate run rewrites it. The memory's rule was followed instead — confirm the
regeneration is a fixed point (`make spellcheck` twice, identical
`git hash-object` both times), confirm the delta is a pattern swap and nothing
dropped, and commit that exact revision. That makes the gate's own input
stable: the re-run on the new head reports `current: typos.toml` rather than
`refreshed:`, which is what "fixed point" looks like from outside.

**A gate certificate was re-bound to the new head rather than carried over.**
`fa7f5b57` was certified green on four gates with the candidate unmoved across
every reading. Committing `typos.toml` after that moved HEAD, so the
certificate was re-established rather than transferred: the delta reaches only
`spellcheck`, which reads the file directly, so `make markdownlint` was run
again on `cf84c96f` and passed with the tree clean before and after.
`cargo fmt`, `mdtablefix --check` and the contracts suite do not read
`typos.toml`, so their results transfer on content identity — stated as a
transfer, with the empty diff as evidence, rather than implied as a fresh pass.

## Round 24: the rebase onto main's 8e4d3de3

The branch was rebased from `bc6dc41c` onto `origin/main` at `8e4d3de3`, seven
commits ahead of the shared merge-base `24698d0a`. `mergeStateStatus` was
`DIRTY`, so a real conflict was expected rather than a clean fast-forward. The
replay boundary is the merge-base itself, which is legitimate here because the
branch was never stacked on a squash-merged parent: the graph is a plain fork,
`git rev-list --merges 24698d0a..bc6dc41c` is empty, and the first replayed
commit is unambiguously child-owned.

**Weave was registered but not selected, and that was proven rather than
assumed.** The global config carries `merge.weave.driver`, but an empty
`~/.config/git/attributes`, no managed block, and no repository
`.gitattributes` rule for any extension leave every path reporting
`merge: unspecified` — checked directly on all six overlapping paths, not just
the two named in the probe list. Registration is not selection. The replay
therefore used Git's own text merge with `zdiff3`, and no driver override was
needed because nothing was routing through the driver in the first place.

**The conflict surface was one file, and the preview said so before any state
changed.** `git merge-tree --write-tree --merge-base` reported exactly one
content conflict, `ortho_config_macros/src/derive/load_impl/mod.rs`; the other
five overlapping paths auto-merged. Five of the six overlaps trace to a single
target commit, `fc4b590f` ("Inject sources into global composition"), which is
what made a one-file conflict likely and a mechanical resolution unsafe.

**The two sides were orthogonal, so the resolution is their union.** The target
moved `build_config_impl_delegates` out of `mod.rs` into `source.rs` to stay
under the 400-line ceiling, and added it to the `use source::{...}` group. The
branch hoisted `LoadSourceTokens` into a `pub(crate) use` and removed it from
that group. Both edits rewrite the same import block, which is the whole
conflict. Taking either side alone breaks the build: dropping the target's
addition leaves `build_config_impl_delegates` unimported at its call site, and
dropping the branch's hoist breaks `policy_impl.rs:5`, which reads
`LoadSourceTokens` *through* `load_impl` rather than from `source` directly.
Keeping `LoadSourceTokens` in both places would instead be a duplicate import.
The resolved block therefore keeps the hoist and adds
`build_config_impl_delegates` to the group.

**The replay was audited against evidence, not against a clean exit.**
`range-diff` shows 57 of 58 commits as `=`, with exactly one `!` — the conflict
commit — which localizes every semantic difference to the one hunk that was
actually resolved. The stronger test is a set comparison: the paths differing
between the old head and the new head are a bijection with the paths main
changed, 47 against 47, with no path in either difference. That proves two
things at once — no branch change was lost or mangled in the replay, and no
target change was dropped in the resolution. Each branch-owned production file
(`discovery/load.rs`, `discovery/policy.rs`, `discovery/scoped.rs`,
`source_scan.py`, `trybuild_tier.py`) is byte-identical to its old-head
revision, and no line deleted against the target is absent from the merge-base,
so nothing the target added was destroyed.

`Cargo.lock` is byte-identical to main's, which is what the rebase brief
requires of a lock file: take main's side, then rebuild only if the merge
changed a manifest. It did not. The only manifest main touched is
`examples/hello_world/Cargo.toml`, whose four added lines are main's own and
whose lock entries arrived with main's lock file; `cargo metadata --locked`
resolves, and `quote` sits at the `1.0.47` main pinned. No rebuild was needed,
and none was improvised.
