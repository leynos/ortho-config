//! Ordered parameter prose for MAML help details.
//!
//! Only the MAML renderer consumes these paragraphs. Long help takes priority
//! over short help; CLI, default, possible-value, source, and deprecation
//! paragraphs follow in that order. XML escaping belongs to the caller.

use crate::ir::LocalizedFieldMetadata;
use crate::schema::{CliMetadata, ValueType};

/// Assemble the ordered description paragraphs shown for a parameter.
///
/// The long help text takes precedence over short help, followed by CLI,
/// default, possible-value, source and deprecation details when present.
pub(super) fn parameter_paragraphs(field: &LocalizedFieldMetadata) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let description = field.long_help.as_ref().unwrap_or(&field.help).clone();
    paragraphs.push(description);

    push_cli_paragraphs(field, &mut paragraphs);
    push_default_paragraph(field, &mut paragraphs);
    push_possible_values(field, &mut paragraphs);
    push_source_paragraphs(field, &mut paragraphs);
    push_deprecation_paragraph(field, &mut paragraphs);

    paragraphs
}

/// Append CLI spellings and repeatability information for a CLI option.
fn push_cli_paragraphs(field: &LocalizedFieldMetadata, paragraphs: &mut Vec<String>) {
    let Some(cli) = field.cli.as_ref() else {
        return;
    };
    if let Some(short) = cli.short {
        paragraphs.push(format!("Short flag: -{short}."));
    }
    if let Some(long) = cli.long.as_ref() {
        paragraphs.push(format!("Long flag: --{long}."));
    }
    if cli.value_optional {
        paragraphs.push(optional_value_paragraph(cli));
    }
    if cli.multiple {
        paragraphs.push("This option may be supplied multiple times.".to_owned());
    }
}

/// Explains the optional `=<BOOL>` value in prose.
///
/// The sentence names the flag the way a user would type it, preferring the
/// long form and falling back to the short form, because `-e=false` is the
/// spelling a user copies from the help. Metadata that carries neither flag
/// cannot name a spelling, so the sentence describes the value without quoting
/// one rather than rendering the nonsense `the flag=false`.
fn optional_value_paragraph(cli: &CliMetadata) -> String {
    let Some(spelling) = cli
        .long
        .as_ref()
        .map(|long| format!("--{long}"))
        .or_else(|| cli.short.map(|short| format!("-{short}")))
    else {
        return String::from(concat!(
            "The value is optional: the flag without a value means `true`, and ",
            "an explicit `false` clears a lower-precedence `true`."
        ));
    };
    format!(
        concat!(
            "The value is optional: `{spelling}` means `true`, and ",
            "`{spelling}=false` supplies an explicit `false`."
        ),
        spelling = spelling
    )
}

/// Append the displayed default only when metadata supplies one.
fn push_default_paragraph(field: &LocalizedFieldMetadata, paragraphs: &mut Vec<String>) {
    let Some(default) = field.default.as_ref() else {
        return;
    };
    paragraphs.push(format!("Default: {}.", default.display));
}

/// Append the sorted set of possible values when any source describes them.
fn push_possible_values(field: &LocalizedFieldMetadata, paragraphs: &mut Vec<String>) {
    let values = collect_possible_values(field);
    if values.is_empty() {
        return;
    }
    paragraphs.push(format!("Possible values: {}.", values.join(", ")));
}

/// Combine enum variants and clap possible values into a sorted, deduplicated list.
fn collect_possible_values(field: &LocalizedFieldMetadata) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(ValueType::Enum { variants }) = field.value.as_ref() {
        values.extend(variants.iter().cloned());
    }
    if let Some(cli) = field.cli.as_ref() {
        values.extend(cli.possible_values.iter().cloned());
    }
    if values.is_empty() {
        return values;
    }
    values.sort();
    values.dedup();
    values
}

/// Append the environment variable and serialized config key when available.
fn push_source_paragraphs(field: &LocalizedFieldMetadata, paragraphs: &mut Vec<String>) {
    if let Some(env) = field.env.as_ref() {
        paragraphs.push(format!("Environment variable: {}.", env.var_name));
    }
    if let Some(file) = field.file.as_ref() {
        paragraphs.push(format!("Config key: {}.", file.key_path));
    }
}

/// Append deprecation guidance without changing the field's other help details.
fn push_deprecation_paragraph(field: &LocalizedFieldMetadata, paragraphs: &mut Vec<String>) {
    let Some(deprecation) = field.deprecated.as_ref() else {
        return;
    };
    paragraphs.push(format!("Deprecated: {}.", deprecation.note));
}
