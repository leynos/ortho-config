//! Behavioural coverage for the hello world CLI surface.
//!
//! Tests use fallible fixtures so setup failures surface as rich diagnostics
//! rather than panicking via `expect`.

use super::*;
#[cfg(unix)]
use crate::cli::discovery::collect_config_candidates;
use crate::cli::{FarewellChannel, GlobalArgs, GreetOverrides};
use crate::error::ValidationError;
use anyhow::{Context, Result, anyhow, ensure};
use camino::Utf8PathBuf;
use ortho_config::figment;
use rstest::{fixture, rstest};

type CommandAssertion = fn(CommandLine) -> Result<()>;

type HelloWorldCliFixture = Result<HelloWorldCli>;
type GreetCommandFixture = Result<GreetCommand>;
type TakeLeaveCommandFixture = Result<TakeLeaveCommand>;

#[fixture]
fn base_cli() -> HelloWorldCliFixture {
    let cli = HelloWorldCli::default();
    ensure!(
        !cli.salutations.is_empty(),
        "default salutations should contain at least one entry"
    );
    Ok(cli)
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
#[case::greet(
    &[
        "--recipient",
        "Crew",
        "-s",
        "Hi",
        "greet",
        "--preamble",
        "Good morning",
        "--punctuation",
        "?!",
    ],
    assert_greet_command as CommandAssertion
)]
#[case::take_leave(
    &[
        "--is-excited",
        "take-leave",
        "--parting",
        "Cheerio",
        "--gift",
        "flowers",
        "--remind-in",
        "20",
        "--channel",
        "message",
        "--wave",
    ],
    assert_take_leave_command as CommandAssertion
)]
fn command_line_parses_expected_variants(
    #[case] args: &[&str],
    #[case] assert_cli: CommandAssertion,
) -> Result<()> {
    let cli = parse_command_line(args)?;
    assert_cli(cli)?;
    Ok(())
}

#[rstest]
fn hello_world_cli_detects_conflicting_modes(base_cli: HelloWorldCliFixture) -> Result<()> {
    let mut cli = base_cli?;
    cli.is_excited = true;
    cli.is_quiet = true;
    let Err(err) = cli.validate() else {
        return Err(anyhow!(
            "expected conflicting delivery modes to fail validation"
        ));
    };
    ensure!(
        err == ValidationError::ConflictingDeliveryModes,
        "unexpected validation error: {err:?}"
    );
    Ok(())
}

#[rstest]
#[case::missing_salutations(
    |cli: &mut HelloWorldCli| {
        cli.salutations.clear();
        Ok(())
    },
    ValidationError::MissingSalutation
)]
#[case::blank_salutation(
    |cli: &mut HelloWorldCli| {
        cli.salutations.first_mut().map_or_else(
            || Err(anyhow!("expected at least one salutation")),
            |first| {
                *first = String::from("   ");
                Ok(())
            },
        )
    },
    ValidationError::BlankSalutation(0)
)]
fn hello_world_cli_validation_errors<F>(
    base_cli: HelloWorldCliFixture,
    #[case] mutate: F,
    #[case] expected: ValidationError,
) -> Result<()>
where
    F: Fn(&mut HelloWorldCli) -> Result<()>,
{
    let mut cli = base_cli?;
    mutate(&mut cli)?;
    let Err(err) = cli.validate() else {
        return Err(anyhow!("expected validation to fail with {expected:?}"));
    };
    ensure!(err == expected, "unexpected validation error: {err:?}");
    Ok(())
}

#[rstest]
#[case::excited(true, false, DeliveryMode::Enthusiastic)]
#[case::quiet(false, true, DeliveryMode::Quiet)]
#[case::standard(false, false, DeliveryMode::Standard)]
fn delivery_mode_from_flags(
    base_cli: HelloWorldCliFixture,
    #[case] excited: bool,
    #[case] quiet: bool,
    #[case] expected: DeliveryMode,
) -> Result<()> {
    let mut cli = base_cli?;
    cli.is_excited = excited;
    cli.is_quiet = quiet;
    let mode = cli.delivery_mode();
    ensure!(mode == expected, "unexpected delivery mode: {mode:?}");
    Ok(())
}

