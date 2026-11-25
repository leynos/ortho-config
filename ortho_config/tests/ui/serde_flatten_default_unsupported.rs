//! A flattened field's declared default would be layered under a key the
//! configuration type never reads, because Serde takes a flattened child's
//! leaves from the enclosing object. The derive rejects that mapping even when
//! the field is excluded from CLI parsing.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// A flattened child whose leaves sit at the root.
#[derive(Debug, Deserialize, Serialize)]
struct Child {
    host: String,
}

/// The default cannot be expressed under a single configuration key.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct Root {
    #[serde(flatten)]
    #[ortho_config(skip_cli, default = Child { host: String::from("localhost") })]
    extra: Child,
}

fn main() {}
