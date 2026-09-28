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

| Row                            | Sev     | Disposition                                                                                                                                                                               | Evidence                                                                                                                                        |
| ------------------------------ | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Testing (Overall)              | Error   | **Partially addressed** — closed for `selected_error`, `reportable_errors`, `merged_file_value`, `push_into`; the "CLI/environment selector precedence" ask is closed by the Round 6 fold | Each of the four named items has assertions at `scoped_layers.rs:271`, `:305`, `:348`, `:359`; ordered precedence at `:121`                     |
| Testing (Unit And Behavioural) | Error   | **Partially addressed** — both bracketed and parenthesized list forms now tested, plus all four `project_root_from` failures                                                              | `discovery_validation.rs` (6 tests); valid `Option<PathBuf>` at `policy_sources.rs:90`                                                          |
| Unit Architecture              | Error   | **Repaired**                                                                                                                                                                              | `ScanError(OSError)` at `trybuild_tier.py:64`; `os.walk(..., onerror=refuse)` at `:99`; all six named functions exist and route through `_scan` |
| Docstring Coverage             | Warning | **Partially addressed by constraint** — see Round 7                                                                                                                                       | Delta private items 100% except two files at the 400-line cap                                                                                   |
| User-Facing Documentation      | Warning | **Addressed** — the guide work landed at `e8adc959`; the migration-guide half is Round 8                                                                                                  | `developers-guide.md:732`; `documentation_examples_tests.rs:42`                                                                                 |
| Developer Documentation        | Warning | **Already satisfied on this head; the walkthrough text is stale**                                                                                                                         | `developers-guide.md:732`, `:750`, `:778`, `:836` name every type the row lists                                                                 |
| Testing (Property / Proof)     | Warning | **Addressed**                                                                                                                                                                             | `scoped_stacking_proptest.rs` is a reference-model property suite over generated candidate sets, scope orders, and canonical aliases            |
| Testing (Compile-Time / Ui)    | Warning | **Declined, with a reason**                                                                                                                                                               | See below                                                                                                                                       |
| Observability                  | Warning | **Addressed**                                                                                                                                                                             | `load_outcome` ends in `count_outcome` (`telemetry.rs:295`); `discovery.policy` event added; both documented                                    |

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

**`main` is clean, which is what makes it ours.** `cs check` on
`git show origin/main:tests/workflow_contracts/nextest_budgets.py` scores
10.00, so the finding is introduced by this branch rather than inherited, and
the plan's earlier "it is genuinely ours" reasoning applies here too.

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
  "absent", not a claim that the event carries an empty value.

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
  `<!-- tested-example: guide-scoped-discovery -->` example that
  `ortho_config/tests/documentation_examples_tests.rs:42` actually compiles and
  runs. The migration-guide half is the one genuinely open piece and is handled
  in the reconciliation below.

The scribe then found a defect **in the brief it was given**, which is worth
recording because the brief was mine. It was told to add `policy_resolve` to
`discovery.load`'s `operation` list, and did; but it pointed out that naming
`policy_resolve` there makes the row's `outcome` list wrong. The legacy path
reaches `load_outcome` with only `success` or `not_found` — a failed candidate
is reported through `discovery.candidate` instead — whereas the policy path has
no candidate event at all and so carries all four terminals itself: `success`
(`policy.rs:149`), `not_found` (`163`), `required_failure` (`170`, `178`), and
`optional_failure` (`186`). Both documents therefore under-described the
operation. The `outcome` list in `docs/design.md` and the `outcome` cell in
`docs/developers-guide.md` were widened to say so, and the reason is stated
inline rather than left as unexplained vocabulary.

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
   Renaming defeated the point of the previous round's decision: the sibling
   `-ised` identifier at `clap_attrs.rs:300` is **pre-existing on `origin/main`
   ** and is not ours to rename, so the precedent we now follow is the one the
   overlay already uses for quoted identifiers. The gate scans prose and inline
   code spans and does **not** flag `.rs` files — proven, not assumed: the same
   spelling at `clap_attrs.rs:300` is tracked and unflagged, while the
   plain-text rewrite of it in the plan had failed the gate minutes earlier.
3. **My own new paragraph tripped `check-fmt`.** `mdtablefix --wrap` wanted a
   line joined at 80 columns. Fixed with `make fmt` (the tool the gate runs)
   rather than by hand-wrapping, which is what produced the discrepancy in the
   first place: `mdtablefix` reflows with its own fragment model, so a
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
and the constraint is explicit: other agents' processes are not ours to kill.**
The earlier reading of this same process as merely slow was wrong; it is
self-deadlocked and needs its owner. Recorded so the next attempt escalates
against a diagnosis instead of re-deriving it.

Both mistakes this round were mine and both were the same shape: a delegation
brief asserting a fact I had not checked — a script that does not exist, and an
anchor that was one of two candidates. Confirmed after the fact by reading
`AGENTS.md:138` (`make check-fmt`, `make lint`, `make test`, then commit; there
is no gated-commit wrapper) and by reading the heading levels.

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

The deeper cause is the commit's own shape. `51e2e974` swept up a large body of
previously-uncommitted work along with the round-4 repairs — 16 files, ~1900
insertions — so the compile error arrived in a commit whose message described
only the scan guard, the spellings, and the anchor. Small, focused commits are
what `AGENTS.md:65` asks for and what makes a failure like this attributable to
one change rather than to sixteen.

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
