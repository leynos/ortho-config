//! File-loading and layer-composition routines for `ConfigDiscovery`.

use std::borrow::Cow;
use std::io;
use std::path::Path;
use std::sync::Arc;

use camino::Utf8PathBuf;

use crate::{
    MergeLayer, OrthoError, OrthoMergeExt, OrthoResult, load_config_file, load_config_file_as_chain,
};

use super::outcome::DiscoveryOutcome;
use super::{ConfigDiscovery, DiscoveryLayerOutcome, DiscoveryLayersOutcome, DiscoveryLoadOutcome};

impl ConfigDiscovery {
    fn discover_first<T, F>(&self, mut build: F) -> DiscoveryOutcome<T>
    where
        F: FnMut(figment::Figment, &Path) -> Result<T, Arc<OrthoError>>,
    {
        let mut required_errors = Vec::new();
        let mut optional_errors = Vec::new();
        let (candidates, required_bound) = self.candidates_with_required_bound();
        for (idx, path) in candidates.into_iter().enumerate() {
            match load_config_file(&path) {
                Ok(Some(figment)) => match build(figment, &path) {
                    Ok(value) => {
                        return DiscoveryOutcome {
                            value: Some(value),
                            required_errors,
                            optional_errors,
                        };
                    }
                    Err(err) if idx < required_bound => required_errors.push(err),
                    Err(err) => optional_errors.push(err),
                },
                Ok(None) if idx < required_bound => {
                    required_errors.push(Self::missing_required_error(&path));
                }
                Ok(None) => {}
                Err(err) if idx < required_bound => required_errors.push(err),
                Err(err) => optional_errors.push(err),
            }
        }
        DiscoveryOutcome {
            value: None,
            required_errors,
            optional_errors,
        }
    }

    /// Records a missing candidate error if this is a required path.
    fn record_missing_candidate(
        idx: usize,
        required_bound: usize,
        path: &Path,
        required_errors: &mut Vec<Arc<OrthoError>>,
    ) {
        if idx < required_bound {
            required_errors.push(Self::missing_required_error(path));
        }
    }

    /// Returns true if the candidate at `idx` is required.
    const fn is_required_candidate(idx: usize, required_bound: usize) -> bool {
        idx < required_bound
    }

    /// Loads the first available configuration file using [`load_config_file`].
    ///
    /// # Behaviour
    ///
    /// Skips candidates that fail to load and continues scanning until an
    /// existing configuration file is parsed successfully.
    ///
    /// # Errors
    ///
    /// When every candidate fails, returns an error containing all recorded
    /// discovery diagnostics; if no candidates exist, returns `Ok(None)`.
    pub fn load_first(&self) -> OrthoResult<Option<figment::Figment>> {
        let (figment, errors) = self.load_first_with_errors();
        if let Some(found_figment) = figment {
            return Ok(Some(found_figment));
        }
        if let Some(err) = OrthoError::try_aggregate(errors) {
            return Err(Arc::new(err));
        }
        Ok(None)
    }

    /// Attempts to load the first available configuration file while partitioning errors.
    ///
    /// Required explicit candidates populate [`DiscoveryLoadOutcome::required_errors`]
    /// even when a later fallback succeeds, enabling callers to surface them eagerly.
    /// Optional candidates populate [`DiscoveryLoadOutcome::optional_errors`] so they
    /// can be reported once discovery exhausts every location.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::discovery::ConfigDiscovery;
    ///
    /// let discovery = ConfigDiscovery::builder("demo")
    ///     .add_required_path("missing.toml")
    ///     .build();
    /// let outcome = discovery.load_first_partitioned();
    /// assert!(outcome.figment.is_none());
    /// assert_eq!(outcome.required_errors.len(), 1);
    /// ```
    pub fn load_first_partitioned(&self) -> DiscoveryLoadOutcome {
        let outcome = self.discover_first(|figment, _| Ok(figment));
        DiscoveryLoadOutcome {
            figment: outcome.value,
            required_errors: outcome.required_errors,
            optional_errors: outcome.optional_errors,
        }
    }

    /// Composes the first available configuration file into a merge layer.
    ///
    /// Captures errors for required and optional candidates separately so
    /// callers can mirror the aggregation semantics of [`Self::load_first`].
    pub fn compose_layer(&self) -> DiscoveryLayerOutcome {
        let outcome = self.discover_first(|figment, path| {
            figment
                .extract::<crate::serde_json::Value>()
                .into_ortho_merge()
                .map(|value| {
                    let utf8_path = Utf8PathBuf::from_path_buf(path.to_path_buf())
                        .ok()
                        .unwrap_or_else(|| Utf8PathBuf::from(path.to_string_lossy().into_owned()));
                    MergeLayer::file(Cow::Owned(value), Some(utf8_path))
                })
        });
        DiscoveryLayerOutcome {
            value: outcome.value,
            required_errors: outcome.required_errors,
            optional_errors: outcome.optional_errors,
        }
    }

    /// Composes the first available configuration file into multiple merge layers.
    ///
    /// Unlike [`compose_layer`](Self::compose_layer), this method preserves each
    /// file in an `extends` chain as a separate layer. This allows declarative
    /// merge strategies (such as append for vectors) to be applied across the
    /// inheritance chain rather than using Figment's replacement semantics.
    ///
    /// Captures errors for required and optional candidates separately so
    /// callers can mirror the aggregation semantics of [`Self::load_first`].
    pub fn compose_layers(&self) -> DiscoveryLayersOutcome {
        let mut required_errors = Vec::new();
        let mut optional_errors = Vec::new();
        let (candidates, required_bound) = self.candidates_with_required_bound();

        for (idx, candidate_path) in candidates.into_iter().enumerate() {
            match load_config_file_as_chain(&candidate_path) {
                Ok(Some(chain)) => {
                    let layers = chain
                        .values
                        .into_iter()
                        .map(|(value, path)| MergeLayer::file(Cow::Owned(value), Some(path)))
                        .collect();
                    return DiscoveryLayersOutcome {
                        value: layers,
                        required_errors,
                        optional_errors,
                    };
                }
                Ok(None) => {
                    Self::record_missing_candidate(
                        idx,
                        required_bound,
                        &candidate_path,
                        &mut required_errors,
                    );
                }
                Err(err) if Self::is_required_candidate(idx, required_bound) => {
                    required_errors.push(err);
                }
                Err(err) => {
                    optional_errors.push(err);
                }
            }
        }

        DiscoveryLayersOutcome {
            value: Vec::new(),
            required_errors,
            optional_errors,
        }
    }

    /// Attempts to load the first available configuration file while collecting errors.
    #[must_use]
    pub fn load_first_with_errors(&self) -> (Option<figment::Figment>, Vec<Arc<OrthoError>>) {
        let DiscoveryLoadOutcome {
            figment,
            mut required_errors,
            mut optional_errors,
        } = self.load_first_partitioned();
        required_errors.append(&mut optional_errors);
        (figment, required_errors)
    }

    fn missing_required_error(path: &Path) -> Arc<OrthoError> {
        Arc::new(OrthoError::File {
            path: path.to_path_buf(),
            source: Box::new(io::Error::new(
                io::ErrorKind::NotFound,
                "required configuration file not found",
            )),
        })
    }
}
