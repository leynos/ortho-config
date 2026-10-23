//! Configuration-owned Clap adapters and compatibility runtime re-exports.

pub use ortho_l10n::{
    FluentBundleSource, FluentLocalizer, FluentLocalizerBuilder, FluentLocalizerError, FluentValue,
    FormattingIssue, FormattingIssueReporter, LocalizationArgs, Localizer, NoOpLocalizer,
    message_id_for,
};

mod clap_command;
mod clap_error;
mod localization_ids;

pub use clap_command::{LocalizeCmd, LocalizedParse, WithBase, parse_localized_command};
pub use clap_error::{clap_error_formatter, localize_clap_error, localize_clap_error_with_command};
pub use localization_ids::{ArgLocalizationIds, OrthoConfigLocalization};

#[cfg(test)]
mod tests;
