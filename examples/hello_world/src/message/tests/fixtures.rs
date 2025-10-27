use super::*;
use crate::cli::{
    GlobalArgs, GreetCommand, HelloWorldCli, TakeLeaveCommand, load_global_config,
    load_greet_defaults,
};
use crate::test_support::{figment_error, with_jail};
use anyhow::{Result, anyhow, ensure};
use camino::Utf8PathBuf;
use ortho_config::figment;
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
pub(crate) type GreetCommandFixture = Result<GreetCommand>;
pub(crate) type TakeLeaveCommandFixture = Result<TakeLeaveCommand>;

pub(crate) type PlanBuilder = fn(HelloWorldCli, GreetCommand, TakeLeaveCommand) -> Result<Plan>;

pub(crate) struct PlanVariantCase {
    pub greet_setup: GreetSetup,
    pub leave_setup: LeaveSetup,
    pub expected: ExpectedPlan,
    pub builder: PlanBuilder,
}

pub(crate) type GreetSetup = fn(&mut HelloWorldCli, &mut GreetCommand) -> Result<()>;
pub(crate) type LeaveSetup = fn(&mut HelloWorldCli, &mut TakeLeaveCommand) -> Result<()>;

pub(crate) fn build_plan_from(
    config: HelloWorldCli,
    mut greet: GreetCommand,
    mut leave: TakeLeaveCommand,
) -> Result<Plan> {
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
    config.is_excited = true;
    ensure!(config.is_excited, "excitement flag must be enabled");
    Ok(())
}

pub(crate) fn setup_sample_greet(
    config: &mut HelloWorldCli,
    greet: &mut GreetCommand,
) -> Result<()> {
    with_sample_config(|cfg| {
        *config = cfg.clone();
        *greet = load_greet_defaults().map_err(figment_error)?;
        Ok(())
    })?;
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
    leave.wave = true;
    leave.gift = Some(String::from("biscuits"));
    ensure!(leave.gift.is_some(), "festive leave should include a gift");
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

#[fixture]
pub(crate) fn greet_command() -> GreetCommandFixture {
    let command = GreetCommand::default();
    ensure!(
        !command.punctuation.trim().is_empty(),
        "default greet punctuation must not be empty"
    );
    Ok(command)
}

#[fixture]
pub(crate) fn take_leave_command() -> TakeLeaveCommandFixture {
    let command = TakeLeaveCommand::default();
    ensure!(
        !command.parting.trim().is_empty(),
        "default farewell must not be empty"
    );
    Ok(command)
}

pub(crate) fn build_plan_direct(
    config: HelloWorldCli,
    greet: GreetCommand,
    leave: TakeLeaveCommand,
) -> Result<Plan> {
    with_jail(|jail| {
        jail.clear_env();
        build_plan_from(config, greet, leave).map_err(figment_error)
    })
}

pub(crate) fn build_plan_with_sample_env(
    config: HelloWorldCli,
    greet: GreetCommand,
    leave: TakeLeaveCommand,
) -> Result<Plan> {
    with_sample_config(move |_| build_plan_from(config, greet, leave).map_err(figment_error))
}

pub(crate) fn with_sample_config<R, F>(action: F) -> Result<R>
where
    F: FnOnce(&HelloWorldCli) -> figment::error::Result<R>,
{
    with_jail(|jail| {
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
        let config = load_global_config(&GlobalArgs::default(), None).map_err(figment_error)?;
        action(&config)
    })
}
