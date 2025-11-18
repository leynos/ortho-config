//! Localisation primitives shared across the workspace.
//!
//! `Localizer` abstracts string lookup so `clap` integration, examples, and
//! future tooling can localise help text without coupling to a concrete
//! translation backend. The trait is intentionally minimal: implementations
//! return owned `String` values so callers can cache the resolved text or fall
//! back to defaults supplied by `clap` when no translation exists.

use fluent_bundle::FluentValue;
use std::collections::HashMap;

/// Arguments forwarded to localisation lookups.
///
/// Implementations can inspect these values while formatting a translation. In
/// Fluent-backed scenarios the map keys correspond to the placeholder names
/// declared in the `.ftl` resources.
pub type LocalizationArgs<'value> = HashMap<&'value str, FluentValue<'value>>;

/// Provides localised strings for user-facing CLI output.
///
/// Implementations may forward lookups to Fluent bundles, embed simple maps for
/// testing, or proxy to other translation sources. Consumers invoke these
/// helpers instead of directly hardcoding strings so help text and diagnostics
/// can be translated consistently. The trait is object-safe, allowing
/// applications to store it behind `Arc<dyn Localizer>` and thread it through
/// builders at runtime.
pub trait Localizer: Send + Sync {
    /// Returns the message for the requested identifier.
    fn get_message(&self, id: &str) -> Option<String>;

    /// Returns the message for the identifier, formatting it with optional
    /// arguments when available.
    fn get_message_with_args(
        &self,
        id: &str,
        args: Option<&LocalizationArgs<'_>>,
    ) -> Option<String> {
        let _ = args;
        self.get_message(id)
    }

    /// Resolves the message, falling back to the provided string when no
    /// translation exists.
    fn message_or(&self, id: &str, fallback: &str) -> String {
        self.message_with_args_or(id, None, fallback)
    }

    /// Resolves the message with arguments, falling back to a default when no
    /// translation exists.
    fn message_with_args_or(
        &self,
        id: &str,
        args: Option<&LocalizationArgs<'_>>,
        fallback: &str,
    ) -> String {
        self.get_message_with_args(id, args)
            .unwrap_or_else(|| fallback.to_owned())
    }
}

/// Default localiser that declines to translate messages.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpLocalizer;

impl NoOpLocalizer {
    /// Creates a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Localizer for NoOpLocalizer {
    fn get_message(&self, _id: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::collections::HashMap;

    #[rstest]
    fn noop_localizer_relies_on_fallback() {
        let localizer = NoOpLocalizer::new();
        let resolved = localizer.message_or("cli.about", "fallback");
        assert_eq!(resolved, "fallback");
    }

    struct StubLocalizer;

    impl Localizer for StubLocalizer {
        fn get_message(&self, id: &str) -> Option<String> {
            Some(format!("{id}:no-args"))
        }

        fn get_message_with_args(
            &self,
            id: &str,
            args: Option<&LocalizationArgs<'_>>,
        ) -> Option<String> {
            let values = args?;
            let subject = values
                .get("subject")
                .and_then(|value| match value {
                    FluentValue::String(text) => Some(text.to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| String::from("<missing>"));
            Some(format!("{id}:{subject}"))
        }
    }

    #[rstest]
    fn stub_localizer_uses_args() {
        let localizer = StubLocalizer;
        let mut args: LocalizationArgs<'static> = HashMap::new();
        args.insert("subject", FluentValue::from("hello"));
        let resolved = localizer.message_with_args_or("cli.about", Some(&args), "fallback");
        assert_eq!(resolved, "cli.about:hello");
    }
}
