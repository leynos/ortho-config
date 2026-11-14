//! Trybuild fixture for the standalone `RequiredConfigSchema` derive.
//!
//! Proves that a renamed crate path, a generic child with an explicit schema
//! bound, nested composition, and Serde naming all compile from a downstream
//! crate without an `OrthoConfig` loader on the child type.

use ortho_config as my_cfg;
use serde::{Deserialize, Serialize};

/// A generic nested object whose bound is declared by the caller.
#[derive(Debug, Deserialize, Serialize, my_cfg::RequiredConfigSchema)]
#[ortho_config(crate = "my_cfg")]
struct Wrapper<T>
where
    T: my_cfg::RequiredConfigSchema,
{
    /// The wrapped child schema.
    value: T,
}

/// A leaf child using a container rename rule and an alias.
#[derive(Debug, Deserialize, Serialize, my_cfg::RequiredConfigSchema)]
#[ortho_config(crate = "my_cfg")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct Child {
    /// Renamed at deserialization and accepted under an alias.
    #[serde(alias = "alt_host")]
    host_name: String,
}

/// A root composing the child schema and a wrapped child.
#[derive(Debug, Deserialize, Serialize, my_cfg::OrthoConfig)]
#[ortho_config(crate = "my_cfg", prefix = "TB_")]
struct Root {
    /// Optional nesting: an absent object deactivates the subtree.
    #[ortho_config(nested, skip_cli)]
    child: Option<Box<Child>>,
    /// A wrapped child that is not opted into schema composition.
    #[ortho_config(skip_cli)]
    wrapped: Option<Wrapper<Child>>,
}

fn main() {
    let _ = Root::merge_from_layers(std::iter::empty::<my_cfg::MergeLayer<'_>>());
}