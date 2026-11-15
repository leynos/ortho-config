//! Trybuild fixture: the `_with_env` loaders accept one shared environment.
//!
//! A bespoke source implementing both `EnvSource` and `ScanEnvSource` backs the
//! whole load through one `Arc`, for the trait method, the generated
//! composition and profile methods, and the subcommand merge.

use clap::Parser;
use ortho_config::{EnvSource, OrthoConfig, ScanEnvSource, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::sync::Arc;

/// A source that holds nothing and reports a fixed directory.
#[derive(Debug)]
struct Fixed;

impl EnvSource for Fixed {
    fn get(&self, _key: &str) -> Option<OsString> {
        None
    }
}

impl ScanEnvSource for Fixed {
    fn scan(&self) -> Vec<(OsString, OsString)> {
        Vec::new()
    }
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "FIXED_", profiles)]
struct Config {
    #[ortho_config(default = 1)]
    port: u16,
}

#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, Default)]
#[command(name = "fixed")]
#[ortho_config(prefix = "FIXED_")]
struct Command {
    #[arg(long)]
    name: Option<String>,
}

fn main() {
    let env = Arc::new(Fixed);
    let _ = Config::load_from_iter_with_env(["fixed"], env.clone());
    let _ = Config::compose_layers_from_iter_with_env(["fixed"], env.clone());
    let _ = Config::load_with_profile_from_iter_with_env(["fixed"], env.clone());
    let _ = Command::default().load_and_merge_with_env(env);
}
