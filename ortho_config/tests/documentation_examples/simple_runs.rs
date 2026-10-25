//! Documented examples whose contract is their output for fixed arguments.
//!
//! Each entry needs no fixture files or environment, so the table states the
//! whole contract: the arguments a reader would pass and the output the
//! surrounding prose or transcript promises.

use anyhow::{Context, Result, ensure};

use crate::workspace::{ExampleId, ExampleWorkspace};

/// A documented example whose contract is its standard output for fixed
/// arguments, with no fixture files or environment.
pub(super) struct SimpleRun {
    id: &'static str,
    args: &'static [&'static str],
    stdout: &'static str,
}

/// Simple runs, in guide order. The expected output is the value the
/// surrounding prose promises, or the transcript beside the example.
const SIMPLE_RUNS: &[SimpleRun] = &[
    SimpleRun {
        id: "readme-main",
        args: &["--host", "0.0.0.0", "--port", "3000"],
        stdout: "Listening on 0.0.0.0:3000\n",
    },
    SimpleRun {
        id: "guide-first-cli",
        args: &[
            "--host",
            "0.0.0.0",
            "--port",
            "3000",
            "--log-level",
            "debug",
        ],
        stdout: "host=0.0.0.0 port=3000 log_level=debug\n",
    },
    SimpleRun {
        id: "guide-discovery",
        args: &[],
        stdout: "port=8080\n",
    },
    SimpleRun {
        id: "guide-scoped-discovery",
        args: &[],
        stdout: "port=8080\n",
    },
    SimpleRun {
        id: "guide-load-first-outcomes",
        args: &[],
        stdout: "discovery=absent\n",
    },
    SimpleRun {
        id: "guide-subcommand",
        args: &["serve", "--port", "3000"],
        stdout: "port=Some(3000)\n",
    },
    SimpleRun {
        id: "guide-clap-only-parse",
        args: &["--format", "json"],
        stdout: "format=Some(\"json\")\n",
    },
    SimpleRun {
        id: "guide-localization",
        args: &[],
        stdout: "verbose=true\n",
    },
    SimpleRun {
        id: "guide-hermetic-discovery",
        args: &[],
        stdout: "candidate=/srv/acme/server.toml\n",
    },
    SimpleRun {
        id: "guide-source-aware-load",
        args: &[],
        stdout: "port=9000\n",
    },
    SimpleRun {
        id: "guide-compose-layers-with-sources",
        args: &[],
        stdout: "composition=ready\n",
    },
    SimpleRun {
        id: "guide-csv-env-with-source",
        args: &[],
        stdout: "host=localhost tags=[\"one\", \"two\"]\n",
    },
    SimpleRun {
        id: "guide-subcommand-merge-sources",
        args: &[],
        stdout: "port=Some(9000)\n",
    },
    SimpleRun {
        id: "guide-selected-subcommand-sources",
        args: &[],
        stdout: "",
    },
    SimpleRun {
        id: "guide-subcommand-sources",
        args: &[],
        stdout: "port=Some(9000)\n",
    },
    SimpleRun {
        id: "guide-orthohelp-metadata",
        args: &[],
        stdout: "field=host\n",
    },
    SimpleRun {
        id: "guide-behaviour-metadata",
        args: &[],
        stdout: "mutation=Some(Delete) bypass=Some(\"--force\")\n",
    },
    SimpleRun {
        id: "api-guide-policy-report-constructors",
        args: &[],
        stdout: "",
    },
    SimpleRun {
        id: "api-guide-agent-context-constructor",
        args: &[],
        stdout: "",
    },
];

/// Runs every [`SimpleRun`] against the built workspace.
pub(super) fn assert_simple_runs(workspace: &mut ExampleWorkspace) -> Result<()> {
    SIMPLE_RUNS
        .iter()
        .try_for_each(|run| assert_simple_run(workspace, run))
}

/// Runs one [`SimpleRun`] and asserts its standard output.
fn assert_simple_run(workspace: &mut ExampleWorkspace, run: &SimpleRun) -> Result<()> {
    let SimpleRun { id, args, stdout } = run;
    let output = workspace.run(ExampleId(id), args.iter())?;
    ensure!(
        output.status.success(),
        "{id} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual =
        String::from_utf8(output.stdout).with_context(|| format!("{id} stdout is UTF-8"))?;
    ensure!(
        actual == *stdout,
        "{id} stdout differed: expected {stdout:?}, got {actual:?}"
    );
    Ok(())
}
