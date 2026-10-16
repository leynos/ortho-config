//! Compile and assert the API-guide examples against exported schema constants.

use super::documentation_examples::DocumentedExample;
use super::workspace::{ExampleId, ExampleWorkspace};
use anyhow::{Context, Result, ensure};
use std::ffi::OsString;

/// Adds the schema-version probe binary to the generated example workspace.
pub(super) fn add_schema_version_probe(workspace: &mut ExampleWorkspace) -> Result<()> {
    workspace.add_binary(&schema_version_probe())
}

/// Runs the probe and returns the exported policy and agent-context versions.
pub(super) fn schema_versions(workspace: &mut ExampleWorkspace) -> Result<(String, String)> {
    let output = workspace.run(
        ExampleId("schema-version-probe"),
        std::iter::empty::<&str>(),
    )?;
    ensure!(
        output.status.success(),
        "schema-version probe failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("schema-version probe is UTF-8")?;
    let (policy, agent_context) = stdout
        .trim_end()
        .split_once('\t')
        .context("schema-version probe should print two tab-separated values")?;
    ensure!(
        !agent_context.contains('\t'),
        "schema-version probe should print exactly two values"
    );
    Ok((policy.to_owned(), agent_context.to_owned()))
}

/// Checks an example's version field and deserializes its JSON into the schema type.
pub(super) fn assert_json_schema_version(
    workspace: &mut ExampleWorkspace,
    ExampleId(id): ExampleId<'_>,
    version_field: &str,
    expected_version: &str,
) -> Result<()> {
    let output = workspace.run(ExampleId(id), std::iter::empty::<&str>())?;
    ensure!(
        output.status.success(),
        "{id} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = String::from_utf8(output.stdout).with_context(|| format!("{id} JSON is UTF-8"))?;
    let payload: ortho_config::serde_json::Value = ortho_config::serde_json::from_str(&json)
        .with_context(|| format!("{id} should print JSON"))?;
    let actual_version = payload
        .get(version_field)
        .and_then(|value| value.as_str())
        .with_context(|| format!("{id} JSON {version_field} should be a string"))?;
    ensure!(
        actual_version == expected_version,
        "{id} JSON {version_field} should equal {expected_version:?}, got {actual_version:?}"
    );
    let schema_kind = match version_field {
        "version" => "policy",
        "schema_version" => "agent-context",
        _ => anyhow::bail!("unsupported schema-version field {version_field:?}"),
    };
    let typed_validation = workspace.run(
        ExampleId("schema-version-probe"),
        [OsString::from(schema_kind), OsString::from(json)],
    )?;
    ensure!(
        typed_validation.status.success(),
        "{id} JSON failed typed schema validation:\n{}",
        String::from_utf8_lossy(&typed_validation.stderr)
    );
    Ok(())
}

/// Builds a helper binary that reports constants and validates serialized schemas.
fn schema_version_probe() -> DocumentedExample {
    DocumentedExample {
        id: "schema-version-probe".to_owned(),
        language: "rust".to_owned(),
        body: concat!(
            "fn main() -> Result<(), Box<dyn std::error::Error>> {\n",
            "    let mut arguments = std::env::args().skip(1);\n",
            "    match arguments.next().as_deref() {\n",
            "        Some(\"policy\") => {\n",
            "            let json = arguments.next().ok_or_else(|| std::io::Error::other(\"missing policy JSON\"))?;\n",
            "            let report: cargo_orthohelp::policy::PolicyReport = serde_json::from_str(&json)?;\n",
            "            assert_eq!(report.version, cargo_orthohelp::policy::ORTHO_POLICY_REPORT_SCHEMA_VERSION);\n",
            "            assert_eq!(report.summary.total, report.results.len());\n",
            "        }\n",
            "        Some(\"agent-context\") => {\n",
            "            let json = arguments.next().ok_or_else(|| std::io::Error::other(\"missing agent-context JSON\"))?;\n",
            "            let context: ortho_config::AgentContext = serde_json::from_str(&json)?;\n",
            "            assert_eq!(context.schema_version, ortho_config::agent_context::ORTHO_AGENT_CONTEXT_SCHEMA_VERSION);\n",
            "            assert_eq!(context.kind, ortho_config::agent_context_kind(&context.package));\n",
            "        }\n",
            "        None => println!(\"{}\\t{}\", cargo_orthohelp::policy::ORTHO_POLICY_REPORT_SCHEMA_VERSION, ",
            "ortho_config::agent_context::ORTHO_AGENT_CONTEXT_SCHEMA_VERSION),\n",
            "        Some(kind) => return Err(std::io::Error::other(format!(\"unknown schema kind {kind}\")).into()),\n",
            "    }\n",
            "    Ok(())\n",
            "}\n",
        )
        .to_owned(),
        source: "schema version probe",
        line: 1,
    }
}
