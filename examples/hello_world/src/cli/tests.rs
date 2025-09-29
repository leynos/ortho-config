use super::*;
use crate::error::ValidationError;
use camino::Utf8PathBuf;
use rstest::{fixture, rstest};

/// Provides a default CLI configuration for tests.
#[fixture]
fn base_cli() -> HelloWorldCli {
    HelloWorldCli::default()
}

/// Provides a default greet command for tests.
#[fixture]
fn greet_command() -> GreetCommand {
    GreetCommand::default()
}

/// Provides a default take-leave command for tests.
#[fixture]
fn take_leave_command() -> TakeLeaveCommand {
    TakeLeaveCommand::default()
}

type CommandAssertion = fn(CommandLine);

/// Parses command-line invocations and asserts the resulting command variant.
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
) {
    let cli = parse_command_line(args);
    assert_cli(cli);
}

/// Ensures the hello world CLI rejects conflicting delivery modes.
#[rstest]
fn hello_world_cli_detects_conflicting_modes(mut base_cli: HelloWorldCli) {
    base_cli.is_excited = true;
    base_cli.is_quiet = true;
    let err = base_cli.validate().expect_err("validation should fail");
    assert_eq!(err, ValidationError::ConflictingDeliveryModes);
}

/// Enumerates validation errors for the global CLI options.
#[rstest]
#[case::missing_salutations(
    |cli: &mut HelloWorldCli| cli.salutations.clear(),
    ValidationError::MissingSalutation
)]
#[case::blank_salutation(
    |cli: &mut HelloWorldCli| cli.salutations[0] = String::from("   "),
    ValidationError::BlankSalutation(0)
)]
fn hello_world_cli_validation_errors<F>(
    mut base_cli: HelloWorldCli,
    #[case] mutate: F,
    #[case] expected: ValidationError,
) where
    F: Fn(&mut HelloWorldCli),
{
    mutate(&mut base_cli);
    let err = base_cli.validate().expect_err("validation should fail");
    assert_eq!(err, expected);
}

/// Derives the delivery mode based on global CLI flags.
#[rstest]
#[case::excited(true, false, DeliveryMode::Enthusiastic)]
#[case::quiet(false, true, DeliveryMode::Quiet)]
#[case::standard(false, false, DeliveryMode::Standard)]
fn delivery_mode_from_flags(
    mut base_cli: HelloWorldCli,
    #[case] excited: bool,
    #[case] quiet: bool,
    #[case] expected: DeliveryMode,
) {
    base_cli.is_excited = excited;
    base_cli.is_quiet = quiet;
    assert_eq!(base_cli.delivery_mode(), expected);
}

/// Trims incidental whitespace from salutation overrides.
#[rstest]
fn trimmed_salutations_remove_whitespace(mut base_cli: HelloWorldCli) {
    base_cli.salutations = vec![String::from("  Hi"), String::from("Team  ")];
    assert_eq!(
        base_cli.trimmed_salutations(),
        vec![String::from("Hi"), String::from("Team")]
    );
}

/// Rejects blank inputs supplied to the greet command.
#[rstest]
#[case::punctuation(
    |command: &mut GreetCommand| command.punctuation = String::from("   "),
    ValidationError::BlankPunctuation,
    "greeting punctuation must contain visible characters",
)]
#[case::preamble(
    |command: &mut GreetCommand| command.preamble = Some(String::from("   ")),
    ValidationError::BlankPreamble,
    "preambles must contain visible characters when supplied",
)]
fn greet_command_rejects_blank_inputs<F>(
    mut greet_command: GreetCommand,
    #[case] mutate: F,
    #[case] expected_error: ValidationError,
    #[case] expected_message: &str,
) where
    F: Fn(&mut GreetCommand),
{
    mutate(&mut greet_command);
    let err = greet_command
        .validate()
        .expect_err("validation should fail");
    assert_eq!(err, expected_error);
    assert_eq!(err.to_string(), expected_message);
}

