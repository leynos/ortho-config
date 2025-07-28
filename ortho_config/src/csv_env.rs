//! Environment provider that parses comma-separated lists.
//!
//! Wraps `figment::providers::Env` and converts values containing commas
//! into arrays unless they look like structured data (starting with `[` or
//! `{` or a quote). This allows environment variables such as
//! `DDLINT_RULES=A,B,C` to be deserialised as `Vec<String>`.

use figment::providers::Env;
use figment::{
    Profile, Provider,
    error::Error,
    util::nest,
    value::{Dict, Map, Value},
};
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
            let trimmed = v.trim();
            let value = if trimmed.contains(',')
                && !trimmed.starts_with('[')
                && !trimmed.starts_with('{')
                && !trimmed.starts_with('"')
                && !trimmed.starts_with('\'')
            {
                let arr = trimmed
                    .split(',')
                    .map(|s| Value::from(s.trim().to_string()))
                    .collect::<Vec<_>>();
                Value::from(arr)
            } else {
                v.parse().expect("infallible")
            };
            let nested = nest(k.as_str(), value)
                .into_dict()
                .expect("key is non-empty: must have dict");
            dict.extend(nested);
        }
        Ok(self.inner.profile.collect(dict))
    }
}
