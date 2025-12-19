//! Behavioural tests for the post-merge hook in `GreetCommand`.
//!
//! Validates that the `PostMergeHook` implementation normalizes whitespace-only
//! preambles to `None` after configuration layers are merged.

use anyhow::{Result, anyhow, ensure};
use ortho_config::serde_json::json;
use ortho_config::{MergeComposer, PostMergeContext, PostMergeHook};
use rstest::rstest;

use crate::cli::GreetCommand;

fn to_anyhow<T, E: std::fmt::Display>(result: Result<T, E>) -> anyhow::Result<T> {
    result.map_err(|err| anyhow!(err.to_string()))
}

fn assert_preamble_normalization(
    preamble_input: Option<&str>,
    expected: Option<&str>,
    error_msg: &str,
) -> Result<()> {
    let mut composer = MergeComposer::new();
    let defaults = preamble_input.map_or_else(
        || json!({ "punctuation": "!" }),
        |preamble| json!({ "punctuation": "!", "preamble": preamble }),
    );
    composer.push_defaults(defaults);

    let config = to_anyhow(GreetCommand::merge_from_layers(composer.layers()))?;
    ensure!(
        config.preamble.as_deref() == expected,
        "{error_msg}, got {:?}",
        config.preamble
    );
    Ok(())
}

#[rstest]
fn greet_command_normalizes_whitespace_preamble() -> Result<()> {
    assert_preamble_normalization(
        Some("   "),
        None,
        "expected whitespace-only preamble to be normalized to None",
    )
}

#[rstest]
fn greet_command_normalizes_empty_preamble() -> Result<()> {
    assert_preamble_normalization(
        Some(""),
        None,
        "expected empty preamble to be normalized to None",
    )
}

#[rstest]
fn greet_command_preserves_valid_preamble() -> Result<()> {
    assert_preamble_normalization(
        Some("Good morning"),
        Some("Good morning"),
        "expected preamble to be preserved",
    )
}

#[rstest]
fn greet_command_normalizes_tabs_only_preamble() -> Result<()> {
    assert_preamble_normalization(
        Some("\t\t"),
        None,
        "expected tab-only preamble to be normalized to None",
    )
}

#[rstest]
fn greet_command_normalizes_newlines_only_preamble() -> Result<()> {
    assert_preamble_normalization(
        Some("\n\n"),
        None,
        "expected newline-only preamble to be normalized to None",
    )
}

#[rstest]
fn greet_command_preserves_none_preamble() -> Result<()> {
    assert_preamble_normalization(
        None,
        None,
        "expected preamble to remain None when not specified",
    )
}

#[rstest]
fn greet_command_post_merge_hook_can_be_called_directly() -> Result<()> {
    let mut command = GreetCommand {
        preamble: Some("   ".to_owned()),
        punctuation: "!".to_owned(),
    };

    let ctx = PostMergeContext::new("TEST_");
    to_anyhow(command.post_merge(&ctx))?;

    ensure!(
        command.preamble.is_none(),
        "expected direct post_merge call to normalize preamble"
    );
    Ok(())
}
