//! A flattened field has no configuration key of its own, so the derive rejects
//! a shape that would otherwise parse the whole child as one CLI argument and
//! layer it under a key the configuration type never reads.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// A flattened child whose leaves sit at the root.
#[derive(Debug, Deserialize, Serialize)]
struct Child {
    host: String,
}

/// The generated CLI would treat `extra` as a single `--extra` argument.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct Root {
    #[serde(flatten)]
    extra: Child,
}

fn main() {}
