//! Run-time assertion for the documented `env_vars` alias chain.
//!
//! `guide-scoped-discovery` is the only derive in the tree that writes
//! `env_vars`, so this module owns the only execution coverage that attribute
//! has. It lives beside `workspace` rather than inside
//! `documentation_examples_rust_tests.rs` because that file sits close to the
//! 400-line ceiling in `AGENTS.md`.

use anyhow::{Context, Result, ensure};
use std::path::Path;

use crate::workspace::{EnvironmentVariable, ExampleId, ExampleWorkspace, RunFile};

const ALIAS_EXAMPLE: &str = "guide-scoped-discovery";

/// The documented `env_vars` chain resolves at run time, in declared order.
///
/// The three cases below are the three behaviours the attribute promises: the
/// first declared variable wins when both are set, a later one is reached when
/// the earlier is unset or empty, and a populated alias suppresses automatic
/// discovery entirely rather than merging with it. The last of those is what
/// distinguishes an explicit selection from the stacked scopes underneath it,
/// so the fixture also stages a file the automatic scopes *would* find and
/// gives it a third value: a leak then shows up as `port=3333` rather than
/// coincidentally matching a selector file.
///
/// An empty value is treated as unset by `ConfigPathSelector::resolve`, which
/// is why the middle case can leave `ACME_CONFIG_PATH` set to the empty string
/// instead of unsetting it. Every case therefore sets both variables, so the
/// child environment does not depend on what the harness happened to pass.
///
/// The selector values are **absolute** paths under this workspace's root. The
/// child's working directory is its run directory, so a bare filename would
/// resolve there and happen to work; naming the full path keeps the fixture
/// unambiguous and avoids relying on exactly the coincidence that discovery
/// elsewhere goes out of its way not to rely on.
pub(super) fn assert_env_alias_chain(workspace: &mut ExampleWorkspace) -> Result<()> {
    write_alias_fixtures(workspace)?;
    let first_selector = selector_path(workspace, FIRST_FILE)?;
    let second_selector = selector_path(workspace, SECOND_FILE)?;
    run_alias_cases(workspace, &first_selector, &second_selector)
}

/// The absolute path of a run-directory file, as a UTF-8 selector value.
fn selector_path(workspace: &ExampleWorkspace, file: &str) -> Result<String> {
    let path = workspace.path_in_root(&format!("run-{ALIAS_EXAMPLE}/{file}"))?;
    Ok(path.to_str().context("selector path is UTF-8")?.to_owned())
}

/// Run the three selector combinations: first wins, empty falls through, and
/// the alias agrees with the primary.
fn run_alias_cases(
    workspace: &mut ExampleWorkspace,
    first_selector: &str,
    second_selector: &str,
) -> Result<()> {
    let cases: [(&str, &str, &str); 3] = [
        (first_selector, second_selector, "port=1111\n"),
        ("", second_selector, "port=2222\n"),
        (first_selector, first_selector, "port=1111\n"),
    ];
    for (first_value, second_value, expected) in cases {
        assert_alias_case(workspace, first_value, second_value, expected)?;
    }
    Ok(())
}

/// The two selector files, and a decoy in the XDG location that must never win.
const FIRST_FILE: &str = "run-first.toml";
const SECOND_FILE: &str = "run-second.toml";

/// Write the two selector files and the XDG decoy into the example's run directory.
fn write_alias_fixtures(workspace: &mut ExampleWorkspace) -> Result<()> {
    let files = [
        (FIRST_FILE, "port = 1111\n"),
        (SECOND_FILE, "port = 2222\n"),
        ("xdg/acme/config.toml", "port = 3333\n"),
    ];
    for (name, contents) in files {
        workspace.write_run_file(
            ExampleId(ALIAS_EXAMPLE),
            RunFile {
                path: Path::new(name),
                contents,
            },
        )?;
    }
    Ok(())
}

/// Run the example with both selector variables set and check its output.
fn assert_alias_case(
    workspace: &mut ExampleWorkspace,
    first_value: &str,
    second_value: &str,
    expected: &str,
) -> Result<()> {
    let output = workspace.run_with_environment(
        ExampleId(ALIAS_EXAMPLE),
        std::iter::empty::<&str>(),
        [
            EnvironmentVariable {
                name: "ACME_CONFIG_PATH",
                value: first_value,
            },
            EnvironmentVariable {
                name: "ACME_LEGACY_CONFIG",
                value: second_value,
            },
        ],
    )?;
    ensure!(
        output.status.success(),
        "{} failed with ACME_CONFIG_PATH={first_value:?} and \
         ACME_LEGACY_CONFIG={second_value:?}:\n{}",
        ALIAS_EXAMPLE,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("alias stdout is UTF-8")?;
    ensure!(
        stdout == expected,
        "alias chain with ACME_CONFIG_PATH={first_value:?} and \
         ACME_LEGACY_CONFIG={second_value:?}: expected {expected:?}, got {stdout:?}"
    );
    Ok(())
}
