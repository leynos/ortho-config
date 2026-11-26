//! Locale-scoped agent-context step definitions.
//!
//! Kept in a sibling module so the core agent-context steps stay within the
//! repository's file-length limit.

use rstest_bdd_macros::{then, when};
use serde_json::Value;

use super::steps::{OrthoHelpContext, StepResult, get_out_dir};
use super::steps_agent_context::{
    BASE_AGENT_CONTEXT_ARGS, JsonField, expect_str_field, run_with_agent_context_args,
};

/// Runs the fixture with an explicit `fr-FR` summary locale.
#[when("I run cargo-orthohelp with format agent-context for the fixture in locale fr-FR")]
fn run_with_format_agent_context_fr_fr(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let mut args = Vec::from(BASE_AGENT_CONTEXT_ARGS);
    args.extend(["--locale", "fr-FR"]);
    run_with_agent_context_args(orthohelp_context, &args)
}

/// The artefact, not only stdout, carries the requested locale's summary.
#[then("the agent-context root summary is the French fixture summary")]
fn agent_context_root_summary_is_french(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let json = read_agent_context_artefact(orthohelp_context)?;
    let command = json
        .get(JsonField::Commands.as_str())
        .and_then(Value::as_array)
        .and_then(|commands| commands.first())
        .ok_or("first command missing from agent-context.json")?;
    expect_str_field(
        command,
        JsonField::Summary,
        "Configuration du fixture Orthohelp.",
    )
}

/// Reads and parses the scenario's generated `agent-context.json`.
///
/// # Errors
///
/// Returns an error when the output directory is unavailable, the artefact is
/// absent or unreadable, or its contents are not valid JSON.
fn read_agent_context_artefact(orthohelp_context: &OrthoHelpContext) -> StepResult<Value> {
    let out_dir = get_out_dir(orthohelp_context)?;
    let directory =
        cap_std::fs_utf8::Dir::open_ambient_dir(out_dir.as_path(), cap_std::ambient_authority())?;
    let json = directory.read_to_string("agent-context.json")?;
    Ok(serde_json::from_str(&json)?)
}