/// Enumerates validation errors raised by the take-leave command.
#[rstest]
#[case::blank_parting(
    |cmd: &mut TakeLeaveCommand| cmd.parting = String::from(" "),
    ValidationError::BlankFarewell,
    "farewell messages must contain visible characters"
)]
#[case::zero_reminder(
    |cmd: &mut TakeLeaveCommand| cmd.remind_in = Some(0),
    ValidationError::ReminderOutOfRange,
    "reminder minutes must be greater than zero"
)]
#[case::blank_gift(
    |cmd: &mut TakeLeaveCommand| cmd.gift = Some(String::from("   ")),
    ValidationError::BlankGift,
    "gift descriptions must contain visible characters"
)]
#[case::blank_greeting_preamble(
    |cmd: &mut TakeLeaveCommand| cmd.greeting_preamble = Some(String::from("   ")),
    ValidationError::BlankPreamble,
    "preambles must contain visible characters when supplied"
)]
#[case::blank_greeting_punctuation(
    |cmd: &mut TakeLeaveCommand| cmd.greeting_punctuation = Some(String::from("   ")),
    ValidationError::BlankPunctuation,
    "greeting punctuation must contain visible characters"
)]
fn take_leave_command_validation_errors<F>(
    mut take_leave_command: TakeLeaveCommand,
    #[case] setup: F,
    #[case] expected_error: ValidationError,
    #[case] expected_message: &str,
) where
    F: Fn(&mut TakeLeaveCommand),
{
    setup(&mut take_leave_command);
    let err = take_leave_command
        .validate()
        .expect_err("validation should fail");
    assert_eq!(err, expected_error);
    assert_eq!(err.to_string(), expected_message);
}

/// Loads configuration by merging CLI and environment sources.
#[rstest]
fn load_global_config_applies_overrides() {
    ortho_config::figment::Jail::expect_with(|jail| {
        jail.clear_env();
        jail.set_env("HELLO_WORLD_RECIPIENT", "Team");
        jail.create_file(".hello_world.toml", "")?;
        jail.set_env("HELLO_WORLD_SALUTATIONS", "Hi");
        let cli = parse_command_line(&["-r", "Team", "-s", "Hi", "greet"]);
        let config = load_global_config(&cli.globals).expect("load config");
        assert_eq!(config.recipient, "Team");
        assert_eq!(config.trimmed_salutations(), vec![String::from("Hi")]);
        Ok(())
    });
}

/// Preserves environment-derived configuration when the CLI omits overrides.
#[rstest]
fn load_global_config_preserves_env_when_not_overridden() {
    ortho_config::figment::Jail::expect_with(|jail| {
        jail.clear_env();
        jail.set_env("HELLO_WORLD_RECIPIENT", "Library");
        let cli = parse_command_line(&["greet"]);
        let config = load_global_config(&cli.globals).expect("load config");
        assert_eq!(config.recipient, "Library");
        Ok(())
    });
}

// Propagate figment errors without erasing diagnostics.
#[expect(
    clippy::result_large_err,
    reason = "figment::Error originates in an external crate"
)]
fn setup_sample_jail(
    jail: &mut ortho_config::figment::Jail,
) -> Result<(), ortho_config::figment::Error> {
    jail.clear_env();
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_dir = cap_std::fs::Dir::open_ambient_dir(
        manifest_dir.join("config").as_std_path(),
        cap_std::ambient_authority(),
    )
    .expect("open hello_world sample config directory");
    let baseline = config_dir
        .read_to_string("baseline.toml")
        .expect("read baseline sample configuration");
    let overrides = config_dir
        .read_to_string("overrides.toml")
        .expect("read overrides sample configuration");
    jail.create_file("baseline.toml", &baseline)?;
    jail.create_file(".hello_world.toml", &overrides)?;
    Ok(())
}

fn assert_sample_greet_defaults(greet: &GreetCommand) {
    assert_eq!(greet.preamble.as_deref(), Some("Layered hello"));
    assert_eq!(greet.punctuation, "!!!");
}

