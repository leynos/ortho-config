//! Compile-fail fixture: discarding a `SpawnGuard` query must be diagnosed.
//!
//! `process_api_compile_contracts` compares the output against
//! `spawn_guard_queries_must_use.stderr`, which records this file's line numbers.

#![deny(unused_must_use)]

use ortho_config_test_helpers::process::spawn_guard::SpawnGuard;

fn main() {
    let guard = SpawnGuard::new();
    guard.root_child_seen();
    guard.kill_attempted();
    guard.kill_error();
}
