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
"fixed" by inventing work. The branch is `harden-lint-subcommand-environment-replay-r2-20260923`,
whose PR head is `7029f0b7c5fc022af40d1b2c1ff56afd4cac6401`.

## Constraints

- All changes stay within the subcommand file-discovery layer plus documentation;
  the PR deliberately does not resolve the native `ProcessEnv` enumeration policy
  conflict or complete issue #452.
- Every functional change that introduces an error path must be tested: the
  repository's gates include mutation-style expectations, and new match arms add
  coverage obligations.
- `OrthoError` is `#[non_exhaustive]` and `merge_telemetry::error_category` matches
  it exhaustively, so any new variant needs a telemetry category.
- Rust files must stay under the repository's 400-line module limit, and Markdown
  prose must pass the `typos` en-GB-oxendict gate (never quote a misspelling
  verbatim in prose or inline code spans).
- `make check-fmt`, `make lint`, `make typecheck`, `make test`, `make markdownlint`
  must all pass, run sequentially (never in parallel), each captured with `tee`.
- The `docs/users-guide.md` and `README.md` Rust fences are compiled and executed
  by `ortho_config/tests/documentation_examples_rust_tests.rs`; new example
  identifiers must also be added to `EXPECTED_EXAMPLE_IDS`.

## Tolerances

- Escalate if a finding can only be satisfied by changing public API behaviour
  beyond the discovery layer, or if a property test cannot be expressed without
  mutating the process environment.
- Escalate if the docstring threshold cannot be met without editing files outside
  the PR's diff scope.

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
- [x] (2026-09-27) Finding 2: `load_and_merge_subcommand_with_matches_with_sources`
      now delegates to the `..._at` form with `ProcessEnv` and a `"."` base,
      matching `load_and_merge_subcommand_with_sources`; the
      `load_file_and_env_defaults` import is gone. `# Usage` documents that
      file discovery stays process-backed.
- [x] (2026-09-27) Finding 2 regression coverage: `process_backed_matches_wrapper_delegates_to_the_at_form`
      in `cli_default_as_absent_collections.rs` pins both halves of the
      delegation — the `./.app.toml` file layer (via `ProcessEnv` plus the `"."`
      base) and the injected `SharedScanEnvSource` environment layer. No
      coverage existed for the wrapper before, only for the `_at` form.
- [x] (2026-09-27) Testing (Overall): `xdg_dirs_search_prefers_first_directory_with_config`
      (two absolute `XDG_CONFIG_DIRS` entries, both holding `config.toml`, first
      wins) and `xdg_dirs_search_falls_through_to_later_directory`.
- [x] (2026-09-27) Unit Architecture: `candidate_paths_at`, `push_xdg_candidates`,
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
      registration: `&MapEnv::new()` in a `let` initialiser is a temporary that
      dies at the end of the statement, so the context would have borrowed a
      dropped value; and with `XDG_CONFIG_DIRS` unset the search reaches the
      real `/etc/xdg`, which contradicted the hermeticity claim. The fence now
      binds the map to a named local and names both XDG variables.
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
- [ ] Gates: fmt, lint, typecheck, test, markdownlint.
- [ ] Commit, push, request CodeRabbit re-review, update PR body.

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
  `OrthoError` exhaustively; a new error variant breaks it at compile time.
  The same applies to `paths_telemetry::error_category`, which mirrors it.
- The "Docstring Coverage" warning is not reproducible from anything in the
  repository: there is no docstring gate in the `Makefile`, `lading.toml`, the
  workflows, or `codecov.yml`, so the percentage comes from the review tool's
  own diff-scoped analysis. Measured locally over every symbol kind in the
  files this branch changes, the branch sits well below 80%; the largest and
  most fixable concentrations are the test helpers and test cases the PR
  itself adds (`paths_tests.rs` was 14.3% before this round). Adding docs
  there is both the honest fix and the one that moves the number most.
- The reference revision's docstring "fix" deleted existing doc comments in
  `env_source.rs` and `cli_default_as_absent.rs`. That raises a ratio by
  removing the documented items from the numerator and denominator at once,
  and is precisely the suppression this task forbids. Rejected.

## Decision Log

- Decision: not fix at the third `try_exists()` site with a `.map_err(...)?`
  rewrite, because splitting error handling from candidate selection inside the
  `find` closure would make the code harder to read and the site is on the
  single named base path where `NotFound` is the common case.
  Rationale: convert the matching `Err` into a `NotFound` file error, keeping
  the selection predicate total, and let the caller's existing error handling
  surface it. Recorded here because the reviewer's suggested shape (return
  `Result` and propagate) is otherwise followed.
- Decision: represent the new failure as `OrthoError::File { path, source }`
  with an `std::io::Error` source rather than a new `OrthoError` variant.
  Rationale: `File` already means "error originating from a configuration path"
  and carries the affected path; the `downcast_ref::<std::io::Error>()` idiom is
  already used by `file/path.rs:151`. This keeps the `non_exhaustive` enum and
  its exhaustive telemetry match untouched.

## Outcomes & Retrospective

(Filled in as work completes.)
