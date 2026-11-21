//! Independent inventory coverage for the public clap-error matrix.
//!
//! These cases are enumerated by hand from the supported `clap_builder`
//! identity rather than derived from [`CLAP_ERROR_IDS`], so a matrix row that
//! is missing, duplicated, or reclassified fails here even when the table
//! length is unchanged. `CLAP_ERROR_KIND_COUNT` is the reviewed total for that
//! identity; the pre-publication gate cross-checks it against the resolved
//! source.

use anyhow::{Result, ensure};
use clap::error::ErrorKind;
use ortho_config::{CLAP_ERROR_IDS, ClapErrorTranslation};
use rstest::rstest;
use std::collections::BTreeSet;

#[path = "support/clap_reviewed.rs"]
mod clap_reviewed;
#[path = "fixtures/clap_matrix/drifted_rows.rs"]
mod drifted_rows;

use clap_reviewed::CLAP_ERROR_KIND_COUNT;

// The matrix and the reviewed total must move together. Failing at compile
// time means a new `clap_builder` variant cannot ship with a matrix that kept
// its old length, even if the runtime comparison below is edited away.
static_assertions::const_assert_eq!(CLAP_ERROR_IDS.len(), CLAP_ERROR_KIND_COUNT);

/// Number of matrix rows that name an embedded en-US translation.
const TRANSLATED_ROW_COUNT: usize = 14;

/// Returns the outcome the matrix registers for `kind`, if any.
fn translation_for(kind: ErrorKind) -> Option<ClapErrorTranslation> {
    CLAP_ERROR_IDS
        .iter()
        .find(|(listed, _)| *listed == kind)
        .map(|(_, translation)| *translation)
}

/// Builds the expected outcome for one review case.
fn expected_translation(identifier: Option<&'static str>) -> ClapErrorTranslation {
    identifier.map_or(
        ClapErrorTranslation::DisplayOnly,
        ClapErrorTranslation::Translated,
    )
}

/// Every supported kind appears once with its reviewed translation outcome.
///
/// The four established identifiers keep their exact historical spelling so
/// existing consumer overrides continue to resolve.
#[rstest]
#[case::invalid_value(ErrorKind::InvalidValue, Some("clap-error-invalid-value"))]
#[case::unknown_argument(ErrorKind::UnknownArgument, Some("clap-error-unknown-argument"))]
#[case::invalid_subcommand(ErrorKind::InvalidSubcommand, Some("clap-error-invalid-subcommand"))]
#[case::no_equals(ErrorKind::NoEquals, Some("clap-error-no-equals"))]
#[case::value_validation(ErrorKind::ValueValidation, Some("clap-error-value-validation"))]
#[case::too_many_values(ErrorKind::TooManyValues, Some("clap-error-too-many-values"))]
#[case::too_few_values(ErrorKind::TooFewValues, Some("clap-error-too-few-values"))]
#[case::wrong_number_of_values(
    ErrorKind::WrongNumberOfValues,
    Some("clap-error-wrong-number-of-values")
)]
#[case::argument_conflict(ErrorKind::ArgumentConflict, Some("clap-error-argument-conflict"))]
#[case::missing_required_argument(
    ErrorKind::MissingRequiredArgument,
    Some("clap-error-missing-argument")
)]
#[case::missing_subcommand(ErrorKind::MissingSubcommand, Some("clap-error-missing-subcommand"))]
#[case::invalid_utf8(ErrorKind::InvalidUtf8, Some("clap-error-invalid-utf8"))]
#[case::io(ErrorKind::Io, Some("clap-error-io"))]
#[case::format(ErrorKind::Format, Some("clap-error-format"))]
#[case::display_help(ErrorKind::DisplayHelp, None)]
#[case::display_help_on_missing_input(ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand, None)]
#[case::display_version(ErrorKind::DisplayVersion, None)]
fn reviewed_kind_has_the_reviewed_outcome(
    #[case] kind: ErrorKind,
    #[case] identifier: Option<&'static str>,
) {
    assert_eq!(
        translation_for(kind),
        Some(expected_translation(identifier)),
        "{kind:?} should carry the reviewed translation outcome"
    );
}

/// One matrix row: the kind and the outcome the matrix registers for it.
type Row = (ErrorKind, ClapErrorTranslation);

/// Fail unless `rows` has one entry per reviewed `clap_builder` variant.
fn ensure_reviewed_length(rows: &[Row], reviewed: usize) -> Result<()> {
    let listed = rows.len();
    ensure!(
        listed == reviewed,
        "the matrix lists {listed} rows but the reviewed total is {reviewed}",
    );
    Ok(())
}

