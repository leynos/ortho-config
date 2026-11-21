//! `clap` error localization helpers.
//!
//! Provides a formatter that maps [`clap::ErrorKind`] variants onto Fluent
//! identifiers and forwards any captured argument context to the supplied
//! [`Localizer`]. When no translation exists the original `clap` message is
//! preserved so applications retain the stock diagnostics. The formatter is
//! intended for display purposes: it rewrites the user-facing message and
//! preserves the rendered usage tail, but it does not carry over the internal
//! `clap` context beyond what is present in the original `Display` output.

use super::clap_error_ids::{ClapErrorTranslation, translation_for};
use crate::{LocalizationArgs, Localizer};
use clap::{
    Command as ClapCommand,
    error::{ContextKind, ContextValue, Error as ClapError, ErrorKind},
};
use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error as _;
use std::sync::Arc;

/// Builds a formatter suitable for [`clap::Command::try_get_matches`] error
/// handling.
///
/// The returned closure is [`Clone`] so it can be reused when the command tree
/// is rebuilt. It applies [`localize_clap_error`] with the shared localizer,
/// preserving the original `clap` output when no translation is available.
#[must_use = "Attach this formatter when wiring clap error handling"]
pub fn clap_error_formatter(
    localizer: Arc<dyn Localizer>,
) -> impl Fn(ClapError) -> ClapError + Clone {
    move |error| localize_clap_error(error, localizer.as_ref())
}

/// Rewrites a `clap` error message using the provided localizer.
///
/// - Consults [`CLAP_ERROR_IDS`](super::CLAP_ERROR_IDS) for the kind's
///   outcome: translated kinds name their Fluent identifier, display-only
///   kinds are returned untouched, and a kind newer than the matrix derives a
///   `clap-error-<kebab-kind>` identifier.
/// - Includes relevant [`ContextKind`] values as Fluent arguments (for
///   example, `argument`, `value`, `expected`, `actual`), plus the details
///   `clap` keeps outside its context map: a value parser's rejection reason
///   as `reason`, and, for a conflict, the prior argument as `prior` and which
///   of the two conflict shapes surfaced as `conflict_kind`.
/// - Falls back to the original `clap` message when the localizer does not
///   return a translation, so a missing Fluent argument never replaces a
///   usable diagnostic with an unresolved placeholder.
/// - Intended for display-only scenarios: the returned [`ClapError`] carries
///   the translated text and original rendered tail but does not preserve the
///   internal `clap` context/command metadata.
///
/// # Observability
///
/// Missing translations emit a warn-level `tracing` event before the original
/// error is returned. The event includes stable `identifier`, `error_kind`, and
/// `locale` fields so applications can observe catalogue gaps without parsing
/// display text.
#[must_use]
pub fn localize_clap_error(error: ClapError, localizer: &dyn Localizer) -> ClapError {
    localize_clap_error_with_command(error, localizer, None)
}

/// Rewrites a `clap` error message, enriching localization arguments with
/// details from the provided [`ClapCommand`] where possible.
///
/// # Observability
///
/// If the computed `clap-error-*` identifier resolves to the fallback text, the
/// function emits a warn-level `tracing` event with `identifier`, `error_kind`,
/// and `locale` fields, then returns the original `clap` error unchanged.
#[must_use]
pub fn localize_clap_error_with_command(
    error: ClapError,
    localizer: &dyn Localizer,
    command: Option<&ClapCommand>,
) -> ClapError {
    let id = match catalogue_id(error.kind()) {
        CatalogueId::Skip => return error,
        CatalogueId::Translate(identifier) => Cow::Borrowed(identifier),
        CatalogueId::Derived(identifier) => Cow::Owned(identifier),
    };
    let args = localization_args(&error, command);
    let args_ref = (!args.is_empty()).then_some(&args);

    let rendered = error.to_string();
    let (first_line, tail) = split_rendered(&rendered);
    let fallback = strip_error_prefix(first_line).to_owned();

    let translated = localizer.message(&id, args_ref, &fallback);
    if translated == fallback {
        warn_missing_translation(&id, error.kind(), localizer);
        return error;
    }

    let message = append_tail(translated, &tail);

    let _ = command; // preserved for API symmetry; context is not retained
    ClapError::raw(error.kind(), message)
}

/// Split a rendered `clap` error into its message line and its usage tail.
///
/// The first line is the text `clap` would show after `error: `, which is what
/// gets translated; everything after it is `clap`'s rendered usage and help
/// output, which is preserved verbatim because it is not catalogue text.
fn split_rendered(rendered: &str) -> (&str, String) {
    let mut lines = rendered.lines();
    let first_line = lines.next().unwrap_or_default();
    let tail = lines.collect::<Vec<_>>().join("\n");
    (first_line, tail)
}

