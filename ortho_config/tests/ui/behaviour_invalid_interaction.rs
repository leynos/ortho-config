//! Compile-fail fixture: behaviour invalid interaction

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behaviour(interaction = "sometimes"))]
struct Bad {
    value: u8,
}

/// Compile-fail harness needs a `main`, but compilation stops at the
/// attribute that rejects an unrecognised `interaction` value. The pinned
/// diagnostic is compared verbatim from the sibling `.stderr` file.
fn main() {}
