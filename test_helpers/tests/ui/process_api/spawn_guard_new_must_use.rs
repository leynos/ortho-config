#![deny(unused_must_use)]

use ortho_config_test_helpers::process::spawn_guard::SpawnGuard;

fn main() {
    SpawnGuard::new();
}
