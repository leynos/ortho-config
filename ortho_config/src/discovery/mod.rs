//! Cross-platform configuration file discovery helpers.
//!
//! Applications can use [`ConfigDiscovery`] to enumerate configuration file
//! candidates in the same order exercised by the `hello_world` example. The
//! helper inspects explicit paths, XDG directories, Windows application data
//! folders, the user's home directory and project roots.

use std::path::PathBuf;
use std::sync::Arc;

use crate::{MergeLayer, OrthoError};

mod builder;
mod candidates;
mod load;
mod outcome;

pub use builder::ConfigDiscoveryBuilder;

/// Cross-platform configuration discovery helper mirroring the `hello_world` example.
#[derive(Debug, Clone)]
pub struct ConfigDiscovery {
    env_var: Option<String>,
    explicit_paths: Vec<PathBuf>,
    required_explicit_paths: Vec<PathBuf>,
    app_name: String,
    config_file_name: String,
    dotfile_name: String,
    project_file_name: String,
    project_roots: Vec<PathBuf>,
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
mod tests;

#[cfg(test)]
mod dedup_tests;
