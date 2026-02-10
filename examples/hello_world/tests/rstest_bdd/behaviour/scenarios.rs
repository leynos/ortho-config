//! Binds the `hello_world` behavioural feature file to rstest-bdd scenarios.

use crate::behaviour::harness::Harness;
use crate::fixtures::hello_world_harness;
use rstest_bdd_macros::scenarios;

mod non_yaml {
    use super::*;

    scenarios!(
        "tests/features/global_parameters.feature",
        fixtures = [hello_world_harness: Harness],
        tags = "not @requires_yaml"
    );
}

#[cfg(feature = "yaml")]
mod yaml {
    use super::*;

    scenarios!(
        "tests/features/global_parameters.feature",
        fixtures = [hello_world_harness: Harness],
        tags = "@requires_yaml"
    );
}
