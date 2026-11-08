//! Compile-fail fixture: discarding a `Reader` query must be diagnosed.
//!
//! `process_api_compile_contracts` compares the output against
//! `reader_queries_must_use.stderr`, which records this file's line numbers.

#![deny(unused_must_use)]

use std::process::{Command, Stdio};

use ortho_config_test_helpers::process::capture::Reader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new("cargo");
    command.stdout(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("child stdout was not captured"))?;
    let reader = Reader::spawn("stdout", stdout, 1024, |_| {})?;
    reader.is_finished();
    reader.stream();
    reader.error();
    reader.was_cancelled();
    reader.snapshot();
    Ok(())
}
