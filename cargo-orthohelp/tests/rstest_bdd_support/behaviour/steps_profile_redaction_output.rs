//! Assertions on generated profile-redaction output.
//!
//! These `then` steps consume the artefacts the `when` steps wrote in
//! [`super::steps_profile_redaction`] and check both halves of the disclosure
//! boundary: what must disappear, and what deliberate public metadata must
//! survive. Value-level assertions live in
//! [`super::profile_redaction_assertions`].

use rstest_bdd_macros::then;
use serde_json::Value;

use super::profile_redaction_assertions::{
    RedactionExpectation, assert_ordinary_ir_examples, assert_public_ir_fields,
    cli_possible_values, command_name, context_enum_values, find_redaction, redaction_records,
    require_context_enum_values, require_context_input_default, require_profiles_supported,
    require_records, section_examples,
};
use super::profile_redaction_support::{
    ARTEFACT_LOCALES, UNRELATED_CACHE_SENTINEL, UNSUPPORTED_SENTINELS, all_protected_sentinels,
    assert_artefact_inventory, assert_sentinels_absent, command_nodes, context_command,
    context_input, ir_field, last_streams, paired_named_contexts, read_artefacts, read_irs,
    read_out_json, read_stdout_json, read_unrelated_cache_entry, require, serialized,
};
use super::steps::{OrthoHelpContext, StepResult};

/// Records the fixture must publish for the profile-enabled tree.
///
/// The table is the positive half of the disclosure contract: these identities
/// stay visible, each with the classification its author declared.
const EXPECTED_REDACTION: [RedactionExpectation; 6] = [
    RedactionExpectation {
        command_path: &["redaction_fixture"],
        field: "token",
        exposure: "secret",
    },
    RedactionExpectation {
        command_path: &["redaction_fixture"],
        field: "locator",
        exposure: "reference_only",
    },
    RedactionExpectation {
        command_path: &["redaction_fixture"],
        field: "api_key",
        exposure: "secret",
    },
    RedactionExpectation {
        command_path: &["redaction_fixture"],
        field: "credential_file",
        exposure: "reference_only",
    },
    RedactionExpectation {
        command_path: &["redaction_fixture"],
        field: "inferred_secret",
        exposure: "secret",
    },
    RedactionExpectation {
        command_path: &["redaction_fixture", "rotate"],
        field: "replacement",
        exposure: "secret",
    },
];

// ---------------------------------------------------------------------------
// Then: assert disclosure, retention, and names-only behaviour.
// ---------------------------------------------------------------------------

#[then("public field descriptors remain visible")]
fn public_field_descriptors_remain_visible(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let context = read_out_json(orthohelp_context, "agent-context.json")?;
    require_profiles_supported(&context)?;
    let command = context_command(&context, &["redaction_fixture"])
        .ok_or("redaction_fixture command missing from context")?;
    require_context_input_default(command, "region", "eu-west")?;
    require_context_input_default(command, "mode", "fast")?;
    require_context_enum_values(command, "mode", &["fast", "safe"])?;
    for protected in ["token", "locator"] {
        let input = context_input(command, protected)
            .ok_or_else(|| format!("{protected} input missing"))?;
        require(
            input.get("default").is_some_and(Value::is_null),
            format!("{protected} should have no context default"),
        )?;
        require(
            context_enum_values(input).is_empty(),
            format!("{protected} should have no context enum values"),
        )?;
    }
    for (locale, ir) in read_irs(orthohelp_context)? {
        assert_public_ir_fields(&ir, &locale)?;
    }
    Ok(())
}

