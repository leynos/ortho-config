# Prepare the v0.9.0 user documentation

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
and `Outcomes & retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Someone evaluating OrthoConfig should be able to reach a working, layered Rust
command-line application from the repository README within one screen. Someone
already using v0.8.0 should be able to identify every v0.9.0 change that
affects their code or runtime behaviour, distinguish required migrations from
useful opt-ins, and follow a tested example for each adoption path.

The completed change rewrites `README.md`, `docs/users-guide.md`, and
`docs/v0-9-0-migration-guide.md`. Every fenced example in the README and user's
guide is loaded from the Markdown by Rust integration tests through the same
marker-and-registry pattern used by Netsuke. Scenario tests then compile,
execute, parse, or otherwise exercise the exact extracted text. The result is
observable by running the focused documentation-example tests and the
repository's formatting, lint, test, spelling, Markdown, and Mermaid gates.

## Constraints

- Do not change OrthoConfig's public API or runtime behaviour. This work
  documents the v0.8.0-to-v0.9.0 delta and tests published examples.
- Treat tag `v0.8.0`, current `HEAD`, `CHANGELOG.md`, public Rust APIs, and
  existing integration tests as the release evidence. Do not infer a migration
  claim solely from roadmap or design prose.
- Keep `README.md` a short df12-style entry point. It must explain benefits,
  present a copyable quick start, and signpost deeper material instead of
  restating the user's guide.
- Keep all public prose in en-GB Oxford English and wrap Markdown prose at 80
  columns. Preserve upstream API spellings inside code and identifiers.
- Every fenced code block in `README.md` and `docs/users-guide.md` must have a
  unique `<!-- tested-example: ID -->` marker immediately before it. The loader
  must reject unmarked, duplicate, malformed, or unterminated examples.
- Tests must use the exact Markdown body loaded at runtime; copied fixture text
  does not satisfy the documentation contract.
- Behavioural checks must be proportional to the fence language: compile or run
  Rust programs, parse Cargo/TOML/JSON data, and execute documented CLI flows.
  `console` output may be checked against the command that produces it.
- Do not introduce a new production dependency. Existing `ortho_config`
  development dependencies may support the test harness.
- Keep each Rust source file at or below 400 lines and begin every new module
  with a `//!` comment.
- Document the new test helper's ownership and reuse policy in
  `docs/developers-guide.md`, because it is a repository abstraction shared by
  multiple documentation-example test binaries.
- Run Rust formatting, Clippy, Whitaker, tests, Markdown formatting and lint,
  spelling, Mermaid validation, and Makefile validation before committing.
- Use a scrutineer agent for the final deterministic gate run and summary.
- Create or update a draft pull request after a clean commit and push, following
  the `pr-creation` skill.

## Tolerances (exception triggers)

- Scope: stop and escalate if the implementation needs more than 12 tracked
  files, more than 2,500 net new lines, or changes outside documentation,
  documentation-example tests, test-only manifest configuration, and the
  documentation index.
- Interface: stop and escalate if an example cannot be made correct without a
  public API or runtime behaviour change.
- Dependencies: stop and escalate before adding any external dependency.
- Precedent: stop and escalate if faithfully adapting Netsuke's marker loader
  requires weakening the rule that every fence is registered.
- Iterations: stop and escalate if the focused documentation-example suite
  still fails after five distinct repair attempts for the same failure class.
- Full gates: stop and report if the same full-gate failure persists across
  three attempts after focused checks pass.
- Ambiguity: stop and present options if evidence supports two materially
  different descriptions of a public compatibility contract.

## Risks

- Risk: current documentation contains many overlapping and stale examples, so
  preserving all of them would make behavioural coverage unwieldy. Severity:
  high. Likelihood: high. Mitigation: rewrite around a smaller set of complete
  worked journeys, while covering every supported common task in prose and
  linking specialist tools.

- Risk: examples that spawn nested Cargo commands may deadlock on the outer
  build directory or make tests unacceptably slow. Severity: medium.
  Likelihood: medium. Mitigation: assemble a single temporary example workspace
  per test run, use a separate target directory, prefer `--offline`, and
  compile related examples together.

- Risk: `v0.9.0` is not yet tagged and manifests still report `0.8.0`, so
  copyable dependency declarations cannot resolve the unreleased version from
  crates.io during tests. Severity: medium. Likelihood: high. Mitigation:
  document `0.9.0` as the target release, then replace that dependency with an
  absolute path to the current crate only inside the temporary test workspace.
  Assert that the published fence still says `0.9.0`.

