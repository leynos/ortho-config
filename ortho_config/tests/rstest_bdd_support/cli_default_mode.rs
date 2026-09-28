//! Value-enum fixture for CLI-default-as-absent scenarios.

use serde::{Deserialize, Serialize};

/// Output modes used to exercise clap `ValueEnum` default inference.
#[derive(Debug, Deserialize, Serialize, clap::ValueEnum, Default, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CliDefaultMode {
    #[default]
    Fast,
    Safe,
}
