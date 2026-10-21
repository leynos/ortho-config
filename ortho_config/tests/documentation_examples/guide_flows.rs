//! Run-time contracts for the user's-guide examples that need fixtures.
//!
//! Each flow writes the files its example reads, runs the compiled fence
//! bodies, and compares the observable output with the published text. Flows
//! that only check standard output for fixed arguments live in the simple-run
//! table in `documentation_examples_rust_tests.rs` instead.

use anyhow::{Context, Result, ensure};
use ortho_config::serde_json::{self, Value};
use ortho_config::{AgentCommand, AgentContext, InteractionMode, MutationEffect};
use std::path::Path;

use crate::documentation_examples::{DocumentedExample, documented_example};
use crate::workspace::{EnvironmentVariable, ExampleId, ExampleWorkspace, RunFile};
use crate::{Invocation, assert_run, assert_run_with_environment};

/// Binaries the guide flows add beyond the published examples.
pub(super) fn add_probes(workspace: &mut ExampleWorkspace) -> Result<()> {
    workspace.add_binary(&profile_metadata_probe())
}

/// Write a documented file into an example's working directory.
fn stage_file(
    workspace: &mut ExampleWorkspace,
    ExampleId(id): ExampleId<'_>,
    path: &str,
    example_id: &str,
) -> Result<()> {
    let file = documented_example(example_id)?;
    workspace.write_run_file(
        ExampleId(id),
        RunFile {
            path: Path::new(path),
            contents: &file.body,
        },
    )
}

const PROFILE_ID: &str = "guide-profiles";

/// The profile walkthrough's transcript matches the compiled example, and
/// each precedence tier outranks the one below it.
pub(super) fn assert_profile_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    const ID: &str = PROFILE_ID;
    stage_file(
        workspace,
        ExampleId(ID),
        ".acme.toml",
        "guide-profiles-file",
    )?;
    assert_profile_transcript(workspace)?;
    // Environment beats the profile, and the flag beats the selector variable.
    assert_run_with_environment(
        workspace,
        ExampleId(ID),
        Invocation {
            args: ["--profile", "staging"],
            environment: [
                EnvironmentVariable {
                    name: "ACME_PORT",
                    value: "7000",
                },
                EnvironmentVariable {
                    name: "ACME_PROFILE",
                    value: "missing",
                },
            ],
        },
        "profile=staging from the --profile flag\nlog_level=debug port=7000\n",
    )?;
    // `default` and an empty selector both mean "no profile".
    for selector in ["default", ""] {
        assert_run(
            workspace,
            ExampleId(ID),
            ["--profile", selector],
            "log_level=info port=8080\n",
        )?;
    }

    let unknown = workspace.run(ExampleId(ID), ["--profile", "prod"])?;
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    ensure!(
        !unknown.status.success() && stderr.contains("prod") && stderr.contains("staging"),
        "an unknown profile should fail and list the available names:\n{stderr}"
    );
    Ok(())
}

/// The published transcript matches the three runs it describes.
fn assert_profile_transcript(workspace: &mut ExampleWorkspace) -> Result<()> {
    const ID: &str = PROFILE_ID;
    let transcript = documented_example("guide-profiles-run")?;
    ensure!(
        transcript.body
            == concat!(
                "$ cargo run\n",
                "log_level=info port=8080\n",
                "$ cargo run -- --profile staging\n",
                "profile=staging from the --profile flag\n",
                "log_level=debug port=9000\n",
                "$ ACME_PROFILE=staging cargo run -- --port 3000\n",
                "profile=staging from the selector environment variable\n",
                "log_level=debug port=3000\n",
            ),
        "profile transcript drifted"
    );

    assert_run(workspace, ExampleId(ID), [], "log_level=info port=8080\n")?;
    assert_run(
        workspace,
        ExampleId(ID),
        ["--profile", "staging"],
        "profile=staging from the --profile flag\nlog_level=debug port=9000\n",
    )?;
    assert_run_with_environment(
        workspace,
        ExampleId(ID),
        Invocation {
            args: ["--port", "3000"],
            environment: [EnvironmentVariable {
                name: "ACME_PROFILE",
                value: "staging",
            }],
        },
        "profile=staging from the selector environment variable\nlog_level=debug port=3000\n",
    )
}

/// The collection example reads the documented file, and maps merge by key.
pub(super) fn assert_collection_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    const ID: &str = "guide-collections";
    stage_file(
        workspace,
        ExampleId(ID),
        ".acme.toml",
        "guide-collection-file",
    )?;
    assert_run(
        workspace,
        ExampleId(ID),
        [],
        concat!(
            "worker=queue-a concurrency=4\n",
            "worker=queue-b concurrency=2\n",
            "label region=eu-west\n",
            "label tier=worker\n",
        ),
    )?;
    // A higher layer replaces only the keys it names under `keyed`.
    assert_run_with_environment(
        workspace,
        ExampleId(ID),
        Invocation {
            args: [],
            environment: [EnvironmentVariable {
                name: "ACME_LABELS__TIER",
                value: "api",
            }],
        },
        concat!(
            "worker=queue-a concurrency=4\n",
            "worker=queue-b concurrency=2\n",
            "label region=eu-west\n",
            "label tier=api\n",
        ),
    )
}

