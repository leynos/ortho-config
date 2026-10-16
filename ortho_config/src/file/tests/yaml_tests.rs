//! YAML provider coverage.
//! Ensures `serde-saphyr` integration preserves YAML 1.2 semantics and reports
//! malformed input clearly.

use super::{TestRoot, test_root, to_anyhow};
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use figment::Figment;
use rstest::rstest;

use crate::file::{SaphyrYaml, load_config_file};

#[rstest]
#[case("yes")]
#[case("Yes")]
#[case("YES")]
#[case("no")]
#[case("No")]
#[case("NO")]
#[case("on")]
#[case("On")]
#[case("ON")]
#[case("off")]
#[case("Off")]
#[case("OFF")]
fn yaml_legacy_boolean_literals_remain_strings(#[case] literal: &str) -> Result<()> {
    let path = Utf8PathBuf::from("config.yaml");
    let figment = Figment::from(SaphyrYaml::string(&path, format!("recipient: {literal}")));
    let recipient = figment
        .extract_inner::<String>("recipient")
        .map_err(anyhow::Error::new)?;
    ensure!(recipient == literal, "expected string literal {literal:?}");
    Ok(())
}

#[rstest]
#[case("true", true)]
#[case("false", false)]
fn yaml_respects_boolean_scalars(#[case] literal: &str, #[case] expected: bool) -> Result<()> {
    let path = Utf8PathBuf::from("config.yaml");
    let figment = Figment::from(SaphyrYaml::string(&path, format!("recipient: {literal}")));
    let recipient = figment
        .extract_inner::<bool>("recipient")
        .map_err(anyhow::Error::new)?;
    ensure!(recipient == expected, "expected boolean {expected}");
    Ok(())
}

/// Assert the file loader reads a real YAML file from disk, end to end.
///
/// The sibling tests above build a `Figment` from an in-memory string, which
/// exercises the provider but never the path handling. Writing the fixture
/// through the isolation root's `Dir` and loading its absolute path covers
/// the read, and would catch a loader that opened the path relative to the
/// process working directory.
#[rstest]
fn yaml_loader_reads_files_via_saphyr(test_root: Result<TestRoot>) -> Result<()> {
    let root = test_root?;
    root.dir.write("config.yaml", b"recipient: friend")?;
    let figment = to_anyhow(load_config_file(&root.root.join("config.yaml")))?
        .expect("expected configuration figment");
    let recipient = figment
        .extract_inner::<String>("recipient")
        .map_err(anyhow::Error::new)?;
    ensure!(recipient == "friend", "expected YAML file recipient");
    Ok(())
}

/// Assert a malformed file's error names the file it came from.
///
/// `load_config_file` canonicalizes the path before reading, so without the
/// name in the message a caller holding several candidate files could not
/// tell which one failed to parse. The fixture is deliberately truncated
/// mid-flow-sequence so the failure is a parse error, not a read error.
#[rstest]
fn yaml_loader_reports_parse_errors_with_paths(test_root: Result<TestRoot>) -> Result<()> {
    let root = test_root?;
    root.dir.write("config.yaml", b"recipient: [")?;
    let err = to_anyhow(load_config_file(&root.root.join("config.yaml")))
        .expect_err("expected load failure for invalid YAML");
    ensure!(
        err.to_string().contains("config.yaml"),
        "expected error to mention source path, got: {err}"
    );
    Ok(())
}

#[rstest]
#[case("recipient: first\nrecipient: second", "duplicate mapping key")]
#[case("recipient: [", "while parsing")]
fn yaml_provider_surfaces_errors(#[case] contents: &str, #[case] expected: &str) -> Result<()> {
    let path = Utf8PathBuf::from("config.yaml");
    let figment = Figment::from(SaphyrYaml::string(&path, contents));
    let err = figment
        .extract::<crate::serde_json::Value>()
        .expect_err("expected YAML parsing failure");
    ensure!(
        err.to_string().contains(expected),
        "expected error to mention '{expected}', got: {err}"
    );
    Ok(())
}
