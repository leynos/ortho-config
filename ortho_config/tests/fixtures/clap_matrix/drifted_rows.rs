//! Durable matrix-drift fixtures for the public clap-error inventory.
//!
//! The live matrix cannot hold a damaging shape: a shorter table or a repeated
//! row would fail the suite that publishes it. These tables are committed
//! stand-ins instead, so each invariant over the matrix can be shown to reject
//! the shape it names. They are deliberately independent of
//! [`CLAP_ERROR_IDS`](ortho_config::CLAP_ERROR_IDS), so a change to the live
//! table cannot quietly reshape the control that is meant to police it.

use clap::error::ErrorKind;
use ortho_config::ClapErrorTranslation::{DisplayOnly, Translated};

/// One matrix row: the kind and the translation outcome registered for it.
pub(crate) type Row = (ErrorKind, ortho_config::ClapErrorTranslation);

/// Sixteen rows: the supported matrix with `DisplayVersion` removed.
pub(crate) const MISSING_A_ROW: &[Row] = &[
    (
        ErrorKind::InvalidValue,
        Translated("clap-error-invalid-value"),
    ),
    (
        ErrorKind::UnknownArgument,
        Translated("clap-error-unknown-argument"),
    ),
    (
        ErrorKind::InvalidSubcommand,
        Translated("clap-error-invalid-subcommand"),
    ),
    (ErrorKind::NoEquals, Translated("clap-error-no-equals")),
    (
        ErrorKind::ValueValidation,
        Translated("clap-error-value-validation"),
    ),
    (
        ErrorKind::TooManyValues,
        Translated("clap-error-too-many-values"),
    ),
    (
        ErrorKind::TooFewValues,
        Translated("clap-error-too-few-values"),
    ),
    (
        ErrorKind::WrongNumberOfValues,
        Translated("clap-error-wrong-number-of-values"),
    ),
    (
        ErrorKind::ArgumentConflict,
        Translated("clap-error-argument-conflict"),
    ),
    (
        ErrorKind::MissingRequiredArgument,
        Translated("clap-error-missing-argument"),
    ),
    (
        ErrorKind::MissingSubcommand,
        Translated("clap-error-missing-subcommand"),
    ),
    (
        ErrorKind::InvalidUtf8,
        Translated("clap-error-invalid-utf8"),
    ),
    (ErrorKind::Io, Translated("clap-error-io")),
    (ErrorKind::Format, Translated("clap-error-format")),
    (ErrorKind::DisplayHelp, DisplayOnly),
    (
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        DisplayOnly,
    ),
];

/// Seventeen rows where `DisplayVersion` gave way to a second
/// `MissingSubcommand`, so the table length still matches and only the
/// duplicate-plus-omission check can catch it.
pub(crate) const DUPLICATE_PLUS_OMISSION: &[Row] = &[
    (
        ErrorKind::InvalidValue,
        Translated("clap-error-invalid-value"),
    ),
    (
        ErrorKind::UnknownArgument,
        Translated("clap-error-unknown-argument"),
    ),
    (
        ErrorKind::InvalidSubcommand,
        Translated("clap-error-invalid-subcommand"),
    ),
    (ErrorKind::NoEquals, Translated("clap-error-no-equals")),
    (
        ErrorKind::ValueValidation,
        Translated("clap-error-value-validation"),
    ),
    (
        ErrorKind::TooManyValues,
        Translated("clap-error-too-many-values"),
    ),
    (
        ErrorKind::TooFewValues,
        Translated("clap-error-too-few-values"),
    ),
    (
        ErrorKind::WrongNumberOfValues,
        Translated("clap-error-wrong-number-of-values"),
    ),
    (
        ErrorKind::ArgumentConflict,
        Translated("clap-error-argument-conflict"),
    ),
    (
        ErrorKind::MissingRequiredArgument,
        Translated("clap-error-missing-argument"),
    ),
    (
        ErrorKind::MissingSubcommand,
        Translated("clap-error-missing-subcommand"),
    ),
    (
        ErrorKind::InvalidUtf8,
        Translated("clap-error-invalid-utf8"),
    ),
    (ErrorKind::Io, Translated("clap-error-io")),
    (ErrorKind::Format, Translated("clap-error-format")),
    (ErrorKind::DisplayHelp, DisplayOnly),
    (
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        DisplayOnly,
    ),
    (
        ErrorKind::MissingSubcommand,
        Translated("clap-error-missing-subcommand"),
    ),
];
