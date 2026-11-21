//! Canonical matrix of clap error kinds and their translation outcome.
//!
//! `clap::error::ErrorKind` is `#[non_exhaustive]`, so a clap release can add a
//! variant without warning. This module publishes, in one place, which kinds
//! this release translates and which clap renders itself, and the localization
//! pipeline consults the same table. Consumers iterate it to build coverage
//! tests or translator briefs instead of hard-coding identifiers.

use clap::error::ErrorKind;

/// Outcome the crate has registered for a clap [`ErrorKind`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClapErrorTranslation {
    /// The crate ships an en-US translation under this Fluent identifier and
    /// consumer overrides should target it.
    Translated(&'static str),
    /// clap renders the surface itself (help text, version text). The
    /// localization pipeline must not substitute a translated body.
    DisplayOnly,
}

/// Every [`ErrorKind`] this release classifies, in review order.
///
/// The four identifiers that predate this matrix keep their historical
/// spelling, so consumer overrides written against earlier releases keep
/// resolving. The three render-it-yourself kinds carry
/// [`ClapErrorTranslation::DisplayOnly`] and are never looked up.
///
/// # Examples
///
/// ```rust
/// use ortho_config::{CLAP_ERROR_IDS, ClapErrorTranslation};
///
/// let translated = CLAP_ERROR_IDS
///     .iter()
///     .filter(|(_, outcome)| matches!(outcome, ClapErrorTranslation::Translated(_)))
///     .count();
/// assert_eq!(translated, 14);
/// ```
pub const CLAP_ERROR_IDS: &[(ErrorKind, ClapErrorTranslation)] = &[
    (
        ErrorKind::InvalidValue,
        ClapErrorTranslation::Translated("clap-error-invalid-value"),
    ),
    (
        ErrorKind::UnknownArgument,
        ClapErrorTranslation::Translated("clap-error-unknown-argument"),
    ),
    (
        ErrorKind::InvalidSubcommand,
        ClapErrorTranslation::Translated("clap-error-invalid-subcommand"),
    ),
    (
        ErrorKind::NoEquals,
        ClapErrorTranslation::Translated("clap-error-no-equals"),
    ),
    (
        ErrorKind::ValueValidation,
        ClapErrorTranslation::Translated("clap-error-value-validation"),
    ),
    (
        ErrorKind::TooManyValues,
        ClapErrorTranslation::Translated("clap-error-too-many-values"),
    ),
    (
        ErrorKind::TooFewValues,
        ClapErrorTranslation::Translated("clap-error-too-few-values"),
    ),
    (
        ErrorKind::WrongNumberOfValues,
        ClapErrorTranslation::Translated("clap-error-wrong-number-of-values"),
    ),
    (
        ErrorKind::ArgumentConflict,
        ClapErrorTranslation::Translated("clap-error-argument-conflict"),
    ),
    (
        ErrorKind::MissingRequiredArgument,
        ClapErrorTranslation::Translated("clap-error-missing-argument"),
    ),
    (
        ErrorKind::MissingSubcommand,
        ClapErrorTranslation::Translated("clap-error-missing-subcommand"),
    ),
    (
        ErrorKind::InvalidUtf8,
        ClapErrorTranslation::Translated("clap-error-invalid-utf8"),
    ),
    (
        ErrorKind::Io,
        ClapErrorTranslation::Translated("clap-error-io"),
    ),
    (
        ErrorKind::Format,
        ClapErrorTranslation::Translated("clap-error-format"),
    ),
    (ErrorKind::DisplayHelp, ClapErrorTranslation::DisplayOnly),
    (
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        ClapErrorTranslation::DisplayOnly,
    ),
    (ErrorKind::DisplayVersion, ClapErrorTranslation::DisplayOnly),
];

/// Resolves the outcome registered for `kind`.
///
/// Returns `None` for a kind this matrix predates. The caller keeps the
/// pre-matrix derived `clap-error-<kebab-kind>` identifier in that case, so an
/// unmapped kind still tries a translation instead of silently passing through.
pub(super) fn translation_for(kind: ErrorKind) -> Option<ClapErrorTranslation> {
    CLAP_ERROR_IDS
        .iter()
        .find(|(listed, _)| *listed == kind)
        .map(|(_, outcome)| *outcome)
}
