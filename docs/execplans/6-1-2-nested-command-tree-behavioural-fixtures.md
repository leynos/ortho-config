# Cover nested command trees with behavioural fixtures

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS

This plan covers roadmap item 6.1.2 only. It builds on roadmap item 6.1.1
(see `docs/execplans/6-1-1-recursive-doc-metadata-subcommands-values.md`),
which introduced the `OrthoConfigSubcommandDocs` trait and derive and made
`DocMetadata.subcommands` populate recursively. This plan does not change the
documentation IR schema, the bridge pipeline, `cargo-orthohelp`'s CLI surface,
or the agent-context and policy-report schemas. It does not introduce any new
runtime dependency.

## Purpose / big picture

Phase 6 of the roadmap ("Deliver whole-CLI introspection", `docs/roadmap.md`
§6) cannot be considered complete on the strength of unit tests against the
`OrthoConfigDocs` derive alone. Roadmap item 6.1.2 closes the verification gap
by raising the floor of behavioural coverage so that consumers can rely on the
populated tree at every observable surface:

1. the `DocMetadata` tree returned by `<RootType as OrthoConfigDocs>::get_doc_metadata()`,
   read end-to-end (parent + children + grandchildren) including `fields`,
   `examples`, command names, and `windows` metadata where declared;
2. the recursive JSON IR emitted by `cargo orthohelp --format ir`;
3. the man pages emitted by `cargo orthohelp --format man` (both inline-COMMANDS
   and `--split-subcommands` shapes), proving the existing renderer continues
   to work with a non-empty tree;
4. the `PowerShell` wrapper module emitted by `cargo orthohelp --format ps`
   (both with and without `split_subcommands_into_functions`), proving the
   existing renderer continues to work with a non-empty tree, including
   `Windows`-metadata-driven aliases and function exports.

After this plan ships, a maintainer working on Phase 6.2 (compact
agent-context output) or Phase 7 (vocabulary policy) inherits a fixture and
a behavioural harness that asserts the shape of the tree the downstream
features consume. Without this work, every later phase has to re-prove the
same invariants from scratch and can mistake "no obvious regression" for
"correct".

Observable success is checked by:

- a new behavioural fixture CLI (an `rstest`/`rstest-bdd` fixture struct tree,
  living in `ortho_config/tests/rstest_bdd/scenario_state.rs` and re-exported
  through `ortho_config/tests/docs_ir_subcommands.rs`) containing at minimum
  three top-level subcommands: one leaf command with options but no
  subcommands, one leaf command with no options at all, and one command whose
  inner type itself has its own `#[command(subcommand)]` field;
- new `rstest` cases in `ortho_config/tests/docs_ir_subcommands.rs` that walk
  the tree and assert (a) recursive shape and ordering, (b) child `fields`
  shape (flag, env, default, value type), (c) child examples, (d) command
  names including kebab-case defaults and `#[command(name = "...")]`
  overrides, and (e) `windows` metadata where the variant's inner struct
  declared `#[ortho_config(windows(...))]`;
- new `rstest-bdd` scenarios in `ortho_config/tests/features/docs_ir.feature`
  exercising the same fixture, written in user-observable language;
- extended renderer tests in `cargo-orthohelp/src/roff/mod.rs::tests` and
  `cargo-orthohelp/src/powershell/wrapper.rs::tests` (plus a new test module
  in `cargo-orthohelp/src/powershell/mod.rs::tests`) that drive the existing
  public entry points with a populated tree;
- a golden snapshot harness (built on the already-present `insta` dependency
  at `cargo-orthohelp/Cargo.toml:35`) covering the roff output, the
  `PowerShell` wrapper module, and the MAML help XML for the fixture, with
  deterministic redactions over dates and absolute paths;
- a new end-to-end integration test under `cargo-orthohelp/tests/` driving the
  bridge for a fixture crate that opts into the new derives and asserting on
  the `--format ir`, `--format man`, and `--format ps` outputs;
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all passing at the close of each milestone; and
- `coderabbit review --agent` returning clean (or with all concerns resolved)
  before each milestone is marked done.

## Constraints

Hard invariants that must hold throughout implementation. Violating any of
these requires escalation in `Decision Log`, not a workaround.

- Do not implement code, tests, examples, or documentation in this branch
  until this ExecPlan is explicitly approved by the maintainer. A `DRAFT`
  plan remains a planning artefact only.
