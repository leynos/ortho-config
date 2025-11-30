// Shared test localisers for clap localisation scenarios.
use crate_root::{LocalizationArgs, Localizer};
use fluent_bundle::FluentValue;

#[derive(Debug, Clone, Copy)]
pub struct ArgumentEchoLocalizer {
    missing_placeholder: &'static str,
}

impl ArgumentEchoLocalizer {
    #[must_use]
    pub const fn new(missing_placeholder: &'static str) -> Self {
        Self {
            missing_placeholder,
        }
    }
}

impl Localizer for ArgumentEchoLocalizer {
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        let argument = args
            .and_then(|values| values.get("argument"))
            .and_then(|value| match value {
                FluentValue::String(text) => Some(text.to_string()),
                _ => None,
            })
            .unwrap_or_else(|| self.missing_placeholder.to_owned());
        Some(format!("{id}:{argument}"))
    }
}
