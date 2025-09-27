//! CLI configuration for the `hello_world` example.
//!
//! Binds CLI, environment, and default layers via `OrthoConfig` so tests can
//! drive the binary with predictable inputs.
use std::ffi::OsString;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use ortho_config::{OrthoConfig, OrthoMergeExt};
use serde::{Deserialize, Serialize};

use crate::error::{HelloWorldError, ValidationError};

/// Command-line surface exposed by the example.
#[derive(Debug, Parser)]
#[command(
    name = "hello-world",
    about = "Friendly greeting demo showcasing OrthoConfig layering",
    version
)]
pub struct CommandLine {
    /// Global switches shared by every subcommand.
    #[command(flatten)]
    pub globals: GlobalArgs,
    /// Selected workflow to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// CLI overrides for the global greeting options.
#[derive(Debug, Default, Clone, PartialEq, Eq, Args, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "HELLO_WORLD_")]
pub struct GlobalArgs {
    /// Recipient of the greeting when supplied on the CLI.
    #[arg(short = 'r', long = "recipient", value_name = "NAME", id = "recipient")]
    #[ortho_config(cli_short = 'r')]
    pub recipient: Option<String>,
    /// Replacement salutations supplied on the CLI.
    #[arg(
        short = 's',
        long = "salutation",
        value_name = "WORD",
        id = "salutations"
    )]
    #[ortho_config(cli_short = 's')]
    pub salutations: Vec<String>,
    /// Enables an enthusiastic delivery mode from the CLI.
    #[arg(long = "is-excited", action = ArgAction::SetTrue, id = "is_excited")]
    #[ortho_config(default = false)]
    pub is_excited: bool,
    /// Enables a quiet delivery mode from the CLI.
    #[arg(long = "is-quiet", action = ArgAction::SetTrue, id = "is_quiet")]
    #[ortho_config(default = false)]
    pub is_quiet: bool,
}

/// Subcommands implemented by the example.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Commands {
    /// Prints a greeting using the configured style.
    #[command(name = "greet")]
    Greet(GreetCommand),
    /// Says goodbye while describing how the farewell will be delivered.
    #[command(name = "take-leave")]
    TakeLeave(TakeLeaveCommand),
}

/// Customisation options for the `greet` command.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "HELLO_WORLD_")]
pub struct GreetCommand {
    /// Optional preamble printed before the greeting.
    #[arg(long, value_name = "PHRASE", id = "preamble")]
    pub preamble: Option<String>,
    /// Punctuation appended to the greeting when not whispered.
    #[arg(
        long,
        value_name = "TEXT",
        id = "punctuation",
        default_value_t = default_punctuation()
    )]
    #[ortho_config(default = default_punctuation())]
    pub punctuation: String,
}

impl Default for GreetCommand {
    fn default() -> Self {
        Self {
            preamble: None,
            punctuation: default_punctuation(),
        }
    }
}

impl GreetCommand {
    /// Ensures user-provided options are well formed.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.punctuation.trim().is_empty() {
            return Err(ValidationError::BlankPunctuation);
        }
        if self
            .preamble
            .as_deref()
            .is_some_and(|text| text.trim().is_empty())
        {
            return Err(ValidationError::BlankPreamble);
        }
        Ok(())
    }
}

fn default_punctuation() -> String {
    String::from("!")
}

/// Options controlling the `take-leave` workflow.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "HELLO_WORLD_")]
pub struct TakeLeaveCommand {
    /// Parting phrase to use when saying goodbye.
    #[arg(
        long,
        value_name = "PHRASE",
        id = "parting",
        default_value_t = default_parting()
    )]
    #[ortho_config(default = default_parting())]
    pub parting: String,
    /// Optional preamble printed before the farewell greeting.
    #[arg(long = "preamble", value_name = "PHRASE", id = "farewell_preamble")]
    pub greeting_preamble: Option<String>,
    /// Optional punctuation override appended to the farewell greeting.
    #[arg(long = "punctuation", value_name = "TEXT", id = "farewell_punctuation")]
    pub greeting_punctuation: Option<String>,
    /// Describes how the farewell follow-up will be delivered.
    #[arg(long = "channel", value_enum, id = "channel")]
    pub channel: Option<FarewellChannel>,
    /// Optional reminder delay in minutes.
    #[arg(long = "remind-in", value_name = "MINUTES", id = "remind_in")]
    pub remind_in: Option<u16>,
    /// Optional gift noted in the farewell.
    #[arg(long, value_name = "ITEM", id = "gift")]
    pub gift: Option<String>,
    /// Records whether the caller waves while leaving.
    #[arg(long, action = ArgAction::SetTrue, id = "wave")]
    #[ortho_config(default = false)]
    pub wave: bool,
}

