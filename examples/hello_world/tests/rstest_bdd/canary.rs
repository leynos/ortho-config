//! Connects the hello_world canary feature to the fixtures.

use super::fixtures::{hello_world_binary, hello_world_state, HelloWorldState};
use rstest_bdd_macros::scenario;

#[scenario(path = "tests/features/rstest_bdd_canary.feature")]
fn hello_world_canary(hello_world_binary: &'static str, hello_world_state: HelloWorldState) {
    let _ = (hello_world_binary, hello_world_state);
}
