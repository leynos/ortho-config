# Developers guide

This guide documents the repository's testing workflow and behavioural testing
conventions for contributors.

## Quality gates

Run the required gates from the repository root before completing a change:

- `make check-fmt`
- `make lint`
- `make test`

The Make targets are the source of truth because they encode the exact
workspace policy (warnings denied, full-target linting, and the combined Rust +
Python test suites).

## Behavioural tests (current approach)

The behavioural suite for this repository currently lives in:

- `examples/hello_world/tests/features/`
- `examples/hello_world/tests/rstest_bdd/`

Key conventions in use:

- Gherkin scenarios define user-observable behaviour.
- Step definitions use `rstest-bdd` macros and `rstest` fixtures.
- Each scenario executes with an isolated harness fixture that owns a temporary
  working directory and scenario-scoped environment variables.
- YAML-specific behaviour is gated with `@requires.yaml` and Rust `cfg`
  feature checks so non-YAML builds remain deterministic.

## `rstest-bdd` v0.5.0 migration strategy

The migration plan is tracked in
`docs/rstest-bdd-v0-5-0-execplan.md`.

Planned strategy updates:

- Upgrade `rstest-bdd` and `rstest-bdd-macros` to `0.5.0`.
- Replace manual per-scenario Rust bindings with `scenarios!` autodiscovery
  where fixture wiring remains explicit.
- Increase type safety and clarity in step signatures with `StepArgs` (and
  typed table parsing where it improves readability).
- Enable compile-time step validation for `rstest-bdd-macros` so missing steps
  are detected during compilation.

Until that migration is implemented, contributors should follow the current
suite layout and conventions above.

## Adding or modifying behavioural coverage

When adding coverage:

- Prefer extending existing feature files only when the scenario describes the
  same user-facing behaviour family.
- Add new step definitions only when existing steps cannot express the intent
  clearly.
- Keep assertions in `Then` steps focused on observable command output or exit
  status.
- Keep scenario state local to fixtures; do not depend on scenario execution
  order.

After any behavioural test change, run all three quality gates.
