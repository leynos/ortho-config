//! Demo localiser showcasing how applications can layer Fluent resources over
//! the embedded defaults provided by `ortho_config`.
//!
//! The example embeds a small English catalogue and builds a
//! [`FluentLocalizer`] so the CLI copy is translated through the same abstraction
//! used by consumer binaries. Should localisation setup fail, the localiser
//! falls back to [`NoOpLocalizer`] and retains the stock `clap` strings.

use ortho_config::{
    FluentLocalizer, FluentLocalizerError, LocalizationArgs, Localizer, NoOpLocalizer,
};
use tracing::warn;

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

const HELLO_WORLD_EN_US: &str = include_str!("../locales/en-US/messages.ftl");

/// Localiser that layers the example's Fluent catalogue over the embedded defaults.
pub struct DemoLocalizer {
    inner: Option<FluentLocalizer>,
}

impl Default for DemoLocalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoLocalizer {
    /// Builds the demo localiser, falling back to [`NoOpLocalizer`] if Fluent setup fails.
    ///
    /// # Examples
    /// ```rust
    /// use clap::CommandFactory;
    /// use hello_world::cli::{CommandLine, LocalizeCmd};
    /// use hello_world::localizer::DemoLocalizer;
    ///
    /// let localizer = DemoLocalizer::new();
    /// let command = CommandLine::command().localize(&localizer);
    /// assert!(command.get_about().is_some());
    /// ```
    ///
    #[must_use]
    pub fn new() -> Self {
        Self::try_new().unwrap_or_else(|error| {
            warn!(?error, "falling back to no-op localiser");
            Self { inner: None }
        })
    }

    /// Attempts to construct the demo localiser without falling back.
    ///
    /// # Errors
    ///
    /// Returns an error when the embedded Fluent catalogue cannot be parsed
    /// or registered.
    pub fn try_new() -> Result<Self, FluentLocalizerError> {
        Ok(Self {
            inner: Some(FluentLocalizer::with_en_us_defaults([HELLO_WORLD_EN_US])?),
        })
    }

    /// Returns the underlying [`FluentLocalizer`] when available so consumers
    /// can reuse the same catalogue instance.
    #[must_use]
    pub const fn fluent(&self) -> Option<&FluentLocalizer> {
        self.inner.as_ref()
    }

    /// Provides a no-op localiser for callers that do not ship translations yet.
    #[must_use]
    pub const fn noop() -> NoOpLocalizer {
        NoOpLocalizer::new()
    }
}

impl Localizer for DemoLocalizer {
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        self.inner.as_ref().map_or_else(
            || NoOpLocalizer::new().lookup(id, args),
            |fluent| fluent.lookup(id, args),
        )
    }
}

impl std::fmt::Debug for DemoLocalizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = if self.inner.is_some() {
            "fluent"
        } else {
            "noop"
        };

        f.debug_struct("DemoLocalizer")
            .field("inner", &inner)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn demo_localiser_returns_copy() {
        let localiser = DemoLocalizer::try_new().expect("demo localiser should build");
        let about = localiser
            .lookup(CLI_ABOUT_MESSAGE_ID, None)
            .expect("demo copy should exist");
        assert!(about.contains("greeting"));
    }

    #[test]
    fn demo_localiser_formats_long_description() {
        let localiser = DemoLocalizer::try_new().expect("demo localiser should build");
        let mut args: LocalizationArgs<'_> = HashMap::new();
        args.insert("binary", "hello-world".into());
        let long_about = localiser
            .lookup(CLI_LONG_ABOUT_MESSAGE_ID, Some(&args))
            .expect("long description should exist");
        assert!(long_about.contains("hello-world"));
    }

    #[test]
    fn fluent_returns_inner_reference() {
        let localiser = DemoLocalizer::try_new().expect("demo localiser should build");
        let inner_ptr = localiser
            .inner
            .as_ref()
            .map(std::ptr::from_ref::<FluentLocalizer>)
            .expect("expected fluent inner");

        let exposed = localiser.fluent().expect("fluent inner should be exposed");
        assert!(std::ptr::eq(inner_ptr, exposed));
    }

    #[test]
    fn fluent_is_none_when_noop() {
        let localiser = DemoLocalizer { inner: None };

        assert!(localiser.fluent().is_none());
    }
}
