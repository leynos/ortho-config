//! Regression coverage for CLI values whose spellings differ by namespace.
//!
//! The synthetic CLI struct serializes under the Rust member name, while
//! `#[serde(rename = "...")]` changes the configuration key and
//! `#[arg(id = "...")]` changes the clap lookup. These cases pin that an
//! explicit CLI value is read from the member key, stored under the
//! configuration's deserialization key, and still outranks an environment
//! value when it equals the configured default. Both layers must also agree on
//! that key when the field carries an alias or a serialize-only rename.

#[path = "support/hermetic_env.rs"]
mod hermetic_env;

use anyhow::{Context as _, Result, anyhow, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use clap::Parser;
use hermetic_env::hermetic;
use ortho_config::{MapEnv, MergeLayer, MergeProvenance, OrthoConfig};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Configuration whose field spellings differ across namespaces.
#[derive(Debug, Parser, Deserialize, Serialize, OrthoConfig)]
#[command(name = "renamed-defaults")]
#[ortho_config(prefix = "EQRN_")]
struct RenamedDefaults {
    /// Rust member and CLI flag `port`; configuration key `service_port`.
    ///
    /// The alias makes the pre-rename key a second accepted spelling. Both
    /// layers must still emit only `service_port`, or deserialization sees the
    /// same field twice.
    #[serde(rename = "service_port", alias = "port")]
    #[ortho_config(default = 443)]
    port: u16,
    /// Rust member and CLI flag `mode`; clap argument ID `listen_mode`.
    #[arg(id = "listen_mode")]
    #[ortho_config(default = String::from("fast"))]
    mode: String,
    /// Rust member `wire`; serialization key `wire_port`, deserialization `wire`.
    #[serde(rename(serialize = "wire_port"))]
    #[ortho_config(default = 8080)]
    wire: u16,
}

/// Write an empty `app.toml` beneath `root`.
fn write_config(root: &Utf8Path) -> Result<()> {
    let cap = Dir::open_ambient_dir(root, ambient_authority()).context("open fixture dir")?;
    cap.write("app.toml", b"").context("write app.toml")?;
    Ok(())
}

/// One row of the member/id/key matrix.
struct Case {
    /// Command-line arguments, including the program name.
    cli_args: &'static [&'static str],
    /// Value supplied through `EQRN_SERVICE_PORT`.
    service_port: &'static str,
    /// Value supplied through `EQRN_MODE`.
    mode: &'static str,
    /// Value supplied through `EQRN_WIRE`.
    wire: &'static str,
    /// Expected merged port.
    expected_port: u16,
    /// Expected merged mode.
    expected_mode: &'static str,
    /// Expected merged wire value.
    expected_wire: u16,
    /// Whether the composition must include a CLI layer.
    expect_cli_layer: bool,
}

/// Compose one case and assert the merged values and layer presence.
#[rstest]
#[case::explicit_renamed_member_restates_default(Case {
    cli_args: &["prog", "--port", "443"],
    service_port: "8443",
    mode: "slow",
    wire: "9090",
    expected_port: 443,
    expected_mode: "slow",
    expected_wire: 9090,
    expect_cli_layer: true,
})]
#[case::absent_renamed_member_defers(Case {
    cli_args: &["prog"],
    service_port: "8443",
    mode: "slow",
    wire: "9090",
    expected_port: 8443,
    expected_mode: "slow",
    expected_wire: 9090,
    expect_cli_layer: false,
})]
#[case::explicit_custom_arg_id_restates_default(Case {
    cli_args: &["prog", "--mode", "fast"],
    service_port: "8443",
    mode: "slow",
    wire: "9090",
    expected_port: 8443,
    expected_mode: "fast",
    expected_wire: 9090,
    expect_cli_layer: true,
})]
#[case::absent_custom_arg_id_defers(Case {
    cli_args: &["prog"],
    service_port: "8443",
    mode: "slow",
    wire: "9090",
    expected_port: 8443,
    expected_mode: "slow",
    expected_wire: 9090,
    expect_cli_layer: false,
})]
#[case::explicit_serialize_only_rename_restates_default(Case {
    cli_args: &["prog", "--wire", "8080"],
    service_port: "8443",
    mode: "slow",
    wire: "9090",
    expected_port: 8443,
    expected_mode: "slow",
    expected_wire: 8080,
    expect_cli_layer: true,
})]
#[case::absent_serialize_only_rename_defers(Case {
    cli_args: &["prog"],
    service_port: "8443",
    mode: "slow",
    wire: "9090",
    expected_port: 8443,
    expected_mode: "slow",
    expected_wire: 9090,
    expect_cli_layer: false,
})]
fn member_name_and_serde_keys_are_bridged(#[case] case: Case) -> Result<()> {
    let dir = tempfile::tempdir().context("create fixture dir")?;
    let root = Utf8Path::from_path(dir.path())
        .ok_or_else(|| anyhow!("temporary fixture path is not UTF-8"))?
        .to_owned();
    write_config(&root)?;

    let environment = MapEnv::new()
        .with_var("HOME", root.join("home-empty").as_std_path())
        .with_var("EQRN_CONFIG_PATH", root.join("app.toml").as_std_path())
        .with_var("EQRN_SERVICE_PORT", case.service_port)
        .with_var("EQRN_MODE", case.mode)
        .with_var("EQRN_WIRE", case.wire);
    let env = hermetic(environment, root.as_std_path());

    let composition = RenamedDefaults::compose_layers_from_iter_with_env(
        case.cli_args.iter().copied(),
        Arc::new(env),
    );
    let (layers, errors) = composition.into_parts();
    ensure!(errors.is_empty(), "composition errors: {errors:?}");

    let has_cli_layer = layers
        .iter()
        .any(|layer| layer.provenance() == MergeProvenance::Cli);
    ensure!(
        has_cli_layer == case.expect_cli_layer,
        "expected CLI layer present = {}, got {:?}",
        case.expect_cli_layer,
        layers
            .iter()
            .map(MergeLayer::provenance)
            .collect::<Vec<_>>()
    );

    let merged = RenamedDefaults::merge_from_layers(layers)
        .map_err(|err| anyhow!(err))
        .context("merge the composed layers")?;
    ensure!(
        merged.port == case.expected_port,
        "expected port {}, got {}",
        case.expected_port,
        merged.port
    );
    ensure!(
        merged.mode == case.expected_mode,
        "expected mode {:?}, got {:?}",
        case.expected_mode,
        merged.mode
    );
    ensure!(
        merged.wire == case.expected_wire,
        "expected wire {}, got {}",
        case.expected_wire,
        merged.wire
    );
    Ok(())
}
