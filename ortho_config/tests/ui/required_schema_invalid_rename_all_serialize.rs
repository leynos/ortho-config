//! A directional `rename_all` value is rejected in both directions.
//!
//! The schema reads only the deserialization rule, but an unsupported name is
//! a mistake in the declaration and is reported whichever direction carries
//! it.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[serde(rename_all(serialize = "Sentence case"))]
struct Root {
    reference: String,
}

fn main() {}