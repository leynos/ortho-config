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

mod composer;
mod convert;
mod layer;
mod merge;

pub use composer::{LayerComposition, MergeComposer};
pub use convert::{from_value, from_value_merge};
pub use layer::{MergeLayer, MergeProvenance};
pub use merge::{DeclarativeMerge, merge_value};
