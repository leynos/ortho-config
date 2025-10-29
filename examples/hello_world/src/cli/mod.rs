//! CLI configuration for the `hello_world` example.
//!
//! Binds CLI, environment, and default layers via `OrthoConfig` so tests can
//! drive the binary with predictable inputs.
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use ortho_config::{OrthoConfig, OrthoMergeExt, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};

use crate::error::{HelloWorldError, ValidationError};

mod discovery;
use self::discovery::discover_config_figment;

/// Command-line surface exposed by the example.
#[derive(Debug, Parser)]
#[command(
    name = "hello-world",
    about = "Friendly greeting demo showcasing OrthoConfig layering",
    version
)]
pub struct CommandLine {
    /// Overrides configuration discovery with an explicit file path.
    #[arg(
        long = "config",
        short = 'c',
        value_name = "PATH",
        global = true,
        help = "Path to the configuration file"
    )]
    pub config_path: Option<PathBuf>,
    /// Global switches shared by every subcommand.
    #[command(flatten)]
    pub globals: GlobalArgs,
    /// Selected workflow to execute.
    #[command(subcommand)]
    pub command: Commands,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde skip hooks receive references to field values"
)]
const fn is_false(value: &bool) -> bool {
    !*value
}

/// CLI overrides for the global greeting options.
#[derive(Debug, Default, Clone, PartialEq, Eq, Args, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "HELLO_WORLD")]
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
    #[serde(skip_serializing_if = "crate::cli::is_false")]
    #[ortho_config(default = false)]
    pub is_excited: bool,
    /// Enables a quiet delivery mode from the CLI.
    #[arg(long = "is-quiet", action = ArgAction::SetTrue, id = "is_quiet")]
    #[serde(skip_serializing_if = "crate::cli::is_false")]
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
#[ortho_config(prefix = "HELLO_WORLD")]
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
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] when punctuation or preamble values are
    /// blank after trimming.
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
#[ortho_config(prefix = "HELLO_WORLD")]
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
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] when any validation check fails.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_greeting_overrides()?;
        self.validate_parting()?;
        self.validate_reminder()?;
        self.validate_gift()?;
        Ok(())
    }

    /// Ensures the farewell parting phrase is not blank.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::BlankFarewell`] when the parting phrase
    /// contains only whitespace.
    fn validate_parting(&self) -> Result<(), ValidationError> {
        if self.parting.trim().is_empty() {
            return Err(ValidationError::BlankFarewell);
        }
        Ok(())
    }

    /// Ensures reminder durations fall within the supported range.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::ReminderOutOfRange`] when the reminder is
    /// present but zero minutes.
    fn validate_reminder(&self) -> Result<(), ValidationError> {
        if self.remind_in.is_some_and(|minutes| minutes == 0) {
            return Err(ValidationError::ReminderOutOfRange);
        }
        Ok(())
    }

    /// Ensures gifts, when provided, are not blank strings.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::BlankGift`] when the caller supplies an
    /// empty gift description.
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
    /// Describes how the farewell will be delivered for user messaging.
    ///
    /// # Examples
    ///
    /// ```
    /// use hello_world::cli::FarewellChannel;
    /// assert_eq!(FarewellChannel::Email.describe(), "an email");
    /// ```
    #[must_use]
    pub const fn describe(&self) -> &'static str {
        match self {
            Self::Message => "a message",
            Self::Call => "a call",
            Self::Email => "an email",
        }
    }
}

#[derive(Serialize)]
struct Overrides<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    recipient: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salutations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_excited: Option<bool>,
    #[serde(skip_serializing_if = "crate::cli::is_false")]
    is_quiet: bool,
}

#[derive(Serialize)]
struct FileLayer {
    #[serde(skip_serializing_if = "Option::is_none")]
    is_excited: Option<bool>,
}

/// Resolves the global configuration by layering defaults with CLI overrides.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when discovery fails or configuration cannot
/// be deserialised.
pub fn load_global_config(
    globals: &GlobalArgs,
    config_override: Option<&Path>,
) -> Result<HelloWorldCli, HelloWorldError> {
    let base = HelloWorldCli::load_from_iter(build_cli_args(config_override).into_iter())?;
    let salutations = trimmed_salutations(globals);
    let file_overrides = load_config_overrides()?;
    let overrides = build_overrides(
        globals,
        salutations,
        file_overrides.as_ref(),
        config_override,
    );

    let mut figment = ortho_config::figment::Figment::from(
        ortho_config::figment::providers::Serialized::defaults(&base),
    );
    if let Some(ref file) = file_overrides {
        figment = figment.merge(ortho_config::figment::providers::Serialized::defaults(
            &FileLayer {
                is_excited: file.is_excited,
            },
        ));
    }
    figment = figment.merge(ortho_config::sanitized_provider(&overrides)?);
    let merged = figment.extract::<HelloWorldCli>().into_ortho_merge()?;
    merged.validate()?;
    Ok(merged)
}

