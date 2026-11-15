# Ship the en-US clap error matrix (11.2.1)

Status: M3 complete — the matrix, the en-US strings, the runtime consultation,
the pre-publication gate, the consumer and developer documentation, and the
roadmap checkbox are all in place. The branch has been replayed onto
`origin/main` after PR #664 merged, its local review findings are
dispositioned, and the rebased candidate is awaiting its full gate certificate
before publication (2026-11-15).

This ExecPlan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current at every milestone.

## Purpose / big picture

Consumers will be able to enumerate the clap error kinds supported by this
release, identify which Fluent message to override, and receive embedded en-US
text for every non-display error. Help and version requests will pass through
untouched. A pre-publication test gate will compare the source inventory for
the supported clap identity with its reviewed count and fail when that
inventory changes. A compile-time assertion will separately ensure that the
matrix length equals that reviewed count. This is roadmap task 11.2.1 only.

The deliverable of this branch is the feature, its pre-publication gate, and
the governing documents that describe them. M0 established that the approved
build-time mechanism cannot bind consumers to their governing Clap resolution.
At the user's request the pre-publication workspace gate replaced it, and M0.1
amended the design, recorded ADR-0022, and reconciled the roadmap and the
developers' guide. M1 onward implements the matrix, strings, runtime
integration, and gate.

## Constraints

Implement the public matrix and en-US strings from
[CLI localization design](../cli-localization-design.md) §§6.1 and 6.2. Replace
§6.1.1's unsupported consumer build-script binding with a pre-publication
workspace gate. The gate must read the governing workspace `Cargo.lock` for the
selected production `clap` package identity (version, source, checksum) and its
`clap` → `clap_builder` dependency edge, then cross-check the resolution with
`cargo metadata --locked --offline` using the workspace's production
target/features. Resolve the metadata `manifest_path` for that exact
`clap_builder` package and count its top-level `ErrorKind` variants with `syn`.

Support only an explicitly reviewed identity and an unambiguous, audited
variant inventory. Fail the gate on unknown or overridden identities,
lock/metadata/source/edge mismatches, ambiguous resolution, or cfg-dependent
declarations whose complete inventory has not been audited. The parser counts
every declared variant without classifying display-only entries. Keep a
reviewed test constant for the supported identity's variant count.
Independently require the source count to equal that reviewed count, then use
`static_assertions::const_assert_eq!(CLAP_ERROR_IDS.len(), CLAP_ERROR_KIND_COUNT)`
to compare the matrix length to the reviewed constant. Source growth therefore
fails the publication gate until reviewed and the constant is deliberately
updated; the compile-time assertion alone does not detect dependency source
growth.

This gate describes the repository's selected workspace resolution at
publication time. It does not claim to identify an external consumer's Clap
resolution or reject that consumer's build. Do not add `build.rs`, `rustc-env`,
`OUT_DIR` count artefacts, or a consumer-side identity rejection. Do not use an
unbound hard-coded count, registry-directory glob, enum discriminant maximum,
or a second independently resolved dependency as the authoritative inventory.

Preserve the four existing Fluent identifiers and their argument contracts.
Ship en-US strings unconditionally. Add the public enum and constant through
both existing export layers. Use the matrix as the runtime mapping authority;
all three display-only entries bypass lookup and return the original error. The
publication gate detects a changed supported source inventory before release;
runtime builds retain the crate's existing fallback behaviour for any kind the
current matrix does not translate. Do not implement the coverage-report
builder, richer clap context preservation, locale lifecycle, embed bridge, or
derive work. Those belong to other roadmap entries.

Keep the declared MSRV, Rust 1.89, and validate with the workspace toolchain in
`rust-toolchain.toml` (currently Rust 1.99); retain caret dependency
requirements. Do not change the toolchain, compiler admission, shared Cargo
cache, or Whitaker installation. Use capability-oriented filesystem access and
UTF-8 paths in new Rust test tooling. New Rust files must have module
documentation and remain at most 400 lines. Do not add runtime or build
dependencies for source inspection. Keep `cargo_metadata` and `syn` scoped to
the publication gate and its tests, and `static_assertions` scoped to the
matrix test.

Implementation uses test-first development. Do not commit an intentionally
failing red stage. Every code milestone must pass sequential `make check-fmt`,
`make test`, `make typecheck`, and `make lint` before its commit. Use the
scrutineer agent for full gates and keep logs under `/tmp`; build products stay
under `target/`.

## Tolerances (exception triggers)

Stop implementation and set this plan to BLOCKED if the publication gate cannot
uniquely match the locked production facade edge to an explicitly supported
`clap_builder` identity, resolve its source manifest, or count its complete
`ErrorKind` inventory. Unknown identities, source overrides, mismatches,
ambiguous graphs, and unaudited cfg-dependent declarations must fail closed. Do
not claim or add a downstream consumer rejection guarantee. Record the evidence
and return to the design owner if the selected workspace-gate contract cannot
be met. Escalate public APIs beyond the requested enum and constant, changes
outside the runtime localizer, publication-gate/test support, and their
documentation, toolchain changes, or dependencies outside the test-only
candidates listed below. There is no time limit. Infrastructure failures are
blocked validation, not permission to weaken a gate. Disk exhaustion requires
stopping and reporting.

## Risks

- High impact, **realized and addressed by the selected plan revision**
  (2026-11-09): a dependency build script cannot establish the consumer's
  governing resolution. M0 found this limitation for external path and
  unpacked-package consumers; the workspace probe could observe its governing
  resolution. The selected contract checks the repository's supported workspace
  resolution at publication time only; see "M0 outcome". It makes no claim
  about arbitrary downstream dependency graphs.
- High impact, medium likelihood: equal matrix and enum lengths can conceal a
  duplicate plus an omission. Test uniqueness and exact inventory agreement.
- Medium impact, high likelihood: clap does not supply every context variable
  for every error construction. Inventory real contexts before writing FTL;
  generic errors must render without unresolved Fluent variables.
- Medium impact, medium likelihood: the reviewed count can become stale after
  a lock, source, patch, or Cargo configuration change. Recompute source
  inventory in the publication gate and fail when it differs from the reviewed
  identity record; never accept stale count evidence.
- Medium impact, medium likelihood: changing the missing-argument help variant
  to pass-through affects existing BDD expectations. Update those expectations
  deliberately while retaining actual `MissingSubcommand` translation tests.

## Progress

- [x] (2026-10-14) Read repository guidance and requested skills; renamed
      branch.
- [x] (2026-10-14) Gathered CodeGraph and Wyvern source/documentation evidence.
- [x] (2026-10-14) Checked Cargo and clap documentation through Firecrawl.
- [x] (2026-10-14) Completed six-perspective expert review and incorporated
  its fixture, identity, failure-diagnostic, and invalidation findings.
- [x] Publish this plan in draft PR #588.
- [x] (2026-11-08) Rebase the published draft plan on PR #651 (`ddba3578`) and
      refresh its baseline and integration contracts.
- [x] (2026-11-09) Rebase onto the timeout-fix branch for PR #664.
- [x] Retarget draft PR #588 to `issue-644-nested-cargo-timeouts`.
- [x] Complete fresh formatting, lint, typecheck, and full test validation on
  parent `501ab831`.
- [x] Validate documentation and workflow contracts; prepare the revised plan
  for publication.
- [x] (2026-11-09) Obtain explicit implementation approval.
- [x] (2026-11-09) M0: investigate identity-manifest and facade-edge
      feasibility; original build-time mechanism reached its stop condition.
- [x] (2026-11-09) Select the pre-publication workspace gate after M0.
- [x] (2026-11-09) M0.1: amended design §6.1.1, §6.1, and §12; recorded
      [ADR-0022](../adrs/adr-0022-pre-publication-clap-coverage-gate.md);
      reconciled roadmap 11.2.1 and the developers' guide; indexed the ADR.
- [x] (2026-11-10) M1: implemented the canonical matrix, the ten new en-US
      strings, runtime consultation, and the V1–V3 tests. The matrix,
      translation, display-only, BDD, and CLI suites pass; both negative
      controls turned the intended assertions red and the restored tree went
      green again.
- [x] (2026-11-10) M1 follow-up: the full workspace test run exposed
      `examples/hello_world`, which still expected a missing-input parse error
      to carry consumer copy. Realigned the example with the sentinel contract
      (its consumer override now targets `clap-error-invalid-subcommand`) and
      updated design §4.12 and the users' guide; `hello_world --lib` is green.
- [x] (2026-11-14) M1 committed as `54a7e97e` after a full gate certificate on
      frozen tree `2f37742e`: `check-fmt`, `typecheck`, `lint`, `test`,
      `markdownlint`, and `nixie` all passed, plus `cargo test -p ortho_config
      --doc` (80 passed). `make test` reported 2468 passed, 0 failed, 37
      ignored across 112 binaries, with the pytest step green (119 passed, 16
      skipped). `clap_error_matrix` (22), `clap_error_translations` (21),
      `clap_error_cli` (5), and `localised_help` (19) all executed.
