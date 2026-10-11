//! Canonical agent-native vocabulary defaults.
//!
//! Roadmap item 7.1.1 asks the policy layer to provide canonical defaults for
//! command verbs and flags. `docs/agent-native-cli-design.md` §5 defines the
//! canonical list; this module is the single source of truth that the 7.1.2
//! lint rules and the agent-context verb mapper both consult, so the two
//! consumers cannot drift apart.
//!
//! Every policy report advertises this list through its `vocabulary` block, so
//! [`Vocabulary::canonical`] is the one builder that both [`evaluate`] and the
//! behaviour rules use; a second copy would let the advertised surface drift
//! from the list the rules are actually checked against.
//!
//! [`evaluate`]: crate::policy::evaluate::evaluate

use crate::policy::Vocabulary;

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

impl Vocabulary {
    /// Builds the canonical vocabulary block advertised by a policy report.
    ///
    /// The returned lists are owned copies of [`CANONICAL_VERBS`] and
    /// [`CANONICAL_FLAGS`], in declaration order, so a serialized report
    /// reproduces the constants a consumer sees documented.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cargo_orthohelp::policy::Vocabulary;
    ///
    /// let vocabulary = Vocabulary::canonical();
    /// assert!(vocabulary.verbs.contains(&"get".to_owned()));
    /// assert!(vocabulary.flags.contains(&"--json".to_owned()));
    /// ```
    #[must_use]
    pub fn canonical() -> Self {
        Self {
            verbs: CANONICAL_VERBS
                .iter()
                .map(|verb| (*verb).to_owned())
                .collect(),
            flags: CANONICAL_FLAGS
                .iter()
                .map(|flag| (*flag).to_owned())
                .collect(),
        }
    }
}

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
