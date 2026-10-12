//! Compile-fail fixture: `localized_default` on a field.
//!
//! The attribute is deferred (Decision D-4) and is rejected wherever it is
//! written. A field-level spelling is the more dangerous of the two: field
//! keys that go unrecognised are discarded, so without an explicit rejection
//! the attribute would be accepted in silence and its text would simply never
//! reach a catalogue.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(prefix = "X")]
struct Bad {
    #[ortho_config(localized_default = "Hello")]
    greeting: String,
}

/// Compile-fail harness needs a `main`, but compilation stops at the
/// attribute that rejects a deferred `localized_default` placed on a field.
/// The pinned diagnostic is compared verbatim from the sibling `.stderr` file.
fn main() {}
