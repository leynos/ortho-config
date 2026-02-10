# Developers guide

This guide documents how contributors work with tests in this repository. It
focuses on behavioural tests because they span multiple crates and have the
highest maintenance cost when patterns drift.

## Current testing strategy

The workspace runs one unified test workflow via Make targets:

- `make check-fmt`
- `make lint`
- `make test`

These are required quality gates for code changes. Behavioural coverage runs
inside the standard Rust test harness, not a bespoke test runner.

## Behavioural test layout

Behavioural suites live in crate-local integration test targets:

- `ortho_config/tests/rstest_bdd/`
- `examples/hello_world/tests/rstest_bdd/`

Feature files are in:

- `ortho_config/tests/features/`
- `examples/hello_world/tests/features/`

Step definitions use `rstest-bdd` macros (`#[given]`, `#[when]`, `#[then]`) and
consume `rstest` fixtures. Scenario-local mutable state is modelled with
fixtures and `Slot<T>` values inside `#[derive(ScenarioState)]` structs.
Cross-scenario mutable sharing is forbidden; use `#[once]` only for expensive,
effectively read-only infrastructure.

## `rstest-bdd` v0.5.0 migration strategy

Status: adopted. See `docs/execplans/adopt-rstest-bdd-v0-5-0.md` for execution
history and rationale.

Migration guidance for contributors:

- Upgrade workspace pins to `rstest-bdd = "0.5.0"` and
  `rstest-bdd-macros = "0.5.0"`.
- Scenario functions must return `()` or explicit unit results
  (`Result<(), E>` / `rstest_bdd::StepResult<(), E>`). Avoid return type
  aliases in scenario signatures.
- Prefer `scenarios!(..., fixtures = [...], tags = ...)` for large feature
  bindings to reduce handwritten wrapper boilerplate.
- Prefer descriptive placeholder names over generic `{string}` placeholders so
  step signatures remain explicit and compile-time checked.
- Prefer underscore-prefixed fixture names only when no step resolves that
  fixture by name.
- Remove file-wide lint suppressions used only for historical generated-fixture
  warnings; retain only narrow, item-level `#[expect(...)]` annotations when
  still required.
- Keep scenario isolation as the default and reserve `#[once]` for shared
  infrastructure only.
- If a sync step needs async bridging, use
  `rstest_bdd::async_step::sync_to_async`.
- Keep tag names filter-friendly (`@name_part` style). Avoid dots in tag names
  used with `tags = "..."` expressions.

## Adding or changing behavioural tests

When adding scenarios or steps:

1. Add or edit the `.feature` file first.
2. Implement or update step definitions under the matching `tests/rstest_bdd`
   module.
3. Bind scenarios using `scenarios!` where possible; use explicit `#[scenario]`
   only when a feature needs bespoke fixtures or per-scenario control.
4. Keep assertions user-observable (`Then` steps) and avoid asserting private
   internals unless the behaviour cannot be observed externally.
5. Run the full required quality gates before finalizing.

## Command checklist

Run from repository root:

    set -o pipefail; make check-fmt 2>&1 | tee /tmp/make-check-fmt.log
    set -o pipefail; make lint 2>&1 | tee /tmp/make-lint.log
    set -o pipefail; make test 2>&1 | tee /tmp/make-test.log

For targeted behavioural debugging:

    cargo test -p ortho_config --tests
    cargo test -p hello_world --tests --all-features