/// Loads the sample configuration shipped with the demo scripts.
#[rstest]
fn load_sample_configuration() {
    ortho_config::figment::Jail::expect_with(|jail| {
        setup_sample_jail(jail)?;
        let config = load_global_config(&GlobalArgs::default()).expect("load config");
        assert_eq!(config.recipient, "Excited crew");
        assert_eq!(
            config.trimmed_salutations(),
            vec![String::from("Hello"), String::from("Hey config friends")]
        );
        assert!(config.is_excited);
        let greet_defaults = load_greet_defaults().expect("merge greet defaults");
        assert_sample_greet_defaults(&greet_defaults);
        Ok(())
    });
}

/// Returns `None` when no configuration file is present.
#[rstest]
fn load_config_overrides_returns_none_without_files() {
    ortho_config::figment::Jail::expect_with(|jail| {
        jail.clear_env();
        assert!(load_config_overrides().expect("load overrides").is_none());
        Ok(())
    });
}

/// Reads overrides from an explicit configuration path when provided.
#[rstest]
fn load_config_overrides_uses_explicit_path() {
    ortho_config::figment::Jail::expect_with(|jail| {
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
        let overrides = load_config_overrides()
            .expect("load overrides")
            .expect("overrides not found");
        assert_eq!(overrides.is_excited, Some(true));
        assert_eq!(
            overrides.cmds.greet,
            Some(GreetOverrides {
                preamble: Some(String::from("From explicit path")),
                punctuation: Some(String::from("?")),
            })
        );
        Ok(())
    });
}

/// Prefers XDG configuration directories before local files.
#[rstest]
fn load_config_overrides_prefers_xdg_directories() {
    ortho_config::figment::Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_dir("xdg")?;
        jail.create_dir("xdg/hello_world")?;
        jail.create_file(
            "xdg/hello_world/config.toml",
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
        let overrides = load_config_overrides()
            .expect("load overrides")
            .expect("expected overrides");
        assert_eq!(overrides.is_excited, None);
        assert_eq!(
            overrides.cmds.greet,
            Some(GreetOverrides {
                preamble: None,
                punctuation: Some(String::from("???")),
            })
        );
        Ok(())
    });
}

/// Applies file overrides to the greet command defaults.
#[rstest]
fn apply_greet_overrides_updates_command(mut greet_command: GreetCommand) {
    ortho_config::figment::Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file(
            ".hello_world.toml",
            r#"[cmds.greet]
preamble = "From file"
punctuation = "?!"
"#,
        )?;
        apply_greet_overrides(&mut greet_command).expect("apply overrides");
        assert_eq!(greet_command.preamble.as_deref(), Some("From file"));
        assert_eq!(greet_command.punctuation, String::from("?!"));
        Ok(())
    });
}

fn parse_command_line(args: &[&str]) -> CommandLine {
    let mut full_args = Vec::with_capacity(args.len() + 1);
    full_args.push("hello-world");
    full_args.extend_from_slice(args);
    CommandLine::try_parse_from(full_args).expect("parse command line")
}

fn assert_greet_command(cli: CommandLine) {
    assert_eq!(cli.globals.recipient.as_deref(), Some("Crew"));
    assert_eq!(cli.globals.salutations, vec![String::from("Hi")]);
    let greet = expect_greet(cli.command);
    assert_eq!(greet.preamble.as_deref(), Some("Good morning"));
    assert_eq!(greet.punctuation, "?!");
}

fn assert_take_leave_command(cli: CommandLine) {
    assert!(cli.globals.is_excited);
    let command = expect_take_leave(cli.command);
    assert_eq!(command.parting, "Cheerio");
    assert_eq!(command.gift.as_deref(), Some("flowers"));
    assert_eq!(command.remind_in, Some(20));
    assert_eq!(command.channel, Some(FarewellChannel::Message));
    assert!(command.wave);
}

fn expect_greet(command: Commands) -> GreetCommand {
    match command {
        Commands::Greet(greet) => greet,
        Commands::TakeLeave(_) => panic!("expected greet command, found take-leave"),
    }
}

fn expect_take_leave(command: Commands) -> TakeLeaveCommand {
    match command {
        Commands::TakeLeave(command) => command,
        Commands::Greet(_) => panic!("expected take-leave command, found greet"),
    }
}