- [x] (2026-11-14) M2: added the test-only publication gate.
      `clap_error_coverage` reads `cargo metadata --locked --offline`, resolves
      `clap` and `clap_builder` across the normal facade edge, cross-checks
      both against the workspace lock (version, source, checksum, and edge),
      locates the `ErrorKind` source through metadata's `manifest_path`, and
      compares the `syn`-parsed variant count with `CLAP_ERROR_KIND_COUNT`. The
      same constant drives `static_assertions::const_assert_eq!` in the matrix
      test. Committed fixtures cover the supported and an unknown identity,
      checksum and source drift, a missing facade edge, an ambiguous builder,
      variant drift, a feature-gated variant, a duplicate definition, a file
      without a definition, and malformed input; three `proptest` properties
      cover declaration order, shape, comments, discriminants, conditional
      variants, and truncation. The target reports 17 passed. Controls: 16 in
      place of 17 failed the const assertion with `E0080` at the assertion
      line, and replacing the `DisplayVersion` matrix row with a duplicate
      `DisplayHelp` row at unchanged length failed `matrix_kinds_are_unique`,
      `display_only_rows_are_the_three_render_variants`, and the
      `display_version` inventory case. Both mutations were restored.
- [x] (2026-11-15) M2 committed as `d726f3cc` (25 files). The first gate run
      failed `lint` on two Clippy diagnostics in the new proptest helper
      (`doc_markdown` on `ExecPlan`, `collapsible_if` at the gate insertion);
      after backticking the term and folding the nested `if` into a let-chain,
      a full re-run on frozen tree `39cd27c3` passed all seven gates:
      `check-fmt` (2 s), `typecheck` (1 s), `lint` (36 s, including Whitaker and
      docstring coverage 598/598), `boundary` (45 s, 35 passed), `test` (256 s,
      116 green suite results, pytest 119 passed), `markdownlint` (12 s), and
      `nixie` (1 s). The frozen tree re-derived identically after the last
      gate.
- [x] (2026-11-15) M3: reconciled the consumer and developer documentation.
      `docs/users-guide.md` gained the enumeration example, the 17-row
      `ErrorKind` outcome table, the four historical identifiers, override
      guidance built on `FluentLocalizer::with_en_us_defaults`, the display-only
      handling, the missing-input help change, and the fallback. The example is
      registered as `guide-clap-error-matrix` in its fence marker, in
      `SIMPLE_RUNS`, in `STANDARD_RUST_EXAMPLES`, and in
      `EXPECTED_EXAMPLE_IDS`, so both documentation-example targets execute it
      rather than only list it. `docs/developers-guide.md` documents gate
      ownership, prerequisites, the two run commands, a refusal-to-repair
      table, and the seven-step procedure for a new clap variant;
      `docs/repository-layout.md` inventories the new target;
      `docs/cli-localization-design.md` §6.1.1 names where the reviewed identity
      and count live; `CHANGELOG.md` records the added coverage and the changed
      missing-input behaviour. Roadmap 11.2.1 and its three sub-items are
      checked, with their finding and validation notes. A full seven-gate
      certificate on frozen tree `529721bb` is green: `check-fmt` (1 s),
      `typecheck` (2 s), `lint` (10 s, including rustdoc, Clippy, Whitaker, and
      docstring coverage 598/598), `boundary` (13 s, 35 passed, 7 ignored),
      `test` (171 s, 116 green suite results, the documentation-example targets
      at 58 passed/13 ignored and 14 passed, pytest 119 passed), `markdownlint`
      (18 s, 99 files, spellcheck and `typos.toml` unchanged), and `nixie`
      (2 s). The frozen tree re-derived identically after the last gate.
