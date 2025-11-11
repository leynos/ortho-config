//! Fixtures shared by the `hello_world` rstest-bdd scaffolding.
//!
//! Exposes the `HelloWorldState` scenario storage plus the `hello_world_state`
//! and `hello_world_binary` fixtures consumed by rstest-bdd scenarios.

use hello_world::cli::HelloWorldCli;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

#[derive(Debug, Default, ScenarioState)]
pub struct HelloWorldState {
    pub cli: Slot<HelloWorldCli>,
}

/// Provides a resettable scenario state shared across hello_world steps.
#[fixture]
pub fn hello_world_state() -> HelloWorldState {
    HelloWorldState::default()
}

/// Supplies the canonical binary name so steps can build CLI arg lists.
#[fixture]
pub fn hello_world_binary() -> &'static str {
    "hello-world"
}
