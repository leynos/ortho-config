//! Data structures that capture layered configuration overrides.
//!
//! These types exist solely to bridge between figment providers and the
//! higher-level CLI structures, keeping serialization concerns isolated from
//! command parsing logic.

use serde::{Deserialize, Serialize};

use crate::cli::is_false;

#[derive(Serialize)]
pub(crate) struct Overrides<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recipient: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) salutations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) is_excited: Option<bool>,
    #[serde(skip_serializing_if = "is_false")]
    pub(crate) is_quiet: bool,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct FileOverrides {
    #[serde(default)]
    pub(crate) is_excited: Option<bool>,
    #[serde(default)]
    pub(crate) cmds: CommandOverrides,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct CommandOverrides {
    #[serde(default)]
    pub(crate) greet: Option<GreetOverrides>,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct GreetOverrides {
    #[serde(default)]
    pub(crate) preamble: Option<String>,
    #[serde(default)]
    pub(crate) punctuation: Option<String>,
}