#[rstest]
fn trimmed_salutations_remove_whitespace(base_cli: HelloWorldCliFixture) -> Result<()> {
    let mut cli = base_cli?;
    cli.salutations = vec![String::from("  Hi"), String::from("Team  ")];
    let expected = vec![String::from("Hi"), String::from("Team")];
    ensure!(
        cli.trimmed_salutations() == expected,
        "expected trimmed salutations"
    );
    Ok(())
}

#[rstest]
#[case::punctuation(
    |command: &mut GreetCommand| {
        command.punctuation = String::from("   ");
        Ok(())
    },
    ValidationError::BlankPunctuation,
    "greeting punctuation must contain visible characters",
)]
#[case::preamble(
    |command: &mut GreetCommand| {
        command.preamble = Some(String::from("   "));
        Ok(())
    },
    ValidationError::BlankPreamble,
    "preambles must contain visible characters when supplied",
)]
fn greet_command_rejects_blank_inputs<F>(
    greet_command: GreetCommandFixture,
    #[case] mutate: F,
    #[case] expected_error: ValidationError,
    #[case] expected_message: &str,
) -> Result<()>
where
    F: Fn(&mut GreetCommand) -> Result<()>,
{
    let mut command = greet_command?;
    mutate(&mut command)?;
    let Err(err) = command.validate() else {
        return Err(anyhow!("expected validation to fail"));
    };
    ensure!(
        err == expected_error,
        "unexpected validation error: {err:?}"
    );
    ensure!(
        err.to_string() == expected_message,
        "unexpected validation message"
    );
    Ok(())
}

#[rstest]
#[case::blank_parting(
    |cmd: &mut TakeLeaveCommand| {
        cmd.parting = String::from(" ");
        Ok(())
    },
    ValidationError::BlankFarewell,
    "farewell messages must contain visible characters"
)]
#[case::zero_reminder(
    |cmd: &mut TakeLeaveCommand| {
        cmd.remind_in = Some(0);
        Ok(())
    },
    ValidationError::ReminderOutOfRange,
    "reminder minutes must be greater than zero"
)]
#[case::blank_gift(
    |cmd: &mut TakeLeaveCommand| {
        cmd.gift = Some(String::from("   "));
        Ok(())
    },
    ValidationError::BlankGift,
    "gift descriptions must contain visible characters"
)]
#[case::blank_greeting_preamble(
    |cmd: &mut TakeLeaveCommand| {
        cmd.greeting_preamble = Some(String::from("   "));
        Ok(())
    },
    ValidationError::BlankPreamble,
    "preambles must contain visible characters when supplied"
)]
#[case::blank_greeting_punctuation(
    |cmd: &mut TakeLeaveCommand| {
        cmd.greeting_punctuation = Some(String::from("   "));
        Ok(())
    },
    ValidationError::BlankPunctuation,
    "greeting punctuation must contain visible characters"
)]
fn take_leave_command_validation_errors<F>(
    take_leave_command: TakeLeaveCommandFixture,
    #[case] setup: F,
    #[case] expected_error: ValidationError,
    #[case] expected_message: &str,
) -> Result<()>
where
    F: Fn(&mut TakeLeaveCommand) -> Result<()>,
{
    let mut command = take_leave_command?;
    setup(&mut command)?;
    let Err(err) = command.validate() else {
        return Err(anyhow!("expected validation to fail"));
    };
    ensure!(
        err == expected_error,
        "unexpected validation error: {err:?}"
    );
    ensure!(
        err.to_string() == expected_message,
        "unexpected validation message"
    );
    Ok(())
}

#[rstest]
fn load_global_config_applies_overrides() -> Result<()> {
    let cli = parse_command_line(&["-r", "Team", "-s", "Hi", "greet"])?;
    let config = with_jail(|jail| {
        jail.clear_env();
        jail.set_env("HELLO_WORLD_RECIPIENT", "Team");
        jail.create_file(".hello_world.toml", "")?;
        jail.set_env("HELLO_WORLD_SALUTATIONS", "Hi");
        load_global_config(&cli.globals, None).map_err(|err| figment_error(&err))
    })?;
    ensure!(
        config.recipient == "Team",
        "unexpected recipient: {}",
        config.recipient
    );
    ensure!(
        config.trimmed_salutations() == vec![String::from("Hi")],
        "unexpected salutations"
    );
    Ok(())
}

