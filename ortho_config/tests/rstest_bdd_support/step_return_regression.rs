//! Regression cover for `rstest-bdd` 0.6.0 step-return classification.
//!
//! `rstest-bdd` 0.6.0 replaced macro-time syntax classification of step
//! returns with type-directed dispatch (upstream ADR-019). This workspace
//! declares every step as a fallible alias — `anyhow::Result<T>` in
//! `ortho_config`, and `Result<T, Box<dyn Error + Send + Sync>>` in
//! `cargo-orthohelp` — so the change is invisible here until someone writes a
//! step returning a non-unit payload. The tests below pin both halves of that
//! contract for both alias shapes, so a future upgrade that stopped treating
//! an alias as fallible would fail here rather than in a scenario whose
//! failure message pointed at a configuration assertion three frames away:
//!
//! - an `Err` from an aliased step fails the scenario, and
//! - an `Ok(T)` payload is injected as `T`, while `Ok(())` injects nothing.
//!
//! The assertions drive the real registry through [`execute_step`] and then
//! mirror the scenario runner's `insert_value` call, so registration, fixture
//! binding and override precedence are exercised as a scenario would exercise
//! them, not by calling the normalizer directly.
//!
//! One deliberate limitation: the tests reach the runner's insertion branch by
//! hand instead of generating a scenario. The dispatch they pin is emitted by
//! the step wrapper macro, which is what both paths share; generating a
//! scenario would additionally need a feature file for a case the behavioural
//! suite deliberately does not cover today.

use std::any::Any;

use anyhow::Result;
use rstest_bdd::{
    FixtureBorrowError, InsertOutcome, StepContext, StepError, StepKeyword,
    execution::{ExecutionError, StepExecutionRequest, execute_step},
};
use rstest_bdd_macros::{given, then};

/// A non-unit payload carried out of a step as an override.
#[derive(Debug, PartialEq, Eq)]
struct Level(u8);

/// Alias of `Result<T, E>` with a `Display` error, mirroring the
/// `anyhow::Result` alias used by this crate's own steps.
type StepAlias<T> = Result<T, &'static str>;

/// Mirrors `cargo-orthohelp`'s step error type, which is a boxed trait object
/// rather than a concrete error, so the `Display` bound is exercised through
/// a trait object as well as through `anyhow::Error`.
type BoxedStepError = Box<dyn std::error::Error + Send + Sync>;

/// Mirrors `cargo-orthohelp`'s `StepResult<T>` alias exactly.
type BoxedStepResult<T> = Result<T, BoxedStepError>;

/// The outcome type produced by [`execute_step`] for a single step.
type StepOutcome = std::result::Result<Option<Box<dyn Any>>, ExecutionError>;

/// An alias whose `Err` must fail the scenario rather than pass silently.
#[given("a step return alias reports failure")]
fn alias_reports_failure() -> StepAlias<()> {
    Err("aliased step failed")
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
/// observes the override the runner recorded under that name. The test
/// registers a `Level` fixture there so the wrapper's required-fixture check
/// passes, exactly as a scenario's fixture argument would.
#[then("the injected level is {expected:u8}")]
fn injected_level_is(level: &Level, expected: u8) -> Result<()> {
    anyhow::ensure!(
        level.0 == expected,
        "injected level {}; expected {expected}",
        level.0
    );
    Ok(())
}

/// Builds a request that resolves against one of the steps above.
fn request(keyword: StepKeyword, text: &str) -> StepExecutionRequest<'_> {
    StepExecutionRequest {
        index: 0,
        keyword,
        text,
        docstring: None,
        table: None,
        feature_path: "tests/features/step_return_regression.feature",
        scenario_name: "step return classification regression",
    }
}

/// Runs one step against the supplied context and returns its outcome.
fn run_step(keyword: StepKeyword, text: &str, ctx: &mut StepContext<'_>) -> StepOutcome {
    execute_step(&request(keyword, text), ctx)
}

/// Unwraps a failing outcome, panicking with `context` when the step passed.
fn expect_failure(outcome: StepOutcome, context: &str) -> ExecutionError {
    match outcome {
        Ok(_) => panic!("{context}: the step unexpectedly succeeded"),
        Err(error) => error,
    }
}

