//! Step definitions for the skill-manifest validation scenarios.
//!
//! `rstest-bdd` publishes every compiled step through `inventory`, so a step
//! defined here is discoverable by the scenarios declared in the parent module
//! even though it lives in a separate file. Keeping the steps apart from the
//! scenario state and fixture builders keeps both files below the repository's
//! module-size cap.

use cargo_orthohelp::policy::PolicyMode;
use cargo_orthohelp::skill_validation::{
    EffectiveCommandSurface, FindingCategory, ManifestClaims, ResolvedAgentPolicy,
    SkillValidationReport, codes, validate,
};
use rstest_bdd_macros::{given, then, when};

use super::{
    SkillContext, StepResult, add_command, add_example, add_manifest, add_option_to_last_command,
    add_reference, command, example, option_use, reference,
};

#[given("a command contract with a nested command and effective options")]
fn nested_command_contract(context: &mut SkillContext) {
    add_command(context, command(&["tool"], &["json"]));
    add_command(context, command(&["tool", "records", "get"], &["json"]));
}

#[given("an operation skill fixture with a valid reference and an unknown flag")]
fn operation_skill_claims(context: &mut SkillContext) -> StepResult<()> {
    add_manifest(
        context,
        ManifestClaims {
            id: "operation".to_owned(),
            commands: Vec::new(),
        },
    );
    add_reference(context, reference(&["tool", "records", "get"], &["json"]))?;
    add_reference(context, reference(&["tool", "records", "get"], &["yaml"]))
}

#[given("a command contract with explicit effective global options")]
fn explicit_global_options(context: &mut SkillContext) {
    add_command(context, command(&["tool"], &["color", "json"]));
    add_command(context, command(&["tool", "build"], &["color", "json"]));
}

#[given("a build workflow example with an option name and value")]
fn build_workflow_example(context: &mut SkillContext) -> StepResult<()> {
    add_manifest(
        context,
        ManifestClaims {
            id: "build-workflow".to_owned(),
            commands: Vec::new(),
        },
    );
    add_reference(context, reference(&["tool", "build"], &["color"]))?;
    add_example(
        context,
        example(
            "build-workflow",
            &["tool", "build"],
            vec![option_use("color", Some("sometimes"), 1)],
        ),
    );
    Ok(())
}

#[given("a real command accepting the declared option {option:string}")]
fn real_command_accepting_option(context: &mut SkillContext, option: String) -> StepResult<()> {
    add_command(context, command(&["tool"], &[]));
    add_option_to_last_command(context, &option)?;
    add_manifest(
        context,
        ManifestClaims {
            id: "operation".to_owned(),
            commands: Vec::new(),
        },
    );
    add_reference(context, reference(&["tool"], &[option.as_str()]))
}

#[given("an example using {option:string} with value {value:string}")]
fn example_using_option_with_value(context: &mut SkillContext, option: String, value: String) {
    add_example(
        context,
        example(
            "operation",
            &["tool"],
            vec![option_use(&option, Some(&value), 1)],
        ),
    );
}

#[given("deny policy without an exception for that option")]
fn deny_policy(context: &mut SkillContext) {
    context.policy.set(ResolvedAgentPolicy {
        mode: PolicyMode::Deny,
        exceptions: Vec::new(),
    });
}

#[given("a root command with a non-global option {option:string}")]
fn root_command_with_non_global_option(
    context: &mut SkillContext,
    option: String,
) -> StepResult<()> {
    add_command(context, command(&["tool"], &[]));
    add_option_to_last_command(context, &option)
}

#[given("a child whose explicit effective options omit {option:string}")]
fn child_without_option(context: &mut SkillContext, _option: String) {
    add_command(context, command(&["tool", "build"], &["json"]));
}

#[given("a child example declaring and using {option:string}")]
fn child_example_using_option(context: &mut SkillContext, option: String) -> StepResult<()> {
    add_manifest(
        context,
        ManifestClaims {
            id: "build-workflow".to_owned(),
            commands: Vec::new(),
        },
    );
    add_reference(context, reference(&["tool", "build"], &[option.as_str()]))?;
    add_example(
        context,
        example(
            "build-workflow",
            &["tool", "build"],
            vec![option_use(&option, None, 1)],
        ),
    );
    Ok(())
}

#[given("a manifest with no command references or examples")]
fn prose_only_manifest(context: &mut SkillContext) {
    add_manifest(
        context,
        ManifestClaims {
            id: "prose-only".to_owned(),
            commands: Vec::new(),
        },
    );
}

#[when("the consumer adapter submits the extracted claims")]
fn submits_extracted_claims(context: &mut SkillContext) -> StepResult<()> {
    submit(context)
}

#[when("the adapter submits the normalized example")]
fn submits_normalized_example(context: &mut SkillContext) -> StepResult<()> {
    submit(context)
}

#[when("the consumer submits the normalized example")]
fn consumer_submits_normalized_example(context: &mut SkillContext) -> StepResult<()> {
    submit(context)
}

#[when("the consumer submits it for validation")]
fn consumer_submits_for_validation(context: &mut SkillContext) -> StepResult<()> {
    submit(context)
}

