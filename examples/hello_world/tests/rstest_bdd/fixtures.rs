//! Fixtures shared by the `hello_world` rstest-bdd scaffolding.

use hello_world::cli::HelloWorldCli;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

#[derive(Debug, Default, ScenarioState)]
pub struct HelloWorldState {
    pub cli: Slot<HelloWorldCli>,
}

#[fixture]
pub fn hello_world_state() -> HelloWorldState {
    HelloWorldState::default()
}

#[fixture]
pub fn hello_world_binary() -> &'static str {
    "hello-world"
}
