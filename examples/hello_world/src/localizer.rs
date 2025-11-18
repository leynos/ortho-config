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

/// Identifier used for the short CLI description.
pub const CLI_ABOUT_MESSAGE_ID: &str = "hello_world.cli.about";
/// Identifier used for the long CLI description.
pub const CLI_LONG_ABOUT_MESSAGE_ID: &str = "hello_world.cli.long_about";

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
    fn get_message(&self, id: &str) -> Option<String> {
        self.catalogue.get(id).map(|msg| String::from(*msg))
    }

    fn get_message_with_args(
        &self,
        id: &str,
        args: Option<&LocalizationArgs<'_>>,
    ) -> Option<String> {
        if id == CLI_LONG_ABOUT_MESSAGE_ID {
            let binary = args
                .and_then(|values| values.get("binary"))
                .and_then(|value| match value {
                    FluentValue::String(text) => Some(text.to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| String::from("hello-world"));
            return Some(format!(
                "Use {binary} to explore layered greetings across files, env vars, \
                 and CLI flags.",
            ));
        }
        self.get_message(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn demo_localiser_returns_copy() {
        let localiser = DemoLocalizer::default();
        let about = localiser
            .get_message(CLI_ABOUT_MESSAGE_ID)
            .expect("demo copy should exist");
        assert!(about.contains("demo"));
    }

    #[test]
    fn demo_localiser_formats_long_description() {
        let localiser = DemoLocalizer::default();
        let mut args: LocalizationArgs<'_> = HashMap::new();
        args.insert("binary", "hello-world".into());
        let long_about = localiser
            .get_message_with_args(CLI_LONG_ABOUT_MESSAGE_ID, Some(&args))
            .expect("long description should exist");
        assert!(long_about.contains("hello-world"));
    }
}
