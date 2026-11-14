//! Skill-manifest descriptors declared by an agent-context surface.
//!
//! Extracted from `mod.rs` so the parent module keeps its size budget while the
//! skill-manifest feature stays in one place. The public paths are unchanged:
//! the parent re-exports both types.
//!
//! These types model *that* a downstream manifest exists and *what* it claims;
//! they do not read, validate, or fetch it. Validation against the real command
//! tree belongs to `cargo-orthohelp` (roadmap 6.3.2).

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// Descriptor for a downstream skill manifest referenced by this command
/// surface.
///
/// Skill manifests live in the application's repository, not in `OrthoConfig`.
/// This descriptor records *that* a manifest exists, *which*
/// downstream-owned schema version it declares, and *which* commands and
/// flags it claims to reference. Validating those claims against the real
/// command tree is performed by `cargo-orthohelp` in a later roadmap item
/// (6.3.2); `OrthoConfig` only models the contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillManifest {
    /// Stable opaque identifier for the manifest within this
    /// [`AgentContext`](super::AgentContext). Validator findings should quote this value rather
    /// than the filesystem path so that diagnostics survive path
    /// canonicalization differences between platforms. Should be unique
    /// inside one `AgentContext`; duplicate identifiers are permitted by
    /// this schema but are expected to be diagnosed by later validation
    /// work.
    pub id: String,
    /// Filesystem path to the manifest, relative to the application's
    /// repository root.
    ///
    /// This schema version (`ORTHO_AGENT_CONTEXT_SCHEMA_VERSION = "1"`)
    /// only models local filesystem manifests. Future schema versions may
    /// describe other sources (for example container registry images or
    /// inline manifests); 6.3.1 does not commit to any such variant.
    pub path: Utf8PathBuf,
    /// Version string declared by the downstream manifest format.
    ///
    /// `OrthoConfig` treats this value as opaque. Prior art is split between
    /// non-semver strings (such as `OpenAI`'s `"v1"`) and semver-shaped
    /// strings (such as Microsoft Copilot's `"2.1"`); a single typed
    /// representation would reject one of these conventions. Downstream
    /// tools may parse the value to gate their own compatibility checks.
    pub manifest_schema_version: String,
    /// Command index entries: each entry declares one command path the
    /// manifest references, together with the flag names it depends on.
    ///
    /// Defaults to the empty vector so older agent-context payloads remain
    /// readable. An empty vector means "the manifest is present but
    /// declares no commands", which is a legitimate value for prose-only
    /// skills.
    #[serde(default)]
    pub commands: Vec<SkillCommandRef>,
}

/// One command-path-and-flags pair claimed by a skill manifest.
///
/// `path` must match [`AgentCommand::path`](super::AgentCommand::path) exactly: a sequence of
/// invocation path segments such as `["cargo", "orthohelp"]`. `flags` must
/// match [`AgentInput::long`](super::AgentInput::long) exactly: long flag names without the leading
/// `--`. These exact-match contracts let 6.3.2's validator compare
/// references against the real command tree without normalisation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillCommandRef {
    /// Invocation path. Matches [`AgentCommand::path`](super::AgentCommand::path) exactly.
    pub path: Vec<String>,
    /// Long flag names referenced by the manifest, without the leading
    /// `--`. Matches [`AgentInput::long`](super::AgentInput::long) exactly.
    #[serde(default)]
    pub flags: Vec<String>,
}
