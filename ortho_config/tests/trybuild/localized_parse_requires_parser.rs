//! Compile-time contract proving `LocalizedParse` is not available to types
//! that do not implement `clap::Parser`.

use ortho_config::{LocalizedParse, NoOpLocalizer};

struct NotAParser;

fn main() {
    let localizer = NoOpLocalizer::new();
    let _parsed = NotAParser::try_parse_localized_from(["demo"], &localizer);
}
