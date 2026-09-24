//! Shared fixtures and builders for message-planning tests, allowing
//! scenarios to construct consistent `HelloWorldCli` configurations and
//! expected plans across multiple cases.
use super::*;
pub(crate) use crate::cli::tests::helpers::{
    TakeLeaveCommandFixture, greet_command, take_leave_command,
};
use crate::cli::{
    GlobalArgs, GreetCommand, HelloWorldCli, TakeLeaveCommand, load_global_config_with_sources,
    load_greet_defaults_with_sources,
};
use crate::test_support::{ConfigFixture, global_sources};
use anyhow::{Context, Result, anyhow, ensure};
use camino::Utf8PathBuf;
use ortho_config::MapEnv;
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

pub(crate) fn setup_sample_greet(
    config: &mut HelloWorldCli,
    greet: &mut GreetCommand,
) -> Result<()> {
    with_sample_config(|cfg, sample_greet| {
        *config = cfg.clone();
        *greet = sample_greet.clone();
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
    variant: PlanVariant,
) -> Result<Plan> {
    match variant {
        PlanVariant::Direct => build_plan_from(config, greet, leave, GreetCommand::default()),
        PlanVariant::SampleEnv => with_sample_config(move |cfg, defaults| {
            let sample_greet = greet.clone();
            let sample_leave = leave.clone();
            build_plan_from(cfg.clone(), &sample_greet, &sample_leave, defaults.clone())
        }),
    }
}

pub(crate) fn with_sample_config<R, F>(action: F) -> Result<R>
where
    F: FnOnce(&HelloWorldCli, &GreetCommand) -> Result<R>,
{
    let fixture = ConfigFixture::new()?;
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_dir = cap_std::fs::Dir::open_ambient_dir(
        manifest_dir.join("config").as_std_path(),
        cap_std::ambient_authority(),
    )
    .context("open sample configuration")?;
    let baseline = config_dir.read_to_string("baseline.toml")?;
    let overrides = config_dir.read_to_string("overrides.toml")?;
    fixture.write("baseline.toml", &baseline)?;
    let selected = fixture.write(".hello_world.toml", &overrides)?;
    let discovery = MapEnv::new().with_var("HELLO_WORLD_CONFIG_PATH", selected);
    let config = load_global_config_with_sources(
        &GlobalArgs::default(),
        None,
        "hello-world",
        global_sources(discovery.clone(), MapEnv::new()),
    )?;
    let greet =
        load_greet_defaults_with_sources(fixture.path(), global_sources(discovery, MapEnv::new()))?;
    action(&config, &greet)
}