/// Remove the stock `error: ` prefix `clap` puts in front of its message.
fn strip_error_prefix(line: &str) -> &str {
    line.strip_prefix("error: ").unwrap_or(line)
}

/// Warn about a catalogue gap before returning the untranslated error.
///
/// Extracted so the emission is a single `tracing` call: under feature-unified
/// `tracing/log` the macro expands into a branch cluster, and keeping that
/// cluster out of the caller preserves its readability.
fn warn_missing_translation(id: &str, kind: ErrorKind, localizer: &dyn Localizer) {
    let locale = localizer
        .locale()
        .map_or_else(|| "unknown".to_owned(), std::string::ToString::to_string);
    tracing::warn!(
        identifier = %id,
        error_kind = ?kind,
        locale = %locale,
        "missing translation for clap error"
    );
}

/// Reattach `clap`'s rendered usage tail to the translated message.
fn append_tail(translated: String, tail: &str) -> String {
    if tail.is_empty() {
        translated
    } else {
        format!("{translated}\n{tail}")
    }
}

/// The catalogue decision for one clap error kind.
enum CatalogueId {
    /// The matrix names an embedded identifier to look up.
    Translate(&'static str),
    /// clap renders this surface itself, so the error is returned untouched.
    Skip,
    /// The matrix predates the kind, so derive the legacy identifier shape.
    Derived(String),
}

/// Consults the published matrix before deriving an identifier.
///
/// A kind newer than the matrix keeps the pre-matrix derived identifier, so an
/// unmapped kind still tries a translation instead of silently passing through.
fn catalogue_id(kind: ErrorKind) -> CatalogueId {
    match translation_for(kind) {
        Some(ClapErrorTranslation::Translated(identifier)) => CatalogueId::Translate(identifier),
        Some(ClapErrorTranslation::DisplayOnly) => CatalogueId::Skip,
        None => CatalogueId::Derived(format!("clap-error-{}", to_kebab_case(kind))),
    }
}

/// Copies supported `clap` context into the placeholder keys used by error catalogues.
///
/// If `clap` does not supply valid subcommands, command metadata supplies that list
/// when available; an empty list is omitted so a translation can use its own fallback.
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
    // Only a conflict carries a prior argument, so the key is absent for every
    // other kind rather than empty.
    insert_context(&mut args, "prior", error.get(ContextKind::PriorArg));
    insert_value_parser_reason(&mut args, error);
    insert_conflict_kind(&mut args, error);

    // let-chains require edition 2024 (stabilised in Rust 1.88).
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

/// Adds the rejection reason a value parser supplied for a rejected value.
///
/// clap keeps this reason on the error itself rather than in its context map,
/// and renders it after the generic "invalid value" text. Without it, the
/// translation would replace a diagnostic that names the actual problem with
/// one that only says a parser declined the value.
fn insert_value_parser_reason(args: &mut LocalizationArgs<'static>, error: &ClapError) {
    let Some(reason) = error.source() else {
        return;
    };
    let text = reason.to_string();
    if !text.is_empty() {
        args.insert("reason", text.into());
    }
}

/// Names which conflict surfaced, so the message can describe it correctly.
///
/// clap reports both a repeated argument (`--debug --debug`) and a collision
/// between distinct arguments as `ArgumentConflict`, and distinguishes them
/// only by whether the offending argument is also the prior one. A message
/// that always blamed "another argument" would misdescribe the repeated case,
/// where there is no other argument involved.
fn insert_conflict_kind(args: &mut LocalizationArgs<'static>, error: &ClapError) {
    if error.kind() != ErrorKind::ArgumentConflict {
        return;
    }
    let offending = error.get(ContextKind::InvalidArg);
    let is_repeated = offending.is_some() && offending == error.get(ContextKind::PriorArg);
    args.insert(
        "conflict_kind",
        if is_repeated { "repeated" } else { "conflict" }.into(),
    );
}

/// Adds one non-empty context value, leaving absent and unsupported values out of the map.
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

/// Converts displayable `clap` context to interpolation text and logs unknown value kinds.
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
        _ => {
            tracing::debug!(?context, "unhandled clap context value");
            String::new()
        }
    }
}

/// Converts `clap`'s `PascalCase` error-kind debug name into a lower-case
/// catalogue suffix.
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
