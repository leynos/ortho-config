//! CLI configuration for the `hello_world` example.
//!
//! Binds CLI, environment, and default layers via `OrthoConfig` so tests can
//! drive the binary with predictable inputs.
use std::collections::BTreeMap;
use std::mem;
use std::path::{Path, PathBuf};

use clap::{ArgAction, Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use fluent_bundle::FluentValue;
use ortho_config::{LocalizationArgs, Localizer, OrthoConfig, OrthoMergeExt, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};

use crate::error::{HelloWorldError, ValidationError};
use crate::localizer::{
    CLI_ABOUT_MESSAGE_ID, CLI_BASE_MESSAGE_ID, CLI_LONG_ABOUT_MESSAGE_ID, CLI_USAGE_MESSAGE_ID,
};

mod commands;
mod config_loading;
mod discovery;
mod overrides;

use self::config_loading::FileLayer;
#[cfg(test)]
pub(crate) use self::config_loading::{
    build_cli_args, build_overrides, file_excited_value, load_config_overrides, trimmed_salutations,
};
pub use commands::{FarewellChannel, GreetCommand, TakeLeaveCommand};
#[cfg(test)]
pub(crate) use overrides::{CommandOverrides, FileOverrides, GreetOverrides, Overrides};

/// Command-line surface exposed by the example.
#[derive(Debug, Parser)]
#[command(
    name = "hello-world",
    bin_name = "hello-world",
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

impl CommandLine {
    /// Parses command-line arguments using the supplied localiser.
    ///
    /// # Errors
    ///
    /// Returns a [`clap::Error`] when parsing fails.
    pub fn try_parse_localized_env(localizer: &dyn Localizer) -> Result<Self, clap::Error> {
        Self::try_parse_localized(std::env::args_os(), localizer)
    }

    /// Parses the provided iterator of arguments using the supplied localiser.
    ///
    /// # Errors
    ///
    /// Returns a [`clap::Error`] when parsing fails.
    pub fn try_parse_localized<I, T>(
        iter: I,
        localizer: &dyn Localizer,
    ) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let mut command = Self::command().localize(localizer);
        let mut matches = command.try_get_matches_from_mut(iter)?;
        Self::from_arg_matches_mut(&mut matches).map_err(|err| err.with_cmd(&command))
    }
}

/// Extension trait that applies localisation to a `clap::Command` tree.
pub trait LocalizeCmd {
    /// Rewrites the command metadata (about, help, usage, etc.) using the provided localiser.
    #[must_use]
    fn localize(self, localizer: &dyn Localizer) -> Self;
}

impl LocalizeCmd for clap::Command {
    fn localize(mut self, localizer: &dyn Localizer) -> Self {
        let mut path = Vec::new();
        localize_command_tree(&mut self, localizer, &mut path);
        self
    }
}

fn localize_command_tree(
    command: &mut clap::Command,
    localizer: &dyn Localizer,
    path: &mut Vec<String>,
) {
    apply_command_metadata(command, localizer, path);
    for subcommand in command.get_subcommands_mut() {
        path.push(subcommand.get_name().to_owned());
        localize_command_tree(subcommand, localizer, path);
        path.pop();
    }
}

fn apply_command_metadata(command: &mut clap::Command, localizer: &dyn Localizer, path: &[String]) {
    let args = localization_args_for(command);
    let about_id = message_id(path, "about");
    if let Some(about) = localizer.lookup(&about_id, None) {
        *command = mem::take(command).about(about);
    }
    let long_about_id = message_id(path, "long_about");
    if let Some(long_about) = localizer.lookup(&long_about_id, Some(&args)) {
        *command = mem::take(command).long_about(long_about);
    }
    let usage_id = message_id(path, "usage");
    if let Some(usage) = localizer.lookup(&usage_id, Some(&args)) {
        *command = mem::take(command).override_usage(usage);
    }
}

fn localization_args_for(command: &clap::Command) -> LocalizationArgs<'static> {
    let mut args = LocalizationArgs::default();
    args.insert("binary", FluentValue::from(command.get_name().to_owned()));
    args
}

fn message_id(path: &[String], suffix: &str) -> String {
    if path.is_empty() {
        match suffix {
            "about" => CLI_ABOUT_MESSAGE_ID.to_owned(),
            "long_about" => CLI_LONG_ABOUT_MESSAGE_ID.to_owned(),
            "usage" => CLI_USAGE_MESSAGE_ID.to_owned(),
            other => format!("{CLI_BASE_MESSAGE_ID}.{other}"),
        }
    } else {
        format!("{CLI_BASE_MESSAGE_ID}.{}.{}", path.join("."), suffix)
    }
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
    /// Template overrides loaded from configuration files.
    #[arg(skip = BTreeMap::new())]
    #[serde(default)]
    #[ortho_config(skip_cli, merge_strategy = "replace")]
    pub greeting_templates: BTreeMap<String, String>,
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
    let base =
        HelloWorldCli::load_from_iter(config_loading::build_cli_args(config_override).into_iter())?;
    let salutations = config_loading::trimmed_salutations(globals);
    let file_overrides = config_loading::load_config_overrides()?;
    let overrides = config_loading::build_overrides(
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

/// Applies greeting-specific overrides derived from configuration defaults.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when greeting defaults cannot be loaded.
pub fn apply_greet_overrides(command: &mut GreetCommand) -> Result<(), HelloWorldError> {
    if let Some(greet) =
        config_loading::load_config_overrides()?.and_then(|overrides| overrides.cmds.greet)
    {
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
pub(crate) mod tests;
