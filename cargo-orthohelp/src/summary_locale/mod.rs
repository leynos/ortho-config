//! Summary-locale selection for agent-context command summaries.
//!
//! Agent context is one non-locale-scoped document, but its command summaries
//! still come from a Fluent catalogue. This module chooses that catalogue: the
//! first requested or metadata locale, then `en-US`, then a discovered locale
//! matching an earlier candidate's language, and finally the embedded `en-US`
//! defaults. Selection is best-effort because a summary is cosmetic; a failed
//! candidate is recorded in the outcome and the next one is tried.
//!
//! Planning ([`plan`]) is pure and diagnostics ([`diagnostics`]) are emitted by
//! the caller from the returned [`SummarySelection`]. The selection module
//! itself performs resource I/O without logging, so each concern stays
//! testable on its own.

mod candidate;
mod diagnostics;
mod plan;

pub use candidate::{
    SummaryLocaleCandidate, SummaryLocaleSkip, SummaryLocaleSkipKind, SummaryLocaleSource,
    SummaryLocalizer, SummarySelection,
};
pub(crate) use plan::{same_language_summary_locales, summary_locale_preferences};

use crate::cli::Args;
use crate::locale;
use crate::metadata::PackageSelection;

/// Selects the summary localizer for a run that needs agent-context output.
///
/// Candidates are tried in preference order. A candidate counts only when the
/// package ships resources for it and those resources build; the first such
/// candidate wins. When none qualifies, the embedded `en-US` defaults are
/// tried so a package without catalogues keeps its summaries and output is
/// unchanged from earlier releases. Every skipped candidate and every invalid
/// explicit value is recorded in the outcome for the caller to report.
pub fn select_summary_localizer(args: &Args, selection: &PackageSelection) -> SummarySelection {
    let preferences = summary_locale_preferences(
        &args.locale,
        selection.locales.as_deref().unwrap_or_default(),
    );
    let (fallbacks, discovery_error) =
        match same_language_summary_locales(&selection.package_root, &preferences.candidates) {
            Ok(fallbacks) => (fallbacks, None),
            Err(error) => (Vec::new(), Some(error.to_string())),
        };
    let mut skipped = Vec::new();
    let mut selected = None;
    'candidates: for candidate in preferences.candidates.iter().chain(fallbacks.iter()) {
        match locale::load_consumer_resources(&selection.package_root, &candidate.locale) {
            Ok(resources) if resources.is_empty() => skipped.push(SummaryLocaleSkip {
                candidate: candidate.clone(),
                kind: SummaryLocaleSkipKind::NoResources,
                reason: "no Fluent resources".to_owned(),
            }),
            Ok(resources) => match locale::build_localizer(&candidate.locale, resources) {
                Ok(localizer) => {
                    selected = Some(SummaryLocalizer {
                        locale: candidate.locale.clone(),
                        source: candidate.source,
                        localizer,
                        has_package_resources: true,
                    });
                    break 'candidates;
                }
                Err(error) => skipped.push(failed_candidate(candidate, &error)),
            },
            Err(error) => skipped.push(failed_candidate(candidate, &error)),
        }
    }
    if selected.is_none() {
        selected = build_embedded_default_localizer(&mut skipped);
    }
    SummarySelection {
        selected,
        preferences: preferences.candidates,
        skipped,
        discovery_error,
        invalid: preferences.invalid,
    }
}

/// Records a candidate whose resources could not be loaded or built.
fn failed_candidate(
    candidate: &SummaryLocaleCandidate,
    error: &crate::error::OrthohelpError,
) -> SummaryLocaleSkip {
    SummaryLocaleSkip {
        candidate: candidate.clone(),
        kind: SummaryLocaleSkipKind::Failed,
        reason: error.to_string(),
    }
}

/// Builds the embedded `en-US` fallback, recording a failure in `skipped`.
fn build_embedded_default_localizer(
    skipped: &mut Vec<SummaryLocaleSkip>,
) -> Option<SummaryLocalizer> {
    let candidate = SummaryLocaleCandidate {
        locale: plan::en_us_identifier(),
        source: SummaryLocaleSource::EmbeddedDefaults,
    };
    match locale::build_localizer(&candidate.locale, Vec::new()) {
        Ok(localizer) => Some(SummaryLocalizer {
            locale: candidate.locale,
            source: SummaryLocaleSource::EmbeddedDefaults,
            localizer,
            has_package_resources: false,
        }),
        Err(error) => {
            skipped.push(failed_candidate(&candidate, &error));
            None
        }
    }
}

/// Reports the selection outcome through structured diagnostics.
///
/// Called at the generation boundary so the policy functions stay free of
/// logging. Invalid values, skipped candidates, a discovery failure, the
/// chosen locale and its source, and total omission are all reported here.
pub fn log_summary_selection(selection: &SummarySelection) {
    diagnostics::log_selection(selection);
}

#[cfg(test)]
#[path = "../summary_locale_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../summary_locale_proptests.rs"]
mod proptests;

#[cfg(test)]
#[path = "../summary_locale_warning_tests.rs"]
mod warning_tests;
