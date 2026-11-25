//! CLI flag and option formatting for roff output.
//!
//! Flag and option spellings are assembled from package metadata: the long
//! and short names come from the command's declaration rather than from a
//! person typing at a terminal, so each name is encoded before it is
//! interpolated. [`format_flag`] and its siblings build the bold spellings,
//! and [`format_option`] chooses between the switch, required-value and
//! optional-value forms.
//!
//! Names are encoded with [`escape_quoted_arg`]: a name sits inside a bold
//! span on a line that already carries the literal `\-` prefix, so it never
//! starts a line, and keeping the name on one line is all that is needed.

use std::borrow::Cow;

use crate::schema::{CliMetadata, ValueType};

use super::escape_quoted_arg;

/// Formats a CLI option, choosing the optional-value form for boolean flags.
///
/// Boolean options render as `--flag[=BOOL]`, with the bracketed placeholder
/// italicized; everything else renders as an ordinary flag with a required
/// value, or as a bare switch when [`CliMetadata::takes_value`] is false.
///
/// `fallback_placeholder` supplies a value name when the metadata does not
/// carry one, derived from the field's semantic [`ValueType`]. An empty
/// placeholder counts as absent: [`value_type_placeholder`] returns `""` for
/// [`ValueType::Bool`], and every roff call site passes that result in here, so
/// without the filter a boolean would render the malformed `--flag[=]` instead
/// of a readable `--flag[=VALUE]`.
#[must_use]
pub fn format_option(
    cli: &CliMetadata,
    fallback_placeholder: Option<&str>,
    fallback: &str,
) -> String {
    if !cli.takes_value {
        return format_flag(cli.long.as_deref(), cli.short);
    }
    let value_name = cli
        .value_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .or_else(|| fallback_placeholder.filter(|name| !name.is_empty()))
        .unwrap_or(fallback);
    if cli.value_optional {
        format_flag_with_optional_value(cli.long.as_deref(), cli.short, value_name)
    } else {
        format_flag_with_value(cli.long.as_deref(), cli.short, value_name)
    }
}

/// Formats a CLI flag with proper roff markup.
///
/// Returns bold formatted flags, combining long and short forms when both
/// are present. Flag names come from package metadata rather than from a
/// person typing at a terminal, so each one is passed through
/// [`escape_quoted_arg`] and cannot break the line it sits on.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::format_flag;
///
/// assert_eq!(
///     format_flag(Some("verbose"), Some('v')),
///     "\\fB\\-\\-verbose\\fR, \\fB\\-v\\fR",
///     "both flag spellings should be rendered"
/// );
/// assert_eq!(
///     format_flag(Some("help"), None),
///     "\\fB\\-\\-help\\fR",
///     "a long flag alone should be rendered"
/// );
/// assert_eq!(
///     format_flag(None, Some('h')),
///     "\\fB\\-h\\fR",
///     "a short flag alone should be rendered"
/// );
/// ```
#[must_use]
pub fn format_flag(long: Option<&str>, short: Option<char>) -> String {
    format_flag_with_suffix(long, short, "")
}

/// Renders one bold flag spelling from a literal prefix and an escaped name.
///
/// A flag name sits inside a bold span on a line that already carries the
/// literal `\-` prefix, so it never starts a line. [`escape_quoted_arg`] is
/// the right encoder: it keeps the name on one line and escapes the
/// metacharacters without pretending the name is a macro argument.
fn format_flag_spelling(prefix: &str, name: &str, suffix: &str) -> String {
    format!("\\fB{prefix}{}\\fR{suffix}", escape_quoted_arg(name))
}

/// Appends a preformatted `suffix` to each present flag.
///
/// `suffix` carries whatever separates a flag from the text that follows it, so
/// [`format_flag_with_value`] passes a leading space while
/// [`format_flag_with_optional_value`] passes none. When neither flag is
/// present there is no flag for a separator to follow, so a leading space is
/// dropped and the suffix alone is returned: the bare placeholder both public
/// functions have always produced for that combination.
fn format_flag_with_suffix(long: Option<&str>, short: Option<char>, suffix: &str) -> String {
    let mut spellings: Vec<String> = Vec::with_capacity(2);
    if let Some(long_name) = long {
        spellings.push(format_flag_spelling("\\-\\-", long_name, suffix));
    }
    if let Some(short_name) = short {
        let short_text = short_name.to_string();
        spellings.push(format_flag_spelling("\\-", &short_text, suffix));
    }

    if spellings.is_empty() {
        return suffix.strip_prefix(' ').unwrap_or(suffix).to_owned();
    }
    spellings.join(", ")
}

