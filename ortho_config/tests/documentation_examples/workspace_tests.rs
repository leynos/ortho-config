//! Regression coverage for documented-example workspaces.

use super::layout::{render_manifest, seed_lockfile};
use super::{
    DependencyAlias, DocumentedExample, ExampleId, ExampleWorkspace, RunFile, cargo_dispatch_path,
};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Verifies that a quoted Windows dependency path survives TOML serialization.
#[test]
fn windows_dependency_path_produces_valid_toml() {
    let windows_path = r#"D:\a\"quoted\"\ortho-config\ortho_config"#;
    let serialized_path = toml::Value::String(windows_path.to_owned()).to_string();
    let generated = render_manifest("config_layer", &serialized_path, &serialized_path);
    let parsed = toml::from_str::<toml::Value>(&generated)
        .expect("serialized documentation manifest should parse as TOML");
    let dependencies = parsed
        .get("dependencies")
        .expect("generated manifest should declare dependencies");
    let parsed_path = dependencies
        .get("config_layer")
        .and_then(|dependency| dependency.get("path"))
        .and_then(toml::Value::as_str);
    assert_eq!(parsed_path, Some(windows_path));
    assert_eq!(
        dependencies
            .get("config_layer")
            .and_then(|dependency| dependency.get("package"))
            .and_then(toml::Value::as_str),
        Some("ortho_config")
    );
    assert_eq!(
        dependencies
            .get("cargo_orthohelp")
            .and_then(|dependency| dependency.get("package"))
            .and_then(toml::Value::as_str),
        Some("cargo-orthohelp")
    );
    assert_eq!(
        dependencies.get("serde_json").and_then(toml::Value::as_str),
        Some("1")
    );
}

#[test]
fn cargo_dispatch_path_keeps_only_fixture_when_path_is_absent() -> Result<()> {
    let fixture_bin_dir = Path::new("fixture-bin");
    let path = cargo_dispatch_path(fixture_bin_dir, None)?;
    let entries = std::env::split_paths(OsStr::new(&path)).collect::<Vec<_>>();

    ensure!(
        entries == [fixture_bin_dir.to_path_buf()],
        "dispatch PATH should contain only the fixture binary directory"
    );
    Ok(())
}

#[test]
fn cargo_dispatch_path_prepends_fixture_to_inherited_entries() -> Result<()> {
    let fixture_bin_dir = Path::new("fixture-bin");
    let inherited = std::env::join_paths([PathBuf::from("first"), PathBuf::from("second")])?;
    let path = cargo_dispatch_path(fixture_bin_dir, Some(&inherited))?;
    let entries = std::env::split_paths(OsStr::new(&path)).collect::<Vec<_>>();

    ensure!(
        entries
            == [
                fixture_bin_dir.to_path_buf(),
                PathBuf::from("first"),
                PathBuf::from("second"),
            ],
        "dispatch PATH should prepend the fixture binary directory"
    );
    Ok(())
}

#[test]
#[serial_test::serial(documentation_build)]
fn independently_owned_workspaces_support_concurrent_interleavings() -> Result<()> {
    std::thread::scope(|scope| {
        let first = scope.spawn(|| exercise_workspace_interleavings("first", "updated-first"));
        let second = scope.spawn(|| exercise_workspace_interleavings("second", "updated-second"));

        first
            .join()
            .map_err(|_| anyhow::anyhow!("first workspace thread panicked"))??;
        second
            .join()
            .map_err(|_| anyhow::anyhow!("second workspace thread panicked"))??;
        Ok(())
    })
}

fn exercise_workspace_interleavings(initial: &str, updated: &str) -> Result<()> {
    let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    // Ownership does not depend on any external crate; avoid two cold graphs.
    workspace.directory.write(
        "Cargo.toml",
        concat!(
            "[package]\nname = \"documented-examples\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
            "[workspace]\n",
        ),
    )?;
    workspace.add_binary(&file_probe())?;
    workspace.build()?;

    write_probe_value(&mut workspace, initial)?;
    assert_probe_output(&mut workspace, initial)?;
    write_probe_value(&mut workspace, updated)?;
    assert_probe_output(&mut workspace, updated)
}

fn file_probe() -> DocumentedExample {
    DocumentedExample {
        id: "workspace-probe".to_owned(),
        language: "rust".to_owned(),
        body: concat!(
            "fn main() -> std::io::Result<()> {\n",
            "    print!(\"{}\", std::fs::read_to_string(\"value.txt\")?);\n",
            "    Ok(())\n",
            "}\n",
        )
        .to_owned(),
        source: "workspace ownership probe",
        line: 1,
    }
}

fn write_probe_value(workspace: &mut ExampleWorkspace, contents: &str) -> Result<()> {
    workspace.write_run_file(
        ExampleId("workspace-probe"),
        RunFile {
            path: Path::new("value.txt"),
            contents,
        },
    )
}

fn assert_probe_output(workspace: &mut ExampleWorkspace, expected: &str) -> Result<()> {
    let output = workspace.run(ExampleId("workspace-probe"), std::iter::empty::<&str>())?;
    ensure!(output.status.success(), "workspace probe should succeed");
    let stdout = String::from_utf8(output.stdout).context("workspace probe output is UTF-8")?;
    ensure!(stdout == expected, "workspace probe output differed");
    Ok(())
}

#[rstest]
#[case::parent_lockfile(Some("version = 4\n"))]
#[case::no_parent_lockfile(None)]
fn documentation_lockfile_is_a_private_copy(#[case] contents: Option<&str>) -> Result<()> {
    let repository = tempfile::tempdir()?;
    let workspace = tempfile::tempdir()?;
    let source = Dir::open_ambient_dir(repository.path(), ambient_authority())?;
    let destination = Dir::open_ambient_dir(workspace.path(), ambient_authority())?;
    if let Some(body) = contents {
        source.write("Cargo.lock", body)?;
    }
    seed_lockfile(repository.path(), &destination)?;
    ensure!(destination.try_exists("Cargo.lock")? == contents.is_some());
    if let Some(body) = contents {
        ensure!(destination.read_to_string("Cargo.lock")? == body);
        destination.write("Cargo.lock", "private generated-root update")?;
        ensure!(source.read_to_string("Cargo.lock")? == body);
    }
    Ok(())
}
