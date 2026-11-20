//! Fixture catalogue and artefact inspection for profile-redaction scenarios.
//!
//! The scenarios in `profile_redaction.feature` need three things that a
//! generic step module cannot supply: the exact sentinel strings the fixture
//! declares, a recursive read of every generated file, and in-process
//! construction of two consumer contexts that differ only in profile bodies.
//!
//! Keeping them here lets the step module read as the Gherkin text does. The
//! sentinel catalogue itself lives in [`crate::fixtures`] because the golden
//! and contract suites scan the same literals.

use std::collections::{BTreeMap, BTreeSet};

use cap_std::{ambient_authority, fs_utf8::Dir};
use ortho_config::{AgentContext, ProfileName, ProfileSelectionContract, ProfilesDeclaration};
use serde_json::Value;

use super::steps::{OrthoHelpContext, StepResult, get_out_dir, run_orthohelp, scenario_target_dir};

pub(super) use crate::fixtures::profile_redaction_sentinels::{
    ARTEFACT_LOCALES, COMMAND_EXAMPLE_SENTINELS, ORDINARY_ROOT_TYPE, REDACTION_ROOT_TYPE,
    REFERENCE_SENTINELS, SECRET_SENTINELS, UNSUPPORTED_ROOT_TYPE, UNSUPPORTED_SENTINELS,
    all_protected_sentinels,
};

/// Bodies attached to the same public names in the two catalogues.
const CATALOGUE_A_BODIES: [(&str, &str); 2] = [
    ("dev", "PROFILE-BODY-ALPHA-SENTINEL"),
    ("prod", "PROFILE-BODY-GAMMA-SENTINEL"),
];
const CATALOGUE_B_BODIES: [(&str, &str); 2] = [
    ("dev", "PROFILE-BODY-OMEGA-SENTINEL"),
    ("prod", "PROFILE-BODY-ZETA-SENTINEL"),
];

pub(super) use super::profile_redaction_artefacts::{
    assert_artefact_inventory, assert_sentinels_absent, read_artefacts,
};

/// Fails the scenario when `condition` is false.
pub(super) fn require(condition: bool, message: impl Into<String>) -> StepResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

/// Runs `--format all` for `root_type` in both artefact locales.
pub(super) fn run_all_formats(
    orthohelp_context: &mut OrthoHelpContext,
    root_type: &str,
) -> StepResult<()> {
    let mut args = vec!["--format", "all", "--package", "orthohelp_fixture"];
    args.extend(["--root-type", root_type]);
    for locale in ARTEFACT_LOCALES {
        args.extend(["--locale", locale]);
    }
    run_checked(orthohelp_context, &args)
}

/// Runs `--format agent-context`, which writes the context to stdout.
pub(super) fn run_agent_context(
    orthohelp_context: &mut OrthoHelpContext,
    root_type: &str,
) -> StepResult<()> {
    run_checked(
        orthohelp_context,
        &[
            "--format",
            "agent-context",
            "--package",
            "orthohelp_fixture",
            "--root-type",
            root_type,
        ],
    )
}

/// Runs `cargo-orthohelp`, failing the scenario unless it succeeds.
pub(super) fn run_checked(
    orthohelp_context: &mut OrthoHelpContext,
    args: &[&str],
) -> StepResult<()> {
    let output = run_orthohelp(orthohelp_context, args)?;
    if !output.status.success() {
        return Err(format!(
            "cargo-orthohelp should succeed: {:?}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    orthohelp_context.last_output.set(output);
    Ok(())
}

/// Reads one JSON artefact written beneath the output directory.
pub(super) fn read_out_json(
    orthohelp_context: &OrthoHelpContext,
    relative: &str,
) -> StepResult<Value> {
    let out_dir = get_out_dir(orthohelp_context)?;
    let dir = Dir::open_ambient_dir(&out_dir, ambient_authority())?;
    Ok(serde_json::from_slice(&dir.read(relative)?)?)
}

/// Reads the agent-context document a `--format agent-context` run wrote out.
pub(super) fn read_stdout_json(orthohelp_context: &OrthoHelpContext) -> StepResult<Value> {
    let bytes = orthohelp_context
        .last_output
        .with_ref(|output| output.stdout.clone())
        .ok_or("last_output should be set")?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Reads the per-locale IR documents produced by `--format all`.
pub(super) fn read_irs(orthohelp_context: &OrthoHelpContext) -> StepResult<Vec<(String, Value)>> {
    ARTEFACT_LOCALES
        .iter()
        .map(|locale| {
            let document = read_out_json(orthohelp_context, &format!("ir/{locale}.json"))?;
            Ok(((*locale).to_owned(), document))
        })
        .collect()
}

/// Every command node in a document tree, including the root.
pub(super) fn command_nodes(root: &Value) -> Vec<&Value> {
    let mut nodes = Vec::new();
    push_command_nodes(root, &mut nodes);
    nodes
}

fn push_command_nodes<'a>(node: &'a Value, nodes: &mut Vec<&'a Value>) {
    nodes.push(node);
    if let Some(children) = node.get("subcommands").and_then(Value::as_array) {
        for child in children {
            push_command_nodes(child, nodes);
        }
    }
}

/// Finds a field by name on one command node of the documentation IR.
pub(super) fn ir_field<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    node.get("fields")?
        .as_array()?
        .iter()
        .find(|field| field.get("name").and_then(Value::as_str) == Some(name))
}

/// Finds a command by its tokenised path in the agent-context document.
pub(super) fn context_command<'a>(context: &'a Value, path: &[&str]) -> Option<&'a Value> {
    context.get("commands")?.as_array()?.iter().find(|command| {
        command
            .get("path")
            .and_then(Value::as_array)
            .is_some_and(|actual| {
                actual
                    .iter()
                    .filter_map(Value::as_str)
                    .eq(path.iter().copied())
            })
    })
}

