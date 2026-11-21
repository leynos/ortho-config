//! Regressions for injection through the configured root type.
//!
//! The root type from `--root-type` or `package.metadata.ortho_config.root_type`
//! is spliced into generated bridge source, so both input sources are exercised
//! with values that would compile and run if validation were bypassed. Each
//! case asserts the semantic diagnostic and that the payload's marker file was
//! never written.

use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use std::error::Error;
use std::process::{Command, Output};

use crate::fixtures;
use crate::{SUBPROCESS_DEADLINE, run_direct, stderr};

/// Writes a complete, buildable package for the metadata injection payload.
///
/// The fixture derives every trait the bridge requires and defaults its field,
/// so the payload would compile and write its marker if validation were
/// bypassed.
fn write_injection_fixture(
    dir: &Dir,
    ortho_config_path: &Utf8Path,
    root_type: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    dir.create_dir("src")?;
    dir.write(
        "src/lib.rs",
        concat!(
            "use clap::Parser;\n",
            "use ortho_config::OrthoConfig;\n",
            "use serde::{Deserialize, Serialize};\n",
            "\n",
            "#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]\n",
            "#[ortho_config(prefix = \"ROOT_TYPE_INJECTION\")]\n",
            "pub struct Config {\n",
            "    /// Value used only to satisfy the derive's named-field rule.\n",
            "    #[ortho_config(default = String::from(\"value\"))]\n",
            "    pub value: String,\n",
            "}\n",
        ),
    )?;
    dir.write(
        "Cargo.toml",
        format!(
            concat!(
                "[package]\n",
                "name = \"root_type_injection_fixture\"\n",
                "version = \"0.1.0\"\n",
                "edition = \"2024\"\n",
                "\n",
                "[dependencies]\n",
                "ortho_config = {{ path = {ortho_config:?} }}\n",
                "serde = {{ version = \"1\", features = [\"derive\"] }}\n",
                "clap = {{ version = \"4\", features = [\"derive\"] }}\n",
                "\n",
                "[package.metadata.ortho_config]\n",
                "root_type = {root_type:?}\n",
            ),
            ortho_config = ortho_config_path.as_str(),
            root_type = root_type,
        ),
    )?;
    Ok(())
}

/// Asserts the generator rejected the root type with the semantic diagnostic.
fn assert_root_type_rejected(
    output: &Output,
    failure_message: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if output.status.success() {
        return Err(failure_message.into());
    }
    let stderr = stderr(output);
    if !stderr.contains("InvalidRootType") {
        return Err(format!("unexpected diagnostic:\n{stderr}").into());
    }
    Ok(())
}

/// Asserts the bridge never ran by checking the payload's marker is absent.
fn assert_marker_absent(dir: &Dir) -> Result<(), Box<dyn Error + Send + Sync>> {
    if dir.exists("injected.marker") {
        return Err("the bridge executed the injected payload".into());
    }
    Ok(())
}

/// The payload would be executable if validation were bypassed: it names a real
/// root type from the fixture and writes a marker file as a side effect.
#[test]
fn cli_root_type_injection_is_rejected_before_the_bridge_runs()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let marker_dir = tempfile::tempdir()?;
    let marker_path = marker_dir.path().join("injected.marker");
    let payload = format!(
        concat!(
            "orthohelp_fixture::SimpleFixtureConfig as OrthoConfigDocs>::",
            "get_doc_metadata(); std::fs::write({marker:?}, b\"injected\")",
            ".expect(\"marker write\"); let _ = <orthohelp_fixture::SimpleFixtureConfig",
        ),
        marker = marker_path,
    );

    let output = run_direct(&[
        "orthohelp",
        "--package",
        "orthohelp_fixture",
        "--root-type",
        &payload,
    ])?;
    assert_root_type_rejected(&output, "injection-shaped root type should fail")?;

    let marker_dir_path = Utf8Path::from_path(marker_dir.path())
        .ok_or("temporary marker path should be valid UTF-8")?;
    let dir = Dir::open_ambient_dir(marker_dir_path, ambient_authority())?;
    assert_marker_absent(&dir)
}

#[test]
fn metadata_root_type_injection_is_rejected() -> Result<(), Box<dyn Error + Send + Sync>> {
    let package_dir = tempfile::tempdir()?;
    let package_path = Utf8Path::from_path(package_dir.path())
        .ok_or("temporary package path should be valid UTF-8")?;
    let dir = Dir::open_ambient_dir(package_path, ambient_authority())?;
    let marker_path = package_dir.path().join("injected.marker");
    let root_type = format!(
        concat!(
            "root_type_injection_fixture::Config as OrthoConfigDocs>::get_doc_metadata(); ",
            "std::fs::write({marker:?}, b\"injected\").expect(\"marker write\"); ",
            "let _ = <root_type_injection_fixture::Config",
        ),
        marker = marker_path,
    );
    let ortho_config_path = fixtures::workspace_root()?.join("ortho_config");
    write_injection_fixture(&dir, &ortho_config_path, &root_type)?;

    let exe = fixtures::cargo_orthohelp_exe()?;
    let mut command = Command::new(exe.as_str());
    command.current_dir(package_dir.path()).args([
        "orthohelp",
        "--format",
        "agent-context",
        "--out-dir",
        "out",
    ]);
    let output = test_helpers::process::run_with_deadline(
        command,
        SUBPROCESS_DEADLINE,
        "cargo-orthohelp metadata injection probe",
    )?;

    assert_root_type_rejected(&output, "metadata root type injection should fail")?;
    assert_marker_absent(&dir)
}
