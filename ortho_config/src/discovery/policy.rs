//! Policy-driven configuration-file selection and scoped layer resolution.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::{MergeComposer, MergeLayer, OrthoError, OrthoResult, load_config_file_as_chain};

use super::policy_selector::{ConfigPathSelector, ExplicitMode, ResolvedSelection};
use super::{
    AutomaticMode, ConfigDiscovery, ConfigDiscoveryBuilder, DiscoveryLayersOutcome, DiscoveryScope,
    telemetry,
};

/// Replayable result of resolving file layers.
#[derive(Debug, Default)]
#[must_use]
#[non_exhaustive]
pub struct FileLayerOutcome {
    /// Owned file layers retained until a consumer drains or consumes
    /// the outcome.
    layers: Vec<MergeLayer<'static>>,
    /// Winning explicit selector, present even when its selected file fails.
    selection: Option<ResolvedSelection>,
    /// Automatic scopes that contributed at least one surviving file layer.
    origins: Vec<DiscoveryScope>,
    /// Fatal explicit-selection failure; it takes precedence over other
    /// diagnostics.
    selected_error: Option<Arc<OrthoError>>,
    /// Diagnostics always surfaced, including optional-mode
    /// selected-file errors.
    reportable_errors: Vec<Arc<OrthoError>>,
    /// Optional-candidate failures surfaced only when no file layer loaded.
    ignorable_errors: Vec<Arc<OrthoError>>,
}

impl FileLayerOutcome {
    /// Load the selected path as a replayable layer chain.
    ///
    /// Each arm yields its terminal outcome alongside the value it builds, so
    /// the emitted event and the returned diagnostics are decided once and
    /// cannot disagree. The outcome is drawn from the existing `OUTCOME_*`
    /// vocabulary — selection adds no second outcome set — and `mode` only
    /// decides what a failure means: `optional_failure` under
    /// [`ExplicitMode::Optional`], `required_failure` otherwise.
    fn selected(selection: ResolvedSelection, mode: ExplicitMode) -> Self {
        let selected_path = selection.path_buf();
        let (outcome, this) = match load_config_file_as_chain(&selected_path) {
            Ok(Some(chain)) => (
                telemetry::OUTCOME_SUCCESS,
                Self {
                    layers: chain
                        .values
                        .into_iter()
                        .map(|(file_value, layer_path)| {
                            MergeLayer::file(Cow::Owned(file_value), Some(layer_path))
                        })
                        .collect(),
                    selection: Some(selection),
                    ..Self::default()
                },
            ),
            Ok(None) if matches!(mode, ExplicitMode::Optional) => (
                telemetry::OUTCOME_NOT_FOUND,
                Self {
                    selection: Some(selection),
                    ..Self::default()
                },
            ),
            Ok(None) => (
                telemetry::OUTCOME_REQUIRED_FAILURE,
                Self {
                    selection: Some(selection),
                    selected_error: Some(ConfigDiscovery::missing_required_error(&selected_path)),
                    ..Self::default()
                },
            ),
            Err(err) if matches!(mode, ExplicitMode::RequiredExclusive) => (
                telemetry::OUTCOME_REQUIRED_FAILURE,
                Self {
                    selection: Some(selection),
                    selected_error: Some(err),
                    ..Self::default()
                },
            ),
            Err(err) => (
                telemetry::OUTCOME_OPTIONAL_FAILURE,
                Self {
                    selection: Some(selection),
                    reportable_errors: vec![err],
                    ..Self::default()
                },
            ),
        };
        telemetry::load_outcome(telemetry::OPERATION_POLICY_RESOLVE, outcome, None, None);
        this
    }

    /// Build a scalar-only early-preview object from loaded file layers.
    #[must_use]
    pub fn merged_file_value(&self) -> Value {
        let mut result = Map::new();
        for layer in &self.layers {
            Self::insert_scalar_values(&mut result, layer.clone().into_value());
        }
        Value::Object(result)
    }