fn build_overrides<'a>(
    globals: &'a GlobalArgs,
    salutations: Option<Vec<String>>,
    file_overrides: Option<&FileOverrides>,
    config_override: Option<&Path>,
) -> Overrides<'a> {
    let file_is_excited = file_excited_value(file_overrides, config_override);
    Overrides {
        recipient: globals.recipient.as_ref(),
        salutations,
        is_excited: globals.is_excited.then_some(true).or(file_is_excited),
        is_quiet: globals.is_quiet,
    }
}

fn build_cli_args(config_override: Option<&Path>) -> Vec<OsString> {
    let binary = std::env::args_os()
        .next()
        .unwrap_or_else(|| OsString::from("hello-world"));
    let mut args = vec![binary];
    if let Some(path) = config_override {
        args.push(OsString::from("--config"));
        args.push(path.as_os_str().to_os_string());
    }
    args
}

fn trimmed_salutations(globals: &GlobalArgs) -> Option<Vec<String>> {
    (!globals.salutations.is_empty()).then(|| {
        globals
            .salutations
            .iter()
            .map(|value| value.trim().to_owned())
            .collect()
    })
}

/// Resolves the `is_excited` value from configuration sources with priority fallback.
///
/// Attempts to extract the value from `config_override` first. If that source is absent,
/// invalid, or parsing fails, falls back to the value in `file_overrides`. Returns `None`
/// only if both sources are absent or yield no value.
fn file_excited_value(
    file_overrides: Option<&FileOverrides>,
    config_override: Option<&Path>,
) -> Option<bool> {
    config_override
        .and_then(|path| {
            ortho_config::load_config_file(path)
                .ok()?
                .and_then(|fig| fig.extract_inner::<bool>("is_excited").ok())
        })
        .or_else(|| file_overrides.and_then(|file| file.is_excited))
}

fn load_config_overrides() -> Result<Option<FileOverrides>, HelloWorldError> {
    if let Some(figment) = discover_config_figment()? {
        let overrides: FileOverrides = figment.extract().map_err(|err| {
            HelloWorldError::Configuration(Arc::new(ortho_config::OrthoError::merge(err)))
        })?;
        Ok(Some(overrides))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
struct FileOverrides {
    #[serde(default)]
    is_excited: Option<bool>,
    #[serde(default)]
    cmds: CommandOverrides,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
struct CommandOverrides {
    #[serde(default)]
    greet: Option<GreetOverrides>,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
struct GreetOverrides {
    #[serde(default)]
    preamble: Option<String>,
    #[serde(default)]
    punctuation: Option<String>,
}

/// Applies greeting-specific overrides derived from configuration defaults.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when greeting defaults cannot be loaded.
pub fn apply_greet_overrides(command: &mut GreetCommand) -> Result<(), HelloWorldError> {
    if let Some(greet) = load_config_overrides()?.and_then(|overrides| overrides.cmds.greet) {
        if let Some(preamble) = greet.preamble {
            command.preamble = Some(preamble);
        }
        if let Some(punctuation) = greet.punctuation {
            command.punctuation = punctuation;
        }
    }
    Ok(())
}

pub(crate) fn load_greet_defaults() -> Result<GreetCommand, HelloWorldError> {
    let mut command = GreetCommand::default().load_and_merge()?;
    apply_greet_overrides(&mut command)?;
    Ok(command)
}

/// Top-level configuration for the hello world demo.
///
/// The struct collects the global options exposed by the example, keeping
/// fields public so the command dispatcher can inspect the resolved values
/// without extra accessor boilerplate.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "HELLO_WORLD",
    discovery(
        app_name = "hello_world",
        config_file_name = "hello_world.toml",
        dotfile_name = ".hello_world.toml",
        project_file_name = ".hello_world.toml",
        config_cli_long = "config",
        config_cli_short = 'c',
        config_cli_visible = true,
    )
)]
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
    const fn has_conflicting_modes(&self) -> bool {
        self.is_excited && self.is_quiet
    }

    /// Validates the resolved configuration before execution.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::ConflictingDeliveryModes`] when mutually
    /// exclusive delivery options are enabled.
    ///
    /// # Examples
    /// ```ignore
    /// # use hello_world::cli::{DeliveryMode, HelloWorldCli};
    /// let mut cli = HelloWorldCli::default();
    /// cli.is_excited = true;
    /// assert!(cli.validate().is_ok());
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
mod tests;
