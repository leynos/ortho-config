//! Policy advice applied to resolved skill claims.
//!
//! Policy is a separate function over claims that already resolved against
//! the command surface. `off` mode silences this module only; it can never
//! hide a structural finding. Canonical vocabulary and the global-option
//! glossary come from [`crate::policy::vocabulary`], so this module holds no
//! second copy of either list.

use crate::policy::config::{ExceptionKind, PolicyConfig, PolicyException};
use crate::policy::vocabulary::{
    global_options::{
        GlobalOptionConcept, accepted_value, concept_for_long_name, near_miss_concept,
        off_policy_flag_replacement, off_policy_verb_replacement, replacement_hint,
    },
    is_canonical_flag,
};
use crate::policy::{PolicyMode, PolicySeverity};

use super::report::codes;
use super::resolve::join_path;

/// Policy configuration resolved for one validation call.
///
/// Per-rule levels are reserved for roadmap 7.1.2; this feature honours the
/// mode and the explicit exception list, which 7.1.1 already provides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedAgentPolicy {
    /// Enforcement mode.
    pub mode: PolicyMode,
    /// Explicit project exceptions.
    pub exceptions: Vec<PolicyException>,
}

impl From<PolicyConfig> for ResolvedAgentPolicy {
    fn from(config: PolicyConfig) -> Self {
        Self {
            mode: config.mode,
            exceptions: config.exceptions,
        }
    }
}

impl ResolvedAgentPolicy {
    /// Returns whether policy advice should be evaluated at all.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.mode != PolicyMode::Off
    }

    /// Returns the severity policy findings carry under the configured mode.
    ///
    /// `off` is never reached for an emitted finding; it maps to `off` only so
    /// the function is total.
    #[must_use]
    pub const fn severity(&self) -> PolicySeverity {
        match self.mode {
            PolicyMode::Off => PolicySeverity::Off,
            PolicyMode::Warn => PolicySeverity::Warn,
            PolicyMode::Deny => PolicySeverity::Deny,
        }
    }

    /// Returns whether an exception excuses `name` on `path`.
    fn allows(&self, kind: ExceptionKind, name: &str, path: &[String]) -> bool {
        self.exceptions.iter().any(|exception| {
            exception.kind == kind
                && names_match(kind, &exception.name, name)
                && scope_matches(exception, path)
        })
    }
}

/// One piece of policy advice for a resolved claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyAdvice {
    /// Stable finding code.
    pub(crate) code: &'static str,
    /// Offending option or verb name.
    pub(crate) option: Option<String>,
    /// Human-readable diagnostic naming the canonical replacement.
    pub(crate) message: String,
}

/// How an option name appears in a claim.
///
/// The two variants carry nothing but a borrowed name, so the type is `Copy`
/// and can be passed by value without a needless clone.
#[derive(Clone, Copy)]
enum OptionUse<'a> {
    /// A manifest only declares that the option exists.
    Declaration,
    /// An example shows the option being used with an optional value.
    Invocation(Option<&'a str>),
}

impl OptionUse<'_> {
    /// Returns the value the claim supplies, if any.
    const fn value(&self) -> Option<&str> {
        match self {
            Self::Declaration => None,
            Self::Invocation(value) => *value,
        }
    }
}

/// Returns policy advice for a flag a manifest merely declares.
///
/// A declaration makes no claim about a value, so the value shape of a
/// glossary option is deliberately not checked here. Reporting a missing
/// value for every declared `--color` would be advice the manifest never
/// contradicted.
pub(crate) fn flag_name_advice(
    policy: &ResolvedAgentPolicy,
    path: &[String],
    name: &str,
) -> Option<PolicyAdvice> {
    advice(policy, path, name, OptionUse::Declaration)
}

