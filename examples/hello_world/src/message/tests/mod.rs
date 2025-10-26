//! Tests for greeting and farewell planning in the `hello_world` example.

use super::*;
use crate::cli::{FarewellChannel, GlobalArgs, GreetCommand, HelloWorldCli, TakeLeaveCommand};
use crate::error::ValidationError;
use crate::test_support::figment_error;
use anyhow::{Result, anyhow, ensure};
use camino::Utf8PathBuf;
use ortho_config::figment;
use rstest::{fixture, rstest};
struct Plan {
    config: HelloWorldCli,
    greeting: GreetingPlan,
    take_leave: TakeLeavePlan,
}

struct ExpectedPlan {
    recipient: &'static str,
    message: &'static str,
    is_excited: bool,
}

fn build_plan_from(
    config: HelloWorldCli,
    mut greet: GreetCommand,
    mut leave: TakeLeaveCommand,
) -> Result<Plan> {
    // Take mutable references so the bindings justify their mutability without
    // altering behaviour; future callers may tweak the commands in place.
    let _ = &mut greet;
    let _ = &mut leave;

    let greeting = build_plan(&config, &greet).map_err(|err| anyhow!(err.to_string()))?;
    let take_leave =
        build_take_leave_plan(&config, &leave).map_err(|err| anyhow!(err.to_string()))?;

    Ok(Plan {
        config,
        greeting,
        take_leave,
    })
}

