use super::fixtures::{ExpectedPlan, Plan, with_sample_config};
use super::*;
use anyhow::{Result, ensure};

pub(crate) fn assert_plan(plan: &Plan, expected: &ExpectedPlan) -> Result<()> {
    ensure!(
        plan.config.recipient == expected.recipient,
        "unexpected recipient {}; expected {}",
        plan.config.recipient,
        expected.recipient
    );
    let actual_message = plan.greeting.message();
    ensure!(
        actual_message == expected.message,
        "unexpected greeting message {actual_message}; expected {}",
        expected.message
    );
    let leave_greeting = plan.take_leave.greeting();
    ensure!(
        leave_greeting.message() == expected.message,
        "unexpected take-leave greeting {}; expected {}",
        leave_greeting.message(),
        expected.message
    );
    ensure!(
        leave_greeting.mode() == plan.greeting.mode(),
        "take-leave greeting mode {:?} did not match {:?}",
        leave_greeting.mode(),
        plan.greeting.mode()
    );
    ensure!(
        plan.config.is_excited == expected.is_excited,
        "unexpected excitement flag {}; expected {}",
        plan.config.is_excited,
        expected.is_excited
    );
    Ok(())
}

pub(crate) fn assert_sample_config_greeting<F>(build_fn: F) -> Result<()>
where
    F: FnOnce(&HelloWorldCli) -> ortho_config::figment::error::Result<GreetingPlan>,
{
    let plan = with_sample_config(build_fn)?;
    // With declarative merge semantics, Vec<T> appends across defaults + extends chain
    assert_greeting(
        &plan,
        DeliveryMode::Enthusiastic,
        "HELLO HELLO FROM CONFIG HEY CONFIG FRIENDS, EXCITED CREW!!!",
        Some("Layered hello"),
    )
}

pub(crate) fn assert_greeting(
    plan: &GreetingPlan,
    expected_mode: DeliveryMode,
    expected_message: &str,
    expected_preamble: Option<&str>,
) -> Result<()> {
    ensure!(plan.mode() == expected_mode, "unexpected delivery mode");
    ensure!(
        plan.message() == expected_message,
        "unexpected greeting message"
    );
    ensure!(
        plan.preamble() == expected_preamble,
        "unexpected greeting preamble"
    );
    Ok(())
}
