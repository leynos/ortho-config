# ADR-002: Replace `cucumber-rs` with `rstest-bdd` for behavioural tests

Date: 2025-11-09

Status: Proposed

## Context and problem statement

`ortho_config` and the `hello_world` example currently rely on the
`cucumber-rs` test harness (`ortho_config/tests/cucumber.rs` and
`examples/hello_world/tests/cucumber/mod.rs`) to execute the behavioural
specifications described in `docs/behavioural-tests.md` and
`docs/behavioural-testing-in-rust-with-cucumber.md`. The harnesses register a
custom async `#[tokio::main]` runner, wrap all state in monolithic `World`
structs, and require `[[test]]` targets with `harness = false`. This diverges
from the `rstest`-centric strategy documented in
`docs/rust-testing-with-rstest-fixtures.md`: fixtures defined for unit tests
cannot be injected into behavioural steps, and behaviour tests do not run under
`cargo test` without bespoke entrypoints. The existing approach also couples
`hello_world` to Tokio's process API even though most steps simply invoke the
compiled binary synchronously.

`rstest-bdd` (<https://github.com/leynos/rstest-bdd>) provides Given/When/Then
macros that integrate directly with `rstest`. Scenarios run as ordinary Rust
integration tests without disabling the test harness, scenario state is modeled
via fixtures or the lightweight `ScenarioState` derive, and features such as
data tables or docstrings map cleanly onto strongly typed parameters. Migrating
the suite aligns the behavioural layer with the rest of the workspace and
reduces the maintenance burden of a separate async runner and world model.

## Decision drivers

- **Single runner**: Execute every behavioural scenario under `cargo test`
  using the stock harness, so CI and local workflows remain consistent with the
  `make test` expectation.
- **Fixture reuse**: Reuse the `rstest` fixtures already described in
  `docs/rust-testing-with-rstest-fixtures.md`, rather than maintaining parallel
  Cucumber-specific helpers.
- **Lean dependencies**: Remove `cucumber`, `gherkin`, and the Tokio test
  runtime from dev-dependencies when they are only needed for the bespoke
  runner.
- **Typed step inputs**: Adopt `rstest-bdd`'s placeholder parsing,
  `#[step_args]` structs, docstring/data-table helpers, and `Slot<T>` scenario
  storage, so steps can be intention-revealing without re-parsing strings.
- **Tag-aware filtering**: Replace the bespoke `requires.yaml` filter in
  `hello_world` with `#[scenario(tags = …)]` / `scenarios!(…, tags = …)`, so
  compile-time filtering documents the dependency on optional Cargo features.
- **Better ergonomics**: Avoid the current pattern of re-exporting async helper
  methods purely to satisfy the Cucumber `World` trait; instead, share ordinary
  fixtures and helper structs across unit and behavioural tests.

## Considered options

### Option 1: Retain `cucumber-rs` and incrementally patch the harness

- **Pros**: No immediate code churn; existing world structures continue to
  operate.
- **Cons**: Keeps the out-of-band runner, prevents fixture reuse, and forces
  us to maintain async steps just to call synchronous helpers. Does not solve
  the dependency bloat or the mismatch with our documentation.
- **Viability**: Rejected; it entrenches the current drawbacks.

### Option 2: Wrap `cucumber-rs` inside an `rstest` facade

- **Pros**: Could keep feature files untouched while exposing limited fixtures
  via adapter traits.
- **Cons**: Requires writing and maintaining a bespoke bridge between the two
  frameworks, duplicates capabilities already implemented by `rstest-bdd`, and
  still depends on the async runner and `World` derive. Does not reduce
  dependencies or simplify the build.
- **Viability**: Rejected; the added complexity outweighs the benefit of
  postponing a clean migration.

### Option 3: Adopt `rstest-bdd` for all behavioural suites (recommended)

- **Pros**: Aligns the behavioural layer with the `rstest`
  recommendations, enables fixture sharing, removes the async harness, and lets
  us take advantage of features like `ScenarioState`, typed data tables, and
  compile-time tag filtering.
- **Cons**: Requires porting every step module, refactoring helper APIs to be
  synchronous, and updating documentation and developer workflows. We must also
  vet the maturity of `rstest-bdd` and stabilise its use across two crates.
- **Viability**: Preferred; the one-time migration cost unlocks structural
  simplifications and better long-term maintainability.

## Decision outcome

Proceed with Option 3. `rstest-bdd` already targets Rust 1.75, matches the
workspace edition, and keeps scenarios inside the ordinary test runner. The
migration preserves the `.feature` files but rewrites the harness so
behavioural coverage benefits from the same fixture system, lint setup, and CI
entrypoints as the rest of the workspace.

## Implementation plan

### Phase 0 – Discovery and parity baseline

1. Catalogue the current behavioural coverage: list every feature file,
   scenario outline, tag, docstring usage, and data table reference across
   `ortho_config/tests/features` and
   `examples/hello_world/tests/features/global_parameters.feature`.
2. Document which step modules depend on async operations, Tokio, or the
   current `World` struct internals (e.g., command spawning, `figment::Jail`).
3. Freeze the baseline by running `make test` and archiving the cucumber
   JSON/text output, so regressions can be compared after the port.

### Phase 1 – Introduce `rstest-bdd` scaffolding

1. Add dev-dependencies on `rstest-bdd` and `rstest-bdd-macros` to
   `Cargo.toml` in both `ortho_config` and `examples/hello_world`.
2. Create a shared `test_support::bdd` module (one per crate) that re-exports
   the macros, defines common fixtures (`#[fixture] fn jail() -> JailGuard`,
   command runners, temp directories), and provides helper traits for parsing
   placeholders, so helper code stays focused on the core extraction logic.
3. Ensure the new helpers compile by adding a canary test that binds a tiny
   `.feature` file via `#[scenario]` and `scenarios!`, proving that the macros
   integrate with our toolchain before touching the production suites.

### Phase 2 – Replace the cucumber worlds with fixtures and `ScenarioState`

1. Re-express the `World` structs as small `rstest` fixtures plus, where needed,
   `rstest_bdd::ScenarioState` structs backed by `Slot<T>` for mutable per-
   scenario data (`CommandResult`, `MergeComposer`, etc.).
2. Move helper methods (for spawning the CLI, managing env vars, composing
   declarative layers, or resetting `figment::Jail`) onto plain structs, so
   they can be shared between unit, integration, and behavioural tests.
3. Provide synchronous wrappers around async helpers. For example, introduce a
   `CommandHarness` fixture that owns a `tokio::runtime::Runtime` (or switches
   to `std::process::Command` where possible), so `#[given]` / `#[when]` /
   `#[then]` functions can remain synchronous as required by `rstest-bdd`.
4. Expose ergonomic constructors, so step modules do not need to know whether a
   fixture uses Tokio internally.

### Phase 3 – Port `ortho_config` step modules and features

1. Translate each module in `ortho_config/tests/steps` to the
   `rstest-bdd` macros:
   - Swap `use cucumber::{given, when, then};` for
     `use rstest_bdd_macros::{given, when, then};`.
   - Replace manual string parsing with typed placeholders and, where helpful,
     `#[step_args]` structs.
   - Update docstring consumers to accept a `docstring: String` parameter
     rather than referencing `cucumber::gherkin::Step`.
2. Bind scenarios to tests:
   - Use `scenarios!("tests/features", tags = "not @wip")` for features that
     only need global fixtures.
   - For modules that require custom fixtures (e.g., subcommand scenarios
     requiring CLI structs), create per-feature modules containing
     `#[scenario(path = "tests/features/…", name = "…")]` functions that
     request the necessary `rstest` fixtures.
   - Encode any tag-based exclusions (for example, YAML-only scenarios) as
     compile-time filters using the macro's `tags` argument.
3. Delete `ortho_config/tests/cucumber.rs` once all scenarios are bound and
   ensure `cargo test -p ortho_config --tests` executes them via the default
   harness.
4. Update the behavioural documentation files to describe the new runner,
   including fixture patterns and how to add new scenarios.
5. Run `make test`, `make lint`, and `make check-fmt`, fixing regressions or
   formatting issues introduced during the migration.

### Phase 4 – Port the `hello_world` example suite

1. Mirror the approach from Phase 3 within `examples/hello_world/tests`:
   - Turn `tests/cucumber/world` into a `fixtures` module that exposes the temp
     directory, environment map, declarative globals, and command assertions via
     `ScenarioState` + `Slot<T>` wrappers.
   - Rework async steps (`run_without_args`, `run_with_args`) to call into the
     synchronous command harness.
   - Replace `GherkinStep` docstring extraction with the `docstring: String`
     parameter.
2. Bind every scenario in `tests/features/global_parameters.feature` either via
   `scenarios!` or explicit `#[scenario]` functions when fixtures are required.
   Use two `scenarios!` invocations gated by `cfg(feature = "yaml")` to keep
   the `@requires.yaml` logic declarative.
3. Remove the bespoke tag-filtering logic and the `[[test]]` harness definition
   from `examples/hello_world/Cargo.toml` once all tests run under the stock
   harness.
4. Ensure `cargo test -p hello_world --tests` passes for every feature flag
   combination exercised in CI.

### Phase 5 – Clean-up and documentation alignment

1. Remove the `cucumber`, `gherkin`, and Tokio `process` dev-dependencies from
   `Cargo.toml` files and run `cargo update -p cucumber --workspace` to drop
   the crates from `Cargo.lock`.
2. Delete any unused helper modules that only existed to satisfy `World` trait
   bounds.
3. Update `docs/behavioural-tests.md`,
   `docs/behavioural-testing-in-rust-with-cucumber.md`, the `hello_world`
   README, and `docs/users-guide.md` to describe the `rstest-bdd` workflow.
4. Capture the migration in `CHANGELOG.md` and cross-link this ADR, so future
   contributors understand the rationale.
5. Add CI guidance (e.g., `make test` already covers the suite, but note the
   new dependency) and ensure developer onboarding docs reference the
   `rstest-bdd` user's guide for advanced usage.

### Acceptance criteria

- All `.feature` files are executed via `rstest-bdd`; no crate declares a
  `[[test]]` with `harness = false` for behavioural coverage.
- Behavioural fixtures rely on `rstest` (and `ScenarioState`) rather than
  bespoke `World` structs.
- Optional scenarios gated by `@requires.yaml` or similar tags compile-time
  filter correctly via macro `tags` expressions.
- Documentation and examples reference `rstest-bdd`, and the new workflow is
  reflected in `docs/roadmap.md` tasks.
