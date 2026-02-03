//! Roff man page generation step definitions.

use std::io::Read;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::{then, when};

use super::steps::{get_out_dir, run_orthohelp, OrthoHelpContext};

#[when("I run cargo-orthohelp with format man for the fixture")]
fn run_with_format_man(orthohelp_context: &mut OrthoHelpContext) {
    let output = run_orthohelp(
        orthohelp_context,
        &["--format", "man", "--package", "orthohelp_fixture", "--locale", "en-US"],
    );
    assert!(output.status.success(), "cargo-orthohelp should succeed: {:?}", String::from_utf8_lossy(&output.stderr));
    orthohelp_context.last_output.set(output);
}

#[when("I run cargo-orthohelp with format man and section {section} for the fixture")]
fn run_with_format_man_section(orthohelp_context: &mut OrthoHelpContext, section: u8) {
    let section_str = section.to_string();
    let output = run_orthohelp(
        orthohelp_context,
        &[
            "--format", "man",
            "--man-section", &section_str,
            "--package", "orthohelp_fixture",
            "--locale", "en-US",
        ],
    );
    assert!(output.status.success(), "cargo-orthohelp should succeed: {:?}", String::from_utf8_lossy(&output.stderr));
    orthohelp_context.last_output.set(output);
}

#[when("I run cargo-orthohelp with format all for the fixture")]
fn run_with_format_all(orthohelp_context: &mut OrthoHelpContext) {
    let output = run_orthohelp(
        orthohelp_context,
        &["--format", "all", "--package", "orthohelp_fixture", "--locale", "en-US"],
    );
    assert!(output.status.success(), "cargo-orthohelp should succeed: {:?}", String::from_utf8_lossy(&output.stderr));
    orthohelp_context.last_output.set(output);
}

#[then("the output contains a man page for {name}")]
fn output_contains_man_page(orthohelp_context: &mut OrthoHelpContext, name: String) {
    let out_root = get_out_dir(orthohelp_context);
    let man_path = out_root.join(format!("man/man1/{name}.1"));
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())
        .expect("open output dir");

    let mut file = dir
        .open(&Utf8PathBuf::from(format!("man/man1/{name}.1")))
        .unwrap_or_else(|e| panic!("man page should exist at {man_path}: {e}"));

    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("read man page content");

    assert!(content.contains(".TH"), "man page should contain .TH header");
}

#[then("the output contains a man page at section {section} for {name}")]
fn output_contains_man_page_section(orthohelp_context: &mut OrthoHelpContext, section: u8, name: String) {
    let out_root = get_out_dir(orthohelp_context);
    let man_path = out_root.join(format!("man/man{section}/{name}.{section}"));
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())
        .expect("open output dir");

    let mut file = dir
        .open(&Utf8PathBuf::from(format!("man/man{section}/{name}.{section}")))
        .unwrap_or_else(|e| panic!("man page should exist at {man_path}: {e}"));

    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("read man page content");

    assert!(
        content.contains(&format!(".TH \"{}\" \"{section}\"", name.to_uppercase())),
        "man page should have correct .TH header for section {section}"
    );
}

#[then("the man page contains section {section_name}")]
fn man_page_contains_section(orthohelp_context: &mut OrthoHelpContext, section_name: String) {
    let out_root = get_out_dir(orthohelp_context);
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())
        .expect("open output dir");

    let mut file = dir
        .open(&Utf8PathBuf::from("man/man1/orthohelp_fixture.1"))
        .expect("man page should exist");

    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("read man page content");

    assert!(
        content.contains(&format!(".SH {section_name}")),
        "man page should contain .SH {section_name} section"
    );
}
