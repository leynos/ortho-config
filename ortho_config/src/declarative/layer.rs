//! Layer metadata and transport values for declarative merges.

use std::borrow::Cow;

use camino::{Utf8Path, Utf8PathBuf};
use serde_json::Value;

/// Provenance of a merge layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MergeProvenance {
    /// Default values baked into the configuration struct.
    Defaults,
    /// Values loaded from configuration files.
    File,
    /// Values overlayed by a selected profile (roadmap 9.1.1).
    Profile,
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

    /// Construct a layer originating from a selected profile table.
    #[must_use]
    pub const fn profile(value: Cow<'a, Value>, path: Option<Utf8PathBuf>) -> Self {
        Self {
            provenance: MergeProvenance::Profile,
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

    /// Borrows the JSON value carried by the layer.
    ///
    /// Unlike [`MergeLayer::into_value`], this does not consume the layer, so
    /// extraction helpers can read a layer's value and still push the layer
    /// itself. Profile-table extraction (roadmap 9.1.1) uses this to derive
    /// profile layers from file layers without cloning the file value.
    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
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
