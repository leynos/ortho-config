//! Rejects a user field claiming the generated `<PREFIX>PROFILE` binding.
//!
//! Exercises collision projection (c) from the profile-opt-in contract: with
//! `prefix = "APP_"` the selector environment variable is `APP_PROFILE`, so a
//! user `#[arg(env = "APP_PROFILE")]` is a compile-time error rather than two
//! clap arguments reading the same variable.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Parser, OrthoConfig)]
#[ortho_config(prefix = "APP_", profiles)]
struct Collides {
    #[arg(env = "APP_PROFILE")]
    thing: Option<String>,
}

fn main() {}