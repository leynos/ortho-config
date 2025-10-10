use crate::{OrthoError, OrthoResult};
use figment::Error as FigmentError;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Trait implemented by generated merge state machines.
pub trait DeclarativeMerge: Sized {
    /// Final configuration type produced after merging all layers.
    type Output;

    /// Merge a single configuration layer into the accumulated state.
    ///
    /// # Errors
    ///
    /// Returns an [`OrthoError`] when the layer payload is invalid.
    fn merge_layer(&mut self, layer: MergeLayer<'_>) -> OrthoResult<()>;

    /// Finalise the merge, producing the concrete configuration type.
    ///
    /// # Errors
    ///
    /// Returns an [`OrthoError`] when deserialisation of the merged value fails.
    fn finish(self) -> OrthoResult<Self::Output>;
}

/// Describes the source of a configuration layer.
#[derive(Clone, Debug)]
pub enum MergeSource<'a> {
    /// Default values generated from struct attributes.
    Defaults,
    /// Configuration parsed from a file discovered on disk.
    File { path: Cow<'a, Path> },
    /// Environment variable overrides.
    Environment,
    /// Command-line overrides.
    Cli,
}

impl<'a> MergeSource<'a> {
    /// Builds a file layer reference from any path-like value.
    pub fn file<P>(path: P) -> Self
    where
        P: Into<Cow<'a, Path>>,
    {
        MergeSource::File { path: path.into() }
    }
}

impl fmt::Display for MergeSource<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeSource::Defaults => write!(f, "defaults"),
            MergeSource::Environment => write!(f, "environment"),
            MergeSource::Cli => write!(f, "CLI"),
            MergeSource::File { path } => write!(f, "file '{}'", path.display()),
        }
    }
}

/// Borrowed or owned JSON value describing a configuration layer.
#[derive(Clone, Debug)]
pub struct MergeLayer<'a> {
    source: MergeSource<'a>,
    value: Cow<'a, Value>,
}

impl<'a> MergeLayer<'a> {
    /// Creates a new layer from the provided source and JSON payload.
    #[must_use]
    pub fn new(source: MergeSource<'a>, value: Cow<'a, Value>) -> Self {
        Self { source, value }
    }

    /// Creates a defaults layer from an owned value.
    #[must_use]
    pub fn defaults(value: Value) -> Self {
        Self::new(MergeSource::Defaults, Cow::Owned(value))
    }

    /// Creates a file layer from an owned value and path.
    #[must_use]
    pub fn file(path: PathBuf, value: Value) -> Self {
        Self::new(MergeSource::file(path), Cow::Owned(value))
    }

    /// Creates an environment layer from an owned value.
    #[must_use]
    pub fn environment(value: Value) -> Self {
        Self::new(MergeSource::Environment, Cow::Owned(value))
    }

    /// Creates a CLI layer from an owned value.
    #[must_use]
    pub fn cli(value: Value) -> Self {
        Self::new(MergeSource::Cli, Cow::Owned(value))
    }

    /// Returns the layer source.
    #[must_use]
    pub fn source(&self) -> &MergeSource<'a> {
        &self.source
    }

    /// Returns the underlying JSON payload.
    #[must_use]
    pub fn value(&self) -> &Value {
        self.value.as_ref()
    }
}

fn make_merge_error(msg: String) -> Arc<OrthoError> {
    Arc::new(OrthoError::merge(FigmentError::from(msg)))
}

fn expect_object<'a>(
    value: &'a Value,
    source: &MergeSource<'_>,
) -> OrthoResult<&'a Map<String, Value>> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(make_merge_error(format!(
            "{source} layer must be a JSON object to support declarative merging"
        ))),
    }
}

fn merge_value(
    target: &mut Map<String, Value>,
    value: &Value,
    source: &MergeSource<'_>,
) -> OrthoResult<()> {
    let map = expect_object(value, source)?;
    merge_map(target, map);
    Ok(())
}

fn merge_map(target: &mut Map<String, Value>, overlay: &Map<String, Value>) {
    for (key, value) in overlay {
        match value {
            Value::Object(child) => match target.get_mut(key) {
                Some(Value::Object(existing)) => merge_map(existing, child),
                _ => {
                    target.insert(key.clone(), Value::Object(child.clone()));
                }
            },
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

fn json_to_config<T: DeserializeOwned>(map: Map<String, Value>) -> OrthoResult<T> {
    serde_json::from_value(Value::Object(map)).map_err(|err| {
        make_merge_error(format!("failed to deserialize merged configuration: {err}"))
    })
}

/// Merge the provided layer into the accumulator map.
///
/// # Errors
///
/// Returns an [`OrthoError`] if the layer payload cannot be merged.
pub fn merge_layer_into_map(
    accumulator: &mut Map<String, Value>,
    layer: &MergeLayer<'_>,
) -> OrthoResult<()> {
    merge_value(accumulator, layer.value(), layer.source())
}

/// Convert an accumulated JSON map into the desired configuration type.
///
/// # Errors
///
/// Returns an [`OrthoError`] when the merged payload cannot be deserialised.
pub fn merged_map_into<T: DeserializeOwned>(map: Map<String, Value>) -> OrthoResult<T> {
    json_to_config(map)
}
