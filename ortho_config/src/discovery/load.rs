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
use super::telemetry;
use super::{ConfigDiscovery, DiscoveryLayerOutcome, DiscoveryLayersOutcome, DiscoveryLoadOutcome};

/// Candidate errors partitioned by whether the candidate was required.
///
/// Bundling the two vectors keeps the recording helper within the project's
/// four-argument ceiling, but the real reason is that the partition and the
/// telemetry share one decision — whether the candidate was required. Making
/// that decision in a single place is what stops the emitted event and the
/// reported error disagreeing.
#[derive(Debug, Default)]
pub(super) struct PartitionedErrors {
    /// Failures from required candidates, retained even if a fallback succeeds.
    required: Vec<Arc<OrthoError>>,
    /// Failures from optional candidates, kept separate for caller policy.
    optional: Vec<Arc<OrthoError>>,
}

/// Where and how one candidate failed, in bounded labels only.
///
/// Grouping the labels keeps [`PartitionedErrors::record`] within the
/// four-argument ceiling and keeps the telemetry decision in one place: the
/// error's category is derived from the error itself at the recording site,
/// so the emitted event and the stored error cannot disagree.
///
/// `scope` is the scope whose walk met the candidate. The flat first-wins walk
/// has no scope concept and passes `None`, so the field is omitted from its
/// events rather than rendered empty.
pub(super) struct CandidateFailure {
    /// Bounded operation label identifying which discovery action failed.
    pub(super) operation: &'static str,
    /// Whether absence or parse failure must be reported as required.
    pub(super) required: bool,
    /// Bounded candidate source label, never a path or user value.
    pub(super) source: &'static str,
    /// Scope label for scoped walks; absent for the flat first-wins walk.
    pub(super) scope: Option<&'static str>,
}

impl PartitionedErrors {
    /// Emits bounded failure telemetry and stores the error in its policy bucket.
    ///
    /// # Examples
    ///
    /// Recording an error with `required: true` appends it to
    /// `required_errors`; an optional failure is retained separately.
    pub(super) fn record(&mut self, failure: &CandidateFailure, err: Arc<OrthoError>) {
        telemetry::candidate_failure(&telemetry::CandidateFailureFields {
            operation: failure.operation,
            required: failure.required,
            source: failure.source,
            category: telemetry::error_category(&err),
            scope: failure.scope,
        });
        if failure.required {
            self.required.push(err);
        } else {
            self.optional.push(err);
        }
    }

    /// Transfers both error partitions into the generic single-value result.
    fn into_outcome<T>(self, value: Option<T>) -> DiscoveryOutcome<T> {
        DiscoveryOutcome {
            value,
            required_errors: self.required,
            optional_errors: self.optional,
        }
    }

    /// Moves both error buckets into the result for a composed layer stack.
    ///
    /// # Examples
    ///
    /// Converting an empty partition with two layers preserves both layers and
    /// returns empty required and optional error lists.
    pub(super) fn into_layers_outcome(
        self,
        value: Vec<MergeLayer<'static>>,
    ) -> DiscoveryLayersOutcome {
        DiscoveryLayersOutcome {
            value,
            required_errors: self.required,
            optional_errors: self.optional,
        }
    }