#[rstest]
fn load_global_config_preserves_env_when_not_overridden() -> Result<()> {
    let cli = parse_command_line(&["greet"])?;
    let config = with_jail(|jail| {
        jail.clear_env();
        jail.set_env("HELLO_WORLD_RECIPIENT", "Library");
        load_global_config(&cli.globals, None).map_err(|err| figment_error(&err))
    })?;
    ensure!(
        config.recipient == "Library",
        "unexpected recipient: {}",
        config.recipient
    );
    Ok(())
}

fn assert_sample_greet_defaults(greet: &GreetCommand) -> Result<()> {
    ensure!(
        greet.preamble.as_deref() == Some("Layered hello"),
        "unexpected sample greet preamble: {:?}",
        greet.preamble
    );
    ensure!(
        greet.punctuation == "!!!",
        "unexpected sample punctuation: {}",
        greet.punctuation
    );
    Ok(())
}

#[rstest]
fn load_sample_configuration() -> Result<()> {
    let (config, greet_defaults) = with_jail(|jail| {
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
        let config =
            load_global_config(&GlobalArgs::default(), None).map_err(|err| figment_error(&err))?;
        let greet_defaults = load_greet_defaults().map_err(|err| figment_error(&err))?;
        Ok((config, greet_defaults))
    })?;
    ensure!(config.recipient == "Excited crew", "unexpected recipient");
    ensure!(
        config.trimmed_salutations()
            == vec![String::from("Hello"), String::from("Hey config friends")],
        "unexpected salutations"
    );
    ensure!(config.is_excited, "expected excited configuration");
    assert_sample_greet_defaults(&greet_defaults)?;
    Ok(())
}

#[rstest]
fn load_config_overrides_returns_none_without_files() -> Result<()> {
    let overrides = with_jail(|jail| {
        jail.clear_env();
        load_config_overrides().map_err(|err| figment_error(&err))
    })?;
    ensure!(overrides.is_none(), "expected overrides to be absent");
    Ok(())
}

#[rstest]
fn load_config_overrides_uses_explicit_path() -> Result<()> {
    let overrides = with_jail(|jail| {
        jail.clear_env();
        jail.create_file(
            "custom.toml",
            r#"is_excited = true

[cmds.greet]
preamble = "From explicit path"
punctuation = "?"
"#,
        )?;
        jail.set_env("HELLO_WORLD_CONFIG_PATH", "custom.toml");
        load_config_overrides().map_err(|err| figment_error(&err))
    })?
    .ok_or_else(|| anyhow!("expected overrides"))?;
    ensure!(
        overrides.is_excited == Some(true),
        "unexpected excitement override"
    );
    ensure!(
        overrides.cmds.greet
            == Some(GreetOverrides {
                preamble: Some(String::from("From explicit path")),
                punctuation: Some(String::from("?")),
            }),
        "unexpected greet overrides"
    );
    Ok(())
}

