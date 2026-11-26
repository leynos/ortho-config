//! Candidate and outcome types for agent-context summary-locale selection.
//!
//! Keeping the data types in their own module lets the planning module stay a
//! set of pure functions and the selection module own construction. The
//! outcome records the chosen localizer, every skipped candidate, and the
//! invalid explicit values, so diagnostics can be emitted at the boundary
//! without the policy functions performing I/O or logging.

use ortho_config::{FluentLocalizer, LanguageIdentifier};

/// Where an agent-context summary locale candidate came from.
///
/// The label is emitted as the `source` field of the selection tracing event,
/// so operators can tell a requested locale from the historical `en-US`
/// fallback without inferring it from the locale tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryLocaleSource {
    /// The first parseable `--locale` value supplied on the command line.
    Requested,
    /// The first parseable locale declared in `[package.metadata.ortho_config]`.
    Metadata,
    /// The fixed `en-US` fallback appended after the preferred locale.
    EnUs,
    /// A discovered locale sharing an earlier candidate's language.
    SameLanguage,
    /// The embedded defaults used when no package locale could be built.
    EmbeddedDefaults,
}

impl SummaryLocaleSource {
    /// Stable label used in the `source` tracing field.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Metadata => "metadata",
            Self::EnUs => "en-US",
            Self::SameLanguage => "same-language",
            Self::EmbeddedDefaults => "embedded-defaults",
        }
    }
}

/// One ordered locale candidate for agent-context command summaries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryLocaleCandidate {
    /// Candidate locale.
    pub locale: LanguageIdentifier,
    /// Why this candidate is in the list.
    pub source: SummaryLocaleSource,
}

/// Why a candidate was skipped during selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryLocaleSkipKind {
    /// The locale directory exists but holds no `.ftl` resources.
    NoResources,
    /// Reading or building the candidate's resources failed.
    Failed,
}

/// A candidate that selection could not use, with the reason it was skipped.
#[derive(Debug, Clone)]
pub struct SummaryLocaleSkip {
    /// Candidate that was skipped.
    pub candidate: SummaryLocaleCandidate,
    /// Whether the skip was an empty directory or a failure.
    pub kind: SummaryLocaleSkipKind,
    /// Human-readable reason, preserved for the boundary's warning.
    pub reason: String,
}

/// An explicit locale value that could not be parsed.
#[derive(Debug, Clone)]
pub struct InvalidSummaryLocale {
    /// Original CLI or metadata value.
    pub value: String,
    /// Parser diagnostic.
    pub reason: String,
}

/// The ordered preference lists and invalid values resolved from the inputs.
#[derive(Debug, Clone)]
pub struct SummaryPreferences {
    /// Preferred candidates: the requested or metadata locale, then `en-US`.
    pub candidates: Vec<SummaryLocaleCandidate>,
    /// Explicit values that could not be parsed, in input order.
    pub invalid: Vec<InvalidSummaryLocale>,
}

/// A summary localizer chosen for agent-context output.
pub struct SummaryLocalizer {
    /// Locale whose catalogue was selected.
    pub locale: LanguageIdentifier,
    /// Why this locale was selected.
    pub source: SummaryLocaleSource,
    /// Localizer built for that catalogue.
    pub localizer: FluentLocalizer,
    /// Whether the localizer was built from package resources.
    ///
    /// Only such a localizer may stand in for human-document localization.
    /// The embedded-defaults fallback exists for summaries alone: reusing it
    /// would bypass a requested locale's missing or malformed catalogue and
    /// silently generate human documentation from library defaults.
    pub has_package_resources: bool,
}

impl SummaryLocalizer {
    /// Reports whether this localizer may localize `locale` for human output.
    ///
    /// Reuse requires both an exact identifier match and a localizer built
    /// from the package's own resources, so an `en-US` embedded fallback never
    /// masks a broken `en-US` catalogue.
    pub fn is_reusable_for(&self, locale: &LanguageIdentifier) -> bool {
        self.has_package_resources && &self.locale == locale
    }
}

/// The complete outcome of one summary-locale selection.
pub struct SummarySelection {
    /// The selected localizer, absent only when even embedded defaults failed.
    pub selected: Option<SummaryLocalizer>,
    /// Preferred candidates, in order, before same-language fallbacks.
    pub preferences: Vec<SummaryLocaleCandidate>,
    /// Candidates examined and skipped, in examination order.
    pub skipped: Vec<SummaryLocaleSkip>,
    /// Discovery failure, when the `locales/` tree could not be read.
    pub discovery_error: Option<String>,
    /// Explicit values that could not be parsed.
    pub invalid: Vec<InvalidSummaryLocale>,
}
