//! Declarative merging primitives used by the derive macro.
//!
//! The traits defined here allow configuration structs to be merged from a
//! sequence of declarative layers without exposing Figment in the public API.
//! Layers are represented as serialised [`serde_json::Value`] blobs so tests
//! and behavioural fixtures can compose deterministic inputs without touching
//! the filesystem. See the
//! [declarative merging design](https://github.com/leynos/ortho-config/blob/main/docs/design.md#introduce-declarative-configuration-merging)
//! for the architectural context and trade-offs.
//!
//! # Example
//!
//! ```rust
//! use ortho_config::declarative::{MergeComposer, MergeLayer};
//! use ortho_config::{DeclarativeMerge, OrthoConfig};
//! use serde::{Deserialize, Serialize};
//! use serde_json::json;
//!
//! #[derive(Debug, Deserialize, Serialize, OrthoConfig)]
//! #[ortho_config(prefix = "APP")]
//! struct AppConfig {
//!     #[ortho_config(default = 3000)]
//!     port: u16,
//! }
//!
//! let mut composer = MergeComposer::new();
//! composer.push_defaults(json!({"port": 3000}));
//! composer.push_cli(json!({"port": 4000}));
//!
//! let config = AppConfig::merge_from_layers(composer.layers())
//!     .expect("layers merge successfully");
//! assert_eq!(config.port, 4000);
//! ```
//!
//! The derive generates an internal state machine that implements
//! [`DeclarativeMerge`], so `merge_from_layers` can iterate through
//! [`MergeLayer`] values deterministically.

use std::{borrow::Cow, sync::Arc};

use camino::{Utf8Path, Utf8PathBuf};
use serde_json::{Map, Value};

use crate::{OrthoError, OrthoResult};

/// Provenance of a merge layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MergeProvenance {
    /// Default values baked into the configuration struct.
    Defaults,
    /// Values loaded from configuration files.
    File,
    /// Values collected from environment variables.
    Environment,
    /// Values supplied on the command line.
    Cli,
}

/// Representation of a configuration layer.
#[derive(Clone, Debug)]
pub struct MergeLayer<'a> {
    provenance: MergeProvenance,
    value: Cow<'a, Value>,
    path: Option<Utf8PathBuf>,
}

impl<'a> MergeLayer<'a> {
    /// Construct a layer originating from default values.
    #[must_use]
    pub const fn defaults(value: Cow<'a, Value>) -> Self {
        Self {
            provenance: MergeProvenance::Defaults,
            value,
            path: None,
        }
    }

    /// Construct a layer originating from a configuration file.
    #[must_use]
    pub const fn file(value: Cow<'a, Value>, path: Option<Utf8PathBuf>) -> Self {
        Self {
            provenance: MergeProvenance::File,
            value,
            path,
        }
    }

    /// Construct a layer originating from environment variables.
    #[must_use]
    pub const fn environment(value: Cow<'a, Value>) -> Self {
        Self {
            provenance: MergeProvenance::Environment,
            value,
            path: None,
        }
    }

    /// Construct a layer originating from CLI arguments.
    #[must_use]
    pub const fn cli(value: Cow<'a, Value>) -> Self {
        Self {
            provenance: MergeProvenance::Cli,
            value,
            path: None,
        }
    }

    /// Returns the provenance of the layer.
    #[must_use]
    pub const fn provenance(&self) -> MergeProvenance {
        self.provenance
    }

    /// Returns the associated path if this layer was sourced from a file.
    #[must_use]
    pub fn path(&self) -> Option<&Utf8Path> {
        self.path.as_deref()
    }

    /// Returns an owned JSON value representing the layer.
    #[must_use]
    pub fn into_value(self) -> Value {
        self.value.into_owned()
    }

    /// Convert this layer into a `'static` owned variant.
    #[must_use]
    pub fn into_owned(self) -> MergeLayer<'static> {
        MergeLayer {
            provenance: self.provenance,
            value: Cow::Owned(self.value.into_owned()),
            path: self.path,
        }
    }
}

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

/// Trait implemented by derive-generated merge state machines.
///
/// # Example
///
/// ```rust
/// use ortho_config::declarative::{from_value, merge_value, MergeComposer, MergeLayer};
/// use ortho_config::DeclarativeMerge;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct AppSettings {
///     port: u16,
/// }
///
/// #[derive(Default)]
/// struct AppSettingsMerge {
///     buffer: serde_json::Value,
/// }
///
/// impl DeclarativeMerge for AppSettingsMerge {
///     type Output = AppSettings;
///
///     fn merge_layer(&mut self, layer: MergeLayer<'_>) -> ortho_config::OrthoResult<()> {
///         merge_value(&mut self.buffer, layer.into_value());
///         Ok(())
///     }
///
///     fn finish(self) -> ortho_config::OrthoResult<Self::Output> {
///         from_value(self.buffer)
///     }
/// }
///
/// let mut composer = MergeComposer::new();
/// composer.push_defaults(json!({"port": 3000}));
/// composer.push_cli(json!({"port": 4000}));
///
/// let mut merge = AppSettingsMerge::default();
/// for layer in composer.layers() {
///     merge.merge_layer(layer)?;
/// }
/// let settings = merge.finish()?;
/// assert_eq!(settings.port, 4000);
/// # Ok::<_, ortho_config::OrthoError>(())
/// ```
pub trait DeclarativeMerge: Sized {
    /// Output type returned after applying all layers.
    type Output;

