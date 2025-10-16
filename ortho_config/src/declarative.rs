//! Declarative merging primitives used by the derive macro.
//!
//! The traits defined here allow configuration structs to be merged from a
//! sequence of declarative layers without exposing Figment in the public API.
//! Layers are represented as serialised [`serde_json::Value`] blobs so tests
//! and behavioural fixtures can compose deterministic inputs without touching
//! the filesystem.

use std::borrow::Cow;

use camino::Utf8PathBuf;
use serde_json::{Map, Value};

use crate::{OrthoResult, OrthoResultExt};

/// Provenance of a merge layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    pub fn defaults(value: Cow<'a, Value>) -> Self {
        Self {
            provenance: MergeProvenance::Defaults,
            value,
            path: None,
        }
    }

    /// Construct a layer originating from a configuration file.
    #[must_use]
    pub fn file(value: Cow<'a, Value>, path: Option<Utf8PathBuf>) -> Self {
        Self {
            provenance: MergeProvenance::File,
            value,
            path,
        }
    }

    /// Construct a layer originating from environment variables.
    #[must_use]
    pub fn environment(value: Cow<'a, Value>) -> Self {
        Self {
            provenance: MergeProvenance::Environment,
            value,
            path: None,
        }
    }

    /// Construct a layer originating from CLI arguments.
    #[must_use]
    pub fn cli(value: Cow<'a, Value>) -> Self {
        Self {
            provenance: MergeProvenance::Cli,
            value,
            path: None,
        }
    }

    /// Returns the provenance of the layer.
    #[must_use]
    pub fn provenance(&self) -> MergeProvenance {
        self.provenance
    }

    /// Returns the associated path if this layer was sourced from a file.
    #[must_use]
    pub fn path(&self) -> Option<&Utf8PathBuf> {
        self.path.as_ref()
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
#[derive(Default)]
pub struct MergeComposer {
    layers: Vec<MergeLayer<'static>>,
}

impl MergeComposer {
    /// Create an empty composer.
    #[must_use]
    pub fn new() -> Self {
        Self { layers: Vec::new() }
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

/// Trait implemented by derive-generated merge state machines.
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
/// # Panics
///
/// Panics when `target` contains a non-object value while merging an object
/// layer. This happens only when the caller tries to merge an object into a
/// scalar, which violates JSON object semantics.
pub fn merge_value(target: &mut Value, layer: Value) {
    match layer {
        Value::Object(map) => merge_object(target, map),
        Value::Array(_) | Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            *target = layer;
        }
    }
}

/// Merge the provided JSON object `map` into `target`.
///
/// This helper guarantees that `target` is an object before merging so the
/// recursive calls in [`merge_value`] do not need to repeat the initialisation
/// guard. It preserves the documented panic semantics by initialising
/// non-object targets to an empty map before reborrowing them mutably.
///
/// # Examples
///
/// ```ignore
/// use serde_json::{json, Map, Value};
///
/// let mut target = json!({"greeting": "hi"});
/// let mut incoming = Map::new();
/// incoming.insert("audience".into(), json!("world"));
/// merge_object(&mut target, incoming);
/// assert_eq!(target, json!({"greeting": "hi", "audience": "world"}));
/// ```
fn merge_object(target: &mut Value, map: Map<String, Value>) {
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let target_map = target.as_object_mut().expect("object after initialisation");
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
pub fn from_value<T: serde::de::DeserializeOwned>(value: Value) -> OrthoResult<T> {
    serde_json::from_value(value).into_ortho()
}
