//! Compact machine-readable command context for agent-native CLIs.
//!
//! These types describe invocation contracts for tools that want to expose a
//! small JSON surface to automation. They intentionally sit beside the
//! localized documentation IR instead of replacing it.

use serde::{Deserialize, Serialize};

/// Current agent-context schema version.
pub const ORTHO_AGENT_CONTEXT_SCHEMA_VERSION: &str = "1";

/// Canonical suffix used for agent-context document kinds.
pub const AGENT_CONTEXT_KIND_SUFFIX: &str = "agent_context";

/// Top-level agent-context document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentContext {
    /// Agent-context schema version string.
    pub schema_version: String,
    /// Stable machine-readable document kind.
    pub kind: String,
    /// Package or command surface name.
    pub package: String,
    /// Commands exposed by this package.
    pub commands: Vec<AgentCommand>,
    /// Profile support declared by the application.
    #[serde(default)]
    pub profiles: SupportDeclaration,
    /// Feedback support declared by the application.
    #[serde(default)]
    pub feedback: SupportDeclaration,
    /// Agent-native policy mode advertised for this command surface.
    #[serde(default)]
    pub policy: AgentPolicy,
}

impl AgentContext {
    /// Creates an empty context for a package using the current schema version.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::{AgentContext, ORTHO_AGENT_CONTEXT_SCHEMA_VERSION};
    ///
    /// let context = AgentContext::new("example-cli");
    /// assert_eq!(context.schema_version, ORTHO_AGENT_CONTEXT_SCHEMA_VERSION);
    /// assert_eq!(context.package, "example-cli");
    /// assert!(context.commands.is_empty());
    /// ```
    #[must_use]
    pub fn new(package: impl Into<String>) -> Self {
        let package_name = package.into();
        Self {
            schema_version: ORTHO_AGENT_CONTEXT_SCHEMA_VERSION.to_owned(),
            kind: format!("{package_name}.{AGENT_CONTEXT_KIND_SUFFIX}"),
            package: package_name,
            commands: Vec::new(),
            profiles: SupportDeclaration::default(),
            feedback: SupportDeclaration::default(),
            policy: AgentPolicy::default(),
        }
    }
}

/// Metadata for one invocable command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCommand {
    /// Invocation path, for example `["cargo", "orthohelp"]`.
    pub path: Vec<String>,
    /// Concise command-selection summary when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Canonical verb when known, for example `get`, `list`, or `apply`.
    #[serde(default)]
    pub canonical_verb: Option<String>,
    /// Declared command inputs.
    #[serde(default)]
    pub inputs: Vec<AgentInput>,
    /// Machine or human output modes declared by the command.
    #[serde(default)]
    pub output_modes: Vec<String>,
    /// Whether the command can prompt or block for user interaction.
    #[serde(default)]
    pub interaction_mode: InteractionMode,
    /// Mutation boundary declared for the command.
    #[serde(default)]
    pub mutation_effect: MutationEffect,
    /// Asynchronous submission contract for commands that enqueue work.
    #[serde(default)]
    pub async_submission: Option<AsyncSubmission>,
    /// Delivery route contract for commands that send artefacts elsewhere.
    #[serde(default)]
    pub delivery_route: Option<DeliveryRoute>,
    /// Pagination contract when the command lists bounded resources.
    #[serde(default)]
    pub pagination: Option<PaginationContract>,
    /// Short examples suitable for agent prompt context.
    #[serde(default)]
    pub examples: Vec<AgentExample>,
}

/// Asynchronous submission metadata for a command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AsyncSubmission {
    /// Submission style, such as inline execution or queued work.
    pub mode: AsyncSubmissionMode,
    /// Optional noun used for submitted work, for example `job` or `run`.
    #[serde(default)]
    pub noun: Option<String>,
}

/// Supported asynchronous submission styles.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AsyncSubmissionMode {
    /// Command completes before returning to the caller.
    Inline,
    /// Command submits work and returns a handle.
    Submit,
}

/// Delivery metadata for generated or submitted artefacts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryRoute {
    /// Whether delivery is supported for this command.
    pub supported: bool,
    /// Optional delivery target name, such as `stdout`, `file`, or `remote`.
    #[serde(default)]
    pub target: Option<String>,
}

/// Metadata for one command input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentInput {
    /// Stable input name without leading flag punctuation.
    pub name: String,
    /// Long CLI flag when the input is available as an option.
    #[serde(default)]
    pub long: Option<String>,
    /// Semantic value type, for example `string`, `bool`, or `path`.
    #[serde(default)]
    pub value_type: Option<String>,
    /// Whether callers must supply the input.
    #[serde(default)]
    pub required: bool,
    /// Declared default value when present and stable.
    #[serde(default)]
    pub default: Option<String>,
    /// Allowed values for enum-like inputs.
    #[serde(default)]
    pub enum_values: Vec<String>,
}

/// Compact example command line and optional expected output mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentExample {
    /// Example command line.
    pub command: String,
    /// Output mode used by the example.
    #[serde(default)]
    pub output_mode: Option<String>,
}

/// Boolean support declaration used for optional agent-native capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SupportDeclaration {
    /// Whether the capability is supported.
    pub supported: bool,
}

/// Agent-native policy mode advertised by a command surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPolicy {
    /// `off`, `warn`, or `deny`.
    pub agent_native: PolicyMode,
}

impl Default for AgentPolicy {
    fn default() -> Self {
        Self {
            agent_native: PolicyMode::Warn,
        }
    }
}

/// Enforcement mode for agent-native policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    /// Do not run policy checks.
    Off,
    /// Emit findings without failing the command.
    Warn,
    /// Treat matching findings as hard failures.
    Deny,
}

/// Prompting or interaction behaviour for a command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteractionMode {
    /// Legacy or undeclared interaction behaviour.
    Unknown,
    /// Command does not prompt and can run unattended.
    NonInteractive,
    /// Command may require operator interaction.
    Interactive,
}

impl Default for InteractionMode {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Mutation boundary for a command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MutationEffect {
    /// Legacy or undeclared mutation behaviour.
    #[serde(rename = "unknown")]
    Unknown,
    /// Read-only command.
    #[serde(rename = "read-only")]
    ReadOnly,
    /// Command may write local or remote state.
    #[serde(rename = "write")]
    Write,
    /// Command may delete local or remote state.
    #[serde(rename = "delete")]
    Delete,
    /// Command submits asynchronous work.
    #[serde(rename = "submit")]
    Submit,
}

impl Default for MutationEffect {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Pagination metadata for list-style commands.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginationContract {
    /// Flag or field that controls result limits.
    #[serde(default)]
    pub limit_input: Option<String>,
    /// Flag or field that carries the continuation cursor.
    #[serde(default)]
    pub cursor_input: Option<String>,
}

#[cfg(test)]
mod tests;
