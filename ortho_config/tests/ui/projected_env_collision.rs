//! Rejects an environment-name collision introduced by the mounted namespace.
//!
//! With `prefix = "APP"` and `#[command(name = "run")]`, the implicit `token`
//! field is documented as `APP_CMDS_RUN_TOKEN` when mounted. A second field
//! with that explicit environment name would advertise the same variable, so
//! the derive rejects it.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Parser, OrthoConfig)]
#[command(name = "run")]
#[ortho_config(prefix = "APP")]
struct Mounted {
    token: Option<String>,
    #[ortho_config(env(name = "APP_CMDS_RUN_TOKEN"))]
    override_token: Option<String>,
}

fn main() {}