//! Layer composition helpers for declarative merging.

use std::borrow::Cow;
use std::sync::Arc;

use camino::Utf8PathBuf;
use serde_json::Value;

use crate::{OrthoError, OrthoResult};

use super::MergeLayer;

/// Builder that accumulates [`MergeLayer`] instances.
///
/// # Collection merge strategies
///
/// The composer honours field-level merge strategies emitted by the derive
/// macro. Vector fields append by default but can opt into replacement; maps
/// default to keyed merging and can be replaced wholesale.
///
/// ```rust
/// use ortho_config::declarative::MergeComposer;
/// use serde::{Deserialize, Serialize};
/// use serde_json::json;
/// use std::collections::BTreeMap;
///
/// #[derive(Debug, Deserialize, Serialize, ortho_config::OrthoConfig)]
/// struct Templates {
///     #[serde(default)]
///     #[ortho_config(merge_strategy = "replace")]
///     greetings: Vec<String>,
///     #[serde(default)]
///     #[ortho_config(merge_strategy = "replace")]
///     templates: BTreeMap<String, String>,
/// }
///
/// let mut composer = MergeComposer::new();
/// composer.push_defaults(json!({
///     "greetings": ["Hello"],
///     "templates": { "friendly": "Hello, {name}!" }
/// }));
/// composer.push_environment(json!({
///     "templates": { "formal": "Good day, {name}." }
/// }));
/// composer.push_cli(json!({
///     "greetings": ["Hi"],
///     "templates": { "casual": "Hey {name}!" }
/// }));
///
/// let cfg = Templates::merge_from_layers(composer.layers())?;
/// assert_eq!(cfg.greetings, vec![String::from("Hi")]);
/// assert_eq!(cfg.templates.len(), 1);
/// assert_eq!(
///     cfg.templates.get("casual"),
///     Some(&String::from("Hey {name}!"))
/// );
/// # Ok::<_, ortho_config::OrthoError>(())
/// ```
#[derive(Default)]
pub struct MergeComposer {
    layers: Vec<MergeLayer<'static>>,
}

impl MergeComposer {
    /// Create an empty composer.
    #[must_use]
    pub const fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Create a composer with preallocated capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            layers: Vec::with_capacity(capacity),
        }
    }

    /// Push a defaults layer.
    pub fn push_defaults(&mut self, value: Value) {
        self.push_layer(MergeLayer::defaults(Cow::Owned(value)));
    }

    /// Push a configuration file layer.
    pub fn push_file(&mut self, value: Value, path: Option<Utf8PathBuf>) {
        self.push_layer(MergeLayer::file(Cow::Owned(value), path));
    }

    /// Push an environment layer.
    pub fn push_environment(&mut self, value: Value) {
        self.push_layer(MergeLayer::environment(Cow::Owned(value)));
    }

    /// Push a CLI layer.
    pub fn push_cli(&mut self, value: Value) {
        self.push_layer(MergeLayer::cli(Cow::Owned(value)));
    }

    /// Push an arbitrary layer.
    pub fn push_layer(&mut self, layer: MergeLayer<'static>) {
        self.layers.push(layer);
    }

    /// Consume the composer and return the accumulated layers.
    #[must_use]
    pub fn layers(self) -> Vec<MergeLayer<'static>> {
        self.layers
    }
}

/// Result of composing configuration layers alongside any collected errors.
#[derive(Debug)]
pub struct LayerComposition {
    layers: Vec<MergeLayer<'static>>,
    errors: Vec<Arc<OrthoError>>,
}

impl LayerComposition {
    /// Create a new composition from `layers` and `errors`.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Constructing Vec-based compositions requires allocation"
    )]
    pub fn new(layers: Vec<MergeLayer<'static>>, errors: Vec<Arc<OrthoError>>) -> Self {
        Self { layers, errors }
    }

    /// Decompose the composition into its constituent parts.
    #[must_use]
    pub fn into_parts(self) -> (Vec<MergeLayer<'static>>, Vec<Arc<OrthoError>>) {
        (self.layers, self.errors)
    }

    /// Indicates whether any errors were captured while composing layers.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Borrowing the error buffer is not const in stable Rust"
    )]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    fn errors_to_result<T>(mut errors: Vec<Arc<OrthoError>>) -> OrthoResult<T> {
        if errors.len() == 1 {
            Err(errors.remove(0))
        } else {
            Err(Arc::new(OrthoError::aggregate(errors)))
        }
    }

    /// Consume the composition and merge layers using `merge`.
    ///
    /// Aggregates any pre-recorded composition errors with merge failures to
    /// mirror the behaviour of the generated `load_from_iter` path.
    ///
    /// # Errors
    ///
    /// Returns an aggregated [`crate::OrthoError`] when either composition or
    /// merge steps fail.
    pub fn into_merge_result<T, F>(self, merge: F) -> OrthoResult<T>
    where
        F: FnOnce(Vec<MergeLayer<'static>>) -> OrthoResult<T>,
    {
        let (layers, mut errors) = self.into_parts();
        match merge(layers) {
            Ok(cfg) => {
                if errors.is_empty() {
                    Ok(cfg)
                } else {
                    Self::errors_to_result(errors)
                }
            }
            Err(err) => {
                errors.push(err);
                Self::errors_to_result(errors)
            }
        }
    }
}

impl IntoIterator for MergeComposer {
    type Item = MergeLayer<'static>;
    type IntoIter = std::vec::IntoIter<MergeLayer<'static>>;

    fn into_iter(self) -> Self::IntoIter {
        self.layers.into_iter()
    }
}
