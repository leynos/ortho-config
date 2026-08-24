//! Cross-platform configuration file discovery helpers.
//!
//! Applications can use [`ConfigDiscovery`] to enumerate configuration file
//! candidates in the same order exercised by the `hello_world` example. The
//! helper inspects explicit paths, XDG directories, Windows application data
//! folders, the user's home directory and project roots.

use std::path::PathBuf;
use std::sync::Arc;

use crate::env_source::SharedEnvSource;
use crate::{MergeLayer, OrthoError};

mod builder;
mod candidate_set;
mod candidates;
mod load;
mod outcome;
mod policy;
mod scope;
mod telemetry;

pub use builder::ConfigDiscoveryBuilder;
pub use policy::{
    ConfigFilePolicy, ConfigPathSelector, ExplicitMode, FileLayerOutcome, ResolvedSelection,
};
pub use scope::{AutomaticMode, DiscoveryScope};

/// Cross-platform configuration discovery helper mirroring the `hello_world` example.
#[derive(Clone)]
pub struct ConfigDiscovery {
    env_var: Option<String>,
    explicit_paths: Vec<PathBuf>,
    required_explicit_paths: Vec<PathBuf>,
    app_name: String,
    config_file_name: String,
    dotfile_name: String,
    project_file_name: String,
    project_roots: Vec<PathBuf>,
    env_source: SharedEnvSource,
}

/// Debug output omits the environment source and every path.
///
/// The source may hold secret-shaped values, and this project treats paths
/// as sensitive in diagnostics, so only names and counts are printed.
impl std::fmt::Debug for ConfigDiscovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigDiscovery")
            .field("env_var", &self.env_var)
            .field("app_name", &self.app_name)
            .field("config_file_name", &self.config_file_name)
            .field("dotfile_name", &self.dotfile_name)
            .field("project_file_name", &self.project_file_name)
            .field("explicit_paths", &self.explicit_paths.len())
            .field(
                "required_explicit_paths",
                &self.required_explicit_paths.len(),
            )
            .field("project_roots", &self.project_roots.len())
            .finish_non_exhaustive()
    }
}

/// Result of a discovery attempt that keeps required and optional errors separate.
///
/// Callers can surface [`DiscoveryLoadOutcome::required_errors`] regardless of whether a configuration
/// file eventually loads, while deferring [`DiscoveryLoadOutcome::optional_errors`] until fallbacks are
/// exhausted. This mirrors the builder contract where required explicit paths
/// must exist.
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
#[derive(Debug, Default)]
#[must_use]
pub struct DiscoveryLoadOutcome {
    /// Successfully loaded configuration file, if any.
    pub figment: Option<figment::Figment>,
    /// Errors originating from required explicit candidates.
    pub required_errors: Vec<Arc<OrthoError>>,
    /// Errors produced by optional discovery candidates.
    pub optional_errors: Vec<Arc<OrthoError>>,
}

/// Generic composition result that captures a discovered value along with errors.
///
/// This type unifies single-layer and multi-layer discovery outcomes, avoiding
/// duplication of error-handling logic.
#[derive(Debug, Default)]
#[must_use]
pub struct LayerDiscoveryOutcome<T> {
    /// Successfully composed value, if any.
    pub value: T,
    /// Errors originating from required explicit candidates.
    pub required_errors: Vec<Arc<OrthoError>>,
    /// Errors produced by optional discovery candidates.
    pub optional_errors: Vec<Arc<OrthoError>>,
}

/// Composition result that captures the first discovered configuration layer.
pub type DiscoveryLayerOutcome = LayerDiscoveryOutcome<Option<MergeLayer<'static>>>;

/// Composition result that captures multiple file layers from an extends chain.
///
/// When a configuration file uses `extends`, each file in the inheritance chain
/// is returned as a separate layer. This allows declarative merge strategies
/// (such as append for vectors) to be applied across the chain.
pub type DiscoveryLayersOutcome = LayerDiscoveryOutcome<Vec<MergeLayer<'static>>>;

impl ConfigDiscovery {
    /// Creates a new builder initialised for `app_name`.
    #[must_use]
    pub fn builder(app_name: impl Into<String>) -> ConfigDiscoveryBuilder {
        ConfigDiscoveryBuilder::new(app_name)
    }
}

#[cfg(test)]
mod telemetry_test_support;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod dedup_tests;
