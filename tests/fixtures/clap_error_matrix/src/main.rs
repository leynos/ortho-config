//! Subprocess fixture for clap error rendering through an embedded locale.
//!
//! The runner injects the locale with `CLAP_ERROR_FIXTURE_LOCALE` and disables
//! colour with `NO_COLOR`, so localized and stock renderings can be compared
//! without mutating the test process's environment. Exit status and stream
//! selection come from `clap` itself, which is what the runner asserts on.

use clap::{Arg, ArgAction, ColorChoice, Command};
use ortho_config::{FluentLocalizer, LanguageIdentifier, langid, localize_clap_error};
use std::process::ExitCode;

/// The demo command tree exercised by the runner.
///
/// `arg_required_else_help` reproduces clap's own missing-input help, and
/// `require_equals` reproduces a parser error the matrix translates.
fn demo_command() -> Command {
    Command::new("clap-error-fixture")
        .version("1.2.3")
        .color(ColorChoice::Never)
        .arg_required_else_help(true)
        .arg(
            Arg::new("colour")
                .long("colour")
                .action(ArgAction::Set)
                .require_equals(true),
        )
        .subcommand(Command::new("greet").about("Greet someone"))
}

/// Resolves the injected locale, defaulting to en-US when it is absent or
/// cannot be parsed as a language identifier.
fn requested_locale() -> LanguageIdentifier {
    std::env::var("CLAP_ERROR_FIXTURE_LOCALE")
        .ok()
        .and_then(|value| value.parse::<LanguageIdentifier>().ok())
        .unwrap_or_else(|| langid!("en-US"))
}

/// Builds the localizer, or yields [`None`] when the catalogue cannot load.
///
/// An unbuildable catalogue leaves the stock clap diagnostic in place rather
/// than failing the process for a reason unrelated to the error under test.
fn build_localizer() -> Option<FluentLocalizer> {
    FluentLocalizer::builder(requested_locale())
        .try_build()
        .ok()
}

fn main() -> ExitCode {
    let localizer = build_localizer();
    let Err(error) = demo_command().try_get_matches_from(std::env::args_os()) else {
        return ExitCode::SUCCESS;
    };

    // `exit` prints through clap and terminates with clap's own status, so the
    // runner observes exactly what an unlocalized application would emit.
    match &localizer {
        Some(catalogue) => localize_clap_error(error, catalogue).exit(),
        None => error.exit(),
    }
}
