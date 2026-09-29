//! Rejects a bare `#[arg(env)]` field that claims the selector binding.
//!
//! The `env = "NAME"` form is covered by `profile_env_binding_collision.rs`.
//! This case covers the bare form, where clap derives the variable name from
//! the field identifier in `SCREAMING_SNAKE_CASE`: with `prefix = "APP_"` the
//! selector environment variable is `APP_PROFILE`, so a field named
//! `app_profile` declaring `#[arg(long, env)]` makes both clap arguments read
//! `APP_PROFILE`. That must be the same compile-time error rather than a
//! silent double binding.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Parser, OrthoConfig)]
#[ortho_config(prefix = "APP_", profiles)]
struct Collides {
    #[arg(long, env)]
    app_profile: Option<String>,
}

fn main() {}
