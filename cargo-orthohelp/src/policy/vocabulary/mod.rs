//! Canonical agent-native vocabulary defaults.
//!
//! Roadmap item 7.1.1 asks the policy layer to provide canonical defaults for
//! command verbs and flags. `docs/agent-native-cli-design.md` §5 defines the
//! canonical list; this module is the single source of truth that the 7.1.2
//! lint rules and the agent-context verb mapper both consult, so the two
//! consumers cannot drift apart.

/// Canonical command verbs from design §5.
pub const CANONICAL_VERBS: &[&str] = &[
    "get", "list", "create", "update", "delete", "jobs", "profile", "feedback",
];

/// Canonical long flags from design §5, written with the leading `--` so the
/// list reads the way a user types them.
pub const CANONICAL_FLAGS: &[&str] = &[
    "--json",
    "--no-input",
    "--force",
    "--dry-run",
    "--limit",
    "--cursor",
    "--wait",
    "--profile",
    "--deliver",
];

/// Returns whether `verb` is one of the canonical command verbs.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::policy::vocabulary::is_canonical_verb;
///
/// assert!(is_canonical_verb("get"));
/// assert!(!is_canonical_verb("info"));
/// ```
#[must_use]
pub fn is_canonical_verb(verb: &str) -> bool {
    CANONICAL_VERBS.contains(&verb)
}

/// Returns whether `flag` is one of the canonical long flags.
///
/// The long name is accepted with or without the leading `--` prefix, so both
/// `--json` and `json` match. The normalization is the only prefix handling
/// this function performs; whitespace and other punctuation are compared
/// verbatim.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::policy::vocabulary::is_canonical_flag;
///
/// assert!(is_canonical_flag("--json"));
/// assert!(is_canonical_flag("json"));
/// assert!(!is_canonical_flag("--format"));
/// ```
#[must_use]
pub fn is_canonical_flag(flag: &str) -> bool {
    let long = flag.strip_prefix("--").unwrap_or(flag);
    CANONICAL_FLAGS
        .iter()
        .any(|canonical| canonical.strip_prefix("--") == Some(long))
}

#[cfg(test)]
mod tests;
