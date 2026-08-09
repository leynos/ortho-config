//! Policy configuration model and Cargo-metadata parsing.
//!
//! The 7.1.1 configuration surface is `[package.metadata.ortho_config.policy]`:
//! one enforcement mode plus an explicit exception list. The wire form
//! [`PolicyConfigMetadata`] is deserialized strictly from the metadata table
//! (unknown keys are errors, per Decision D7), then resolved into the domain
//! [`PolicyConfig`] consumed by the policy evaluator. Roadmap 7.1.2 reserves a
//! `rules` key inside the table for per-rule levels; until then the strict
//! deserializer rejects it, which is the documented version-skew consequence of
//! the strictness trade-off.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::policy::PolicyMode;

/// Resolved policy configuration for one evaluation.
///
/// The enforcement default is `PolicyMode::Off`, so running the check against
/// a package with no policy table checks nothing (Decision D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyConfig {
    /// Enforcement mode in effect for the evaluation.
    pub mode: PolicyMode,
    /// Explicit project exceptions honouring the configuration.
    pub exceptions: Vec<PolicyException>,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            mode: PolicyMode::Off,
            exceptions: Vec::new(),
        }
    }
}

/// Wire form of `[package.metadata.ortho_config.policy]` as Cargo provides it.
///
/// Unknown keys inside the table are rejected so a misspelt option fails in
/// all modes instead of silently disabling policy (Decision D7). The reserved
/// `rules` key from Decision D1 is therefore also rejected until 7.1.2 lands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyConfigMetadata {
    /// Enforcement mode; defaults to `off` (opt-in).
    #[serde(default = "default_policy_mode")]
    pub mode: PolicyMode,
    /// Explicit project exceptions.
    #[serde(default)]
    pub exceptions: Vec<PolicyException>,
}

impl Default for PolicyConfigMetadata {
    fn default() -> Self {
        Self {
            mode: PolicyMode::Off,
            exceptions: Vec::new(),
        }
    }
}

impl PolicyConfigMetadata {
    /// Reads the optional policy table from a package's Cargo metadata value.
    ///
    /// `Cargo` exposes `package.metadata` as a JSON object; a missing
    /// `ortho_config` or `policy` key yields `None`, and a present table is
    /// deserialized strictly.
    ///
    /// # Errors
    ///
    /// Returns a `serde_json` error when the policy table exists but does not
    /// match the strict schema (for example an unknown key or a missing
    /// `reason` on an exception).
    pub fn from_package_metadata(
        metadata: &serde_json::Value,
    ) -> Result<Option<Self>, serde_json::Error> {
        let Some(ortho_config) = metadata.get("ortho_config") else {
            return Ok(None);
        };
        let Some(policy) = ortho_config.get("policy") else {
            return Ok(None);
        };
        serde_json::from_value(policy.clone()).map(Some)
    }
}

impl From<PolicyConfigMetadata> for PolicyConfig {
    fn from(metadata: PolicyConfigMetadata) -> Self {
        Self {
            mode: metadata.mode,
            exceptions: metadata.exceptions,
        }
    }
}

impl From<&PolicyConfigMetadata> for PolicyConfig {
    fn from(metadata: &PolicyConfigMetadata) -> Self {
        Self {
            mode: metadata.mode.clone(),
            exceptions: metadata.exceptions.clone(),
        }
    }
}

/// One explicit project exception from the policy table.
///
/// `reason` is mandatory so exceptions stay honest and reviewable; a missing
/// reason is a deserialization error. `command_path`, when present, scopes the
/// exception to one space-separated invocation path; otherwise the exception
/// is global (Decision D3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyException {
    /// Whether the exception names a verb or a flag.
    pub kind: ExceptionKind,
    /// Verb or flag name, written the way it appears on the command line.
    pub name: String,
    /// Why the project exempts this vocabulary item.
    pub reason: String,
    /// Optional command path the exception is scoped to.
    #[serde(default)]
    pub command_path: Option<String>,
}

/// Whether a policy exception names a verb or a flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExceptionKind {
    /// A canonical command verb such as `get` or `list`.
    Verb,
    /// A canonical long flag such as `--json`.
    Flag,
}

impl fmt::Display for ExceptionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verb => f.write_str("verb"),
            Self::Flag => f.write_str("flag"),
        }
    }
}

/// Inputs to a policy evaluation besides the configuration.
///
/// Empty in 7.1.1. Roadmap 7.1.2 passes the bridge IR command tree through
/// this struct additively, so the evaluator seam stays source-compatible.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PolicyInputs {}

const fn default_policy_mode() -> PolicyMode {
    PolicyMode::Off
}

#[cfg(test)]
mod tests;
