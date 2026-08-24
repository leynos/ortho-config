//! Policy-driven configuration-file selection and scoped layer resolution.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::{MergeComposer, MergeLayer, OrthoError, OrthoResult, load_config_file_as_chain};

use super::{
    AutomaticMode, ConfigDiscovery, ConfigDiscoveryBuilder, DiscoveryLayersOutcome, DiscoveryScope,
};

/// An ordered explicit configuration-path selector.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ConfigPathSelector {
    path: Option<PathBuf>,
    environment_variable: Option<String>,
    label: String,
    legacy: bool,
}

impl ConfigPathSelector {
    /// Select a path supplied by a CLI adapter.
    #[must_use]
    pub fn cli(path: Option<PathBuf>) -> Self {
        Self {
            path,
            environment_variable: None,
            label: String::from("cli"),
            legacy: false,
        }
    }

    /// Select a path named by an environment variable.
    #[must_use]
    pub fn env(variable_name: impl Into<String>) -> Self {
        let environment_variable = variable_name.into();
        Self {
            path: None,
            environment_variable: Some(environment_variable.clone()),
            label: environment_variable,
            legacy: false,
        }
    }

    /// Give the selector a diagnostics label.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Mark the selector as a legacy compatibility rung.
    #[must_use]
    pub const fn legacy_alias(mut self) -> Self {
        self.legacy = true;
        self
    }

    fn resolve(&self, discovery: &ConfigDiscovery) -> Option<PathBuf> {
        self.path.clone().or_else(|| {
            self.environment_variable.as_ref().and_then(|name| {
                discovery
                    .env_source
                    .get(name)
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
            })
        })
    }
}

/// Behaviour when an explicit selector yields a path.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExplicitMode {
    /// A selected path is required and suppresses automatic discovery.
    #[default]
    RequiredExclusive,
    /// A missing selected path is ignored, while still suppressing automatic discovery.
    Optional,
}

/// The selector that won ordered explicit resolution.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ResolvedSelection {
    label: String,
    path: PathBuf,
    legacy: bool,
}

impl ResolvedSelection {
    /// Returns the winning selector label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the selected path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Returns whether the selector is a legacy alias.
    #[must_use]
    pub const fn legacy(&self) -> bool {
        self.legacy
    }
}

/// Replayable result of resolving file layers.
#[derive(Debug, Default)]
#[must_use]
#[non_exhaustive]
pub struct FileLayerOutcome {
    layers: Vec<MergeLayer<'static>>,
    selection: Option<ResolvedSelection>,
    origins: Vec<DiscoveryScope>,
    selected_error: Option<Arc<OrthoError>>,
    reportable_errors: Vec<Arc<OrthoError>>,
    ignorable_errors: Vec<Arc<OrthoError>>,
}

impl FileLayerOutcome {
    fn selected(selection: ResolvedSelection, mode: ExplicitMode) -> Self {
        let selected_path = selection.path.clone();
        match load_config_file_as_chain(&selected_path) {
            Ok(Some(chain)) => Self {
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
            Ok(None) if matches!(mode, ExplicitMode::Optional) => Self {
                selection: Some(selection),
                ..Self::default()
            },
            Ok(None) => Self {
                selection: Some(selection),
                selected_error: Some(ConfigDiscovery::missing_required_error(&selected_path)),
                ..Self::default()
            },
            Err(err) if matches!(mode, ExplicitMode::RequiredExclusive) => Self {
                selection: Some(selection),
                selected_error: Some(err),
                ..Self::default()
            },
            Err(err) => Self {
                selection: Some(selection),
                reportable_errors: vec![err],
                ..Self::default()
            },
        }
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

    /// Returns non-fatal errors that should always be reported.
    #[must_use]
    pub fn reportable_errors(&self) -> &[Arc<OrthoError>] {
        &self.reportable_errors
    }

    /// Returns scopes that contributed automatic file layers.
    #[must_use]
    pub fn origins(&self) -> &[DiscoveryScope] {
        &self.origins
    }

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
    discovery: ConfigDiscovery,
    selectors: Vec<ConfigPathSelector>,
    explicit_mode: ExplicitMode,
    automatic_mode: AutomaticMode,
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
    pub fn resolve_layers(&self) -> FileLayerOutcome {
        if let Some((selector, path)) = self.selectors.iter().find_map(|selector| {
            selector
                .resolve(&self.discovery)
                .map(|path| (selector, path))
        }) {
            return FileLayerOutcome::selected(
                ResolvedSelection {
                    label: selector.label.clone(),
                    path,
                    legacy: selector.legacy,
                },
                self.explicit_mode,
            );
        }

        let mut outcome = FileLayerOutcome::from(
            self.discovery
                .compose_scoped_layers(self.automatic_mode, &self.scope_order),
        );
        outcome.origins.clone_from(&self.scope_order);
        outcome
    }
}
