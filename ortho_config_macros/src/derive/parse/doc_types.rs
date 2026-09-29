//! Documentation attribute type definitions.
//!
//! Extracted from `doc_attrs` to keep module sizes under 400 lines.

/// Struct-level documentation attributes captured during parsing.
#[derive(Default, Clone)]
pub(crate) struct DocStructAttrs {
    /// Translation key for the generated command's about text.
    pub about_id: Option<String>,
    /// Translation key for the generated synopsis text.
    pub synopsis_id: Option<String>,
    /// Explicit executable name retained for generated help and discovery metadata.
    pub bin_name: Option<String>,
    /// Per-section translation-key overrides; unset entries use built-in keys.
    pub headings: HeadingOverrides,
    /// Optional source ordering and rationale used to explain configuration precedence.
    pub precedence: Option<PrecedenceAttrs>,
    /// Examples kept in source order for inclusion in generated documentation.
    pub examples: Vec<DocExampleAttr>,
    /// Related links kept in source order for inclusion in generated documentation.
    pub links: Vec<DocLinkAttr>,
    /// Explanatory notes kept in source order for inclusion in generated documentation.
    pub notes: Vec<DocNoteAttr>,
    /// Windows help metadata, absent when no Windows-specific section was declared.
    pub windows: Option<WindowsAttrs>,
}

/// Heading ID overrides for documentation sections.
#[derive(Default, Clone)]
pub(crate) struct HeadingOverrides {
    /// Replacement translation key for the command name heading.
    pub name: Option<String>,
    /// Replacement translation key for the synopsis heading.
    pub synopsis: Option<String>,
    /// Replacement translation key for the description heading.
    pub description: Option<String>,
    /// Replacement translation key for the options heading.
    pub options: Option<String>,
    /// Replacement translation key for the environment heading.
    pub environment: Option<String>,
    /// Replacement translation key for the configuration files heading.
    pub files: Option<String>,
    /// Replacement translation key for the precedence heading.
    pub precedence: Option<String>,
    /// Replacement translation key for the exit-status heading.
    pub exit_status: Option<String>,
    /// Replacement translation key for the examples heading.
    pub examples: Option<String>,
    /// Replacement translation key for the related-links heading.
    pub see_also: Option<String>,
    /// Replacement translation key for the subcommands heading.
    pub commands: Option<String>,
}

/// Precedence configuration attributes.
#[derive(Default, Clone)]
pub(crate) struct PrecedenceAttrs {
    /// Declared source names in highest-to-lowest precedence order.
    pub order: Vec<String>,
    /// Translation key for the explanation accompanying the declared order.
    pub rationale_id: Option<String>,
}

/// Windows-specific documentation metadata attributes.
#[derive(Default, Clone)]
pub(crate) struct WindowsAttrs {
    /// `PowerShell` module name used when generating Windows help metadata.
    pub module_name: Option<String>,
    /// Additional exported command names represented by generated help.
    pub export_aliases: Vec<String>,
    /// Whether common `PowerShell` parameters appear in generated help.
    pub include_common_parameters: Option<bool>,
    /// Whether each subcommand is emitted as a separate help function.
    pub split_subcommands: Option<bool>,
    /// Optional URI attached to generated `PowerShell` help metadata.
    pub help_info_uri: Option<String>,
}

/// Field-level documentation attributes captured during parsing.
#[derive(Default, Clone)]
pub(crate) struct DocFieldAttrs {
    /// Translation key for the field's short help text.
    pub help_id: Option<String>,
    /// Translation key for the field's extended help text.
    pub long_help_id: Option<String>,
    /// Display type override used by generated field documentation.
    pub value_type: Option<String>,
    /// Translation key for the note explaining a deprecated field.
    pub deprecated_note_id: Option<String>,
    /// Explicit requiredness override retained for generated documentation.
    pub required: Option<bool>,
    /// Explicit environment-variable name associated with the field.
    pub env_name: Option<String>,
    /// Configuration-file key path associated with the field.
    pub file_key_path: Option<String>,
    /// Display name for values supplied through the CLI.
    pub cli_value_name: Option<String>,
    /// Hides the field's CLI value from generated help output when true.
    pub cli_hide_in_help: bool,
    /// Field examples kept in declaration order.
    pub examples: Vec<DocExampleAttr>,
    /// Field links kept in declaration order.
    pub links: Vec<DocLinkAttr>,
    /// Field notes kept in declaration order.
    pub notes: Vec<DocNoteAttr>,
}

/// An example entry for documentation.
#[derive(Clone)]
pub(crate) struct DocExampleAttr {
    /// Optional translation key for the example title.
    pub title_id: Option<String>,
    /// Literal example body retained for generated documentation.
    pub code: String,
    /// Optional translation key for explanatory text below the example.
    pub body_id: Option<String>,
}

/// A link entry for documentation.
#[derive(Clone)]
pub(crate) struct DocLinkAttr {
    /// Optional translation key for the link label.
    pub text_id: Option<String>,
    /// Destination retained verbatim for the generated link.
    pub uri: String,
}

/// A note entry for documentation.
#[derive(Clone)]
pub(crate) struct DocNoteAttr {
    /// Translation key for the note text emitted in generated documentation.
    pub text_id: String,
}