impl Default for TakeLeaveCommand {
    fn default() -> Self {
        Self {
            parting: default_parting(),
            greeting_preamble: None,
            greeting_punctuation: None,
            channel: None,
            remind_in: None,
            gift: None,
            wave: false,
        }
    }
}

impl TakeLeaveCommand {
    /// Validates caller-provided farewell customisation.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_greeting_overrides()?;
        self.validate_parting()?;
        self.validate_reminder()?;
        self.validate_gift()?;
        Ok(())
    }

    fn validate_parting(&self) -> Result<(), ValidationError> {
        if self.parting.trim().is_empty() {
            return Err(ValidationError::BlankFarewell);
        }
        Ok(())
    }

    fn validate_reminder(&self) -> Result<(), ValidationError> {
        if self.remind_in.is_some_and(|minutes| minutes == 0) {
            return Err(ValidationError::ReminderOutOfRange);
        }
        Ok(())
    }

    fn validate_gift(&self) -> Result<(), ValidationError> {
        if self
            .gift
            .as_deref()
            .is_some_and(|gift| gift.trim().is_empty())
        {
            return Err(ValidationError::BlankGift);
        }
        Ok(())
    }

    fn validate_greeting_overrides(&self) -> Result<(), ValidationError> {
        if self
            .greeting_preamble
            .as_deref()
            .is_some_and(|text| text.trim().is_empty())
        {
            return Err(ValidationError::BlankPreamble);
        }
        if self
            .greeting_punctuation
            .as_deref()
            .is_some_and(|text| text.trim().is_empty())
        {
            return Err(ValidationError::BlankPunctuation);
        }
        Ok(())
    }
}

fn default_parting() -> String {
    String::from("Take care")
}

/// Delivery channels supported by the `take-leave` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum FarewellChannel {
    /// Sends a follow-up via instant message.
    Message,
    /// Schedules a quick voice call.
    Call,
    /// Dispatches a friendly email.
    Email,
}

impl FarewellChannel {
    #[must_use]
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "Borrowed API avoids consuming FarewellChannel values when describing them."
    )]
    pub fn describe(&self) -> &'static str {
        match self {
            FarewellChannel::Message => "a message",
            FarewellChannel::Call => "a call",
            FarewellChannel::Email => "an email",
        }
    }
}

/// Resolves the global configuration by layering defaults with CLI overrides.
pub fn load_global_config(globals: &GlobalArgs) -> Result<HelloWorldCli, HelloWorldError> {
    #[derive(Serialize)]
    struct Overrides<'a> {
        #[serde(skip_serializing_if = "Option::is_none")]
        recipient: Option<&'a String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        salutations: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_excited: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_quiet: Option<bool>,
    }

    let binary = std::env::args_os()
        .next()
        .unwrap_or_else(|| OsString::from("hello-world"));
    let base = HelloWorldCli::load_from_iter(std::iter::once(binary))?;
    let salutations = if globals.salutations.is_empty() {
        None
    } else {
        Some(
            globals
                .salutations
                .iter()
                .map(|value| value.trim().to_string())
                .collect(),
        )
    };
    let overrides = Overrides {
        recipient: globals.recipient.as_ref(),
        salutations,
        is_excited: globals.is_excited.then_some(true),
        is_quiet: globals.is_quiet.then_some(true),
    };
    let figment = ortho_config::figment::Figment::from(
        ortho_config::figment::providers::Serialized::defaults(&base),
    )
    .merge(ortho_config::sanitized_provider(&overrides)?);
    let merged = figment.extract::<HelloWorldCli>().into_ortho_merge()?;
    merged.validate()?;
    Ok(merged)
}

/// Top-level configuration for the hello world demo.
///
/// The struct collects the global options exposed by the example, keeping
/// fields public so the command dispatcher can inspect the resolved values
/// without extra accessor boilerplate.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "HELLO_WORLD_")]
pub struct HelloWorldCli {
    /// Recipient of the greeting. Defaults to a friendly placeholder.
    #[ortho_config(default = default_recipient(), cli_short = 'r')]
    pub recipient: String,
    /// Words used to open the greeting. Demonstrates repeated parameters.
    #[ortho_config(default = default_salutations(), cli_short = 's')]
    pub salutations: Vec<String>,
    /// Enables an enthusiastic delivery mode.
    #[ortho_config(default = false)]
    pub is_excited: bool,
    /// Selects a quiet delivery mode.
    #[ortho_config(default = false)]
    pub is_quiet: bool,
}

