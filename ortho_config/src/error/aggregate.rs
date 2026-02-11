//! Aggregation container and iteration support for multiple `OrthoError` values.

use std::{error::Error, fmt, sync::Arc};

use super::OrthoError;

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
    pub const fn new(errors: Vec<Arc<OrthoError>>) -> Self {
        Self(errors)
    }

    /// Iterate over the contained errors.
    #[must_use = "iterators should be consumed to inspect errors"]
    pub fn iter(&self) -> impl Iterator<Item = &OrthoError> {
        self.0.iter().map(Arc::as_ref)
    }

    /// Number of errors in the aggregation.
    #[must_use]
    pub const fn len(&self) -> usize {
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
