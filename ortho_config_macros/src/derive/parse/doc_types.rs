//! Documentation attribute type definitions.
//!
//! Extracted from `doc_attrs` to keep module sizes under 400 lines.

/// Struct-level documentation attributes captured during parsing.
#[derive(Default, Clone)]
pub(crate) struct DocStructAttrs {
    pub about_id: Option<String>,
    pub synopsis_id: Option<String>,
    pub bin_name: Option<String>,
    pub headings: HeadingOverrides,
    pub precedence: Option<PrecedenceAttrs>,
    pub examples: Vec<DocExampleAttr>,
    pub links: Vec<DocLinkAttr>,
    pub notes: Vec<DocNoteAttr>,
    pub windows: Option<WindowsAttrs>,
}

/// Heading ID overrides for documentation sections.
#[derive(Default, Clone)]
pub(crate) struct HeadingOverrides {
    pub name: Option<String>,
    pub synopsis: Option<String>,
    pub description: Option<String>,
    pub options: Option<String>,
    pub environment: Option<String>,
    pub files: Option<String>,
    pub precedence: Option<String>,
    pub exit_status: Option<String>,
    pub examples: Option<String>,
    pub see_also: Option<String>,
}

/// Precedence configuration attributes.
#[derive(Default, Clone)]
pub(crate) struct PrecedenceAttrs {
    pub order: Vec<String>,
    pub rationale_id: Option<String>,
}

/// Windows-specific documentation metadata attributes.
#[derive(Default, Clone)]
pub(crate) struct WindowsAttrs {
    pub module_name: Option<String>,
    pub export_aliases: Vec<String>,
    pub include_common_parameters: Option<bool>,
    pub split_subcommands: Option<bool>,
    pub help_info_uri: Option<String>,
}

/// Field-level documentation attributes captured during parsing.
#[derive(Default, Clone)]
pub(crate) struct DocFieldAttrs {
    pub help_id: Option<String>,
    pub long_help_id: Option<String>,
    pub value_type: Option<String>,
    pub deprecated_note_id: Option<String>,
    pub required: Option<bool>,
    pub env_name: Option<String>,
    pub file_key_path: Option<String>,
    pub cli_value_name: Option<String>,
    pub cli_hide_in_help: bool,
    pub examples: Vec<DocExampleAttr>,
    pub links: Vec<DocLinkAttr>,
    pub notes: Vec<DocNoteAttr>,
}

/// An example entry for documentation.
#[derive(Clone)]
pub(crate) struct DocExampleAttr {
    pub title_id: Option<String>,
    pub code: String,
    pub body_id: Option<String>,
}

/// A link entry for documentation.
#[derive(Clone)]
pub(crate) struct DocLinkAttr {
    pub text_id: Option<String>,
    pub uri: String,
}

/// A note entry for documentation.
#[derive(Clone)]
pub(crate) struct DocNoteAttr {
    pub text_id: String,
}
