//! Rejects `profile_field` on a flattened container.
//!
//! The docs builder does not expand a flattened container into descendant
//! metadata, so accepting the annotation would promise protection the emitted
//! metadata cannot provide. The leaf fields on the child type must be marked
//! instead.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Credentials {
    token: String,
}

#[derive(Deserialize, OrthoConfig)]
struct Bad {
    #[ortho_config(profile_field = "secret")]
    #[serde(flatten)]
    credentials: Credentials,
}

fn main() {}