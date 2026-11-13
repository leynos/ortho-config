//! Compile-fail fixture: an unbound `SpawnGuard` must be diagnosed.
//!
//! `process_api_compile_contracts` compares the output against
//! `spawn_guard_new_must_use.stderr`, which records this file's line numbers.

#![deny(unused_must_use)]

use ortho_config_test_helpers::process::spawn_guard::SpawnGuard;

fn main() {
    SpawnGuard::new();
}
