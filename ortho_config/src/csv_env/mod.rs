//! Environment provider that parses comma-separated lists.
//!
//! Wraps `figment::providers::Env` and converts values containing commas
//! into arrays unless they look like structured data (starting with `[` or
//! `{` or a quote). This allows environment variables such as
//! `DDLINT_RULES=A,B,C` to be deserialized as `Vec<String>`. Values with
//! embedded commas must be wrapped in quotes or brackets to avoid being split.

use figment::providers::Env;
use figment::{
    Profile, Provider,
    error::Error,
    util::nest,
    value::{Dict, Map, Value},
};
use std::ops::Deref;
use uncased::{Uncased, UncasedStr};

mod injected;
mod options;
use crate::merge_telemetry;
use options::{Csv, KeyTransform, Lowercase, Options, Uppercase};

/// Environment provider with CSV list support.
///
/// Wraps the standard [`Env`] provider to interpret comma-separated
/// values as arrays, whilst leaving JSON strings untouched.
#[derive(Clone)]
pub struct CsvEnv {
    /// Inner environment provider that performs the actual variable access.
    inner: Env,
    options: Options,
}

impl CsvEnv {
    /// Create an unprefixed provider.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::CsvEnv;
    /// let env = CsvEnv::raw();
    /// let _ = env;
    /// ```
    #[must_use]
    pub fn raw() -> Self {
        Self::new(Env::raw(), None)
    }

    /// Create a provider using `prefix`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::CsvEnv;
    /// let env = CsvEnv::prefixed("APP_");
    /// let _ = env;
    /// ```
    #[must_use]
    pub fn prefixed(prefix: &str) -> Self {
        Self::new(Env::prefixed(prefix), Some(prefix.into()))
    }

    /// Split keys at `pattern`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::CsvEnv;
    /// let env = CsvEnv::raw().split("__");
    /// let _ = env;
    /// ```
    #[must_use]
    pub fn split(mut self, pattern: &str) -> Self {
        self.inner = self.inner.split(pattern);
        self.options.split_pattern = Some(pattern.into());
        self
    }

    /// Map keys using `mapper`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::CsvEnv;
    /// use uncased::Uncased;
    /// let env = CsvEnv::raw().map(|k| Uncased::from(format!("APP_{k}")));
    /// let _ = env;
    /// ```
    #[must_use]
    pub fn map<F>(mut self, mapper: F) -> Self
    where
        F: Fn(&UncasedStr) -> Uncased<'_> + Clone + 'static,
    {
        self.inner = self.inner.map(mapper);
        self.options.key_transform = KeyTransform::Opaque;
        self
    }

    /// Filter and map keys using `f`.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use ortho_config::CsvEnv;
    /// use uncased::Uncased;
    /// let env = CsvEnv::raw().filter_map(|k| k.strip_prefix("APP_").map(Uncased::from));
    /// // requires `UncasedStr::strip_prefix`; shown for illustration only
    /// let _ = env;
    /// ```
    #[must_use]
    pub fn filter_map<F>(mut self, f: F) -> Self
    where
        F: Fn(&UncasedStr) -> Option<Uncased<'_>> + Clone + 'static,
    {
        self.inner = self.inner.filter_map(f);
        self.options.key_transform = KeyTransform::Opaque;
        self
    }

    /// Whether to lowercase keys before emitting them.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::CsvEnv;
    /// let env = CsvEnv::raw().lowercase(true);
    /// let _ = env;
    /// ```
    #[must_use]
    pub fn lowercase(mut self, lowercase: bool) -> Self {
        self.inner = self.inner.lowercase(lowercase);
        self.options.lowercase = Lowercase::from_bool(lowercase);
        self
    }

    /// Whether to uppercase keys before splitting and lowercasing them.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::CsvEnv;
    /// let env = CsvEnv::raw().uppercase(true);
    /// let _ = env;
    /// ```
    #[must_use]
    pub fn uppercase(mut self, uppercase: bool) -> Self {
        let uppercase_transform = Uppercase::from_bool(uppercase);
        self.inner = self.inner.map(move |source_key| {
            let key_name = source_key.as_str();
            if uppercase_transform.is_enabled() {
                Uncased::from(key_name.to_ascii_uppercase())
            } else {
                Uncased::from(key_name)
            }
        });
        self.options.uppercase = uppercase_transform;
        self
    }

    /// Whether comma-containing values should be parsed as CSV lists.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::CsvEnv;
    /// let env = CsvEnv::raw().csv(false);
    /// let _ = env;
    /// ```
    #[must_use]
    pub const fn csv(mut self, csv: bool) -> Self {
        self.options.csv = Csv::from_bool(csv);
        self
    }

    /// Read variables from `source` instead of the process environment.
    ///
    /// This path replays the provider's declarative key transforms. It rejects
    /// arbitrary [`Self::map`] and [`Self::filter_map`] closures, because their
    /// behaviour cannot be recovered from the wrapped [`Env`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::{CsvEnv, MapEnv};
    /// use std::sync::Arc;
    ///
    /// let source = Arc::new(MapEnv::new().with_var("APP_HOST", "localhost"));
    /// let env = CsvEnv::prefixed("APP_").with_source(source);
    /// let _ = env;
    /// ```
    #[must_use]
    pub fn with_source(mut self, source: crate::SharedScanEnvSource) -> Self {
        self.options.source = Some(source);
        self
    }

