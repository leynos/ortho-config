//! Regressions for bounded Cargo JSON parsing and artifact validation.

use super::{GeneratedTarget, MAX_CARGO_DIAGNOSTIC_BYTES, binary_file_name, read_cargo_messages};
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;
use serde_json::{Value, json};
use std::io::Cursor;

const TARGET_NAME: &str = "orthohelp_bridge_fixture";

#[path = "bridge_build_protocol_property_tests.rs"]
mod property_tests;

#[test]
fn compiler_diagnostics_are_returned_without_a_presentation_sink() -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let record = json!({
        "reason": "compiler-message",
        "message": { "rendered": "warning: probe\n" }
    });

    let messages = read_records(&[record], &target)?;

    ensure!(messages.diagnostics == "warning: probe\n");
    ensure!(!messages.diagnostics_truncated);
    ensure!(messages.protocol_error.is_none());
    Ok(())
}

#[test]
fn compiler_diagnostics_are_bounded_while_the_json_stream_is_drained() -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let rendered = format!("warning: {}\n", "x".repeat(99_990));
    let records = (0..12)
        .map(|_| {
            json!({
                "reason": "compiler-message",
                "message": { "rendered": rendered }
            })
        })
        .collect::<Vec<_>>();
    let stream = encode_records(&records)?;
    let mut reader = Cursor::new(stream.as_slice());

    let messages = read_cargo_messages(&mut reader, &target)?;

    ensure!(messages.diagnostics.len() == MAX_CARGO_DIAGNOSTIC_BYTES);
    ensure!(messages.diagnostics_truncated);
    ensure!(reader.position() == stream.len() as u64);
    Ok(())
}

#[test]
fn bridge_artifact_records_keep_only_generated_target_outputs() -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let executable = target_directory
        .join("debug")
        .join(binary_file_name(TARGET_NAME));
    let executable_text = executable.to_string();
    let dep_info = target_directory
        .join("debug")
        .join(format!("{TARGET_NAME}.d"));
    let hashed = target_directory
        .join("debug")
        .join("deps")
        .join(format!("{TARGET_NAME}-abc123"));
    let debug_info = target_directory
        .join("debug")
        .join(format!("{TARGET_NAME}.pdb"));
    let decoy = target_directory
        .join("debug")
        .join("deps")
        .join(format!("{TARGET_NAME}_other-abc123"));
    let outside = target_directory
        .parent()
        .context("target fixture directory should have a parent")?
        .join("outside")
        .join(format!("{TARGET_NAME}-abc123"));
    let record = artifact_record(
        TARGET_NAME,
        &[
            executable_text.clone(),
            debug_info.to_string(),
            dep_info.to_string(),
            hashed.to_string(),
            decoy.to_string(),
            outside.to_string(),
        ],
        Some(&executable_text),
    );

    let messages = read_records(&[record], &target)?;

    ensure!(messages.protocol_error.is_none());
    ensure!(messages.generated_outputs.len() == 4);
    ensure!(messages.generated_outputs.contains(&executable));
    ensure!(messages.generated_outputs.contains(&debug_info));
    ensure!(messages.generated_outputs.contains(&dep_info));
    ensure!(messages.generated_outputs.contains(&hashed));
    Ok(())
}

#[test]
fn relative_executable_paths_are_rejected() -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let file_name = binary_file_name(TARGET_NAME);
    let record = artifact_record(
        TARGET_NAME,
        &[format!("debug/{file_name}")],
        Some(&format!("debug/{file_name}")),
    );

    assert_protocol_error(&[record], &target, "path was not absolute")
}

#[test]
fn artifacts_for_a_different_target_name_are_ignored() -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let executable = target_directory
        .join("debug")
        .join(binary_file_name(TARGET_NAME))
        .to_string();
    let record = artifact_record(
        "another_binary",
        std::slice::from_ref(&executable),
        Some(&executable),
    );

    let messages = read_records(&[record], &target)?;

    ensure!(messages.protocol_error.is_none());
    ensure!(messages.executable.is_none());
    ensure!(messages.generated_outputs.is_empty());
    Ok(())
}

#[test]
fn an_unexpected_executable_filename_is_rejected() -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let unexpected = target_directory
        .join("debug")
        .join("unexpected-executable")
        .to_string();
    let record = artifact_record(
        TARGET_NAME,
        std::slice::from_ref(&unexpected),
        Some(&unexpected),
    );

    assert_protocol_error(&[record], &target, "unexpected generated executable name")
}