- [x] (2026-11-15) M3 follow-up: local review, rebase, and review repairs.
      A local `coderabbit review --agent` pass covered all 62 changed paths
      across the eight branch commits (`501ab831..ee3eed22`, CLI 0.9.5, exit 0,
      357 s) and raised seven findings. Four were actioned, three were
      dispositioned with evidence. Actioned: the ADR link in
      `docs/cli-localization-design.md` used `../adrs/` from inside `docs/`;
      `clap_error_cli` now builds and runs its fixture through
      `test_helpers::process::run_with_deadline`; the fixture executable is read
      back from Cargo's JSON artefact stream instead of being derived from the
      test binary's own directory, which was wrong for a release or cross-target
      run; and `load_metadata` now runs `cargo metadata` through the same
      deadline-bounded runner using `MetadataCommand::cargo_command`. Two
      assertion groups gained expectation-specific messages. Dispositioned: the
      claim that six `Cargo.lock` fixtures are untracked is false (`git
      ls-files` lists all six and none is ignored), and two findings prescribed
      a helper that did not exist at the reviewed commit. The branch was then
      rebased onto `origin/main` (`959dbd7e`, containing PR #664 as squash
      `eaf4c028`); the four timeout-fix commits were dropped as already
      squashed. The rebase exposed an ADR number collision — main had taken
      0018 and 0019 — so this branch's ADR moved to `adr-0020-pre-publication-
      clap-coverage-gate.md` (since 0022, per the rebase entry below) and every
      reference was renumbered in the same commits. The first full certificate
      on the rebased candidate (`89613ae2`) was green on six gates and red on
      `make lint`: Whitaker's `conditional-max-n-branches` rejected the
      three-term predicate in `reported_executable`. The predicate is now split
      into named one- and two-term helpers, and the record selection runs
      through `filter_map`/`filter`/`find_map` so no redundant closure remains
      for Clippy. `lint-clippy`, `lint-whitaker`, `docstring-coverage`, and the
      five-case CLI suite all pass on the repair. Full re-gating follows.
- [x] (2026-11-15) Second local review round and its dispositions. A second
      `coderabbit review --agent` pass over `959dbd7e..8b1761e6` (62 files, CLI
      0.9.5, exit 0, 408 s) raised four findings. Three were actioned. The
      gate's `cargo metadata` call gained `--filter-platform host-tuple`: an
      unfiltered resolve includes every target, and an offline run fails when a
      dependency for another platform was never fetched, which the Linux CI
      cache cannot guarantee. The shared runner's documentation no longer
      claims output bounding — the runner retains each stream's complete
      output, so its deadline bounds time rather than bytes. The matrix test
      dropped `googletest`/`pretty_assertions` in favour of the standard
      `assert_eq!`, matching the developers' guide's scoping of those crates to
      the profile tests. One finding was declined: it read "an incomplete
      locale and the embedded en-US catalogue" as a non-en-US locale, but the
      scenario is titled "An unavailable override retains usable English" and
      asserts that a missing consumer override falls back to the embedded en-US
      message; the non-en-US path is covered by
      `unavailable_locale_retains_the_stock_diagnostic` and by the translation
      suite.
- [x] (2026-11-15) Third local review round and its dispositions. A third
      `coderabbit review --agent` pass over `959dbd7e..984a25d1` (62 files, CLI
      0.9.5, exit 0, 397 s) raised three findings. Two were actioned. The
      renumber left the gate ADR's heading reading `ADR 0018` while its
      filename and every inbound link said 0020; the heading now agrees.
      `run_fixture` clears the child's environment before injecting the locale
      and `NO_COLOR`, so a variable inherited from the developer's shell cannot
      reach the rendering under test; the child-only mutation is the
      repository's stated exemption. The third finding is declined as false:
      it reports that no file under `tests/fixtures/clap_publication` is
      tracked, while `git ls-files` lists all twelve at `984a25d1` and nothing
      ignores them. That is the second round in which this reviewer reported
      tracked fixtures as untracked, so the claim is a review-tool artefact
      rather than a repository state.
- [x] (2026-11-15) CodeScene repair before readiness. `cs delta 959dbd7e
      4cbce31f` reported two introduced findings, both in new test modules:
      `Large Method` on `parse_error` (70 lines against a 70-line threshold)
      and `Bumpy Road Ahead` on `render_enum` (two nested conditional blocks).
      The reproducer table split into value and structure families behind one
      `reproducer` selector, and the enum renderer split into gate, variant,
      and unit-variant writers. `cs review` now reports 10.0 with no findings
      for both files, and the mutable working tree's delta is empty.
- [x] (2026-11-15) Lint repair on the CodeScene commit. The refactor's
      `reproducer` selector ended in `.unwrap_or_else(|| panic!(..))`, which
      Whitaker's `no_unwrap_or_else_panic` denies, so `make lint` stopped
      before `docstring-coverage`. Rewriting it as a `match` instead tripped
      Clippy's `clippy::option-if-let-else`, which prescribes
      `Option::map_or_else`; both lints are satisfied by a `let-else` binding
      that panics in its else block. `make lint` then passed with
      `docstring-coverage` at 100.0%, and the `clap_error_translations` suite
      passed 21 of 21.
- [x] (2026-11-15) Hosted workflow-contract repair. `build-test (linux)` on
      `4cbce31f` failed its "Workflow contract tests" step:
      `child_cargo_tier_test.py` reads child-cargo binaries from the Rust
      sources and found `clap_error_cli` and `clap_error_coverage` each on the
      profile's 600 s base allowance. Both now take the class's 960 s entry in
      `.config/nextest.toml`. They stay out of the `orthohelp_cache` group
      because neither clears `target/orthohelp` nor builds against
      `target/documentation-examples/v2`, so the developers' guide records the
      class as wider than the group instead of claiming the two coincide.
      Lesson for the gate brief: `make test-workflow-contracts` is a
      `build-test` step that no target in the seven-gate certificate ran, so a
      green local `make test` could not see this class of failure.
- [x] (2026-11-15) Rebase onto `f086edc9` and the second ADR renumber. The
      `0dffb908` certificate went stale for hosting reasons rather than a
      defect: PR #615 merged as `f086edc9` after that head was published, the
      PR turned `mergeable=false`, and GitHub creates no `pull_request` run for
      a conflicting PR, so the "Workflow contract tests" step could not be
      confirmed on the branch head. The 15 commits were replayed onto
      `f086edc9`; the only conflict was `docs/contents.md`, where main's new
      ADR-0020 (lock evidence newtypes) and ADR-0021 (test seams) entries
      collided with this branch's ADR-0020. Both sides were kept, and the gate
      ADR then moved to `adr-0022-pre-publication-clap-coverage-gate.md` with
      its heading and every inbound reference renumbered in one commit — the
      design, `docs/contents.md`, the developers' guide, the `CHANGELOG`, the
      `ortho_config` manifest comment, the gate and fixture module docs, and
      this plan. The lock-evidence references in `evidence_trybuild.rs` and
      ADR-0021 keep 0020, so the sweep was by exact filename and by the
      documents that name the gate, not by number alone.
- [x] (2026-11-15) Boundary flake classified; the hosted Lint red attributed
      to a base-layer rule change. The first eight-gate certificate run on
      `33d3a26f` was red on `make boundary` only: the allowance probe
      `an_allowance_spent_while_waiting_is_reported` exhausted its 600 s
      in-process `NESTED_CARGO` acquisition while the serialized suite ran
      678.47 s from a cold fixture. An isolated re-run of that gate on the same
      head, with nothing else running, passed in 21.64 s of test time (44
      passed, 0 failed, 10 ignored), so the failure is cold-host contention
      rather than a defect in the candidate. The remaining red is hosted and is
      not this branch's: the `build-test (linux)` Lint step reports 11
      `bumpy-road-function` errors in `ortho_config_macros/src/derive/parse/*`,
      a rule introduced by Whitaker's rolling suite at `880939d1`, published
      04:14:59Z on 2026-11-15. A dispatched CI run on `main` at `f086edc9`
      reports the same step failing, as do unrelated PRs #694 and #592, so no
      branch change can clear it; the local Whitaker install predates the rule,
      and the wrapper refuses to pin a suite that is published as a rolling
      release. Recorded as a blocker with the upstream owner rather than
      silenced by threshold tuning or an `allow`.
- [x] (2026-11-15) Whole-plan assessment answered: three in-scope findings
      repaired, two documentation items actioned, one item rebutted. The
      assessment of `f5e23086` (Assessment-ID
      `EP-11-2-1-whole-plan-f5e23086-20261115T0544Z`) returned "not yet
      complete".
      - F1, the source inventory counted an `ErrorKind` that a file-level
        `#![cfg]` or a conditional `mod` hop could exclude from the build. The
        gate now walks plain `mod` declarations from `lib.rs`, refuses a
        declaring file it cannot certify as unconditionally compiled, and
        treats `#[cfg_attr(cond, ..)]` as conditional only when it applies a
        nested `cfg`, so clap_builder's `#![cfg_attr(docsrs, feature(doc_cfg))]`
        still counts. Six committed `module_tree` fixtures prove the counted
        chain, the conditional hop, the orphan declaration, the file-level
        gate, the inline hop, and a tree with no crate root.
      - F2, the rendering suite neither asserted that each reproducer returned
        the kind under test nor pinned the display-only rows, and the real
        help, version, and missing-input paths had no zero-lookup witness of
        their own. `parse_error` now asserts the kind, the snapshot carries all
        17 matrix rows in review order, and a case table drives the three
        display-only kinds through real parses, asserting kind, unchanged text,
        exit status, stream selection, context, and zero catalogue lookups
        against a conspicuous localizer.
      - F3, `clap_error_cli` used a process-local `OnceLock` while nextest runs
        each case in its own process, and its 240 s child deadline left the
        wait charged to nobody. Fixture builds now take an advisory `fs4` lock
        opened beside the test binary, the one path every process of a run
        agrees on, and a 600 s budget covers both the wait and the build inside
        nextest's 960 s allowance. A unit case proves a held lock blocks a
        second acquisition.
      - Documentation: the v0.11.0 migration guide gains §8 and an impact row
        for overrides of `clap-error-missing-subcommand`, whose missing-input
        effect is gone, and the developers' guide records the module-chain
        certification and its two new refusals.
      - Rebutted: the "no committed `trybuild` UI coverage" item. V1 and V2 ask
        for observed and recorded negative controls — a 16-row matrix and a
        duplicated display row — both of which the M2 record holds; neither
        milestone asks for a committed `trybuild` fixture.
      - Gate repairs on the repair commit itself. The first certificate on
        `a391e752` was red on `make lint` with four denied Clippy findings in
        the new test-support modules — two `shadow_unrelated` closures,
        `excessive_nesting` on the module-file loop, and `or_fun_call`. All
        four were corrected in place rather than allowed. The amended head
        `d37d1d15` then certified Clippy, rustdoc, and `make test`, but
        Whitaker's `no_std_fs_operations` rejected `std::fs::File` in the
        fixture build lock: cap-std exposes no locking API and `fs4` locks a
        `std::fs::File` and nothing else. The lock moved into its own
        `clap_error_cli::fixture_build_lock` module, exempted through a narrow
        `excluded_paths` entry with its rationale, keeping every other file in
        the suite under the capability policy.
- [x] (2026-11-15) Second whole-plan assessment (Assessment-ID
      `EP-11-2-1-whole-plan-2e858de4-20261115T0818Z`, head `2e858de4`)
      returned "not yet complete" with two findings; it accepted the earlier
      `trybuild` rebuttal. Both findings are repaired.
      - F1, the module walk certified two shapes it could not establish. A
        condition applied through a nested `cfg_attr` — which the compiler
        expands recursively — was read as unconditional, and a `#[path]`
        declaration was resolved by default file names, so a file the build
        excludes could be counted and the file the build really loads could be
        missed. The walk now searches a `cfg_attr` recursively for a `cfg`, and
        refuses a `mod` declaration carrying a `#[path]` instead of guessing
        which file the compiler loads. `module_tree/cfg_attr_nested` and
        `module_tree/path_override` are the durable controls. Both were proved
        non-vacuous: with the two behaviours temporarily reverted, each control
        failed with an empty refusal, and the file was restored to a `cmp`-
        identical copy before any further work.
      - F2, M2's durable matrix-drift fixtures were recorded mutations rather
        than committed data. `tests/fixtures/clap_matrix/drifted_rows.rs` now
        holds a sixteen-row table and a seventeen-row duplicate-plus-omission
        table, independent of `CLAP_ERROR_IDS`. The four matrix invariants were
        factored into row-taking functions, so the live matrix and both
        fixtures run the same checks; the duplicate table passes the length
        check and only the uniqueness check catches it.
      - Documentation: the v0.11.0 migration guide gains §9 and an impact row
        for `CLAP_ERROR_IDS` and `ClapErrorTranslation`; the developers' guide
        gains a subsection on the CLI suite's fixture build and lock, plus the
        two new refusals in its troubleshooting table; the repository layout
        lists the `clap_error_matrix` and `clap_error_cli` targets.
      - Compile-time/UI check: `tests/ui/clap_error_translation_non_exhaustive.rs`
        pins the crate's own `#[non_exhaustive]` contract — a downstream match
        without a wildcard must fail with E0004 — so the check is satisfied by
        a contract this change introduces, not by compiling unrelated code.

## Surprises & discoveries

The plan first incorporated merged PR #651 at
`ddba3578fae04227e426ea3a61aa2b24aa043232`. On 2026-11-09 it was restacked onto
the timeout-fix branch for PR #664 at
`501ab83134eaba0f9e97e1daeaa29c793a819c53`. The M0 probe observed `clap` and
`clap_builder` 4.6.7 in the workspace. The design's 4.6.0 identity is
illustrative, not the observed workspace resolution. M0 found that consumer
build scripts cannot bind to downstream resolution; the selected gate records
and checks only the explicitly supported workspace identity. The upstream API
inventory observed during M0 had 17 variants; the implementation must recount
the source resolved at gate time and compare it with a reviewed constant.

M0 (2026-11-09) falsified that binding. A dependency's build script cannot
observe its dependent's dependency resolution. The only consumption form where
the walk-up from `CARGO_MANIFEST_DIR` finds the governing lock is our own
workspace build; an external path consumer and an unpacked packaged consumer
both resolve to a lock that does not govern the build. The packaged case is
worse than a miss: Cargo 1.84 and later place a publish-time `Cargo.lock` next
to the packaged manifest, so the naive rule silently binds a file that
describes the publish-time resolution, including dev-dependencies the consumer
never compiles. Details and citations are in "M0 outcome".

The requested nine additions plus four existing entries total thirteen.
`InvalidSubcommand`, explicitly named by §6.1, is also required: ten new
translations produce fourteen translated entries and three sentinels.

The full workspace test run found one consumer of the superseded mapping.
`examples/hello_world` asserted that a missing-input parse error carried the
consumer's `clap-error-missing-subcommand` copy. Under the sentinel contract
that error is display-only, so the example now demonstrates a consumer override
on `clap-error-invalid-subcommand` (an unknown subcommand) instead. The
behaviour change is recorded in the users' guide and design §4.12.

The design's build-script outputs are removed by the selected mechanism. The
source inventory is read by a pre-publication test; because that runtime test
value cannot initialize a Rust const, keep a reviewed count constant in the
test source, compare the parsed source count to it, and use `const_assert_eq!`
only for matrix length versus the reviewed count.

PR #617 completed the reusable `ortho_l10n` extraction; PR #651 added pinned,
opt-in Kani/Verus tooling. The Fluent runtime and embedded catalogues now live
in `ortho_l10n`; the Clap adapter, exhaustive matrix, and Clap-specific
pre-publication test gate remain in `ortho_config`. The prover tooling's
integration fixtures validate the runner and its negative controls, not this
feature's production claims. This finite enum/catalogue mapping and bounded
source counter do not warrant a Verus proof; tests and explicit inventory
review are proportionate.

`DisplayHelpOnMissingArgumentOrSubcommand` currently maps to the missing
subcommand message. The new sentinel contract requires pass-through instead.
The current formatter reconstructs translated errors and preserves only a
rendered tail. Rich structured context preservation remains task 11.2.2.

A full local `make test` run can fail on the `ortho_l10n` boundary contract
without any defect in this change:
`consumer::workspace_facade_consumer_builds_and_tests` builds a cold
138-package consumer under the fixed 440 s `CARGO_DEADLINE` in
`ortho_l10n/tests/boundary/fixture_cargo/environment.rs`. On a saturated
compile-admission pool that budget expires while third-party crates are still
compiling, and the runner kills the tree. The discriminator is that the failure
reports a deadline with no rustc diagnostic.
`target/localization-boundary/cargo-target` is shared by every fixture, so
`make boundary` warms it and the next `make test` finishes the same test in
about 90 s. Treat a first-run deadline as environmental, warm the fixture
target, and re-run the gate instead of changing the deadline or the test.

M1 (2026-11-10) confirmed five implementation facts. clap's stock `NoEquals`
text is "equal sign is needed when assigning values to '--colour'", not the
"required" wording the catalogue uses, so the fallback control asserts "needed".
`DisplayHelpOnMissingArgumentOrSubcommand` exits with status 2 on stderr, while
`DisplayHelp` and `DisplayVersion` exit 0 on stdout; the CLI fixture pins all
three. Real clap parse errors carry every variable the new strings use; only
`Io`, `Format`, and a raw `InvalidUtf8` are context-free. The new BDD steps
needed a second module so both step files stay within the 400-line cap. The CLI
fixture is a workspace member because `tests/*/main.rs` auto-discovery does not
descend into `tests/fixtures/*/src/main.rs`, and the runner builds it once so
targeted runs work.

Negative controls were exercised and reverted. Mapping the three display-only
kinds back to a translated identifier failed all three display unit cases and
the missing-input BDD scenario, reporting that the catalogue was consulted
once. Deleting the `clap-error-no-equals` message failed the catalogue-presence
test, the real-parse `NoEquals` case, and the rendered snapshot. After both
edits were reverted, every affected suite returned green.

M2 (2026-11-14) settled where the duplicate-plus-omission control belongs. The
`rstest` case table *is* the enumeration of supported kinds, so a duplicated
case line silently drops the kind it replaced. The control therefore applies to
`CLAP_ERROR_IDS`, the inventory the suite consumes: replacing the
`DisplayVersion` row with a second `DisplayHelp` row keeps the length at 17 and
fails three independent checks. A duplicated case line remains undetectable by
construction; the production matrix, the reviewed constant, and the publication
gate are what bound the enumeration.

The gate uses `cargo metadata --locked --offline` with the workspace's default
feature resolution and every target platform. `clap` and `clap_builder` are
unconditional dependencies, so a wider graph cannot hide the facade edge, and a
`cfg`-conditional declaration in the resolved source fails closed rather than
narrowing the count.

`cargo fmt` here is rustfmt's own `cargo-fmt` 1.10.0, which rejects
`--workspace`; `make check-fmt` uses `--all`, so use `--all` in ad-hoc runs.

M3 (2026-11-15) could not use `mapsplice` for the roadmap completion. Both
`mapsplice replace docs/roadmap.md 11.2.1 <fragment>` and
`mapsplice replace docs/roadmap.md 11.2 <fragment>` fail closed with
`dependency anchor '11.2.1' was not found in the target roadmap`
(`error_class=dangling_dependency`), and an identity replacement of the
unchanged step 11.2 reproduces it. The validator resolves `Requires` references
against the roadmap with the addressed anchor removed and before the fragment
is spliced back, so any task another item depends on cannot be replaced. Task
11.2.3 is also required by a later step (line 819), which is why the step-level
replacement reported it. The roadmap item therefore received the minimal hand
edit the library would have produced: no numbering changes, three checkboxes
flipped, and two nested notes added, followed by the Markdown formatter and
gates. Recorded here rather than treating an infrastructure refusal as a pass.

Rebasing onto main on 2026-11-15 showed that an ADR number is a shared resource
across the whole estate, not a branch-local choice. ADR-0018 was free when M0.1
recorded this branch's gate decision, and main had taken 0018 and 0019 by the
time the branch returned. The replay had to renumber the ADR to 0020 inside the
commits that introduce it, not in a trailing fixup, or the history would carry
a commit that links to a file it does not create.

The local review's most severe finding was false: it reported that the six
publication `Cargo.lock` fixtures were untracked, while `git ls-files` lists
all six and `git check-ignore` matches none. Two further findings prescribed
`test_helpers::process`, a module that exists on main but not at the reviewed
commit, which is exactly why the rebase had to land before those findings could
be judged. The lesson is to check a review's premises against the commit it
names before either repairing or dismissing it.

The fixture locator was wrong in a way the passing gate could not show. The
suite derived the fixture path from the test executable's own directory, but
`cargo test` for this package places that executable under
`target/debug/build/ortho_config/<hash>/out/`, while the nested `cargo build`
writes the fixture to `target/debug/`. It passed only because the stale path
happened to resolve in earlier runs. Reading the executable back from Cargo's
JSON artefact stream removes the assumption entirely.

## Decision log

- 2026-10-14: propose the design's build-script mechanism, with M0 as an
  explicit feasibility decision. M0 later falsified its downstream identity
  assumption; see the outcome below.
- 2026-11-08: require identity and facade-edge evidence; the chosen mechanism
  narrows that guarantee to the supported workspace publication gate.
- 2026-11-09: M0 completed with a no-go for the original build-time binding.
  At the user's request, this revision selects the pre-publication workspace
  gate, preserving the public total-count requirement while removing
  unsupported consumer claims.
- 2026-11-09: amend design §6.1.1 and record the mechanism in an ADR before
  runtime implementation. Reconcile roadmap wording. The present change records
  the plan decision only.
- 2026-11-09: put metadata/source-count checks in a dedicated test-only
  publication gate. Keep the reviewed count constant in test source; compare
  the parsed count to it at test time, and assert matrix length against it at
  compile time. No build-script fixture or generated build output is needed.
- 2026-11-09: M0.1 complete. Design §6.1.1 is retitled "Pre-publication
  coverage gate" and states the workspace-only limit; §6.1 and §12 point at it;
  ADR-0022 records the decision and the rejected alternatives; roadmap 11.2.1
  and the developers' guide no longer claim a build script or a consumer
  guarantee; `docs/contents.md` indexes the ADR.
- 2026-10-14: include `InvalidSubcommand` and all three sentinels, following the
  complete-matrix requirement and the design rather than an incomplete list.
- 2026-10-14: use finite exhaustive tests for the matrix and property tests for
  source-count invariance. No Verus milestone is warranted for this bounded
  catalogue lookup and integration plumbing; reasoning and its limits follow.
- 2026-10-14: add no compatibility wrapper. Existing identifiers remain stable
  as required by the localization contract; the crate is pre-1.0.
- 2026-11-10: place the CLI fixture at repository-root
  `tests/fixtures/clap_error_matrix` beside the existing fixture members rather
  than inside `ortho_config/tests/fixtures/`, so it is not nested in a
  published package. The runner invokes `cargo build -p` once so a targeted
  suite run does not depend on `cargo test --workspace` having built it.
- 2026-11-15: run both nested Cargo invocations in this work — the fixture
  build and `cargo metadata` — through
  `test_helpers::process::run_with_deadline`, with explicit deadlines. The
  shared runner already owns process-tree cleanup and stream draining, and
  retains each stream's complete output, so the gate does not carry a second
  copy of that logic. Its deadline bounds time, not captured bytes.
- 2026-11-15: resolve the fixture executable from Cargo's JSON artefact stream
  rather than from the test executable's own directory. The nested build's
  profile and target directory are its own, so a derived path is an assumption
  the test cannot check.

## Outcomes & retrospective

M3 closed the implementation on 2026-11-15. The users' guide enumerates the
matrix, names the override identifiers, and documents the display-only and
missing-input behaviour; the developers' guide owns the gate's troubleshooting
procedure and its variant-intake steps; the roadmap records 11.2.1 as done with
its finding and validation notes. A seven-gate certificate on frozen tree
`529721bb` is green, and it includes the two documentation-example targets that
execute the new example instead of only listing it. The only remaining work is
delivery: publish the branch, refresh the draft pull request, and take it
through review. Acceptance criteria E1–E4 are met and evidenced in Progress;
none is outstanding.

The plan incorporates the merged prover tooling from PR #651, and the nested
Cargo timeout fix that blocked validation on main has since landed from
[PR #664](https://github.com/leynos/ortho-config/pull/664) as squash
`eaf4c028`. Draft PR #588 is therefore based on `main` directly, with the
timeout fix inherited rather than carried. Formatting, lint, typecheck, and
full tests passed on the rebased candidate. Documentation and workflow-contract
checks also passed.

The earlier plan was approved on 2026-11-09, and M0 ran immediately. M0 ended
with a no-go for the original build-time identity binding: the evidence showed
that a dependency build script cannot reliably establish an external consumer's
governing Clap resolution. At the user's request, this revision selects the
recommended pre-publication workspace gate. The governing design and roadmap
wording still need reconciliation, and the mechanism decision must be recorded
in an ADR before implementation. No feature code exists yet.

Earlier validation on main hit inherited 240-second packaging and 300-second
consumer deadlines. On the initial PR #664 parent (`26275a4e`), the ordinary
formatting, typecheck, lint, Markdown, diagram, and workflow-contract gates
passed. Packaging preparation with verification enabled and matching Makefile
flags allowed `make boundary` to pass 32 tests, with none failed and one
ignored. The subsequent full `make test` still failed two workspace consumers
at the 440-second deadline: 30 boundary tests passed and one was ignored.
Retained output showed dependency compilation and compiler-admission waits.
Neither an earlier exit-143 run nor that focused pass constitutes a full
test-gate pass.

The parent subsequently added whole-process-tree cleanup, bounded reader
collection, and an operating-system lease for the identifier fixture target.
Replay of the plan commit onto `501ab831` was conflict-free and unchanged under
`git range-diff`. Those implementation changes belong to PR #664. The current
candidate preserves them intact. Fresh `make fmt`, `make check-fmt`,
`make lint`, `make typecheck`, and `make test` passed. The full test run
included 35 passing boundary tests, none failed, and seven ignored; all three
workspace consumers passed. Python tests reported 119 passed and 16 skipped. No
deadlines, assertions, compiler-admission settings, or shared Cargo-cache
policy were changed by this plan revision. `make markdownlint`, `make nixie`,
and `git diff --check` passed; `make test-workflow-contracts` passed 394 tests
with two skipped. No validation exception is needed.

### Temporary PR base and return to main

Target draft PR #588 at `issue-644-nested-cargo-timeouts` until PR #664 merges.
The inherited parent head is `501ab83134eaba0f9e97e1daeaa29c793a819c53`;
preserve that replay boundary when publishing this stack. Only the plan commit
and its revisions belong to this branch above that boundary. Fetch the parent
and main again before implementation; any further restack requires validation
of the resulting candidate.

After PR #664 merges, the implementing agent can rebase this branch onto a
freshly fetched `origin/main` and retarget the PR to `main`. First verify the
parent's merge and landing, record the current remote head for an explicit
force-with-lease, and preserve recovery refs. Use the `rebase` skill to replay
only this branch's commits above the inherited parent boundary; do not replay
the timeout fix again if GitHub squash-merges it. Review any conflicts, retain
the parent fix, run the ordinary gates, then publish with the recorded lease.
If the parent is rewritten before merging, reconcile its old and new history
before choosing a replacement boundary. Returning to main does not grant
implementation approval.

This returned to main on 2026-11-15. PR #664 landed as squash `eaf4c028`, with
the follow-up review commit `9ede81a1` on top; `origin/main` stood at
`959dbd7e`. The eight branch commits were replayed onto that head with
`git rebase --onto origin/main 501ab831`, dropping the four timeout-fix commits
as already squashed. Two conflicts arose: this branch's ADR entry in
`docs/contents.md`, because main had taken ADR-0018 and ADR-0019, and the
ExecPlan status line. The ADR was renumbered to 0020 in every replayed commit
with a per-commit sweep, and the status line took the milestone's own wording.
The pre-rebase head is retained at `refs/recovery/11-2-1-prerebase`
(`ee3eed22`) for comparison. PR #588 already targets `main`, so no retarget was
needed beyond that.

M1 (2026-11-10) delivered the public matrix (`ortho_config::CLAP_ERROR_IDS` and
`ClapErrorTranslation`, exported through both the crate root and
`ortho_config::localizer`), the ten new en-US strings, and the runtime
consultation that skips the three display-only kinds. Focused suites pass: 22
matrix tests, 21 catalogue and real-parse tests, 17 localizer unit tests, 12
localized-parse tests, the whole BDD target with three new scenarios, and five
subprocess CLI cases covering help, version, missing-input help, a translated
parser error, and the unavailable-locale fallback. Full commit gates are
pending before this milestone commits. M2 remains: the reviewed-count constant,
the pre-publication inventory gate, and its drift controls.

## Context and orientation

`ortho_config/src/localizer/clap_error.rs` owns `message_id`,
`localization_args`, and `localize_clap_error_with_command`. It currently
synthesizes identifiers from enum debug names, with exceptions for the four
shipped messages. `ortho_l10n/locales/en-US/messages.ftl` is the embedded
English catalogue, and `ortho_l10n` owns Fluent formatting, catalogue
embedding, and shared localization types. `ortho_config/src/localizer/mod.rs`
is the compatibility facade and owns the Clap adapter;
`ortho_config/src/lib.rs` exposes the crate-level API. The standalone
`ortho_l10n` runtime must not gain a Clap dependency.

`ortho_config/src/localizer/tests.rs` already tests Clap fallback, captured
context, and rendered tails; Fluent runtime tests live in
`ortho_l10n/src/tests.rs`. Extend coverage in separate focused modules rather
than growing a file beyond the repository limit. Behavioural tests use
`ortho_config/tests/features/localizer.feature`, steps in
`ortho_config/tests/rstest_bdd_support/behaviour/steps/localizer_steps.rs`, and
the `ortho_config/tests/rstest_bdd.rs` target. Check scenario registration when
adding cases. There is no current `ortho_config/build.rs`. Existing Cargo
metadata parsing helpers under
`ortho_l10n/tests/boundary/fixture_cargo/metadata.rs` belong to boundary-test
fixtures; inspect their scope before reusing or extracting them.

Fluent is the message format; a catalogue identifier names a message, and its
variables receive clap context. A sentinel is an enum value indicating that
clap itself owns display. Cargo metadata describes resolved package identities
and dependency edges; a package name alone is not an identity. `fluent-syntax`
is owned by `ortho_l10n`; keep syntax and formatting assertions there unless a
specific `ortho_config` integration test justifies an explicit dev dependency.

## Conformance basis

This plan is based on the roadmap and design files at the baseline commit
above. No separate Terms of Reference governs this task. Roadmap 11.1.1 is
checked complete and its promoted command-localization API exists. Recheck that
prerequisite and the live source after approval or any rebase.

Trace the three requirements as follows:

- R1, design §6.1 and roadmap 11.2.1 strings: M1, fourteen catalogue-rendering
  cases and CLI tests; M3, consumer documentation.
- R2, design §6.2 exhaustive public matrix: M1, independent inventory,
  uniqueness, public-import, override, and pass-through tests.
- R3, design §6.1.1 total-count mechanism, as amended by
  [ADR-0022](../adrs/adr-0022-pre-publication-clap-coverage-gate.md): M0
  evidence and selected deviation; M0.1, design/ADR/roadmap reconciliation; M2,
  workspace identity gate, source count comparison, matrix const assertion, and
  deliberate drift controls.

Read these references before implementation; they own the surrounding rules:

- [Core design](../design.md) and
  [CLI localization design](../cli-localization-design.md): localizer
  ownership, fallback semantics, matrix, and the authoritative gate.
- [cargo-orthohelp design](../cargo-orthohelp-design.md),
  [clap-dispatch integration][dispatch],
  [agent-native CLI design](../agent-native-cli-design.md), and
  [improved errors](../improved-error-message-design.md): adjacent consumers
  and contracts; this task does not redesign their schemas or dispatch.
- [rstest fixtures](../rust-testing-with-rstest-fixtures.md),
  [doctest guidance](../rust-doctest-dry-guide.md),
  [dependency injection][injection], and
  [rstest-bdd guide](../rstest-bdd-users-guide.md): fixture ownership,
  executable API examples, injected inputs, and scenario registration. The
  requested `rtest-bdd-users-guide.md` spelling has no file; this is its actual
  repository path.
- [Fluent libraries](../localizable-rust-libraries-with-fluent.md),
  [complexity guidance][complexity], for fallback and small modules; the
  [scripting standards](../scripting-standards.md) govern repository scripts.
- [ADR 0015](../adrs/adr-0015-localization-runtime-boundary.md) and the
  [users' guide](../users-guide.md) describe the extracted runtime boundary;
  keep Clap-specific APIs in `ortho_config`.
- [ADR 0022](../adrs/adr-0022-pre-publication-clap-coverage-gate.md) owns the
  pre-publication coverage gate and supersedes the build-time identity
  mechanism design §6.1.1 previously proposed.
- [ADR 0017](../adrs/adr-0017-consumer-side-prover-tooling.md), the
  [Kani/Verus tooling plan](import-prover-tools.md), and the
  [developers' guide](../developers-guide.md#kani-and-verus-verification-tools)
  describe optional proof tooling. Ordinary gates do not install or run provers.
  `FORMAL-EMPTY` is a non-success result, and tooling fixtures are not feature
  proofs. If later evidence warrants production verification, register
  production sources, their proof/refinement relationship, inputs and bounds,
  assumptions, expected check, and a negative control; run the exact registered
  proof gate. Do not use the import plan's fixtures as proof of this feature.
- [Documentation style](../documentation-style-guide.md),
  [contents](../contents.md), and [layout](../repository-layout.md): document
  placement and test target registration.

Use the `execplans`, `codegraph-mcp`, and `rust-router` skills for planning and
Rust design; follow the router to `rust-unit-testing`, `rust-verification`, and
`proptest` for the relevant verification work. Use `firecrawl-mcp` for
unresolved external contracts, `logisphere-experts` for design review,
`commit-message` for commits, and `pr-creation` for publication. Reuse existing
metadata helpers only if their boundary-test ownership permits it; do not
introduce a dependency from the standalone localization runtime to Clap. Before
any helper extraction, search for an equivalent and document ownership and
permitted reuse in the developers' guide. Publication-gate helpers belong only
to test support and the gate.

## Verification plan

### Model, preconditions, and representation

Let I be the explicitly supported `clap_builder` identity, recorded by package
version, registry/source identity, and Cargo.lock checksum. Let E(I) be the set
of top-level `ErrorKind` variants parsed from the `manifest_path` resolved for
that identity by workspace `cargo metadata --locked --offline`. The gate also
checks that the selected production `clap` package and its normal dependency
edge resolve to I for the configured publication target and features. Unknown,
overridden, ambiguous, mismatched, or incompletely audited cfg-dependent
inventories have no accepted E(I) and fail the gate.

Let N(I) be the reviewed integer constant in the matrix test for I, and M the
public sequence of matrix pairs. Each pair has a real `ErrorKind` and either
`Translated(id)` or `DisplayOnly`; D is the three display variants from §6.1.
The publication test requires the freshly parsed source count to equal N(I).
Rust's `const_assert_eq!` separately requires `len(M) == N(I)`. The source
parser's runtime result cannot initialize a Rust const. Therefore a source
variant addition fails the publication test until reviewed; after updating
N(I), a missing or extra matrix row fails the compile-time assertion. The
assertion alone is not an upstream-source-growth detector.

The remaining postconditions are that each member of E(I) occurs exactly once
in M; exactly D receives the sentinel; every other row names a unique,
renderable embedded English message; runtime lookup agrees with M; and
display-only errors return unchanged without consulting the localizer. The
finite coverage argument is that distinct keys drawn from E(I), together with
`len(M) == N(I) == |E(I)|`, cover the inventory. Length equality alone lacks
the distinctness premise. This is a review argument discharged for the concrete
inventory, not a machine-checked theorem. No contractual business algorithm or
unbounded semantic kernel is introduced, so Verus, Kani, and solver scaffolding
would not improve the relevant assurance. Never use `assume`, `admit`, or
trusted bodies to bypass a proof.

Axioms are Cargo's locked workspace resolution and metadata contracts, the
selected registry package's source layout, clap's published type and context
contracts, Rust's typed enum constructors and const evaluation, and Fluent's
parser/formatter contract. Tests exercise our use of these boundaries; they do
not purport to prove those dependencies correct. The gate establishes only the
repository's configured publication graph, not downstream consumer graphs.

### V1: exact inventory and uniqueness

Add `ortho_config/tests/clap_error_matrix.rs` with an independently enumerated
`rstest` table for the exact supported `clap_builder` identity. Check pairwise
key uniqueness, identifier uniqueness, exact expected kind membership, and the
sentinel partition. Compare the table length with the reviewed identity count
using the compile-time assertion; the publication gate independently counts
source variants. Compile imports through both `ortho_config` and
`ortho_config::localizer`. Use the standard `assert_eq!` macros, following the
developers' guide's scoping of `googletest` and `pretty_assertions` to the
profile test modules rather than to new tests elsewhere.

The witnesses include every supported real kind and all three sentinels.
Replace a row with a duplicate while keeping length unchanged: the
uniqueness/inventory test must fail. Remove or add a row: the M2 const
assertion must fail. These controls prevent a shared production/test omission
from looking exhaustive.

### V2: messages, overrides, and missing context

Keep catalogue syntax and Fluent rendering assertions with `ortho_l10n`, which
owns `fluent-syntax` and the embedded resources. In
`ortho_config/tests/clap_error_translations.rs`, verify that every translated
matrix identifier has an actual embedded message rather than accepting fallback
as evidence; add an explicit `fluent-syntax` dev dependency only if this
cross-crate test needs to parse the catalogue itself. Render each identifier
with a real `FluentLocalizer` and representative arguments; require no
formatting issues and nonempty output. Exercise consumer overrides, en-US
fallback from an incomplete locale, and unchanged stock errors with
`NoOpLocalizer`.

Use real clap parsing to produce each reachable parser error and assert its
kind before testing the translation. Use explicit `Error::raw` constructions for
`Io` and `Format`, which need no argv parser reproducer. Test context-free
errors as well: never emit unresolved variables or lose the original diagnostic
on a missing-argument formatting failure. Keep required-context translations on
the existing fallback path when information is absent. Prefer context-free new
strings when clap cannot supply a stable variable contract. Inventory and
record each new string's variables before implementing it.

Use `insta` snapshots for the finite, colour-disabled rendered error matrix,
including the existing usage tail. Snapshot review must be paired with semantic
assertions for kind and selected identifier. Delete one FTL message and alter
one mapping as negative controls; direct catalogue and override tests must fail
even if fallback happens to look like valid English.

### V3: display-only and CLI integration

Use a recording localizer that would return conspicuous replacement text. For
all three display variants, assert zero lookups, unchanged text, kind, exit
code, stream selection, and context. Include both raw unit witnesses and real
clap help/version/missing-input paths. This exposes the existing bug in the
third variant instead of testing pass-through with a no-op localizer.

Extend `localizer.feature` with these scenarios and register their steps:

```gherkin
Feature: Complete clap error localization
  Scenario: A newly covered parser error is translated
    Given a localized command requiring equals for its option
    When I parse that option without equals
    Then the error kind is NoEquals
    And the en-US no-equals translation is rendered

  Scenario: Missing-input help remains clap output
    Given a localized command showing help when arguments are absent
    When I parse no arguments
    Then the error kind is DisplayHelpOnMissingArgumentOrSubcommand
    And the localizer is not called
    And the output and exit status equal stock clap

  Scenario: An unavailable override retains usable English
    Given an incomplete locale and the embedded en-US catalogue
    When I trigger a newly covered parser error
    Then the embedded en-US translation is rendered without formatting errors
```

Add a small subprocess fixture in `tests/fixtures/clap_error_matrix/` and a
runner `ortho_config/tests/clap_error_cli.rs` to exercise stdout, stderr, and
exit status for help, version, missing-input help, and a translated parser
error. Keep fixture compilation sequential and outside `/tmp`; inject locale
and colour settings through the child process rather than mutate global env.

### V4: publication identity, source inventory, and freshness

Implement the publication check as a test-only gate, not a build script. Read
the root workspace `Cargo.lock` and select the unique locked `clap` package
used by `ortho_config` as a normal production dependency for the configured
target and features. Verify its package version, source identity, and checksum
against the lock entry. Follow the resolved `clap` → `clap_builder` normal
dependency edge and require one unambiguous `clap_builder` package identity.
Cross-check both package IDs and the edge with workspace
`cargo metadata --locked --offline` for that same target/features selection.
Reject lock/metadata disagreement, source override, multiple candidates,
missing edges, unexpected dependency kinds, or unsupported identities. For the
supported registry identity, use metadata's `manifest_path` to locate the exact
source defining `ErrorKind`; do not infer the path from registry directory
names.

Parse the enum with `syn` and count all top-level variants without assigning
meaning to any of them. Audit attributes and cfg conditions for the supported
source. If the inventory's compiled shape cannot be established exhaustively,
fail closed and require an explicit source-support decision. Use `proptest` on
the implemented parser: adding a variant adds one, while declaration order,
comments, and discriminants do not change the count; malformed input fails, and
unsupported conditional declarations fail closed. Record generator acceptance
and classification so every relevant case is exercised. Do not filter away
declaration forms relevant to the supported source.

Keep `CLAP_ERROR_KIND_COUNT` as a reviewed test constant associated with the
allow-listed package identity. The gate independently requires the parsed
source count to equal that constant. In the matrix test, compile
`static_assertions::const_assert_eq!(CLAP_ERROR_IDS.len(),
CLAP_ERROR_KIND_COUNT)`.
Thus source growth fails the publication gate until reviewed; after the
constant is updated, a matrix omission or extra row fails the const assertion.
The assertion does not parse source or detect upstream growth by itself.

Negative controls must prove the checks are live: alter the supported identity
checksum/source or facade edge and require identity validation to fail; supply
an unknown identity and require a fail-closed diagnostic; alter only the parsed
source count and require the publication test to fail against the reviewed
constant; update the constant without the matrix and require compile failure;
and introduce a duplicate plus omission with unchanged length and require the
semantic inventory test to fail. Restore all controls before the final gates.
The gate validates this repository's publication graph only; consumer
end-to-end tests continue to verify observable behaviour, not a downstream
dependency-identity guarantee.

## Plan of work and milestones

### M0: establish the original identity-manifest feasibility

M0 tested the proposed build-script identity resolver against workspace,
external path, and unpacked packaged consumption. Preserve its evidence and
historical probe commands below. The experiment found the build script could
not establish the external consumer's governing lock and compiled facade edge.
M0 therefore ended the original mechanism at its stop condition. It did not
implement a fallback or feature code. Its acceptance result is a documented
no-go for the original design, not a go for the feature.

#### M0 outcome (2026-11-09): original mechanism stopped; evidence retained

The build-time identity contract holds for our own workspace build and for no
other consumption form. A dependency's build script cannot observe its
dependent's dependency resolution, and the Cargo team treats the reverse
direction as out of scope: "we do not want to allow side-banned communication
between dependencies and dependents"
([rust-lang/cargo#3946](https://github.com/rust-lang/cargo/issues/3946),
2025-08-26). There is no `CARGO_WORKSPACE_DIR`.

Evidence came from a temporary probe build script, since removed, and from
disposable consumers under `target/m0/`:

| Consumption form           | Governing lock reachable | Observed                                                                                                                                                                                                                                                             |
| -------------------------- | ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Workspace build            | Yes                      | `CARGO_MANIFEST_DIR=<worktree>/ortho_config`; the walk-up finds `<worktree>/Cargo.lock`, which does govern the build.                                                                                                                                                |
| External path consumer     | No                       | `CARGO_MANIFEST_DIR` still resolves inside our checkout, so the walk-up binds *our* workspace lock, not the consumer's lock.                                                                                                                                         |
| Unpacked packaged consumer | No                       | An adjacent publish-time `Cargo.lock` exists (80,043 bytes, 332 packages, including dev-only `trybuild`, `proptest`, `cargo_metadata`, `process-wrap`, and `insta`), while the consumer's governing lock is a different file (32,629 bytes, none of those packages). |

Four routes to the compiled identity were each closed:

- `DEP_*` bridge: none. `clap` and `clap_builder` declare no `links`, and the
  probe observed no `DEP_` variable of any kind.
- Target-directory introspection: at build-script time the target directory
  names no `clap` package. The build-script unit's fingerprint carries
  `deps: []`; the unit that does list `clap` is written only after our build
  script runs, and records a numeric package-id hash with no version, source,
  or checksum.
- `OUT_DIR` ancestry: this does reach the consumer's target directory, and one
  level further reaches the consumer's own lock, but the layout is not
  contractual and breaks under `CARGO_TARGET_DIR`, `--target-dir`, and
  `build.target-dir`
  ([rust-lang/cargo#9661](https://github.com/rust-lang/cargo/issues/9661)).
  Tools that do this describe it as guessing, not resolving.
- `cargo metadata` from the build script: since Cargo 1.84 a library package
  ships its own lock, so the call honours that publish-time lock, can require
  the network, and can resolve versions the consumer never compiles
  ([rust-lang/cargo#15272](https://github.com/rust-lang/cargo/issues/15272)).

Renamed dependencies, feature selection, target-specific edges, multiple `clap`
versions, and source overrides were not exercised: the decisive failure occurs
earlier, because no governing lock is reachable at all, so those sub-cases
cannot change the verdict.

This satisfied the original tolerance trigger, "the active dependency source
cannot be identified without guessing", and ruled out the original build-time
mechanism. The revised decision accepts a workspace publication gate with an
explicitly limited guarantee. No feature code was written.

**Upstream design impact.** Design §6.1.1 requires a build-time check binding
the count to the resolved `clap_builder` identity and to the compiled `clap`
facade edge. That requirement rests on an assumption Cargo does not support for
consumers, so §6.1.1 needs amendment whichever option is accepted. The public
surface in §6.2, the enum and the constant, is unaffected.

**Decision and rejected options.** At the user's request, this revision selects
the recommended option: move identity binding to the pre-publication workspace
test gate. Read the workspace `Cargo.lock` for package identity and edge;
cross-check with `cargo metadata --locked --offline` (M0 observed exit 0,
`clap` 4.6.7, `clap_builder` 4.6.7, the facade edge, and a `manifest_path` into
the resolved source); count source variants with `syn`; and compare with a
reviewed identity-specific test constant. Keep `const_assert_eq!` for matrix
length versus that constant. This retains total-count coverage and fail-closed
publication checks without claiming downstream consumer rejection. It removes
`build.rs`, `rustc-env`, `OUT_DIR` count output, and build-time identity
resolution.

The following alternatives remain rejected:

1. Emit nothing outside a workspace build. This creates the consumer escape
   hatch the original tolerance forbade and leaves downstream checking vacuous.
2. Ask clap upstream for a version constant or a `links` key. Neither is
   available in the observed API and this task cannot add it.
3. Publish a bridge crate declaring `links`. Its build script is equally blind
   to its dependent's resolution, so the value would be a hard-coded literal.

Recipes, kept out of the product tree:

```bash
# Workspace build: add a probe build script, then
cargo build -p ortho_config
# and read target/debug/build/ortho_config-*/stderr.

