//! CLI configuration for the `hello_world` example.
//!
//! Binds CLI, environment, and default layers via `OrthoConfig` so tests can
//! drive the binary with predictable inputs.
use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{ArgAction, Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::{Localizer, OrthoConfig};
use serde::{Deserialize, Serialize};

use crate::error::ValidationError;
mod commands;
mod config_loading;
mod discovery;
mod global_config;
mod localization;
mod overrides;

use self::localization::localize_parse_error;

#[cfg(test)]
pub(crate) use self::config_loading::load_config_overrides;
pub use commands::{FarewellChannel, GreetCommand, TakeLeaveCommand};
pub use global_config::{apply_greet_overrides, load_global_config, load_greet_defaults};
/// Extension trait for applying localisation to a [`clap::Command`] tree.
///
/// Re-exported to allow consumers to localise CLI metadata (about, help, usage)
/// using a [`Localizer`] implementation.
pub use localization::LocalizeCmd;
#[cfg(test)]
pub(crate) use overrides::{CommandOverrides, FileOverrides, GreetOverrides};

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

/// Result of parsing command-line arguments, including the raw matches.
#[derive(Debug)]
pub struct ParsedCommandLine {
    /// The parsed command-line structure.
    pub cli: CommandLine,
    /// The raw argument matches for subcommand CLI extraction.
    pub matches: clap::ArgMatches,
}

impl CommandLine {
    /// Parses command-line arguments using the supplied localizer.
    ///
    /// # Errors
    ///
    /// Returns a [`clap::Error`] when parsing fails.
    pub fn try_parse_localized_env(localizer: &dyn Localizer) -> Result<Self, clap::Error> {
        Self::try_parse_localized(std::env::args_os(), localizer).map(|parsed| parsed.cli)
    }

    /// Parses command-line arguments, returning both the struct and matches.
    ///
    /// This variant is useful when you need access to `ArgMatches` for
    /// features like `cli_default_as_absent`.
    ///
    /// # Errors
    ///
    /// Returns a [`clap::Error`] when parsing fails.
    pub fn try_parse_localized_with_matches_env(
        localizer: &dyn Localizer,
    ) -> Result<ParsedCommandLine, clap::Error> {
        Self::try_parse_localized(std::env::args_os(), localizer)
    }

    /// Parses the provided iterator of arguments using the supplied localizer.
    ///
    /// Returns both the parsed struct and the raw `ArgMatches` for use with
    /// `load_and_merge_with_matches`.
    ///
    /// # Errors
    ///
    /// Returns a [`clap::Error`] when parsing fails. Errors are localized via
    /// [`ortho_config::localize_clap_error_with_command`], falling back to the
    /// stock `clap` message when a translation is unavailable.
    pub fn try_parse_localized<I, T>(
        iter: I,
        localizer: &dyn Localizer,
    ) -> Result<ParsedCommandLine, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let mut command = Self::command().localize(localizer);
        let matches = command
            .try_get_matches_from_mut(iter)
            .map_err(|err| localize_parse_error(err, localizer, &command))?;
        let cli = Self::from_arg_matches(&matches).map_err(|parse_err| {
            let err_with_command = parse_err.with_cmd(&command);
            localize_parse_error(err_with_command, localizer, &command)
        })?;
        Ok(ParsedCommandLine { cli, matches })
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

impl GlobalArgs {
    /// Strips incidental whitespace from salutations for consistent output.
    ///
    /// # Examples
    /// ```ignore
    /// # use hello_world::cli::GlobalArgs;
    /// let mut globals = GlobalArgs::default();
    /// globals.salutations = vec!["  Hello".into(), "world  ".into()];
    /// assert_eq!(globals.trimmed_salutations(), vec!["Hello", "world"]);
    /// ```
    #[must_use]
    pub fn trimmed_salutations(&self) -> Vec<String> {
        self.salutations
            .iter()
            .map(|value| value.trim().to_owned())
            .collect()
    }
}

/// Subcommands implemented by the example.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand, ortho_config_macros::SelectedSubcommandMerge)]
pub enum Commands {
    /// Prints a greeting using the configured style.
    #[command(name = "greet")]
    #[ortho_subcommand(with_matches)]
    Greet(GreetCommand),
    /// Says goodbye while describing how the farewell will be delivered.
    #[command(name = "take-leave")]
    TakeLeave(TakeLeaveCommand),
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
