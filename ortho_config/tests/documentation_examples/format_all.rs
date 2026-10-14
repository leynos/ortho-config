//! Published-contract guard for the documented `--format all` example.
//!
//! The user's guide publishes both the combined invocation and the directory
//! tree it produces. This test keeps those two fences from drifting apart, and
//! keeps the published tree honest about what the generator writes: the
//! expected paths below were read from a real `cargo-orthohelp --format all`
//! run, and the `Generate all formats` scenario in
//! `cargo-orthohelp/tests/features/orthohelp_roff.feature` re-runs the
//! generator against a fixture, so this file does not pay for a second
//! isolated Cargo build to re-derive them.

use anyhow::{Result, ensure};

use crate::documentation_examples::documented_example;

/// Files the guide promises for a single-locale `--format all` run.
///
/// Order matches the guide's tree: root artefacts first, then `ir/`, `man/`,
/// and `powershell/`.
const EXPECTED_FILES: &[&str] = &[
    "agent-context.json",
    "ir/en-US.json",
    "man/man1/hello_world.1",
    "powershell/HelloWorld/HelloWorld.psd1",
    "powershell/HelloWorld/HelloWorld.psm1",
    "powershell/HelloWorld/en-US/HelloWorld-help.xml",
    "powershell/HelloWorld/en-US/about_HelloWorld.help.txt",
];

/// Checks the published console fence and tree fence agree with each other.
#[test]
fn documented_format_all_example_publishes_the_expected_tree() -> Result<()> {
    let command = documented_example("guide-orthohelp-format-all")?;
    ensure!(command.language == "console");
    ensure!(
        command.body == "cargo orthohelp --format all --package hello_world --locale en-US\n",
        "cargo-orthohelp combined-format command contract drifted"
    );

    let tree = documented_example("guide-orthohelp-format-all-tree")?;
    ensure!(tree.language == "plaintext");
    let published = tree_files(&tree.body);
    let expected = EXPECTED_FILES
        .iter()
        .map(|file| (*file).to_owned())
        .collect::<Vec<_>>();
    ensure!(
        published == expected,
        "published format-all tree drifted\nexpected: {expected:#?}\nactual: {published:#?}"
    );
    Ok(())
}

/// Reads the file paths a rendered tree fence draws.
///
/// The guide's tree is a drawing rather than data, so the smallest way to read
/// it back is to render it. The root line carries no elbow and is skipped;
/// every other entry carries an indent drawn from box-drawing glyphs and
/// spaces, then a `── ` elbow, then a node name. Directory entries end in `/`
/// and become the parent of the entries indented beneath them.
fn tree_files(tree: &str) -> Vec<String> {
    // An entry's parent is the nearest preceding directory indented less
    // deeply. Comparing indentation directly avoids reconstructing a numeric
    // depth level from the drawing.
    let mut ancestors: Vec<(usize, &str)> = Vec::new();
    let mut files = Vec::new();
    for line in tree.lines() {
        let Some((branch, name)) = line.split_once("── ") else {
            continue;
        };
        // Count characters rather than bytes: every box-drawing glyph is one
        // column but three bytes, so byte offsets would misjudge indentation.
        let indent = branch.chars().count();
        while ancestors
            .last()
            .is_some_and(|(parent_indent, _)| *parent_indent >= indent)
        {
            ancestors.pop();
        }
        if let Some(directory) = name.strip_suffix('/') {
            ancestors.push((indent, directory));
            continue;
        }
        let mut path = ancestors
            .iter()
            .map(|(_, directory)| *directory)
            .collect::<Vec<_>>()
            .join("/");
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(name);
        files.push(path);
    }
    files
}
