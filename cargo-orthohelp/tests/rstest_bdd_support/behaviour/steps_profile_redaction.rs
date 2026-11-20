//! Input steps for the profile-redaction scenarios.
//!
//! The `given` steps assert properties of the fixture catalogue the scenario
//! relies on; the `when` steps run the generator against the root type the
//! scenario names, including the cache paths that must not resurrect stale
//! unredacted metadata. Output assertions live in
//! [`super::steps_profile_redaction_output`].

use rstest_bdd_macros::{given, when};

use super::profile_redaction_support::{
    COMMAND_EXAMPLE_SENTINELS, ORDINARY_ROOT_TYPE, REDACTION_ROOT_TYPE, REFERENCE_SENTINELS,
    SECRET_SENTINELS, UNSUPPORTED_ROOT_TYPE, UNSUPPORTED_SENTINELS, all_protected_sentinels,
    paired_named_contexts, require, run_agent_context, run_all_formats, run_checked,
    seed_unrelated_cache_entry, sentinel_categories,
};
use super::steps::{OrthoHelpContext, StepResult, run_orthohelp};
use super::steps_cache::record_cache_state;

/// Arguments for a cache-enabled run of the profile-enabled fixture.
const REDACTION_CACHE_ARGS: &[&str] = &[
    "--cache",
    "--package",
    "orthohelp_fixture",
    "--root-type",
    REDACTION_ROOT_TYPE,
    "--locale",
    "en-US",
    "--locale",
    "fr-FR",
];

// ---------------------------------------------------------------------------
// Given: the fixture catalogue declares what the scenarios claim it declares.
// ---------------------------------------------------------------------------

#[given("a profile-enabled command with secret and reference-only fields")]
fn fixture_declares_both_classifications() -> StepResult<()> {
    for (label, sentinels) in [
        ("secret", SECRET_SENTINELS),
        ("reference-only", REFERENCE_SENTINELS),
    ] {
        let categories = sentinel_categories(sentinels);
        for category in ["DEFAULT", "ENUM", "EXAMPLE"] {
            require(
                categories.contains(category),
                format!("{label} fields need a {category} sentinel"),
            )?;
        }
    }
    require(
        REDACTION_ROOT_TYPE.contains("RedactionFixtureConfig"),
        "the scenario must name the profile-enabled root type",
    )
}

#[given("their defaults, enum members and examples contain distinct sentinels")]
fn sentinels_are_distinct() -> StepResult<()> {
    let sentinels = all_protected_sentinels();
    let unique = sentinels
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    require(
        unique.len() == sentinels.len(),
        "a repeated sentinel would make an absence check ambiguous",
    )
}

#[given("a nested command tree with a secret child field")]
fn fixture_has_a_nested_secret_child() -> StepResult<()> {
    require(
        SECRET_SENTINELS
            .iter()
            .any(|sentinel| sentinel.starts_with("NESTED-SECRET")),
        "the fixture must classify a child command's field",
    )
}

#[given("parent and sibling command examples contain its sentinel")]
fn command_examples_repeat_the_child_sentinel() -> StepResult<()> {
    let protected = all_protected_sentinels();
    for example in COMMAND_EXAMPLE_SENTINELS {
        require(
            protected.contains(example),
            format!("{example} must be a literal the scan treats as protected"),
        )?;
    }
    require(
        !COMMAND_EXAMPLE_SENTINELS.is_empty(),
        "the fixture must supply a command-level example",
    )
}

#[given("a command with no marked fields")]
fn fixture_has_an_unclassified_command() -> StepResult<()> {
    require(
        ORDINARY_ROOT_TYPE.contains("OrdinaryFixtureConfig"),
        "the scenario must name the unclassified root type",
    )?;
    require(
        !all_protected_sentinels().contains(&"plain-default"),
        "an ordinary public default must never be scanned as protected",
    )
}

#[given("two catalogues with the same validated public names")]
fn catalogues_share_public_names() -> StepResult<()> {
    let (first, second, _) = paired_named_contexts()?;
    require(
        first.profiles.available_names() == second.profiles.available_names(),
        "the catalogues must expose the same names",
    )?;
    require(
        first.profiles.available_names().len() >= 2,
        "at least two names make ordering observable",
    )
}

