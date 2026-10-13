//! Rejects a user field that serializes under the reserved `profile` key.
//!
//! Exercises collision projection (a) from the profile-opt-in contract, for
//! the case the field *name* does not reveal: opting in reserves the
//! `profile` root key, so a field renamed there by `#[serde(rename)]` is a
//! compile-time error even though the Rust identifier is something else.
//! `profile_field_name_collision.rs` covers the mirror case where the field is
//! literally named `profile`.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, OrthoConfig)]
#[ortho_config(profiles)]
struct Collides {
    #[serde(rename = "profile")]
    selected: Option<String>,
}

fn main() {}
