//! Localisation primitives shared across the workspace.
//!
//! `Localizer` abstracts string lookup so `clap` integration, examples, and
//! future tooling can localise help text without coupling to a concrete
//! translation backend. The trait is intentionally minimal: implementations
//! return owned `String` values so callers can cache the resolved text or fall
//! back to defaults supplied by `clap` when no translation exists.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentError, FluentResource, FluentValue};
use fluent_syntax::parser::ParserError;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use thiserror::Error;
use unic_langid::{LanguageIdentifier, langid};

mod fluent;
use fluent::{BundleWithLocale, bundle_from_resources, default_resources, normalize_identifier};

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
    /// Performs a localisation lookup for the provided identifier.
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String>;

    /// Resolves the message and returns a fallback string when no translation exists.
    ///
    /// # Examples
    /// ```rust
    /// use ortho_config::{LocalizationArgs, Localizer};
    ///
    /// struct AlwaysFallback;
    ///
    /// impl Localizer for AlwaysFallback {
    ///     fn lookup(&self, _id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
    ///         None
    ///     }
    /// }
    ///
    /// let localizer = AlwaysFallback;
    /// assert_eq!(localizer.message("cli.about", None, "fallback"), "fallback");
    /// ```
    fn message(&self, id: &str, args: Option<&LocalizationArgs<'_>>, fallback: &str) -> String {
        self.lookup(id, args).unwrap_or_else(|| fallback.to_owned())
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
    fn lookup(&self, _id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        None
    }
}

/// Denotes which bundle produced a localisation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluentBundleSource {
    /// Application-provided catalogue layered over the defaults.
    Consumer,
    /// Embedded catalogue shipped with the crate.
    Default,
}

/// Captures formatting failures encountered when resolving Fluent patterns.
#[derive(Debug, Clone)]
pub struct FormattingIssue {
    /// Identifier that failed to resolve.
    pub id: String,
    /// Locale associated with the bundle.
    pub locale: LanguageIdentifier,
    /// Source bundle that produced the failure.
    pub source: FluentBundleSource,
    /// Formatting or resolver errors emitted by Fluent.
    pub errors: Vec<FluentError>,
}

/// Reporter invoked when Fluent raises formatting errors.
pub type FormattingIssueReporter = Arc<dyn Fn(&FormattingIssue) + Send + Sync>;

/// Fluent-powered localiser that layers consumer bundles over embedded defaults.
pub struct FluentLocalizer {
    consumer: Option<BundleWithLocale>,
    defaults: Option<BundleWithLocale>,
    report_issue: FormattingIssueReporter,
}

/// Builds a [`FluentLocalizer`].
pub struct FluentLocalizerBuilder {
    locale: LanguageIdentifier,
    consumer_resources: Vec<&'static str>,
    consumer_bundle: Option<BundleWithLocale>,
    report_issue: FormattingIssueReporter,
    use_defaults: bool,
}

/// Errors surfaced when constructing a [`FluentLocalizer`].
#[derive(Debug, Error)]
pub enum FluentLocalizerError {
    /// Requested a locale without embedded resources.
    #[error("no embedded Fluent resources exist for locale {locale}")]
    UnsupportedLocale {
        /// Locale requested by the caller.
        locale: LanguageIdentifier,
    },

    /// Failed to parse Fluent text into resources.
    #[error("failed to parse {catalogue:?} resources for {locale}")]
    Parser {
        /// Locale associated with the catalogue.
        locale: LanguageIdentifier,
        /// Which catalogue failed.
        catalogue: FluentBundleSource,
        /// Parser errors emitted by Fluent.
        errors: Vec<ParserError>,
    },

    /// Fluent rejected a resource while registering it in the bundle.
    #[error("failed to register {catalogue:?} resources for {locale}")]
    Registration {
        /// Locale associated with the catalogue.
        locale: LanguageIdentifier,
        /// Which catalogue failed.
        catalogue: FluentBundleSource,
        /// Errors returned by Fluent during registration.
        errors: Vec<FluentError>,
    },

