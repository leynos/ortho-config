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

/// Environment provider with CSV list support.
///
/// Wraps the standard [`Env`] provider to interpret comma-separated
/// values as arrays, whilst leaving JSON strings untouched.
#[derive(Clone)]
pub struct CsvEnv {
    /// Inner environment provider that performs the actual variable access.
    inner: Env,
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
        Env::raw().into()
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
        Env::prefixed(prefix).into()
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
    pub fn split(self, pattern: &str) -> Self {
        self.inner.split(pattern).into()
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
    pub fn map<F>(self, mapper: F) -> Self
    where
        F: Fn(&UncasedStr) -> Uncased<'_> + Clone + 'static,
    {
        self.inner.map(mapper).into()
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
    pub fn filter_map<F>(self, f: F) -> Self
    where
        F: Fn(&UncasedStr) -> Option<Uncased<'_>> + Clone + 'static,
    {
        self.inner.filter_map(f).into()
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
    pub fn lowercase(self, lowercase: bool) -> Self {
        self.inner.lowercase(lowercase).into()
    }

    fn iter(&self) -> impl Iterator<Item = (Uncased<'static>, String)> + '_ {
        self.inner.iter()
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

    fn parse_value(raw: &str) -> Value {
        let trimmed = raw.trim();
        if Self::should_parse_as_csv(trimmed) {
            trimmed
                .split(',')
                .map(|s| Value::from(s.trim().to_owned()))
                .collect::<Vec<_>>()
                .into()
        } else {
            trimmed
                .parse()
                .unwrap_or_else(|_| Value::from(trimmed.to_owned()))
        }
    }
}

impl Provider for CsvEnv {
    fn metadata(&self) -> figment::Metadata {
        self.inner.metadata()
    }

    fn profile(&self) -> Option<Profile> {
        Some(self.inner.profile.clone())
    }

    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        let mut dict = Dict::new();
        for (k, v) in self.iter() {
            let value = Self::parse_value(&v);
            let Some(nested) = nest(k.as_str(), value).into_dict() else {
                return Err(Error::from(format!(
                    "environment key `{k}` produced a non-object value"
                )));
            };
            dict.extend(nested);
        }
        Ok(self.inner.profile.collect(dict))
    }
}

impl From<Env> for CsvEnv {
    fn from(inner: Env) -> Self {
        Self { inner }
    }
}

impl Deref for CsvEnv {
    type Target = Env;

    fn deref(&self) -> &Env {
        &self.inner
    }
}
