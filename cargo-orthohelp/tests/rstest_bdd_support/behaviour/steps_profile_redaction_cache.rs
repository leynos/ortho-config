//! Assertions on runs that read a hand-edited cache entry.
//!
//! A released generator never writes the cache bodies these scenarios present:
//! one retains a protected default, the other repeats a protected record. The
//! steps here check that the binary projects whatever it loads, and that a
//! duplicate identity stops the run instead of publishing an ambiguous list.
//!
//! They live apart from [`super::steps_profile_redaction_output`] because they
//! assert on the load boundary rather than on generated content.

use rstest_bdd_macros::then;

use serde_json::Value;

use super::profile_redaction_artefacts::Artefact;
use super::profile_redaction_support::{
    HAND_EDITED_SENTINEL, assert_sentinels_absent, read_artefacts, require,
    surviving_ordinary_default,
};
use super::steps::{OrthoHelpContext, StepResult};
use super::steps_cache::read_cache_file;

#[then("no rendered artefact carries the hand-edited sentinel")]
fn hand_edited_default_is_projected(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let cache_path = orthohelp_context
        .cache_ir_path
        .with_ref(Clone::clone)
        .ok_or("cached IR path should be recorded")?;
    let cached = read_cache_file(&cache_path)?;
    require(
        cached.contains(HAND_EDITED_SENTINEL),
        "the hand-edited entry must still carry the literal, or the scan proves nothing",
    )?;
    let edited: Value = serde_json::from_str(&cached)?;
    let surviving = surviving_ordinary_default(&edited)
        .ok_or("an ordinary default must survive the edit")?
        .to_owned();
    let succeeded = orthohelp_context
        .last_output
        .with_ref(|output| output.status.success())
        .ok_or("last_output should be set")?;
    require(
        succeeded,
        "the no-build run must succeed before its output is scanned",
    )?;
    // Only the man run's own pages can show that the renderer publishes
    // defaults; an earlier run's artefacts stay in the same directory, so a
    // whole-directory scan could pass on content this run never wrote.
    let roff: Vec<_> = read_artefacts(orthohelp_context)?
        .into_iter()
        .filter(Artefact::is_roff)
        .collect();
    require(!roff.is_empty(), "the man run must write at least one page")?;
    require(
        roff.iter().any(|page| page.content.contains(&surviving)),
        format!(
            "the man pages must publish the surviving ordinary default {surviving}, or the scan proves nothing"
        ),
    )?;
    assert_sentinels_absent(&roff, &[HAND_EDITED_SENTINEL])
}

#[then("the command fails naming the duplicate redaction")]
fn duplicate_redaction_fails_the_run(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let (succeeded, stderr) = orthohelp_context
        .last_output
        .with_ref(|output| {
            (
                output.status.success(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
            )
        })
        .ok_or("last_output should be set")?;
    require(
        !succeeded,
        "two records sharing one identity must fail the run",
    )?;
    require(
        stderr.contains("duplicate profile redaction"),
        format!("the diagnostic must name the duplicate: {stderr}"),
    )
}