/// Returns the named agent-context input, if the command exposes it.
pub(super) fn context_input<'a>(command: &'a Value, name: &str) -> Option<&'a Value> {
    command
        .get("inputs")?
        .as_array()?
        .iter()
        .find(|input| input.get("name").and_then(Value::as_str) == Some(name))
}

/// Builds two contexts whose catalogues share names but differ in every body.
///
/// The bodies exist only in this function's local maps: [`ProfilesDeclaration`]
/// accepts names, so a consumer holding a catalogue cannot pass its contents
/// through. Returning the bodies lets a scenario assert their absence.
pub(super) fn paired_named_contexts() -> StepResult<(AgentContext, AgentContext, Vec<&'static str>)>
{
    let catalogue_a = CATALOGUE_A_BODIES.into_iter().collect::<BTreeMap<_, _>>();
    // Reverse insertion order: the declaration, not the caller, canonicalises.
    let catalogue_b = CATALOGUE_B_BODIES
        .into_iter()
        .rev()
        .collect::<BTreeMap<_, _>>();
    let contract = ProfileSelectionContract {
        flag: "profile".to_owned(),
        env_var: "REDACTION_FIXTURE_PROFILE".to_owned(),
    };
    let first = context_with_names(&contract, catalogue_a.keys().copied())?;
    let second = context_with_names(&contract, catalogue_b.keys().copied())?;
    let bodies = CATALOGUE_A_BODIES
        .into_iter()
        .chain(CATALOGUE_B_BODIES)
        .map(|(_, body)| body)
        .collect();
    Ok((first, second, bodies))
}

fn context_with_names<'a>(
    contract: &ProfileSelectionContract,
    names: impl Iterator<Item = &'a str>,
) -> StepResult<AgentContext> {
    let validated = names
        .map(|name| ProfileName::new(name).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut context = AgentContext::new("redaction-fixture");
    context.profiles =
        ProfilesDeclaration::supported(contract.clone()).with_available_names(validated);
    Ok(context)
}

/// Serialises a context with the wire encoder consumers actually receive.
pub(super) fn serialized(context: &AgentContext) -> StepResult<String> {
    Ok(serde_json::to_string(context)?)
}

/// Returns the sorted set of sentinel categories present in a list.
pub(super) fn sentinel_categories(sentinels: &[&str]) -> BTreeSet<&'static str> {
    sentinels
        .iter()
        .filter_map(|sentinel| {
            ["DEFAULT", "ENUM", "EXAMPLE"]
                .into_iter()
                .find(|category| sentinel.contains(category))
        })
        .collect()
}
/// Directory the pre-change cache scenario fabricates beneath the cache root.
const PRE_CHANGE_CACHE_DIR: &str = "pre-change-entry";

/// Literal no current generator can produce, so finding it proves reuse.
pub(super) const PRE_CHANGE_CACHE_SENTINEL: &str = "STALE-CACHE-SENTINEL";

/// Writes a cache entry shaped like the pre-redaction generator's output.
///
/// The directory name is deliberately unrelated to the real cache key. The
/// scenario proves that an entry written before the disclosure salt existed is
/// not selected, even though it sits under the live cache root.
pub(super) fn seed_pre_change_cache(ctx: &OrthoHelpContext) -> StepResult<()> {
    let cache_root = scenario_target_dir(ctx)?.join("orthohelp");
    Dir::create_ambient_dir_all(&cache_root, ambient_authority())?;
    let dir = Dir::open_ambient_dir(cache_root.as_path(), ambient_authority())?;
    dir.create_dir(PRE_CHANGE_CACHE_DIR)?;
    let body = serde_json::json!({
        "ir_version": ortho_config::docs::ORTHO_DOCS_IR_VERSION,
        "app_name": "redaction_fixture",
        "about_id": PRE_CHANGE_CACHE_SENTINEL,
    });
    dir.write(format!("{PRE_CHANGE_CACHE_DIR}/ir.json"), body.to_string())?;
    Ok(())
}

/// Reads the fabricated pre-change entry back.
pub(super) fn read_pre_change_cache(ctx: &OrthoHelpContext) -> StepResult<String> {
    let cache_root = scenario_target_dir(ctx)?.join("orthohelp");
    let dir = Dir::open_ambient_dir(cache_root.as_path(), ambient_authority())?;
    Ok(dir.read_to_string(format!("{PRE_CHANGE_CACHE_DIR}/ir.json"))?)
}

/// Returns the last run's stdout and stderr as text.
pub(super) fn last_streams(ctx: &OrthoHelpContext) -> StepResult<(String, String)> {
    ctx.last_output
        .with_ref(|output| {
            (
                String::from_utf8_lossy(&output.stdout).into_owned(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
            )
        })
        .ok_or_else(|| "last_output should be set".into())
}
