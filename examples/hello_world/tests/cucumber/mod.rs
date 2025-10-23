//! Cucumber test harness for the `hello_world` example.

use std::time::Duration;

use camino::Utf8PathBuf;
use cucumber::World as _;

mod config;
#[path = "../steps/mod.rs"]
mod steps;
mod world;

pub use config::SampleConfigError;
pub use world::World;

pub(crate) const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const CONFIG_FILE: &str = ".hello_world.toml";
pub(crate) const ENV_PREFIX: &str = "HELLO_WORLD_";

fn binary_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!(
        "CARGO_BIN_EXE_hello_world",
        "Cargo must set the hello_world binary path for integration tests",
    ))
}

#[tokio::main]
async fn main() {
    World::run("tests/features").await;
}
