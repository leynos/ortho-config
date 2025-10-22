//! Error types produced by the configuration loader.

use figment::Error as FigmentError;
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
        /// Path that triggered the configuration failure.
        path: std::path::PathBuf,
        /// Underlying error reported by the file loader.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Cycle detected while resolving `extends`.
    #[error("cyclic extends detected: {cycle}")]
    CyclicExtends {
        /// Chain of configuration files participating in the cycle.
        cycle: String,
    },

    /// Error while gathering configuration from providers.
    #[error("Failed to gather configuration: {0}")]
    Gathering(#[from] Box<FigmentError>),

    /// Failure merging CLI values over configuration sources.
    #[error("Failed to merge CLI with configuration: {source}")]
    Merge {
        /// Underlying error describing the merge failure.
        #[source]
        source: Box<FigmentError>,
    },

    /// Validation failures when building configuration.
    #[error("Validation failed for '{key}': {message}")]
    Validation {
        /// Configuration key that failed validation.
        key: String,
        /// Human-readable explanation of the validation failure.
        message: String,
    },

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
        self.0.iter().map(Arc::as_ref)
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

impl<'a> IntoIterator for &'a AggregatedErrors {
    type Item = &'a OrthoError;
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, Arc<OrthoError>>,
        fn(&'a Arc<OrthoError>) -> &'a OrthoError,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(Arc::as_ref)
    }
}

impl IntoIterator for AggregatedErrors {
    type Item = Arc<OrthoError>;
    type IntoIter = std::vec::IntoIter<Arc<OrthoError>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl OrthoError {
    /// Try to build an [`OrthoError`] from an iterator of errors.
    ///
    /// Returns `None` if the iterator is empty.
    ///
    /// # Panics
    ///
    /// This function never panics. If `Arc::try_unwrap` detects outstanding
    /// references when a single error is provided, the error is wrapped in an
    /// aggregate instead.
    #[must_use]
    pub fn try_aggregate<I, E>(errors: I) -> Option<Self>
    where
        I: IntoIterator<Item = E>,
        E: Into<Arc<Self>>,
    {
        let mut arcs: Vec<Arc<Self>> = errors.into_iter().map(Into::into).collect();
        if arcs.is_empty() {
            return None;
        }
        Some(if arcs.len() == 1 {
            let last = arcs.pop()?;
            match Arc::try_unwrap(last) {
                Ok(err) => err,
                Err(shared) => Self::Aggregate(Box::new(AggregatedErrors::new(vec![shared]))),
            }
        } else {
            Self::Aggregate(Box::new(AggregatedErrors::new(arcs)))
        })
    }

    /// Build an [`OrthoError`] from at least one error, each of which can be
    /// an `OrthoError` or an `Arc<OrthoError>`.
    ///
    /// # Panics
    ///
    /// Panics if `errors` is empty. Use [`OrthoError::try_aggregate`] to avoid panicking when the error list may be empty.
    #[must_use]
    #[track_caller]
    pub fn aggregate<I, E>(errors: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<Arc<Self>>,
    {
        Self::try_aggregate(errors).map_or_else(
            || panic!("aggregate requires at least one error"),
            |err| err,
        )
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
    pub fn merge(source: FigmentError) -> Self {
        Self::Merge {
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
    pub fn gathering(source: FigmentError) -> Self {
        Self::Gathering(Box::new(source))
    }

    /// Construct a gathering error from a [`figment::Error`] wrapped in an
    /// [`Arc`].
    ///
    /// This helper reduces repetition in call sites that need an
    /// `Arc<OrthoError>` (for example, when aggregating multiple errors).
    ///
    /// # Examples
    ///
    /// ```
    /// use ortho_config::OrthoError;
    /// let fe = figment::Error::from("boom");
    /// let e = OrthoError::gathering_arc(fe);
    /// assert!(matches!(&*e, OrthoError::Gathering(_)));
    /// ```
    #[must_use]
    pub fn gathering_arc(source: FigmentError) -> Arc<Self> {
        Arc::new(Self::gathering(source))
    }
}

/// Convert JSON encoding or decoding failures into
/// [`OrthoError::Gathering`].
impl From<serde_json::Error> for OrthoError {
    fn from(e: serde_json::Error) -> Self {
        Self::Gathering(Box::new(figment::Error::from(format!(
            "JSON error: {} at line {}, column {}",
            e,
            e.line(),
            e.column()
        ))))
    }
}

impl From<clap::Error> for OrthoError {
    fn from(e: clap::Error) -> Self {
        Self::CliParsing(e.into())
    }
}

impl From<FigmentError> for OrthoError {
    fn from(e: FigmentError) -> Self {
        Self::Gathering(e.into())
    }
}

impl From<OrthoError> for FigmentError {
    /// Allow using `?` in tests and examples that return `figment::Error`.
    fn from(e: OrthoError) -> Self {
        match e {
            // Preserve the original Figment error (keeps kind, metadata, and sources).
            OrthoError::Merge { source: fe } | OrthoError::Gathering(fe) => *fe,
            // Fall back to a message for other variants.
            other => Self::from(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OrthoError;
    use std::sync::Arc;

    fn run_aggregate_tests<F>(name: &str, f: F)
    where
        F: Fn(Vec<Arc<OrthoError>>) -> OrthoError,
    {
        // single-owned
        let err = Arc::new(OrthoError::Validation {
            key: "k".into(),
            message: "m".into(),
        });
        let single_owned = f(vec![err]);
        match single_owned {
            OrthoError::Validation { .. } => {}
            other => panic!("{name}: expected Validation, got {other:?}"),
        }

        // single-shared
        let shared = OrthoError::gathering_arc(figment::Error::from("boom"));
        let shared_result = f(vec![Arc::clone(&shared)]);
        if let OrthoError::Aggregate(aggregate) = shared_result {
            assert_eq!(aggregate.len(), 1);
        } else {
            panic!("{name}: expected Aggregate");
        }

        // multi
        let e1 = OrthoError::gathering_arc(figment::Error::from("one"));
        let e2 = OrthoError::gathering_arc(figment::Error::from("two"));
        let merged_result = f(vec![e1, e2]);
        if let OrthoError::Aggregate(aggregate) = merged_result {
            let aggregate_ref = aggregate.as_ref();
            let iter_items: Vec<_> = aggregate_ref.iter().collect();
            assert_eq!(iter_items.len(), 2);
            let mut borrowed_items = Vec::new();
            for e in aggregate_ref {
                borrowed_items.push(e);
            }
            assert_eq!(borrowed_items.len(), 2);
            let display = aggregate_ref.to_string();
            let owned_items: Vec<_> = aggregate.into_iter().collect();
            assert_eq!(owned_items.len(), 2);
            assert!(display.starts_with("1:"));
            assert!(display.contains("\n2:"));
        } else {
            panic!("{name}: expected Aggregate");
        }
    }

    #[test]
    fn aggregate_panics_on_empty() {
        let empty: Vec<Arc<OrthoError>> = vec![];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            OrthoError::aggregate(empty)
        }));
        assert!(result.is_err());
    }

    #[test]
    fn try_aggregate_none_on_empty() {
        assert!(OrthoError::try_aggregate(Vec::<Arc<OrthoError>>::new()).is_none());
    }

    #[test]
    fn both_aggregate_behaviours() {
        run_aggregate_tests("try_aggregate", |v| {
            OrthoError::try_aggregate(v).map_or_else(
                || panic!("expected error aggregation to yield a value"),
                |err| err,
            )
        });
        run_aggregate_tests("aggregate", OrthoError::aggregate);
    }
}
