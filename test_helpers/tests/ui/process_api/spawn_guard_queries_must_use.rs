#![deny(unused_must_use)]

use ortho_config_test_helpers::process::spawn_guard::SpawnGuard;

fn main() {
    let guard = SpawnGuard::new();
    guard.root_child_seen();
    guard.kill_attempted();
    guard.kill_error();
}
