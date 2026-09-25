//! Compile-fail fixture: behaviour unknown nested key

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behaviour(interation = "interactive"))]
struct Bad {
    value: u8,
}

/// Compile-fail harness needs a `main`, but compilation stops at the
/// attribute that rejects a misspelt key nested inside `behaviour(...)`.
/// The pinned diagnostic is compared verbatim from the sibling `.stderr`
/// file.
fn main() {}
