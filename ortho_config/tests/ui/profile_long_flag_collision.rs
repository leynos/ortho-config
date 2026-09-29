//! Rejects a user field claiming the generated `--profile` long flag.
//!
//! Exercises collision projection (b) from the profile-opt-in contract:
//! opting in reserves the `--profile` long flag, so a user
//! `cli_long = "profile"` is a compile-time error rather than two clap
//! arguments sharing the flag. `profile_config_long_collision.rs` covers the
//! mirror case where the generated config flag claims it instead.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, OrthoConfig)]
#[ortho_config(profiles)]
struct Collides {
    #[ortho_config(cli_long = "profile")]
    thing: Option<String>,
}

fn main() {}