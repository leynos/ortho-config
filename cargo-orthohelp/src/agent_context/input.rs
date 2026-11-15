//! Projection of IR field metadata onto agent-facing inputs.
//!
//! The bridge reports the invocable surface of a command, so not every IR field
//! becomes an input: a field hidden from help, or one that names no flag and
//! takes no value, is configuration for another source. What remains is
//! described in the agent-context vocabulary — a lowercase value type, the
//! default rendered for display, and the closed set of permitted values — which
//! is why the mapping lives here rather than beside the tree walk.

use crate::schema::{FieldMetadata, ValueType};

use super::default_display::normalize_default_display;

/// Maps CLI-visible field metadata into an agent input.
///
/// Returns `None` for fields without CLI metadata, fields hidden from help, and
/// non-positional fields with no long or short flag. Returned inputs populate
/// name, long flag, value type, required state, default display, and enum
/// values.
pub(super) fn build_input(field: &FieldMetadata) -> Option<ortho_config::AgentInput> {
    let cli = field.cli.as_ref()?;
    if cli.hide_in_help {
        return None;
    }
    if should_skip_non_flag_input(field) {
        warn_skipped_input(field);
        return None;
    }
    Some(ortho_config::AgentInput {
        name: field.name.clone(),
        long: cli.long.clone(),
        value_type: map_input_value_type(field),
        required: field.required,
        default: field
            .default
            .as_ref()
            .map(|default| normalize_default_display(&default.display)),
        enum_values: enum_values(field),
    })
}

/// Warn that a CLI-visible field has no invocable surface, so is dropped.
///
/// Kept out of [`build_input`] so that function contains a single `tracing`
/// call at most: the `log` bridge `tracing` expands into when another crate in
/// the graph enables it is branch-heavy enough to matter to the workspace's
/// cognitive-complexity ceiling.
fn warn_skipped_input(field: &FieldMetadata) {
    tracing::warn!(
        field = field.name,
        "skipping CLI input with no long or short flag that does not take a value"
    );
}

/// Returns whether a field has CLI metadata but no invocable CLI surface.
///
/// When `FieldMetadata.cli.long` and `FieldMetadata.cli.short` are both absent
/// and `takes_value` is false, the field is intended for other configuration
/// sources rather than command-line invocation.
const fn should_skip_non_flag_input(field: &FieldMetadata) -> bool {
    let Some(cli) = field.cli.as_ref() else {
        return false;
    };
    cli.long.is_none() && cli.short.is_none() && !cli.takes_value
}

/// Reports the agent-facing value type for a CLI-visible input.
///
/// An enumerated field is reported as `"enum"` whether the variants come from
/// the value type or from clap's `possible_values`, because agents act on the
/// closed value set either way. Fields with no declared value type yield
/// `None`, which the agent-context schema renders as an absent `value_type`.
fn map_input_value_type(field: &FieldMetadata) -> Option<String> {
    // Booleans advertise `true`/`false` as possible values, which would
    // otherwise be classified as an enum. Keep the boolean classification so
    // agents see a flag rather than a choice between named variants.
    if matches!(&field.value, Some(ValueType::Bool)) {
        return Some("bool".to_owned());
    }
    if matches!(&field.value, Some(ValueType::Enum { .. })) {
        return Some("enum".to_owned());
    }
    if field
        .cli
        .as_ref()
        .is_some_and(|cli| !cli.possible_values.is_empty())
    {
        return Some("enum".to_owned());
    }
    field.value.as_ref().map(map_value_type)
}

/// Renders an IR value type as the lowercase name used on the agent-context wire.
///
/// A [`ValueType::Custom`] keeps its declared name verbatim because only the
/// consumer's derive macro knows that type; every built-in maps to a fixed
/// lowercase spelling.
fn map_value_type(value: &ValueType) -> String {
    match value {
        ValueType::String => "string".to_owned(),
        ValueType::Integer { .. } => "integer".to_owned(),
        ValueType::Float { .. } => "float".to_owned(),
        ValueType::Bool => "bool".to_owned(),
        ValueType::Duration => "duration".to_owned(),
        ValueType::Path => "path".to_owned(),
        ValueType::IpAddr => "ipaddr".to_owned(),
        ValueType::Hostname => "hostname".to_owned(),
        ValueType::Url => "url".to_owned(),
        ValueType::Enum { .. } => "enum".to_owned(),
        ValueType::List { .. } => "list".to_owned(),
        ValueType::Map { .. } => "map".to_owned(),
        ValueType::Custom { name } => name.clone(),
    }
}

/// Collects the permitted values for an enumerated input.
///
/// The declared [`ValueType::Enum`] variants win; otherwise clap's
/// `possible_values` are used, so a `value_parser`-constrained string field is
/// still reported as a closed set.
///
/// A boolean yields no values. It is not a choice between named variants:
/// `value_type` already reports `bool`, and repeating the two literals as enum
/// values would hand an agent a second, differently shaped description of the
/// same input. A field that is enumerated by neither route likewise yields an
/// empty vector.
fn enum_values(field: &FieldMetadata) -> Vec<String> {
    match &field.value {
        Some(ValueType::Enum { variants }) => variants.clone(),
        Some(ValueType::Bool) => Vec::new(),
        _ => field
            .cli
            .as_ref()
            .map(|cli| cli.possible_values.clone())
            .unwrap_or_default(),
    }
}
