//! Localisation-identifier collision fixture.
//!
//! Two fields whose clap argument ids normalise to the same Fluent segment
//! must be rejected by the derive. The second id is set explicitly because
//! clap's derived id is the raw Rust field name, not its kebab-cased
//! spelling, so two plain field names can only collide through case alone.

use clap::Parser;
use ortho_config::OrthoConfig;

#[derive(Parser, OrthoConfig)]
struct Colliding {
    foo_bar: String,
    #[arg(id = "FOO_BAR")]
    unrelated: String,
}
