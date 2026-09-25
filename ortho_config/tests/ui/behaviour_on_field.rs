//! Compile-fail fixture: behaviour on field

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(prefix = "X")]
struct Bad {
    #[ortho_config(behaviour(mutation = "write"))]
    value: u8,
}

/// Compile-fail harness needs a `main`, but compilation stops at the
/// attribute that rejects a `behaviour(...)` attribute placed on a field
/// rather than a struct. The pinned diagnostic is compared verbatim from
/// the sibling `.stderr` file.
fn main() {}
