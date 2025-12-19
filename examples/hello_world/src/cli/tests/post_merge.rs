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

#[rstest]
fn greet_command_normalizes_whitespace_preamble() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "punctuation": "!",
        "preamble": "   "
    }));

    let config = to_anyhow(GreetCommand::merge_from_layers(composer.layers()))?;
    ensure!(
        config.preamble.is_none(),
        "expected whitespace-only preamble to be normalized to None, got {:?}",
        config.preamble
    );
    Ok(())
}

#[rstest]
fn greet_command_normalizes_empty_preamble() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "punctuation": "!",
        "preamble": ""
    }));

    let config = to_anyhow(GreetCommand::merge_from_layers(composer.layers()))?;
    ensure!(
        config.preamble.is_none(),
        "expected empty preamble to be normalized to None, got {:?}",
        config.preamble
    );
    Ok(())
}

#[rstest]
fn greet_command_preserves_valid_preamble() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "punctuation": "!",
        "preamble": "Good morning"
    }));

    let config = to_anyhow(GreetCommand::merge_from_layers(composer.layers()))?;
    ensure!(
        config.preamble.as_deref() == Some("Good morning"),
        "expected preamble to be preserved, got {:?}",
        config.preamble
    );
    Ok(())
}

#[rstest]
fn greet_command_normalizes_tabs_only_preamble() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "punctuation": "!",
        "preamble": "\t\t"
    }));

    let config = to_anyhow(GreetCommand::merge_from_layers(composer.layers()))?;
    ensure!(
        config.preamble.is_none(),
        "expected tab-only preamble to be normalized to None, got {:?}",
        config.preamble
    );
    Ok(())
}

#[rstest]
fn greet_command_normalizes_newlines_only_preamble() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "punctuation": "!",
        "preamble": "\n\n"
    }));

    let config = to_anyhow(GreetCommand::merge_from_layers(composer.layers()))?;
    ensure!(
        config.preamble.is_none(),
        "expected newline-only preamble to be normalized to None, got {:?}",
        config.preamble
    );
    Ok(())
}

#[rstest]
fn greet_command_preserves_none_preamble() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "punctuation": "!" }));

    let config = to_anyhow(GreetCommand::merge_from_layers(composer.layers()))?;
    ensure!(
        config.preamble.is_none(),
        "expected preamble to remain None when not specified, got {:?}",
        config.preamble
    );
    Ok(())
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
