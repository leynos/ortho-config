//! Cucumber test harness for the `hello_world` example.

use std::time::Duration;

use camino::Utf8PathBuf;
use cucumber::{World as _, gherkin};

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

// Detect scenarios tagged with `@requires.yaml` so we can skip them when the
// corresponding Cargo feature is disabled.
fn requires_yaml(
    feature: &gherkin::Feature,
    rule: Option<&gherkin::Rule>,
    scenario: &gherkin::Scenario,
) -> bool {
    const TAG: &str = "requires.yaml";
    feature
        .tags
        .iter()
        .chain(rule.into_iter().flat_map(|r| r.tags.iter()))
        .chain(scenario.tags.iter())
        .any(|tag| tag == TAG)
}

#[tokio::main]
async fn main() {
    let yaml_enabled = cfg!(feature = "yaml");
    World::cucumber()
        .filter_run("tests/features", move |feature, rule, scenario| {
            yaml_enabled || !requires_yaml(feature, rule, scenario)
        })
        .await;
}
