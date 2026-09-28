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
    let first = "run-first.toml";
    let second = "run-second.toml";
    for (name, contents) in [(first, "port = 1111\n"), (second, "port = 2222\n")] {
        workspace.write_run_file(
            ExampleId(ALIAS_EXAMPLE),
            RunFile {
                path: Path::new(name),
                contents,
            },
        )?;
    }
    workspace.write_run_file(
        ExampleId(ALIAS_EXAMPLE),
        RunFile {
            path: Path::new("xdg/acme/config.toml"),
            contents: "port = 3333\n",
        },
    )?;

    let first_path = workspace.path_in_root(&format!("run-{ALIAS_EXAMPLE}/{first}"))?;
    let second_path = workspace.path_in_root(&format!("run-{ALIAS_EXAMPLE}/{second}"))?;
    let first = first_path.to_str().context("selector path is UTF-8")?;
    let second = second_path.to_str().context("selector path is UTF-8")?;

    let cases: [(&str, &str, &str); 3] = [
        (first, second, "port=1111\n"),
        ("", second, "port=2222\n"),
        (first, first, "port=1111\n"),
    ];
    for (first_value, second_value, expected) in cases {
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
    }
    Ok(())
}
