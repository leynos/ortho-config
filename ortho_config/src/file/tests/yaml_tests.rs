//! YAML provider coverage.
//! Ensures `serde-saphyr` integration preserves YAML 1.2 semantics and reports
//! malformed input clearly.

use super::to_anyhow;
use anyhow::{Result, anyhow, ensure};
use figment::Figment;
use rstest::rstest;
use std::path::PathBuf;

use crate::file::{SaphyrYaml, load_config_file};

#[rstest]
fn yaml_yes_remains_a_string() -> Result<()> {
    let path = PathBuf::from("config.yaml");
    let figment = Figment::from(SaphyrYaml::string(&path, "recipient: yes"));
    let recipient = figment
        .extract_inner::<String>("recipient")
        .map_err(|err| anyhow!(err.to_string()))?;
    ensure!(recipient == "yes", "expected string literal \"yes\"");
    Ok(())
}

#[rstest]
fn yaml_loader_reads_files_via_saphyr() -> Result<()> {
    super::with_jail(|jail| {
        jail.create_file("config.yaml", "recipient: friend")?;
        let figment = to_anyhow(load_config_file(PathBuf::from("config.yaml").as_path()))?
            .expect("expected configuration figment");
        let recipient = figment
            .extract_inner::<String>("recipient")
            .map_err(|err| anyhow!(err.to_string()))?;
        ensure!(recipient == "friend", "expected YAML file recipient");
        Ok(())
    })
}

#[rstest]
#[case("recipient: first\nrecipient: second", "duplicate mapping key")]
#[case("recipient: [", "while parsing")]
fn yaml_provider_surfaces_errors(#[case] contents: &str, #[case] expected: &str) -> Result<()> {
    let figment = Figment::from(SaphyrYaml::string("config.yaml", contents));
    let err = figment
        .extract::<crate::serde_json::Value>()
        .expect_err("expected YAML parsing failure");
    ensure!(
        err.to_string().contains(expected),
        "expected error to mention '{expected}', got: {err}"
    );
    Ok(())
}