- Risk: broad Markdown formatting may touch unrelated long-form documents.
  Severity: low. Likelihood: medium. Mitigation: inspect formatter output, keep
  only necessary changes, and isolate unavoidable formatter-only drift if
  repository policy requires it.

- Risk: the current root and crate READMEs may overlap.
  Severity: medium. Likelihood: high. Mitigation: keep this task scoped to the
  root `README.md`, and verify that its links and claims do not contradict
  `ortho_config/README.md`. Escalate rather than silently expanding scope to a
  second README.

## Progress

- [x] (2026-08-09 15:10Z) Confirmed a clean `v0-9-0-prep` baseline at
  `2ff35cf` and compared `v0.8.0..HEAD`.
- [x] (2026-08-09 15:10Z) Used GrepAI's healthy `Projects` index for
  configuration, derive, localization, injected-environment, and Netsuke
  documentation-example discovery.
- [x] (2026-08-09 15:10Z) Registered the worktree with Leta and recorded that
  Rust symbol queries remained empty after a language-server restart.
- [x] (2026-08-09 15:10Z) Audited Netsuke's `tested-example` loader, registry,
  parser failure tests, behaviour checks, and end-to-end tests.
- [x] (2026-08-09 15:10Z) Drafted this ExecPlan and stopped at its approval
  gate.
- [x] (2026-08-09 15:39Z) Obtained explicit approval and changed status to
  `IN PROGRESS`.
- [x] (2026-08-09 16:17Z) Added the strict documentation-example loader,
  malformed-input tests, closed registry, and the expected red failure on the
  first unmarked README fence.
- [x] (2026-08-09 16:17Z) Rewrote the README and user's guide around 23 marked,
  behaviourally checked examples: 11 Rust, 7 TOML, 3 console, 1 JSON, and 1
  YAML.
- [x] (2026-08-09 16:17Z) Completed the v0.9.0 impact inventory and worked
  migrations.
- [x] (2026-08-09 16:17Z) Updated the documentation index and recorded the
  documentation-test helper's ownership and reuse boundary.
- [x] (2026-08-09 16:17Z) Passed focused default/all-feature example tests,
  formatting, Markdown lint, spelling, Mermaid, Makefile validation, and
  `git diff --check`.
- [x] (2026-08-09 16:32Z) Passed rustdoc, Clippy, Whitaker, the complete
  all-target/all-feature Rust suite, and the Python test suite.
