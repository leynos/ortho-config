//! Golden tests for `PowerShell` output generation.

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use cargo_orthohelp::ir::{LocalizedDocMetadata, LocalizedHeadings, LocalizedSectionsMetadata};
use cargo_orthohelp::powershell::{PowerShellConfig, generate};
use cargo_orthohelp::schema::{CliMetadata, DefaultValue, EnvMetadata, FileMetadata, ValueType};
use rstest::{fixture, rstest};
use std::error::Error;
use std::io::Read;

#[fixture]
fn minimal_doc() -> LocalizedDocMetadata {
    LocalizedDocMetadata {
        ir_version: "1.1".to_owned(),
        locale: "en-US".to_owned(),
        app_name: "fixture".to_owned(),
        bin_name: None,
        about: "Fixture app".to_owned(),
        synopsis: None,
        sections: LocalizedSectionsMetadata {
            headings: LocalizedHeadings {
                name: "NAME".to_owned(),
                synopsis: "SYNOPSIS".to_owned(),
                description: "DESCRIPTION".to_owned(),
                options: "OPTIONS".to_owned(),
                environment: "ENVIRONMENT".to_owned(),
                files: "FILES".to_owned(),
                precedence: "PRECEDENCE".to_owned(),
                exit_status: "EXIT STATUS".to_owned(),
                examples: "EXAMPLES".to_owned(),
                see_also: "SEE ALSO".to_owned(),
                commands: "COMMANDS".to_owned(),
            },
            discovery: None,
            precedence: None,
            examples: vec![],
            links: vec![],
            notes: vec![],
        },
        fields: vec![cargo_orthohelp::ir::LocalizedFieldMetadata {
            name: "port".to_owned(),
            help: "Port used by the fixture service.".to_owned(),
            long_help: None,
            value: Some(ValueType::Integer {
                bits: 16,
                signed: false,
            }),
            default: Some(DefaultValue {
                display: "8080".to_owned(),
            }),
            required: false,
            deprecated: None,
            cli: Some(CliMetadata {
                long: Some("port".to_owned()),
                short: Some('p'),
                value_name: None,
                multiple: false,
                takes_value: true,
                possible_values: vec![],
                hide_in_help: false,
            }),
            env: Some(EnvMetadata {
                var_name: "FIXTURE_PORT".to_owned(),
            }),
            file: Some(FileMetadata {
                key_path: "server.port".to_owned(),
            }),
            examples: vec![],
            links: vec![],
            notes: vec![],
        }],
        subcommands: vec![],
        windows: None,
    }
}

#[rstest]
fn powershell_outputs_match_goldens(
    minimal_doc: LocalizedDocMetadata,
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let out_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .expect("temp dir path should be utf-8");

    let config = PowerShellConfig {
        out_dir: out_dir.clone(),
        module_name: "FixtureHelp".to_owned(),
        module_version: "0.1.0".to_owned(),
        bin_name: "fixture".to_owned(),
        export_aliases: vec!["fixture-help".to_owned()],
        include_common_parameters: true,
        split_subcommands: false,
        help_info_uri: None,
        ensure_en_us: true,
    };

    generate(&[minimal_doc], &config).expect("generate powershell output");

    let module_root = out_dir.join("powershell").join("FixtureHelp");
    let dir = Dir::open_ambient_dir(&module_root, ambient_authority()).expect("open module root");

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

fn assert_text_matches(dir: &Dir, relative: &str, expected: &str) -> Result<(), Box<dyn Error>> {
    let mut file = dir.open(relative)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let actual = normalise_text(&buffer);
    let expected_text = normalise_text(expected.as_bytes());
    if actual != expected_text {
        return Err(format!("content mismatch for {relative}").into());
    }
    Ok(())
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