impl Default for HelloWorldCli {
    fn default() -> Self {
        Self {
            recipient: default_recipient(),
            salutations: default_salutations(),
            is_excited: false,
            is_quiet: false,
        }
    }
}

/// Delivery strategy derived from the global switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    /// Standard delivery keeps the message as-is.
    Standard,
    /// Enthusiastic delivery shouts the greeting.
    Enthusiastic,
    /// Quiet delivery whispers the message.
    Quiet,
}

impl HelloWorldCli {
    #[inline]
    fn has_conflicting_modes(&self) -> bool {
        self.is_excited && self.is_quiet
    }

    /// Validates the resolved configuration before execution.
    ///
    /// # Examples
    /// ```ignore
    /// # use hello_world::cli::{DeliveryMode, HelloWorldCli};
    /// let mut cli = HelloWorldCli::default();
    /// cli.is_excited = true;
    /// cli.validate().unwrap();
    /// assert_eq!(cli.delivery_mode(), DeliveryMode::Enthusiastic);
    /// ```
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.has_conflicting_modes() {
            return Err(ValidationError::ConflictingDeliveryModes);
        }
        if self.salutations.is_empty() {
            return Err(ValidationError::MissingSalutation);
        }
        for (index, word) in self.salutations.iter().enumerate() {
            if word.trim().is_empty() {
                return Err(ValidationError::BlankSalutation(index));
            }
        }
        Ok(())
    }

    /// Calculates the delivery mode once the configuration is valid.
    #[must_use]
    pub fn delivery_mode(&self) -> DeliveryMode {
        debug_assert!(
            !self.has_conflicting_modes(),
            "Call validate() before delivery_mode(); conflicting flags set",
        );
        match (self.is_excited, self.is_quiet) {
            (true, false) => DeliveryMode::Enthusiastic,
            (false, true) => DeliveryMode::Quiet,
            _ => DeliveryMode::Standard,
        }
    }

    /// Strips incidental whitespace from salutations for consistent output.
    ///
    /// # Examples
    /// ```ignore
    /// # use hello_world::cli::HelloWorldCli;
    /// let mut cli = HelloWorldCli::default();
    /// cli.salutations = vec!["  Hello".into(), "world  ".into()];
    /// assert_eq!(cli.trimmed_salutations(), vec!["Hello", "world"]);
    /// ```
    #[must_use]
    pub fn trimmed_salutations(&self) -> Vec<String> {
        self.salutations
            .iter()
            .map(|word| String::from(word.trim()))
            .collect()
    }
}

fn default_recipient() -> String {
    String::from("World")
}

fn default_salutations() -> Vec<String> {
    vec![String::from("Hello")]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ValidationError;
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
    #[case::blank_parting(|cmd: &mut TakeLeaveCommand| cmd.parting = String::from(" "), ValidationError::BlankFarewell, "farewell messages must contain visible characters")]
    #[case::zero_reminder(|cmd: &mut TakeLeaveCommand| cmd.remind_in = Some(0), ValidationError::ReminderOutOfRange, "reminder minutes must be greater than zero")]
    #[case::blank_gift(|cmd: &mut TakeLeaveCommand| cmd.gift = Some(String::from("   ")), ValidationError::BlankGift, "gift descriptions must contain visible characters")]
    #[case::blank_greeting_preamble(|cmd: &mut TakeLeaveCommand| cmd.greeting_preamble = Some(String::from("   ")), ValidationError::BlankPreamble, "preambles must contain visible characters when supplied")]
    #[case::blank_greeting_punctuation(|cmd: &mut TakeLeaveCommand| cmd.greeting_punctuation = Some(String::from("   ")), ValidationError::BlankPunctuation, "greeting punctuation must contain visible characters")]
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
            let cli =
                CommandLine::try_parse_from(["hello-world", "-r", "Team", "-s", "Hi", "greet"])
                    .expect("parse CLI");
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
            let cli = CommandLine::try_parse_from(["hello-world", "greet"]).expect("parse CLI");
            let config = load_global_config(&cli.globals).expect("load config");
            assert_eq!(config.recipient, "Library");
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
}
