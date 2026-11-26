//! Structured diagnostics for agent-context summary-locale selection.
//!
//! Every event names the locale and, where relevant, the candidate's source, so
//! an operator can reconstruct the decision from stderr alone. The helpers live
//! in their own module and consume a [`SummarySelection`], keeping selection
//! policy free of logging.

use super::candidate::{
    InvalidSummaryLocale, SummaryLocaleCandidate, SummaryLocaleSkip, SummaryLocaleSkipKind,
    SummaryLocaleSource, SummaryLocalizer, SummarySelection,
};

/// Reports one complete selection outcome.
pub(super) fn log_selection(selection: &SummarySelection) {
    for invalid in &selection.invalid {
        warn_invalid_summary_locale(invalid);
    }
    if let Some(error) = &selection.discovery_error {
        warn_locale_discovery_failed(error);
    }
    for skipped in &selection.skipped {
        report_skip(skipped);
    }
    match &selection.selected {
        Some(selected) => report_selected(selected, selection),
        None => warn_no_localizer(selection),
    }
}

/// Reports one skipped candidate at the severity its kind warrants.
fn report_skip(skipped: &SummaryLocaleSkip) {
    match skipped.kind {
        SummaryLocaleSkipKind::NoResources => log_candidate_without_resources(&skipped.candidate),
        SummaryLocaleSkipKind::Failed => warn_candidate_failed(&skipped.candidate, &skipped.reason),
    }
}

/// Reports the chosen locale, its source, and any fall-through.
fn report_selected(selected: &SummaryLocalizer, selection: &SummarySelection) {
    tracing::info!(
        locale = %selected.locale,
        source = selected.source.as_str(),
        "agent-context summary locale selected",
    );
    if selected.source == SummaryLocaleSource::EmbeddedDefaults {
        warn_embedded_defaults(selected, selection);
        return;
    }
    if let Some(first) = selection
        .preferences
        .first()
        .filter(|first| first.locale != selected.locale)
    {
        tracing::warn!(
            first_preference = %first.locale,
            tried = %render_skipped(&selection.skipped),
            chosen = %selected.locale,
            "agent-context summary locale differs from the first preference",
        );
    }
}

/// Report an invalid summary-locale value without failing generation.
fn warn_invalid_summary_locale(invalid: &InvalidSummaryLocale) {
    tracing::warn!(
        locale = invalid.value,
        error = invalid.reason,
        "ignoring invalid agent-context summary locale",
    );
}

/// Report that locale discovery failed, so no same-language fallback exists.
fn warn_locale_discovery_failed(error: &str) {
    tracing::warn!(
        error = error,
        "locale discovery failed; no same-language agent-context summary locale",
    );
}

/// Report that a candidate's resources could not be loaded or built.
fn warn_candidate_failed(candidate: &SummaryLocaleCandidate, reason: &str) {
    tracing::warn!(
        locale = %candidate.locale,
        source = candidate.source.as_str(),
        error = reason,
        "agent-context summary locale candidate failed; trying the next",
    );
}

/// Report that a candidate's locale directory holds no Fluent resources.
fn log_candidate_without_resources(candidate: &SummaryLocaleCandidate) {
    tracing::debug!(
        locale = %candidate.locale,
        source = candidate.source.as_str(),
        "agent-context summary locale candidate has no Fluent resources; skipping",
    );
}

/// Report that only the embedded defaults resolved the summaries.
fn warn_embedded_defaults(selected: &SummaryLocalizer, selection: &SummarySelection) {
    tracing::warn!(
        locale = %selected.locale,
        tried = %render_skipped(&selection.skipped),
        "no package resources; agent-context summaries use embedded defaults",
    );
}

/// Report that no localizer could be built, so summaries are omitted.
fn warn_no_localizer(selection: &SummarySelection) {
    tracing::warn!(
        tried = %render_skipped(&selection.skipped),
        "no agent-context summary localizer; agent-context summaries omitted",
    );
}

/// Renders the candidates that were actually examined, not merely planned.
///
/// Selection stops at the first success, so a fallback that was never loaded
/// must not appear as tried.
fn render_skipped(skipped: &[SummaryLocaleSkip]) -> String {
    skipped
        .iter()
        .map(|entry| entry.candidate.locale.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
