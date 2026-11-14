//! Reusable validation of skill-manifest claims against real commands.
//!
//! A consumer extracts command references and structured examples from a
//! skill manifest, supplies an explicit *effective* command surface, and
//! receives one deterministic report. The validator checks:
//!
//! - every referenced command path exists in the surface;
//! - every declared flag exists on the resolved command;
//! - every example binds to one manifest reference and uses only options that
//!   the reference declares and the real command accepts;
//! - canonical vocabulary and the global-option glossary hold under the
//!   configured policy mode and exceptions.
//!
//! Structural findings are independent of policy mode. `off` silences
//! vocabulary advice only; it can never make a false command claim valid.
//!
//! The validator never reads files, parses Markdown, or executes examples.
//! Manifest paths are inert metadata, and findings quote the stable manifest
//! ID plus a one-based source ordinal rather than a path or argument value.
//!
//! # Examples
//!
//! ```rust
//! use cargo_orthohelp::policy::PolicyMode;
//! use cargo_orthohelp::skill_validation::{
//!     CommandContract, EffectiveCommandSurface, EffectiveOption, ManifestClaims,
//!     ResolvedAgentPolicy, SkillValidationInput, validate,
//! };
//! use ortho_config::SkillCommandRef;
//!
//! let path = vec!["tool".to_owned(), "records".to_owned(), "get".to_owned()];
//! let surface = EffectiveCommandSurface {
//!     commands: vec![CommandContract {
//!         path: path.clone(),
//!         options: vec![EffectiveOption {
//!             long: "json".to_owned(),
//!             value_type: Some("bool".to_owned()),
//!         }],
//!         canonical_verb: Some("get".to_owned()),
//!     }],
//! };
//! let input = SkillValidationInput {
//!     manifests: vec![ManifestClaims {
//!         id: "operation-skill".to_owned(),
//!         commands: vec![SkillCommandRef {
//!             path,
//!             flags: vec!["json".to_owned()],
//!         }],
//!     }],
//!     examples: Vec::new(),
//! };
//! let policy = ResolvedAgentPolicy {
//!     mode: PolicyMode::Off,
//!     exceptions: Vec::new(),
//! };
//! let report = validate(&input, &surface, &policy).expect("surface is unambiguous");
//! assert!(report.findings.is_empty());
//! assert_eq!(report.coverage.resolved_command_references, 1);
//! ```

pub mod input;
pub mod policy_check;
pub mod report;
mod resolve;
mod validate;

pub use input::{
    CommandContract, CommandSurfaceError, EffectiveCommandSurface, EffectiveOption,
    ExampleExtraction, ManifestClaims, NormalizedExample, OptionUse, SkillValidationInput,
};
pub use policy_check::ResolvedAgentPolicy;
pub use report::{
    Coverage, FindingCategory, SkillValidationReport, ValidationFinding, ValidationStatus, codes,
};
pub use validate::validate;

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_examples;
#[cfg(test)]
mod tests_policy;