# External path consumer: standalone `[workspace]` package under target/m0/
# with `ortho_config = { path = "<worktree>/ortho_config" }`, then
cargo build --offline --manifest-path target/m0/path-consumer/Cargo.toml

# Packaged consumer: package, extract outside the workspace, consume the
# unpacked archive (a checkout path dependency is not equivalent).
cargo package -p ortho_config --allow-dirty --no-verify
tar xzf target/package/ortho_config-0.10.0.crate -C target/m0/unpacked
cargo build --offline --manifest-path target/m0/pkg-consumer/Cargo.toml
```

#### Selected mechanism handoff

At the user's request, this revision selects the pre-publication gate. Before
feature code, perform M0.1: amend `docs/cli-localization-design.md` §6.1.1 and
record the substantive change in an ADR; reconcile any roadmap wording that
implies consumer-side failure; and update this ExecPlan's conformance links.
The selected mechanism limits identity assurance to the repository's configured
publication graph. It does not require a consumer to run our test gate.

### M1: canonical matrix and executable translations

First add failing public matrix, catalogue, sentinel, and override tests.
Record the expected red result, then define the enum and matrix in a focused
`ortho_config/src/localizer/clap_error_ids.rs` module and expose both public
paths. Add ten en-US strings, including `clap-error-invalid-subcommand`. The
other nine new identifiers use the existing kebab naming convention. Preserve
`clap-error-missing-argument`, `clap-error-unknown-argument`,
`clap-error-invalid-value`, and `clap-error-missing-subcommand` exactly.

Make `clap_error.rs` consult this matrix; sentinel rows return immediately.
Keep unknown-kind fallback and existing rendered-tail behaviour. Avoid the
rich-context redesign of 11.2.2. Expand context extraction only where a new
message requires information actually supplied by clap, with tests for absent
context. Add the BDD and CLI cases from V3 and snapshots from V2. Add API
documentation and consumer guidance alongside the behaviour change.

Acceptance E1 is all V1–V3 tests passing, red evidence retained in this plan,
all ordinary gates green, and unchanged existing-identifier override tests.
Review R1/R2 conformance and upstream assumptions before committing. The
publication inventory check remains for M2. Recovery is to fix forward or
revert this atomic milestone, without adding compatibility shims.

### M2: pre-publication inventory gate and compile-time matrix assertion

After completing M0.1, add a test-only `ortho_config` publication gate. The
implementation must read the workspace lock, identify the selected production
`clap` package and normal facade edge, and confirm both against
`cargo metadata --locked --offline` using the configured target/features. Check
version, source, and checksum against the locked supported identity. Follow the
metadata-resolved `clap` dependency edge to exactly one `clap_builder`, resolve
its `manifest_path`, and use `syn` to count every top-level `ErrorKind`
variant. Do not read consumer locks or pretend to reject consumer builds. Fail
closed before publishing for unknown identities, altered sources, checksum or
edge mismatches, ambiguous candidates, metadata disagreement, and unaudited cfg
shapes. Do not add `build.rs`, `rustc-env`, `OUT_DIR` artefacts, or generated
build outputs for this check.

Keep the supported identity and reviewed total in the test as explicit data.
The gate compares its parsed source count to the reviewed count at test time.
Use
`static_assertions::const_assert_eq!(CLAP_ERROR_IDS.len(),
CLAP_ERROR_KIND_COUNT)`
to check the matrix against that reviewed constant at compile time. The
test-time parsed value cannot initialize the Rust const. A new source variant
therefore fails the publication gate until reviewed; after the count is
intentionally updated, a missing or extra matrix row fails the const assertion.
Unknown identities fail our publication gate only; no consumer build rejection
is promised.

Add durable fixtures for supported and unsupported lock/source identities, the
`clap` facade edge, variant-count drift, and matrix drift. Require each
negative control to fail for its stated reason; restore all mutations before
running gates. Acceptance E2 requires the supported workspace graph to pass,
unknown or mismatched identities to fail closed, source-count drift to fail
test-time comparison, matrix length drift to fail the const assertion, and
semantic tests to reject duplicate-plus-omission errors. Include an explicit
rerun/freshness check if the gate caches parsed results; otherwise do not add
cache invalidation machinery. No support code classifies display-only variants.
Commit after a conformance review of revised R3.

### M3: reconcile documentation and complete delivery

Update `docs/users-guide.md` with matrix enumeration, override IDs, sentinel
handling, and the missing-input help change. Update `docs/developers-guide.md`
with publication-gate ownership, prerequisites, troubleshooting, and the
procedure when clap adds a variant. M0.1 must update §6.1.1 of the localization
design to state the supported workspace-only guarantee and its limits, and
record the deviation in an ADR linked from the design and contents. Use
`docs/design.md` for the component boundary where appropriate. Do not advertise
deferred APIs or consumer-side identity rejection.

Run all ordinary gates, documentation gates, focused integration tests, and new
executable doctests. Reconcile all R1–R3 evidence and upstream decisions. Only
when the feature is complete mark roadmap 11.2.1 `done` in the existing
checkbox convention. Use the `mapsplice` skill for that structural roadmap
edit. The planning PR must leave its checkbox open. Update this plan's status
and retrospective, then gate and commit the final milestone.

## Concrete steps and acceptance commands

Run from the repository root. Before each major milestone run the relevant
focused suites to establish the baseline. During its red stage run the newly
added test before production edits and record the expected failure. After
green, repeat the focused test, then the full gates sequentially. Do not
interpret a filtered run with zero tests as success.

```bash
cargo test -p ortho_config --test clap_error_matrix
cargo test -p ortho_config --test clap_error_translations
cargo test -p ortho_config --test clap_error_cli
cargo test -p ortho_config --test rstest_bdd
cargo test -p ortho_config --test clap_error_coverage --locked --offline
cargo test -p ortho_config --doc
```

New target commands become available in their named milestones. Their expected
result is a nonzero executed test count, all named cases passing, and no
ignored failures. Keep seed/failure records for property tests. Record the gate
fixture commands and exact expected diagnostics in M2 when the test target is
implemented; M0 historical probe commands are retained in the outcome above.

Use the canonical Make targets, not a reduced substitute. The live Makefile
runs rustdoc, Clippy, and Whitaker for lint; test includes all-feature Rust
coverage and Python tests. Capture every command's true status using pipefail:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-11-2-1.out
make test 2>&1 | tee /tmp/test-ortho-config-11-2-1.out
make typecheck 2>&1 | tee /tmp/typecheck-ortho-config-11-2-1.out
make lint 2>&1 | tee /tmp/lint-ortho-config-11-2-1.out
```