    /// Consumer bundle locale did not match the builder's locale.
    #[error("consumer bundle locale {consumer} mismatches builder locale {builder}")]
    ConsumerLocaleMismatch {
        /// Locale requested for the localiser.
        builder: LanguageIdentifier,
        /// Locale attached to the provided consumer bundle.
        consumer: LanguageIdentifier,
    },
}

impl FluentLocalizer {
    /// Starts building a localiser for the provided locale.
    ///
    /// # Examples
    /// ```rust
    /// use ortho_config::{FluentLocalizer, Localizer, langid};
    ///
    /// let localizer = FluentLocalizer::builder(langid!("en-US"))
    ///     .try_build()
    ///     .expect("embedded resources should be valid");
    /// assert!(localizer.lookup("cli.about", None).is_some());
    /// ```
    #[must_use]
    pub fn builder(locale: LanguageIdentifier) -> FluentLocalizerBuilder {
        FluentLocalizerBuilder::new(locale)
    }

    /// Builds a localiser using only the embedded catalogue for the locale.
    ///
    /// This helper avoids repeating `builder(locale).try_build()` in consumers
    /// that do not need to layer additional resources.
    ///
    /// # Errors
    ///
    /// Returns [`FluentLocalizerError::UnsupportedLocale`] when no embedded
    /// catalogue exists for the requested locale or propagates parsing and
    /// registration failures surfaced while constructing the bundles.
    pub fn embedded(locale: LanguageIdentifier) -> Result<Self, FluentLocalizerError> {
        Self::builder(locale).try_build()
    }

    /// Builds a localiser that layers consumer resources over the embedded defaults.
    ///
    /// # Errors
    ///
    /// Returns [`FluentLocalizerError`] when the embedded catalogue is missing
    /// for the requested locale or when any provided resource fails to parse or
    /// register.
    pub fn with_embedded_and(
        locale: LanguageIdentifier,
        resources: impl IntoIterator<Item = &'static str>,
    ) -> Result<Self, FluentLocalizerError> {
        Self::builder(locale)
            .with_consumer_resources(resources)
            .try_build()
    }

    /// Builds an English (en-US) localiser with the provided consumer resources.
    ///
    /// # Errors
    ///
    /// Returns [`FluentLocalizerError`] when the embedded en-US catalogue or
    /// consumer resources fail to parse or register.
    pub fn with_en_us_defaults(
        resources: impl IntoIterator<Item = &'static str>,
    ) -> Result<Self, FluentLocalizerError> {
        Self::with_embedded_and(langid!("en-US"), resources)
    }
}

impl Localizer for FluentLocalizer {
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        let fluent_args = args.map(fluent_args_from);
        let normalized_id = normalize_identifier(id);
        let lookup_ids = if normalized_id.as_ref() == id {
            [id, id]
        } else {
            [id, normalized_id.as_ref()]
        };
        let bundles = [self.consumer.as_ref(), self.defaults.as_ref()];

        for bundle in bundles.into_iter().flatten() {
            let pattern_opt = lookup_ids.iter().find_map(|lookup_id| {
                bundle
                    .bundle
                    .get_message(lookup_id)
                    .and_then(|message| message.value())
            });

            let Some(pattern) = pattern_opt else { continue };

            let mut errors = Vec::new();
            let rendered = bundle
                .bundle
                .format_pattern(pattern, fluent_args.as_ref(), &mut errors);

            if errors.is_empty() {
                return Some(rendered.into_owned());
            }

            (self.report_issue)(&FormattingIssue {
                id: id.to_owned(),
                locale: bundle.locale.clone(),
                source: bundle.kind,
                errors,
            });
        }

        None
    }
}

impl FluentLocalizerBuilder {
    /// Creates a builder for the requested locale.
    #[must_use]
    pub fn new(locale: LanguageIdentifier) -> Self {
        Self {
            locale,
            consumer_resources: Vec::new(),
            consumer_bundle: None,
            report_issue: default_reporter(),
            use_defaults: true,
        }
    }