    /// Add only top-level scalar members to the early-preview map.
    ///
    /// # Example
    ///
    /// Given `{"port": 8080, "nested": {"enabled": true}, "tags": ["dev"]}`,
    /// this keeps `port` and skips the object and array; later scalar members
    /// replace earlier values with the same key.
    fn insert_scalar_values(result: &mut Map<String, Value>, file_value: Value) {
        let Value::Object(object) = file_value else {
            return;
        };
        result.extend(
            object
                .into_iter()
                .filter(|(_, entry)| !entry.is_array() && !entry.is_object()),
        );
    }

    /// Drain the layers into an existing composer.
    pub fn push_into(&mut self, composer: &mut MergeComposer) {
        for layer in std::mem::take(&mut self.layers) {
            composer.push_layer(layer);
        }
    }

    /// Returns the winning explicit selector, when one was supplied.
    #[must_use]
    pub const fn selection(&self) -> Option<&ResolvedSelection> {
        self.selection.as_ref()
    }

    /// Returns the fatal selected-file error, when selection failed closed.
    #[must_use]
    pub const fn selected_error(&self) -> Option<&Arc<OrthoError>> {
        self.selected_error.as_ref()
    }

    /// Returns whether resolution produced no file layer at all.
    ///
    /// Public because a consumer must be able to ask this *before* consuming
    /// the outcome: [`Self::into_layers_and_errors`] takes `self` and returns
    /// the layers, so the emptiness of the chain is not otherwise observable
    /// once the errors have been routed. Profile extraction needs it to tell a
    /// chain that was searched and found wanting from one that never loaded.
    #[must_use]
    pub const fn layers_are_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Returns non-fatal errors that should always be reported.
    #[must_use]
    pub fn reportable_errors(&self) -> &[Arc<OrthoError>] {
        &self.reportable_errors
    }

    /// Returns failures that are only surfaced when no layer loaded.
    ///
    /// A consumer asking "did anything fail" has to consult this alongside
    /// [`Self::reportable_errors`]: an optional candidate that failed on the
    /// way to a later success lands here, and its presence is what
    /// distinguishes an empty chain that was searched from one where no
    /// candidate existed at all.
    #[must_use]
    pub fn ignorable_errors(&self) -> &[Arc<OrthoError>] {
        &self.ignorable_errors
    }

    /// Returns scopes that contributed automatic file layers, in scope order.
    ///
    /// A scope appears once at least one of its layers survives into the
    /// composed layer list, so a requested scope whose candidates were all
    /// absent — or whose every layer was already contributed by an earlier
    /// scope — is not an origin. Under
    /// [`AutomaticMode::FirstWins`](crate::AutomaticMode::FirstWins) this is
    /// always empty, because that mode walks a flat candidate list and scope
    /// has no bearing on what loads. A failed explicit selection likewise
    /// reports none: selection is not automatic discovery.
    #[must_use]
    pub fn origins(&self) -> &[DiscoveryScope] {
        &self.origins
    }

    /// Route diagnostics according to selected-file and fallback outcomes.
    ///
    /// # Example
    ///
    /// If an optional candidate fails but a later candidate loads, its
    /// ignorable error is withheld; if no layer loads, that error is returned.
    /// A fatal selected-file error instead clears the layers and stands alone.
    fn drain_errors(&mut self) -> Vec<Arc<OrthoError>> {
        if let Some(error) = self.selected_error.take() {
            self.layers.clear();
            return vec![error];
        }
        let mut errors = std::mem::take(&mut self.reportable_errors);
        if self.layers.is_empty() {
            errors.append(&mut self.ignorable_errors);
        }
        errors
    }

    /// Consume file layers, appending errors according to their visibility policy.
    pub fn into_layers_and_errors(
        mut self,
        errors: &mut Vec<Arc<OrthoError>>,
    ) -> Vec<MergeLayer<'static>> {
        errors.append(&mut self.drain_errors());
        std::mem::take(&mut self.layers)
    }

    /// Consume the outcome into either layers or an aggregated error.
    ///
    /// # Errors
    ///
    /// Returns the selected error alone, or aggregates reportable errors and
    /// ignorable errors when no layer was loaded.
    pub fn into_result(mut self) -> OrthoResult<Vec<MergeLayer<'static>>> {
        let errors = self.drain_errors();
        if let Some(error) = OrthoError::try_aggregate(errors) {
            return Err(Arc::new(error));
        }
        Ok(std::mem::take(&mut self.layers))
    }
}