- Keep this work focused on roadmap item 6.1.2 ("Cover nested command trees
  with behavioural fixtures"). Anything that requires touching
  `OrthoConfigSubcommandDocs`, the struct derive, the `WindowsMetadata`
  shape, or the IR schema is out of scope and must be escalated.
- Do not change `ORTHO_DOCS_IR_VERSION`, `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`,
  or `ORTHO_POLICY_REPORT_SCHEMA_VERSION`. This is a test-coverage
  workstream; the IR shape established by 6.1.1 is treated as fixed.
- Do not add or rename fields on any IR type (`DocMetadata`, `WindowsMetadata`,
  `FieldMetadata`, `CliMetadata`, `EnvMetadata`, `Example`, `Link`, `Note`,
  `HeadingIds`, etc.). Do not add or rename clap-attribute keys
  (`#[ortho_config(...)]`, `#[command(...)]`, `#[clap(...)]`). Additive new
  helper functions inside `ortho_config_macros` are out of scope.
- Preserve the boundary established by ADR-003
  (`docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`) and
  re-affirmed by ADR-005
  (`docs/adr-005-subcommand-docs-companion-trait.md`). Test fixtures and
  golden snapshots belong to the human-documentation IR contract; they must
  not introduce dependencies on the agent-context schema, the bridge, or
  policy reports.
- Keep the mirrored schema in `cargo-orthohelp/src/schema/mod.rs`
  byte-for-byte aligned with `ortho_config/src/docs/ir.rs`. The
  version-alignment test in `cargo-orthohelp/src/schema/tests.rs` must
  continue to pass.
- Keep `cargo orthohelp --format ir`, `--format man`, `--format ps`, and
  `--format all` output stable for any consumer whose top-level config has no
  subcommand selector. New behavioural assertions only describe behaviour
  observable when the consumer opts in.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests per
  `docs/developers-guide.md` and `docs/rstest-bdd-users-guide.md`. Use
  `insta` for golden snapshots (it is already declared at
  `cargo-orthohelp/Cargo.toml:35` and `ortho_config/Cargo.toml:60`). Do not
  introduce `proptest`, `kani`, `verus`, `snapbox`, or `goldenfile`; the
  invariants are deterministic property assertions on a small fixed fixture,
  not properties over a range of inputs.
- Use `cap_std`/`camino` instead of `std::fs`/`std::path` for any test or
  example that introduces filesystem I/O. The existing crates already follow
  this rule; the new end-to-end test must do the same.
- Keep every Rust file under 400 lines. Every module begins with a `//!`
  comment. Use en-GB-oxendict spelling and grammar in documentation and
  comments, except for external API names such as `name`, `color`, and clap
  attribute keys.
- Follow `docs/documentation-style-guide.md` for documentation edits:
  Markdown wrapping at 80 columns, fenced code blocks have a language
  identifier (`plaintext` for non-code text), ADR and design-document
  structure.
- Run validation commands sequentially and capture output with `tee` into
  `/tmp` log files. Use the template
  `/tmp/$ACTION-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out`.
- Snapshot fixtures must be deterministic. Mask absolute paths, locale-driven
  dates, and any other environment-dependent substrings via `insta`
  `with_settings!` filters before snapshotting. Snapshots that contain
  workspace-absolute paths must not be checked in.
- Do not require `pwsh` (PowerShell) to be installed for `make test` to pass.
  If a `pwsh` AST parse gate is added, it must be `#[cfg]`- or
  runtime-skipped when `pwsh` is missing from `PATH`. The current sandbox
  reports `pwsh: command not found`; the gate must remain advisory.
- Do not mark roadmap item 6.1.2 complete in `docs/roadmap.md` until every
  validation gate in "Validation and acceptance" has passed and the draft
  pull request created from this plan has been moved out of draft state.

If satisfying the objective requires violating a constraint, stop, document
the conflict in `Decision Log`, and ask the maintainer for direction.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the
boundaries of autonomous action, not quality criteria.

- Approval: stop after drafting this plan and wait for explicit maintainer
  approval before starting any milestone other than Milestone 0.
- Scope: stop if the implementation requires changes to more than 18 files
  or more than 900 net lines of code and documentation (excluding `insta`
  golden snapshot files, which are reviewed separately and can be
  arbitrarily large).
- Public API: stop if any existing public type, trait, constant, function,
  command flag, derived attribute key, or feature flag must be renamed or
  removed.
- IR schema: stop if any field on any IR type would need to change to make
  an assertion expressible. The "Windows metadata propagation" question
  surfaced during research (parent-to-child inheritance is currently
  absent) is explicitly out of scope; if the assertions you want to write
  require such propagation, stop and present the trade-off.
- Subcommand derive: stop if implementing the fixture forces a change to
  `OrthoConfigSubcommandDocs` (for example, to support unit variants or
  externally subcommanded variants). Roadmap item 6.1.2 must work with the
  validation envelope established by 6.1.1.
- Dependencies: stop if any new external crate is required. `insta`,
  `rstest`, `rstest-bdd`, `clap`, `serde`, `serde_json`, `anyhow`, and
  `figment` are already declared and are the only acceptable dependencies
  for new tests.
- Renderer divergence: stop if rendering the populated fixture exposes a
  rendering bug that cannot be fixed in the renderer without changing the
  IR shape (e.g., a panic in `cargo-orthohelp/src/roff/mod.rs` on nested
  trees). Document the failure in `Surprises & Discoveries` and present
  options before patching.
- Snapshot churn: stop if a snapshot needs to be re-baselined more than
  twice in a single milestone without an obvious cause. Snapshot tests
  should converge; repeated drift suggests an underlying non-determinism
  that must be diagnosed.
- Tests: stop if `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, or `make nixie` still fails after two focused fix
  attempts.
- Documentation: stop if `docs/design.md`, `docs/cargo-orthohelp-design.md`,
  `docs/agent-native-cli-design.md`, `docs/users-guide.md`, and
  `docs/developers-guide.md` cannot describe the same fixture and harness
  without contradiction.
- Process: stop if branch rename, push, draft pull-request creation, or
  `coderabbit review --agent` fails in a way that might hide review
  feedback or leave the repository in an inconsistent state.
- Iteration: stop if a single milestone takes more than three working
  sessions without observable progress on its acceptance criteria. Record
  the cause in `Surprises & Discoveries`.

Adjust these values only with explicit maintainer approval recorded in
`Decision Log`.

## Risks

Known uncertainties that might affect the plan. Update as work proceeds.

- Risk: parent-to-child `WindowsMetadata` propagation is not implemented
  (see "Repository orientation"). Subcommand variants emit `windows: None`
  unless the variant's inner type itself carries
  `#[ortho_config(windows(...))]`. The assertion "Windows wrapper metadata
  where applicable" in the roadmap is therefore satisfied today by
  per-variant declaration, not by inheritance. Severity: medium.
  Likelihood: high. Mitigation: explicitly model "where applicable" as
  "per-variant opt-in" in both the fixture and the behavioural scenarios;
  record the propagation question as deferred work and surface it in
  `Decision Log` so a future roadmap item can take it up cleanly. Do not
  conflate test-coverage work with a design change.
- Risk: `insta` snapshots over roff output are sensitive to date rendering
  in the `.TH` macro and to absolute output paths in `RoffConfig`. Severity:
  medium. Likelihood: high. Mitigation: pin `config.date` to a literal
  string in the test fixture; redirect output into a `tempfile::TempDir`
  and never include the temp path in snapshots. Use
  `insta::with_settings!({ filters => vec![...] }, { ... })` to mask
  remaining sources of drift (e.g., the source/manual tokens) and pin the
  locale to `en-US` when building `LocalizedDocMetadata`.
- Risk: `insta` snapshots over `PowerShell` output are sensitive to CRLF
  handling and to the `$PSScriptRoot`-based wrapper resolution logic in
  `cargo-orthohelp/src/powershell/wrapper.rs:141-148`. Severity: low.
  Likelihood: medium. Mitigation: the wrapper already pushes CRLF
  explicitly via `crate::powershell::text::CRLF`; snapshot raw bytes and
  let `insta`'s default text mode handle line endings consistently. The
  `$PSScriptRoot` path is a runtime expression, not a substituted absolute
  path, so it does not require redaction.
- Risk: `make test` is required to succeed in environments without
  `pwsh`. A naive `Invoke-ScriptAnalyzer` or AST-parse gate would fail in
  the current sandbox (`pwsh: command not found`). Severity: high.
  Likelihood: high. Mitigation: do not introduce a `pwsh`-dependent gate
  in `make test`. If a `pwsh` AST parse helper is added, gate it on
  `which::which("pwsh").is_ok()` at runtime and `eprintln!` a skip notice
  instead of failing.
- Risk: the rstest-bdd harness already covers a single subcommand variant
  in `ortho_config/tests/rstest_bdd/scenario_state.rs:235-258`
  (`DocsConfig` / `DocsCommand` / `DocsGreetArgs`). Extending the fixture
  may force a rename to express the richer shape, breaking unrelated
  scenarios that depend on the `DocsContext` fixture. Severity: medium.
  Likelihood: medium. Mitigation: introduce a new, parallel
  `NestedDocsConfig` / `NestedDocsCommand` fixture tree alongside the
  existing `DocsConfig` rather than mutating it. Bind the new scenarios
  via a new feature file scoped to the new context type, and re-export
  the existing `DocsContext` unchanged.
- Risk: rstest-bdd step files have a 400-line cap and already host the
  bulk of the docs IR step definitions
  (`ortho_config/tests/rstest_bdd/behaviour/steps/docs_steps.rs`, currently
  ~153 lines). Adding richer tree-walking steps may push this past 400
  lines. Severity: low. Likelihood: medium. Mitigation: place the
  nested-tree step definitions in a new sibling file
  `nested_docs_steps.rs` and add it to `steps/mod.rs`.
- Risk: golden snapshots for a non-trivial fixture become large and noisy
  in code review. Severity: low. Likelihood: medium. Mitigation: keep the
  fixture small (three top-level variants, one nested level, ≤ three
  fields per variant). Snapshot one canonical output per renderer plus
  one variant with `split_subcommands_into_functions = true`. Do not
  snapshot every renderer permutation; pick the smallest set that proves
  the contract.
- Risk: registering an `examples/hello_world` variant or new
  `ortho_config/examples/` fixture for the end-to-end bridge test may
  trigger the existing pre-existing `markdownlint` debt described in the
  6.1.1 plan's "Surprises & Discoveries" section. Severity: low.
  Likelihood: medium. Mitigation: keep new example files Markdown-clean
  on first commit; do not run `make fmt` on unrelated files.
- Risk: `leta` (LSP-backed code navigation) may fail to start if
  `rust-analyzer` is unavailable. Severity: low. Likelihood: medium.
  Mitigation: install `rust-analyzer` via `rustup component add
  rust-analyzer` at session start; fall back to `Grep` for symbol
  navigation; record the limitation in `Surprises & Discoveries`.
- Risk: a snapshot baselined under one locale (the default `en-US`)
  drifts if a contributor runs the suite with a different `LANG`.
  Severity: low. Likelihood: low. Mitigation: pin `locale` to `en-US`
  explicitly when building the `LocalizedDocMetadata` for snapshots;
  do not rely on the process locale.

## Skills and source signposts

The implementation must use these skills (loaded via the `Skill` tool)
deliberately:

- `rust-router`: route Rust-specific implementation questions to the
  smallest useful skill.
- `leta`: default tool for Rust symbol navigation (`leta show`,
  `leta refs`, `leta grep`, `leta calls`); fall back to `Grep` if
  `rust-analyzer` is unavailable.
- `rust-testing-with-rstest-fixtures`
  (`docs/rust-testing-with-rstest-fixtures.md`): fixture patterns for the
  new test files.
- `reliable-testing-in-rust-via-dependency-injection`
  (`docs/reliable-testing-in-rust-via-dependency-injection.md`): keep new
  end-to-end tests deterministic.
- `rust-doctest-dry-guide` (`docs/rust-doctest-dry-guide.md`): doctest
  patterns if the new fixture is exposed in any `///` example.
- `nextest`: optional, when running targeted test groups under
  `cargo nextest run`.
- `domain-cli-and-daemons`: keep `cargo-orthohelp` stdout, stderr, exit
  codes, and machine-readable output stable.
- `hexagonal-architecture`: protect the IR contract in `ortho_config`
  from any awareness of bridge, renderer, filesystem, or process I/O.
- `rust-types-and-apis`: when shaping new test helper APIs, keep them
  small and obvious.
- `rust-errors`: tests use `Result` with `.expect(...)`; production
  helpers return `Result` and propagate errors via `?`.
- `code-review`: gate each milestone with a self-review pass before
  invoking `coderabbit review --agent`.
- `commit-message`: write commit messages from staged diffs without
  passing `-m` strings on the command line.
- `pr-creation`: open the draft PR with the standard df12-style body and
  the lody session link.
- `en-gb-oxendict`: documentation spelling and grammar.
- `complexity-antipatterns-and-refactoring-strategies`
  (`docs/complexity-antipatterns-and-refactoring-strategies.md`): split
  step modules and fixture builders before files exceed 400 lines.
- `localizable-rust-libraries-with-fluent`
  (`docs/localizable-rust-libraries-with-fluent.md`): the recursive IR
  carries Fluent identifiers in every subcommand entry; assertions on
  child `about_id`, `help_id`, and `long_help_id` use the same
  `<app_name>.fields.<field>.help` pattern as the parent.

The implementation must keep aligned with:

- `docs/roadmap.md` (especially §6.1.2 and the parent §6 framing);
- `docs/design.md` §4.2 ("The `#[derive(OrthoConfig)]` Macro") and §9
  ("Decision log");
- `docs/cargo-orthohelp-design.md` §2.1, §3.1, §3.5, and §13.1;
- `docs/agent-native-cli-design.md` §3.1, §4, and §9;
- `docs/users-guide.md` (the `OrthoConfigDocs` and `OrthoConfigSubcommandDocs`
  sections, the subcommand walkthroughs);
- `docs/developers-guide.md` (the "Schema ownership", "Behavioural tests",
  and "Adding or changing behavioural tests" sections);
- `docs/documentation-style-guide.md` (ADR template, design-document
  guidance, en-GB-oxendict rules);
- `docs/adr-005-subcommand-docs-companion-trait.md` (the trait shape this
  plan relies on);
- `docs/execplans/6-1-1-recursive-doc-metadata-subcommands-values.md` (the
  precursor plan whose outcomes this plan extends).

External prior art checked during planning:

- `clap_mangen` and `clap_complete` use `snapbox::file!` file snapshots
  driven by a shared fixture module (e.g.
  `clap_mangen/tests/testsuite/common.rs` exposing `basic_command`,
  `sub_subcommands_command`); structural assertions use runtime walks
  over `cmd.get_subcommands()`
  (<https://github.com/clap-rs/clap/blob/master/clap_mangen/tests/testsuite/roff.rs>,
  <https://github.com/clap-rs/clap/blob/master/clap_complete/tests/testsuite/bash.rs>).
  This plan adopts the *shape* of that pattern (shared fixture module,
  one renderer test file per output format, runtime walks for structural
  assertions) without adopting the *tool* (`snapbox`); the project's
  established snapshot tool is `insta`.
- `insta` is the de-facto Rust snapshot tool and supports `with_settings!`
  redactions and `cargo insta review`
  (<https://insta.rs/docs/>). It is already a workspace dependency.
- `groff_man(7)` documents that the `.TH` date register is build-environment
  dependent (<https://man7.org/linux/man-pages/man7/groff_man.7.html>);
  snapshots over roff output must pin or redact the date.
- `PowerShell` AST-parse validation via
  `[System.Management.Automation.Language.Parser]::ParseFile($path,
  [ref]$tokens, [ref]$errors)` is the standard "did it parse" gate and
  runs under `pwsh` on Linux
  (<https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/parsing>);
  this plan keeps any such gate optional and skipped when `pwsh` is
  absent.

These sources inform the design but do not override repository documents.

## Repository orientation

The relevant code and tests are concentrated in three crates and two
support directories.

`ortho_config` (runtime crate). Documentation IR lives in
`ortho_config/src/docs/`. `DocMetadata` is at
`ortho_config/src/docs/ir.rs:9-29` and carries `subcommands:
Vec<DocMetadata>` at line 26 and `windows: Option<WindowsMetadata>` at
line 28. `WindowsMetadata` itself is at
`ortho_config/src/docs/ir.rs:262-275`. The trait declarations
`OrthoConfigDocs` and `OrthoConfigSubcommandDocs` live in
`ortho_config/src/docs/mod.rs`. Reference unit coverage for the recursive
tree is at `ortho_config/tests/docs_ir_subcommands.rs:1-137` and exercises
a four-variant root with one nested subcommand (`AdminArgs ->
AdminCommands -> AuditArgs`).

`ortho_config_macros` (proc-macro crate). The struct derive entry point is
at `ortho_config_macros/src/lib.rs:44-99`; the docs-generation hook is at
line 80. The enum derive for `OrthoConfigSubcommandDocs` is at
`ortho_config_macros/src/lib.rs:109-116` and its implementation at
`ortho_config_macros/src/subcommand_docs.rs`. The struct-side recursion
into subcommand variants lives in
`ortho_config_macros/src/derive/generate/docs/mod.rs:38-99`
(`build_subcommands_metadata` calls
`<#inner_ty as OrthoConfigSubcommandDocs>::get_subcommand_doc_metadata()`).
The Windows metadata builder is at
`ortho_config_macros/src/derive/generate/docs/sections.rs:71-101`. The
parser for `#[ortho_config(windows(...))]` is at
`ortho_config_macros/src/derive/parse/doc_attrs.rs:166-189`. The parser
for `#[ortho_config(example(code = ...))]` is at the same file lines
191-217. These touch-points are read-only for this plan; do not change
them.

`cargo-orthohelp` (reference tool). The bridge wrapper is
`cargo-orthohelp/src/bridge.rs`. The localised IR (renderers consume this)
is `cargo-orthohelp/src/ir.rs`; the recursion mapping is at
`cargo-orthohelp/src/ir.rs:198-202`. The mirrored IR schema is at
`cargo-orthohelp/src/schema/mod.rs`; round-trip and version-alignment tests
are at `cargo-orthohelp/src/schema/tests.rs`. Roff rendering lives in
`cargo-orthohelp/src/roff/`; inline-COMMANDS rendering is at
`cargo-orthohelp/src/roff/mod.rs:182-199` and split-page rendering at
`cargo-orthohelp/src/roff/mod.rs:44-57`. Existing roff tests are at
`cargo-orthohelp/src/roff/mod.rs:254-330` and cover the inline-COMMANDS
path with two empty-field subcommands. `PowerShell` wrapper rendering
lives in `cargo-orthohelp/src/powershell/wrapper.rs`; tests at lines
237-268 cover `wrapper_renders_subcommand_functions` with the
`minimal_doc_with_subcommand()` fixture (no fields). MAML rendering lives
in `cargo-orthohelp/src/powershell/maml/`; tests exist for help XML but
do not exercise subcommands. The `about.help.txt` writer is at
`cargo-orthohelp/src/powershell/about.rs`; the manifest writer at
`cargo-orthohelp/src/powershell/manifest.rs`. Both are flat and do not
recurse into subcommands today (subcommand iteration happens in
`cargo-orthohelp/src/powershell/mod.rs:216-247`, which builds the
`CommandSpec` vector that the MAML renderer consumes).

Existing golden tests are at
`cargo-orthohelp/tests/golden/powershell_tests.rs:58-91`; they compare
generated `.psm1`, `.psd1`, `-help.xml`, and `-about.help.txt` against
embedded golden strings using `minimal_doc()` (no subcommands).

Tests live at:

- `ortho_config/tests/docs_ir_subcommands.rs` (recursive unit assertions,
  to be extended);
- `ortho_config/tests/features/docs_ir.feature` (rstest-bdd feature
  file, to be extended);
- `ortho_config/tests/rstest_bdd/behaviour/steps/docs_steps.rs` (existing
  step definitions; do not bloat);
- `ortho_config/tests/rstest_bdd/behaviour/steps/mod.rs` (registers
  per-feature step modules);
- `ortho_config/tests/rstest_bdd/behaviour/scenarios.rs` (binds feature
  files to fixtures);
- `ortho_config/tests/rstest_bdd/scenario_state.rs` (defines `DocsConfig`
  and `DocsContext` at lines 228-258 and the existing fixtures);
- `cargo-orthohelp/src/roff/mod.rs::tests` and
  `cargo-orthohelp/src/powershell/wrapper.rs::tests` (renderer unit
  coverage);
- `cargo-orthohelp/src/powershell/test_fixtures.rs:39-47`
  (`minimal_doc_with_subcommand` helper);
- `cargo-orthohelp/tests/golden/powershell_tests.rs` (existing
  PowerShell golden harness);
- `cargo-orthohelp/src/schema/tests.rs::sample_metadata` /
  `sample_subcommand` / `sample_windows` (round-trip coverage).

Examples live at `ortho_config/examples/registry_ctl.rs` and the
`examples/hello_world` crate. Either is a valid candidate for the
end-to-end smoke test; this plan uses the existing `examples/hello_world`
crate to minimise unrelated churn.

`pwsh` (PowerShell Core) is not installed in the development sandbox
(`pwsh: command not found`). Any AST-parse or `PSScriptAnalyzer` gate
must be optional and skipped when `pwsh` is absent.

## Recommended design

This plan introduces test fixtures, behavioural scenarios, renderer
assertions, golden snapshots, and an end-to-end bridge smoke test. It
introduces no new public API and no IR change. The design choices are:

### Fixture shape

A new, named fixture tree alongside (not replacing) the existing
`DocsConfig`. The new tree models a realistic CLI with mixed shapes:

- `NestedDocsConfig` (the root `Parser` struct) carries a single
  `#[command(subcommand)]` selector `command: NestedDocsCommand`, a
  required global flag, and a `#[ortho_config(windows(...))]`
  declaration at the struct level.
- `NestedDocsCommand` is a `#[derive(Subcommand,
  OrthoConfigSubcommandDocs)]` enum with three single-tuple variants:
  - `Greet(NestedGreetArgs)`: a leaf command. Wraps a `Parser` struct
    with one required and one optional flag, one
    `#[ortho_config(example(code = "..."))]` declaration, and no
    further `#[command(subcommand)]` field. Demonstrates the "command
    with no subcommands" requirement from the roadmap.
  - `Version(NestedVersionArgs)`: a no-options leaf command. Demonstrates
    that an empty `fields` array survives the renderer.
  - `Admin(NestedAdminArgs)`: a nested-subcommand command. Wraps a
    `Parser` struct with its own `#[command(subcommand)]` selector
    referencing `NestedAdminCommand`, which has two variants:
    `Audit(NestedAuditArgs)` and `Grant(NestedGrantArgs)`. The
    `Grant` variant declares
    `#[command(name = "grant-access")]` to exercise the kebab override
    path. `NestedAdminArgs` declares its own
    `#[ortho_config(windows(split_subcommands = true))]` to exercise
    per-variant Windows metadata (because parent-to-child propagation
    is not implemented; see "Risks").

This fixture lives in `ortho_config/tests/rstest_bdd/scenario_state.rs`
(beside `DocsConfig`) so it can be reused from both `rstest` units
(`ortho_config/tests/docs_ir_subcommands.rs`) and `rstest-bdd`
behavioural scenarios. The fixture's struct definitions stay under the
400-line cap by extracting the related helper into a new file
`ortho_config/tests/rstest_bdd/nested_docs_fixture.rs` if
`scenario_state.rs` would otherwise overflow.

### Behavioural-test shape

The new behavioural scenarios live in
`ortho_config/tests/features/docs_ir_nested.feature` (a new file) and are
bound to a new `NestedDocsContext` in `scenarios.rs`. The new step
definitions live in
`ortho_config/tests/rstest_bdd/behaviour/steps/nested_docs_steps.rs`
to keep the existing `docs_steps.rs` under the 400-line cap. Steps are
written in user-observable language ("Given the nested CLI fixture",
"Then the tree contains the command 'admin grant-access'", "Then the
'greet' command has the field 'recipient' with default 'World'", "Then
the 'admin' command exposes Windows wrapper metadata that splits
subcommands into functions"), per
`docs/developers-guide.md`.

### Renderer-test shape

Renderer compatibility is asserted at two layers:

1. *Targeted unit assertions* in the existing renderer test modules,
   driven by a new `LocalizedDocMetadata` builder that mirrors the
   `NestedDocsConfig` shape. These tests assert the presence of specific
   substrings in the rendered output (e.g. `.SS admin grant-access`,
   `function fixture_admin_grant-access` style functions) and explicitly
   exercise both `should_split_subcommands = true` and
   `should_split_subcommands = false`.
2. *Golden snapshots* in a new file `cargo-orthohelp/tests/snapshots/`
   directory using `insta` (already a workspace dependency, see
   `cargo-orthohelp/Cargo.toml:35`). The snapshot suite is a small new
   integration test
   `cargo-orthohelp/tests/golden/nested_subcommand_snapshots.rs`. It
   covers:
   - the roff output for the inline-COMMANDS path (one file);
   - the roff output for the split-subcommands path (one file per
     subcommand);
   - the `PowerShell` wrapper module for both
     `split_subcommands_into_functions` settings (two files);
   - the MAML help XML for the populated subcommands (one file).
   All snapshots use `insta::with_settings!({ filters => ... }, { ... })`
   to redact dates and absolute paths. Snapshots are reviewed with
   `cargo insta review`.

### End-to-end smoke test

A new file `cargo-orthohelp/tests/nested_subcommand_end_to_end.rs`
drives the bridge against a fixture crate (a new minimal binary under
`examples/hello_world/` or a new `cargo-orthohelp/tests/fixtures/`
crate). The test:

1. invokes the bridge with `--format ir` and asserts the resulting JSON
   parses, has the expected number of nested levels, and lists the
   expected command names in order;
2. invokes the bridge with `--format man` and asserts that the generated
   files exist with the expected names and that one of them includes a
   non-empty `.SH COMMANDS` section;
3. invokes the bridge with `--format ps` and asserts that the `.psm1`
   contains the expected `function fixture_admin` style wrappers when
   `split_subcommands_into_functions` is on.

The smoke test uses `cap_std`/`camino` for path handling and
`tempfile::TempDir` for output. It does not invoke `pwsh`.

### Out of scope

The following are deliberately not addressed by this plan:

- changing the IR schema to add propagation of `WindowsMetadata` from
  parents to children, hidden/aliased/deprecated subcommand metadata,
  or unit-variant support;
- snapshotting every renderer permutation; the plan picks the smallest
  set that proves the contract and lets `insta` review handle updates;
- introducing `snapbox`, `goldenfile`, `proptest`, `kani`, `verus`, or
  any new external crate;
- changing `cargo-orthohelp` CLI options, default formats, or stdout
  shapes;
- emitting subcommand-aware agent-context entries (roadmap §6.2) or
  policy reports (§7); those phases consume this plan's fixture later.

## Planned implementation milestones

Each milestone ends with a validation gate. Do not begin the next
milestone until the previous one's gate is green and
`coderabbit review --agent` is clear. Commit at the end of each milestone
(or more frequently if local checkpoints are useful) with descriptive
messages following `docs/documentation-style-guide.md` and `AGENTS.md`.

### Milestone 0: approve plan

Goal: turn this plan into an approved design decision before touching
code.

Steps:

1. Submit this ExecPlan for review and wait for explicit maintainer
   approval. Record the approval date in `Progress`. Update `Status:` to
   `APPROVED`.
2. No ADR is required: this plan adds tests and snapshots; it makes no
   design or schema decisions beyond those already recorded in ADR-005.
   If review surfaces a design decision (for example, "snapshots belong
   in `tests/snapshots/`, not `tests/golden/`"), record it in
   `Decision Log` instead of opening a new ADR.

Validation:

```sh
set -o pipefail
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make nixie 2>&1 \
  | tee /tmp/nixie-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
```

Expected: both commands exit successfully. Record pre-existing failures
in `Surprises & Discoveries` and ask the maintainer whether to expand
scope.

Run `coderabbit review --agent` and clear concerns.

Acceptance: the plan is `APPROVED` and `coderabbit review --agent` is
clean against the plan document.

### Milestone 1: introduce the fixture tree and the recursive unit assertions

Goal: stand up the `NestedDocsConfig` fixture and prove the recursive IR
shape, fields, examples, command-name overrides, and per-variant Windows
metadata via deterministic `rstest` cases.

Steps:

1. Add the `NestedDocsConfig` / `NestedDocsCommand` / `NestedGreetArgs`
   / `NestedVersionArgs` / `NestedAdminArgs` / `NestedAdminCommand` /
   `NestedAuditArgs` / `NestedGrantArgs` types to
   `ortho_config/tests/rstest_bdd/scenario_state.rs` immediately after
   the existing `DocsConfig` block. Each `Args` struct must derive
   `clap::Args`, `serde::Deserialize`, `serde::Serialize`,
   `OrthoConfig`, and `Default`. Each `Subcommand` enum must derive
   `clap::Subcommand` and `OrthoConfigSubcommandDocs` and implement
   `Default` returning the first variant. Use `#[serde(skip)]` and
   `#[command(subcommand)]` on subcommand selector fields, mirroring
   `DocsConfig` and `RootWithSubcommands`.
2. Declare `#[ortho_config(windows(module_name = "Nested",
   include_common_parameters = true))]` on `NestedDocsConfig` and
   `#[ortho_config(windows(module_name = "NestedAdmin",
   split_subcommands = true))]` on `NestedAdminArgs`. Leave
   `NestedGreetArgs`, `NestedVersionArgs`, `NestedAuditArgs`, and
   `NestedGrantArgs` without a `windows` declaration; they prove the
   "no Windows metadata where not declared" assertion.
3. Declare at least one `#[ortho_config(example(code = "...", title_id =
   "..."))]` on `NestedGreetArgs` (struct-level) and at least one on a
   field of `NestedAdminArgs` to exercise both example-attachment
   points.
4. If `scenario_state.rs` would exceed 400 lines, extract the new types
   into a new module `ortho_config/tests/rstest_bdd/nested_docs_fixture.rs`
   and re-export from `scenario_state.rs`.
5. Extend `ortho_config/tests/docs_ir_subcommands.rs` (or add a sibling
   `nested_docs_ir.rs`, choosing whichever keeps each file under 400
   lines) with the following `rstest` cases:
   - `nested_root_lists_all_top_level_commands_in_declaration_order`
     (asserts the `subcommands` vector is `["greet", "version", "admin"]`);
   - `nested_root_fields_excludes_subcommand_selector` (asserts that the
     `global` flag is the only entry in `fields`);
   - `nested_greet_command_has_expected_fields_and_examples` (asserts
     `app_name == "greet"`, presence of `recipient` field with the
     declared default and value type, and a non-empty `examples` array
     on the command);
   - `nested_version_command_has_no_fields` (asserts the empty `fields`
     vector survives);
   - `nested_admin_command_lists_audit_and_grant_access` (asserts
     `subcommands[2].subcommands.iter().map(|c| c.app_name).collect()
     == ["audit", "grant-access"]`, exercising the `#[command(name =
     "grant-access")]` override);
   - `nested_admin_command_carries_split_subcommands_windows_metadata`
     (asserts `subcommands[2].windows.as_ref().unwrap()
     .split_subcommands_into_functions == true`);
   - `nested_greet_command_has_no_windows_metadata` (asserts
     `subcommands[0].windows.is_none()`);
   - `nested_admin_audit_has_inherited_fluent_id_pattern` (asserts the
     audit command's `about_id == "audit.about"` and one field's
     `help_id` follows the `"audit.fields.<field>.help"` pattern,
     confirming the variant-name override flows through the Fluent ID
     generator).

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
```

Expected: all three commands exit successfully. The new tests fail
before this milestone's edits and pass after.

Run `coderabbit review --agent` and clear concerns.

Acceptance: the recursive IR generated for `NestedDocsConfig` carries
the expected tree shape, fields, examples, command names, and Windows
metadata, asserted by deterministic `rstest` cases.

### Milestone 2: bind the fixture into rstest-bdd behavioural scenarios

Goal: re-express the structural assertions from Milestone 1 in
behavioural language so a maintainer reading the feature file
understands the contract without consulting the test source.

Steps:

1. Add a new fixture function `nested_docs_context()` returning
   `NestedDocsContext` (a `ScenarioState`-derived type holding a
   `Slot<DocMetadata>` for the captured metadata, mirroring
   `DocsContext`) to `ortho_config/tests/rstest_bdd/scenario_state.rs`.
   Re-export `NestedDocsContext` and `nested_docs_context` through the
   module's `pub use` block.
2. Add `tests/features/docs_ir_nested.feature` with scenarios:
   - "Nested tree exposes every top-level command in declaration order";
   - "Greet command exposes its recipient field and example";
   - "Version command exposes no fields";
   - "Admin command exposes audit and grant-access subcommands in order";
   - "Admin command exposes Windows wrapper metadata that splits
     subcommands into functions";
   - "Greet command exposes no Windows wrapper metadata".
   Phrase steps so they read as user-observable behaviour
   (`Given the nested CLI fixture`, `When I request the docs metadata`,
   `Then the tree contains the command "admin grant-access"`, etc.).
3. Add
   `ortho_config/tests/rstest_bdd/behaviour/steps/nested_docs_steps.rs`
   with the new step definitions. Reuse helpers from
   `ortho_config/tests/rstest_bdd/behaviour/steps/helpers.rs` where
   possible; add new helpers only when the existing ones do not fit.
   Register the new module in
   `ortho_config/tests/rstest_bdd/behaviour/steps/mod.rs`.
4. Bind the feature file in
   `ortho_config/tests/rstest_bdd/behaviour/scenarios.rs` via a new
   `scenarios!("tests/features/docs_ir_nested.feature", fixtures =
   [nested_docs_context: NestedDocsContext]);` block.
5. Confirm the existing `docs_ir.feature` scenarios continue to pass
   unchanged.

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
```

Expected: all four commands pass. The behavioural scenarios pass on
first run because they read the same `DocMetadata` already proven
correct by Milestone 1.

Run `coderabbit review --agent` and clear concerns.

Acceptance: a maintainer reading
`tests/features/docs_ir_nested.feature` understands the recursive
contract without reading any Rust source.

### Milestone 3: renderer compatibility tests and golden snapshots

Goal: prove that the existing roff, `PowerShell` wrapper, and MAML
renderers handle a populated tree correctly and that their output stays
stable across changes.

Steps:

1. Add a `LocalizedDocMetadata` builder helper in a new file
   `cargo-orthohelp/src/test_support/nested_fixture.rs` (kept inside a
   `#[cfg(test)]` module so it does not pollute the crate's public
   surface) that constructs a fixture matching the shape of the
   `NestedDocsConfig` tree (the same names, ordering, fields, examples,
   and per-node Windows metadata). Re-use the existing
   `LocalizedHeadings` and `LocalizedSectionsMetadata` initialisers.
2. Extend `cargo-orthohelp/src/roff/mod.rs::tests` with:
   - `inline_subcommands_render_for_nested_fixture` (asserts
     `.SH COMMANDS`, `.SS greet`, `.SS version`, `.SS admin` and the
     expected `--recipient` option flag under `.SS greet`);
   - `split_subcommands_render_for_nested_fixture` (drives `generate`
     with `should_split_subcommands = true` into a `TempDir` and
     asserts the resulting filenames include
     `fixture-greet.1`, `fixture-version.1`, `fixture-admin.1`, and
     that the admin man page contains a `.SH COMMANDS` section listing
     `audit` and `grant-access`).
3. Extend `cargo-orthohelp/src/powershell/wrapper.rs::tests` with:
   - `wrapper_renders_nested_subcommands_when_splitting` (drives
     `render_wrapper` with the nested fixture and
     `should_split_subcommands = true`; asserts the output contains
     `function fixture_greet`, `function fixture_version`, and
     `function fixture_admin`, and does *not* contain
     `function fixture_admin_audit` because the wrapper renders only
     one level today).
4. Extend `cargo-orthohelp/src/powershell/maml/tests.rs` with a
   single subcommand-aware case
   (`render_help_includes_nested_subcommand_help`) that drives the MAML
   renderer with a `CommandSpec` vector containing the greet and audit
   commands and asserts the generated XML contains both
   `<command:name>greet</command:name>` and
   `<command:name>audit</command:name>`.
5. Add `cargo-orthohelp/tests/golden/nested_subcommand_snapshots.rs`
   driving:
   - `roff::generate_to_string` with the nested fixture into one
     `insta::assert_snapshot!` baseline;
   - `roff::generate` with `should_split_subcommands = true` into a
     `TempDir`, then read and `insta::assert_snapshot!` each generated
     file (one snapshot per file, with the file name in the snapshot's
     suffix);
   - the full `PowerShell` output bundle for the nested fixture (one
     snapshot for `.psm1`, one for `.psd1`, one for the MAML XML, one
     for `about.help.txt`), with `split_subcommands_into_functions =
     true`.
   Use `insta::with_settings!({ filters => vec![
       (r"^\.TH .*", "[TH-MASK]"),
       (r"/tmp/[^\s]+", "[TMP-PATH]"),
   ], ... })` to redact dates and absolute paths.
6. Run `cargo insta review` locally; commit the accepted snapshots to
   `cargo-orthohelp/tests/golden/snapshots/`. Snapshots live next to
   the test file per `insta`'s default layout. Do not commit pending
   `.pending-snap` files.
7. Confirm `cargo-orthohelp/tests/golden/powershell_tests.rs::powershell_outputs_match_goldens`
   (the existing hand-rolled golden test) still passes against the
   `minimal_doc()` fixture.

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
```

Expected: all commands pass. The renderer assertions and golden
snapshots prove the existing renderers handle the populated tree
without regressions.

Run `coderabbit review --agent` and clear concerns.

Acceptance: a developer who edits the renderers sees a clearly named
snapshot diff in `cargo insta pending-snapshots` rather than a
diffuse runtime panic.

### Milestone 4: end-to-end bridge smoke test and example wiring

Goal: drive the bridge end-to-end against a real consumer crate that
opts into the new derives, proving the IR, man, and PowerShell formats
all carry the populated tree.

Steps:

1. Add `cargo-orthohelp/tests/nested_subcommand_end_to_end.rs`. The
   test uses `cap_std`/`camino` for paths and `tempfile::TempDir` for
   output. It points the bridge at the `examples/hello_world` crate
   (or a smaller `cargo-orthohelp/tests/fixtures/nested_cli/` crate if
   reusing `hello_world` would expand its surface beyond what the
   maintainer wants). The fixture crate's root config must derive
   `OrthoConfig`, hold a `#[command(subcommand)]` field, and the
   subcommand enum must derive `OrthoConfigSubcommandDocs`. The test:
   - invokes the bridge with `--format ir` (via the bridge's library
     entry point, not by shelling out to `cargo orthohelp`), parses the
     JSON into `serde_json::Value`, and asserts the
     `subcommands[*].app_name` values match the fixture in declaration
     order, including the nested level;
   - invokes the bridge with `--format man` and asserts each expected
     file exists in the output directory and one of them includes
     `.SH COMMANDS`;
   - invokes the bridge with `--format ps` and asserts the `.psm1`
     contains the expected `function` lines.
2. If `examples/hello_world` is used, extend its `Commands` enum with
   a second variant (or add a nested subcommand on the existing
   variant) so the fixture covers more than one branch. Keep the
   change minimal; do not re-shape the example for stylistic
   improvements.
3. If a new `cargo-orthohelp/tests/fixtures/nested_cli/` crate is
   used instead, declare it as a `path` dependency in the workspace
   `Cargo.toml` *only as a dev-dependency of `cargo-orthohelp`*, not a
   workspace member, to avoid expanding `cargo build --workspace`.

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
```

Expected: all commands pass. The end-to-end test proves the bridge,
renderer, and PowerShell paths all consume the populated tree without
regressions.

Run `coderabbit review --agent` and clear concerns.

Acceptance: a contributor who breaks the bridge or any renderer sees a
clearly named end-to-end failure in `cargo test
nested_subcommand_end_to_end` rather than a downstream surprise.

### Milestone 5: documentation, schema round-trip update, roadmap close-out

Goal: bring documentation, the schema round-trip fixture, and the
roadmap entry in line with the new coverage.

Steps:

1. Extend `cargo-orthohelp/src/schema/tests.rs::sample_metadata` (line
   51-) and `::sample_subcommand` (line 62-) so the round-trip exercise
   includes a two-level nested subcommand and at least one
   `WindowsMetadata` block on a child node. Confirm the existing
   `schema_round_trips_against_ortho_config` test passes against the
   new sample.
2. Update `docs/cargo-orthohelp-design.md`:
   - §3.5 (implementation notes): note that renderer regressions on
     nested trees are gated by `insta` snapshots in
     `cargo-orthohelp/tests/golden/nested_subcommand_snapshots.rs`.
   - §13.1 (IR JSON excerpt): include a two-level nested example with
     a populated `windows` block on a child node.
3. Update `docs/agent-native-cli-design.md` §4 to note that the
   behavioural-fixture coverage from this plan satisfies the "whole-CLI
   introspection" verification gate. Update §9 to remove or mark
   resolved the bullet at line 613 if it has not already been struck
   by 6.1.1.
4. Update `docs/users-guide.md`:
   - in the "Documentation metadata
     (`OrthoConfigDocs`/`OrthoConfigSubcommandDocs`)" section, add a
     short walkthrough showing how to extend a clap CLI with the new
     derives so a downstream consumer's tree is asserted by
     `cargo orthohelp --format ir`.
   - cross-reference the new feature file
     (`ortho_config/tests/features/docs_ir_nested.feature`) as an
     example of behavioural-test style.
5. Update `docs/developers-guide.md`:
   - in the "Behavioural tests" or "Adding or changing behavioural
     tests" section, document the new fixture
     (`NestedDocsConfig`) and the convention of placing
     fixture-specific step modules in
     `tests/rstest_bdd/behaviour/steps/*_steps.rs`.
   - in the "Snapshots" subsection (or add one if absent), document
     the `insta` snapshot convention, the `cargo insta review`
     workflow, and the redaction filters used by
     `nested_subcommand_snapshots.rs`.
6. Update `CHANGELOG.md` "Unreleased / Added" with two bullets:
   - "Behavioural fixtures and step definitions covering nested
     subcommand trees (`ortho_config/tests/features/docs_ir_nested.feature`).";
   - "Renderer compatibility tests and `insta` golden snapshots for
     populated nested-subcommand `DocMetadata`
     (`cargo-orthohelp/tests/golden/nested_subcommand_snapshots.rs`).".
7. Mark the relevant `docs/roadmap.md` entries done:
   - `[x] 6.1.2. Cover nested command trees with behavioural fixtures.`;
   - the three sub-bullets ("Add a fixture CLI...", "Assert that
     generated IR...", "Ensure existing man-page and PowerShell
     output...").

Validation:

```sh
set -o pipefail
make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
make nixie 2>&1 \
  | tee /tmp/nixie-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
```

Expected: all five commands pass. Run `coderabbit review --agent` and
clear concerns. Move the draft pull request out of draft state once the
maintainer has reviewed.

Acceptance: the roadmap entry is closed; every doc references the new
fixture and harness consistently; the draft PR is moved to
ready-for-review; the change has landed on `main`.

## Concrete steps

The following commands are the canonical operations a fresh agent should
run. They are deliberately idempotent: re-running them after a partial
failure recreates the same state without drift.

Repository-orientation (read-only):

```sh
git fetch origin
git branch --show-current
ls docs/execplans/6-1-2-nested-command-tree-behavioural-fixtures.md
```

Per-milestone validation (sequential, with `tee`):

```sh
set -o pipefail

make check-fmt 2>&1 \
  | tee /tmp/check-fmt-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out

make lint 2>&1 \
  | tee /tmp/lint-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out

make test 2>&1 \
  | tee /tmp/test-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out

make markdownlint 2>&1 \
  | tee /tmp/markdownlint-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out

make nixie 2>&1 \
  | tee /tmp/nixie-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
```

Targeted iteration during development:

```sh
cargo test -p ortho_config --tests docs_ir_subcommands
cargo test -p ortho_config --tests rstest_bdd
cargo test -p cargo-orthohelp --tests
cargo insta review
```

CodeRabbit gate after each milestone:

```sh
coderabbit review --agent 2>&1 \
  | tee /tmp/coderabbit-ortho-config-6-1-2-nested-command-tree-behavioural-fixtures.out
```

Branch hygiene (only after maintainer approval; do not run while plan is
DRAFT):

```sh
git push -u origin 6-1-2-nested-command-tree-behavioural-fixtures
```

Draft pull-request creation (once Milestone 0 completes; see PR template
notes at the end of this document):

```sh
gh pr create --draft \
  --title "Plan: nested command tree behavioural fixtures (6.1.2)" \
  --body-file /tmp/pr-body-6-1-2-nested-command-tree-behavioural-fixtures.md
```

Update this `Concrete steps` section whenever a milestone changes the
commands a new contributor must run.

## Validation and acceptance

A change implementing this plan is "done" when all of the following
hold, verified by the commands above:

- Tests
  - `make test` passes from a clean checkout, with the new fixture
    types, `rstest` cases, `rstest-bdd` scenarios, renderer unit
    cases, `insta` snapshots, and end-to-end smoke test all passing.
  - The new `ortho_config/tests/features/docs_ir_nested.feature`
    scenarios pass.
  - `cargo-orthohelp/tests/golden/nested_subcommand_snapshots.rs`
    passes with committed snapshots; no `*.pending-snap` files remain.
  - `cargo-orthohelp/tests/nested_subcommand_end_to_end.rs` passes.
  - Existing tests, including
    `cargo-orthohelp/tests/golden/powershell_tests.rs::powershell_outputs_match_goldens`,
    continue to pass unchanged.
- Lint and format
  - `make check-fmt` passes.
  - `make lint` passes (no new `#[allow]` or `#[expect]` annotations
    beyond those already in place).
  - `make markdownlint` passes for all new and edited markdown.
- Documentation rendering
  - `make nixie` passes for diagrams referenced from the design and
    user guides.
- Behaviour
  - `cargo run -p cargo-orthohelp --bin cargo-orthohelp -- --format ir`
    against the fixture crate emits a JSON document with a two-level
    nested `subcommands` tree in declaration order, with the
    expected `app_name`, `fields`, `examples`, and `windows` values.
  - `--format man` emits man pages whose `.SH COMMANDS` sections list
    the expected child names.
  - `--format ps` emits a `.psm1` whose function exports include the
    expected nested wrappers.
- Process
  - `coderabbit review --agent` is clean at the end of each
    milestone.
  - The draft pull request is moved out of draft state by the
    maintainer after a review pass.
  - `docs/roadmap.md` `[ ] 6.1.2. ...` is updated to `[x]`.

Quality criteria for "done":

- Public API: no public item is renamed or removed; no IR field is
  added or renamed.
- Schema versions: unchanged.
- Files: no Rust file exceeds 400 lines; every new module starts with
  `//!`.
- Language: en-GB-oxendict spelling and grammar in documentation and
  comments.
- Tests: no fixture is shared mutably across scenarios; `#[once]` is
  used only for effectively read-only infrastructure.
- Snapshots: every snapshot is deterministic under repeated runs
  without `cargo insta accept`; no snapshot embeds an absolute path,
  build date, or environment-dependent string.

## Idempotence and recovery

- All validation steps are read-only and re-runnable.
- All edits to source code, documentation, snapshots, and roadmap are
  tracked by git; recovery from a half-completed milestone is
  `git status` followed by reverting unstaged changes or committing
  the partial work as a checkpoint.
- `cargo insta review` is idempotent; pending snapshots can be
  inspected and accepted or rejected without affecting committed
  baselines.
- `coderabbit review --agent` is idempotent per branch state;
  re-running it after addressing comments produces a fresh report.
- The renamed branch
  `6-1-2-nested-command-tree-behavioural-fixtures` tracks
  `origin/6-1-2-nested-command-tree-behavioural-fixtures`; re-pushing
  is safe because no other agent or human is expected to push to that
  branch.

## Artefacts and notes

This section captures evidence that helped shape the plan. Update it
as implementation proceeds (transcripts of failing test runs,
comparative output before and after a change, etc.).

- The existing recursive unit test at
  `ortho_config/tests/docs_ir_subcommands.rs:88-136` proves the
  derive emits a populated tree but does not exercise field metadata,
  examples, or Windows metadata on children. This plan extends, not
  replaces, that test file.
- The existing `DocsConfig` fixture at
  `ortho_config/tests/rstest_bdd/scenario_state.rs:228-258` covers a
  single-variant subcommand enum (`Greet`). Extending it in place
  risks breaking unrelated docs scenarios that depend on its shape;
  the new `NestedDocsConfig` lives beside it.
- The existing roff inline-subcommands renderer test at
  `cargo-orthohelp/src/roff/mod.rs:306-329` proves the path does not
  panic on a two-element subcommand vector but does not assert on
  option flags or nested levels.
- The existing PowerShell wrapper subcommand-functions test at
  `cargo-orthohelp/src/powershell/wrapper.rs:251-256` proves the
  function-export path generates a wrapper for one subcommand. The
  new tests extend the fixture and assert on multiple wrappers and
  on the absence of second-level wrappers (because the renderer
  currently produces only one level).
- The existing PowerShell golden test at
  `cargo-orthohelp/tests/golden/powershell_tests.rs:58-91` uses an
  empty-subcommands fixture and serves as a useful baseline for the
  new nested snapshots.
- The Windows metadata propagation question is recorded in `Risks`
  and `Decision Log` as deferred work; this plan asserts the
  observable behaviour (per-variant declaration), not the future
  behaviour (inheritance).
- `pwsh` is not installed in the sandbox; any `pwsh`-backed
  validation gate must be advisory and runtime-skipped.

## Interfaces and dependencies

This plan introduces no new public types, traits, attributes, or
functions. The new test-only items live entirely under
`ortho_config/tests/`, `cargo-orthohelp/tests/`, and
`cargo-orthohelp/src/test_support/` (a new `#[cfg(test)]` module).
Their concrete identifiers are:

```rust
// ortho_config/tests/rstest_bdd/scenario_state.rs (new types beside DocsConfig)
#[derive(Debug, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "NESTED_APP_",
    discovery(app_name = "nested-app"),
    windows(module_name = "Nested", include_common_parameters = true)
)]
pub struct NestedDocsConfig {
    pub global: String,
    #[serde(skip)]
    #[command(subcommand)]
    pub command: NestedDocsCommand,
}

#[derive(Debug, Subcommand, OrthoConfigSubcommandDocs)]
pub enum NestedDocsCommand {
    Greet(NestedGreetArgs),
    Version(NestedVersionArgs),
    Admin(NestedAdminArgs),
}

// ... plus NestedGreetArgs, NestedVersionArgs, NestedAdminArgs (with
// `#[ortho_config(windows(split_subcommands = true))]`), NestedAdminCommand,
// NestedAuditArgs, NestedGrantArgs (with `#[command(name = "grant-access")]`).

// ortho_config/tests/rstest_bdd/scenario_state.rs (new fixture function)
#[derive(Debug, Default, ScenarioState)]
pub struct NestedDocsContext {
    pub metadata: Slot<DocMetadata>,
}

#[fixture]
pub fn nested_docs_context() -> NestedDocsContext {
    NestedDocsContext::default()
}
```

No new external crate dependency is introduced. The implementation reuses
already-declared dependencies:

- `clap`, `serde`, `serde_json`, `anyhow`, `figment`, `rstest`,
  `rstest-bdd` for tests;
- `insta` for snapshots
  (`cargo-orthohelp/Cargo.toml:35`, `ortho_config/Cargo.toml:60`);
- `tempfile` and `cap_std`/`camino` for filesystem isolation in the
  end-to-end test.

## Progress

Use a list with checkboxes to summarise granular steps. Every stopping
point must be documented here, even if it requires splitting a
partially completed task into two ("done" vs. "remaining"). This
section must always reflect the actual current state of the work.

- [x] (2026-05-31) Draft ExecPlan created.
- [x] (2026-06-04) Maintainer approval recorded; status set to
  `IN PROGRESS` because implementation has begun.
- [x] (2026-06-04) Milestone 0 complete (plan approved,
  markdownlint and nixie clean, CodeRabbit clear).
- [ ] Milestone 1 complete (fixture types added; recursive `rstest`
  assertions pass).
- [ ] Milestone 2 complete (rstest-bdd scenarios bind and pass).
- [ ] Milestone 3 complete (renderer compatibility tests and `insta`
  snapshots pass; baselines committed).
- [ ] Milestone 4 complete (end-to-end bridge smoke test passes;
  example crate wired).
- [ ] Milestone 5 complete (documentation, changelog, schema round-trip
  fixture, roadmap entry marked done).
- [ ] Draft pull request moved to ready-for-review.
- [ ] Pull request merged into `main`.

Use timestamps to detect tolerance breaches and to feed retrospectives.

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as
risks. Document with evidence so future work benefits.

- 2026-06-04: `leta files` accepts one path argument, not multiple path
  arguments. Repository orientation therefore used one invocation per
  relevant path (`ortho_config/tests`, `cargo-orthohelp/src`, and
  `cargo-orthohelp/tests`). This affects only navigation, not the
  implementation design.

## Decision log

Record every significant decision made while working on the plan.
Include decisions to escalate, decisions on ambiguous requirements, and
design choices.

- Decision: introduce a new `NestedDocsConfig` fixture beside the
  existing `DocsConfig` rather than extending `DocsConfig` in place.
  Rationale: `DocsConfig` is used by every scenario in the existing
  `docs_ir.feature` file; mutating it risks behavioural drift in
  unrelated scenarios. A parallel fixture isolates the new contract,
  costs only a small amount of duplication, and lets the new
  behavioural scenarios bind to a dedicated context. Date/Author:
  2026-05-31 (planner).
- Decision: assert "Windows wrapper metadata where applicable" as
  per-variant declaration, not parent-to-child inheritance. Rationale:
  the current derive emits `windows: None` on every nested
  `DocMetadata` unless the variant's inner struct itself carries
  `#[ortho_config(windows(...))]` (see "Repository orientation" and
  "Risks"). The roadmap text "where applicable" is honestly satisfied
  by declaring `windows` per variant. Changing inheritance would
  require a schema decision, an ADR, and a `ORTHO_DOCS_IR_VERSION`
  bump; that is a separate roadmap item. Date/Author: 2026-05-31
  (planner).
- Decision: use `insta` for golden snapshots instead of `snapbox` or
  `goldenfile`. Rationale: `insta` is already a workspace dependency
  (`cargo-orthohelp/Cargo.toml:35`), supports redaction filters via
  `with_settings!`, integrates with `cargo insta review`, and avoids
  adding a third snapshot tool to the repository. The clap ecosystem
  uses `snapbox`, but the precedent in this repository
  (e.g. the existing `cargo-orthohelp/tests/golden/powershell_tests.rs`
  hand-rolled goldens; the workspace `insta` dependency) tips the
  balance towards `insta`. Date/Author: 2026-05-31 (planner).
- Decision: keep `pwsh`-backed validation (AST parse,
  `PSScriptAnalyzer`) out of the default `make test` gate. Rationale:
  `pwsh` is not installed in the development sandbox (`pwsh: command
  not found`) and adding a hard-required PowerShell gate would break
  `make test` for every contributor without PowerShell. If a `pwsh`
  helper is added, it must be runtime-skipped via
  `which::which("pwsh")`. Date/Author: 2026-05-31 (planner).
- Decision: place the end-to-end bridge smoke test against the
  existing `examples/hello_world` crate, extending its `Commands` enum
  by one variant rather than creating a brand-new fixture crate.
  Rationale: minimises workspace churn, exercises the same code path
  consumers exercise, and avoids dragging a fresh `Cargo.toml`
  through the workspace build. If review prefers a dedicated fixture
  crate, this is reversible with a small edit. Date/Author: 2026-05-31
  (planner).
- Decision: bind new behavioural scenarios to a new
  `NestedDocsContext` rather than reusing `DocsContext`. Rationale:
  fixture isolation; the existing `DocsContext` already holds a
  `Slot<DocMetadata>` for the `DocsConfig` walkthrough. Sharing it
  risks scenario interleaving issues with the rstest-bdd harness.
  Date/Author: 2026-05-31 (planner).
- Decision: treat the maintainer's 2026-06-04 instruction to
  "proceed with implementation" as explicit approval of the existing
  ExecPlan. Rationale: the instruction directly requests execution of
  this plan and supplies milestone validation requirements, so a
  separate approval pause would contradict the current task. Date/Author:
  2026-06-04 (implementer).

## Outcomes & retrospective

Summarise outcomes, gaps, and lessons learned at major milestones or
at completion. Compare the result against the original purpose. Note
what would be done differently next time.

- (none yet — populate at completion)
- 2026-06-04: Milestone 0 completed. `make markdownlint` and
  `make nixie` passed, and `coderabbit review --agent` reported
  zero findings for the approval/status update.

## Notes for the accompanying draft pull request

When opening the draft pull request that ships this ExecPlan, follow
these guidelines (the PR opened by the agent at the close of plan
authoring carries only this plan file; implementation PRs that follow
must reference back to this file):

- Title: `Plan: nested command tree behavioural fixtures (6.1.2)`.
- Body must mention this plan file
  (`docs/execplans/6-1-2-nested-command-tree-behavioural-fixtures.md`)
  and the roadmap entry `(6.1.2)`.
- Body must include a `## References` section that links to the lody
  session via the `LODY_SESSION_ID` environment variable.
- Mark the pull request as draft until the plan is approved; mark it
  ready-for-review once approval is recorded in `Decision Log` and the
  `Status` field above is updated to `APPROVED`.

## Revision history

This section records edits to this plan after the first draft. Each
entry states what changed, why it changed, and how it affects
remaining work.

- 2026-05-31 (planner): initial draft created.
- 2026-06-04 (implementer): recorded approval, moved status to
  `IN PROGRESS`, and added the initial navigation discovery before
  implementation edits.
- 2026-06-04 (implementer): recorded successful Milestone 0 validation
  and CodeRabbit review outcome.
