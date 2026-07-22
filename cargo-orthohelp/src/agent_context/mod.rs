//! Transform bridge documentation IR into compact agent-context metadata.
//!
//! This module owns the `cargo-orthohelp` adapter from the documentation
//! oriented bridge IR to the reusable `ortho_config::agent_context` schema.

use ortho_config::{
    AgentCommand, AgentContext, AgentInput, InteractionMode, Localizer, MutationEffect,
};

use crate::schema::{DocMetadata, FieldMetadata, ValueType};

const CANONICAL_VERBS: &[&str] = &[
    "get", "list", "create", "update", "delete", "jobs", "profile", "feedback",
];

/// Builds an agent-context document from bridge documentation metadata.
///
/// The transform is deterministic: command paths and command inputs are sorted
/// before returning so callers get stable JSON after serialization.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::agent_context::bridge_ir_to_agent_context;
/// use cargo_orthohelp::schema::{DocMetadata, HeadingIds, SectionsMetadata};
///
/// let metadata = DocMetadata {
///     ir_version: "1.1".to_owned(),
///     app_name: "example".to_owned(),
///     bin_name: None,
///     about_id: "example.about".to_owned(),
///     synopsis_id: None,
///     sections: SectionsMetadata {
///         headings_ids: HeadingIds {
///             name: "heading.name".to_owned(),
///             synopsis: "heading.synopsis".to_owned(),
///             description: "heading.description".to_owned(),
///             options: "heading.options".to_owned(),
///             environment: "heading.environment".to_owned(),
///             files: "heading.files".to_owned(),
///             precedence: "heading.precedence".to_owned(),
///             exit_status: "heading.exit-status".to_owned(),
///             examples: "heading.examples".to_owned(),
///             see_also: "heading.see-also".to_owned(),
///             commands: None,
///         },
///         discovery: None,
///         precedence: None,
///         examples: Vec::new(),
///         links: Vec::new(),
///         notes: Vec::new(),
///     },
///     fields: Vec::new(),
///     subcommands: Vec::new(),
///     windows: None,
/// };
///
/// let context = bridge_ir_to_agent_context(&metadata, "example", None);
/// assert_eq!(context.kind, "example.agent_context");
/// assert_eq!(context.commands[0].path, ["example"]);
/// ```
#[must_use]
pub fn bridge_ir_to_agent_context(
    meta: &DocMetadata,
    package: &str,
    localizer: Option<&dyn Localizer>,
) -> AgentContext {
    tracing::debug!(
        package = %package,
        root = %meta.app_name,
        "starting bridge IR to agent-context transformation",
    );
    let mut context = AgentContext::new(package);
    walk(meta, &[], &mut context.commands, localizer);
    context
        .commands
        .sort_by(|left, right| left.path.cmp(&right.path));
    for command in &mut context.commands {
        command
            .inputs
            .sort_by(|left, right| left.name.cmp(&right.name));
    }
    tracing::debug!(
        package = %package,
        command_count = context.commands.len(),
        "bridge IR to agent-context transformation complete",
    );
    context
}

/// Recursively transforms a `DocMetadata` node into `AgentCommand` entries.
///
/// `meta` is appended to `out`, `parent_path` supplies the already-resolved
/// command prefix, and `localizer` optionally resolves a concise summary.
/// Every child in `meta.subcommands` is then visited with the current command
/// path as its parent.
fn walk(
    meta: &DocMetadata,
    parent_path: &[String],
    out: &mut Vec<AgentCommand>,
    localizer: Option<&dyn Localizer>,
) {
    let path = command_path(meta, parent_path);
    let last_segment = path.last().map(String::as_str);
    out.push(AgentCommand {
        path: path.clone(),
        summary: resolve_summary(meta, localizer),
        canonical_verb: last_segment.and_then(canonical_verb_for),
        inputs: meta.fields.iter().filter_map(build_input).collect(),
        output_modes: Vec::new(),
        interaction_mode: InteractionMode::default(),
        mutation_effect: MutationEffect::default(),
        async_submission: None,
        delivery_route: None,
        pagination: None,
        examples: Vec::new(),
    });

    for subcommand in &meta.subcommands {
        walk(subcommand, &path, out, localizer);
    }
}

/// Builds the full command path for one metadata node.
///
/// The root command prefers `bin_name` because it is the invocable binary.
/// Child commands append `app_name` to the inherited path; a missing root
/// `bin_name` naturally falls back to `app_name`.
fn command_path(meta: &DocMetadata, parent_path: &[String]) -> Vec<String> {
    if parent_path.is_empty() {
        return vec![meta.bin_name.as_ref().unwrap_or(&meta.app_name).to_owned()];
    }
    let mut path = parent_path.to_vec();
    path.push(meta.app_name.clone());
    path
}

