//! Behavioural tests ensuring empty configuration structs derive correctly.

use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig, PartialEq, Eq)]
struct EmptyConfig {}

#[rstest]
fn derives_for_struct_without_fields() {
    let cfg = EmptyConfig::load_from_iter(["bin"]).expect("load");
    assert_eq!(cfg, EmptyConfig {});
}
