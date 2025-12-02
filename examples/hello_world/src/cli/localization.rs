//! Localisation helpers applied to the hello world CLI command tree.
//!
//! Mirrors the derive-level localisation hooks so tests can exercise the
//! public CLI surface with custom `Localizer` implementations.

use fluent_bundle::FluentValue;
use ortho_config::{LocalizationArgs, Localizer, localize_clap_error_with_command};

use crate::localizer::{
    CLI_ABOUT_MESSAGE_ID, CLI_BASE_MESSAGE_ID, CLI_LONG_ABOUT_MESSAGE_ID, CLI_USAGE_MESSAGE_ID,
};

/// Extension trait that applies localisation to a `clap::Command` tree.
pub trait LocalizeCmd {
    /// Rewrites the command metadata (about, help, usage, etc.) using the provided localizer.
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

pub fn localize_parse_error(
    err: clap::Error,
    localizer: &dyn Localizer,
    command: &clap::Command,
) -> clap::Error {
    localize_clap_error_with_command(err, localizer, Some(command))
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
    let mut working = command.clone();
    let about_id = message_id(path, "about");
    if let Some(about) = localizer.lookup(&about_id, None) {
        working = working.about(about);
    }
    let long_about_id = message_id(path, "long_about");
    if let Some(long_about) = localizer.lookup(&long_about_id, Some(&args)) {
        working = working.long_about(long_about);
    }
    let usage_id = message_id(path, "usage");
    if let Some(usage) = localizer.lookup(&usage_id, Some(&args)) {
        working = working.override_usage(usage);
    }
    *command = working;
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
