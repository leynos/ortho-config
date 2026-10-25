//! Probe binary confirming documented examples run in a sanitized environment.
//!
//! Every documented binary runs with a cleared environment plus deterministic
//! home directories, so an example cannot pass by reading the developer's own
//! configuration. The probe reports what the child actually sees.

use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};

use crate::documentation_examples::DocumentedExample;
use crate::workspace::{ExampleId, ExampleWorkspace};

/// Builds the probe binary added beside the documented examples.
pub(super) fn environment_probe() -> DocumentedExample {
    DocumentedExample {
        id: "environment-probe".to_owned(),
        language: "rust".to_owned(),
        body: concat!(
            "fn main() {\n",
            "    let current = std::env::current_dir().expect(\"read current directory\");\n",
            "    let home = std::env::var_os(\"HOME\").expect(\"HOME should be set\");\n",
            "    let xdg = std::env::var_os(\"XDG_CONFIG_HOME\").expect(\"XDG should be set\");\n",
            "    println!(\"current={}\", current.display());\n",
            "    println!(\"home={}\", std::path::Path::new(&home).display());\n",
            "    println!(\"xdg={}\", std::path::Path::new(&xdg).display());\n",
            "    println!(\"path_present={}\", std::env::var_os(\"PATH\").is_some());\n",
            "}\n",
        )
        .to_owned(),
        source: "environment probe",
        line: 1,
    }
}

/// Asserts the probe saw only the deterministic, sanitized environment.
pub(super) fn assert_sanitized_binary_environment(workspace: &mut ExampleWorkspace) -> Result<()> {
    ensure!(
        std::env::var_os("PATH").is_some(),
        "the parent test process should provide PATH"
    );
    let output = workspace.run(ExampleId("environment-probe"), std::iter::empty::<&str>())?;
    ensure!(output.status.success(), "environment probe should succeed");
    let stdout = String::from_utf8(output.stdout).context("environment probe output is UTF-8")?;
    let values = stdout
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect::<std::collections::HashMap<_, _>>();
    let value = |name| {
        values
            .get(name)
            .copied()
            .with_context(|| format!("environment probe should report {name}"))
    };
    let current = PathBuf::from(value("current")?);
    ensure!(Path::new(value("home")?) == current);
    ensure!(Path::new(value("xdg")?) == current.join("xdg"));
    ensure!(value("path_present")? == "false");
    Ok(())
}
