//! Fluent bundle utilities extracted from the localisation module.
//!
//! Keeping parsing, resource registration, and identifier normalisation in a
//! dedicated module keeps `mod.rs` concise while retaining cohesion around
//! Fluent-specific concerns.

use fluent_bundle::FluentResource;
use fluent_bundle::concurrent::FluentBundle;
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;
use unic_langid::{LanguageIdentifier, langid};

use super::{
    FluentBundleSource, FluentLocalizer, FluentLocalizerBuilder, FluentLocalizerError,
    FormattingIssueReporter,
};

pub(super) struct BundleWithLocale {
    pub(super) locale: LanguageIdentifier,
    pub(super) bundle: FluentBundle<Arc<FluentResource>>,
    pub(super) kind: FluentBundleSource,
}

impl fmt::Debug for BundleWithLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BundleWithLocale")
            .field("locale", &self.locale)
            .field("bundle", &"<fluent bundle>")
            .field("kind", &self.kind)
            .finish()
    }
}

const EN_US_CATALOGUE: &str = include_str!("../../locales/en-US/messages.ftl");
static EN_US_RESOURCES: [&str; 1] = [EN_US_CATALOGUE];

pub(super) fn default_resources(locale: &LanguageIdentifier) -> Option<&'static [&'static str]> {
    if locale == &langid!("en-US") {
        Some(&EN_US_RESOURCES)
    } else {
        None
    }
}

pub(super) fn bundle_from_resources(
    locale: &LanguageIdentifier,
    resources: &[&'static str],
    catalogue: FluentBundleSource,
) -> Result<BundleWithLocale, FluentLocalizerError> {
    let mut bundle = FluentBundle::new_concurrent(vec![locale.clone()]);
    for resource in resources {
        let parsed = Arc::new(
            FluentResource::try_new(normalize_resource_ids(resource)).map_err(
                |(_resource, errors)| FluentLocalizerError::Parser {
                    locale: locale.clone(),
                    catalogue,
                    errors,
                },
            )?,
        );

        bundle
            .add_resource(parsed)
            .map_err(|errors| FluentLocalizerError::Registration {
                locale: locale.clone(),
                catalogue,
                errors,
            })?;
    }

    Ok(BundleWithLocale {
        locale: locale.clone(),
        bundle,
        kind: catalogue,
    })
}

pub(super) fn normalize_identifier(id: &str) -> Cow<'_, str> {
    if id.contains('.') {
        Cow::Owned(id.replace('.', "-"))
    } else {
        Cow::Borrowed(id)
    }
}

pub(super) fn normalize_resource_ids(resource: &str) -> String {
    resource
        .lines()
        .map(normalize_id_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_id_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return line.to_owned();
    }

    let Some((left, right)) = line.split_once('=') else {
        return line.to_owned();
    };

    let first_char = trimmed.chars().next().unwrap_or(' ');
    if !first_char.is_ascii_alphabetic() {
        return line.to_owned();
    }

    let leading_ws_len = left.len() - left.trim_start().len();
    let id_end = left.trim_end().len();
    if id_end <= leading_ws_len {
        return line.to_owned();
    }

    let Some(id_segment) = left.get(leading_ws_len..id_end) else {
        return line.to_owned();
    };
    let normalised_id = normalize_identifier(id_segment).into_owned();

    let mut rebuilt = String::with_capacity(line.len());
    if let Some(prefix) = left.get(..leading_ws_len) {
        rebuilt.push_str(prefix);
    }
    rebuilt.push_str(&normalised_id);
    if let Some(trailing) = left.get(id_end..) {
        rebuilt.push_str(trailing);
    }
    rebuilt.push('=');
    rebuilt.push_str(right);
    rebuilt
}

impl FluentLocalizerBuilder {
    /// Creates a builder for the requested locale.
    #[must_use]
    pub fn new(locale: LanguageIdentifier) -> Self {
        Self {
            locale,
            consumer_resources: Vec::new(),
            consumer_bundle: None,
            report_issue: super::default_reporter(),
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
