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

/// One resolved claim's policy context.
///
/// The configured policy, the exact command path the claim resolved against,
/// the name the claim used, and whether the surface classifies that option as
/// global travel together in every rule, so they are held as one receiver
/// rather than threaded through each helper.
struct ClaimPolicy<'a> {
    /// Configured policy and explicit exceptions.
    policy: &'a ResolvedAgentPolicy,
    /// Exact command path the claim resolved against.
    path: &'a [String],
    /// Flag or verb name the claim used.
    name: &'a str,
    /// Whether the surface classifies this option as global.
    is_global: bool,
}

impl<'a> ClaimPolicy<'a> {
    /// Returns whether the configured policy silences a claim of `kind`.
    fn is_exempt(&self, kind: ExceptionKind) -> bool {
        !self.policy.is_active() || self.policy.allows(kind, self.name, self.path)
    }

    /// Applies the shared name and value rules for one option.
    ///
    /// Glossary rules describe the *global* option of a canonical name, so
    /// they apply only when the surface says this option is the global one.
    /// Generic vocabulary rules, such as an off-policy flag spelling, are about
    /// the name the CLI offers and apply either way.
    fn option_advice(&self, usage: OptionUse<'a>) -> Option<PolicyAdvice> {
        if self.is_exempt(ExceptionKind::Flag) {
            return None;
        }
        if self.is_global {
            if let Some(concept) = concept_for_long_name(self.name) {
                return match usage {
                    OptionUse::Declaration => None,
                    OptionUse::Invocation(value) => value_advice(concept, value),
                };
            }
            if let Some(concept) = near_miss_concept(self.name) {
                return Some(self.near_miss_advice(concept));
            }
        }
        if is_canonical_flag(self.name) {
            return None;
        }
        off_policy_flag_replacement(self.name, usage.value())
            .map(|replacement| self.off_policy_advice(replacement))
    }

    /// Applies the shared rules to a resolved command verb.
    fn verb_advice(&self) -> Option<PolicyAdvice> {
        if self.is_exempt(ExceptionKind::Verb) {
            return None;
        }
        off_policy_verb_replacement(self.name).map(|replacement| PolicyAdvice {
            code: codes::POLICY_NONCANONICAL_VERB,
            option: Some(self.name.to_owned()),
            message: format!("verb '{}' is off policy; use '{replacement}'", self.name),
        })
    }

    /// Builds advice for a legacy spelling of a glossary concept.
    fn near_miss_advice(&self, concept: &GlobalOptionConcept) -> PolicyAdvice {
        let hint = replacement_hint(concept);
        let bare = self.name.trim_start_matches('-');
        PolicyAdvice {
            code: codes::POLICY_GLOBAL_OPTION_NEAR_MISS,
            option: Some(bare.to_owned()),
            message: format!("option '--{bare}' is a legacy spelling; use '{hint}'"),
        }
    }

    /// Builds advice for a documented off-policy flag spelling.
    fn off_policy_advice(&self, replacement: &str) -> PolicyAdvice {
        let bare = self.name.trim_start_matches('-');
        PolicyAdvice {
            code: codes::POLICY_NONCANONICAL_FLAG,
            option: Some(bare.to_owned()),
            message: format!("option '--{bare}' is off policy; use '{replacement}'"),
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
    is_global: bool,
) -> Option<PolicyAdvice> {
    ClaimPolicy {
        policy,
        path,
        name,
        is_global,
    }
    .option_advice(OptionUse::Declaration)
}

/// One option use inside an example, as the policy rules see it.
///
/// The four fields always travel together: a use has no meaning without the
/// command that accepts it and the surface's classification of that option, so
/// they are passed as one value rather than as parallel arguments.
pub(crate) struct UseSite<'a> {
    /// Exact command path the use resolved against.
    pub(crate) path: &'a [String],
    /// Option name the example used.
    pub(crate) name: &'a str,
    /// Value the example supplied, if any.
    pub(crate) value: Option<&'a str>,
    /// Whether the surface classifies this option as global.
    pub(crate) is_global: bool,
}

/// Returns policy advice for one option use inside an example.
///
/// `--` prefixes are normalized for the glossary lookups because policy asks
/// whether a *name* is canonical. Structural membership deliberately does not
/// normalize.
pub(crate) fn option_use_advice<'a>(
    policy: &'a ResolvedAgentPolicy,
    site: &UseSite<'a>,
) -> Option<PolicyAdvice> {
    ClaimPolicy {
        policy,
        path: site.path,
        name: site.name,
        is_global: site.is_global,
    }
    .option_advice(OptionUse::Invocation(site.value))
}

/// Returns policy advice for a resolved command verb.
pub(crate) fn verb_advice(
    policy: &ResolvedAgentPolicy,
    path: &[String],
    verb: &str,
) -> Option<PolicyAdvice> {
    ClaimPolicy {
        policy,
        path,
        name: verb,
        is_global: false,
    }
    .verb_advice()
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
