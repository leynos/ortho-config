//! Declarative merge trait and JSON merge mechanics.

use serde_json::{Map, Value};

use crate::OrthoResult;

use super::MergeLayer;

/// Trait implemented by derive-generated merge state machines.
///
/// # Feature Requirements
///
/// Declarative merging requires the `serde_json` feature (enabled by default).
/// The derive-generated implementations use [`serde_json::Value`] for layer
/// composition and will not compile without this feature.
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
/// - When merging an object into a non-object target, target is initialized to
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
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }

    let Some(target_map) = target.as_object_mut() else {
        return;
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
