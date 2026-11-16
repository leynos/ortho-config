//! Supplied claim and command-surface types for skill validation.
//!
//! A consumer owns skill-manifest loading, format validation, prose parsing,
//! and example extraction. It hands this module an inert, already-normalized
//! description of what a manifest claims and what each real command accepts.
//! Nothing here reads a file, parses Markdown, or executes a command.

use ortho_config::SkillCommandRef;
use serde::{Deserialize, Serialize};

/// One validation request.
///
/// Manifests and examples are separate lists because an example names its
/// owning manifest by ID. Keeping them apart is what lets a duplicate manifest
/// ID be reported as ambiguous instead of silently resolved by nesting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillValidationInput {
    /// Manifest claims, in the consumer's source order.
    pub manifests: Vec<ManifestClaims>,
    /// Normalized example claims, in the consumer's source order.
    pub examples: Vec<NormalizedExample>,
}

/// The command claims taken from one skill manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestClaims {
    /// Stable opaque identifier. Findings quote this value, never the path.
    pub id: String,
    /// Exact command references declared by the manifest.
    pub commands: Vec<SkillCommandRef>,
}

/// Whether an example was fully extracted or is only partially understood.
///
/// An unsupported example is preserved in the report as unchecked. Counting it
/// as validated would claim coverage the consumer never supplied.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExampleExtraction {
    /// Every command token, option name, and value was extracted.
    #[default]
    Checked,
    /// The consumer could not extract the example completely.
    Unsupported,
}

/// One normalized example claim.
///
/// Option values are preserved verbatim. The validator checks value *shape*
/// only where a policy rule is value-sensitive; it never claims the value is
/// valid for a domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedExample {
    /// ID of the manifest whose command reference this example elaborates.
    pub manifest_id: String,
    /// One-based position of the reference in the owning manifest.
    pub command_reference_ordinal: usize,
    /// Invocation path the example actually shows.
    pub path: Vec<String>,
    /// Option uses in source order.
    pub options: Vec<OptionUse>,
    /// Positional arguments in source order.
    pub positionals: Vec<String>,
    /// One-based position of the example in the consumer's source.
    pub source_ordinal: usize,
    /// Whether extraction was complete.
    pub extraction: ExampleExtraction,
}

/// One option use inside a normalized example.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionUse {
    /// Long option name, without the leading `--`.
    pub name: String,
    /// Attached or separate value; `None` means the option was used as a flag.
    pub value: Option<String>,
    /// One-based position of the option in the example.
    pub source_ordinal: usize,
}

/// The complete, explicit command surface supplied by a consumer or bridge.
///
/// The surface is *effective*: every path lists the options that command
/// really accepts, including any global options resolved onto it. The
/// validator never unions a root command's inputs into a child.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveCommandSurface {
    /// Command contracts in the consumer's order.
    pub commands: Vec<CommandContract>,
}

/// The effective contract of one real command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandContract {
    /// Exact invocation path, for example `["tool", "records", "get"]`.
    pub path: Vec<String>,
    /// Effective long options, with their declared value shapes.
    pub options: Vec<EffectiveOption>,
    /// Verb the command declares, when the bridge knows one.
    pub canonical_verb: Option<String>,
}

/// One effective long option on a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveOption {
    /// Long option name, without the leading `--`.
    pub long: String,
    /// Declared value type such as `string`, `bool`, or `path`.
    pub value_type: Option<String>,
    /// Whether this option is a global option on this command.
    ///
    /// The consumer or bridge states this explicitly; the validator never
    /// infers it from a name, from the root command, or from a parent path.
    /// Global-option policy rules, such as the shared glossary's canonical
    /// spelling and accepted values, apply only when this is true. A domain
    /// command with its own local `--color` therefore never receives advice
    /// meant for the global colour-policy option.
    #[serde(default)]
    pub is_global: bool,
}

/// A typed failure in the supplied command surface.
///
/// Surface integrity is checked before any claim is evaluated, so an
/// ambiguous contract never produces a misleading finding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandSurfaceError {
    /// A command contract has an empty invocation path.
    #[error("command surface entry {index} has an empty path")]
    EmptyPath {
        /// Zero-based position of the offending entry.
        index: usize,
    },
    /// Two command contracts share one path.
    #[error("command surface declares path `{path}` more than once")]
    DuplicatePath {
        /// Space-joined duplicate path.
        path: String,
    },
    /// A command declares an empty option name.
    #[error("command `{path}` declares an empty option name")]
    EmptyOption {
        /// Space-joined command path.
        path: String,
    },
    /// One option name carries conflicting metadata on one command.
    #[error("command `{path}` declares option `{option}` with conflicting metadata")]
    ConflictingOption {
        /// Space-joined command path.
        path: String,
        /// Offending option name.
        option: String,
    },
}