/// Returns policy advice for one option use inside an example.
///
/// `--` prefixes are normalized for the glossary lookups because policy asks
/// whether a *name* is canonical. Structural membership deliberately does not
/// normalize.
pub(crate) fn option_use_advice(
    policy: &ResolvedAgentPolicy,
    path: &[String],
    name: &str,
    value: Option<&str>,
) -> Option<PolicyAdvice> {
    advice(policy, path, name, OptionUse::Invocation(value))
}

/// Applies the shared name and value rules for one option.
fn advice(
    policy: &ResolvedAgentPolicy,
    path: &[String],
    name: &str,
    usage: OptionUse<'_>,
) -> Option<PolicyAdvice> {
    if !policy.is_active() || policy.allows(ExceptionKind::Flag, name, path) {
        return None;
    }
    if let Some(concept) = concept_for_long_name(name) {
        return match usage {
            OptionUse::Declaration => None,
            OptionUse::Invocation(value) => value_advice(concept, value),
        };
    }
    if let Some(concept) = near_miss_concept(name) {
        return Some(near_miss_advice(concept, name));
    }
    if is_canonical_flag(name) {
        return None;
    }
    off_policy_flag_replacement(name, usage.value())
        .map(|replacement| off_policy_advice(name, replacement))
}

/// Returns policy advice for a resolved command verb.
pub(crate) fn verb_advice(
    policy: &ResolvedAgentPolicy,
    path: &[String],
    verb: &str,
) -> Option<PolicyAdvice> {
    if !policy.is_active() || policy.allows(ExceptionKind::Verb, verb, path) {
        return None;
    }
    off_policy_verb_replacement(verb).map(|replacement| PolicyAdvice {
        code: codes::POLICY_NONCANONICAL_VERB,
        option: Some(verb.to_owned()),
        message: format!("verb '{verb}' is off policy; use '{replacement}'"),
    })
}

/// Checks a canonical glossary option's value shape.
fn value_advice(concept: &GlobalOptionConcept, value: Option<&str>) -> Option<PolicyAdvice> {
    if concept.accepted_values.is_empty() {
        return None;
    }
    if value.is_some_and(|candidate| accepted_value(concept, candidate)) {
        return None;
    }
    let hint = replacement_hint(concept);
    Some(PolicyAdvice {
        code: codes::POLICY_GLOBAL_OPTION_VALUE,
        option: Some(concept.canonical.trim_start_matches('-').to_owned()),
        message: format!("option '{}' must be used as '{hint}'", concept.canonical),
    })
}

/// Builds advice for a legacy spelling of a glossary concept.
fn near_miss_advice(concept: &GlobalOptionConcept, name: &str) -> PolicyAdvice {
    let hint = replacement_hint(concept);
    let bare = name.trim_start_matches('-');
    PolicyAdvice {
        code: codes::POLICY_GLOBAL_OPTION_NEAR_MISS,
        option: Some(bare.to_owned()),
        message: format!("option '--{bare}' is a legacy spelling; use '{hint}'"),
    }
}

/// Builds advice for a documented off-policy flag spelling.
fn off_policy_advice(name: &str, replacement: &str) -> PolicyAdvice {
    let bare = name.trim_start_matches('-');
    PolicyAdvice {
        code: codes::POLICY_NONCANONICAL_FLAG,
        option: Some(bare.to_owned()),
        message: format!("option '--{bare}' is off policy; use '{replacement}'"),
    }
}

/// Returns whether an exception name matches the observed name.
fn names_match(kind: ExceptionKind, exception_name: &str, observed: &str) -> bool {
    match kind {
        ExceptionKind::Flag => {
            exception_name.trim_start_matches('-') == observed.trim_start_matches('-')
        }
        ExceptionKind::Verb => exception_name == observed,
    }
}

/// Returns whether an exception's scope covers the observed command path.
fn scope_matches(exception: &PolicyException, path: &[String]) -> bool {
    exception
        .command_path
        .as_deref()
        .is_none_or(|scope| scope == join_path(path))
}
