//! CLI configuration for the `hello_world` example.
//!
//! Binds CLI, environment, and default layers via `OrthoConfig` so tests can
//! drive the binary with predictable inputs.
use std::ffi::OsString;

use clap::{ArgAction, ArgMatches, Args, Parser, Subcommand, ValueEnum};
use ortho_config::OrthoConfig;
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
#[derive(Debug, Default, Clone, PartialEq, Eq, Args)]
pub struct GlobalArgs {
    /// Recipient of the greeting when supplied on the CLI.
    #[arg(short = 'r', long = "recipient", value_name = "NAME", id = "recipient")]
    pub recipient: Option<String>,
    /// Replacement salutations supplied on the CLI.
    #[arg(
        short = 's',
        long = "salutation",
        value_name = "WORD",
        id = "salutations"
    )]
    pub salutations: Vec<String>,
    /// Enables an enthusiastic delivery mode from the CLI.
    #[arg(long = "is-excited", action = ArgAction::SetTrue, id = "is_excited")]
    pub is_excited: bool,
    /// Enables a quiet delivery mode from the CLI.
    #[arg(long = "is-quiet", action = ArgAction::SetTrue, id = "is_quiet")]
    pub is_quiet: bool,
}

/// Subcommands implemented by the example.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Commands {
    /// Prints a greeting using the configured style.
    #[command(name = "greet")]
    Greet(GreetCommand),
    /// Says goodbye whilst describing how the farewell will be delivered.
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
        if let Some(text) = &self.preamble {
            if text.trim().is_empty() {
                return Err(ValidationError::BlankPreamble);
            }
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
    /// Describes how the farewell follow-up will be delivered.
    #[arg(long = "channel", value_enum, id = "channel")]
    pub channel: Option<FarewellChannel>,
    /// Optional reminder delay in minutes.
    #[arg(long = "remind-in", value_name = "MINUTES", id = "remind_in")]
    pub remind_in: Option<u16>,
    /// Optional gift noted in the farewell.
    #[arg(long, value_name = "ITEM", id = "gift")]
    pub gift: Option<String>,
    /// Records whether the caller waves whilst leaving.
    #[arg(long, action = ArgAction::SetTrue, id = "wave")]
    #[ortho_config(default = false)]
    pub wave: bool,
}

