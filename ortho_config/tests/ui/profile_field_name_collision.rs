//! Rejects a user field serializing under the reserved `profile` root key.
//!
//! Exercises collision projection (a) from the profile-opt-in contract:
//! opting in reserves the `profile` root key, so a user field that serializes
//! as `profile` is a compile-time error rather than having the selector
//! overwrite the user's value.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, OrthoConfig)]
#[ortho_config(profiles)]
struct Collides {
    profile: Option<String>,
}

fn main() {}