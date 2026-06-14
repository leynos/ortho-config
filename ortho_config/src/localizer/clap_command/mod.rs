//! Extension traits for localizing `clap::Command` trees.

use super::{LocalizationArgs, Localizer, message_id_for};
use clap::{Arg, ArgAction, Command};
use fluent_bundle::FluentValue;
use std::collections::{HashMap, HashSet};

mod parse;

pub use parse::{LocalizedParse, parse_localized_command};

/// Extension trait that applies a [`Localizer`] to a [`clap::Command`] tree.
pub trait LocalizeCmd: Sized {
    /// Applies the localizer to this command and every subcommand recursively.
    ///
    /// # Panics
    ///
    /// Panics if duplicate subcommand or argument identifiers are present.
    #[must_use]
    fn localize(self, localizer: &dyn Localizer) -> Self;

    /// Applies the localizer to this command without recursing into subcommands.
    ///
    /// # Panics
    ///
    /// Panics if duplicate argument identifiers are present.
    #[must_use]
    fn localize_self(self, localizer: &dyn Localizer) -> Self;

    /// Overrides the root identifier segment used for this command tree.
    #[must_use = "with_base returns a wrapper that must be localized to apply the override"]
    fn with_base(self, base: impl Into<String>) -> WithBase<Self>;
}

impl LocalizeCmd for Command {
    fn localize(self, localizer: &dyn Localizer) -> Self {
        let path = default_base_for(&self);
        localize_command(self, localizer, &path, true)
    }

    fn localize_self(self, localizer: &dyn Localizer) -> Self {
        let path = default_base_for(&self);
        localize_command(self, localizer, &path, false)
    }

    fn with_base(self, base: impl Into<String>) -> WithBase<Self> {
        WithBase {
            command: self,
            base: split_base(base),
        }
    }
}

/// Carrier returned by [`LocalizeCmd::with_base`].
#[must_use]
pub struct WithBase<C> {
    command: C,
    base: Vec<String>,
}

impl WithBase<Command> {
    /// Applies the localizer to this command and every subcommand recursively.
    ///
    /// # Panics
    ///
    /// Panics if duplicate subcommand or argument identifiers are present.
    #[must_use]
    pub fn localize(self, localizer: &dyn Localizer) -> Command {
        localize_command(self.command, localizer, &self.base, true)
    }

    /// Applies the localizer to this command without recursing into subcommands.
    ///
    /// # Panics
    ///
    /// Panics if duplicate argument identifiers are present.
    #[must_use]
    pub fn localize_self(self, localizer: &dyn Localizer) -> Command {
        localize_command(self.command, localizer, &self.base, false)
    }

    /// Replaces the base used for identifier derivation.
    #[must_use = "with_base returns an updated wrapper and does not mutate in place"]
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = split_base(base);
        self
    }
}

fn split_base(base: impl Into<String>) -> Vec<String> {
    base.into().split('.').map(str::to_owned).collect()
}

fn default_base_for(command: &Command) -> Vec<String> {
    vec![
        command
            .get_bin_name()
            .unwrap_or_else(|| command.get_name())
            .to_owned(),
    ]
}

fn localize_command(
    mut command: Command,
    localizer: &dyn Localizer,
    path: &[String],
    should_recurse: bool,
) -> Command {
    command = apply_command_metadata(command, localizer, path);
    command = apply_arg_metadata(command, localizer, path);

    if should_recurse {
        assert_unique_subcommand_ids(&command, path);
        let child_names = command
            .get_subcommands()
            .map(|child| child.get_name().to_owned())
            .collect::<Vec<_>>();

        for child_name in child_names {
            let child_path = child_path(path, &child_name);
            if let Some(child_slot) = command.find_subcommand_mut(&child_name) {
                // The setter chain needs ownership; take avoids aliasing and cloning.
                let child = std::mem::take(child_slot);
                *child_slot = localize_command(child, localizer, &child_path, true);
            }
        }
    }

    command
}

fn child_path(path: &[String], child_name: &str) -> Vec<String> {
    let mut child_path = Vec::with_capacity(path.len() + 1);
    child_path.extend_from_slice(path);
    child_path.push(child_name.to_owned());
    child_path
}