impl Default for TakeLeaveCommand {
    fn default() -> Self {
        Self {
            parting: default_parting(),
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
        if self.parting.trim().is_empty() {
            return Err(ValidationError::BlankFarewell);
        }
        if let Some(minutes) = self.remind_in {
            if minutes == 0 {
                return Err(ValidationError::ReminderOutOfRange);
            }
        }
        if let Some(gift) = &self.gift {
            if gift.trim().is_empty() {
                return Err(ValidationError::BlankGift);
            }
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
    pub fn describe(self) -> &'static str {
        match self {
            FarewellChannel::Message => "a message",
            FarewellChannel::Call => "a call",
            FarewellChannel::Email => "an email",
        }
    }
}

/// Resolves the global configuration by layering defaults with CLI overrides.
pub fn load_global_config(matches: &ArgMatches) -> Result<HelloWorldCli, HelloWorldError> {
    let mut args = vec![OsString::from("hello-world")];

    if let Some(recipient) = matches.get_one::<String>("recipient") {
        args.push(OsString::from("-r"));
        args.push(OsString::from(recipient));
    }

    let salutation_overrides: Vec<String> = matches
        .get_many::<String>("salutations")
        .map(|values| values.cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    for value in &salutation_overrides {
        args.push(OsString::from("-s"));
        args.push(OsString::from(value));
    }

    if matches.get_flag("is_excited") {
        args.push(OsString::from("--is-excited"));
    }
    if matches.get_flag("is_quiet") {
        args.push(OsString::from("--is-quiet"));
    }

    let mut config = HelloWorldCli::load_from_iter(args)?;

    if !salutation_overrides.is_empty() {
        config.salutations = salutation_overrides
            .into_iter()
            .map(|value| value.trim().to_string())
            .collect();
    }

    config.validate()?;
    Ok(config)
}

/// Top-level configuration for the hello world demo.
///
/// The struct collects the global options exposed by the example, keeping
/// fields public so the command dispatcher can inspect the resolved values
/// without extra accessor boilerplate.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, OrthoConfig)]
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
    use clap::CommandFactory;
    use rstest::{fixture, rstest};

    #[fixture]
    fn base_cli() -> HelloWorldCli {
        HelloWorldCli::default()
    }

    #[fixture]
    fn greet_command() -> GreetCommand {
        GreetCommand::default()
    }

    #[fixture]
    fn take_leave_command() -> TakeLeaveCommand {
        TakeLeaveCommand::default()
    }

    #[rstest]
    fn default_configuration_is_valid(base_cli: HelloWorldCli) {
        base_cli.validate().expect("default config is valid");
    }

    #[rstest]
    fn conflicting_delivery_modes_are_rejected(mut base_cli: HelloWorldCli) {
        base_cli.is_excited = true;
        base_cli.is_quiet = true;
        let err = base_cli
            .validate()
            .expect_err("conflicting modes should fail");
        assert_eq!(err, ValidationError::ConflictingDeliveryModes);
    }

    #[rstest]
    fn missing_salutation_is_rejected(mut base_cli: HelloWorldCli) {
        base_cli.salutations.clear();
        let err = base_cli
            .validate()
            .expect_err("missing salutation should fail");
        assert_eq!(err, ValidationError::MissingSalutation);
    }

    #[rstest]
    fn blank_salutation_is_rejected(mut base_cli: HelloWorldCli) {
        base_cli.salutations[0] = String::from("   ");
        let err = base_cli
            .validate()
            .expect_err("blank salutation should fail");
        assert_eq!(err, ValidationError::BlankSalutation(0));
    }

    #[rstest]
    #[case(false, false, DeliveryMode::Standard)]
    #[case(true, false, DeliveryMode::Enthusiastic)]
    #[case(false, true, DeliveryMode::Quiet)]
    fn delivery_mode_resolves_preference(
        mut base_cli: HelloWorldCli,
        #[case] is_excited: bool,
        #[case] is_quiet: bool,
        #[case] expected: DeliveryMode,
    ) {
        base_cli.is_excited = is_excited;
        base_cli.is_quiet = is_quiet;
        assert_eq!(base_cli.delivery_mode(), expected);
    }

    #[rstest]
    fn trimmed_salutations_strip_whitespace(mut base_cli: HelloWorldCli) {
        base_cli.salutations = vec![String::from("  Hello"), String::from("world  ")];
        let trimmed = base_cli.trimmed_salutations();
        assert_eq!(trimmed, vec![String::from("Hello"), String::from("world")],);
    }

    #[rstest]
    fn greet_command_rejects_blank_punctuation(mut greet_command: GreetCommand) {
        greet_command.punctuation = String::from("   ");
        let err = greet_command
            .validate()
            .expect_err("blank punctuation should fail");
        assert_eq!(err, ValidationError::BlankPunctuation);
    }

    #[rstest]
    fn greet_command_rejects_blank_preamble(mut greet_command: GreetCommand) {
        greet_command.preamble = Some(String::from("   "));
        let err = greet_command
            .validate()
            .expect_err("blank preamble should fail");
        assert_eq!(err, ValidationError::BlankPreamble);
    }

    #[rstest]
    fn take_leave_command_rejects_blank_parting(mut take_leave_command: TakeLeaveCommand) {
        take_leave_command.parting = String::from(" ");
        let err = take_leave_command
            .validate()
            .expect_err("blank parting should fail");
        assert_eq!(err, ValidationError::BlankFarewell);
    }

    #[rstest]
    fn take_leave_command_rejects_zero_reminder(mut take_leave_command: TakeLeaveCommand) {
        take_leave_command.remind_in = Some(0);
        let err = take_leave_command
            .validate()
            .expect_err("zero reminder should fail");
        assert_eq!(err, ValidationError::ReminderOutOfRange);
    }

    #[rstest]
    fn take_leave_command_rejects_blank_gift(mut take_leave_command: TakeLeaveCommand) {
        take_leave_command.gift = Some(String::from("   "));
        let err = take_leave_command
            .validate()
            .expect_err("blank gift should fail");
        assert_eq!(err, ValidationError::BlankGift);
    }

    #[rstest]
    fn load_global_config_applies_overrides() {
        ortho_config::figment::Jail::expect_with(|jail| {
            jail.clear_env();
            let matches = CommandLine::command()
                .try_get_matches_from(["hello-world", "-r", "Team", "-s", "Hi", "greet"])
                .expect("parse CLI");
            let config = load_global_config(&matches).expect("load config");
            assert_eq!(config.recipient, "Team");
            assert_eq!(config.trimmed_salutations(), vec![String::from("Hi")]);
            Ok(())
        });
    }

    #[rstest]
    fn load_global_config_preserves_env_when_not_overridden() {
        ortho_config::figment::Jail::expect_with(|jail| {
            jail.clear_env();
            jail.set_env("HELLO_WORLD_RECIPIENT", "Library");
            let matches = CommandLine::command()
                .try_get_matches_from(["hello-world", "greet"])
                .expect("parse CLI");
            let config = load_global_config(&matches).expect("load config");
            assert_eq!(config.recipient, "Library");
            Ok(())
        });
    }
}