/// Unwraps an injected payload, panicking with `context` otherwise.
fn expect_payload(outcome: StepOutcome, context: &str) -> Box<dyn Any> {
    match outcome {
        Ok(Some(value)) => value,
        Ok(None) => panic!("{context}: step injected no payload"),
        Err(error) => panic!("{context}: step failed: {error}"),
    }
}

/// Asserts that `error` is a handler failure whose message is `expected`.
///
/// `StepError` renders through Fluent as
/// `Error executing step '<pattern>' via function '<function>': <message>`,
/// so the bare handler text is asserted on the variant and the outer rendering
/// is checked for containment. `StepError` is `#[non_exhaustive]`, hence the
/// wildcard arms.
fn assert_handler_failed(failure: ExecutionError, expected: &str) {
    let ExecutionError::HandlerFailed { error, .. } = failure else {
        panic!("expected HandlerFailed, got a different ExecutionError variant: {failure}");
    };
    let StepError::ExecutionError { message, .. } = error.as_ref() else {
        panic!("expected a step execution error, got {error}");
    };
    assert_eq!(
        message.as_str(),
        expected,
        "the step handler's message must reach the error unchanged"
    );
    assert!(
        error.to_string().contains(expected),
        "the rendered failure must include the handler message: {error}"
    );
}

/// An aliased `Result` step that returns `Err` must fail the scenario.
///
/// Before ADR-019 a fallible alias could be emitted as a value, boxing its
/// `Err` as an unused payload and letting the scenario pass. This test is the
/// non-vacuous guard that the defect stays fixed: it reaches the step, and it
/// asserts the failure, so removing dispatch would fail it.
#[test]
fn aliased_err_fails_the_scenario() {
    let mut ctx = StepContext::default();
    let error = expect_failure(
        run_step(
            StepKeyword::Given,
            "a step return alias reports failure",
            &mut ctx,
        ),
        "an Err from an aliased step must fail the scenario",
    );
    assert_handler_failed(error, "aliased step failed");
}

/// The boxed-error alias must behave identically to the `anyhow` one.
///
/// `cargo-orthohelp`'s steps use `Result<T, Box<dyn Error + Send + Sync>>`,
/// which reaches the `Display` bound through a trait object. A trait object
/// can lose `Display` in a way a concrete type cannot, so it is asserted
/// separately rather than assumed.
#[test]
fn boxed_error_alias_fails_the_scenario() {
    let mut ctx = StepContext::default();
    let error = expect_failure(
        run_step(
            StepKeyword::Given,
            "a boxed-error step alias reports failure",
            &mut ctx,
        ),
        "an Err from a boxed-error alias must fail the scenario",
    );
    assert_handler_failed(error, "boxed aliased step failed");
}

/// An aliased `Ok(T)` step must inject `T`, not a boxed `Result`.
#[test]
fn aliased_ok_injects_the_inner_payload() {
    let mut ctx = StepContext::default();
    let payload = expect_payload(
        run_step(
            StepKeyword::Given,
            "a step return alias reports a level of 7",
            &mut ctx,
        ),
        "an Ok from an aliased step must not fail",
    );
    assert_eq!(
        payload.downcast_ref::<Level>(),
        Some(&Level(7)),
        "the injected payload must be the inner T, not a boxed Result"
    );
}

/// The boxed-error alias must also inject its inner payload on `Ok`.
#[test]
fn boxed_error_alias_ok_injects_the_inner_payload() {
    let mut ctx = StepContext::default();
    let payload = expect_payload(
        run_step(
            StepKeyword::Given,
            "a boxed-error step alias reports a level of 3",
            &mut ctx,
        ),
        "an Ok from a boxed-error alias must not fail",
    );
    assert_eq!(
        payload.downcast_ref::<Level>(),
        Some(&Level(3)),
        "the injected payload must be the inner T, not a boxed Result"
    );
}

