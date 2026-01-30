# Build cargo-orthohelp bridge pipeline

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETED

PLANS.md is not present in the repository root, so this ExecPlan follows the
standard execplans format.

## Purpose / big picture

Deliver the `cargo-orthohelp` bridge pipeline described in
`docs/cargo-orthohelp-design.md` so a consumer can generate per-locale
intermediate representation (IR) JSON from a real crate without embedding
tooling in the application binary. Success is observable when a fixture crate
produces locale-specific IR JSON in `--out-dir` and the `--cache` and
`--no-build` modes behave as documented.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not workarounds.

- Follow the bridge pipeline described in
  `docs/cargo-orthohelp-design.md` (sections 6.2 and 6.3) and keep the tool
  clap-agnostic; do not scrape `--help` output or depend on `clap_mangen`.
- Use `cap_std`/`cap_std::fs_utf8` and `camino` for filesystem access and
  paths in the new tooling.
- Tests must use `rstest` fixtures and `rstest-bdd` v0.4.0 for behavioural
  coverage, following the guidance in `docs/rstest-bdd-users-guide.md`.
- Avoid direct environment mutation in tests; use `test_helpers::env` guards
  or dependency injection as documented in
  `docs/reliable-testing-in-rust-via-dependency-injection.md`.
- Documentation updates must use en-GB spelling and wrap paragraphs and
  bullets at 80 columns per `docs/documentation-style-guide.md`.
- Every new module must start with a `//!` module-level comment.
- Do not use `#[allow(...)]`; if a lint exception is unavoidable, use
  `#[expect(..., reason = "...")]` with a narrow scope.

If satisfying the objective requires violating a constraint, do not proceed.
Document the conflict in `Decision Log` and escalate.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Scope: stop if implementation requires changes to more than 18 files or
  more than 1,200 net lines of code.
- Interface: stop if a public API in `ortho_config` or
  `ortho_config_macros` must change in a breaking way.
- Dependencies: stop if more than three new external crates are required.
- Iterations: stop if tests still fail after two fix attempts.
- Ambiguity: stop if multiple valid interpretations of “per-locale IR JSON”
  or cache key composition remain after reviewing the design doc.

## Risks

Known uncertainties that might affect the plan. Identify these upfront and
update as work proceeds. Each risk should note severity, likelihood, and
mitigation or contingency.

- Risk: The design doc does not define the exact schema for the localized IR
  JSON output, which could lead to incompatible tooling later. Severity: high.
  Likelihood: medium. Mitigation: define the output schema and file naming in
  `docs/cargo-orthohelp-design.md` before coding.
- Risk: `cargo metadata` discovery may not uniquely identify a binary when
  multiple bins are present. Severity: medium. Likelihood: medium. Mitigation:
  enforce explicit `--bin`/`--lib` selection and provide a clear error message.
- Risk: Updating to `rstest-bdd` v0.4.0 could require adjustments in existing
  tests or macros. Severity: medium. Likelihood: medium. Mitigation: isolate
  the version bump to workspace dev-dependencies first and fix any compilation
  failures.
- Risk: Cache invalidation is underspecified and might reuse stale IR.
  Severity: medium. Likelihood: medium. Mitigation: document the cache key
  inputs (crate fingerprint, tool version, macro version) and include tests for
  invalidation behaviour.

## Progress

Use a list with checkboxes to summarize granular steps. Every stopping point
must be documented here, even if it requires splitting a partially completed
task into two (“done” vs. “remaining”). This section must always reflect the
actual current state of the work.

- [x] (2026-01-26 00:00Z) Draft ExecPlan created.
- [x] (2026-01-26 00:10Z) Plan approved; implementation started.
- [x] (2026-01-26 00:30Z) Survey existing crate layout, docs, and tests for
  cargo-orthohelp hooks.
- [x] (2026-01-26 00:40Z) Define the localized IR JSON schema and file naming
  in the design doc.
- [x] (2026-01-26 02:05Z) Implement cargo-orthohelp metadata discovery, bridge
  build, cache, and locale resolution.
- [x] (2026-01-26 02:20Z) Add rstest unit tests and rstest-bdd behavioural
  coverage.
- [x] (2026-01-26 02:40Z) Update docs and examples, mark roadmap item 4.1.1 as
  done.
- [x] (2026-01-26 02:55Z) Run `make check-fmt`, `make lint`, and `make test`
  (plus doc checks) with logged output.

## Surprises & Discoveries

Unexpected findings during implementation that were not anticipated as risks.
Document with evidence so future work benefits.

- Observation: The first `make test` run timed out at the default 120s
  command timeout; rerunning with a higher timeout completed successfully.
  Evidence: `/tmp/orthohelp-test.log`. Impact: update timeouts for long-running
  test runs.

## Decision log

