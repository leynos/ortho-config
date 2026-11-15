//! A downstream exhaustive match over the translation outcome must not compile.
//!
//! `ClapErrorTranslation` is `#[non_exhaustive]`, so a consumer match has to
//! carry a wildcard arm; without one, adding an outcome would break every
//! released matcher. Removing the attribute would let this fixture compile,
//! which is the regression this case exists to catch.

use ortho_config::ClapErrorTranslation;

/// Names a translation outcome without a wildcard arm.
fn describe(translation: ClapErrorTranslation) -> &'static str {
    match translation {
        ClapErrorTranslation::Translated(_) => "translated",
        ClapErrorTranslation::DisplayOnly => "display only",
    }
}

fn main() {
    let _ = describe(ClapErrorTranslation::DisplayOnly);
}