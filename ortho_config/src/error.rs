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
        path: std::path::PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Cycle detected while resolving `extends`.
    #[error("cyclic extends detected: {cycle}")]
    CyclicExtends { cycle: String },

    /// Error while gathering configuration from providers.
    #[error("Failed to gather configuration: {0}")]
    Gathering(#[from] Box<FigmentError>),

    /// Failure merging CLI values over configuration sources.
    #[error("Failed to merge CLI with configuration: {source}")]
    Merge {
        #[source]
        source: Box<FigmentError>,
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

impl<'a> IntoIterator for &'a AggregatedErrors {
    type Item = &'a OrthoError;
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, Arc<OrthoError>>,
        fn(&'a Arc<OrthoError>) -> &'a OrthoError,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(AsRef::as_ref)
    }
}

impl OrthoError {
    /// Try to build an [`OrthoError`] from an iterator of errors.
    ///
    /// Returns `None` if the iterator is empty.
    ///
    /// # Panics
    ///
    /// Panics only if `Arc::try_unwrap` fails after a single item is popped,
    /// which is logically unreachable.
    #[must_use]
    pub fn try_aggregate<I, E>(errors: I) -> Option<Self>
    where
        I: IntoIterator<Item = E>,
        E: Into<Arc<OrthoError>>,
    {
        let mut arcs: Vec<Arc<OrthoError>> = errors.into_iter().map(Into::into).collect();
        if arcs.is_empty() {
            return None;
        }
        Some(if arcs.len() == 1 {
            match Arc::try_unwrap(arcs.pop().unwrap()) {
                Ok(err) => err,
                Err(shared) => OrthoError::Aggregate(Box::new(AggregatedErrors::new(vec![shared]))),
            }
        } else {
            OrthoError::Aggregate(Box::new(AggregatedErrors::new(arcs)))
        })
    }

    /// Build an [`OrthoError`] from at least one error, each of which can be
    /// an `OrthoError` or an `Arc<OrthoError>`.
    ///
    /// # Panics
    ///
    /// Panics if `errors` is empty. Use [`OrthoError::try_aggregate`] to avoid panicking when the error list may be empty.
    #[must_use]
    pub fn aggregate<I, E>(errors: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<Arc<OrthoError>>,
    {
        Self::try_aggregate(errors).expect("aggregate requires at least one error")
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
    pub fn gathering(source: FigmentError) -> Self {
        OrthoError::Gathering(Box::new(source))
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

impl From<FigmentError> for OrthoError {
    fn from(e: FigmentError) -> Self {
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
    use std::sync::Arc;

    #[test]
    #[should_panic(expected = "aggregate requires at least one error")]
    fn aggregate_panics_on_empty() {
        let empty: Vec<OrthoError> = vec![];
        let _ = OrthoError::aggregate(empty);
    }

    #[test]
    fn try_aggregate_none_on_empty() {
        let errs: Vec<OrthoError> = vec![];
        assert!(OrthoError::try_aggregate(errs).is_none());
    }

    #[test]
    fn try_aggregate_single_owned_returns_inner() {
        let err = OrthoError::Validation {
            key: "k".into(),
            message: "m".into(),
        };
        let res = OrthoError::try_aggregate(vec![err]).expect("one");
        match res {
            OrthoError::Validation { .. } => {}
            other => panic!("expected Validation, got: {other:?}"),
        }
    }

    #[test]
    fn try_aggregate_single_shared_returns_aggregate() {
        let err = OrthoError::gathering_arc(figment::Error::from("boom"));
        let res = OrthoError::try_aggregate(vec![Arc::clone(&err)]).expect("aggregate");
        match res {
            OrthoError::Aggregate(agg) => assert_eq!(agg.len(), 1),
            other => panic!("expected Aggregate, got: {other:?}"),
        }
    }

    #[test]
    fn try_aggregate_multi_formats_numbered_lines() {
        let e1 = OrthoError::gathering(figment::Error::from("one"));
        let e2 = OrthoError::gathering(figment::Error::from("two"));
        let res = OrthoError::try_aggregate(vec![e1, e2]).expect("aggregate");
        match res {
            OrthoError::Aggregate(agg) => {
                let items: Vec<_> = (&agg).into_iter().collect();
                assert_eq!(items.len(), 2);
                let display = agg.to_string();
                assert!(display.starts_with("1:"));
                assert!(display.contains("\n2:"));
            }
            other => panic!("expected Aggregate, got: {other:?}"),
        }
    }

    #[test]
    fn aggregate_single_owned_returns_inner() {
        let err = OrthoError::Validation {
            key: "k".into(),
            message: "m".into(),
        };
        let res = OrthoError::aggregate(vec![err]);
        match res {
            OrthoError::Validation { .. } => {}
            other => panic!("expected Validation, got: {other:?}"),
        }
    }

    #[test]
    fn aggregate_single_shared_returns_aggregate() {
        let err = OrthoError::gathering_arc(figment::Error::from("boom"));
        let res = OrthoError::aggregate(vec![Arc::clone(&err)]);
        match res {
            OrthoError::Aggregate(agg) => assert_eq!(agg.len(), 1),
            other => panic!("expected Aggregate, got: {other:?}"),
        }
    }

    #[test]
    fn aggregate_multi_formats_numbered_lines() {
        let e1 = OrthoError::gathering(figment::Error::from("one"));
        let e2 = OrthoError::gathering(figment::Error::from("two"));
        let res = OrthoError::aggregate(vec![e1, e2]);
        match res {
            OrthoError::Aggregate(agg) => {
                let items: Vec<_> = (&agg).into_iter().collect();
                assert_eq!(items.len(), 2);
                let display = agg.to_string();
                assert!(display.starts_with("1:"));
                assert!(display.contains("\n2:"));
            }
            other => panic!("expected Aggregate, got: {other:?}"),
        }
    }
}
