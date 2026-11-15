//! Rejects a config long flag that would collide with the generated `--profile`.
//!
//! Exercises collision projection (b) from the profile-opt-in contract: the
//! generated config flag's effective long option is reserved before the
//! profile flag is built, so `discovery(config_cli_long = "profile")` on an
//! opted-in struct is a compile-time error rather than two `--profile`
//! arguments reaching Clap.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, OrthoConfig)]
#[ortho_config(profiles, discovery(config_cli_long = "profile"))]
struct Collides {
    thing: Option<String>,
}

fn main() {}