    /// Merge an additional layer into the accumulated state.
    ///
    /// # Errors
    ///
    /// Implementations may return an [`crate::OrthoError`] when a layer cannot be
    /// deserialised or validated.
    fn merge_layer(&mut self, layer: MergeLayer<'_>) -> OrthoResult<()>;

    /// Finalise the merge, returning the built configuration.
    ///
    /// # Errors
    ///
    /// Returns an [`crate::OrthoError`] when the accumulated state cannot be
    /// transformed into the final configuration.
    fn finish(self) -> OrthoResult<Self::Output>;
}

/// Overlay `layer` onto `target`, updating `target` in place.
///
/// Behaviour:
/// - When merging an object into a non-object target, target is initialised to
///   `{}` first.
/// - Objects are merged recursively (keys are added or overwritten, and nested
///   objects are overlaid).
/// - Arrays and scalars replace `target` wholesale (no deep merge for arrays).
///
/// # Examples
///
/// ```rust
/// use ortho_config::declarative::merge_value;
/// use serde_json::json;
///
/// let mut acc = json!({"a": 1, "b": {"x": 1}});
/// merge_value(&mut acc, json!({"b": {"y": 2}, "c": 3}));
/// assert_eq!(acc, json!({"a": 1, "b": {"x": 1, "y": 2}, "c": 3}));
///
/// // Arrays replace existing values.
/// merge_value(&mut acc, json!({"b": [1, 2, 3]}));
/// assert_eq!(acc["b"], json!([1, 2, 3]));
/// ```
pub fn merge_value(target: &mut Value, layer: Value) {
    match layer {
        Value::Object(map) => merge_object(target, map),
        _ => *target = layer,
    }
}

/// Merge the provided JSON object `map` into `target`.
///
/// Behaviour mirrors [`merge_value`]: non-object targets are converted to empty
/// objects, nested objects merge recursively, and other types replace existing
/// entries. Library users normally experience these semantics via
/// [`merge_value`]; the example below demonstrates the helper's behaviour using
/// that public entrypoint so the doctest compiles against the crate surface.
///
/// # Examples
///
/// ```rust
/// use ortho_config::declarative::merge_value;
/// use serde_json::json;
///
/// let mut target = json!({"greeting": "hi"});
/// merge_value(&mut target, json!({"audience": "world"}));
/// assert_eq!(target, json!({"greeting": "hi", "audience": "world"}));
///
/// // Nested objects merge recursively.
/// merge_value(
///     &mut target,
///     json!({"nested": {"emphasis": "wave"}}),
/// );
/// assert_eq!(target["nested"], json!({"emphasis": "wave"}));
/// ```
fn merge_object(target: &mut Value, map: Map<String, Value>) {
    #[expect(
        clippy::option_if_let_else,
        reason = "initialising target object when absent requires mutable borrow"
    )]
    let target_map = if let Some(map_ref) = target.as_object_mut() {
        map_ref
    } else {
        *target = Value::Object(Map::new());
        #[expect(
            clippy::expect_used,
            reason = "target was just initialised to an object"
        )]
        target.as_object_mut().expect("target is now an object")
    };
    for (key, value) in map {
        match target_map.get_mut(&key) {
            Some(existing) => merge_value(existing, value),
            None => {
                target_map.insert(key, value);
            }
        }
    }
}

/// Deserialise a JSON [`Value`] into `T`.
///
/// # Errors
///
/// Returns an [`crate::OrthoError`] when deserialisation fails.
///
/// # Examples
///
/// ```rust
/// use ortho_config::declarative::from_value;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct App { port: u16 }
///
/// let v = json!({"port": 8080});
/// let app: App = from_value(v).expect("value deserialises");
/// assert_eq!(app.port, 8080);
/// ```
pub fn from_value<T: serde::de::DeserializeOwned>(value: Value) -> OrthoResult<T> {
    serde_json::from_value(value).map_err(|err| {
        let message = format!(
            "merge deserialisation failed: {err} at line {}, column {}",
            err.line(),
            err.column()
        );
        Arc::new(OrthoError::merge(crate::figment::Error::from(message)))
    })
}
