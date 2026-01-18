//! Emits `OrthoConfig` documentation metadata for the hello world demo.

use ortho_config::docs::OrthoConfigDocs;
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metadata = hello_world::cli::GlobalArgs::get_doc_metadata();
    let json = ortho_config::serde_json::to_string_pretty(&metadata)?;
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{json}")?;
    Ok(())
}
