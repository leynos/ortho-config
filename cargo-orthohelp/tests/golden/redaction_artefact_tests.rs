//! Golden and sentinel coverage for the profile-redaction artefact families.
//!
//! The behavioural suite states the disclosure contract; this suite pins the
//! rendered output. It runs the real binary over both locales, proves that no
//! protected literal survives into a man page or `PowerShell` help file, and
//! snapshots the English artefacts so an unrelated regression in the renderer
//! is visible as a diff rather than as a silent change in what readers see.

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use insta::assert_snapshot;
use std::error::Error;
use std::process::{Command, Output};
use std::time::Duration;
use tempfile::TempDir;

use crate::fixtures;
use crate::fixtures::profile_redaction_sentinels::{
    ARTEFACT_LOCALES, REDACTION_ROOT_TYPE, all_protected_sentinels,
};

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

/// Fixed header date, so the roff snapshot does not drift with the clock.
const ARTEFACT_DATE: &str = "2026-11-16";

/// Budget for one `--format all` run over both locales.
///
/// The run renders every artefact family, so it is given a wider allowance
/// than a single-format dispatch test; a saturated host is the realistic case
/// this bound has to survive rather than a tight upper limit.
const ARTEFACT_RUN_DEADLINE: Duration = Duration::from_secs(600);

#[test]
fn rendered_artefacts_publish_no_protected_literal() -> TestResult {
    let out_dir = tempfile::tempdir()?;
    let output = run_all_formats(&out_dir)?;
    if !output.status.success() {
        return Err(format!(
            "cargo-orthohelp should succeed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let root = utf8_path(out_dir.path())?;
    let rendered = read_rendered_artefacts(&root)?;
    assert_no_protected_literal(&rendered)?;
    let roff_en = rendered_content(&rendered, "en-US roff")?;
    let maml_en = rendered_content(&rendered, "en-US MAML")?;
    assert_public_roff_metadata(roff_en)?;
    assert_public_maml_metadata(maml_en)?;
    assert_snapshot!("redaction_roff_en_us", roff_en);
    assert_snapshot!("redaction_maml_en_us", maml_en);
    Ok(())
}

fn run_all_formats(out_dir: &TempDir) -> Result<Output, Box<dyn Error + Send + Sync>> {
    let executable = fixtures::cargo_orthohelp_exe()?;
    let mut command = Command::new(executable.as_str());
    command
        .current_dir(fixtures::workspace_root()?.as_std_path())
        .args([
            "orthohelp",
            "--out-dir",
            out_dir
                .path()
                .to_str()
                .ok_or("output path should be UTF-8")?,
            "--format",
            "all",
            "--man-date",
            ARTEFACT_DATE,
            "--package",
            "orthohelp_fixture",
            "--root-type",
            REDACTION_ROOT_TYPE,
        ]);
    for locale in ARTEFACT_LOCALES {
        command.args(["--locale", locale]);
    }
    // The shared runner owns the deadline, both stream captures, and cleanup of
    // any descendant the renderer leaves behind, so this test cannot leak a
    // process tree or hang on a full pipe.
    Ok(test_helpers::process::run_with_deadline(
        command,
        ARTEFACT_RUN_DEADLINE,
        "cargo-orthohelp --format all",
    )?)
}

/// Reads every artefact family `--format all` emits.
///
/// The IR documents are included deliberately: a slot that survives the
/// projection but never reaches a renderer is still disclosed to any consumer
/// that reads the machine-readable artefact.
fn read_rendered_artefacts(
    root: &Utf8PathBuf,
) -> Result<Vec<(String, String)>, Box<dyn Error + Send + Sync>> {
    let mut artefacts = ARTEFACT_LOCALES
        .iter()
        .flat_map(|locale| {
            [
                (format!("{locale} IR"), format!("ir/{locale}.json")),
                (
                    format!("{locale} roff"),
                    format!("{locale}/man/man1/redaction_fixture.1"),
                ),
                (
                    format!("{locale} MAML"),
                    format!("powershell/redaction_fixture/{locale}/redaction_fixture-help.xml"),
                ),
            ]
        })
        .map(|(label, relative)| Ok((label, read_text(root, &relative)?)))
        .collect::<Result<Vec<_>, Box<dyn Error + Send + Sync>>>()?;
    artefacts.push((
        "agent context".to_owned(),
        read_text(root, "agent-context.json")?,
    ));
    Ok(artefacts)
}

fn rendered_content<'a>(
    rendered: &'a [(String, String)],
    label: &str,
) -> Result<&'a str, Box<dyn Error + Send + Sync>> {
    rendered
        .iter()
        .find(|(name, _)| name == label)
        .map(|(_, content)| content.as_str())
        .ok_or_else(|| format!("{label} artefact missing").into())
}

fn assert_no_protected_literal(rendered: &[(String, String)]) -> TestResult {
    for (label, content) in rendered {
        for sentinel in all_protected_sentinels() {
            require(
                !content.contains(sentinel),
                format!("protected sentinel {sentinel} leaked into {label}"),
            )?;
        }
    }
    Ok(())
}

fn assert_public_roff_metadata(roff: &str) -> TestResult {
    for expected in ["eu-west", "fast, safe", "--token"] {
        require(
            roff.contains(expected),
            format!("the roff page should retain {expected:?}"),
        )?;
    }
    // The protected fields stay documented by name; only their values vanish.
    require(
        roff.contains("Confidential token used by the fixture service."),
        "the roff page should keep the protected field's description",
    )
}

fn assert_public_maml_metadata(maml: &str) -> TestResult {
    for expected in ["eu-west", "fast", "token"] {
        require(
            maml.contains(expected),
            format!("the MAML help should retain {expected:?}"),
        )?;
    }
    Ok(())
}

fn utf8_path(path: &std::path::Path) -> Result<Utf8PathBuf, Box<dyn Error + Send + Sync>> {
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .map_err(|failed| format!("non-UTF-8 output path: {}", failed.display()).into())
}

fn read_text(root: &Utf8PathBuf, relative: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let dir = Dir::open_ambient_dir(root, ambient_authority())?;
    Ok(dir.read_to_string(relative)?)
}
fn require(condition: bool, message: impl Into<String>) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}
