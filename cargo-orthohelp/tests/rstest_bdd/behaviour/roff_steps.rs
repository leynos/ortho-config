//! Roff man page generation step definitions.

use std::io::Read;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::{then, when};

use super::steps::{OrthoHelpContext, StepResult, get_out_dir, run_orthohelp};

/// A section heading recognised by the roff formatter.
#[derive(Debug, Clone)]
enum ManSection {
    Name,
    Synopsis,
    Description,
    Options,
    Other(String),
}

impl std::str::FromStr for ManSection {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "NAME" => Self::Name,
            "SYNOPSIS" => Self::Synopsis,
            "DESCRIPTION" => Self::Description,
            "OPTIONS" => Self::Options,
            other => Self::Other(other.to_owned()),
        })
    }
}

impl ManSection {
    /// Returns the heading string as it will appear in the generated roff output.
    fn expected_heading(&self) -> String {
        match self {
            Self::Name => "[missing: ortho.headings.name]".to_owned(),
            Self::Synopsis => "[missing: ortho.headings.synopsis]".to_owned(),
            Self::Description => "[missing: ortho.headings.description]".to_owned(),
            Self::Options => "[missing: ortho.headings.options]".to_owned(),
            Self::Other(s) => s.clone(),
        }
    }
}

fn run_format_step(orthohelp_context: &mut OrthoHelpContext, args: &[&str]) -> StepResult<()> {
    let output = run_orthohelp(orthohelp_context, args)?;
    assert_orthohelp_succeeded(&output);
    orthohelp_context.last_output.set(output);
    Ok(())
}

fn assert_orthohelp_succeeded(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "cargo-orthohelp should succeed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[when("I run cargo-orthohelp with format man for the fixture")]
fn run_with_format_man(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_format_step(
        orthohelp_context,
        &[
            "--format",
            "man",
            "--package",
            "orthohelp_fixture",
            "--locale",
            "en-US",
        ],
    )
}

#[when("I run cargo-orthohelp with format man and section {section} for the fixture")]
fn run_with_format_man_section(
    orthohelp_context: &mut OrthoHelpContext,
    section: u8,
) -> StepResult<()> {
    let section_str = section.to_string();
    run_format_step(
        orthohelp_context,
        &[
            "--format",
            "man",
            "--man-section",
            &section_str,
            "--package",
            "orthohelp_fixture",
            "--locale",
            "en-US",
        ],
    )
}

#[when("I run cargo-orthohelp with format man for en-US and fr-FR")]
fn run_with_format_man_multiple_locales(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    run_format_step(
        orthohelp_context,
        &[
            "--format",
            "man",
            "--package",
            "orthohelp_fixture",
            "--locale",
            "en-US",
            "--locale",
            "fr-FR",
        ],
    )
}

#[when("I run cargo-orthohelp with format all for the fixture")]
fn run_with_format_all(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_format_step(
        orthohelp_context,
        &[
            "--format",
            "all",
            "--package",
            "orthohelp_fixture",
            "--locale",
            "en-US",
        ],
    )
}

fn read_man_page_content(
    orthohelp_context: &mut OrthoHelpContext,
    relative_path: &Utf8PathBuf,
) -> StepResult<String> {
    let out_root = get_out_dir(orthohelp_context)?;
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())?;
    let mut file = dir.open(relative_path).map_err(|e| {
        format!(
            "man page should exist at {}: {e}",
            out_root.join(relative_path)
        )
    })?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

#[then("the output contains a man page for {name}")]
fn output_contains_man_page(
    orthohelp_context: &mut OrthoHelpContext,
    name: String,
) -> StepResult<()> {
    let relative_path = Utf8PathBuf::from(format!("man/man1/{name}.1"));
    let content = read_man_page_content(orthohelp_context, &relative_path)?;

    assert!(
        content.contains(".TH"),
        "man page should contain .TH header"
    );
    Ok(())
}

#[then("the output contains a localised man page for {locale} and {name}")]
fn output_contains_localised_man_page(
    orthohelp_context: &mut OrthoHelpContext,
    locale: String,
    name: String,
) -> StepResult<()> {
    let relative_path = Utf8PathBuf::from(format!("{locale}/man/man1/{name}.1"));
    let content = read_man_page_content(orthohelp_context, &relative_path)?;

    assert!(
        content.contains(&format!(".TH \"{}\" \"1\"", name.to_uppercase())),
        "man page should have the default section 1 header"
    );
    Ok(())
}

#[then("the output contains a man page at section {section} for {name}")]
fn output_contains_man_page_section(
    orthohelp_context: &mut OrthoHelpContext,
    section: u8,
    name: String,
) -> StepResult<()> {
    let relative_path = Utf8PathBuf::from(format!("man/man{section}/{name}.{section}"));
    let content = read_man_page_content(orthohelp_context, &relative_path)?;

    assert!(
        content.contains(&format!(".TH \"{}\" \"{section}\"", name.to_uppercase())),
        "man page should have correct .TH header for section {section}"
    );
    Ok(())
}

#[then("the man page for {name} contains section {section_name}")]
fn man_page_contains_section(
    orthohelp_context: &mut OrthoHelpContext,
    name: String,
    section_name: ManSection,
) -> StepResult<()> {
    let relative_path = Utf8PathBuf::from(format!("man/man1/{name}.1"));
    let content = read_man_page_content(orthohelp_context, &relative_path)?;

    let expected_heading = section_name.expected_heading();
    assert!(
        content.contains(&format!(".SH {expected_heading}")),
        "man page should contain .SH {expected_heading} section"
    );
    Ok(())
}