- [x] (2026-08-09 17:16Z) Committed the implementation, received a green
  independent scrutineer report, pushed the branch, and opened draft pull
  request [#422](https://github.com/leynos/ortho-config/pull/422).
- [x] (2026-08-11 20:16Z) Validated the completed registry inventory at 23
  examples: 11 Rust, 7 TOML, 3 console, 1 JSON, and 1 YAML.

## Surprises & discoveries

- Observation: the existing migration guide says the release is additive, but
  `ConfigDiscovery::load_first` now returns `Err` when candidates were found
  and all failed. YAML also moved to YAML 1.2 parsing with strict booleans and
  duplicate-key rejection. Evidence: `CHANGELOG.md` lines 62-70 and the
  implementation in `ortho_config/src/discovery/load.rs`. Impact: both changes
  require prominent compatibility guidance and before/after examples.

- Observation: the release surface is considerably wider than the current
  migration guide. It includes dependency aliasing, dependency re-exports,
  forwarded format features, derive-controlled discovery, recursive subcommand
  documentation, public localization helpers, agent-context structures,
  environment injection, tracing, and optional metrics. Evidence:
  `CHANGELOG.md` lines 7-53 and the public re-exports in
  `ortho_config/src/lib.rs`. Impact: the guide needs a complete impact matrix
  rather than three observability-focused sections.

- Observation: Netsuke does not merely compile copied snippets. Its loader
  reads marked fences from public Markdown, rejects every unmarked fence,
  checks a closed expected-ID registry, and gives each example an appropriate
  behavioural contract. Evidence: `tests/documentation_examples/mod.rs`,
  `tests/documentation_examples_loader_tests.rs`,
  `tests/documentation_examples_tests.rs`, and
  `tests/documentation_examples_e2e_tests.rs` in the Netsuke repository.
  Impact: OrthoConfig will adapt that structure and keep syntax parsing
  separate from scenario behaviour.

## Decision log

- Decision: organize the user's guide around CLI developer jobs rather than API
  inventory order. Rationale: the requested outcome is to make common CLI tasks
  easy. Readers should first build and run a layered CLI, then add files,
  environment values, subcommands, validation, localization, testing,
  observability, and generated help as their application grows. Date/Author:
  2026-08-09 15:10Z / Codex.

- Decision: classify migration items as `Required`, `Review`, `Recommended`, or
  `Optional`. Rationale: these labels distinguish breaks, semantic
  compatibility checks, valuable new usage patterns, and low-cost opt-ins more
  clearly than a binary breaking/non-breaking table. Date/Author: 2026-08-09
  15:10Z / Codex.

- Decision: use a strict `tested-example` marker and closed example registry,
  matching Netsuke's approach. Rationale: a strict loader makes an untested
  fence a test failure and prevents documentation drift from silently bypassing
  the behavioural suite. Date/Author: 2026-08-09 15:10Z / Codex.

- Decision: use scoped Git and exact-text navigation after Leta failed to
  return Rust symbols. Rationale: Leta was installed, the worktree was
  registered, and the server was restarted; empty results could not serve as
  evidence. GrepAI remained healthy and supplied the intent-based navigation
  requested by the user. Date/Author: 2026-08-09 15:10Z / Codex.

## Outcomes & retrospective

The public surface now has three distinct levels: a concise README for first
contact, a task-oriented user's guide, and an impact-labelled migration guide.
The strict registry covers 23 fences: eleven Rust programs, seven TOML
manifests or configuration files, three console flows, one JSON document, and
one YAML file. Rust programs compile and run unchanged, console flows execute
their underlying commands, and data formats use their production parsers.
Detailed API signatures remain delegated to rustdoc, while the full
multi-module composition remains delegated to `examples/hello_world`. The
independent scrutineer reran every required gate against the staged
implementation and reported no findings. Draft pull request #422 contains the
approved plan and completed change.

## Context and orientation

The root `README.md` is currently a 530-line reference that duplicates much of
the 1,631-line `docs/users-guide.md` and still uses `0.8.0` in installation
examples. `docs/v0-9-0-migration-guide.md` is only 173 lines and covers
environment injection, discovery telemetry, metrics, and redaction, while
`CHANGELOG.md` records a broader public delta.

`ortho_config/` is the runtime library crate. Its integration tests live in
`ortho_config/tests/`, which already has `anyhow`, `rstest`, `tempfile`, TOML,
and the current crate available as development dependencies. This is the right
home for documentation-example tests because the examples exercise the
published runtime and derive APIs.

The Netsuke precedent uses an HTML comment of the form
`<!-- tested-example: stable-id -->` immediately before every fenced block. A
shared loader returns the fence language and exact body, rejects malformed
documents, and enforces unique identifiers. Separate integration binaries test
the parser itself, a closed identifier registry, command behaviour, and
end-to-end effects.

The existing `examples/hello_world/` crate remains the full application
reference. The rewritten public documents should use smaller examples for
learning, then link to Hello World when localization, generated help, agent
context, or multi-module application structure would overwhelm the first
journey.

## Plan of work

Stage A is complete when this draft is approved. Update its status to
`IN PROGRESS`, add it to `docs/contents.md`, and preserve the approval in the
decision log.

Stage B establishes the red documentation contract. Add
`ortho_config/tests/documentation_examples/mod.rs` for strict Markdown loading,
`ortho_config/tests/documentation_examples_loader_tests.rs` for malformed-input
and property tests, and `ortho_config/tests/documentation_examples_tests.rs`
for the expected registry and scenario contracts. The first focused run must
fail because the current README and user's guide contain unmarked fences and do
not match the new registry. Record that failure here before editing the public
documents.

Stage C rewrites the three public documents. Replace the root README with the
df12 structure: tagline, why, installation, one minimal full program, one run,
feature summary, and prominent resources. Rewrite the user's guide as worked
journeys: first layered CLI; naming and precedence; configuration discovery and
formats; collections and nested values; subcommands; validation and errors;
hermetic tests; localization; tracing and metrics; generated documentation and
agent context; migration and troubleshooting. Each fence receives a stable
marker and a test that consumes its exact body. Rewrite the migration guide
with an impact matrix and sections for every public change recorded in the
v0.8.0-to-HEAD evidence.

Stage D completes the green behavioural implementation. Tests create temporary
Cargo projects from paired dependency and Rust fences, substitute only the
current local crate path, run them offline, exercise their CLI/file/environment
flows, and compare results with any documented output fences. Non-Rust fences
are parsed or used as live inputs. Refactor helpers only after the focused
suite passes, keeping files below 400 lines.

Stage E records the internal convention in `docs/developers-guide.md`, links
the migration guide and this ExecPlan from `docs/contents.md`, formats all
changed files, and runs the repository gates. After focused validation passes,
commit the complete logical change. Then ask the scrutineer to run and
summarize the deterministic gates without editing files. Repair any valid
failures, rerun the relevant gates, push the branch, and create or update the
draft pull request.

## Concrete steps

Run all commands from the repository root.

After approval, begin with the red stage:

```bash
cargo test -p ortho_config --test documentation_examples_loader_tests
cargo test -p ortho_config --test documentation_examples_tests
```

The loader tests should pass. The second command should fail with a precise
message such as:

```plaintext
README.md:<line> fence is missing a tested-example marker
```

After rewriting and implementing scenario checks, run:

```bash
cargo test -p ortho_config --test documentation_examples_loader_tests
cargo test -p ortho_config --test documentation_examples_tests
cargo test -p ortho_config --doc
```

All three commands must pass. Then format and validate the documentation:

```bash
make fmt
make check-fmt
make markdownlint
make spellcheck
make nixie
mbake validate Makefile
git diff --check
```

Finally run the repository Rust gates, using a spacious target directory if a
clean rebuild is needed:

```bash
make lint
make test
```

The scrutineer repeats the deterministic gates after the candidate commit and
returns a structured pass/fail summary. Do not create the pull request until
the worktree is clean and every required gate passes.

## Validation and acceptance

The change is accepted when all of the following are observable:

- A new reader can copy the README dependency and Rust examples, run the
  documented command, and observe the documented value without consulting the
  user's guide.
- The README explains layered configuration's benefits and links to the user's
  guide, v0.9.0 migration guide, Hello World example, API documentation,
  changelog, design, roadmap, and contributing guidance.
- The user's guide provides a complete worked path for common CLI application
  needs without requiring readers to reverse-engineer the derive macro.
- The migration guide accounts for dependency aliasing and re-exports,
  forwarded format features, discovery customization, recursive subcommand
  documentation, localized parsing, agent context, environment injection,
  discovery telemetry and metrics, `load_first` error semantics, YAML 1.2
  semantics, improved `extends` errors, and generated documentation comments.
- Every fenced example in `README.md` and `docs/users-guide.md` has one unique
  marker, appears in the expected registry, and has an appropriate executable
  or parse-level behavioural assertion.
- The red test fails before the documentation rewrite for an unmarked fence and
  the green test passes afterwards.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
  `make spellcheck`, `make nixie`, `mbake validate Makefile`, and
  `git diff --check` all pass.
- The scrutineer's independent summary reports no failures.
- The committed branch is pushed and has a draft pull request whose description
  covers the full branch and links every mentioned file.

## Idempotence and recovery

The loader and tests are read-only apart from temporary directories, so focused
runs are repeatable. Temporary example crates must clean up through
`tempfile::TempDir`. Nested Cargo builds use a separate target directory so a
failed run can be retried without corrupting the outer workspace build.

Markdown formatting is repeatable. Inspect `git diff` after `make fmt`; if it
changes unrelated files, restore only known formatter drift after verifying
those paths were clean at baseline.

## Artefacts and notes

The release evidence starts with:

```plaintext
v0.8.0..HEAD
2ff35cf Chore(deps): Bump proc-macro2 from 1.0.106 to 1.0.107 (#406)
d3c9bdf Add an injectable environment source for configuration discovery
...
```

The strict marker form adapted from Netsuke is:

```html
<!-- tested-example: guide-first-layered-cli -->
```

The marker applies to the immediately following fenced block, with no
intervening blank line. Identifiers are unique across both public documents.

## Interfaces and dependencies

The shared test module in `ortho_config/tests/documentation_examples/mod.rs`
owns Markdown loading, marker parsing, example lookup, and source diagnostics.
`documentation_examples/workspace.rs` owns temporary-project assembly. Only
documentation-example integration test binaries may call them. They must not
become production APIs or general Markdown tooling.

Define a `DocumentedExample` query value containing `id`, `language`, `body`,
and source location. Define `load_documented_examples()` and
`documented_example(id)` as fallible queries. Keep process execution in
separate scenario helpers so parsing remains pure and independently testable.

Use `anyhow` for test-only context, `rstest` for behaviour matrices, `proptest`
for marker/parser invariants, `tempfile` for isolated workspaces, and
`std::process::Command` for nested Cargo or documented CLI execution. These are
already available to `ortho_config` tests; no production dependency changes are
permitted.

Revision note: the user approved the plan on 2026-08-09. The implementation
then replaced the three public documents, added the strict executable example
contract, passed all required gates, and opened draft pull request #422. The
plan is complete. On 2026-08-11, the completed inventory was confirmed at 23
examples and synchronized across the progress and outcome sections; this
changes the recorded counts, not the plan's status or remaining work.
