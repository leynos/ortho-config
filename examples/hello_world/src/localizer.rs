//! Demo localiser used to showcase the `Localizer` trait in action.
//!
//! The example keeps the localisation story simple by embedding a pair of
//! strings. Real applications would load Fluent resources from disk or embed
//! bundles during compilation, but surfacing a concrete `Localizer`
//! implementation here makes it obvious how consumers can plug their own
//! translation strategy into `clap` once the derive macro wires it through.

use std::collections::HashMap;

use fluent_bundle::FluentValue;
use ortho_config::{LocalizationArgs, Localizer, NoOpLocalizer};

/// Base identifier for the hello world CLI catalogue.
pub const CLI_BASE_MESSAGE_ID: &str = "hello_world.cli";
/// Identifier used for the short CLI description.
pub const CLI_ABOUT_MESSAGE_ID: &str = "hello_world.cli.about";
/// Identifier used for the long CLI description.
pub const CLI_LONG_ABOUT_MESSAGE_ID: &str = "hello_world.cli.long_about";
/// Identifier for the usage string presented on `--help`.
pub const CLI_USAGE_MESSAGE_ID: &str = "hello_world.cli.usage";
/// Identifier for the greet subcommand description.
pub const CLI_GREET_ABOUT_MESSAGE_ID: &str = "hello_world.cli.greet.about";
/// Identifier for the take-leave subcommand description.
pub const CLI_TAKE_LEAVE_ABOUT_MESSAGE_ID: &str = "hello_world.cli.take-leave.about";

/// Localiser that returns baked-in demo strings for the CLI entry points.
#[derive(Debug, Clone)]
pub struct DemoLocalizer {
    catalogue: HashMap<&'static str, &'static str>,
}

impl Default for DemoLocalizer {
    fn default() -> Self {
        let mut catalogue = HashMap::new();
        catalogue.insert(
            CLI_ABOUT_MESSAGE_ID,
            "Friendly greeting demo showcasing localised help",
        );
        catalogue.insert(
            CLI_LONG_ABOUT_MESSAGE_ID,
            "Use {binary} to explore layered greetings across config, env, and CLI",
        );
        catalogue.insert(
            CLI_USAGE_MESSAGE_ID,
            "Usage:\n  {binary} [OPTIONS] <COMMAND>\n\nVisit --help for details.",
        );
        catalogue.insert(
            CLI_GREET_ABOUT_MESSAGE_ID,
            "Prints a friendly greeting using any configured templates.",
        );
        catalogue.insert(
            CLI_TAKE_LEAVE_ABOUT_MESSAGE_ID,
            "Describes how the farewell workflow proceeds for the recipient.",
        );
        Self { catalogue }
    }
}

impl DemoLocalizer {
    /// Builds a demo localiser with the shipped catalogue.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a no-op localiser for callers that do not ship translations yet.
    #[must_use]
    pub const fn noop() -> NoOpLocalizer {
        NoOpLocalizer::new()
    }
}

impl Localizer for DemoLocalizer {
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        let template = self.catalogue.get(id)?;
        Some(args.map_or_else(
            || (*template).to_owned(),
            |values| render_template_with_args(template, values),
        ))
    }
}

fn render_template_with_args(template: &str, values: &LocalizationArgs<'_>) -> String {
    let mut rendered = template.to_owned();
    for (key, value) in values {
        let FluentValue::String(text) = value else {
            continue;
        };
        let placeholder = format!("{{{key}}}");
        rendered = rendered.replace(&placeholder, text);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn demo_localiser_returns_copy() {
        let localiser = DemoLocalizer::default();
        let about = localiser
            .lookup(CLI_ABOUT_MESSAGE_ID, None)
            .expect("demo copy should exist");
        assert!(about.contains("demo"));
    }

    #[test]
    fn demo_localiser_formats_long_description() {
        let localiser = DemoLocalizer::default();
        let mut args: LocalizationArgs<'_> = HashMap::new();
        args.insert("binary", "hello-world".into());
        let long_about = localiser
            .lookup(CLI_LONG_ABOUT_MESSAGE_ID, Some(&args))
            .expect("long description should exist");
        assert!(long_about.contains("hello-world"));
    }
}