#[then("context JSON declares both classifications")]
fn context_declares_both_classifications(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let context = read_out_json(orthohelp_context, "agent-context.json")?;
    let records = redaction_records(&context)?;
    for expected in &EXPECTED_REDACTION {
        let record = find_redaction(records, expected).ok_or_else(|| {
            format!(
                "missing redaction record for {} {}",
                expected.command_path.join(" "),
                expected.field
            )
        })?;
        require(
            record.get("exposure").and_then(Value::as_str) == Some(expected.exposure),
            format!(
                "{} should be classified {}",
                expected.field, expected.exposure
            ),
        )?;
    }
    require(
        records.len() == EXPECTED_REDACTION.len(),
        format!(
            "expected {} redaction records, got {}",
            EXPECTED_REDACTION.len(),
            records.len()
        ),
    )
}

#[then("no protected sentinel appears in any generated artefact")]
fn no_protected_sentinel_in_any_artefact(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let artefacts = read_artefacts(orthohelp_context)?;
    assert_artefact_inventory(&artefacts)?;
    assert_sentinels_absent(&artefacts, &all_protected_sentinels())
}

#[then("every command-level example collection is empty")]
fn every_command_example_collection_is_empty(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    for (locale, ir) in read_irs(orthohelp_context)? {
        let commands = command_nodes(&ir);
        require(
            commands.len() >= 3,
            format!(
                "{locale}: expected root, child, and sibling commands, found {}",
                commands.len()
            ),
        )?;
        for command in commands {
            let examples = section_examples(command)
                .ok_or_else(|| format!("{locale}: command-level examples are not an array"))?;
            require(
                examples.is_empty(),
                format!(
                    "{locale}: {} kept command-level examples",
                    command_name(command)
                ),
            )?;
        }
    }
    Ok(())
}

#[then("both serialized contexts are identical")]
fn both_serialized_contexts_are_identical() -> StepResult<()> {
    let (first, second, _) = paired_named_contexts()?;
    let left = serialized(&first)?;
    let right = serialized(&second)?;
    require(
        left == right,
        "contexts built from equal names must serialize identically",
    )
}

#[then("neither context contains profile bodies")]
fn neither_context_contains_profile_bodies() -> StepResult<()> {
    let (first, second, bodies) = paired_named_contexts()?;
    let encoded = [serialized(&first)?, serialized(&second)?];
    for body in bodies {
        for document in &encoded {
            require(
                !document.contains(body),
                format!("profile body {body} leaked into a serialized context"),
            )?;
        }
    }
    Ok(())
}

#[then("its defaults and examples retain their existing representation")]
fn ordinary_defaults_and_examples_are_retained(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let context = read_out_json(orthohelp_context, "agent-context.json")?;
    require(
        context.get("profiles") == Some(&serde_json::json!({"supported": false})),
        "an unclassified command must not invent a profile declaration",
    )?;
    let command = context_command(&context, &["ordinary_fixture"])
        .ok_or("ordinary_fixture command missing from context")?;
    require_context_input_default(command, "name", "plain-default")?;
    require_context_input_default(command, "is_verbose", "false")?;
    for (locale, ir) in read_irs(orthohelp_context)? {
        assert_ordinary_ir_examples(&ir, &locale)?;
    }
    let artefacts = read_artefacts(orthohelp_context)?;
    let pages = artefacts
        .iter()
        .filter(|artefact| artefact.is_roff())
        .collect::<Vec<_>>();
    require(
        pages.len() >= ARTEFACT_LOCALES.len(),
        format!("expected a man page per locale, found {}", pages.len()),
    )?;
    for page in pages {
        require(
            page.content.contains("ordinary-fixture --name demo"),
            format!("{} lost its command example", page.path),
        )?;
        require(
            page.content.contains("plain-default"),
            format!("{} lost its public default", page.path),
        )?;
    }
    Ok(())
}

#[then("the context declares profiles unsupported")]
fn context_declares_profiles_unsupported(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let context = read_out_json(orthohelp_context, "agent-context.json")?;
    let profiles = context
        .get("profiles")
        .ok_or("profiles missing from context")?;
    require(
        profiles.as_object().is_some_and(|map| map.len() == 1),
        "the unsupported declaration must carry only `supported`",
    )?;
    require(
        profiles.get("supported") == Some(&Value::Bool(false)),
        "profiles should be unsupported",
    )
}