Run these as separate sequential invocations and stop on failure. After
Markdown edits run `make fmt`, inspect for unrelated formatter changes, then run
`make check-fmt`, `make markdownlint`, and `make nixie` with separate logs. Do
not hand-edit generated spelling configuration. Planning-only delivery requires
these documentation gates; Rust implementation gates apply to M0–M3 whenever
code is introduced and to every major implementation milestone.

## Interfaces and dependencies

The new public shape, also re-exported from the crate root, is:

```rust
#[non_exhaustive]
pub enum ClapErrorTranslation {
    Translated(&'static str),
    DisplayOnly,
}

pub const CLAP_ERROR_IDS: &[(clap::error::ErrorKind, ClapErrorTranslation)];
```

The declaration above specifies the constant's type; its actual definition must
supply every row. The publication test should use `cargo_metadata` to validate
the workspace production facade edge and `syn` to count variants from the
resolved package source. Both are test-only; neither participates in a build
script. Keep the reviewed supported identity/count beside the matrix test and
use `static_assertions` there for matrix-length comparison. Reuse workspace
versions compatible with Rust 1.89 and caret requirements. Existing dev
dependencies already include `rstest`, `rstest-bdd`, `googletest`,
`pretty_assertions`, `insta`, and `proptest`. No solver dependency is planned.