/// Formats a CLI flag with a value placeholder.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::format_flag_with_value;
///
/// assert_eq!(
///     format_flag_with_value(Some("port"), Some('p'), "NUM"),
///     "\\fB\\-\\-port\\fR \\fINUM\\fR, \\fB\\-p\\fR \\fINUM\\fR",
///     "the placeholder should follow each flag after a space"
/// );
/// ```
#[must_use]
pub fn format_flag_with_value(long: Option<&str>, short: Option<char>, value_name: &str) -> String {
    let value = super::italic(value_name);
    format_flag_with_suffix(long, short, &format!(" {value}"))
}

/// Formats a CLI flag whose value is optional, as in `--flag[=BOOL]`.
///
/// Boolean options accept a value but do not require one: the bare spelling
/// means `true`, and `--flag=false` supplies an explicit `false`. Bracketing
/// the placeholder distinguishes the optional value from the required form
/// produced by [`format_flag_with_value`].
///
/// The placeholder is joined directly to the flag, with no intervening space,
/// because the value must follow an `=`. Rendering `--flag [=BOOL]` would
/// document the invalid `--flag =BOOL` spelling. This matches the suffix that
/// `clap` itself emits for an argument with `require_equals` set.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::format_flag_with_optional_value;
///
/// assert_eq!(
///     format_flag_with_optional_value(Some("is-excited"), Some('i'), "BOOL"),
///     "\\fB\\-\\-is-excited\\fR\\fI[=BOOL]\\fR, \\fB\\-i\\fR\\fI[=BOOL]\\fR",
///     "the optional placeholder should join each flag directly"
/// );
/// ```
#[must_use]
pub fn format_flag_with_optional_value(
    long: Option<&str>,
    short: Option<char>,
    value_name: &str,
) -> String {
    let value = super::italic(&format!("[={value_name}]"));
    format_flag_with_suffix(long, short, &value)
}

/// Returns a human-readable placeholder for a `ValueType`.
///
/// For `Custom` types, returns the uppercased type name.
///
/// # Examples
///
/// ```
/// use cargo_orthohelp::roff::escape::value_type_placeholder;
/// use cargo_orthohelp::schema::ValueType;
///
/// assert_eq!(
///     value_type_placeholder(&ValueType::String),
///     "STRING",
///     "a string placeholder should be named STRING"
/// );
/// assert_eq!(
///     value_type_placeholder(&ValueType::Path),
///     "PATH",
///     "a path placeholder should be named PATH"
/// );
/// assert_eq!(
///     value_type_placeholder(&ValueType::Custom { name: "MyType".to_owned() }),
///     "MYTYPE",
///     "a custom placeholder should be the uppercased type name"
/// );
/// ```
#[must_use]
pub fn value_type_placeholder(value_type: &ValueType) -> Cow<'static, str> {
    match value_type {
        ValueType::String => Cow::Borrowed("STRING"),
        ValueType::Integer { .. } => Cow::Borrowed("INT"),
        ValueType::Float { .. } => Cow::Borrowed("FLOAT"),
        ValueType::Bool => Cow::Borrowed(""),
        ValueType::Duration => Cow::Borrowed("DURATION"),
        ValueType::Path => Cow::Borrowed("PATH"),
        ValueType::IpAddr => Cow::Borrowed("IP"),
        ValueType::Hostname => Cow::Borrowed("HOST"),
        ValueType::Url => Cow::Borrowed("URL"),
        ValueType::Enum { .. } => Cow::Borrowed("CHOICE"),
        ValueType::List { .. } => Cow::Borrowed("LIST"),
        ValueType::Map { .. } => Cow::Borrowed("MAP"),
        ValueType::Custom { name } => Cow::Owned(name.to_uppercase()),
    }
}

#[cfg(test)]
mod tests;
