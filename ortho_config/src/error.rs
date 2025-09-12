//! Error types produced by the configuration loader.

use figment::error::Error as FigmentError;
use std::{error::Error, fmt, sync::Arc};
use thiserror::Error;

/// Errors that can occur while loading configuration.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OrthoError {
    /// Error parsing command-line arguments.
    #[error("Failed to parse command-line arguments: {0}")]
    CliParsing(#[from] Box<clap::Error>),

    /// Error originating from a configuration file.
    #[error("Configuration file error in '{path}': {source}")]
    File {
        path: std::path::PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Cycle detected while resolving `extends`.
    #[error("cyclic extends detected: {cycle}")]
    CyclicExtends { cycle: String },

    /// Error while gathering configuration from providers.
    #[error("Failed to gather configuration: {0}")]
    Gathering(#[from] Box<figment::Error>),

    /// Failure merging CLI values over configuration sources.
    #[error("Failed to merge CLI with configuration: {source}")]
    Merge {
        #[source]
        source: Box<figment::Error>,
    },

    /// Validation failures when building configuration.
    #[error("Validation failed for '{key}': {message}")]
    Validation { key: String, message: String },

    /// Multiple errors occurred while loading configuration.
    #[error("multiple configuration errors:\n{0}")]
    Aggregate(Box<AggregatedErrors>),
}

/// Collection of [`OrthoError`]s produced during a single load attempt.
///
/// # Examples
///
/// ```
/// use ortho_config::OrthoError;
/// let e = OrthoError::aggregate(vec![
///     OrthoError::Validation { key: "port".into(), message: "must be positive".into() },
///     clap::Error::raw(clap::error::ErrorKind::InvalidValue, "bad flag").into(),
/// ]);
/// if let OrthoError::Aggregate(agg) = e {
///     assert_eq!(agg.len(), 2);
/// }
/// ```
#[derive(Debug, Default)]
pub struct AggregatedErrors(Vec<Arc<OrthoError>>);

impl AggregatedErrors {
    /// Create a new aggregation from a vector of errors.
    #[must_use]
    pub fn new(errors: Vec<Arc<OrthoError>>) -> Self {
        Self(errors)
    }

    /// Iterate over the contained errors.
    #[must_use = "iterators should be consumed to inspect errors"]
    pub fn iter(&self) -> impl Iterator<Item = &OrthoError> {
        self.0.iter().map(AsRef::as_ref)
    }

    /// Number of errors in the aggregation.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Display for AggregatedErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{}: {e}", i + 1)?;
        }
        Ok(())
    }
}

impl Error for AggregatedErrors {}

impl OrthoError {
    /// Build an [`OrthoError`] from a list of errors.
    ///
    /// # Panics
    ///
    /// Panics if `errors` is empty.
    #[must_use]
    pub fn aggregate(errors: Vec<OrthoError>) -> Self {
        assert!(!errors.is_empty(), "aggregate requires at least one error");
        if errors.len() == 1 {
            errors.into_iter().next().expect("one error")
        } else {
            let arcs = errors.into_iter().map(Arc::new).collect();
            OrthoError::Aggregate(Box::new(AggregatedErrors::new(arcs)))
        }
    }

    /// Build an [`OrthoError`] from a list of already shared errors.
    ///
    /// Use when upstream code has already materialised `Arc<OrthoError>`
    /// instances.
    ///
    /// # Panics
    ///
    /// Panics if `errors` is empty. Callers must provide at least one error.
    #[must_use]
    pub fn aggregate_arcs(errors: Vec<Arc<OrthoError>>) -> Self {
        assert!(!errors.is_empty(), "aggregate requires at least one error");
        if errors.len() == 1 {
            match Arc::try_unwrap(errors.into_iter().next().expect("one error")) {
                Ok(e) => e,
                Err(shared) => OrthoError::Aggregate(Box::new(AggregatedErrors::new(vec![shared]))),
            }
        } else {
            OrthoError::Aggregate(Box::new(AggregatedErrors::new(errors)))
        }
    }

    /// Construct a merge error from a [`figment::Error`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ortho_config::OrthoError;
    /// let fe = figment::Error::from("boom");
    /// let e = OrthoError::merge(fe);
    /// assert!(matches!(e, OrthoError::Merge { .. }));
    /// ```
    #[must_use]
    pub fn merge(source: figment::Error) -> Self {
        OrthoError::Merge {
            source: Box::new(source),
        }
    }

    /// Construct a gathering error from a [`figment::Error`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ortho_config::OrthoError;
    /// let fe = figment::Error::from("boom");
    /// let e = OrthoError::gathering(fe);
    /// assert!(matches!(e, OrthoError::Gathering(_)));
    /// ```
    #[must_use]
    pub fn gathering(source: figment::Error) -> Self {
        OrthoError::Gathering(Box::new(source))
    }
}

/// Convert JSON encoding or decoding failures into
/// [`OrthoError::Gathering`].
impl From<serde_json::Error> for OrthoError {
    fn from(e: serde_json::Error) -> Self {
        OrthoError::Gathering(Box::new(figment::Error::from(format!(
            "JSON error: {} at line {}, column {}",
            e,
            e.line(),
            e.column()
        ))))
    }
}

impl From<clap::Error> for OrthoError {
    fn from(e: clap::Error) -> Self {
        OrthoError::CliParsing(e.into())
    }
}

impl From<figment::Error> for OrthoError {
    fn from(e: figment::Error) -> Self {
        OrthoError::Gathering(e.into())
    }
}

impl From<OrthoError> for FigmentError {
    /// Allow using `?` in tests and examples that return `figment::Error`.
    fn from(e: OrthoError) -> Self {
        match e {
            // Preserve the original Figment error (keeps kind, metadata, and sources).
            OrthoError::Merge { source: fe } | OrthoError::Gathering(fe) => *fe,
            // Fall back to a message for other variants.
            other => FigmentError::from(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OrthoError;

    #[test]
    #[should_panic(expected = "aggregate requires at least one error")]
    fn aggregate_panics_on_empty() {
        let _ = OrthoError::aggregate(vec![]);
    }
}