/// The localization example prints the identifiers the guide publishes.
pub(super) fn assert_localization_id_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    let published = documented_example("guide-localization-ids-output")?;
    ensure!(published.language == "plaintext");
    assert_run(
        workspace,
        ExampleId("guide-localization-ids"),
        [],
        &published.body,
    )
}

/// The behaviour checker turns an undeclared `purge` command into the report
/// the guide publishes.
pub(super) fn assert_behaviour_check_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    const ID: &str = "guide-check-command";
    let mut context = AgentContext::new("acme");
    context.commands.push(AgentCommand {
        path: vec!["purge".to_owned()],
        summary: None,
        canonical_verb: None,
        inputs: Vec::new(),
        output_modes: Vec::new(),
        interaction_mode: InteractionMode::Unknown,
        mutation_effect: MutationEffect::Unknown,
        bypass_flag: None,
        dry_run_flag: None,
        async_submission: None,
        delivery_route: None,
        pagination: None,
        examples: Vec::new(),
    });
    let context_json = serde_json::to_string(&context)?;
    workspace.write_run_file(
        ExampleId(ID),
        RunFile {
            path: Path::new("agent-context.json"),
            contents: &context_json,
        },
    )?;

    let output = workspace.run(ExampleId(ID), std::iter::empty::<&str>())?;
    ensure!(
        output.status.success(),
        "{ID} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed: Value = serde_json::from_slice(&output.stdout).context("parse printed report")?;
    let published = documented_example("guide-check-report")?;
    let expected: Value =
        serde_json::from_str(&published.body).context("parse published report")?;
    ensure!(
        printed == expected,
        "published behaviour report drifted\nexpected: {expected:#}\nactual: {printed:#}"
    );
    Ok(())
}

/// The profile metadata JSON is what the bridge emits for an `APP_` struct.
pub(super) fn assert_profile_metadata_flow(workspace: &mut ExampleWorkspace) -> Result<()> {
    let output = workspace.run(
        ExampleId("profile-metadata-probe"),
        std::iter::empty::<&str>(),
    )?;
    ensure!(
        output.status.success(),
        "profile metadata probe failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let context: Value = serde_json::from_slice(&output.stdout)?;
    let published = documented_example("guide-profile-metadata")?;
    let expected: Value = serde_json::from_str(&published.body)?;
    ensure!(
        context.get("profiles") == expected.get("profiles"),
        "published profile metadata drifted: {context:#}"
    );
    let commands = context
        .get("commands")
        .and_then(Value::as_array)
        .context("agent context should list commands")?;
    ensure!(
        !commands.is_empty()
            && commands.iter().all(|command| {
                command
                    .get("inputs")
                    .and_then(Value::as_array)
                    .is_some_and(|inputs| {
                        inputs.iter().any(|input| {
                            input.get("long").and_then(Value::as_str) == Some("profile")
                        })
                    })
            }),
        "every command should expose the profile input: {context:#}"
    );
    Ok(())
}

/// Builds a probe that bridges a profile-enabled `APP_` struct to agent context.
fn profile_metadata_probe() -> DocumentedExample {
    DocumentedExample {
        id: "profile-metadata-probe".to_owned(),
        language: "rust".to_owned(),
        body: concat!(
            "use ortho_config::{OrthoConfig, OrthoConfigDocs};\n",
            "use serde::{Deserialize, Serialize};\n\n",
            "#[derive(Deserialize, Serialize, OrthoConfig)]\n",
            "#[ortho_config(prefix = \"APP_\", profiles)]\n",
            "struct Config {\n",
            "    #[ortho_config(default = 1)]\n",
            "    retries: u8,\n",
            "}\n\n",
            "fn main() {\n",
            "    // The bridge reads the IR as JSON, so round-trip it the same way.\n",
            "    let ir = serde_json::to_string(&Config::get_doc_metadata()).expect(\"serialize IR\");\n",
            "    let metadata: cargo_orthohelp::schema::DocMetadata =\n",
            "        serde_json::from_str(&ir).expect(\"read IR\");\n",
            "    let context = cargo_orthohelp::agent_context::bridge_ir_to_agent_context(\n",
            "        &metadata, \"app\", None,\n",
            "    );\n",
            "    println!(\"{}\", serde_json::to_string(&context).expect(\"serialize context\"));\n",
            "}\n",
        )
        .to_owned(),
        source: "profile metadata probe",
        line: 1,
    }
}