/// `Ok(())` from a fallible alias must produce no payload at all.
///
/// This is the classification the 0.6.0 dispatch change touches: the alias
/// still dispatches as `Result`, and the unit payload is dropped before the
/// runner's insertion branch. A `()`-returning step would not prove this,
/// because the macro classifies it as unit and never reaches the dispatch
/// path that this test exercises.
#[test]
fn aliased_unit_ok_injects_nothing() {
    let unit_fixture = ();
    let mut ctx = StepContext::default();
    ctx.insert("flag", &unit_fixture);

    let outcome = run_step(
        StepKeyword::Given,
        "a step return alias succeeds with no payload",
        &mut ctx,
    );
    match outcome {
        Ok(None) => {}
        Ok(Some(_)) => panic!("Ok(()) must not box a payload"),
        Err(error) => panic!("Ok(()) must not fail the scenario: {error}"),
    }

    // A recorded override would be mutable even though the fixture was
    // inserted by shared reference, so `NotMutable` proves no override exists.
    assert!(
        matches!(
            ctx.try_borrow_mut::<()>("flag"),
            Err(FixtureBorrowError::NotMutable { .. })
        ),
        "a unit payload must leave a unit fixture untouched"
    );
}

/// A recorded payload must reach a later step through fixture binding.
///
/// This mirrors the scenario runner exactly: `execute_step` returns the
/// payload, the runner calls `insert_value`, and the next step's `&Level`
/// parameter resolves against the override rather than the original fixture.
#[test]
fn injected_payload_reaches_a_follow_up_step() {
    let original = Level(0);
    let mut ctx = StepContext::default();
    ctx.insert("level", &original);

    let payload = expect_payload(
        run_step(
            StepKeyword::Given,
            "a step return alias reports a level of 7",
            &mut ctx,
        ),
        "the aliased step must inject a payload",
    );
    let outcome = ctx.insert_value(payload);
    assert!(
        matches!(outcome, InsertOutcome::Inserted(None)),
        "the first override must be recorded and displace nothing"
    );

    let follow_up = run_step(StepKeyword::Then, "the injected level is 7", &mut ctx);
    match follow_up {
        Ok(_) => {}
        Err(error) => panic!("the follow-up step must read the injected level: {error}"),
    }
}

/// A later payload must displace the earlier one, last write wins.
#[test]
fn later_override_displaces_earlier_override() {
    let original = Level(0);
    let mut ctx = StepContext::default();
    ctx.insert("level", &original);

    let first = expect_payload(
        run_step(
            StepKeyword::Given,
            "a step return alias reports a level of 7",
            &mut ctx,
        ),
        "the first aliased step must inject a payload",
    );
    assert!(matches!(
        ctx.insert_value(first),
        InsertOutcome::Inserted(None)
    ));

    let second = expect_payload(
        run_step(
            StepKeyword::Given,
            "a step return alias reports a level of 9",
            &mut ctx,
        ),
        "the second aliased step must inject a payload",
    );
    let InsertOutcome::Inserted(previous) = ctx.insert_value(second) else {
        panic!("the second override must be recorded over a fixture");
    };
    let previous = previous.expect("the second override must displace the first");
    assert_eq!(
        previous.downcast_ref::<Level>(),
        Some(&Level(7)),
        "the displaced value must be the first override"
    );

    let follow_up = run_step(StepKeyword::Then, "the injected level is 9", &mut ctx);
    match follow_up {
        Ok(_) => {}
        Err(error) => panic!("the follow-up step must read the latest override: {error}"),
    }
}

/// A payload with no matching fixture type must be dropped, not guessed at.
#[test]
fn payload_without_matching_fixture_is_not_recorded() {
    let mut ctx = StepContext::default();
    let payload = expect_payload(
        run_step(
            StepKeyword::Given,
            "a step return alias reports a level of 7",
            &mut ctx,
        ),
        "the aliased step must inject a payload",
    );

    assert!(
        matches!(ctx.insert_value(payload), InsertOutcome::NoMatch),
        "an unmatched payload must be reported as a no-match"
    );
    assert!(
        ctx.available_fixtures().next().is_none(),
        "a no-match payload must not register a fixture"
    );
}