## Idempotence and recovery

Keep scratch fixtures under a task-specific `target/` directory and logs under
`/tmp`. Never modify the shared cached clap sources for negative controls; copy
fixtures and apply local patches there. Preserve users' unrelated edits. After
any interruption inspect Git status, fixture paths, logs, and this plan before
resuming. Remove deliberate mutations before final gates. Each passing
milestone is a rollback point; revert its commit rather than resetting shared
history. Do not rerun a failed gate until its cited log has been inspected and
the cause fixed or an infrastructure condition has demonstrably changed.

## Artefacts and notes

External contracts checked through Firecrawl on 2026-10-14:

- [clap_builder 4.6.7 ErrorKind](https://docs.rs/clap_builder/4.6.7/clap_builder/error/enum.ErrorKind.html):
  seventeen declared variants, with a non-exhaustive public enum.
- [Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html):
  package IDs, resolved edges, manifest paths, and locked/offline behaviour.
  `--no-deps` cannot provide the required dependency graph.
- [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html):
  historical M0 context for the rejected build-time mechanism. These contracts
  do not prove a dependency's consumer graph identity.

Wyvern evidence was exchanged using context packs `pk_nry25fav` and
`pk_n7jcfn3g`, plus documentation pack `pk_qmi5dgej`; this document contains
the durable decisions and needs no pack to execute. Expert review covered
Pandalump (architecture), Wafflecat (alternatives), Telefono (contracts),
Dinolump (maintenance), Buzzy Bee (build operations), and Doggylump (failure
handling). All actionable findings were incorporated: renamed dependencies and
target/feature cases; tracked fixture recipes; actual packaged consumer builds;
environment invalidation; actionable failures; and source-growth controls. The
2026-11-08 revision supersedes the original growth control with separate
unsupported-identity and supported-manifest-count controls. Implementation
feasibility remains subject to M0. Record validation receipts in the PR; keep
it draft and include its branch-wise plan link and the Lody session link in a
final References section.

Revision note (2026-10-14): initial plan records contracts before
implementation, adds the omitted variant, and makes exact-source feasibility an
explicit gate.

Revision note (2026-10-14): expert review strengthened M0 and M2 acceptance
evidence without widening the requested public API or changing the gate.

[dispatch]: ../clap-dispatch-and-ortho-config-integration.md
[injection]: ../reliable-testing-in-rust-via-dependency-injection.md
[complexity]: ../complexity-antipatterns-and-refactoring-strategies.md

Revision note (2026-11-08): reconcile PR #651 tooling and the current identity
manifest contract, preserve PR #617 runtime boundaries, and retain the explicit
implementation approval gate.

Revision note (2026-11-09): temporarily base the plan on PR #664 to inherit the
nested Cargo timeout fix; record the parent boundary and the implementing
agent's return-to-main procedure after that PR merges.

Revision note (2026-11-09): record M0's evidence that the approved consumer
build-time identity binding is unavailable, select the pre-publication
workspace gate at the user's request, and replace build-script milestones with
a test-time source inventory comparison plus a matrix-versus-reviewed-count
const assertion. The implementor must reconcile §6.1.1 and the roadmap and
record the choice in an ADR before feature implementation. The guarantee covers
the configured workspace publication graph only.
