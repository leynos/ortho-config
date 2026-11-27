//! Pure candidate planning and locale discovery for agent-context summaries.
//!
//! [`summary_locale_preferences`] and [`plan_same_language_candidates`] are
//! pure: they parse strings and order candidates without touching the
//! filesystem or emitting diagnostics. [`same_language_summary_locales`] is the
//! only fallible adapter; its caller decides how to report a discovery error.

use camino::Utf8Path;
use ortho_config::LanguageIdentifier;

use super::candidate::{
    InvalidSummaryLocale, SummaryLocaleCandidate, SummaryLocaleSource, SummaryPreferences,
};
use crate::error::OrthohelpError;
use crate::locale;

/// Resolves the ordered summary-locale preferences for agent context.
///
/// The first parseable `--locale` value wins. When no `--locale` is given, the
/// first parseable locale declared in `[package.metadata.ortho_config]` wins.
/// An explicit list whose values are all invalid yields no preferred candidate;
/// the historical `en-US` candidate follows, then duplicates are removed while
/// keeping the highest-preference source. Invalid values are returned rather
/// than logged so the boundary can report them.
pub fn summary_locale_preferences(
    requested: &[String],
    configured: &[String],
) -> SummaryPreferences {
    let mut invalid = Vec::new();
    let preferred = if requested.is_empty() {
        first_parseable_locale(configured, &mut invalid).map(|locale| SummaryLocaleCandidate {
            locale,
            source: SummaryLocaleSource::Metadata,
        })
    } else {
        first_parseable_locale(requested, &mut invalid).map(|locale| SummaryLocaleCandidate {
            locale,
            source: SummaryLocaleSource::Requested,
        })
    };
    let mut candidates = Vec::new();
    if let Some(candidate) = preferred {
        candidates.push(candidate);
    }
    push_unique_candidate(
        &mut candidates,
        SummaryLocaleCandidate {
            locale: en_us_identifier(),
            source: SummaryLocaleSource::EnUs,
        },
    );
    SummaryPreferences {
        candidates,
        invalid,
    }
}

/// Plans same-language fallbacks from already-discovered directory names.
///
/// The names are sorted before matching, so the result is deterministic even
/// when a caller supplies an unsorted slice. Each preferred candidate
/// contributes every parseable discovered locale sharing its language subtag,
/// in preferred order. A script (`zh-Hans`) or variant (`ca-valencia`,
/// `en-GB-oxendict`) subtag does not disqualify a name: the requirement is to
/// use any available same-language catalogue. `unic-langid` accepts `_` as a
/// separator, so `en_backup` and `en-backup` denote the same locale and
/// collapse to one candidate. Only a name that cannot parse as a language
/// identifier is skipped.
pub fn plan_same_language_candidates(
    preferred: &[SummaryLocaleCandidate],
    discovered: &[String],
) -> Vec<SummaryLocaleCandidate> {
    let mut names = discovered.iter().collect::<Vec<_>>();
    names.sort();
    let mut fallbacks: Vec<SummaryLocaleCandidate> = Vec::new();
    for candidate in preferred {
        for value in &names {
            let Ok(locale) = locale::parse_locale(value) else {
                continue;
            };
            let shares_language = locale.language == candidate.locale.language;
            if shares_language && !contains_locale(preferred, &locale) {
                push_unique_candidate(
                    &mut fallbacks,
                    SummaryLocaleCandidate {
                        locale,
                        source: SummaryLocaleSource::SameLanguage,
                    },
                );
            }
        }
    }
    fallbacks
}

/// Lists discovered locales sharing an earlier candidate's language.
///
/// # Errors
///
/// Returns the discovery error unchanged when the package's `locales/`
/// directory cannot be read. The caller decides whether to continue without
/// fallbacks; this adapter never converts an I/O failure into absence.
pub fn same_language_summary_locales(
    package_root: &Utf8Path,
    preferred: &[SummaryLocaleCandidate],
) -> Result<Vec<SummaryLocaleCandidate>, OrthohelpError> {
    let discovered = locale::discover_locale_dirs(package_root)?;
    Ok(plan_same_language_candidates(preferred, &discovered))
}

/// Parses `values` in order, collecting every invalid value.
fn first_parseable_locale(
    values: &[String],
    invalid: &mut Vec<InvalidSummaryLocale>,
) -> Option<LanguageIdentifier> {
    let mut first = None;
    for value in values {
        match locale::parse_locale(value) {
            Ok(locale) => {
                if first.is_none() {
                    first = Some(locale);
                }
            }
            Err(error) => invalid.push(InvalidSummaryLocale {
                value: value.clone(),
                reason: error.to_string(),
            }),
        }
    }
    first
}

/// Adds `candidate` unless an equal locale is already present.
fn push_unique_candidate(
    candidates: &mut Vec<SummaryLocaleCandidate>,
    candidate: SummaryLocaleCandidate,
) {
    if candidates
        .iter()
        .any(|existing| existing.locale == candidate.locale)
    {
        return;
    }
    candidates.push(candidate);
}

/// Reports whether `candidates` already contains `locale`.
fn contains_locale(candidates: &[SummaryLocaleCandidate], locale: &LanguageIdentifier) -> bool {
    candidates
        .iter()
        .any(|candidate| &candidate.locale == locale)
}

/// The fixed `en-US` identifier used as the historical summary fallback.
pub(crate) const fn en_us_identifier() -> LanguageIdentifier {
    ortho_config::langid!("en-US")
}
