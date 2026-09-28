# Address review findings on PR #509 (subcommand discovery sources)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

PR [#509](https://github.com/leynos/ortho-config/pull/509) ("Inject subcommand
file discovery sources", issue #452) routed subcommand configuration-file
discovery through an explicit environment source and base path. The pull
request has since received a review round with two inline comments, two failing
checks (Testing (Overall), Unit Architecture), and five warnings (Docstring
Coverage, User-Facing Documentation, Developer Documentation, Testing (Property
/ Proof), Observability).

This plan covers addressing that review feedback only. Each finding is verified
against the current head before any change is made; findings that are already
satisfied by the code are recorded and skipped with a reason rather than
"fixed" by inventing work. The branch is
`harden-lint-subcommand-environment-replay-r2-20260923`, whose PR head was
`7029f0b7c5fc022af40d1b2c1ff56afd4cac6401` when this plan was written; see
`Progress` for the restacks since.

## Constraints

- All changes stay within the subcommand file-discovery layer plus
  documentation; the PR deliberately does not resolve the native `ProcessEnv`
  enumeration policy conflict or complete issue #452.
- Every functional change that introduces an error path must be tested: the
  repository's gates include mutation-style expectations, and new match arms
  add coverage obligations.
- `OrthoError` is `#[non_exhaustive]` and `merge_telemetry::error_category`
  matches it exhaustively, so any new variant needs a telemetry category.
- Rust files must stay under the repository's 400-line module limit, and
  Markdown prose must pass the `typos` en-GB-oxendict gate (never quote a
  misspelling verbatim in prose or inline code spans).
- `make check-fmt`, `make lint`, `make typecheck`, `make test`,
  `make markdownlint` must all pass, run sequentially (never in parallel), each
  captured with `tee`.
- The `docs/users-guide.md` and `README.md` Rust fences are compiled and
  executed by `ortho_config/tests/documentation_examples_rust_tests.rs`; new
  example identifiers must also be added to `EXPECTED_EXAMPLE_IDS`.

## Tolerances

- Escalate if a finding can only be satisfied by changing public API behaviour
  beyond the discovery layer, or if a property test cannot be expressed without
  mutating the process environment.
- Escalate if the docstring threshold cannot be met without editing files
  outside the PR's diff scope.

## Progress

- [x] (2026-09-27) Recon: branch had diverged (ahead 11 / behind 13) from a
      restacked origin; verified with `git cherry` that all local-only commits
      were stale pre-restack duplicates, then `git reset --hard origin/...` to
      the PR head. Old local tip `61dbbb8b` retained in reflog.
- [x] (2026-09-27) Recon: `git fetch`, `gh pr checks 509` (CI green,
      `reviewDecision: CHANGES_REQUESTED`).
- [x] (2026-09-27) Recon: read `paths.rs`, `paths_tests.rs`, `sources.rs`,
      `subcommand/mod.rs`, `env_source.rs`, the three doc targets, and the
      telemetry modules.
- [x] (2026-09-27) Finding 1: `docs/developers-guide.md` now names the discovery
      candidate reader and subcommand file discovery as the only production
      `EnvSource` readers, neither calling `std::env::var_os` directly, and
      describes the process-backed loaders as routing through `ProcessEnv`.
- [x] (2026-09-27) Finding 2:
      `load_and_merge_subcommand_with_matches_with_sources`
      now delegates to the `..._at` form with `ProcessEnv` and a `"."` base,
      matching `load_and_merge_subcommand_with_sources`; the
      `load_file_and_env_defaults` import is gone. `# Usage` documents that
      file discovery stays process-backed.
- [x] (2026-09-27) Finding 2 regression coverage:
      `process_backed_matches_wrapper_delegates_to_the_at_form`
      in `cli_default_as_absent_collections.rs` pins both halves of the
      delegation — the `./.app.toml` file layer (via `ProcessEnv` plus the `"."`
      base) and the injected `SharedScanEnvSource` environment layer. No
      coverage existed for the wrapper before, only for the `_at` form.
- [x] (2026-09-27) Testing (Overall):
      `xdg_dirs_search_prefers_first_directory_with_config`
      (two absolute `XDG_CONFIG_DIRS` entries, both holding `config.toml`, first
      wins) and `xdg_dirs_search_falls_through_to_later_directory`.
- [x] (2026-09-27) Unit Architecture: `candidate_paths_at`,
      `push_xdg_candidates`,
      and `collect_unix_paths` are fallible; `xdg_candidate_exists` treats only
      `NotFound` as absence and wraps every other probe error as
      `OrthoError::File { path, source }`. `subcommand/mod.rs` propagates with
      `?`; the affected `# Errors` sections are written.
- [x] (2026-09-27) Docstring Coverage: docs added (never deleted) to
      `paths.rs` internals, `env_source.rs::ProcessEnv::config_dir_fallback`,
      the `paths_tests.rs` helpers and cases, and the proptest strategies.
- [x] (2026-09-27) User-Facing Documentation: `docs/users-guide.md` gained
      "Choose the file-discovery source" with a compiled-and-run example
      (`guide-subcommand-sources`), registered in `EXPECTED_EXAMPLE_IDS` and
      `STANDARD_RUST_EXAMPLES`. The scribe's draft was corrected before
      registration: `&MapEnv::new()` bound inside a `let` statement is a
      temporary that dies at the end of that statement, so the context would
      have borrowed a dropped value; and with `XDG_CONFIG_DIRS` unset the
      search reaches the real `/etc/xdg`, which contradicted the hermeticity
      claim. The fence now binds the map to a named local and names both XDG
      variables.
- [x] (2026-09-27) Developer Documentation: `docs/design.md` gained
      "Subcommand file discovery" under §4.8, covering the injected-source
      candidate search, base ordering, metadata-only existence, the
      process-backed wrappers, and the non-absence probe failure contract.
- [x] (2026-09-27) Testing (Property / Proof): `paths_proptests.rs` — absolute,
      relative, and empty `XDG_CONFIG_DIRS` segments generated; absolute
      entries kept in order, `/etc/xdg/<prefix>` fallback, `XDG_CONFIG_HOME`
      leading the list, and first-existing-per-extension selection.
- [x] (2026-09-27) Observability: `paths_telemetry.rs` (module-private, closed
      vocabulary only) records base resolution (named/fallback/absent,
      `USERPROFILE`, platform config dir), candidate selection by base
      position, absence, and probe failure; counters behind the `metrics`
      feature. `subcommand_paths_telemetry.rs` pins the vocabulary and checks
      no path or value is disclosed.
- [x] (2026-09-27) Gate repairs on the first review commit ("Address PR #509
      review: fallible discovery, telemetry, docs"): the first full six-gate run
      came back red on `check-fmt` (Markdown reflow in three docs), `lint`
      (three Clippy errors) and `markdownlint` (one en-GB-oxydict spelling
      fault).
      `typecheck`, `test` (1415 Rust + 87 pytest) and `nixie` passed. All three
      faults are fixed in the follow-up commit; the spelling fault was repaired
      by rewording, never by quoting the offending form.
- [x] (2026-09-27) Module-size repair: the two files this round pushed past the
      400-line cap are back under it by extraction, not by trimming coverage.
      `SubcmdConfigMerge` moved to `subcommand/config_merge.rs` (`mod.rs`
      405 → 306) and the XDG base/selection cases moved to
      `subcommand/paths_xdg_tests.rs` (`paths_tests.rs` 461 → 349). Both new
      modules carry the module doc the extraction needs.
- [x] (2026-09-27) Whitaker repair: `lint-whitaker` had never actually run
      before, because `lint-clippy` always aborted first; once Clippy passed,
      Whitaker surfaced six `no_std_fs_operations` errors, all in the new
      `ortho_config/tests/subcommand_paths_telemetry.rs`. The file was
      converted to capability handles (`Dir::create_dir_all`,
      `Dir::set_permissions`, `cap_std::fs::Permissions::from_mode`) rather
      than added to `dylint.toml`'s exclusions; `dylint.toml` is unchanged.
      The suite still passes all three cases, and `is_privileged` is
      verifiably false for euid 1000, so the lock case is not vacuous.
- [x] (2026-09-27) Restack of the review branch: the base was force-updated a
      second time, adding "Run every cold trybuild binary alone on Windows"
      (#533). The three review commits were replayed onto the new tip with
      `git rebase --onto`, which skipped the two stale pre-restack ancestors
      that a plain rebase tried to replay (and which caused an add/add conflict
      in `paths_tests.rs`). `git range-diff` confirms all three patches are
      unchanged; the only delta from the previous tip is upstream's three
      files.
- [x] (2026-09-27) Gates: all six gates pass on the frozen review head. The
      first run straddled two revisions, because the execplan commit landed
      partway through it, so the whole suite was re-run on a frozen HEAD with a
      clean tree. Every gate returned 0, and the Whitaker half was verified to
      have genuinely executed — toolchain line, the per-crate checking list,
      and a real 6.69s compile — rather than short-circuiting to a cached
      `Finished`. `typos.toml` was regenerated by the `markdownlint` step and
      came out byte-identical, leaving the tree clean at the end of the run.
- [x] (2026-09-27) Commit, push, and PR body: the ExecPlan record was
      committed and pushed as a fast-forward, and the PR description was
      updated with the review walkthrough, the restack note, and the gate
      evidence above. A cosmetic re-wrap of this file followed, after
      `mdtablefix --check` and `markdownlint` flagged one over-long line that
      the first draft introduced; the Markdown gates were then re-run on the
      amended revision with HEAD frozen and the tree clean at both ends.
- [x] (2026-09-27) CodeRabbit re-review requested via the managed queue
      (`52f1e259`, posting in roughly 21 minutes). Findings are triaged when the
      review lands: each is verified against the current tree before any repair,
      and only still-valid issues are fixed.
- [x] (2026-09-27) Review round on the current branch head: CodeRabbit returned
      `CHANGES_REQUESTED` with two inline findings. Both were verified valid
      against the current tree before any repair.
- [x] (2026-09-27) Finding A (dead code on Windows): reproduced rather than
      reasoned about. `RUSTFLAGS="-D warnings" cargo check -p ortho_config
      --all-features --target x86_64-pc-windows-msvc` returned 101 with 14
      dead-code errors, and CI's `build-test` matrix does run `windows-latest`
      (`ci.yml:59`) with `RUSTFLAGS: -D warnings` (`ci.yml:34`), so this was a
      genuine CI break, not a style note. The non-Unix telemetry entry points
      are now gated; the shared vocabulary they call stays ungated, because
      `candidate_paths_at` and `error_category` reach it on both platforms.
- [x] (2026-09-27) Finding B (lost/mislabelled base event): the non-Unix
      collector's `HOME` branch emitted no event at all and its
      `home_fallback()` branch was labelled `named`. A
      `windows_home_from_fallback` entry point was added and each branch now
      reports its own source.
- [x] (2026-09-27) Two further Windows failures found by the same probe, both
      fixed: `paths_proptests.rs` and `paths_probe_tests.rs` are XDG-only
      branch-introduced suites that called Unix-gated helpers ungated, and are
      now compiled only under `cfg(any(unix, target_os = "redox"))`, matching
      the sibling `paths_xdg_tests.rs`; and `candidate_paths_at`'s
      immediately-called closure lost its only `?` under the Windows cfg,
      tripping `redundant_closure_call`, so each platform now owns its own
      `result` binding and the closure is gone.
- [x] (2026-09-27) Windows verified clean in CI's exact mode —
      `cargo clippy --target x86_64-pc-windows-msvc --all-targets
      --all-features -- -D warnings` (which is what `CLIPPY_FLAGS` expands to,
      and what the Windows lane runs) — with the matching Linux clippy clean
      and the focused `subcommand::paths` suite at 21 passed / 0 failed.
      `paths_proptests.rs`'s four properties all ran on Linux, so the new gates
      preserved their coverage rather than excluding it.

## Surprises & Discoveries

- The origin branch was force-updated by a restack onto `main`; the local
  worktree branch held stale pre-restack commits whose patch-ids do not appear
  upstream (`git cherry` showed `+` for seven of them). Reset was safe and the
  old tip is still reachable from the reflog.
- `xdg` 3.0.0's `find_config_file` does **not** return the first existing file
  across `XDG_CONFIG_DIRS`: `read_file` returns the first hit in `config_home`,
  and otherwise the first hit walking `config_dirs` in order. This matches
  `push_xdg_candidates`, which selects the first existing candidate per
  extension within a single ordered base list (`config_home` then each
  `config_dir`). The new test must therefore assert ordering across the whole
  base list, not just within `config_dirs`.
- `xdg`'s existence probe is metadata-only (`fs::metadata(path).is_ok()`), so a
  directory at a candidate path counts as existing. `push_xdg_candidates` uses
  `try_exists()`, which has the same property; the existing
  `xdg_search_keeps_metadata_existence_contract` test and the XDG-3 oracle
  child-process test pin that parity.
- `merge_telemetry::error_category` (src/merge_telemetry.rs:159) matches
  `OrthoError` exhaustively; a new error variant breaks it at compile time. The
  same applies to `paths_telemetry::error_category`, which mirrors it.
- The "Docstring Coverage" warning is not reproducible from anything in the
  repository: there is no docstring gate in the `Makefile`, `lading.toml`, the
  workflows, or `codecov.yml`, so the percentage comes from the review tool's
  own diff-scoped analysis. Measured locally over every symbol kind in the
  files this branch changes, the branch sits well below 80%; the largest and
  most fixable concentrations are the test helpers and test cases the PR itself
  adds (`paths_tests.rs` was 14.3% before this round). Adding docs there is
  both the honest fix and the one that moves the number most.
- The reference revision's docstring "fix" deleted existing doc comments in
  `env_source.rs` and `cli_default_as_absent.rs`. That raises a ratio by
  removing the documented items from the numerator and denominator at once, and
  is precisely the suppression this task forbids. Rejected.
- **This round pushed two code files past the 400-line cap that `AGENTS.md`
  mandates** ("No single code file may be longer than 400 lines"). Measured
  against the PR head `7029f0b7`: `ortho_config/src/subcommand/mod.rs` 399 →
  405 lines (the `?` change plus the new `# Errors` section on
  `load_file_and_env_defaults_at`) and
  `ortho_config/src/subcommand/paths_tests.rs` 371 → 461 lines (the two new
  XDG-directory cases, the doc comments added for coverage, and the
  fallible-call-site rewrites). `ortho_config/tests/extends.rs` is also 405
  lines, but it is untouched by this diff and was already over at base, so it
  is not ours to fix here. There is no automated gate for this rule
  (`make lint` does not check it), but the branch's own history shows it is
  treated as binding:
  `1dec114f Split oversized configuration test modules (#452)` and
  `f5b7972c Extract subcommand path tests (#452)` are this exact remedy for
  these exact files, and the parent branch is literally named
  `harden-lint-module-size-prerequisites`. Fix by extracting, not by trimming
  coverage: the added cases and their docs are the review's requested work.

- Clippy's suggested fix for the `shadow_reuse` error at
  `paths_telemetry.rs:198` was `u32::try_from(position)`, which would have
  self-recursed: the shadowing binding *is* the `try_from` call. The operand
  was renamed to `base_position` instead, and the saturation is now commented
  because `usize` → `u32` is genuinely lossy on 64-bit hosts.
- The Clippy errors in `ortho_config/tests/subcommand_paths_telemetry.rs` were
  introduced by this round, not inherited. `panic_in_result_fn` fires because
  the sibling telemetry suites (`merge_telemetry.rs`, `discovery_telemetry.rs`)
  return `()` while this one returned `Result<()>`; a `Result`-returning test
  may not `assert!`. The fix aligns with the siblings and the fallible setup is
  unwrapped with `expect`, which `clippy.toml` already permits in tests
  (`allow-expect-in-tests = true`). No assertion was weakened.
- No automated gate enforces the 400-line rule, but the branch history shows it
  is treated as binding, so the extraction was done even though `make lint`
  would not have flagged either file.
- **A `dylint.toml` exemption was drafted for the telemetry test target and then
  rejected.** A repository-wide scan for genuine `std::fs` call sites showed
  the new file was the *only* offender outside the three already-excluded
  crates, and the codebase already states its convention in
  `support/load_source.rs:21` ("Goes through a `cap_std::fs::Dir` handle for
  the same reason"): a capability handle names the directory it may touch, so a
  fixture cannot escape the temporary tree it was given. Adding a fourth
  exclusion would have widened the lint's blind spot to hide a single file's
  nonconformance, while also contradicting the comment already sitting directly
  above `excluded_crates` for `ortho_config` — that entry covers the *lib*
  crate, and the new file was an integration **test target**, which the existing
  `discovery_attributes` entry demonstrates is keyed by target name.
  Converting the file fixes the cause and leaves the lint's coverage intact.
- The conversion carried a subtlety worth recording: `XDG_CONFIG_HOME` must be
  an absolute path, but `cap_std::fs::Dir` addresses paths *relative to the
  directory it was opened on*. The test therefore keeps `locked_relative` (for
  the `Dir` handle) and `locked` (the absolute path it hands to the
  environment) as two names for the same location. Collapsing them would have
  made the variable unsettable, since a relative `XDG_CONFIG_HOME` is rejected
  by `source_xdg_bases`'s `is_absolute` filter — and the telemetry event that
  proves the code path ran would simply not have fired.

## Surprises & Discoveries (continued)

- **Third restack, after the review request was queued.** The base was
  force-updated again, rewriting all six branch commits: every commit kept its
  subject line but received a new object name. A queued CodeRabbit request does
  not pin the revision it will inspect, and this request had already been
  posted when the rewrite was noticed, so it will review whatever head is
  current when it runs. Recovery: fetch, confirm the rewrite with
  `git range-diff` (all six commits `=`), and — decisively — compare
  `git rev-parse <commit>^{tree}` for each old/new pair. Every tree matched, so
  the whole gate certificate transfers and no re-run is needed; only the object
  names move. Then reset the local branch to the remote truth rather than
  force-pushing the old commits back over the rewritten branch.

- **A Linux-only gate certificate cannot see a Windows-cfg defect.** The branch
  was certified green by six gates, yet still carried a break that CI would hit
  on its very next Windows run. The reason is structural, not a slip: every
  local gate compiles the Unix cfg, and the defect lived entirely inside the
  cfg the Unix build never expands. Gating a helper
  `#[cfg(not(any(unix, target_os = "redox")))]` and leaving its only caller on
  the Unix path is invisible to `cargo check`, Clippy, and the test suite on
  Linux, all of which simply never parse the dead branch. The lesson is to run
  the cross-compilation probe whenever a diff touches a platform cfg, because
  it is cheap and the alternative is discovering the break from CI after
  pushing. The same divergence produced three separate failures here — dead
  code, a closure lint that only fires once the `?` is cfg'd out, and two
  XDG-only test modules — so one probe run repaid itself three times.
- **Clippy's platform-dependent lints are part of the cfg contract.** The
  closure in `candidate_paths_at` was fine on Linux and rejected on Windows,
  not because the code differed but because the *post-cfg* item tree did. A
  lint that fires on only one platform cannot be found by any amount of local
  Linux gating. Making each platform own its own `result` binding removed the
  divergence and the lint together, which is the better repair: it deletes the
  construct the lint objects to rather than silencing it.

## Decision Log

- Decision: not fix at the third `try_exists()` site with a `.map_err(...)?`
  rewrite, because splitting error handling from candidate selection inside the
  `find` closure would make the code harder to read and the site is on the
  single named base path where `NotFound` is the common case. Rationale:
  convert the matching `Err` into a `NotFound` file error, keeping the
  selection predicate total, and let the caller's existing error handling
  surface it. Recorded here because the reviewer's suggested shape (return
  `Result` and propagate) is otherwise followed.
- Decision: represent the new failure as `OrthoError::File { path, source }`
  with an `std::io::Error` source rather than a new `OrthoError` variant.
  Rationale: `File` already means "error originating from a configuration path"
  and carries the affected path; the `downcast_ref::<std::io::Error>()` idiom
  is already used by `file/path.rs:151`. This keeps the `non_exhaustive` enum
  and its exhaustive telemetry match untouched.

## Outcomes & Retrospective

Both inline review findings are satisfied and pinned by tests:

- Finding 1 (developers' guide ownership statement): the two production
  candidate readers are named as the only `EnvSource` readers that do not call
  `std::env::var_os` directly, and the process-backed loaders are documented as
  routing through `ProcessEnv`.
- Finding 2 (wrapper delegation): the process-backed matches wrapper delegates
  to the `..._at` form, and
  `process_backed_matches_wrapper_delegates_to_the_at_form` pins both halves —
  the `./.app.toml` file layer and the injected `SharedScanEnvSource`
  environment layer. Previously only the `_at` form had coverage.

All six gates pass on the frozen review head, with Whitaker's execution
positively evidenced rather than inferred from a cached build line.

The dominant lesson of this round is that **a red Clippy gate hides Whitaker
entirely**. `make lint` lists `lint-clippy` and `lint-whitaker` as sibling
prerequisites, and `make` aborts at the first failing prerequisite, so a failing
`lint-clippy` means `lint-whitaker` never runs at all. Six
`no_std_fs_operations` errors in a file this branch itself added stayed
invisible through a whole review round that way. The fix belonged in the code,
not in `dylint.toml`: the file was the only genuine `std::fs` caller outside
the three already-excluded targets, and the repository already documents the
capability convention it should follow. Two further traps are recorded under
`Surprises`: the 400-line cap has no automated gate but is treated as binding
by the branch's own history, and `cap_std::fs::Dir` addresses paths relative to
its open directory while `XDG_CONFIG_HOME` demands an absolute one, so the test
needs two bindings for one location. A final process lesson: a commit landing
mid-gate-run invalidates the run's attribution even when every gate is green,
so HEAD must be frozen before a certificate is claimed.

### Second review round (Windows findings)

The re-review returned `CHANGES_REQUESTED` with two findings, both valid and
both repaired. Finding A was the more instructive one: it was a real CI break
that the six-gate certificate had passed over. CI compiles Windows with
`RUSTFLAGS: -D warnings`, so the non-Unix telemetry helpers — which no Unix
build ever calls, and no Unix build ever parses — became fourteen dead-code
errors. The finding was confirmed by reproducing it
(`cargo check --target x86_64-pc-windows-msvc`, exit 101), and the fix gates
each non-Unix entry point while leaving the shared vocabulary ungated, since
`candidate_paths_at` and `error_category` reach that vocabulary on both
platforms; gating it too would have traded one dead-code error for another.
Finding B was a genuine observability defect: the `HOME` branch produced no
base event and the fallback branch was mislabelled `named`, so an operator
could not tell which lookup won.

Chasing those two through the Windows cfg surfaced two more failures of the
same family, both fixed: the two XDG-only test modules added by this round
called Unix-gated helpers without a gate of their own, and the closure in
`candidate_paths_at` lost the `?` that kept `redundant_closure_call` quiet. The
closure was removed rather than allowed, because each platform owning its own
`result` binding is clearer than a lint exception. Verification is a clean
Windows clippy over the whole workspace in the exact mode CI runs (the
`CLIPPY_FLAGS` default of `--all-targets --all-features -- -D warnings`), plus
a clean Linux clippy and the focused `subcommand::paths` suite at 21 passed / 0
failed.

The round's lesson is recorded in full under `Surprises`: **the six-gate
certificate is a Linux certificate, and a defect confined to a non-Unix cfg is
structurally invisible to it**. The remedy is a cross-compilation probe
whenever a diff touches a platform cfg. That probe cost seconds here and would
have caught all three Windows failures before the review request was ever
queued.

### Third round (pre-merge row dispositions)

The same review left a pre-merge table with two errors and four warnings. Each
row was re-verified against the current head rather than taken at face value,
and the rows did not resolve uniformly — two were already repaired by the
second round's commits, three needed new work, and one was wrong.

Two rows were **already satisfied** and needed only evidence. Observability was
the pre-merge restatement of Finding B, so the `windows_home_from_fallback`
repair answers it; the row was left open only because the table had not been
recomputed. User-Facing Documentation overlapped the guide edit made in the
second round.

Three rows were **valid and needed new work**: Testing (Overall), for the three
`SubcmdConfigMerge` methods that no test reached; Testing (Compile-Time / Ui),
for the absence of any trybuild coverage of the newly public discovery
contracts; and Developer Documentation, which correctly noticed that the plan
still declared `IN PROGRESS` while its own Outcomes section asserted the work
was finished.

The Testing (Overall) row deserves a provenance correction, because its
attribution is easy to get wrong and was wrong in the first draft of this
record. The three methods are *not* new. `git grep` at the live base `5732adf9`
finds all three defined on `pub trait SubcmdConfigMerge` in
`ortho_config/src/subcommand/mod.rs:330,348,362`, with the trait re-exported
from `lib.rs:63`; what this PR contributes is richer coverage, not the API.
`git log -S` traces them to `056ccfa5` (#412). A first-pass check appended
`-- ortho_config/src/subcommand/config_merge.rs` and read the resulting zero,
but that file does not exist at base — the trait was extracted into it by this
branch — so the empty result measured the extraction, not the API. The
corrected statement is stronger for the review, not weaker: the methods are
pre-existing public surface that shipped untested, and the Base-layer evidence
below is what establishes the difference.

Only the discovery surface is genuinely new at the base:
`SubcommandFileContext`, `SubcommandCliMatches`, `candidate_paths_at`, and
`paths_telemetry` all return zero hits at `5732adf9`, and those are the
contracts the trybuild fixtures pin.

One row was **rebutted with base-layer evidence**. Unit Architecture asked for
`candidate_paths_at` to be made side-effect free, arguing that this PR
introduced telemetry into a query. The premise is wrong on both counts. The
started/finished pattern already existed at the PR base: `csv_env/mod.rs`'s
`data()` — a Figment provider method on the read path — calls
`merge_telemetry::csv_env_injected_started()` and its siblings inside the same
function that collects entries, and `merge_telemetry` is a base-layer module,
declared at `lib.rs:57` in the base tree. The row's supporting detail is also
factually wrong: `grep -n 'tracing::\|metrics::' paths.rs` returns nothing,
because every emission goes through the `paths_telemetry` façade. That façade
accepts no caller-controlled text — `candidate_exists` takes only a `usize`
position, and the remaining entry points take no arguments at all — so paths,
environment values, and host state never become event fields. Routing telemetry
through the façade is the design under review, not an oversight in it.

The Developer Documentation row is closed by the status line at the top of this
plan, which now declares `COMPLETE` and matches the checked `Progress` list.

### Third-round repairs and the evidence for them

The three valid rows were repaired in the commits that followed `a3ec6c82` and
each repair was confirmed by execution rather than by inspection, because every
one of them added code that had never been compiled.

Those repairs did not land with PR #509. They were still uncommitted when #509
merged as `f303aa11`, and the branch that carried them was deleted by that
merge, so the rows recorded here were repaired *after* the review that raised
them had already closed. This plan is retained rather than rewritten because
the dispositions above are the evidence for the row-by-row reasoning; the code
they describe is delivered separately, in the pull request that carries this
revision of the file.

- Testing (Overall) — `ortho_config/tests/subcommand_merge_methods.rs`, seven
  cases across the three methods. Each sentinel is distinct per layer
  (`file_ref`/`injected_ref`/`cli_ref` and `4`/`5`/`7`/`9`/clap's `2`), so no
  case can pass by coincidence, and each stages its own temp directory through
  `test_helpers::cwd::set_dir` rather than relying on the ambient one.
  `cargo test -p ortho_config --test subcommand_merge_methods` → 7 passed, 0
  failed.
- Testing (Compile-Time / Ui) — `ortho_config/tests/subcommand_trybuild.rs`
  with a pass fixture and a compile-fail fixture. The first confirmation run
  came back **red**, and the failure was instructive: the pass fixture compiled
  and ran (`[should pass] ... ok`), while the compile-fail fixture failed for
  exactly the intended reason but had no accepted `.stderr`, so trybuild wrote
  `wip/subcommand_merge_requires_extractor.stderr` and failed the test.
  trybuild requires the committed file; it does not accept the snapshot for
  you. The captured diagnostic names `CliValueExtractor` four times across two
  `E0277` blocks, one per withheld method, so it pins the intended bound rather
  than some incidental error. After accepting it byte-exact the suite is
  `1 passed; 0 failed`, and no `wip/` is regenerated. Note where trybuild
  writes: `ortho_config/wip/`, not beside the fixture, and it drops its own
  `.gitignore` containing `*` there — so the snapshot is invisible to
  `git status` and to `git add -A` until it is copied out deliberately.
- User-Facing Documentation and Developer Documentation — the migration-guide
  section, the `COMPLETE` status line, and the `docs/contents.md` index entry,
  all checked for the 80-column fill and the en-GB-oxendict vocabulary.

The `.config/nextest.toml` Windows override was deliberately **not** extended
to name the new binary. The override exists because cold trybuild child builds
exhausted the 600s per-test allowance on Windows, and it names exactly the four
binaries whose tests had done so — a contract test asserts that set exactly.
`subcommand_trybuild` has no such history, and three existing trybuild
binaries, including `localized_parse_trybuild` with its own `compile_fail`
fixture, are likewise unnamed. Naming it without evidence would widen a
contract test and its documentation on speculation. If the Windows leg does
time out on it, the remedy is to add the binary to the override *and* update
`TRYBUILD_BINARIES` in
`tests/workflow_contracts/windows_trybuild_isolation_test.py` in the same
change.
