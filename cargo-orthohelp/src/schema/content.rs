//! Local mirror of the IR's reusable presentation content.
//!
//! Examples, links, and notes are attached at the document, section, and field
//! level, so the renderer meets the same three shapes wherever it walks. This
//! module exists only to mirror `ortho_config::docs::ir::content`.

use serde::{Deserialize, Serialize};

/// Documentation example snippet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Example {
    /// Optional Fluent ID for the example title.
    pub title_id: Option<String>,
    /// Example code snippet.
    pub code: String,
    /// Optional Fluent ID for the example body text.
    pub body_id: Option<String>,
}

/// Related link metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Link {
    /// Optional Fluent ID for the link text.
    pub text_id: Option<String>,
    /// URI associated with the link.
    pub uri: String,
}

/// Documentation note metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Note {
    /// Fluent ID for the note content.
    pub text_id: String,
}
