//! Rejects a `profile_field` label outside the accepted vocabulary.
//!
//! Only `"secret"` and `"reference_only"` have a defined disclosure rule. An
//! unrecognised label is rejected at the label literal's own span, so a
//! misspelt classification cannot silently leave the field unprotected.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
struct Bad {
    #[ortho_config(profile_field = "public")]
    token: String,
}

fn main() {}