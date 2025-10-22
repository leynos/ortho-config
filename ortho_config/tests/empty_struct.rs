//! Behavioural tests ensuring empty configuration structs generate valid code
//! and load successfully.
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]

use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig, PartialEq, Eq)]
struct EmptyConfig {}

#[rstest]
fn loads_empty_struct_successfully() {
    let cfg = EmptyConfig::load_from_iter(["bin"]).expect("load");
    assert_eq!(cfg, EmptyConfig {});
}