#[rstest]
fn load_config_overrides_prefers_xdg_directories() -> Result<()> {
    let overrides = with_jail(|jail| {
        jail.clear_env();
        jail.create_dir("xdg")?;
        jail.create_dir("xdg/hello_world")?;
        jail.create_file(
            "xdg/hello_world/hello_world.toml",
            r#"[cmds.greet]
punctuation = "???"
"#,
        )?;
        jail.create_file(
            ".hello_world.toml",
            r#"[cmds.greet]
punctuation = "!!!"
"#,
        )?;
        jail.set_env("XDG_CONFIG_HOME", "xdg");
        load_config_overrides().map_err(|err| figment_error(&err))
    })?
    .ok_or_else(|| anyhow!("expected overrides"))?;
    ensure!(
        overrides.is_excited.is_none(),
        "unexpected excitement override"
    );
    ensure!(
        overrides.cmds.greet
            == Some(GreetOverrides {
                preamble: None,
                punctuation: Some(String::from("???")),
            }),
        "unexpected greet overrides"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn load_config_overrides_uses_xdg_fallback() -> Result<()> {
    let candidates = collect_config_candidates();
    ensure!(
        candidates.contains(&Utf8PathBuf::from("/etc/xdg/hello_world/hello_world.toml")),
        "expected fallback hello world config in candidate list"
    );
    ensure!(
        candidates.contains(&Utf8PathBuf::from("/etc/xdg/.hello_world.toml")),
        "expected fallback dotfile config in candidate list"
    );
    Ok(())
}

#[rstest]
fn load_config_overrides_reads_localappdata() -> Result<()> {
    let overrides = with_jail(|jail| {
        jail.clear_env();
        jail.create_dir("localdata")?;
        jail.create_dir("localdata/hello_world")?;
        jail.create_file(
            "localdata/hello_world/hello_world.toml",
            "is_excited = true",
        )?;
        jail.create_file(".hello_world.toml", "is_excited = false")?;
        jail.set_env("LOCALAPPDATA", "localdata");
        load_config_overrides().map_err(|err| figment_error(&err))
    })?
    .ok_or_else(|| anyhow!("expected overrides"))?;
    ensure!(
        overrides.is_excited == Some(true),
        "unexpected excitement override"
    );
    Ok(())
}

#[rstest]
fn apply_greet_overrides_updates_command(greet_command: GreetCommandFixture) -> Result<()> {
    let mut command = greet_command?;
    with_jail(|jail| {
        jail.clear_env();
        jail.create_file(
            ".hello_world.toml",
            r#"[cmds.greet]
preamble = "From file"
punctuation = "?!"
"#,
        )?;
        apply_greet_overrides(&mut command).map_err(|err| figment_error(&err))
    })?;
    ensure!(
        command.preamble.as_deref() == Some("From file"),
        "unexpected preamble override"
    );
    ensure!(
        command.punctuation == "?!",
        "unexpected punctuation override"
    );
    Ok(())
}

fn parse_command_line(args: &[&str]) -> Result<CommandLine> {
    let mut full_args = Vec::with_capacity(args.len() + 1);
    full_args.push("hello-world");
    full_args.extend_from_slice(args);
    CommandLine::try_parse_from(full_args).context("parse command line")
}

fn assert_greet_command(cli: CommandLine) -> Result<()> {
    ensure!(cli.config_path.is_none(), "unexpected config path override");
    ensure!(
        cli.globals.recipient.as_deref() == Some("Crew"),
        "unexpected recipient: {:?}",
        cli.globals.recipient
    );
    ensure!(
        cli.globals.salutations == vec![String::from("Hi")],
        "unexpected salutations"
    );
    let greet = expect_greet(cli.command)?;
    ensure!(
        greet.preamble.as_deref() == Some("Good morning"),
        "unexpected preamble"
    );
    ensure!(greet.punctuation == "?!", "unexpected punctuation");
    Ok(())
}

fn assert_take_leave_command(cli: CommandLine) -> Result<()> {
    ensure!(cli.config_path.is_none(), "unexpected config path override");
    ensure!(cli.globals.is_excited, "expected excited global flags");
    let command = expect_take_leave(cli.command)?;
    ensure!(command.parting == "Cheerio", "unexpected parting");
    ensure!(
        command.gift.as_deref() == Some("flowers"),
        "unexpected gift"
    );
    ensure!(command.remind_in == Some(20), "unexpected reminder");
    ensure!(
        command.channel == Some(FarewellChannel::Message),
        "unexpected farewell channel"
    );
    ensure!(command.wave, "expected wave flag");
    Ok(())
}

fn expect_greet(command: Commands) -> Result<GreetCommand> {
    match command {
        Commands::Greet(greet) => Ok(greet),
        Commands::TakeLeave(_) => Err(anyhow!("expected greet command, found take-leave")),
    }
}

fn expect_take_leave(command: Commands) -> Result<TakeLeaveCommand> {
    match command {
        Commands::TakeLeave(take_leave) => Ok(take_leave),
        Commands::Greet(_) => Err(anyhow!("expected take-leave command, found greet")),
    }
}

fn figment_error<E: ToString>(err: &E) -> figment::Error {
    figment::Error::from(err.to_string())
}

fn with_jail<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<T>,
{
    let mut output = None;
    figment::Jail::try_with(|j| {
        output = Some(f(j)?);
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    output.ok_or_else(|| anyhow!("jail closure did not return a value"))
}
