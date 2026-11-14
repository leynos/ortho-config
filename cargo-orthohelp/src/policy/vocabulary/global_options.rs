//! Canonical human-facing global option glossary.
//!
//! `docs/agent-native-cli-design.md` §5 names the concepts every agent-native
//! CLI should express with one long option, the values that option accepts,
//! and the legacy spellings that mean the same thing. Roadmap 7.1.3 owns
//! wiring these rules into the bridge-IR policy pass; this module is the
//! shared data contract, so the skill validator (6.3.2) and that later pass
//! cannot disagree about the glossary.
//!
//! The [`Vocabulary`] wire struct is deliberately not extended. Adding a
//! field would change the policy-report schema, and the glossary is a
//! build-time lookup rather than something a consumer needs on the wire.
//!
//! [`Vocabulary`]: crate::policy::Vocabulary

/// One canonical global option concept from design §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalOptionConcept {
    /// Stable concept identifier, for example `colour`.
    pub concept: &'static str,
    /// Canonical long option name including the leading `--`.
    pub canonical: &'static str,
    /// Values the canonical option accepts; empty for a boolean option.
    pub accepted_values: &'static [&'static str],
    /// Legacy or malformed spellings that mean this concept.
    pub near_misses: &'static [&'static str],
}

/// The canonical global option concepts from design §5.
///
/// A project may omit a concept it does not support; omission is not a
/// finding. A concept that is present must use the canonical name and, where
/// listed, one of the accepted values.
pub const GLOBAL_OPTION_CONCEPTS: &[GlobalOptionConcept] = &[
    GlobalOptionConcept {
        concept: "structured-output",
        canonical: "--json",
        accepted_values: &[],
        near_misses: &["--output-format", "--diag-json", "--json-output"],
    },
    GlobalOptionConcept {
        concept: "plain-output",
        canonical: "--plain",
        accepted_values: &[],
        near_misses: &["--text", "--plain-output"],
    },
    GlobalOptionConcept {
        concept: "colour",
        canonical: "--color",
        accepted_values: &["auto", "always", "never"],
        near_misses: &["--colour", "--colour-policy", "--color-policy"],
    },
    GlobalOptionConcept {
        concept: "emoji",
        canonical: "--emoji",
        accepted_values: &["auto", "always", "never"],
        near_misses: &["--no-emoji", "--emoji-policy"],
    },
    GlobalOptionConcept {
        concept: "progress",
        canonical: "--progress",
        accepted_values: &["auto", "always", "never"],
        near_misses: &["--progress-policy"],
    },
    GlobalOptionConcept {
        concept: "accessibility",
        canonical: "--accessibility",
        accepted_values: &["auto", "on", "off"],
        near_misses: &["--accessible", "--accessibility-mode"],
    },
    GlobalOptionConcept {
        concept: "pager",
        canonical: "--no-pager",
        accepted_values: &[],
        near_misses: &["--no-paging", "--disable-pager"],
    },
    GlobalOptionConcept {
        concept: "width",
        canonical: "--width",
        accepted_values: &[],
        near_misses: &["--columns", "--terminal-width"],
    },
    GlobalOptionConcept {
        concept: "locale",
        canonical: "--locale",
        accepted_values: &[],
        near_misses: &["--lang", "--language"],
    },
    GlobalOptionConcept {
        concept: "quiet",
        canonical: "--quiet",
        accepted_values: &[],
        near_misses: &["--silent"],
    },
    GlobalOptionConcept {
        concept: "verbose",
        canonical: "--verbose",
        accepted_values: &[],
        near_misses: &[],
    },
];

/// Off-policy flag spellings from design §5 and their canonical replacement.
///
/// `None` in the first column means the replacement depends on the option's
/// value, so [`off_policy_flag_replacement`] applies a value-sensitive rule.
const OFF_POLICY_FLAGS: &[(&str, Option<&str>)] = &[
    ("--format", None),
    ("--output", None),
    ("--skip-confirmations", Some("--no-input")),
];

/// Off-policy command verbs from design §5 and their canonical replacement.
const OFF_POLICY_VERBS: &[(&str, &str)] = &[("info", "get"), ("ls", "list")];

/// Strips one optional leading `--` so a caller may pass either spelling.
fn bare_name(name: &str) -> &str {
    name.strip_prefix("--").unwrap_or(name)
}