    /// Adds consumer-provided Fluent resources to layer over the defaults.
    #[must_use]
    pub fn with_consumer_resources(
        mut self,
        resources: impl IntoIterator<Item = &'static str>,
    ) -> Self {
        self.consumer_resources.extend(resources);
        self
    }

    /// Supplies a pre-built consumer bundle, bypassing resource parsing.
    #[must_use]
    pub fn with_consumer_bundle(mut self, bundle: FluentBundle<Arc<FluentResource>>) -> Self {
        self.consumer_bundle = Some(BundleWithLocale {
            locale: self.locale.clone(),
            bundle,
            kind: FluentBundleSource::Consumer,
        });
        self
    }

    /// Disables loading embedded defaults, enabling consumer-only catalogues.
    #[must_use]
    pub const fn disable_defaults(mut self) -> Self {
        self.use_defaults = false;
        self
    }

    /// Installs a hook to report formatting issues surfaced by Fluent.
    #[must_use]
    pub fn with_error_reporter(mut self, reporter: FormattingIssueReporter) -> Self {
        self.report_issue = reporter;
        self
    }

    /// Builds the [`FluentLocalizer`], validating both default and consumer bundles.
    ///
    /// # Errors
    ///
    /// Returns [`FluentLocalizerError`] if any catalogue fails to parse or
    /// registers conflicting identifiers.
    pub fn try_build(self) -> Result<FluentLocalizer, FluentLocalizerError> {
        if let Some(bundle) = &self.consumer_bundle
            && bundle.locale != self.locale
        {
            return Err(FluentLocalizerError::ConsumerLocaleMismatch {
                builder: self.locale,
                consumer: bundle.locale.clone(),
            });
        }

        let defaults = if self.use_defaults {
            Some(bundle_from_resources(
                &self.locale,
                default_resources(&self.locale).ok_or_else(|| {
                    FluentLocalizerError::UnsupportedLocale {
                        locale: self.locale.clone(),
                    }
                })?,
                FluentBundleSource::Default,
            )?)
        } else {
            None
        };

        let consumer = if let Some(bundle) = self.consumer_bundle {
            Some(bundle)
        } else if self.consumer_resources.is_empty() {
            None
        } else {
            Some(bundle_from_resources(
                &self.locale,
                &self.consumer_resources,
                FluentBundleSource::Consumer,
            )?)
        };

        Ok(FluentLocalizer {
            consumer,
            defaults,
            report_issue: self.report_issue,
        })
    }
}

impl fmt::Debug for FluentLocalizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FluentLocalizer")
            .field(
                "consumer",
                &self.consumer.as_ref().map(|bundle| &bundle.locale),
            )
            .field(
                "defaults",
                &self.defaults.as_ref().map(|bundle| &bundle.locale),
            )
            .field("report_issue", &"<formatter>")
            .finish()
    }
}

impl fmt::Debug for FluentLocalizerBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FluentLocalizerBuilder")
            .field("locale", &self.locale)
            .field("consumer_resources_len", &self.consumer_resources.len())
            .field(
                "consumer_bundle",
                &self.consumer_bundle.as_ref().map(|bundle| &bundle.locale),
            )
            .field("use_defaults", &self.use_defaults)
            .field("report_issue", &"<formatter>")
            .finish()
    }
}

fn default_reporter() -> FormattingIssueReporter {
    Arc::new(|issue: &FormattingIssue| {
        tracing::warn!(
            id = %issue.id,
            locale = %issue.locale,
            source = ?issue.source,
            errors = ?issue.errors,
            "failed to format Fluent message"
        );
    })
}

fn fluent_args_from<'a>(args: &'a LocalizationArgs<'a>) -> FluentArgs<'a> {
    let mut fluent_args = FluentArgs::with_capacity(args.len());
    for (key, value) in args {
        fluent_args.set(*key, value.clone());
    }
    fluent_args
}

#[cfg(test)]
mod tests;
