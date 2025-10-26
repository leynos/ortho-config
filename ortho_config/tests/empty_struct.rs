//! Behavioural tests ensuring empty configuration structs generate valid code
//! and load successfully.
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig, PartialEq, Eq)]
struct EmptyConfig {}

#[rstest]
fn loads_empty_struct_successfully() -> Result<()> {
    let cfg = EmptyConfig::load_from_iter(["bin"]).map_err(|err| anyhow!(err))?;
    ensure!(
        cfg == EmptyConfig {},
        "expected default empty config, got {cfg:?}"
    );
    Ok(())
}
