//! Compile-fail fixture: structured-output stdout purpose token
//!
//! The `stdout` leaf accepts only the four declared purposes. An invented
//! token must be rejected at the derive, because the wire keeps a tolerant
//! `unknown` fallback for consumers and this fixture proves the authoring side
//! does not inherit that tolerance.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(output(stdout = "matrix"))]
struct Bad {
    value: u8,
}

/// Compile-fail harness needs a `main`, but compilation stops at the
/// attribute that rejects the unrecognised `stdout` value. The pinned
/// diagnostic is compared verbatim from the sibling `.stderr` file.
fn main() {}