fn assert_plan(plan: &Plan, expected: &ExpectedPlan) -> Result<()> {
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

type HelloWorldCliFixture = Result<HelloWorldCli>;
type GreetCommandFixture = Result<GreetCommand>;
type TakeLeaveCommandFixture = Result<TakeLeaveCommand>;
type PlanBuilder = fn(HelloWorldCli, GreetCommand, TakeLeaveCommand) -> Result<Plan>;

struct PlanVariantCase {
    greet_setup: GreetSetup,
    leave_setup: LeaveSetup,
    expected: ExpectedPlan,
    builder: PlanBuilder,
}

fn setup_default_greet(config: &mut HelloWorldCli, _: &mut GreetCommand) -> Result<()> {
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

fn setup_excited(config: &mut HelloWorldCli, _: &mut GreetCommand) -> Result<()> {
    config.is_excited = true;
    ensure!(config.is_excited, "excitement flag must be enabled");
    Ok(())
}

fn setup_sample_greet(config: &mut HelloWorldCli, greet: &mut GreetCommand) -> Result<()> {
    with_sample_config(|cfg| {
        *config = cfg.clone();
        *greet = crate::cli::load_greet_defaults().map_err(figment_error)?;
        Ok(())
    })?;
    Ok(())
}

fn setup_noop_leave(_: &mut HelloWorldCli, leave: &mut TakeLeaveCommand) -> Result<()> {
    ensure!(
        !leave.parting.trim().is_empty(),
        "default farewell must not be empty"
    );
    Ok(())
}

fn setup_festive_leave(_: &mut HelloWorldCli, leave: &mut TakeLeaveCommand) -> Result<()> {
    leave.wave = true;
    leave.gift = Some(String::from("biscuits"));
    ensure!(leave.gift.is_some(), "festive leave should include a gift");
    Ok(())
}

#[fixture]
fn base_config() -> HelloWorldCliFixture {
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

#[fixture]
fn greet_command() -> GreetCommandFixture {
    let command = GreetCommand::default();
    ensure!(
        !command.punctuation.trim().is_empty(),
        "default greet punctuation must not be empty"
    );
    Ok(command)
}

#[fixture]
fn take_leave_command() -> TakeLeaveCommandFixture {
    let command = TakeLeaveCommand::default();
    ensure!(
        !command.parting.trim().is_empty(),
        "default farewell must not be empty"
    );
    Ok(command)
}

type GreetSetup = fn(&mut HelloWorldCli, &mut GreetCommand) -> Result<()>;
type LeaveSetup = fn(&mut HelloWorldCli, &mut TakeLeaveCommand) -> Result<()>;

fn build_plan_direct(
    config: HelloWorldCli,
    greet: GreetCommand,
    leave: TakeLeaveCommand,
) -> Result<Plan> {
    let mut plan = None;
    figment::Jail::try_with(|jail| {
        jail.clear_env();
        plan = Some(build_plan_from(config, greet, leave).map_err(figment_error)?);
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    plan.ok_or_else(|| anyhow!("direct plan builder did not produce a plan"))
}

fn build_plan_with_sample_env(
    config: HelloWorldCli,
    greet: GreetCommand,
    leave: TakeLeaveCommand,
) -> Result<Plan> {
    with_sample_config(move |_| build_plan_from(config, greet, leave).map_err(figment_error))
}

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
            builder: build_plan_direct,
        },
        PlanVariantCase {
            greet_setup: setup_excited,
            leave_setup: setup_noop_leave,
            expected: ExpectedPlan {
                recipient: "World",
                message: "HELLO, WORLD!",
                is_excited: true,
            },
            builder: build_plan_direct,
        },
        PlanVariantCase {
            greet_setup: setup_sample_greet,
            leave_setup: setup_noop_leave,
            expected: ExpectedPlan {
                recipient: "Excited crew",
                message: "HELLO HEY CONFIG FRIENDS, EXCITED CREW!!!",
                is_excited: true,
            },
            builder: build_plan_with_sample_env,
        },
        PlanVariantCase {
            greet_setup: setup_sample_greet,
            leave_setup: setup_festive_leave,
            expected: ExpectedPlan {
                recipient: "Excited crew",
                message: "HELLO HEY CONFIG FRIENDS, EXCITED CREW!!!",
                is_excited: true,
            },
            builder: build_plan_with_sample_env,
        },
    ];

    for case in cases {
        let mut config = HelloWorldCli::default();
        let mut greet = GreetCommand::default();
        let mut leave = TakeLeaveCommand::default();

        (case.greet_setup)(&mut config, &mut greet)?;
        (case.leave_setup)(&mut config, &mut leave)?;

        let plan = (case.builder)(config, greet, leave)?;
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
    let mut plan_result = None;
    figment::Jail::try_with(|jail| {
        jail.clear_env();
        plan_result = Some(
            build_take_leave_plan(&HelloWorldCli::default(), &take_leave_command)
                .map_err(figment_error)?,
        );
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    let plan =
        plan_result.ok_or_else(|| anyhow!("take-leave plan builder did not produce a plan"))?;
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
    let plan = build_take_leave_plan(&base, &command).map_err(|err| anyhow!(err.to_string()))?;
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
        let greet = crate::cli::load_greet_defaults().map_err(figment_error)?;
        build_plan(config, &greet).map_err(figment_error)
    })
}

fn with_sample_config<R, F>(action: F) -> Result<R>
where
    F: FnOnce(&HelloWorldCli) -> figment::error::Result<R>,
{
    let mut output = None;
    figment::Jail::try_with(|jail| {
        jail.clear_env();
        let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config_dir = cap_std::fs::Dir::open_ambient_dir(
            manifest_dir.join("config").as_std_path(),
            cap_std::ambient_authority(),
        )
        .map_err(figment_error)?;
        let baseline = config_dir
            .read_to_string("baseline.toml")
            .map_err(figment_error)?;
        let overrides = config_dir
            .read_to_string("overrides.toml")
            .map_err(figment_error)?;
        jail.create_file("baseline.toml", &baseline)?;
        jail.create_file(".hello_world.toml", &overrides)?;
        let config =
            crate::cli::load_global_config(&GlobalArgs::default(), None).map_err(figment_error)?;
        output = Some(action(&config)?);
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    output.ok_or_else(|| anyhow!("sample config action did not produce a value"))
}

fn assert_sample_config_greeting<F>(build_fn: F) -> Result<()>
where
    F: FnOnce(&HelloWorldCli) -> figment::error::Result<GreetingPlan>,
{
    let plan = with_sample_config(build_fn)?;
    assert_greeting(
        &plan,
        DeliveryMode::Enthusiastic,
        "HELLO HEY CONFIG FRIENDS, EXCITED CREW!!!",
        Some("Layered hello"),
    )
}

fn assert_greeting(
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
