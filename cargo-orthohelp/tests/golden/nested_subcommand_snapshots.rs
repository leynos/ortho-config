//! Snapshot tests for nested subcommand renderer output.

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use cargo_orthohelp::powershell::{PowerShellConfig, generate as generate_powershell};
use cargo_orthohelp::roff::{RoffConfig, generate as generate_roff, generate_to_string};
use insta::assert_snapshot;
use std::error::Error;
use std::io::Read;

#[path = "../fixtures/nested_fixture.rs"]
mod nested_fixture;

#[test]
fn nested_roff_inline_snapshot() {
    let metadata = nested_fixture::nested_doc();
    let config = RoffConfig {
        date: Some("2026-06-04".to_owned()),
        source: Some("cargo-orthohelp 0.8.0".to_owned()),
        manual: Some("Fixture Manual".to_owned()),
        ..RoffConfig::default()
    };
    let output = generate_to_string(&metadata, &config);

    assert_snapshot!("nested_roff_inline", output);
}

#[test]
fn nested_roff_split_snapshots() -> Result<(), Box<dyn Error>> {
    let metadata = nested_fixture::nested_doc();
    let temp_dir = tempfile::tempdir()?;
    let out_dir = temp_out_dir(&temp_dir)?;
    let config = RoffConfig {
        out_dir: out_dir.clone(),
        date: Some("2026-06-04".to_owned()),
        source: Some("cargo-orthohelp 0.8.0".to_owned()),
        manual: Some("Fixture Manual".to_owned()),
        should_split_subcommands: true,
        ..RoffConfig::default()
    };
    let mut files = generate_roff(&metadata, &config)?.files;
    files.sort();

    let snapshots = files
        .iter()
        .map(|file| {
            let snapshot_name = split_snapshot_name(file);
            let content = read_text(&out_dir, relative_to(&out_dir, file)?)?;
            Ok::<_, Box<dyn Error>>((snapshot_name, content))
        })
        .collect::<Result<Vec<_>, _>>()?;

    for (snapshot_name, content) in snapshots {
        assert_snapshot!(snapshot_name, content);
    }
    Ok(())
}

#[test]
fn nested_powershell_snapshots() -> Result<(), Box<dyn Error>> {
    let metadata = nested_fixture::nested_doc();
    let temp_dir = tempfile::tempdir()?;
    let out_dir = temp_out_dir(&temp_dir)?;
    let config = PowerShellConfig {
        out_dir: out_dir.clone(),
        module_name: "NestedFixture".into(),
        module_version: "0.1.0".into(),
        bin_name: "fixture".into(),
        export_aliases: vec!["fixture-help".into()],
        should_include_common_parameters: true,
        should_split_subcommands: true,
        help_info_uri: None,
        should_ensure_en_us: true,
    };
    generate_powershell(&[metadata], &config)?;

    let module_root = out_dir.join("powershell").join("NestedFixture");
    assert_snapshot!(
        "nested_powershell_module",
        read_text(&module_root, "NestedFixture.psm1")?
    );
    assert_snapshot!(
        "nested_powershell_manifest",
        read_text(&module_root, "NestedFixture.psd1")?
    );
    assert_snapshot!(
        "nested_powershell_maml",
        read_text(&module_root, "en-US/NestedFixture-help.xml")?
    );
    assert_snapshot!(
        "nested_powershell_about",
        read_text(&module_root, "en-US/about_NestedFixture.help.txt")?
    );
    Ok(())
}

fn temp_out_dir(temp_dir: &tempfile::TempDir) -> Result<Utf8PathBuf, Box<dyn Error>> {
    Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).map_err(|path| {
        std::io::Error::other(format!("temp dir path should be UTF-8: {}", path.display())).into()
    })
}

fn split_snapshot_name(file: &Utf8PathBuf) -> String {
    let name = file
        .file_name()
        .unwrap_or("unknown")
        .replace(['.', '-'], "_");
    format!("nested_roff_split_{name}")
}

fn relative_to<'a>(root: &Utf8PathBuf, file: &'a Utf8PathBuf) -> Result<&'a str, Box<dyn Error>> {
    file.strip_prefix(root)
        .map(camino::Utf8Path::as_str)
        .map_err(|error| format!("generated path {file} was outside {root}: {error}").into())
}

fn read_text(root: &Utf8PathBuf, relative: &str) -> Result<String, Box<dyn Error>> {
    let dir = Dir::open_ambient_dir(root, ambient_authority())?;
    let mut file = dir.open(relative)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(normalize_text(&bytes))
}

fn normalize_text(bytes: &[u8]) -> String {
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(body).replace("\r\n", "\n")
}