#[test]
fn executable_paths_outside_the_target_directory_are_rejected() -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let outside = target_directory
        .parent()
        .context("target fixture directory should have a parent")?
        .join("outside")
        .join(binary_file_name(TARGET_NAME))
        .to_string();
    let record = artifact_record(TARGET_NAME, std::slice::from_ref(&outside), Some(&outside));

    assert_protocol_error(
        &[record],
        &target,
        "outside the configured target directory",
    )
}

#[test]
fn compiler_artifacts_without_an_executable_path_are_rejected() -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let file_name = binary_file_name(TARGET_NAME);
    let executable = target_directory.join("debug").join(file_name).to_string();
    let record = artifact_record(TARGET_NAME, &[executable], None);

    assert_protocol_error(&[record], &target, "omitted executable path")
}

#[test]
fn conflicting_executable_records_are_rejected() -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let file_name = binary_file_name(TARGET_NAME);
    let first = target_directory.join("debug").join(&file_name).to_string();
    let second = target_directory
        .join("debug")
        .join("other")
        .join(file_name)
        .to_string();
    let records = [
        artifact_record(TARGET_NAME, std::slice::from_ref(&first), Some(&first)),
        artifact_record(TARGET_NAME, std::slice::from_ref(&second), Some(&second)),
    ];

    assert_protocol_error(&records, &target, "multiple generated bridge executables")
}

/// Only the bridge's own uplifted debug symbols join its generated outputs.
///
/// The negative case is the control: the MSVC linker uplifts `foo.pdb` for
/// binary `foo`, so an unanchored suffix or prefix match would also claim a
/// sibling binary's `foo_other.pdb` and delete another build's symbols.
#[rstest]
#[case(format!("{TARGET_NAME}.pdb"), true)]
#[case(format!("{TARGET_NAME}_other.pdb"), false)]
fn bridge_debug_symbol_names_are_collected_only_for_the_bridge(
    #[case] debug_file_name: String,
    #[case] is_collected: bool,
) -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let executable = target_directory
        .join("debug")
        .join(binary_file_name(TARGET_NAME));
    let executable_text = executable.to_string();
    let debug_info = target_directory.join("debug").join(debug_file_name);
    let record = artifact_record(
        TARGET_NAME,
        &[executable_text.clone(), debug_info.to_string()],
        Some(&executable_text),
    );

    let messages = read_records(&[record], &target)?;

    ensure!(messages.protocol_error.is_none());
    ensure!(messages.generated_outputs.contains(&debug_info) == is_collected);
    Ok(())
}

fn generated_target_directory() -> Result<Utf8PathBuf> {
    let temporary_directory_path = std::env::temp_dir()
        .join("orthohelp-bridge-protocol-tests")
        .join("target");
    let directory =
        Utf8PathBuf::from_path_buf(temporary_directory_path).map_err(|non_utf8_path| {
            anyhow::anyhow!("temporary path is not UTF-8: {}", non_utf8_path.display())
        })?;
    ensure!(directory.is_absolute(), "temporary path must be absolute");
    Ok(directory)
}

fn generated_target(directory: &Utf8Path) -> GeneratedTarget<'_> {
    GeneratedTarget {
        directory,
        binary_name: TARGET_NAME,
    }
}

fn artifact_record(target_name: &str, filenames: &[String], executable: Option<&str>) -> Value {
    let mut record = serde_json::Map::from_iter([
        ("reason".to_owned(), json!("compiler-artifact")),
        (
            "target".to_owned(),
            json!({ "name": target_name, "kind": ["bin"] }),
        ),
        ("filenames".to_owned(), json!(filenames)),
    ]);
    if let Some(executable_path) = executable {
        record.insert("executable".to_owned(), json!(executable_path));
    }
    Value::Object(record)
}

fn read_records(records: &[Value], target: &GeneratedTarget<'_>) -> Result<super::CargoMessages> {
    let stream = encode_records(records)?;
    let mut reader = Cursor::new(stream);
    Ok(read_cargo_messages(&mut reader, target)?)
}

fn encode_records(records: &[Value]) -> Result<Vec<u8>> {
    let mut stream = Vec::new();
    for record in records {
        serde_json::to_writer(&mut stream, record)?;
        stream.push(b'\n');
    }
    Ok(stream)
}

fn assert_protocol_error(
    records: &[Value],
    target: &GeneratedTarget<'_>,
    expected: &str,
) -> Result<()> {
    let messages = read_records(records, target)?;
    let protocol_error = messages
        .protocol_error
        .context("expected the Cargo protocol record to be rejected")?;
    ensure!(protocol_error.to_string().contains(expected));
    Ok(())
}
