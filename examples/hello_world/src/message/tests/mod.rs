//! Tests for greeting and farewell planning in the `hello_world` example.

use super::*;
use crate::cli::{FarewellChannel, GlobalArgs};
use crate::error::ValidationError;
use anyhow::{Result, anyhow, ensure};
use camino::Utf8PathBuf;
use ortho_config::figment;
use rstest::{fixture, rstest};

type HelloWorldCliFixture = Result<HelloWorldCli>;
type GreetCommandFixture = Result<GreetCommand>;
type TakeLeaveCommandFixture = Result<TakeLeaveCommand>;

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

#[rstest]
fn build_plan_produces_default_message(
    base_config: HelloWorldCliFixture,
    greet_command: GreetCommandFixture,
) -> Result<()> {
    let base = base_config?;
    let greet = greet_command?;
    let plan = build_plan(&base, &greet).map_err(|err| anyhow!(err.to_string()))?;
    assert_greeting(&plan, DeliveryMode::Standard, "Hello, World!", None)
}

#[rstest]
fn build_plan_shouts_for_excited(
    base_config: HelloWorldCliFixture,
    greet_command: GreetCommandFixture,
) -> Result<()> {
    let mut base = base_config?;
    base.is_excited = true;
    let greet = greet_command?;
    let plan = build_plan(&base, &greet).map_err(|err| anyhow!(err.to_string()))?;
    assert_greeting(&plan, DeliveryMode::Enthusiastic, "HELLO, WORLD!", None)
}

#[rstest]
fn build_plan_applies_preamble(
    greet_command: GreetCommandFixture,
    base_config: HelloWorldCliFixture,
) -> Result<()> {
    let mut command = greet_command?;
    command.preamble = Some(String::from("Good morning"));
    let base = base_config?;
    let plan = build_plan(&base, &command).map_err(|err| anyhow!(err.to_string()))?;
    assert_greeting(
        &plan,
        DeliveryMode::Standard,
        "Hello, World!",
        Some("Good morning"),
    )
}

#[rstest]
fn build_plan_propagates_validation_errors(base_config: HelloWorldCliFixture) -> Result<()> {
    let mut config = base_config?;
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

#[rstest]
fn build_take_leave_plan_produces_steps() -> Result<()> {
    let take_leave_command = TakeLeaveCommand {
        wave: true,
        gift: Some(String::from("biscuits")),
        channel: Some(FarewellChannel::Email),
        remind_in: Some(10),
        ..TakeLeaveCommand::default()
    };
    let plan = build_take_leave_plan(&HelloWorldCli::default(), &take_leave_command)
        .map_err(|err| anyhow!(err.to_string()))?;
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
fn build_take_leave_plan_uses_greet_defaults() -> Result<()> {
    let plan = with_sample_config(|config| {
        build_take_leave_plan(config, &TakeLeaveCommand::default())
            .map_err(|err| figment_error(&err))
    })?;
    assert_greeting(
        plan.greeting(),
        DeliveryMode::Enthusiastic,
        "HELLO HEY CONFIG FRIENDS, EXCITED CREW!!!",
        Some("Layered hello"),
    )
}

#[rstest]
fn build_plan_uses_sample_overrides() -> Result<()> {
    let plan = with_sample_config(|config| {
        let greet = crate::cli::load_greet_defaults().map_err(|err| figment_error(&err))?;
        build_plan(config, &greet).map_err(|err| figment_error(&err))
    })?;
    assert_greeting(
        &plan,
        DeliveryMode::Enthusiastic,
        "HELLO HEY CONFIG FRIENDS, EXCITED CREW!!!",
        Some("Layered hello"),
    )
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
        .map_err(|err| figment_error(&err))?;
        let baseline = config_dir
            .read_to_string("baseline.toml")
            .map_err(|err| figment_error(&err))?;
        let overrides = config_dir
            .read_to_string("overrides.toml")
            .map_err(|err| figment_error(&err))?;
        jail.create_file("baseline.toml", &baseline)?;
        jail.create_file(".hello_world.toml", &overrides)?;
        let config = crate::cli::load_global_config(&GlobalArgs::default(), None)
            .map_err(|err| figment_error(&err))?;
        output = Some(action(&config)?);
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    output.ok_or_else(|| anyhow!("sample config action did not produce a value"))
}

fn figment_error<E: ToString>(err: &E) -> figment::Error {
    figment::Error::from(err.to_string())
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