/// Returns the concept whose canonical name matches `name`.
///
/// The comparison accepts the name with or without the leading `--`.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::policy::vocabulary::global_options::concept_for_long_name;
///
/// assert_eq!(concept_for_long_name("--color").map(|c| c.concept), Some("colour"));
/// assert!(concept_for_long_name("--format").is_none());
/// ```
#[must_use]
pub fn concept_for_long_name(name: &str) -> Option<&'static GlobalOptionConcept> {
    let wanted = bare_name(name);
    GLOBAL_OPTION_CONCEPTS
        .iter()
        .find(|concept| bare_name(concept.canonical) == wanted)
}

/// Returns the concept whose near-miss list contains `name`.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::policy::vocabulary::global_options::near_miss_concept;
///
/// assert_eq!(
///     near_miss_concept("--colour-policy").map(|c| c.canonical),
///     Some("--color")
/// );
/// assert!(near_miss_concept("--color").is_none());
/// ```
#[must_use]
pub fn near_miss_concept(name: &str) -> Option<&'static GlobalOptionConcept> {
    let wanted = bare_name(name);
    GLOBAL_OPTION_CONCEPTS.iter().find(|concept| {
        concept
            .near_misses
            .iter()
            .any(|near_miss| bare_name(near_miss) == wanted)
    })
}

/// Returns whether `value` is one of a concept's accepted values.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::policy::vocabulary::global_options::{
///     accepted_value, concept_for_long_name,
/// };
///
/// let colour = concept_for_long_name("--color").expect("colour concept");
/// assert!(accepted_value(colour, "always"));
/// assert!(!accepted_value(colour, "sometimes"));
/// ```
#[must_use]
pub fn accepted_value(concept: &GlobalOptionConcept, value: &str) -> bool {
    concept.accepted_values.contains(&value)
}

/// Returns the canonical replacement for an off-policy flag spelling.
///
/// `--format` and `--output` are value-sensitive: only the `json` and `text`
/// spellings from design §5 have a canonical replacement, and other values
/// are left to domain policy.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::policy::vocabulary::global_options::off_policy_flag_replacement;
///
/// assert_eq!(off_policy_flag_replacement("--format", Some("json")), Some("--json"));
/// assert_eq!(off_policy_flag_replacement("--format", Some("text")), Some("--plain"));
/// assert_eq!(off_policy_flag_replacement("--format", Some("yaml")), None);
/// assert_eq!(
///     off_policy_flag_replacement("--skip-confirmations", None),
///     Some("--no-input")
/// );
/// ```
#[must_use]
pub fn off_policy_flag_replacement(name: &str, value: Option<&str>) -> Option<&'static str> {
    let wanted = bare_name(name);
    OFF_POLICY_FLAGS
        .iter()
        .find(|(flag, _)| bare_name(flag) == wanted)
        .and_then(|(_, replacement)| {
            replacement.or_else(|| match (wanted, value) {
                ("format" | "output", Some("json")) => Some("--json"),
                ("format" | "output", Some("text")) => Some("--plain"),
                _ => None,
            })
        })
}

/// Returns the canonical replacement for an off-policy command verb.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::policy::vocabulary::global_options::off_policy_verb_replacement;
///
/// assert_eq!(off_policy_verb_replacement("info"), Some("get"));
/// assert_eq!(off_policy_verb_replacement("list"), None);
/// ```
#[must_use]
pub fn off_policy_verb_replacement(verb: &str) -> Option<&'static str> {
    OFF_POLICY_VERBS
        .iter()
        .find(|(candidate, _)| *candidate == verb)
        .map(|(_, replacement)| *replacement)
}

/// Builds the diagnostic hint for a concept, including accepted values.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::policy::vocabulary::global_options::{
///     concept_for_long_name, replacement_hint,
/// };
///
/// let colour = concept_for_long_name("--color").expect("colour concept");
/// assert_eq!(replacement_hint(colour), "--color auto|always|never");
/// ```
#[must_use]
pub fn replacement_hint(concept: &GlobalOptionConcept) -> String {
    if concept.accepted_values.is_empty() {
        concept.canonical.to_owned()
    } else {
        format!(
            "{} {}",
            concept.canonical,
            concept.accepted_values.join("|")
        )
    }
}