    /// Merge another partition into this one, preserving each bucket's order.
    ///
    /// Scope stacking accumulates diagnostics across scopes, and which bucket an
    /// error lands in is the whole point of the split; appending here keeps the
    /// caller from having to name the two fields and risk transposing them.
    pub(super) fn append(&mut self, mut other: Self) {
        self.required.append(&mut other.required);
        self.optional.append(&mut other.optional);
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

    /// Walk the candidates in order, keeping the first value that loads.
    ///
    /// The two discovery entry points differ only in what they attempt per
    /// candidate and what they build from the result; the traversal — ordering,
    /// the required/optional split, error routing, and the telemetry around it —
    /// is identical, and duplicating it once let the two drift in exactly the
    /// places a reader would assume they agree.
    ///
    /// The first success normally ends the walk. A remaining required candidate
    /// is the exception: its absence or failure must still be reported, even
    /// though the value already found keeps its precedence. That is what stops
    /// an optional explicit path, which is generated before the selector, from
    /// hiding a broken or missing required selector.
    fn walk_candidates<T>(
        &self,
        operation: &'static str,
        mut try_one: impl FnMut(&Path, bool) -> Result<Option<T>, Arc<OrthoError>>,
    ) -> (Option<T>, PartitionedErrors) {
        telemetry::attempt(operation);
        let mut errors = PartitionedErrors::default();
        let set = self.candidate_set();
        set.decisions.emit();
        for err in &set.assembly_errors {
            errors.record(
                &CandidateFailure {
                    operation,
                    required: true,
                    source: telemetry::CANDIDATE_SELECTOR,
                    scope: None,
                },
                Arc::clone(err),
            );
        }
        let mut winner: Option<(T, &'static str)> = None;
        for candidate in set.candidates {
            let required = candidate.required;
            // Once a value has loaded, an optional candidate can neither
            // override it nor contribute a reportable error, so skip it. A
            // required candidate still has to be checked.
            if winner.is_some() && !required {
                continue;
            }
            match try_one(&candidate.path, required) {
                Ok(Some(value)) => {
                    // Keep the first success; a later required candidate that
                    // loads merely satisfies its must-exist contract.
                    winner = winner.or(Some((value, candidate.source)));
                }
                Ok(None) => {}
                Err(err) => errors.record(
                    &CandidateFailure {
                        operation,
                        required,
                        source: candidate.source,
                        scope: None,
                    },
                    err,
                ),
            }
        }
        let outcome = winner
            .as_ref()
            .map_or(telemetry::OUTCOME_NOT_FOUND, |_| telemetry::OUTCOME_SUCCESS);
        let source = winner.as_ref().map(|(_, source)| *source);
        telemetry::load_outcome(operation, outcome, source, None);
        (winner.map(|(value, _)| value), errors)
    }

    /// Runs the shared first-success traversal and maps its value and errors.
    ///
    /// The builder runs only for a parsed candidate. Required failures remain
    /// in the outcome even when a later optional candidate produces a value;
    /// [`Self::load_first`] is what turns that retained error into a failure.
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

    /// Loads the first available configuration file using [`load_config_file`].
    ///
    /// # Behaviour
    ///
    /// Optional candidates that fail to load are skipped so a later candidate
    /// may still succeed. A required candidate is different: a path added with
    /// [`ConfigDiscoveryBuilder::add_required_path`], or a non-empty path named
    /// by the selector configured with
    /// [`ConfigDiscoveryBuilder::env_var`], must load. Its absence or failure
    /// is returned even when a later fallback loads successfully.
    ///
    /// An unset or empty selector value selects nothing, so it does not fail
    /// closed and automatic discovery continues normally. A relative value the
    /// environment source cannot anchor is a required failure, never a fallback
    /// to the process's working directory.
    ///
    /// # Errors
    ///
    /// Returns an error when any required candidate fails, or when no candidate
    /// loaded and an optional candidate recorded an error. The error aggregates
    /// the recorded required diagnostics; optional diagnostics join it only
    /// when nothing loaded. Returns `Ok(None)` when there were no candidates,
    /// or when every candidate was optional and absent.
    ///
    /// [`ConfigDiscoveryBuilder::add_required_path`]: super::ConfigDiscoveryBuilder::add_required_path
    /// [`ConfigDiscoveryBuilder::env_var`]: super::ConfigDiscoveryBuilder::env_var
    pub fn load_first(&self) -> OrthoResult<Option<figment::Figment>> {
        let DiscoveryLoadOutcome {
            figment,
            mut required_errors,
            mut optional_errors,
        } = self.load_first_partitioned();
        let has_figment = figment.is_some();
        if required_errors.is_empty()
            && let Some(found_figment) = figment
        {
            return Ok(Some(found_figment));
        }
        if !has_figment {
            // With no value to return, optional diagnostics are worth
            // reporting alongside the required ones; a loaded fallback
            // suppresses them.
            required_errors.append(&mut optional_errors);
        }
        if let Some(err) = OrthoError::try_aggregate(required_errors) {
            return Err(Arc::new(err));
        }
        Ok(None)
    }

    /// Attempts to load the first available configuration file while partitioning errors.
    ///
    /// Required candidates populate [`DiscoveryLoadOutcome::required_errors`]
    /// even when a later fallback succeeds, enabling callers to surface them
    /// eagerly. Optional candidates populate
    /// [`DiscoveryLoadOutcome::optional_errors`] so they can be reported once
    /// discovery exhausts every location.
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

    /// Load one candidate's `extends` chain as a layer stack.
    ///
    /// Shared with [`ConfigDiscovery::compose_scoped_layers`]: both modes must
    /// expand a chain identically, and the only difference between them is how
    /// many candidates they apply it to.
    pub(super) fn chain_layers(
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

    /// Builds the not-found error used when a required candidate is absent.
    ///
    /// # Examples
    ///
    /// Passing `settings.toml` produces a file error carrying that path and a
    /// not-found cause for the caller to aggregate or return.
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