    /// Delegate process-backed enumeration to Figment without replaying it.
    ///
    /// Retaining this delegation pins the default path to Figment's exact
    /// semantics; only injected sources use the declarative replay below.
    fn iter(&self) -> impl Iterator<Item = (Uncased<'static>, String)> + '_ {
        self.inner.iter()
    }

    /// Pair an existing Figment provider with replayable transform options.
    ///
    /// Providers converted through [`From<Env>`] cannot expose their transform
    /// history, so that conversion marks the key transform opaque separately.
    fn new(inner: Env, prefix: Option<String>) -> Self {
        Self {
            inner,
            options: Options::new(prefix),
        }
    }

    /// Determine if a value should be parsed as comma-separated rather than
    /// structured data.
    ///
    /// The value is treated as CSV when it contains a comma and does not start
    /// with `[` , `{`, `"` or `'`. This avoids misinterpreting JSON or quoted
    /// strings as lists.
    fn should_parse_as_csv(value: &str) -> bool {
        let trimmed = value.trim();
        trimmed.contains(',') && !matches!(trimmed.chars().next(), Some('[' | '{' | '"' | '\''))
    }

    /// Parse a scalar (non-CSV) string into a [`Value`].
    ///
    /// Handles boolean literals case-insensitively, then falls back to
    /// `serde_json` parsing, and finally treats the input as a plain string.
    fn parse_scalar(trimmed: &str) -> Value {
        if trimmed.eq_ignore_ascii_case("true") {
            return true.into();
        }
        if trimmed.eq_ignore_ascii_case("false") {
            return false.into();
        }
        trimmed
            .parse()
            .unwrap_or_else(|_| Value::from(trimmed.to_owned()))
    }

    /// Parse a raw value according to the provider's CSV policy.
    ///
    /// CSV parsing intentionally happens after key transforms, so turning it
    /// off for subcommands changes values only and never key nesting.
    fn parse_value(raw: &str, csv: bool) -> Value {
        let trimmed = raw.trim();
        if csv && Self::should_parse_as_csv(trimmed) {
            trimmed
                .split(',')
                .map(|s| Value::from(s.trim().to_owned()))
                .collect::<Vec<_>>()
                .into()
        } else {
            Self::parse_scalar(trimmed)
        }
    }

    /// Build the provider data after the caller has recorded its source choice.
    ///
    /// Keeping collection separate from telemetry guarantees exactly one
    /// terminal event for either source path, including early provider errors.
    fn collect_data(&self) -> Result<Map<Profile, Dict>, Box<Error>> {
        let mut dict = Dict::new();
        let injected_entries = self
            .options
            .source
            .is_some()
            .then(|| self.injected_entries())
            .transpose()
            .map_err(|error| *error)?;
        let entries = injected_entries.unwrap_or_else(|| self.iter().collect());
        for (k, v) in entries {
            let value = Self::parse_value(&v, self.options.csv.is_enabled());
            let Some(nested) = nest(k.as_str(), value).into_dict() else {
                return Err(Box::new(Error::from(format!(
                    "environment key `{k}` produced a non-object value"
                ))));
            };
            dict.extend(nested);
        }
        Ok(self.inner.profile.collect(dict))
    }
}

impl Provider for CsvEnv {
    /// Preserve Figment's metadata for diagnostics and provider composition.
    fn metadata(&self) -> figment::Metadata {
        self.inner.metadata()
    }

    /// Preserve the inner provider's profile when collecting replayed entries.
    fn profile(&self) -> Option<Profile> {
        Some(self.inner.profile.clone())
    }

    /// Collect process-backed or injected entries and emit bounded telemetry.
    ///
    /// The event records source selection and the terminal outcome, never an
    /// environment key, value, prefix, or raw provider error.
    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        let is_injected = self.options.source.is_some();
        if is_injected {
            merge_telemetry::csv_env_injected_started();
        } else {
            merge_telemetry::csv_env_process_started();
        }

        let result = self.collect_data().map_err(|error| *error);
        match &result {
            Ok(_) if is_injected => merge_telemetry::csv_env_injected_succeeded(),
            Ok(_) => merge_telemetry::csv_env_process_succeeded(),
            Err(_) => merge_telemetry::csv_env_failed(
                is_injected,
                matches!(self.options.key_transform, KeyTransform::Opaque),
            ),
        }
        result
    }
}

impl From<Env> for CsvEnv {
    /// Wrap a preconfigured Figment provider without assuming its transform history.
    ///
    /// Figment stores arbitrary key closures opaquely, so injected loading is
    /// conservatively rejected for this conversion while process behaviour is
    /// preserved unchanged.
    fn from(inner: Env) -> Self {
        let mut provider = Self::new(inner, None);
        provider.options.key_transform = KeyTransform::Opaque;
        provider
    }
}

impl Deref for CsvEnv {
    type Target = Env;

    /// Expose the inner provider for process-backed compatibility operations.
    ///
    /// Callers that configure it through the dereferenced `Env` may introduce
    /// opaque transforms, so injected use remains guarded by [`Self::with_source`].
    fn deref(&self) -> &Env {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for CSV environment-variable parsing.

    use super::*;
    use figment::value::Tag;
    use rstest::rstest;

    #[rstest]
    #[case("true", Value::Bool(Tag::Default, true))]
    #[case("false", Value::Bool(Tag::Default, false))]
    #[case("TRUE", Value::Bool(Tag::Default, true))]
    #[case("FALSE", Value::Bool(Tag::Default, false))]
    #[case("True", Value::Bool(Tag::Default, true))]
    #[case("False", Value::Bool(Tag::Default, false))]
    fn parse_scalar_handles_boolean_strings(#[case] input: &str, #[case] expected: Value) {
        assert_eq!(CsvEnv::parse_scalar(input), expected);
    }

    #[rstest]
    #[case("hello")]
    #[case("some_value")]
    fn parse_scalar_falls_back_to_string(#[case] input: &str) {
        assert_eq!(CsvEnv::parse_scalar(input), Value::from(input.to_owned()));
    }
}
