//! Compile-fail fixture: behaviour bad bypass

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behaviour(bypass = "force"))]
struct Bad {
    value: u8,
}

/// Compile-fail harness needs a `main`, but compilation stops at the
/// attribute that rejects a bypass value outside the pinned
/// `--[a-z0-9]+(-[a-z0-9]+)*` grammar. The pinned diagnostic is compared
/// verbatim from the sibling `.stderr` file.
fn main() {}