#[then("the protected default is absent from the context")]
fn protected_default_absent_from_context(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let context = read_out_json(orthohelp_context, "agent-context.json")?;
    let command = context_command(&context, &["unsupported_redaction_fixture"])
        .ok_or("unsupported_redaction_fixture command missing from context")?;
    let input = context_input(command, "token").ok_or("token input missing")?;
    require(
        input.get("default").is_some_and(Value::is_null),
        "the context must not carry the protected default",
    )?;
    for (locale, ir) in read_irs(orthohelp_context)? {
        let field = ir_field(&ir, "token").ok_or("token field missing from IR")?;
        require(
            field.get("default").is_some_and(Value::is_null),
            format!("{locale}: protected default retained"),
        )?;
        require(
            field
                .get("examples")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            format!("{locale}: protected examples retained"),
        )?;
        require(
            cli_possible_values(field).is_some_and(Vec::is_empty),
            format!("{locale}: protected possible values retained"),
        )?;
    }
    Ok(())
}

#[then("no unsupported sentinel appears in any generated artefact")]
fn no_unsupported_sentinel_in_any_artefact(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let artefacts = read_artefacts(orthohelp_context)?;
    assert_artefact_inventory(&artefacts)?;
    assert_sentinels_absent(&artefacts, UNSUPPORTED_SENTINELS)
}

#[then("the redaction list names both fields")]
fn redaction_list_names_both_fields(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let context = read_stdout_json(orthohelp_context)?;
    let records = redaction_records(&context)?;
    let hidden = ["api_key", "inferred_secret", "credential_file"];
    for expected in EXPECTED_REDACTION
        .iter()
        .filter(|expected| hidden.contains(&expected.field))
    {
        require_records(records, expected)?;
    }
    Ok(())
}

#[then("neither field is an invocable input")]
fn hidden_and_file_only_fields_are_not_invocable(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    let context = read_stdout_json(orthohelp_context)?;
    let commands = context
        .get("commands")
        .and_then(Value::as_array)
        .ok_or("commands missing from context")?;
    require(!commands.is_empty(), "the context must expose commands")?;
    for command in commands {
        for field in ["api_key", "inferred_secret", "credential_file"] {
            require(
                context_input(command, field).is_none(),
                format!("{field} must not be an invocable input"),
            )?;
        }
    }
    let root = context_command(&context, &["redaction_fixture"]).ok_or("root command missing")?;
    for visible in ["token", "locator"] {
        require(
            context_input(root, visible).is_some(),
            format!("{visible} should remain invocable"),
        )?;
    }
    Ok(())
}

#[then("the cached IR carries no protected sentinel")]
fn cached_ir_is_sanitised(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let content = orthohelp_context
        .cache_ir_content
        .with_ref(Clone::clone)
        .ok_or("cached IR content should be recorded")?;
    // Non-vacuity: the projection keeps the classification it acted on.
    require(
        content.contains("\"profile_field\""),
        "the cached IR should still record each classification",
    )?;
    require(
        content.contains("\"name\": \"token\""),
        "the cached IR should still name the protected field",
    )?;
    for sentinel in all_protected_sentinels() {
        require(
            !content.contains(sentinel),
            format!("protected sentinel {sentinel} appeared in the cached IR"),
        )?;
    }
    Ok(())
}

#[then("the unrelated cache entry was not rewritten")]
fn unrelated_entry_not_rewritten(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let content = read_unrelated_cache_entry(orthohelp_context)?;
    require(
        content.contains(UNRELATED_CACHE_SENTINEL),
        "the unrelated entry should keep the literal it was written with",
    )?;
    let (stdout, stderr) = last_streams(orthohelp_context)?;
    for (label, stream) in [("stdout", stdout), ("stderr", stderr)] {
        require(
            !stream.contains(UNRELATED_CACHE_SENTINEL),
            format!("stale cached content leaked into {label}"),
        )?;
    }
    Ok(())
}
