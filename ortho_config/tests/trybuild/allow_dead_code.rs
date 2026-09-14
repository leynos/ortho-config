//! Trybuild fixture for generated compose-layer lint attributes.

#![allow(dead_code)]
#![deny(unfulfilled_lint_expectations)]

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
struct BuildScriptConfig {
    #[serde(default)]
    value: String,
}

fn main() {}