Record every significant decision made while working on the plan. Include
decisions to escalate, decisions on ambiguous requirements, and design choices.

- Decision: Localized IR output mirrors the base IR but resolves IDs to text,
  includes `locale`, and emits JSON under `<out>/ir/<locale>.json`. Rationale:
  Keeps generators simple and avoids mixing identifier resolution into later
  stages. Date/Author: 2026-01-26 (assistant).
- Decision: Consumer Fluent resources are loaded from
  `locales/<locale>/*.ftl` in lexicographic order, falling back to
  `ortho_config` defaults or consumer-only bundles when defaults are missing.
  Rationale: Matches existing example layout while supporting future naming
  conventions. Date/Author: 2026-01-26 (assistant).

## Outcomes & retrospective

Summarize outcomes, gaps, and lessons learned at major milestones or at
completion. Compare the result against the original purpose. Note what would be
done differently next time.

- Outcome: The bridge pipeline now emits per-locale IR JSON for the fixture
  crate, supports cache reuse and `--no-build`, and is covered by rstest unit
  tests plus rstest-bdd behavioural scenarios. Documentation, examples, and the
  roadmap entry were updated to match the new workflow.

## Context and orientation

The IR schema and pipeline requirements live in
`docs/cargo-orthohelp-design.md`. The IR types and `OrthoConfigDocs` trait are
already implemented in `ortho_config/src/docs`, and localization helpers live
in `ortho_config/src/localizer`. The roadmap entry for this work is in
`docs/roadmap.md` under 4.1.1. There is currently no `cargo-orthohelp` crate in
this workspace, so the pipeline must be introduced as a new binary crate and
added to the workspace in `Cargo.toml`.

Behavioural tests live under `ortho_config/tests/rstest_bdd` and
`examples/hello_world/tests/rstest_bdd`, using feature files under
`*/tests/features`. Unit tests should use `rstest` fixtures and follow the
fixtures guidance in `docs/rust-testing-with-rstest-fixtures.md`. When tests
need environment mutation, use the `test_helpers::env` lock/guards described in
`docs/reliable-testing-in-rust-via-dependency-injection.md` rather than direct
`std::env` calls. Documentation updates must follow
`docs/documentation-style-guide.md` and `docs/rust-doctest-dry-guide.md`.

## Plan of work

Stage A: Review the design doc pipeline sections, existing IR types, and the
localizer application programming interface (API). Decide how the tool will
emit per-locale IR JSON (schema and file naming) and record that decision in
`docs/cargo-orthohelp-design.md` before writing code.

Stage B: Scaffold the new `cargo-orthohelp` crate and command-line interface
(CLI) parser. Add a new workspace member (package name `cargo-orthohelp`) with
a `main.rs` that parses the CLI flags described in section 6.1 of the design
doc. Use a small internal module layout (for example `cli`, `metadata`,
`bridge`, `cache`, `locale`, `output`) with module-level `//!` comments. Add or
update workspace `[workspace.dependencies]` entries only as needed (likely
`cargo_metadata`, `serde_json`, `camino`, `cap_std`, and a hashing crate).

Stage C: Implement the bridge pipeline.

- Metadata discovery: use `cargo_metadata` to load the workspace, resolve the
  selected package, and extract `package.metadata.ortho_config` defaults
  (`root_type`, `locales`, `module_name`, and man settings where relevant).
  Validate `--bin`/`--lib` selection and surface actionable errors.
- Ephemeral bridge build: generate a tiny crate under
  `target/orthohelp/<hash>/` that depends on the target crate and
  `ortho_config`. Its `main.rs` should call
  `<Root as OrthoConfigDocs>::get_doc_metadata()` and print JSON to stdout.
  Build it with `cargo build` using the same target directory, then execute the
  binary and capture the JSON.
- Caching: compute a cache key from the crate fingerprint, tool version, and
  macro version, then write `ir.json` under the bridge directory. When
  `--cache` is passed, reuse the cached IR if the key matches. When
  `--no-build` is passed, skip building and error if the cached IR is missing
  or invalid.
- Locale resolution: for each requested locale (CLI `--locale` or
  `--all-locales`), build a `FluentLocalizer` and resolve IDs to strings,
  emitting a per-locale IR JSON file in `--out-dir` using the schema defined in
  Stage A.

Stage D: Tests and documentation. Add rstest unit tests for metadata discovery,
cache behaviour, and locale resolution. Add rstest-bdd behavioural tests that
run the CLI against a fixture crate, verifying that `--cache` and `--no-build`
produce per-locale JSON under `--out-dir`. Update `docs/users-guide.md` with
the new CLI usage, update `examples/hello_world` to show the API consumer
workflow, and mark roadmap item 4.1.1 as done. Record any design decisions in
`docs/cargo-orthohelp-design.md`.

