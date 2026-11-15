//! Compile-fail fixture: behaviour noninteractive bypass split across groups

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(behaviour(interaction = "non_interactive"))]
#[ortho_config(behaviour(bypass = "--force"))]
struct Bad {
    value: u8,
}

/// Compile-fail harness needs a `main`, but compilation stops at the
/// attribute that rejects a non-interactive/bypass contradiction split
/// across two groups. The pinned diagnostic is compared verbatim from the
/// sibling `.stderr` file.
fn main() {}
