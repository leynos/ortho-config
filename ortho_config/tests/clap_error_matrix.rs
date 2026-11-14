//! Independent inventory coverage for the public clap-error matrix.
//!
//! These cases are enumerated by hand from the supported `clap_builder`
//! identity rather than derived from [`CLAP_ERROR_IDS`], so a matrix row that
//! is missing, duplicated, or reclassified fails here even when the table
//! length is unchanged. `CLAP_ERROR_KIND_COUNT` is the reviewed total for that
//! identity; the pre-publication gate cross-checks it against the resolved
//! source.

use clap::error::ErrorKind;
use googletest::prelude::*;
use ortho_config::{CLAP_ERROR_IDS, ClapErrorTranslation};
use pretty_assertions::assert_eq;
use rstest::rstest;
use std::collections::BTreeSet;

/// Total `ErrorKind` variants declared by the supported `clap_builder` 4.6.7.
const CLAP_ERROR_KIND_COUNT: usize = 17;

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
    assert_that!(
        translation_for(kind),
        eq(Some(expected_translation(identifier)))
    );
}

/// The matrix length equals the reviewed variant count for the identity.
#[test]
fn matrix_length_equals_the_reviewed_kind_count() {
    assert_eq!(CLAP_ERROR_IDS.len(), CLAP_ERROR_KIND_COUNT);
}

/// No kind is listed twice, so a duplicate cannot mask an omission.
#[test]
fn matrix_kinds_are_unique() {
    for (index, (kind, _)) in CLAP_ERROR_IDS.iter().enumerate() {
        let repeats = CLAP_ERROR_IDS
            .iter()
            .skip(index + 1)
            .filter(|(other, _)| other == kind)
            .count();
        assert_eq!(repeats, 0, "{kind:?} is listed more than once");
    }
}

/// Translated identifiers are unique and live in the clap-error namespace.
#[test]
fn translated_identifiers_are_unique_and_namespaced() {
    let mut seen = BTreeSet::new();
    for (kind, translation) in CLAP_ERROR_IDS {
        let ClapErrorTranslation::Translated(identifier) = translation else {
            continue;
        };
        assert!(
            identifier.starts_with("clap-error-"),
            "{kind:?} uses {identifier:?}, outside the clap-error namespace"
        );
        assert!(
            seen.insert(*identifier),
            "{identifier:?} is used by more than one kind"
        );
    }
    assert_eq!(seen.len(), TRANSLATED_ROW_COUNT);
}

/// Exactly the three render-it-yourself kinds carry the sentinel.
#[test]
fn display_only_rows_are_the_three_render_variants() {
    let display_only: BTreeSet<String> = CLAP_ERROR_IDS
        .iter()
        .filter(|(_, translation)| matches!(translation, ClapErrorTranslation::DisplayOnly))
        .map(|(kind, _)| format!("{kind:?}"))
        .collect();

    assert_eq!(
        display_only,
        BTreeSet::from([
            "DisplayHelp".to_owned(),
            "DisplayHelpOnMissingArgumentOrSubcommand".to_owned(),
            "DisplayVersion".to_owned(),
        ])
    );
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
