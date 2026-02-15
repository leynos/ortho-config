//! Constructors and aggregation helpers for `OrthoError`.

use std::sync::Arc;

use figment::Error as FigmentError;

use super::{AggregatedErrors, OrthoError};

impl OrthoError {
    /// Tries to build an [`OrthoError`] from an iterator of errors.
    ///
    /// The iterator is consumed eagerly. It returns:
    /// * `None` when no errors are supplied;
    /// * the inner error when a single [`Arc`] is uniquely owned;
    /// * [`Self::Aggregate`] containing that single [`Arc`] when the error is already shared; and
    /// * [`Self::Aggregate`] combining every error for two or more inputs.
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
