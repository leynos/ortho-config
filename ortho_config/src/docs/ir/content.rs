//! Reusable presentation content attached at document or field level.
//!
//! Examples, links, and notes appear in three places: the command overview, a
//! section, and an individual field. Keeping them in one module means a
//! renderer that learns to draw a note draws every note, and the three owners
//! never drift into subtly different shapes.

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
