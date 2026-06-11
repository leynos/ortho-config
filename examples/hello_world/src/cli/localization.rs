//! Localisation helpers applied to the hello world CLI command tree.
//!
//! Re-exports the crate-level command-localisation trait while keeping
//! example-specific parse-error formatting in one place.

pub use ortho_config::LocalizeCmd;
use ortho_config::{Localizer, localize_clap_error_with_command};

pub fn localize_parse_error(
    err: clap::Error,
    localizer: &dyn Localizer,
    command: &clap::Command,
) -> clap::Error {
    localize_clap_error_with_command(err, localizer, Some(command))
}
