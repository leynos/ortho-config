//! File-loading and layer-composition routines for `ConfigDiscovery`.

use std::borrow::Cow;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::sync::Arc;

use camino::Utf8PathBuf;

use crate::file::canonicalise;
use crate::{
    MergeLayer, OrthoError, OrthoMergeExt, OrthoResult, load_config_file, load_config_file_as_chain,
};

use super::outcome::DiscoveryOutcome;
use super::telemetry;
use super::{
    AutomaticMode, ConfigDiscovery, DiscoveryLayerOutcome, DiscoveryLayersOutcome,
    DiscoveryLoadOutcome, DiscoveryScope,
};

/// Candidate errors partitioned by whether the candidate was required.
///
/// Bundling the two vectors keeps the recording helper within the project's
/// four-argument ceiling, but the real reason is that the partition and the
/// telemetry share one decision — whether the candidate was required. Making
/// that decision in a single place is what stops the emitted event and the
/// reported error disagreeing.
#[derive(Debug, Default)]
struct PartitionedErrors {
    required: Vec<Arc<OrthoError>>,
    optional: Vec<Arc<OrthoError>>,
}

/// Where and how one candidate failed, in bounded labels only.
///
/// Grouping the labels keeps [`PartitionedErrors::record`] within the
/// four-argument ceiling and keeps the telemetry decision in one place: the
/// error's category is derived from the error itself at the recording site,
/// so the emitted event and the stored error cannot disagree.
struct CandidateFailure {
    operation: &'static str,
    required: bool,
    source: &'static str,
}

/// One scope's first successful chain and its discovery diagnostics.
struct ScopeLayers {
    layers: Vec<MergeLayer<'static>>,
    errors: PartitionedErrors,
}

impl PartitionedErrors {
    fn record(&mut self, failure: &CandidateFailure, err: Arc<OrthoError>) {
        telemetry::candidate_failure(
            failure.operation,
            failure.required,
            failure.source,
            telemetry::error_category(&err),
        );
        if failure.required {
            self.required.push(err);
        } else {
            self.optional.push(err);
        }
    }

    fn into_outcome<T>(self, value: Option<T>) -> DiscoveryOutcome<T> {
        DiscoveryOutcome {
            value,
            required_errors: self.required,
            optional_errors: self.optional,
        }
    }

    fn into_layers_outcome(self, value: Vec<MergeLayer<'static>>) -> DiscoveryLayersOutcome {
        DiscoveryLayersOutcome {
            value,
            required_errors: self.required,
            optional_errors: self.optional,
        }
    }
}

impl ConfigDiscovery {
    /// Attempt one candidate, distinguishing "absent" from "failed".
    ///
    /// `Ok(None)` means the candidate simply is not there, which is ordinary
    /// for an optional location and an error only for a required one.
    fn try_candidate<T, F>(
        path: &Path,
        required: bool,
        build: &mut F,
    ) -> Result<Option<T>, Arc<OrthoError>>
    where
        F: FnMut(figment::Figment, &Path) -> Result<T, Arc<OrthoError>>,
    {
        match load_config_file(path)? {
            Some(figment) => build(figment, path).map(Some),
            None if required => Err(Self::missing_required_error(path)),
            None => Ok(None),
        }
    }

    /// Walk the candidates in order, stopping at the first that yields a value.
    ///
    /// The two discovery entry points differ only in what they attempt per
    /// candidate and what they build from the result; the traversal — ordering,
    /// the required/optional split, error routing, and the telemetry around it —
    /// is identical, and duplicating it once let the two drift in exactly the
    /// places a reader would assume they agree.
    fn walk_candidates<T>(
        &self,
        operation: &'static str,
        mut try_one: impl FnMut(&Path, bool) -> Result<Option<T>, Arc<OrthoError>>,
    ) -> (Option<T>, PartitionedErrors) {
        telemetry::attempt(operation);
        let mut errors = PartitionedErrors::default();
        let set = self.candidate_set();
        set.decisions.emit();
        for (idx, candidate) in set.candidates.into_iter().enumerate() {
            let required = Self::is_required_candidate(idx, set.required_bound);
            match try_one(&candidate.path, required) {
                Ok(Some(value)) => {
                    telemetry::load_outcome(
                        operation,
                        telemetry::OUTCOME_SUCCESS,
                        Some(candidate.source),
                    );
                    return (Some(value), errors);
                }
                Ok(None) => {}
                Err(err) => errors.record(
                    &CandidateFailure {
                        operation,
                        required,
                        source: candidate.source,
                    },
                    err,
                ),
            }
        }
        telemetry::load_outcome(operation, telemetry::OUTCOME_NOT_FOUND, None);
        (None, errors)
    }

