//! Run-time contracts for the v0.10.0 migration guide.
//!
//! The migration guide publishes before-and-after programs and transcripts.
//! These flows run the compiled fence bodies under the conditions each
//! transcript describes, so the upgrade advice fails the build if the
//! behaviour it promises drifts.

use anyhow::{Result, ensure};
use std::path::Path;

use crate::documentation_examples::documented_example;
use crate::workspace::{EnvironmentVariable, ExampleId, ExampleWorkspace, RunFile};
use crate::{Invocation, assert_run, assert_run_with_environment};

/// Published examples compiled for the migration flows.
pub(super) const MIGRATION_RUST_EXAMPLES: &[&str] = &[
    "migration-explicit-cli",
    "migration-agent-context-profiles",
    "migration-cli-default-before",
    "migration-cli-default-after",
    "migration-l10n-runtime",
];

/// Runs every migration-guide flow against the built workspace.
pub(super) fn assert_migration_flows(workspace: &mut ExampleWorkspace) -> Result<()> {
    assert_explicit_cli_flow(workspace)?;
    assert_run(
        workspace,
        ExampleId("migration-agent-context-profiles"),
        [],
        "{\"supported\":false}\n",
    )?;
    assert_boolean_flow(workspace)?;
    assert_cli_default_flow(workspace)?;
    // The standalone runtime resolves the dotted identifier through its
    // hyphenated fallback to the embedded English catalogue.
    assert_run(
        workspace,
        ExampleId("migration-l10n-runtime"),
        [],
        "OrthoConfig command-line interface.\n",
    )
}

/// The `ACME_PORT` environment the explicit-CLI transcript runs under.
const fn port_env(value: &str) -> [EnvironmentVariable<'_>; 1] {
    [EnvironmentVariable {
        name: "ACME_PORT",
        value,
    }]
}

/// Restating the default on the command line now beats the environment.
fn assert_explicit_cli_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    const ID: &str = "migration-explicit-cli";
    let transcript = documented_example("migration-explicit-cli-run")?;
    ensure!(
        transcript.body
            == concat!(
                "$ ACME_PORT=9000 cargo run -- --port 8080\n",
                "port=8080\n",
                "$ ACME_PORT=9000 cargo run\n",
                "port=9000\n",
            ),
        "explicit CLI transcript drifted"
    );
    assert_run_with_environment(
        workspace,
        ExampleId(ID),
        Invocation {
            args: ["--port", "8080"],
            environment: port_env("9000"),
        },
        "port=8080\n",
    )?;
    assert_run_with_environment(
        workspace,
        ExampleId(ID),
        Invocation {
            args: [],
            environment: port_env("9000"),
        },
        "port=9000\n",
    )
}

/// `--excited=false` clears a configured `true`; the spaced form is rejected.
///
/// The flow reuses the user's guide example the migration guide points to,
/// and runs after `boolean_override` so its staged file cannot leak into the
/// file-free cases there.
fn assert_boolean_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    const ID: &str = "guide-boolean-override";
    let file = documented_example("migration-boolean-file")?;
    workspace.write_run_file(
        ExampleId(ID),
        RunFile {
            path: Path::new(".acme.toml"),
            contents: &file.body,
        },
    )?;
    let transcript = documented_example("migration-boolean-run")?;
    ensure!(
        transcript.body
            == concat!(
                "$ cargo run -- --excited=false\n",
                "excited=false\n",
                "$ cargo run\n",
                "excited=true\n",
            ),
        "boolean transcript drifted"
    );
    assert_run(
        workspace,
        ExampleId(ID),
        ["--excited=false"],
        "excited=false\n",
    )?;
    assert_run(workspace, ExampleId(ID), [], "excited=true\n")?;

    // The example returns the error from `main`, so Rust prints its `Debug`
    // form; clap renders the same `UnknownArgument` kind as "unexpected
    // argument 'false' found".
    let spaced = workspace.run(ExampleId(ID), ["--excited", "false"])?;
    let stderr = String::from_utf8_lossy(&spaced.stderr);
    ensure!(
        !spaced.status.success()
            && stderr.contains("kind: UnknownArgument")
            && stderr.contains("String(\"false\")"),
        "the space-separated boolean value should be rejected:\n{stderr}"
    );
    Ok(())
}

/// The inferred clap default sits below the environment and explicit flags.
///
/// Compiling `migration-cli-default-after` in the example crate is also the
/// downstream regression for `cli_default_as_absent` expanding without a
/// feature test; `workspace_tests` pins the crate shape that makes it one.
fn assert_cli_default_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    const ID: &str = "migration-cli-default-after";
    assert_run(
        workspace,
        ExampleId("migration-cli-default-before"),
        [],
        "port=8080\n",
    )?;
    let transcript = documented_example("migration-cli-default-run")?;
    ensure!(
        transcript.body
            == concat!(
                "$ cargo run\n",
                "port=8080\n",
                "$ ACME_PORT=9000 cargo run\n",
                "port=9000\n",
                "$ ACME_PORT=9000 cargo run -- --port 7000\n",
                "port=7000\n",
            ),
        "clap-default transcript drifted"
    );
    assert_run(workspace, ExampleId(ID), [], "port=8080\n")?;
    assert_run_with_environment(
        workspace,
        ExampleId(ID),
        Invocation {
            args: [],
            environment: port_env("9000"),
        },
        "port=9000\n",
    )?;
    assert_run_with_environment(
        workspace,
        ExampleId(ID),
        Invocation {
            args: ["--port", "7000"],
            environment: port_env("9000"),
        },
        "port=7000\n",
    )
}