impl From<DiscoveryLayersOutcome> for FileLayerOutcome {
    fn from(outcome: DiscoveryLayersOutcome) -> Self {
        Self {
            layers: outcome.value,
            reportable_errors: outcome.required_errors,
            ignorable_errors: outcome.optional_errors,
            ..Self::default()
        }
    }
}

/// Declarative policy that resolves explicit selectors or automatic scopes.
#[derive(Debug)]
#[non_exhaustive]
pub struct ConfigFilePolicy {
    /// Owned discovery inputs, including roots and the lookup environment
    /// source.
    discovery: ConfigDiscovery,
    /// Explicit path selectors tried in order before automatic discovery.
    selectors: Vec<ConfigPathSelector>,
    /// Failure policy applied after the first selector yields a path.
    explicit_mode: ExplicitMode,
    /// Determines whether automatic candidates are flat or scope-stacked.
    automatic_mode: AutomaticMode,
    /// Scope precedence used when automatic discovery stacks scopes.
    scope_order: Vec<DiscoveryScope>,
}

impl ConfigFilePolicy {
    /// Build a policy from the normal discovery builder.
    #[must_use]
    pub fn from_builder(builder: ConfigDiscoveryBuilder) -> Self {
        Self {
            discovery: builder.build(),
            selectors: Vec::new(),
            explicit_mode: ExplicitMode::RequiredExclusive,
            automatic_mode: AutomaticMode::FirstWins,
            scope_order: vec![
                DiscoveryScope::System,
                DiscoveryScope::User,
                DiscoveryScope::Project,
            ],
        }
    }

    /// Set the ordered explicit selector chain.
    #[must_use]
    pub fn selectors(mut self, selectors: impl IntoIterator<Item = ConfigPathSelector>) -> Self {
        self.selectors = selectors.into_iter().collect();
        self
    }

    /// Set explicit-selection behaviour.
    #[must_use]
    pub const fn explicit_mode(mut self, mode: ExplicitMode) -> Self {
        self.explicit_mode = mode;
        self
    }

    /// Set automatic-discovery behaviour.
    #[must_use]
    pub const fn automatic_mode(mut self, mode: AutomaticMode) -> Self {
        self.automatic_mode = mode;
        self
    }

    /// Set the scope order used for automatic stacking.
    #[must_use]
    pub fn scope_order(mut self, scopes: impl IntoIterator<Item = DiscoveryScope>) -> Self {
        self.scope_order = scopes.into_iter().collect();
        self
    }

    /// Replace automatic project roots with one caller-selected root.
    #[must_use]
    pub fn project_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.discovery.project_roots = vec![root.into()];
        self
    }

    /// Resolve selector and discovery layers without discarding diagnostics.
    ///
    /// A policy bypasses the candidate machinery and emits none of the
    /// per-candidate events discovery otherwise would: the attempt, the
    /// resolution and the outcome below are this path's whole visibility.
    pub fn resolve_layers(&self) -> FileLayerOutcome {
        if let Some((selector, path)) = self.selectors.iter().find_map(|selector| {
            selector
                .resolve(&self.discovery)
                .map(|path| (selector, path))
        }) {
            telemetry::attempt(telemetry::OPERATION_POLICY_RESOLVE);
            telemetry::policy_resolution(Some(selector.class()));
            return FileLayerOutcome::selected(
                ResolvedSelection::new(
                    selector.label_text().to_owned(),
                    path,
                    selector.is_legacy(),
                ),
                self.explicit_mode,
            );
        }

        telemetry::policy_resolution(None);
        let (layers, origins) = self
            .discovery
            .compose_scoped_layers_with_origins(self.automatic_mode, &self.scope_order);
        let mut outcome = FileLayerOutcome::from(layers);
        outcome.origins = origins;
        outcome
    }
}
