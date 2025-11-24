//! Demo localiser showcasing how applications can layer Fluent resources over
//! the embedded defaults provided by `ortho_config`.
//!
//! The example embeds a small English catalogue and builds a
//! [`FluentLocalizer`] so the CLI copy is translated through the same abstraction
//! used by consumer binaries. Should localisation setup fail, the localiser
//! falls back to [`NoOpLocalizer`] and retains the stock `clap` strings.

use ortho_config::{
    FluentLocalizer, FluentLocalizerError, LocalizationArgs, Localizer, NoOpLocalizer, langid,
};
use std::{fmt, sync::Arc};
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
    inner: Arc<dyn Localizer + Send + Sync>,
}

impl Default for DemoLocalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DemoLocalizer {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
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
            Self {
                inner: Arc::new(NoOpLocalizer::new()),
            }
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
            inner: Arc::new(build_fluent_localizer()?),
        })
    }

    /// Returns the underlying [`FluentLocalizer`] so consumers can reuse the catalogue.
    ///
    /// # Errors
    ///
    /// Returns an error when the embedded catalogue fails to parse or when
    /// Fluent rejects the resources while building the bundle.
    pub fn fluent() -> Result<FluentLocalizer, FluentLocalizerError> {
        build_fluent_localizer()
    }

    /// Provides a no-op localiser for callers that do not ship translations yet.
    #[must_use]
    pub const fn noop() -> NoOpLocalizer {
        NoOpLocalizer::new()
    }
}

impl Localizer for DemoLocalizer {
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        self.inner.lookup(id, args)
    }
}

impl fmt::Debug for DemoLocalizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DemoLocalizer")
            .field("inner", &"<localizer>")
            .finish()
    }
}

fn build_fluent_localizer() -> Result<FluentLocalizer, FluentLocalizerError> {
    FluentLocalizer::builder(langid!("en-US"))
        .with_consumer_resources([HELLO_WORLD_EN_US])
        .try_build()
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
}