    fn discover_first<T, F>(&self, mut build: F) -> DiscoveryOutcome<T>
    where
        F: FnMut(figment::Figment, &Path) -> Result<T, Arc<OrthoError>>,
    {
        let (value, errors) = self
            .walk_candidates(telemetry::OPERATION_DISCOVER_FIRST, |path, required| {
                Self::try_candidate(path, required, &mut build)
            });
        errors.into_outcome(value)
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
        let (value, errors) =
            self.walk_candidates(telemetry::OPERATION_COMPOSE_LAYERS, Self::chain_layers);
        errors.into_layers_outcome(value.unwrap_or_default())
    }

    /// Compose automatic file layers according to an explicit scope policy.
    ///
    /// [`AutomaticMode::FirstWins`] preserves [`Self::compose_layers`] exactly.
    /// [`AutomaticMode::StackScopes`] finds one successful `extends` chain per
    /// requested scope and appends scopes in order, allowing later scopes to
    /// override earlier scopes through the existing `MergeComposer` semantics.
    pub fn compose_scoped_layers(
        &self,
        mode: AutomaticMode,
        scopes: &[DiscoveryScope],
    ) -> DiscoveryLayersOutcome {
        if matches!(mode, AutomaticMode::FirstWins) {
            return self.compose_layers();
        }

        telemetry::attempt(telemetry::OPERATION_COMPOSE_LAYERS);
        let set = self.candidate_set();
        set.decisions.emit();
        let mut errors = PartitionedErrors::default();
        let mut layers = Vec::new();
        let mut loaded_paths = HashSet::new();

        for scope in scopes {
            let mut scope_layers = Self::compose_scope(*scope, &set, &mut loaded_paths);
            layers.append(&mut scope_layers.layers);
            errors.required.append(&mut scope_layers.errors.required);
            errors.optional.append(&mut scope_layers.errors.optional);
        }

        telemetry::load_outcome(
            telemetry::OPERATION_COMPOSE_LAYERS,
            if layers.is_empty() {
                telemetry::OUTCOME_NOT_FOUND
            } else {
                telemetry::OUTCOME_SUCCESS
            },
            None,
        );
        errors.into_layers_outcome(layers)
    }

    fn compose_scope(
        scope: DiscoveryScope,
        set: &super::candidate_set::CandidateSet,
        loaded_paths: &mut HashSet<std::path::PathBuf>,
    ) -> ScopeLayers {
        let mut errors = PartitionedErrors::default();
        for (index, candidate) in set.candidates.iter().enumerate() {
            if candidate.scope != Some(scope) {
                continue;
            }
            let required = Self::is_required_candidate(index, set.required_bound);
            match Self::chain_layers(&candidate.path, required) {
                Ok(Some(chain)) => {
                    let layers = Self::unique_layers(chain, loaded_paths);
                    telemetry::load_outcome(
                        telemetry::OPERATION_COMPOSE_LAYERS,
                        telemetry::OUTCOME_SUCCESS,
                        Some(candidate.source),
                    );
                    return ScopeLayers { layers, errors };
                }
                Ok(None) => {}
                Err(error) => errors.record(
                    &CandidateFailure {
                        operation: telemetry::OPERATION_COMPOSE_LAYERS,
                        required,
                        source: candidate.source,
                    },
                    error,
                ),
            }
        }
        ScopeLayers {
            layers: Vec::new(),
            errors,
        }
    }

    fn unique_layers(
        chain: Vec<MergeLayer<'static>>,
        loaded_paths: &mut HashSet<std::path::PathBuf>,
    ) -> Vec<MergeLayer<'static>> {
        chain
            .into_iter()
            .filter(|layer| Self::record_first_canonical_path(loaded_paths, layer))
            .collect()
    }

    /// Keep the earliest scope's copy of a successfully loaded file.
    ///
    /// The loader canonicalises every chain path, so canonical identity also
    /// collapses aliases and symlinks without changing public layer metadata.
    fn record_first_canonical_path(
        loaded_paths: &mut HashSet<std::path::PathBuf>,
        layer: &MergeLayer<'static>,
    ) -> bool {
        layer.path().is_none_or(|path| {
            canonicalise(path.as_std_path())
                .map_or(true, |canonical| loaded_paths.insert(canonical))
        })
    }

    /// Load one candidate's `extends` chain as a layer stack.
    fn chain_layers(
        path: &Path,
        required: bool,
    ) -> Result<Option<Vec<MergeLayer<'static>>>, Arc<OrthoError>> {
        match load_config_file_as_chain(path)? {
            Some(chain) => Ok(Some(
                chain
                    .values
                    .into_iter()
                    .map(|(value, layer_path)| {
                        MergeLayer::file(Cow::Owned(value), Some(layer_path))
                    })
                    .collect(),
            )),
            None if required => Err(Self::missing_required_error(path)),
            None => Ok(None),
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

    pub(super) fn missing_required_error(path: &Path) -> Arc<OrthoError> {
        Arc::new(OrthoError::File {
            path: path.to_path_buf(),
            source: Box::new(io::Error::new(
                io::ErrorKind::NotFound,
                "required configuration file not found",
            )),
        })
    }
}
