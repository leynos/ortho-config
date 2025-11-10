//! Binds the canary feature file to the reusable fixtures.

use super::fixtures::{binary_name, canary_state, CanaryState};
use rstest_bdd_macros::scenario;

#[scenario(path = "tests/features/rstest_bdd_canary.feature")]
fn rstest_bdd_canary(binary_name: &'static str, canary_state: CanaryState) {
    let _ = (binary_name, canary_state);
}
