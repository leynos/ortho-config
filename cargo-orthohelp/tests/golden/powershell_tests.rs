//! Golden tests for `PowerShell` output generation.

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use cargo_orthohelp::ir::LocalizedDocMetadata;
use cargo_orthohelp::powershell::{PowerShellConfig, generate};
use rstest::{fixture, rstest};
use std::error::Error;
use std::io::Read;

#[fixture]
fn minimal_doc() -> LocalizedDocMetadata {
    powershell_fixture::minimal_doc()
}

fn doc_for_locale(locale: &str, template: &LocalizedDocMetadata) -> LocalizedDocMetadata {
    let mut doc = template.clone();
    locale.clone_into(&mut doc.locale);
    doc
}

#[path = "../fixtures/powershell_fixture.rs"]
mod powershell_fixture;

#[fixture]
fn ps_setup(
    #[default(true)] should_ensure_en_us: bool,
) -> Result<(tempfile::TempDir, Utf8PathBuf, PowerShellConfig), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let out_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).map_err(|path| {
        std::io::Error::other(format!("temp dir path should be utf-8: {}", path.display()))
    })?;
    let config = PowerShellConfig {
        out_dir,
        module_name: "FixtureHelp".into(),
        module_version: "0.1.0".into(),
        bin_name: "fixture".into(),
        export_aliases: vec!["fixture-help".into()],
        should_include_common_parameters: true,
        should_split_subcommands: false,
        help_info_uri: None,
        should_ensure_en_us,
    };
    let config_out_dir = config.out_dir.clone();
    Ok((temp_dir, config_out_dir, config))
}

fn open_module_root(out_dir: &Utf8PathBuf) -> Result<Dir, Box<dyn Error>> {
    let module_root = out_dir.join("powershell").join("FixtureHelp");
    Dir::open_ambient_dir(&module_root, ambient_authority()).map_err(|source| {
        Box::new(std::io::Error::other(format!(
            "failed to open module root at {module_root}: {source}"
        ))) as Box<dyn Error>
    })
}

#[rstest]
fn powershell_outputs_match_goldens(
    ps_setup: Result<(tempfile::TempDir, Utf8PathBuf, PowerShellConfig), Box<dyn Error>>,
    minimal_doc: LocalizedDocMetadata,
) -> Result<(), Box<dyn Error>> {
    let (_temp_dir, out_dir, config) = ps_setup?;

    generate(&[minimal_doc], &config).expect("generate powershell output");

    let dir = open_module_root(&out_dir)?;

    assert_text_matches(
        &dir,
        "FixtureHelp.psm1",
        include_str!("powershell/fixture.psm1.golden"),
    )?;
    assert_text_matches(
        &dir,
        "FixtureHelp.psd1",
        include_str!("powershell/fixture.psd1.golden"),
    )?;
    assert_text_matches(
        &dir,
        "en-US/FixtureHelp-help.xml",
        include_str!("powershell/fixture-help.xml.golden"),
    )?;
    assert_text_matches(
        &dir,
        "en-US/about_FixtureHelp.help.txt",
        include_str!("powershell/fixture-about.help.txt.golden"),
    )?;

    Ok(())
}

#[rstest]
fn powershell_generates_en_us_fallback_from_non_en_us_locale(
    ps_setup: Result<(tempfile::TempDir, Utf8PathBuf, PowerShellConfig), Box<dyn Error>>,
    minimal_doc: LocalizedDocMetadata,
) -> Result<(), Box<dyn Error>> {
    let (_temp_dir, out_dir, config) = ps_setup?;

    let fr_doc = doc_for_locale("fr-FR", &minimal_doc);
    generate(&[fr_doc], &config).expect("generate powershell output");

    let dir = open_module_root(&out_dir)?;

    assert_text_matches(
        &dir,
        "fr-FR/FixtureHelp-help.xml",
        include_str!("powershell/fixture-help.xml.golden"),
    )?;
    assert_text_matches(
        &dir,
        "fr-FR/about_FixtureHelp.help.txt",
        include_str!("powershell/fixture-about.help.txt.golden"),
    )?;
    assert_text_matches(
        &dir,
        "en-US/FixtureHelp-help.xml",
        include_str!("powershell/fixture-help.xml.golden"),
    )?;
    assert_text_matches(
        &dir,
        "en-US/about_FixtureHelp.help.txt",
        include_str!("powershell/fixture-about.help.txt.golden"),
    )?;

    Ok(())
}

#[rstest]
fn powershell_does_not_generate_en_us_fallback_when_disabled(
    #[with(false)] ps_setup: Result<
        (tempfile::TempDir, Utf8PathBuf, PowerShellConfig),
        Box<dyn Error>,
    >,
    minimal_doc: LocalizedDocMetadata,
) -> Result<(), Box<dyn Error>> {
    let (_temp_dir, out_dir, config) = ps_setup?;

    let fr_doc = doc_for_locale("fr-FR", &minimal_doc);
    generate(&[fr_doc], &config).expect("generate powershell output");

    let dir = open_module_root(&out_dir)?;

    assert_text_matches(
        &dir,
        "fr-FR/FixtureHelp-help.xml",
        include_str!("powershell/fixture-help.xml.golden"),
    )?;
    if dir.open("en-US/FixtureHelp-help.xml").is_ok() {
        return Err("unexpected en-US fallback help file generated".into());
    }
    if dir.open("en-US/about_FixtureHelp.help.txt").is_ok() {
        return Err("unexpected en-US fallback about file generated".into());
    }

    Ok(())
}

fn assert_text_matches(dir: &Dir, relative: &str, expected: &str) -> Result<(), Box<dyn Error>> {
    let mut file = dir.open(relative)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let actual = normalise_text(&buffer);
    let expected_text = normalise_text(expected.as_bytes());
    if actual != expected_text {
        let mismatch = describe_first_mismatch(&actual, &expected_text);
        return Err(format!(
            "content mismatch for {relative}: {mismatch}\n--- expected ---\n{expected_text}\n--- actual ---\n{actual}"
        )
        .into());
    }
    Ok(())
}

fn describe_first_mismatch(actual: &str, expected: &str) -> String {
    for (index, (actual_line, expected_line)) in actual.lines().zip(expected.lines()).enumerate() {
        if actual_line != expected_line {
            return format!(
                "first difference at line {} (expected {:?}, actual {:?})",
                index + 1,
                expected_line,
                actual_line
            );
        }
    }

    let actual_line_count = actual.lines().count();
    let expected_line_count = expected.lines().count();
    if actual_line_count != expected_line_count {
        return format!(
            "line count differs (expected {expected_line_count}, actual {actual_line_count})"
        );
    }

    "text differs but no line-level mismatch was identified".to_owned()
}

fn normalise_text(bytes: &[u8]) -> String {
    let mut start = 0;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        start = 3;
    }
    let slice = bytes.get(start..).unwrap_or_default();
    let text = String::from_utf8_lossy(slice);
    text.replace("\r\n", "\n")
}
