//! Compiles a schema-only derive beside a clap expression `id`.
//!
//! `#[arg(id = HOST_ID)]` is valid clap, but the derive cannot resolve a
//! constant id statically. The schema-only expansion emits no CLI metadata, so
//! it must not inspect that declaration. This fixture fails to compile if the
//! shared parser regains the clap-declaration walk for schema-only derives.

use clap::Parser;
use serde::{Deserialize, Serialize};

const HOST_ID: &str = "host";

#[derive(Debug, Deserialize, Serialize, Parser, ortho_config::RequiredConfigSchema)]
struct SchemaOnlyArgs {
    #[arg(id = HOST_ID, long)]
    host: Option<String>,
}

fn main() {}