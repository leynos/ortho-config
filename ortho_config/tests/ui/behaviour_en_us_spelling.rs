//! Compile-fail fixture: behaviour en us spelling

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behavior(interaction = "interactive"))]
struct Bad {
    value: u8,
}

/// Compile-fail harness needs a `main`, but compilation stops at the
/// attribute that rejects the American spelling `behavior` in place of
/// `behaviour`. The pinned diagnostic is compared verbatim from the sibling
/// `.stderr` file.
fn main() {}
