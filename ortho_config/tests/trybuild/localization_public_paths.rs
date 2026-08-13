//! Trybuild fixture locking the public localization identifier paths.
//!
//! This file must compile: it references the trait and the argument id record
//! through their crate-root paths, which downstream users are expected to use.

use ortho_config::{ArgLocalizationIds, OrthoConfigLocalization};

/// Verifies the root re-exports compile for hand-written implementations.
struct Cli;

impl OrthoConfigLocalization for Cli {
    const LOCALIZATION_BASE: &'static str = "acme.cli";
    const ABOUT_ID: &'static str = "acme-cli-about";
    const LONG_ABOUT_ID: &'static str = "acme-cli-long_about";
    const USAGE_ID: &'static str = "acme-cli-usage";
    const VERSION_ID: &'static str = "acme-cli-version";
    const LONG_VERSION_ID: &'static str = "acme-cli-long_version";
    const AFTER_HELP_ID: &'static str = "acme-cli-after_help";
    const AFTER_LONG_HELP_ID: &'static str = "acme-cli-after_long_help";
    const ARG_IDS: &'static [ArgLocalizationIds] = &[ArgLocalizationIds {
        name: "output",
        help_id: "acme-cli-args-output-help",
        long_help_id: "acme-cli-args-output-long_help",
        value_name_id: "acme-cli-args-output-value_name",
    }];
}

fn main() {
    let _: &'static str = Cli::LOCALIZATION_BASE;
    let _: &'static str = Cli::ABOUT_ID;
    let _: &'static str = Cli::LONG_ABOUT_ID;
    let _: &'static str = Cli::USAGE_ID;
    let _: &'static str = Cli::VERSION_ID;
    let _: &'static str = Cli::LONG_VERSION_ID;
    let _: &'static str = Cli::AFTER_HELP_ID;
    let _: &'static str = Cli::AFTER_LONG_HELP_ID;
    let _: &'static [ArgLocalizationIds] = Cli::ARG_IDS;
}
