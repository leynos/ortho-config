//! `#[ortho_config(nested)]` binds only the marked field's type to the schema
//! trait. A child that never opted in must therefore fail to compile, rather
//! than silently contributing an empty schema.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// A child that never opted into schema composition.
#[derive(Debug, Deserialize, Serialize)]
struct Child {
    host: String,
}

/// The root opts the unqualified child into composition.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "UI_")]
struct Root {
    /// The child must implement `RequiredConfigSchema`.
    #[ortho_config(nested, skip_cli)]
    child: Child,
}

fn main() {}