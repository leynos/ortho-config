# Address review findings on PR #509 (subcommand discovery sources)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

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
