//! Shared fixtures and builders for message-planning tests, allowing
//! scenarios to construct consistent `HelloWorldCli` configurations and
//! expected plans across multiple cases.
use super::*;
pub(crate) use crate::cli::tests::helpers::{greet_command, take_leave_command, TakeLeaveCommandFixture};
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

#[derive(Clone, Copy)]
pub(crate) enum PlanVariant {
    Direct,
    SampleEnv,
}

pub(crate) struct PlanVariantCase {
    pub greet_setup: GreetSetup,
    pub leave_setup: LeaveSetup,
    pub expected: ExpectedPlan,
    pub variant: PlanVariant,
}

pub(crate) type GreetSetup = fn(&mut HelloWorldCli, &mut GreetCommand) -> Result<()>;
pub(crate) type LeaveSetup = fn(&mut HelloWorldCli, &mut TakeLeaveCommand) -> Result<()>;

pub(crate) fn build_plan_from(
    config: HelloWorldCli,
    greet: GreetCommand,
    leave: TakeLeaveCommand,
) -> Result<Plan> {
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
    })
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
    greet: GreetCommand,
    leave: TakeLeaveCommand,
    variant: PlanVariant,
) -> Result<Plan> {
    match variant {
        PlanVariant::Direct => with_jail(|jail| {
            jail.clear_env();
            plan_from_inputs(config, greet, leave)
        }),
        PlanVariant::SampleEnv => with_sample_config(move |cfg| {
            let greet_defaults = load_greet_defaults().map_err(figment_error)?;
            let mut sample_leave = take_leave_command().map_err(figment_error)?;
            sample_leave.parting = leave.parting.clone();
            sample_leave.greeting_preamble = leave.greeting_preamble.clone();
            sample_leave.greeting_punctuation = leave.greeting_punctuation.clone();
            sample_leave.channel = leave.channel;
            sample_leave.remind_in = leave.remind_in;
            sample_leave.gift = leave.gift.clone();
            sample_leave.wave = leave.wave;
            plan_from_inputs(cfg.clone(), greet_defaults, sample_leave)
        }),
    }
}

#[expect(
    clippy::result_large_err,
    reason = "figment::Error originates upstream and remains unboxed elsewhere"
)]
fn plan_from_inputs(
    config: HelloWorldCli,
    greet: GreetCommand,
    leave: TakeLeaveCommand,
) -> figment::error::Result<Plan> {
    build_plan_from(config, greet, leave).map_err(figment_error)
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
