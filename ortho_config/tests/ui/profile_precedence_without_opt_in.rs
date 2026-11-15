//! Rejects a `profile` precedence tier on a struct that did not opt in.
//!
//! A struct without `#[ortho_config(profiles)]` never has a profile layer
//! pushed, so an explicit order naming that tier would publish precedence
//! metadata for a layer the loader does not build. The spelling is rejected at
//! compile time so the documented order and the real order cannot diverge.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(precedence(order = ["defaults", "file", "profile", "env", "cli"]))]
struct Bad {
    #[serde(default)]
    retries: u32,
}

fn main() {}
