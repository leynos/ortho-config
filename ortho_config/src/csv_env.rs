//! Environment provider that parses comma-separated lists.
//!
//! Wraps `figment::providers::Env` and converts values containing commas
//! into arrays unless they look like structured data (starting with `[` or
//! `{` or a quote). This allows environment variables such as
//! `DDLINT_RULES=A,B,C` to be deserialised as `Vec<String>`. Values with
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
#[derive(Clone)]
pub struct CsvEnv {
    inner: Env,
}

impl CsvEnv {
    /// Create an unprefixed provider.
    #[must_use]
    pub fn raw() -> Self {
        Self { inner: Env::raw() }
    }

    /// Create a provider using `prefix`.
    #[must_use]
    pub fn prefixed(prefix: &str) -> Self {
        Self {
            inner: Env::prefixed(prefix),
        }
    }

    /// Split keys at `pattern`.
    #[must_use]
    pub fn split(self, pattern: &str) -> Self {
        Self {
            inner: self.inner.split(pattern),
        }
    }

    /// Map keys using `mapper`.
    #[must_use]
    pub fn map<F>(self, mapper: F) -> Self
    where
        F: Fn(&UncasedStr) -> Uncased<'_> + Clone + 'static,
    {
        Self {
            inner: self.inner.map(mapper),
        }
    }

    /// Filter and map keys using `f`.
    #[must_use]
    pub fn filter_map<F>(self, f: F) -> Self
    where
        F: Fn(&UncasedStr) -> Option<Uncased<'_>> + Clone + 'static,
    {
        Self {
            inner: self.inner.filter_map(f),
        }
    }

    /// Whether to lowercase keys before emitting them.
    #[must_use]
    pub fn lowercase(self, lowercase: bool) -> Self {
        Self {
            inner: self.inner.lowercase(lowercase),
        }
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
                .map(|s| Value::from(s.trim().to_string()))
                .collect::<Vec<_>>()
                .into()
        } else {
            trimmed
                .parse()
                .unwrap_or_else(|_| Value::from(trimmed.to_string()))
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
            let nested = nest(k.as_str(), value)
                .into_dict()
                .expect("key is non-empty: must have dict");
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
