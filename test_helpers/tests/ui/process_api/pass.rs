//! Compile-pass fixture for the public process supervision helpers.
//!
//! `process_api_compile_contracts` compiles this crate root and then runs it,
//! so `main` only binds the uncalled exercise function: the fixture proves the
//! public signatures compile without spawning a child or needing `cargo`.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ortho_config_test_helpers::process::{
    capture::Reader,
    run_with_deadline,
    run_with_deadline_until,
    spawn_guard::SpawnGuard,
};

/// Exercise every public process API so the fixture fails to compile on drift.
///
/// The trybuild harness runs a `pass` fixture after compiling it, and the
/// deadline helpers spawn real children. Keeping the calls in an uncalled
/// function means the fixture only proves the signatures at compile time.
fn exercise_public_api() -> Result<(), Box<dyn std::error::Error>> {
    let guard = SpawnGuard::new();
    let _root_child_seen = guard.root_child_seen();
    let _kill_attempted = guard.kill_attempted();
    let _kill_error = guard.kill_error();

    let mut command = Command::new("cargo");
    command.stdout(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("child stdout was not captured"))?;
    let mut reader = Reader::spawn("stdout", stdout, 1024, |_| {})?;
    let _snapshot = reader.snapshot();
    let _stream = reader.stream();
    let _is_finished = reader.is_finished();
    let _error = reader.error();
    let _was_cancelled = reader.was_cancelled();
    reader.request_stop();
    reader.join_until(Instant::now() + Duration::from_millis(1));
    let _detached = reader.detach_if_running();

    let _output = run_with_deadline(
        Command::new("cargo"),
        Duration::from_secs(1),
        "compile-pass fixture",
    )?;
    let _output = run_with_deadline_until(
        Command::new("cargo"),
        Duration::from_secs(1),
        Instant::now() + Duration::from_secs(1),
        "compile-pass fixture",
    )?;
    Ok(())
}

fn main() {
    // Bind without calling: the fixture is a compile-time contract, not a
    // runtime probe, so it needs no `cargo` on `PATH` and no deadline budget.
    let _api: fn() -> Result<(), Box<dyn std::error::Error>> = exercise_public_api;
}
