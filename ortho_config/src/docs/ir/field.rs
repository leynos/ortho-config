//! Field-level documentation metadata.
//!
//! One configuration field is described by its Rust name, its help message
//! identifiers, its declared type and default, and the surface it is reachable
//! from: the command line, the environment, or a file. Keeping the type and
//! default descriptions next to the field record means a renderer can format a
//! value without consulting another module.

use serde::{Deserialize, Serialize};

use super::content::{Example, Link, Note};

/// Metadata describing a single configuration field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldMetadata {
    /// Rust field name for the setting.
    pub name: String,
    /// Fluent ID for the short help text.
    pub help_id: String,
    /// Optional Fluent ID for long-form help text.
    pub long_help_id: Option<String>,
    /// Optional semantic value type for formatting.
    pub value: Option<ValueType>,
    /// Optional default value display string.
    pub default: Option<DefaultValue>,
    /// Whether the field is required.
    pub required: bool,
    /// Deprecation metadata, if any.
    pub deprecated: Option<Deprecation>,
    /// CLI-related documentation metadata.
    pub cli: Option<CliMetadata>,
    /// Environment-variable documentation metadata.
    pub env: Option<EnvMetadata>,
    /// File configuration documentation metadata.
    pub file: Option<FileMetadata>,
    /// Field-level examples.
    pub examples: Vec<Example>,
    /// Field-level related links.
    pub links: Vec<Link>,
    /// Field-level notes.
    pub notes: Vec<Note>,
}

/// CLI documentation metadata for a field.
///
/// The four flags are independent facts about a single flag rather than the
/// states of one machine: a flag can repeat *and* take a value *and* accept it
/// optionally *and* be hidden. Collapsing them into enums would change the IR
/// wire format that consumers read.
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent flag properties mirror the serialized schema."
)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliMetadata {
    /// Long CLI flag name (without the leading dashes).
    pub long: Option<String>,
    /// Optional short CLI flag character.
    pub short: Option<char>,
    /// Optional CLI value placeholder.
    pub value_name: Option<String>,
    /// Whether the field accepts repeated values.
    pub multiple: bool,
    /// Whether the CLI flag takes a value (false for switches).
    pub takes_value: bool,
    /// Whether the value is optional, as in `--flag[=<BOOL>]`.
    ///
    /// Boolean flags accept a value but do not require one: the bare spelling
    /// means `true`, and `--flag=false` supplies an explicit `false`. Renderers
    /// bracket the placeholder when this is set.
    ///
    /// Defaults to `false` so IR written before this field existed still reads
    /// as a plain switch, per ADR-003.
    #[serde(default)]
    pub value_optional: bool,
    /// Allowed values for enum-like options.
    pub possible_values: Vec<String>,
    /// Whether the flag is hidden from help output.
    pub hide_in_help: bool,
}

/// Environment variable documentation metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvMetadata {
    /// Environment variable name.
    pub var_name: String,
}

/// File configuration metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileMetadata {
    /// Dotted configuration key path.
    pub key_path: String,
}

/// Strongly-typed value metadata for documentation rendering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValueType {
    /// String value.
    String,
    /// Integer value with bit-width and sign.
    Integer {
        /// Integer bit width.
        bits: u8,
        /// Whether the integer is signed.
        signed: bool,
    },
    /// Floating point value with bit-width.
    Float {
        /// Floating point bit width.
        bits: u8,
    },
    /// Boolean switch.
    Bool,
    /// Duration value.
    Duration,
    /// File system path.
    Path,
    /// IP address.
    IpAddr,
    /// Hostname value.
    Hostname,
    /// URL value.
    Url,
    /// Enumeration of allowed values.
    Enum {
        /// Allowed variants for the enum.
        variants: Vec<String>,
    },
    /// List of nested values.
    List {
        /// Element type for list values.
        of: Box<Self>,
    },
    /// Map with value type metadata.
    Map {
        /// Value type for map entries.
        of: Box<Self>,
    },
    /// Custom domain-specific value type.
    Custom {
        /// Human-readable type name.
        name: String,
    },
}

/// Display value for a default.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefaultValue {
    /// Human-readable default string.
    pub display: String,
}

/// Deprecation metadata for a field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Deprecation {
    /// Fluent ID for the deprecation note.
    pub note_id: String,
}
