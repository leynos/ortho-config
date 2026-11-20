//! Rejects two `profile_field` classifications on one field.
//!
//! A field with two labels has no single disclosure rule to apply, so the
//! second annotation is rejected at its own span rather than overwriting the
//! first.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
struct Bad {
    #[ortho_config(profile_field = "secret", profile_field = "reference_only")]
    token: String,
}

fn main() {}