/// Runs the one validation call for a scenario.
fn submit(context: &mut SkillContext) -> StepResult<()> {
    let input = context.input.get().ok_or("no claim input was arranged")?;
    // A prose-only skill claims no commands, so an empty surface is a valid
    // input rather than a missing arrangement.
    let surface = context
        .surface
        .get_or_insert_with(EffectiveCommandSurface::default);
    let policy = context.policy.get().unwrap_or_default();
    let report = validate(&input, &surface, &policy)?;
    context.report.set(report);
    Ok(())
}

/// Applies `inspect` to the report produced by the `When` step.
fn with_report<R>(
    context: &SkillContext,
    inspect: impl FnOnce(&SkillValidationReport) -> R,
) -> StepResult<R> {
    context
        .report
        .with_ref(inspect)
        .ok_or_else(|| "the claims have not been validated".into())
}

#[then("the report resolves the valid reference")]
fn resolves_the_valid_reference(context: &mut SkillContext) -> StepResult<()> {
    let resolved = with_report(context, |report| {
        report.coverage.resolved_command_references
    })?;
    if resolved == 1 {
        Ok(())
    } else {
        Err(format!("expected one resolved reference, got {resolved}").into())
    }
}

#[then("reports the unknown flag for the referenced command")]
fn reports_unknown_flag_for_command(context: &mut SkillContext) -> StepResult<()> {
    let matches = with_report(context, |report| {
        report.findings.iter().any(|finding| {
            finding.code == codes::UNKNOWN_FLAG
                && finding.option.as_deref() == Some("yaml")
                && finding.command_path == vec!["tool", "records", "get"]
        })
    })?;
    if matches {
        Ok(())
    } else {
        Err("no unknown-flag finding for `tool records get --yaml`".into())
    }
}

#[then("identifies the manifest and source ordinal")]
fn identifies_manifest_and_ordinal(context: &mut SkillContext) -> StepResult<()> {
    let matches = with_report(context, |report| {
        report.findings.iter().any(|finding| {
            finding.category == FindingCategory::Structural
                && finding.manifest_id == "operation"
                && finding.source_ordinal == 2
        })
    })?;
    if matches {
        Ok(())
    } else {
        Err("no structural finding identifies operation ordinal 2".into())
    }
}

#[then("the option is checked on that command")]
fn option_is_checked_on_command(context: &mut SkillContext) -> StepResult<()> {
    // `examples_checked` only echoes the consumer's extraction flag, so it
    // stays true whatever the validator does. Assert the finding that example
    // checking alone produces: the glossary rejects `--color sometimes`.
    let matches = with_report(context, |report| {
        report.findings.iter().any(|finding| {
            finding.code == codes::POLICY_GLOBAL_OPTION_VALUE
                && finding.option.as_deref() == Some("color")
                && finding.command_path == vec!["tool", "build"]
        })
    })?;
    if matches {
        Ok(())
    } else {
        Err("no glossary-value finding for `--color sometimes`".into())
    }
}

#[then("value presence and contents are preserved")]
fn value_is_preserved(context: &mut SkillContext) -> StepResult<()> {
    let value = context
        .input
        .get()
        .and_then(|input| input.examples.first().cloned())
        .and_then(|example| example.options.first().cloned())
        .and_then(|option| option.value);
    if value.as_deref() == Some("sometimes") {
        Ok(())
    } else {
        Err(format!("the normalized value was not preserved: {value:?}").into())
    }
}

#[then("structural validation succeeds")]
fn structural_validation_succeeds(context: &mut SkillContext) -> StepResult<()> {
    let failed = with_report(context, SkillValidationReport::has_structural_findings)?;
    if failed {
        Err("unexpected structural finding".into())
    } else {
        Ok(())
    }
}

#[then("the policy finding recommends {option:string}")]
fn policy_finding_recommends(context: &mut SkillContext, option: String) -> StepResult<()> {
    let found = with_report(context, |report| {
        report.findings.iter().any(|finding| {
            finding.category == FindingCategory::Policy && finding.message.contains(&option)
        })
    })?;
    if found {
        Ok(())
    } else {
        Err(format!("no policy finding recommends `{option}`").into())
    }
}

#[then("an unknown-option structural finding is returned")]
fn unknown_option_structural_finding(context: &mut SkillContext) -> StepResult<()> {
    let found = with_report(context, |report| {
        report.findings.iter().any(|finding| {
            finding.category == FindingCategory::Structural
                && finding.code == codes::EXAMPLE_UNKNOWN_OPTION
        })
    })?;
    if found {
        Ok(())
    } else {
        Err("no unknown-option structural finding".into())
    }
}

#[then("no coverage error is reported")]
fn no_coverage_error(context: &mut SkillContext) -> StepResult<()> {
    let failed = with_report(context, SkillValidationReport::has_structural_findings)?;
    if failed {
        Err("a prose-only manifest produced a coverage finding".into())
    } else {
        Ok(())
    }
}

#[then("checked command and example counts are zero")]
fn counts_are_zero(context: &mut SkillContext) -> StepResult<()> {
    let coverage = with_report(context, |report| report.coverage)?;
    let is_zero = coverage.command_references == 0
        && coverage.resolved_command_references == 0
        && coverage.examples_checked == 0
        && coverage.examples_unchecked == 0;
    if is_zero {
        Ok(())
    } else {
        Err(format!("expected zero coverage, got {coverage:?}").into())
    }
}