/// Fail unless each kind appears at most once.
fn ensure_unique_kinds(rows: &[Row]) -> Result<()> {
    for (index, (kind, _)) in rows.iter().enumerate() {
        let repeats = rows
            .iter()
            .skip(index + 1)
            .filter(|(other, _)| other == kind)
            .count();
        ensure!(repeats == 0, "{kind:?} is listed more than once");
    }
    Ok(())
}

/// Fail unless every translated identifier is unique and namespaced.
fn ensure_identifiers_are_namespaced(rows: &[Row]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (kind, translation) in rows {
        let ClapErrorTranslation::Translated(identifier) = translation else {
            continue;
        };
        ensure!(
            identifier.starts_with("clap-error-"),
            "{kind:?} uses {identifier:?}, outside the clap-error namespace",
        );
        ensure!(
            seen.insert(*identifier),
            "{identifier:?} is used by more than one kind",
        );
    }
    let translated = seen.len();
    ensure!(
        translated == TRANSLATED_ROW_COUNT,
        "the matrix translates {translated} kinds, but {TRANSLATED_ROW_COUNT} are reviewed",
    );
    Ok(())
}

/// Fail unless exactly the three render-it-yourself kinds carry the sentinel.
fn ensure_display_only_rows(rows: &[Row]) -> Result<()> {
    let display_only: BTreeSet<String> = rows
        .iter()
        .filter(|(_, translation)| matches!(translation, ClapErrorTranslation::DisplayOnly))
        .map(|(kind, _)| format!("{kind:?}"))
        .collect();
    let expected = BTreeSet::from([
        "DisplayHelp".to_owned(),
        "DisplayHelpOnMissingArgumentOrSubcommand".to_owned(),
        "DisplayVersion".to_owned(),
    ]);

    ensure!(
        display_only == expected,
        "the display-only rows are {display_only:?}, not {expected:?}",
    );
    Ok(())
}

/// The refusal message, or an empty string when the check passed.
fn refusal(result: Result<()>) -> String {
    match result {
        Ok(()) => String::new(),
        Err(error) => error.to_string(),
    }
}

/// The matrix length equals the reviewed variant count for the identity.
#[test]
fn matrix_length_equals_the_reviewed_kind_count() -> Result<()> {
    ensure_reviewed_length(CLAP_ERROR_IDS, CLAP_ERROR_KIND_COUNT)
}

/// No kind is listed twice, so a duplicate cannot mask an omission.
#[test]
fn matrix_kinds_are_unique() -> Result<()> {
    ensure_unique_kinds(CLAP_ERROR_IDS)
}

/// Translated identifiers are unique and live in the clap-error namespace.
#[test]
fn translated_identifiers_are_unique_and_namespaced() -> Result<()> {
    ensure_identifiers_are_namespaced(CLAP_ERROR_IDS)
}

/// Exactly the three render-it-yourself kinds carry the sentinel.
#[test]
fn display_only_rows_are_the_three_render_variants() -> Result<()> {
    ensure_display_only_rows(CLAP_ERROR_IDS)
}

/// A committed sixteen-row table fails the length check, naming both counts.
///
/// The fixture is independent of the live matrix, so this control keeps
/// proving the check even if the published table changes.
#[test]
fn a_short_matrix_fails_the_length_check() -> Result<()> {
    let message = refusal(ensure_reviewed_length(
        drifted_rows::MISSING_A_ROW,
        CLAP_ERROR_KIND_COUNT,
    ));

    ensure!(
        message.contains("16 rows") && message.contains("17"),
        "{message:?} should name sixteen rows and the reviewed seventeen",
    );
    Ok(())
}

/// A duplicate that replaces an omitted kind keeps the reviewed length.
///
/// Only the uniqueness check catches this shape, so the control first proves
/// the fixture passes the length check and then requires the refusal.
#[test]
fn duplicate_plus_omission_fails_the_uniqueness_check() -> Result<()> {
    let rows = drifted_rows::DUPLICATE_PLUS_OMISSION;

    ensure_reviewed_length(rows, CLAP_ERROR_KIND_COUNT)?;
    let message = refusal(ensure_unique_kinds(rows));
    ensure!(
        message.contains("listed more than once"),
        "{message:?} should name the repeated kind",
    );
    Ok(())
}

/// The matrix is reachable through the crate root and the localizer module.
#[test]
fn matrix_is_exported_through_both_public_paths() {
    use ortho_config::localizer::{
        CLAP_ERROR_IDS as MODULE_IDS, ClapErrorTranslation as ModuleTranslation,
    };

    assert_eq!(MODULE_IDS.len(), CLAP_ERROR_IDS.len());
    for (module_row, root_row) in MODULE_IDS.iter().zip(CLAP_ERROR_IDS) {
        assert_eq!(module_row, root_row);
    }
    let module_sentinel: ModuleTranslation = ModuleTranslation::DisplayOnly;
    assert_eq!(module_sentinel, ClapErrorTranslation::DisplayOnly);
}