fn resolve_summary(meta: &DocMetadata, localizer: Option<&dyn Localizer>) -> Option<String> {
    let resolved = localizer?.lookup(&meta.about_id, None)?;
    let trimmed = resolved.trim();
    if trimmed.is_empty() || trimmed.starts_with("[missing:") {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn canonical_verb_for(last_segment: &str) -> Option<String> {
    CANONICAL_VERBS
        .contains(&last_segment)
        .then(|| last_segment.to_owned())
}

/// Maps CLI-visible field metadata into an agent input.
///
/// Returns `None` for fields without CLI metadata, fields hidden from help, and
/// non-positional fields with no long or short flag. Returned inputs populate
/// name, long flag, value type, required state, default display, and enum
/// values.
fn build_input(field: &FieldMetadata) -> Option<AgentInput> {
    let cli = field.cli.as_ref()?;
    if cli.hide_in_help {
        return None;
    }
    if should_skip_non_flag_input(field) {
        tracing::warn!(
            field = field.name,
            "skipping CLI input with no long or short flag that does not take a value"
        );
        return None;
    }
    Some(AgentInput {
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

fn map_input_value_type(field: &FieldMetadata) -> Option<String> {
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

fn enum_values(field: &FieldMetadata) -> Vec<String> {
    match &field.value {
        Some(ValueType::Enum { variants }) => variants.clone(),
        _ => field
            .cli
            .as_ref()
            .map(|cli| cli.possible_values.clone())
            .unwrap_or_default(),
    }
}

fn normalize_default_display(display: &str) -> String {
    let mut normalized = String::with_capacity(display.len());
    let mut chars = display.chars().peekable();
    let mut literal = LiteralState::default();

    while let Some(character) = chars.next() {
        if literal.copy_character(character, &mut normalized, &mut chars) {
            continue;
        }
        if character == '"' {
            literal.start_string(&normalized);
            normalized.push(character);
        } else if character == '\'' && starts_character_literal(&chars) {
            literal.start_character();
            normalized.push(character);
        } else if character == ':' && chars.peek() == Some(&':') {
            normalize_path_separator(&mut normalized, &mut chars);
        } else {
            normalized.push(character);
        }
    }

    normalized
}

#[derive(Default)]
struct LiteralState {
    quote: Option<char>,
    is_escaped: bool,
    raw_hashes: Option<usize>,
}

impl LiteralState {
    fn start_string(&mut self, prefix: &str) {
        self.raw_hashes = raw_string_hash_count(prefix);
        self.quote = self.raw_hashes.is_none().then_some('"');
    }

    const fn start_character(&mut self) {
        self.quote = Some('\'');
    }

    fn copy_character(
        &mut self,
        character: char,
        normalized: &mut String,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> bool {
        if let Some(hash_count) = self.raw_hashes {
            normalized.push(character);
            if character == '"' && next_chars_are_hashes(chars, hash_count) {
                copy_hashes(normalized, chars, hash_count);
                self.raw_hashes = None;
            }
            return true;
        }
        let Some(quote) = self.quote else {
            return false;
        };

        normalized.push(character);
        if self.is_escaped {
            self.is_escaped = false;
        } else if character == '\\' {
            self.is_escaped = true;
        } else if character == quote {
            self.quote = None;
        }
        true
    }
}

fn starts_character_literal(chars: &std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let mut lookahead = chars.clone();
    match lookahead.next() {
        Some('\\') => escaped_character_has_closing_quote(&mut lookahead),
        Some('\'' | '\n' | '\r') | None => false,
        Some(_) => lookahead.next() == Some('\''),
    }
}

fn escaped_character_has_closing_quote(chars: &mut impl Iterator<Item = char>) -> bool {
    match chars.next() {
        Some('x') => chars.nth(2) == Some('\''),
        Some('u') if chars.next() == Some('{') => {
            chars.any(|character| character == '}') && chars.next() == Some('\'')
        }
        Some(_) => chars.next() == Some('\''),
        None => false,
    }
}

fn raw_string_hash_count(prefix: &str) -> Option<usize> {
    let hash_count = prefix.chars().rev().take_while(|char| *char == '#').count();
    let before_hashes = prefix.trim_end_matches('#');
    let before_marker = ["br", "cr", "r"]
        .into_iter()
        .find_map(|marker| before_hashes.strip_suffix(marker))?;

    (!before_marker
        .chars()
        .next_back()
        .is_some_and(|char| char == '_' || char.is_alphanumeric()))
    .then_some(hash_count)
}

fn next_chars_are_hashes(
    chars: &std::iter::Peekable<std::str::Chars<'_>>,
    hash_count: usize,
) -> bool {
    chars.clone().take(hash_count).all(|char| char == '#')
}

fn copy_hashes(
    normalized: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    hash_count: usize,
) {
    for _ in 0..hash_count {
        if let Some(hash) = chars.next() {
            normalized.push(hash);
        }
    }
}

fn normalize_path_separator(
    normalized: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) {
    while normalized
        .chars()
        .next_back()
        .is_some_and(char::is_whitespace)
    {
        normalized.pop();
    }
    normalized.push_str("::");
    chars.next();
    while chars.peek().is_some_and(|next| next.is_whitespace()) {
        chars.next();
    }
}

#[cfg(test)]
mod proptests;

#[cfg(test)]
mod tests;
