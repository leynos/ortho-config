//! Renderer-metadata assertions against per-locale IR artefacts.
//!
//! This is the artefact half of the renderer steps: [`super::steps_renderer`]
//! reads the `renderer` block out of agent-context JSON, while these steps read
//! it out of `ir/{locale}.json`. Both faces are asserted because the two are
//! produced by different code paths, and a declaration can survive one and be
//! dropped by the other.
//!
//! Kept separate from `steps_renderer` so neither module approaches the
//! repository's 400-line ceiling.
//!
//! See ADR-0016 for why renderer metadata must not vary with locale.

use std::io::Read;

use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::{then, when};
use serde_json::Value;

use super::steps::{OrthoHelpContext, StepResult, get_out_dir, run_orthohelp};
use super::steps_renderer::{GREET_COMMAND, VERSION_COMMAND};

/// Locales compared by the protocol-stability scenario.
const COMPARED_LOCALES: [&str; 2] = ["en-US", "fr-FR"];

/// Asserts the scenario's last `cargo-orthohelp` invocation succeeded.
///
/// Per-locale files are written during one run, so a non-zero exit can leave a
/// complete-looking earlier artefact behind. Checking the exit status first
/// attributes such a failure to the run rather than to the assertions.
fn last_run_succeeded(orthohelp_context: &OrthoHelpContext) -> StepResult<()> {
    let succeeded = orthohelp_context
        .last_output
        .with_ref(|output| output.status.success())
        .ok_or("last_output should be set")?;
    if succeeded {
        Ok(())
    } else {
        Err("cargo-orthohelp should succeed".into())
    }
}

/// Generates the nested fixture's IR for both compared locales.
///
/// Two explicit `--locale` values are passed in one invocation, which is the
/// supported way to obtain per-locale artefacts from a single run.
#[when("I generate the nested fixture IR for two locales")]
fn generate_ir_for_two_locales(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let mut args = vec![
        "--format",
        "ir",
        "--package",
        "orthohelp_fixture",
        "--root-type",
        "orthohelp_fixture::NestedFixtureConfig",
    ];
    for locale in COMPARED_LOCALES {
        args.extend(["--locale", locale]);
    }
    let output = run_orthohelp(orthohelp_context, &args)?;
    orthohelp_context.last_output.set(output);
    Ok(())
}

/// Reads one per-locale IR artefact from the scenario output directory.
fn read_localized_ir(orthohelp_context: &OrthoHelpContext, locale: &str) -> StepResult<Value> {
    last_run_succeeded(orthohelp_context)?;
    let out_root = get_out_dir(orthohelp_context)?;
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())?;
    let mut file = dir.open(format!("ir/{locale}.json"))?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;
    Ok(serde_json::from_str(&buffer)?)
}

/// Finds the single IR command whose `app_name` equals `leaf`.
///
/// The walk is recursive because the fixture's declaring commands sit one level
/// below the root, and uniqueness is required so a duplicate name fails loudly
/// instead of asserting whichever match came first.
fn ir_command_by_leaf<'a>(root: &'a Value, leaf: &str) -> StepResult<&'a Value> {
    fn collect<'a>(node: &'a Value, leaf: &str, found: &mut Vec<&'a Value>) {
        if node.get("app_name").and_then(Value::as_str) == Some(leaf) {
            found.push(node);
        }
        for child in node
            .get("subcommands")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            collect(child, leaf, found);
        }
    }

    let mut found = Vec::new();
    collect(root, leaf, &mut found);
    match found.as_slice() {
        [command] => Ok(command),
        other => Err(format!(
            "expected exactly one IR command named {leaf:?}, found {}",
            other.len()
        )
        .into()),
    }
}

/// Returns the renderer block of a command in a localized IR document.
fn ir_renderer(root: &Value, leaf: &str) -> StepResult<Value> {
    let command = ir_command_by_leaf(root, leaf)?;
    command
        .get("renderer")
        .cloned()
        .ok_or_else(|| format!("IR command {leaf:?} carries no renderer block").into())
}

/// Renderer declarations are language-independent, so every locale agrees.
///
/// This is the protocol-stability contract: a localizer may change human-facing
/// text, but it must not rename a field identifier, reclassify localization, or
/// move a policy token. The comparison is over the whole renderer block rather
/// than a selected leaf, so an added or dropped claim also fails.
#[then("renderer field identifiers and policies agree across en-US and fr-FR")]
fn renderer_metadata_agrees_across_locales(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let en_us = read_localized_ir(orthohelp_context, COMPARED_LOCALES[0])?;
    let fr_fr = read_localized_ir(orthohelp_context, COMPARED_LOCALES[1])?;

    let declared = ir_renderer(&en_us, GREET_COMMAND)?;
    let translated = ir_renderer(&fr_fr, GREET_COMMAND)?;
    if declared != translated {
        return Err(format!(
            "renderer metadata should not change with locale; en-US {declared} vs fr-FR {translated}"
        )
        .into());
    }

    let undeclared = ir_command_by_leaf(&fr_fr, VERSION_COMMAND)?;
    if undeclared.get("renderer").is_some() {
        return Err("an undeclared command must not gain a renderer block".into());
    }
    Ok(())
}

/// Human language still changes cosmetic text, so the comparison is not vacuous.
///
/// Without this half the previous step would also pass if localization had been
/// dropped entirely and both locales emitted identical documents.
#[then("the en-US and fr-FR command summaries differ")]
fn summaries_differ_across_locales(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let en_us = read_localized_ir(orthohelp_context, COMPARED_LOCALES[0])?;
    let fr_fr = read_localized_ir(orthohelp_context, COMPARED_LOCALES[1])?;
    let about_of = |root: &Value| -> StepResult<String> {
        root.get("about")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "IR about text missing".into())
    };

    let english = about_of(&en_us)?;
    let french = about_of(&fr_fr)?;
    if english == french {
        return Err(format!("localized summaries should differ, both were {english:?}").into());
    }
    Ok(())
}
