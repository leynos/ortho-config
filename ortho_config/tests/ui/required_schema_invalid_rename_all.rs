//! The required-value schema records the key Serde deserializes, so a naming
//! rule the derive cannot reproduce must be rejected. Silently keeping the Rust
//! identifier would advertise a key the loader never reads.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// The container applies an unrecognised naming rule when deserializing.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[serde(rename_all(deserialize = "Sentence case"))]
struct Root {
    /// The deserialization key cannot be computed from this rule.
    reference: String,
}

fn main() {}