//! Shared fixtures and builders for message-planning tests, allowing
//! scenarios to construct consistent `HelloWorldCli` configurations and
//! expected plans across multiple cases.
use super::*;
pub(crate) use crate::cli::tests::helpers::{
    TakeLeaveCommandFixture, greet_command, take_leave_command,
};
use crate::cli::{GreetCommand, HelloWorldCli, TakeLeaveCommand};
use anyhow::{Result, anyhow, ensure};
use rstest::fixture;

pub(crate) struct Plan {
    pub config: HelloWorldCli,
    pub greeting: GreetingPlan,
    pub take_leave: TakeLeavePlan,
}

pub(crate) struct ExpectedPlan {
    pub recipient: &'static str,
    pub message: &'static str,
    pub is_excited: bool,
}

pub(crate) type HelloWorldCliFixture = Result<HelloWorldCli>;

/// The fallible sample configuration consumed by message-planning tests.
///
/// This is the `rstest` fixture result injected into tests that compose plans
/// from the shipped sample files rather than from process-backed defaults.
pub(crate) type SampleConfigFixture = Result<(HelloWorldCli, GreetCommand)>;

/// One plan-variant case: how to set up its inputs and what it should produce.
///
/// `is_sampled` selects whether the case composes its plan from the injected
/// sample configuration or from the inputs the setup functions prepared.
pub(crate) struct PlanVariantCase {
    pub greet_setup: GreetSetup,
    pub leave_setup: LeaveSetup,
    pub expected: ExpectedPlan,
    pub is_sampled: bool,
}

pub(crate) type GreetSetup = fn(&mut HelloWorldCli, &mut GreetCommand) -> Result<()>;
pub(crate) type LeaveSetup = fn(&mut HelloWorldCli, &mut TakeLeaveCommand) -> Result<()>;

pub(crate) fn build_plan_from(
    config: HelloWorldCli,
    greet: &GreetCommand,
    leave: &TakeLeaveCommand,
    greeting_defaults: GreetCommand,
) -> Result<Plan> {
    let greeting = build_plan(&config, greet).map_err(|err| anyhow!(err.to_string()))?;
    let take_leave =
        build_take_leave_plan_with_greet_loader(&config, leave, || Ok(greeting_defaults))
            .map_err(|err| anyhow!(err.to_string()))?;

    Ok(Plan {
        config,
        greeting,
        take_leave,
    })
}

pub(crate) fn setup_default_greet(config: &mut HelloWorldCli, _: &mut GreetCommand) -> Result<()> {
    *config = HelloWorldCli::default();
    ensure!(
        !config.recipient.trim().is_empty(),
        "default recipient must not be empty"
    );
    ensure!(
        !config.salutations.is_empty(),
        "default salutations should contain at least one entry"
    );
    Ok(())
}

pub(crate) fn setup_excited(config: &mut HelloWorldCli, _: &mut GreetCommand) -> Result<()> {
    ensure!(
        !config.is_quiet,
        "excited setup expects quiet delivery disabled",
    );
    config.is_excited = true;
    Ok(())
}

pub(crate) fn setup_noop_leave(_: &mut HelloWorldCli, leave: &mut TakeLeaveCommand) -> Result<()> {
    ensure!(
        !leave.parting.trim().is_empty(),
        "default farewell must not be empty"
    );
    Ok(())
}

pub(crate) fn setup_festive_leave(
    _: &mut HelloWorldCli,
    leave: &mut TakeLeaveCommand,
) -> Result<()> {
    ensure!(
        !leave.parting.trim().is_empty(),
        "festive leave requires a non-empty parting message",
    );
    leave.wave = true;
    leave.gift = Some(String::from("biscuits"));
    Ok(())
}

#[fixture]
pub(crate) fn base_config() -> HelloWorldCliFixture {
    let config = HelloWorldCli::default();
    ensure!(
        !config.recipient.trim().is_empty(),
        "default recipient must not be empty"
    );
    ensure!(
        !config.salutations.is_empty(),
        "default salutations should contain at least one entry"
    );
    Ok(config)
}

pub(crate) fn build_plan_variant(
    config: HelloWorldCli,
    greet: &GreetCommand,
    leave: &TakeLeaveCommand,
    sample: Option<&(HelloWorldCli, GreetCommand)>,
) -> Result<Plan> {
    sample.map_or_else(
        || build_plan_from(config, greet, leave, GreetCommand::default()),
        |(sample_config, sample_greet)| {
            build_plan_from(
                sample_config.clone(),
                sample_greet,
                leave,
                sample_greet.clone(),
            )
        },
    )
}
