//! `clap` error localisation helpers.
//!
//! Provides a formatter that maps [`clap::ErrorKind`] variants onto Fluent
//! identifiers and forwards any captured argument context to the supplied
//! [`Localizer`]. When no translation exists the original `clap` message is
//! preserved so applications retain the stock diagnostics. The formatter is
//! intended for display purposes: it rewrites the user-facing message and
//! preserves the rendered usage tail, but it does not carry over the internal
//! `clap` context beyond what is present in the original `Display` output.

use crate::{LocalizationArgs, Localizer};
use clap::{
    Command as ClapCommand,
    error::{ContextKind, ContextValue, Error as ClapError, ErrorKind},
};
use std::collections::HashMap;
use std::sync::Arc;

/// Builds a formatter suitable for [`clap::Command::try_get_matches`] error
/// handling.
///
/// The returned closure is [`Clone`] so it can be reused when the command tree
/// is rebuilt. It applies [`localize_clap_error`] with the shared localiser,
/// preserving the original `clap` output when no translation is available.
#[must_use = "Attach this formatter when wiring clap error handling"]
pub fn clap_error_formatter(
    localizer: Arc<dyn Localizer>,
) -> impl Fn(ClapError) -> ClapError + Clone {
    move |error| localize_clap_error(error, localizer.as_ref())
}

/// Rewrites a `clap` error message using the provided localiser.
///
/// - Maps the [`ErrorKind`] to a Fluent identifier of the form
///   `clap-error-<kebab-kind>`.
/// - Includes relevant [`ContextKind`] values as Fluent arguments (for
///   example, `argument`, `value`, `expected`, `actual`).
/// - Falls back to the original `clap` message when the localiser does not
///   return a translation.
#[must_use]
pub fn localize_clap_error(error: ClapError, localizer: &dyn Localizer) -> ClapError {
    localize_clap_error_with_command(error, localizer, None)
}

/// Rewrites a `clap` error message, enriching localisation arguments with
/// details from the provided [`ClapCommand`] where possible.
#[must_use]
pub fn localize_clap_error_with_command(
    error: ClapError,
    localizer: &dyn Localizer,
    command: Option<&ClapCommand>,
) -> ClapError {
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        return error;
    }

    let id = message_id(error.kind());
    let args = localization_args(&error, command);
    let args_ref = (!args.is_empty()).then_some(&args);

    let rendered = error.to_string();
    let mut lines = rendered.lines();
    let first_line = lines.next().unwrap_or_default();
    let tail = lines.collect::<Vec<_>>().join("\n");

    let fallback = first_line
        .strip_prefix("error: ")
        .unwrap_or(first_line)
        .to_owned();

    let localised = localizer.message(&id, args_ref, &fallback);
    if localised == fallback {
        return error;
    }

    let message = if tail.is_empty() {
        localised
    } else {
        format!("{localised}\n{tail}")
    };

    let _ = command; // preserved for API symmetry; context is not retained
    ClapError::raw(error.kind(), message)
}

fn message_id(kind: ErrorKind) -> String {
    match kind {
        ErrorKind::MissingRequiredArgument => "clap-error-missing-argument".to_owned(),
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand | ErrorKind::MissingSubcommand => {
            "clap-error-missing-subcommand".to_owned()
        }
        ErrorKind::UnknownArgument => "clap-error-unknown-argument".to_owned(),
        _ => format!("clap-error-{}", to_kebab_case(kind)),
    }
}

fn localization_args(
    error: &ClapError,
    command: Option<&ClapCommand>,
) -> LocalizationArgs<'static> {
    let mut args: LocalizationArgs<'static> = HashMap::new();

    insert_context(&mut args, "argument", error.get(ContextKind::InvalidArg));
    insert_context(&mut args, "value", error.get(ContextKind::InvalidValue));
    insert_context(
        &mut args,
        "valid_values",
        error.get(ContextKind::ValidValue),
    );
    insert_context(
        &mut args,
        "expected",
        error.get(ContextKind::ExpectedNumValues),
    );
    insert_context(&mut args, "actual", error.get(ContextKind::ActualNumValues));
    insert_context(&mut args, "min", error.get(ContextKind::MinValues));
    insert_context(
        &mut args,
        "subcommand",
        error.get(ContextKind::InvalidSubcommand),
    );
    insert_context(
        &mut args,
        "valid_subcommands",
        error.get(ContextKind::ValidSubcommand),
    );

    if !args.contains_key("valid_subcommands")
        && let Some(cmd) = command
    {
        let names: Vec<String> = cmd
            .get_subcommands()
            .map(clap::Command::get_name)
            .map(str::to_owned)
            .collect();
        if !names.is_empty() {
            args.insert("valid_subcommands", names.join(", ").into());
        }
    }

    args
}

fn insert_context(
    args: &mut LocalizationArgs<'static>,
    key: &'static str,
    context_value: Option<&ContextValue>,
) {
    let Some(actual_value) = context_value else {
        return;
    };
    let text = stringify_context(actual_value);
    if text.is_empty() {
        return;
    }
    args.insert(key, text.into());
}

fn stringify_context(context: &ContextValue) -> String {
    match context {
        ContextValue::Bool(flag) => flag.to_string(),
        ContextValue::String(text) => text.clone(),
        ContextValue::Strings(values) => values.join(", "),
        ContextValue::StyledStr(styled) => styled.to_string(),
        ContextValue::StyledStrs(values) => values
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        ContextValue::Number(count) => count.to_string(),
        _ => String::new(),
    }
}

fn to_kebab_case(kind: ErrorKind) -> String {
    let debug = format!("{kind:?}");
    let mut kebab = String::with_capacity(debug.len());

    for (idx, ch) in debug.chars().enumerate() {
        if ch.is_uppercase() {
            if idx > 0 {
                kebab.push('-');
            }
            kebab.push(ch.to_ascii_lowercase());
        } else {
            kebab.push(ch);
        }
    }

    kebab
}