fn apply_command_metadata(
    mut command: Command,
    localizer: &dyn Localizer,
    path: &[String],
) -> Command {
    let args = localization_args_for(&command);

    // `about` is a brief tagline; longer fields pass args for `{binary}` interpolation.
    if let Some(value) = localizer.lookup(&message_id_for(path, "about"), None) {
        command = command.about(value);
    }
    if let Some(value) = localizer.lookup(&message_id_for(path, "long_about"), Some(&args)) {
        command = command.long_about(value);
    }
    if let Some(value) = localizer.lookup(&message_id_for(path, "usage"), Some(&args)) {
        command = command.override_usage(value);
    }
    if let Some(value) = localizer.lookup(&message_id_for(path, "version"), Some(&args)) {
        command = command.version(value);
    }
    if let Some(value) = localizer.lookup(&message_id_for(path, "long_version"), Some(&args)) {
        command = command.long_version(value);
    }
    if let Some(value) = localizer.lookup(&message_id_for(path, "after_help"), Some(&args)) {
        command = command.after_help(value);
    }
    if let Some(value) = localizer.lookup(&message_id_for(path, "after_long_help"), Some(&args)) {
        command = command.after_long_help(value);
    }

    command
}

fn localization_args_for(command: &Command) -> LocalizationArgs<'static> {
    let mut args = HashMap::new();
    args.insert("binary", FluentValue::from(command.get_name().to_owned()));
    args
}

fn apply_arg_metadata(mut command: Command, localizer: &dyn Localizer, path: &[String]) -> Command {
    assert_unique_arg_ids(&command, path);
    let arg_metadata = command
        .get_arguments()
        .map(|arg| (arg.get_id().to_string(), arg_takes_value(arg)))
        .collect::<Vec<_>>();

    for (arg_id, takes_value) in arg_metadata {
        let context = ArgLocalizationContext {
            localizer,
            path,
            arg_id: &arg_id,
            takes_value,
        };
        command = command.mut_arg(&arg_id, |arg| localize_arg(arg, &context));
    }

    command
}

fn arg_takes_value(arg: &Arg) -> bool {
    matches!(arg.get_action(), ArgAction::Set | ArgAction::Append)
}

struct ArgLocalizationContext<'context> {
    localizer: &'context dyn Localizer,
    path: &'context [String],
    arg_id: &'context str,
    takes_value: bool,
}

fn localize_arg(mut arg: Arg, context: &ArgLocalizationContext<'_>) -> Arg {
    let help_id = message_id_for(context.path, &format!("args.{}.help", context.arg_id));
    if let Some(value) = context.localizer.lookup(&help_id, None) {
        arg = arg.help(value);
    }

    let long_help_id = message_id_for(context.path, &format!("args.{}.long_help", context.arg_id));
    if let Some(value) = context.localizer.lookup(&long_help_id, None) {
        arg = arg.long_help(value);
    }

    let value_name_id =
        message_id_for(context.path, &format!("args.{}.value_name", context.arg_id));
    if context.takes_value
        && let Some(value) = context.localizer.lookup(&value_name_id, None)
    {
        arg = arg.value_name(value);
    }

    arg
}

fn assert_unique_subcommand_ids(command: &Command, path: &[String]) {
    let mut ids = HashSet::new();
    for child in command.get_subcommands() {
        let child_path = child_path(path, child.get_name());
        let id = message_id_for(&child_path, "about");
        assert!(
            ids.insert(id.clone()),
            "localized command identifier collision for subcommand {:?}: {id}",
            child.get_name()
        );
    }
}

fn assert_unique_arg_ids(command: &Command, path: &[String]) {
    let mut ids = HashSet::new();
    for arg in command.get_arguments() {
        let id = message_id_for(path, &format!("args.{}.help", arg.get_id()));
        assert!(
            ids.insert(id.clone()),
            "localized argument identifier collision for argument {:?}: {id}",
            arg.get_id()
        );
    }
}

#[cfg(test)]
mod tests;