#[given("every profile body differs between the catalogues")]
fn catalogue_bodies_all_differ() -> StepResult<()> {
    let (_, _, bodies) = paired_named_contexts()?;
    let unique = bodies
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    require(
        unique.len() == bodies.len(),
        "every body must differ, including across catalogues",
    )
}

#[given("a command that marks a secret field without profile support")]
fn fixture_marks_a_secret_without_profiles() -> StepResult<()> {
    require(
        UNSUPPORTED_ROOT_TYPE.contains("UnsupportedRedactionFixtureConfig"),
        "the scenario must name the unsupported root type",
    )?;
    require(
        UNSUPPORTED_SENTINELS.len() == 1,
        "one literal keeps the absence check unambiguous",
    )
}

#[given("a profile-enabled command with hidden and file-only protected fields")]
fn fixture_declares_hidden_and_file_only_fields() -> StepResult<()> {
    require(
        SECRET_SENTINELS.contains(&"HIDDEN-DEFAULT-SENTINEL"),
        "the fixture must hide a protected CLI field",
    )?;
    require(
        REFERENCE_SENTINELS.contains(&"FILE-ONLY-DEFAULT-SENTINEL"),
        "the fixture must declare a config-only protected field",
    )
}

#[given("a cache entry under an unrelated key retains a protected sentinel")]
fn seed_unrelated_entry(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    seed_unrelated_cache_entry(orthohelp_context)
}

// ---------------------------------------------------------------------------
// When: run the generator against the root type the scenario names.
// ---------------------------------------------------------------------------

#[when("context JSON and human documentation are generated")]
fn generate_context_and_human_documentation(
    orthohelp_context: &mut OrthoHelpContext,
) -> StepResult<()> {
    run_all_formats(orthohelp_context, REDACTION_ROOT_TYPE)
}

#[when("documentation is generated")]
fn generate_documentation(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_all_formats(orthohelp_context, REDACTION_ROOT_TYPE)
}

#[when("context JSON is generated")]
fn generate_context_only(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_agent_context(orthohelp_context, REDACTION_ROOT_TYPE)
}

#[when("context and documentation are generated")]
fn generate_ordinary_outputs(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_all_formats(orthohelp_context, ORDINARY_ROOT_TYPE)
}

#[when("context JSON and documentation are generated for the unsupported fixture")]
fn generate_unsupported_outputs(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_all_formats(orthohelp_context, UNSUPPORTED_ROOT_TYPE)
}

#[when("each consumer attaches only the names to its context")]
fn consumers_attach_only_names() -> StepResult<()> {
    let (first, second, _) = paired_named_contexts()?;
    let expected = ["dev", "prod"];
    for context in [&first, &second] {
        let names = context
            .profiles
            .available_names()
            .iter()
            .map(ortho_config::ProfileName::as_str)
            .collect::<Vec<_>>();
        require(
            names == expected,
            format!("attached names should be {expected:?}, got {names:?}"),
        )?;
    }
    Ok(())
}

#[when("I run cargo-orthohelp with cache for the profile-redaction fixture")]
fn run_with_cache(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_checked(orthohelp_context, REDACTION_CACHE_ARGS)?;
    let (cache_path, content) = record_cache_state(orthohelp_context)?;
    orthohelp_context.cache_ir_path.set(cache_path);
    orthohelp_context.cache_ir_content.set(content);
    Ok(())
}

#[when("I rerun cargo-orthohelp with cache for the profile-redaction fixture")]
fn rerun_with_cache(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_checked(orthohelp_context, REDACTION_CACHE_ARGS)
}

#[when("I run cargo-orthohelp with no-build for the profile-redaction fixture")]
fn run_with_no_build(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let output = run_orthohelp(
        orthohelp_context,
        &[
            "--no-build",
            "--package",
            "orthohelp_fixture",
            "--root-type",
            REDACTION_ROOT_TYPE,
            "--locale",
            "en-US",
        ],
    )?;
    orthohelp_context.last_output.set(output);
    Ok(())
}
