//! Behavioural test harness for the `hello_world` example using `rstest-bdd`.
//!
//! Fixtures and helpers live under [`harness`], while [`steps`] defines the
//! step implementations shared by the feature bindings in [`scenarios`].

use camino::Utf8PathBuf;
use std::time::Duration;

pub(crate) mod config;
pub(crate) mod harness;
pub(crate) mod steps;
mod scenarios;

pub(crate) const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const CONFIG_FILE: &str = ".hello_world.toml";
pub(crate) const ENV_PREFIX: &str = "HELLO_WORLD_";

pub(crate) fn binary_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!(
        "CARGO_BIN_EXE_hello_world",
        "Cargo must set the hello_world binary path for integration tests",
    ))
}
