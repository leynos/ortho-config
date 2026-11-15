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
//! The manual `insert_value` call is deliberate here and is *not* the only
//! coverage of runner behaviour: [`step_return_scenarios`](super::step_return_scenarios)
//! runs the same aliases through the generated scenario runner, so a defect
//! in the code that calls `insert_value` fails there. This module keeps the
//! mutations a scenario cannot express — displacement of an earlier override
//! and a payload with no matching fixture.
//!
//! It also pins unit normalization directly. `Ok(None)` is the proof that an
//! aliased `Ok(())` carries no payload for the runner to insert; a helper
//! that returned `Some(())` would be caught here but would show up in a
//! scenario only as a silently ignored warning.

use std::any::Any;

use rstest::rstest;
use rstest_bdd::{
    InsertOutcome, StepContext, StepError, StepKeyword,
    execution::{ExecutionError, StepExecutionRequest, execute_step},
};

use super::step_return_scenarios::Level;

/// The outcome type produced by [`execute_step`] for a single step.
type StepOutcome = std::result::Result<Option<Box<dyn Any>>, ExecutionError>;

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

/// Drives one `Given` step to completion against a fresh context.
///
/// The outcome owns its injected payload, so the context need not outlive the
/// call; each test body is left with its assertion alone.
fn outcome_of_given(step: &str) -> StepOutcome {
    let mut ctx = StepContext::default();
    run_step(StepKeyword::Given, step, &mut ctx)
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

/// The two step-return alias shapes whose semantics must agree.
///
/// They differ only in how the error type reaches the `Display` bound:
/// `anyhow::Result` uses a concrete error, while `cargo-orthohelp`'s steps use
/// a boxed trait object. Each regression case runs once per shape, so the
/// assertions exist in one place instead of being copied per shape.
#[derive(Clone, Copy, Debug)]
enum AliasShape {
    /// `Result<T, anyhow::Error>`.
    Anyhow,
    /// `Result<T, Box<dyn Error + Send + Sync>>`.
    Boxed,
}

impl AliasShape {
    /// The `Given` step that reports a handler failure for this shape.
    const fn failure_step(self) -> &'static str {
        match self {
            Self::Anyhow => "a step return alias reports failure",
            Self::Boxed => "a boxed-error step alias reports failure",
        }
    }

    /// The handler message this shape's failing step carries.
    const fn failure_message(self) -> &'static str {
        match self {
            Self::Anyhow => "aliased step failed",
            Self::Boxed => "boxed aliased step failed",
        }
    }

    /// The `Given` step that injects a level for this shape.
    const fn success_step(self) -> &'static str {
        match self {
            Self::Anyhow => "a step return alias reports a level of 7",
            Self::Boxed => "a boxed-error step alias reports a level of 3",
        }
    }

    /// The level `success_step` injects for this shape.
    const fn level(self) -> u8 {
        match self {
            Self::Anyhow => 7,
            Self::Boxed => 3,
        }
    }
}

/// An aliased `Result` step that returns `Err` must fail the scenario.
///
/// Before ADR-019 a fallible alias could be emitted as a value, boxing its
/// `Err` as an unused payload and letting the scenario pass. This test is the
/// non-vacuous guard that the defect stays fixed: it reaches the step, and it
/// asserts the failure, so removing dispatch would fail it.
#[rstest]
#[case::anyhow_alias(AliasShape::Anyhow)]
#[case::boxed_alias(AliasShape::Boxed)]
fn aliased_err_fails_the_scenario(#[case] shape: AliasShape) {
    let error = expect_failure(
        outcome_of_given(shape.failure_step()),
        "an Err from an aliased step must fail the scenario",
    );
    assert_handler_failed(error, shape.failure_message());
}

/// An aliased `Ok(T)` step must inject `T`, not a boxed `Result`.
///
/// The two shapes use different levels so a payload injected by one cannot be
/// mistaken for the other's.
#[rstest]
#[case::anyhow_alias(AliasShape::Anyhow)]
#[case::boxed_alias(AliasShape::Boxed)]
fn aliased_ok_injects_the_inner_payload(#[case] shape: AliasShape) {
    let payload = expect_payload(
        outcome_of_given(shape.success_step()),
        "an Ok from an aliased step must not fail",
    );
    assert_eq!(
        payload.downcast_ref::<Level>(),
        Some(&Level(shape.level())),
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
    match outcome_of_given("a step return alias succeeds with no payload") {
        Ok(None) => {}
        Ok(Some(_)) => panic!("Ok(()) must not box a payload"),
        Err(error) => panic!("Ok(()) must not fail the scenario: {error}"),
    }
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
