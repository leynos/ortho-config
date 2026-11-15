//! Aliased `Result` steps driven by the generated scenario runner.
//!
//! The sibling [`step_return_regression`](super::step_return_regression)
//! module reaches the runner's `insert_value` call by hand so that each
//! classification branch can be asserted in isolation. That isolation is
//! useful, but it also means the module cannot notice a defect in the code
//! that *calls* it. These bindings close that gap: they run the real
//! `#[scenario]`-generated body, so the step loop, the override insertion and
//! the deferred failure panic are the ones a feature file actually exercises.
//!
//! Everything the runner needs lives here on purpose. An injected payload is
//! recorded only when exactly one fixture shares its `TypeId`, so the payload
//! type, the step functions that produce it, the fixture that seeds it, and
//! the scenario bindings that consume it must all resolve to the same `Level`.
//! Declaring a second `Level` in the regression module would make
//! `StepContext::insert_value` report `NoMatch` and silently drop every
//! payload, which is why that module imports this one's type instead.
//!
//! Each binding names its scenario explicitly. Without a selector the macro
//! defaults to the feature file's first scenario, so several bindings over
//! one file would otherwise all run the same case.

use anyhow::Result;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then};

/// A non-unit payload carried out of a step as an override.
///
/// `pub(super)` so the regression module can hold the same type where its
/// focused dispatch assertions downcast the injected payload.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Level(pub(super) u8);

/// The level seeded into every passing scenario.
///
/// The scenarios that expect an injected level never use this value, so a
/// step that failed to record its payload would leave the assertion reading
/// the fixture and fail on the number. The unit-step scenario expects this
/// value unchanged, which is what proves that `Ok(())` recorded no override.
#[fixture]
fn level() -> Level {
    Level(5)
}

/// Mirrors this crate's own step return alias exactly.
///
/// Written as `anyhow::Result<T>` rather than `Result<T, E>` on purpose:
/// `anyhow::Result<T>` defaults its error type to `anyhow::Error`, so the
/// alias under test is the one the crate's steps actually return.
type StepAlias<T> = anyhow::Result<T>;

/// Mirrors `cargo-orthohelp`'s boxed error type.
type BoxedStepError = Box<dyn std::error::Error + Send + Sync>;

/// Mirrors `cargo-orthohelp`'s `StepResult<T>` alias.
type BoxedStepResult<T> = Result<T, BoxedStepError>;

/// An alias whose `Err` must fail the scenario rather than pass silently.
#[given("a step return alias reports failure")]
fn alias_reports_failure() -> StepAlias<()> {
    Err(anyhow::anyhow!("aliased step failed"))
}

/// An alias whose `Ok(())` must produce no override payload.
#[given("a step return alias succeeds with no payload")]
fn alias_succeeds_empty() -> StepAlias<()> {
    Ok(())
}

/// An alias whose `Ok` payload must be injected for later steps to read.
#[given("a step return alias reports a level of {level:u8}")]
fn alias_reports_level(level: u8) -> StepAlias<Level> {
    Ok(Level(level))
}

/// A step using the boxed-error alias that returns `Err`.
#[given("a boxed-error step alias reports failure")]
fn boxed_alias_reports_failure() -> BoxedStepResult<()> {
    Err("boxed aliased step failed".into())
}

/// A step using the boxed-error alias that returns `Ok` with a payload.
#[given("a boxed-error step alias reports a level of {level:u8}")]
fn boxed_alias_reports_level(level: u8) -> BoxedStepResult<Level> {
    Ok(Level(level))
}

/// Reads back whatever the preceding aliased step injected.
///
/// The parameter name `level` is the implicit fixture name, so this step
/// observes the override the runner recorded under that name. The scenario's
/// `level: Level` argument seeds that fixture, which is also what lets the
/// runner's `insert_value` find a slot for the payload's `TypeId`.
#[then("the injected level is {expected:u8}")]
fn injected_level_is(level: &Level, expected: u8) -> Result<()> {
    anyhow::ensure!(
        level.0 == expected,
        "injected level {}; expected {expected}",
        level.0
    );
    Ok(())
}

/// Fails if it ever runs.
///
/// The failing scenarios place this after the failing `Given`, so the runner
/// must have stopped before reaching it. Its message deliberately avoids the
/// handler text the scenarios expect, keeping a genuine `#[should_panic]`
/// match distinct from a step that simply panicked where it should not.
#[then("the scenario must have stopped")]
fn scenario_must_have_stopped() -> Result<()> {
    anyhow::bail!("the scenario continued past a failing aliased step")
}

/// An aliased `Err` must fail the scenario through the generated runner.
///
/// The panic is asserted at this boundary rather than inside the step
/// wrapper: the runner defers it until the step loop has finished, so this
/// binding proves the error propagates across the loop as well as out of the
/// handler.
#[scenario(
    path = "tests/features/step_return.feature",
    name = "A failing aliased step fails the scenario"
)]
#[should_panic(expected = "aliased step failed")]
fn aliased_err_fails_the_scenario() {}

/// A boxed-error alias's `Err` must fail the scenario the same way.
///
/// The error type reaches the `Display` bound through a trait object here, so
/// this is the `cargo-orthohelp` shape of the case above.
#[scenario(
    path = "tests/features/step_return.feature",
    name = "A failing boxed-error aliased step fails the scenario"
)]
#[should_panic(expected = "boxed aliased step failed")]
fn boxed_alias_err_fails_the_scenario() {}

/// An aliased `Ok(T)` payload must reach a later step as the fixture value.
///
/// The seeded fixture is 5 and the feature expects 7, so this can only pass
/// if the runner recorded the step's payload as an override.
#[scenario(
    path = "tests/features/step_return.feature",
    name = "An aliased payload reaches a later step"
)]
fn aliased_payload_reaches_a_later_step(level: Level) {
    let _ = level;
}

/// A boxed-error `Ok(T)` payload must reach a later step as the fixture value.
#[scenario(
    path = "tests/features/step_return.feature",
    name = "A boxed-error aliased payload reaches a later step"
)]
fn boxed_alias_payload_reaches_a_later_step(level: Level) {
    let _ = level;
}

/// Runs the aliased unit step through the generated scenario runner.
///
/// The direct `Ok(None)` assertion in `aliased_unit_ok_injects_nothing` pins
/// that the wrapper emits no unit payload. This binding also exercises the
/// alias through the runner; adjacent scenarios cover non-unit payload
/// insertion.
#[scenario(
    path = "tests/features/step_return.feature",
    name = "An aliased unit step produces no override"
)]
fn aliased_unit_step_produces_no_override(level: Level) {
    let _ = level;
}
