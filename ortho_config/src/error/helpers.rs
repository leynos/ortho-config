//! Helpers for classifying and routing external errors.

use clap::{Error as ClapError, error::ErrorKind};

/// Returns `true` when a [`clap::Error`] corresponds to `--help` or
/// `--version`.
///
/// Clap surfaces these requests via specialised [`ErrorKind`] variants so
/// entry points can delegate to [`clap::Error::exit`] and preserve the
/// expected zero exit status. Applications frequently need this inspection
/// when they prefer `Cli::try_parse()` over `Cli::parse()` to keep full
/// control over diagnostics and logging.
#[must_use]
pub fn is_display_request(err: &ClapError) -> bool {
    matches!(
        err.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    )
}