Stage E: Validation. Run formatting, linting, tests, and documentation checks
via Makefile targets with log capture, then confirm the behavioural test
outputs align with the acceptance criteria.

## Concrete steps

1. Inspect existing IR and localizer modules and identify reuse points:

   - `rg -n "OrthoConfigDocs" ortho_config/src`
   - `rg -n "FluentLocalizer" ortho_config/src`

2. Create the new crate and module layout under `cargo-orthohelp/`, update
   `Cargo.toml` workspace membership, and wire CLI parsing.

3. Implement metadata discovery via `cargo_metadata` and derive defaults from
   `package.metadata.ortho_config` when CLI flags are omitted.

4. Implement bridge generation, caching, and locale resolution per the design
   doc, then define output paths for per-locale JSON in `--out-dir`.

5. Add rstest unit tests and rstest-bdd behavioural tests with a fixture crate
   under `tests/fixtures/orthohelp-fixture` (or equivalent), ensuring the
   fixture can be built and executed by the new tool.

6. Update docs and examples:

   - `docs/cargo-orthohelp-design.md` (decisions on IR output and caching)
   - `docs/users-guide.md`
   - `examples/hello_world` (README and/or source)
   - `docs/roadmap.md` (mark 4.1.1 as done)

7. Run validation commands with logging:

   - `set -o pipefail && make fmt 2>&1 | tee /tmp/orthohelp-fmt.log`
   - `set -o pipefail && make markdownlint 2>&1 | tee /tmp/orthohelp-md.log`
   - `set -o pipefail && make nixie 2>&1 | tee /tmp/orthohelp-nixie.log`
   - `set -o pipefail && make check-fmt 2>&1 | tee /tmp/orthohelp-check.log`
   - `set -o pipefail && make lint 2>&1 | tee /tmp/orthohelp-lint.log`
   - `set -o pipefail && make test 2>&1 | tee /tmp/orthohelp-test.log`

## Validation and acceptance

Acceptance is met when:

- `cargo-orthohelp` can locate a fixture crate, discover its root type, and
  emit per-locale IR JSON files into `--out-dir`.
- Running the tool with `--cache` reuses cached IR when valid, and
  `--no-build` refuses to proceed without a cached IR.
- The per-locale IR JSON schema and file naming are documented in
  `docs/cargo-orthohelp-design.md` and reflected in the output files.
- Unit tests (rstest) cover metadata discovery, cache key behaviour, and
  locale resolution.
- Behavioural tests (rstest-bdd v0.4.0) cover the CLI end-to-end flow using
  the fixture crate and verify generated files.
- `docs/users-guide.md` and `examples/hello_world` explain how to use the new
  tool from a consumer’s perspective.
- `docs/roadmap.md` entry 4.1.1 is marked done.
- `make check-fmt`, `make lint`, and `make test` succeed, along with
  documentation checks (`make fmt`, `make markdownlint`, `make nixie`).

## Idempotence and recovery

All steps are safe to re-run. If a test or lint step fails, inspect the
corresponding `/tmp/orthohelp-*.log`, fix the issue, and re-run the failed
command. Cached IR can be removed by deleting `target/orthohelp/<hash>/` and
re-running the tool without `--no-build`.

## Artifacts and notes

Capture a short example of the per-locale JSON output in a test fixture (kept
under 120 columns) to validate schema expectations, for example a file named
`tests/fixtures/orthohelp-fixture/expected/en-GB.json`.

## Interfaces and dependencies

The tool should expose a binary named `cargo-orthohelp` with CLI flags matching
section 6.1 of `docs/cargo-orthohelp-design.md`. The internal modules should
expose small, testable units such as:

- `cargo_orthohelp::metadata::WorkspaceSelection` to resolve package/bin/root
  type inputs from CLI or `Cargo.toml` metadata.
- `cargo_orthohelp::bridge::BridgeBuilder` that writes the ephemeral crate and
  returns the path to the built binary and cached IR.
- `cargo_orthohelp::cache::CacheKey` that hashes the crate fingerprint, tool
  version, and macro version.
- `cargo_orthohelp::locale::resolve_ir_for_locale` that takes a `DocMetadata`
  and a `Localizer` and returns a resolved, per-locale IR struct.

If new dependencies are required, prefer:

- `cargo_metadata` for metadata discovery.
- `serde_json` for JSON output.
- `camino` and `cap_std` for paths and filesystem operations.
- A small hashing crate (for example `sha2`) for cache keys.

Document any additional dependencies and rationale in `docs/design.md` if they
impact architectural decisions.

## Revision note (required when editing an ExecPlan)

Initial draft created for roadmap item 4.1.1 (bridge pipeline).

Revision 1 (2026-01-26): marked plan as in progress after approval.
