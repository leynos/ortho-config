//! Tests for greeting and farewell planning in the `hello_world` example.

mod assertions;
mod fixtures;

use super::*;
use crate::cli::{
    FarewellChannel, GreetCommand, HelloWorldCli, TakeLeaveCommand, load_greet_defaults,
};
use crate::error::ValidationError;
use crate::test_support::{figment_error, with_jail};
use anyhow::{Result, anyhow, ensure};
use assertions::{assert_greeting, assert_plan, assert_sample_config_greeting};
use fixtures::{
    ExpectedPlan, HelloWorldCliFixture, PlanVariant, PlanVariantCase, TakeLeaveCommandFixture,
    base_config, build_plan_variant, greet_command, setup_default_greet, setup_excited,
    setup_festive_leave, setup_noop_leave, setup_sample_greet, take_leave_command,
};
use rstest::rstest;

#[test]
fn build_plan_variants() -> Result<()> {
    let cases = [
        PlanVariantCase {
            greet_setup: setup_default_greet,
            leave_setup: setup_noop_leave,
            expected: ExpectedPlan {
                recipient: "World",
                message: "Hello, World!",
                is_excited: false,
            },
            variant: PlanVariant::Direct,
        },
        PlanVariantCase {
            greet_setup: setup_excited,
            leave_setup: setup_noop_leave,
            expected: ExpectedPlan {
                recipient: "World",
                message: "HELLO, WORLD!",
                is_excited: true,
            },
            variant: PlanVariant::Direct,
        },
        PlanVariantCase {
            greet_setup: setup_sample_greet,
            leave_setup: setup_noop_leave,
            expected: ExpectedPlan {
                recipient: "Excited crew",
                message: "HELLO HEY CONFIG FRIENDS, EXCITED CREW!!!",
                is_excited: true,
            },
            variant: PlanVariant::SampleEnv,
        },
        PlanVariantCase {
            greet_setup: setup_sample_greet,
            leave_setup: setup_festive_leave,
            expected: ExpectedPlan {
                recipient: "Excited crew",
                message: "HELLO HEY CONFIG FRIENDS, EXCITED CREW!!!",
                is_excited: true,
            },
            variant: PlanVariant::SampleEnv,
        },
    ];

    for case in cases {
        let mut config = HelloWorldCli::default();
        let mut greet = GreetCommand::default();
        let mut leave = TakeLeaveCommand::default();

        (case.greet_setup)(&mut config, &mut greet)?;
        (case.leave_setup)(&mut config, &mut leave)?;

        let plan = build_plan_variant(config, &greet, &leave, case.variant)?;
        assert_plan(&plan, &case.expected)?;
    }

    Ok(())
}

#[test]
fn build_plan_applies_preamble() -> Result<()> {
    let mut command = greet_command()?;
    command.preamble = Some(String::from("Good morning"));
    let base = base_config()?;
    let plan = build_plan(&base, &command).map_err(|err| anyhow!(err.to_string()))?;
    assert_greeting(
        &plan,
        DeliveryMode::Standard,
        "Hello, World!",
        Some("Good morning"),
    )
}

#[test]
fn build_plan_respects_excitement_flag() -> Result<()> {
    let cases = [
        (false, DeliveryMode::Standard, "Hello, World!"),
        (true, DeliveryMode::Enthusiastic, "HELLO, WORLD!"),
    ];

    for (is_excited, expected_mode, expected_message) in cases {
        let mut base = base_config()?;
        base.is_excited = is_excited;
        let command = greet_command()?;
        let plan = build_plan(&base, &command).map_err(|err| anyhow!(err.to_string()))?;
        assert_greeting(&plan, expected_mode, expected_message, None)?;
    }

    Ok(())
}

#[test]
fn build_plan_propagates_validation_errors() -> Result<()> {
    let mut config = base_config()?;
    config.salutations.clear();
    let Err(err) = build_plan(&config, &GreetCommand::default()) else {
        return Err(anyhow!("expected build_plan to fail"));
    };
    ensure!(
        matches!(
            err,
            HelloWorldError::Validation(ValidationError::MissingSalutation)
        ),
        "expected missing salutation error"
    );
    Ok(())
}

#[test]
fn build_take_leave_plan_produces_steps() -> Result<()> {
    let take_leave_command = TakeLeaveCommand {
        wave: true,
        gift: Some(String::from("biscuits")),
        channel: Some(FarewellChannel::Email),
        remind_in: Some(10),
        ..TakeLeaveCommand::default()
    };
    let plan = with_jail(|jail| {
        jail.clear_env();
        build_take_leave_plan(&HelloWorldCli::default(), &take_leave_command).map_err(figment_error)
    })?;
    ensure!(
        plan.greeting().message() == "Hello, World!",
        "unexpected greeting"
    );
    let farewell = plan.farewell();
    ensure!(
        farewell.contains("waves enthusiastically"),
        "missing wave note"
    );
    ensure!(farewell.contains("leaves biscuits"), "missing gift note");
    ensure!(
        farewell.contains("follows up with an email"),
        "missing email note"
    );
    ensure!(farewell.contains("10 minutes"), "missing reminder note");
    Ok(())
}

#[rstest]
fn build_take_leave_plan_applies_greeting_overrides(
    base_config: HelloWorldCliFixture,
    take_leave_command: TakeLeaveCommandFixture,
) -> Result<()> {
    let mut command = take_leave_command?;
    command.greeting_preamble = Some(String::from("Until next time"));
    command.greeting_punctuation = Some(String::from("?"));
    let base = base_config?;
    let plan = with_jail(|jail| {
        jail.clear_env();
        build_take_leave_plan(&base, &command).map_err(figment_error)
    })?;
    ensure!(
        plan.greeting().preamble() == Some("Until next time"),
        "unexpected greeting preamble"
    );
    ensure!(
        plan.greeting().message().ends_with('?'),
        "unexpected punctuation"
    );
    Ok(())
}

#[rstest]
fn join_fragments_writes_list() -> Result<()> {
    let parts = vec![
        String::from("waves"),
        String::from("leaves biscuits"),
        String::from("follows up with an email"),
    ];
    ensure!(
        join_fragments(&parts) == "waves, leaves biscuits, and follows up with an email",
        "unexpected triple fragment join"
    );

    let pair = vec![String::from("waves"), String::from("leaves biscuits")];
    ensure!(
        join_fragments(&pair) == "waves and leaves biscuits",
        "unexpected double fragment join"
    );
    Ok(())
}

#[rstest]
fn build_take_leave_plan_uses_greet_defaults() -> Result<()> {
    assert_sample_config_greeting(|config| {
        build_take_leave_plan(config, &TakeLeaveCommand::default())
            .map(|plan| plan.greeting().clone())
            .map_err(figment_error)
    })
}

#[rstest]
fn build_plan_uses_sample_overrides() -> Result<()> {
    assert_sample_config_greeting(|config| {
        let greet = load_greet_defaults().map_err(figment_error)?;
        build_plan(config, &greet).map_err(figment_error)
    })
}
