//! Integration tests for post-merge hook functionality.
//!
//! Validates that structs with `#[ortho_config(post_merge_hook)]` invoke the
//! `PostMergeHook::post_merge` method after `merge_from_layers` completes, and
//! that the `PostMergeContext` provides accurate metadata about the merge.

use anyhow::{Result, anyhow, ensure};
use camino::Utf8PathBuf;
use ortho_config::{MergeComposer, OrthoConfig, OrthoResult, PostMergeContext, PostMergeHook};
use rstest::rstest;
use serde::Deserialize;
use serde_json::json;

/// Sample config with a post-merge hook that normalizes empty strings to None.
#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "HOOK_TEST_", post_merge_hook)]
struct HookSample {
    name: String,
    preamble: Option<String>,
}

impl PostMergeHook for HookSample {
    fn post_merge(&mut self, _ctx: &PostMergeContext) -> OrthoResult<()> {
        // Normalize whitespace-only preambles to None
        if self
            .preamble
            .as_ref()
            .is_some_and(|text| text.trim().is_empty())
        {
            self.preamble = None;
        }
        Ok(())
    }
}

/// Sample config with a post-merge hook that uses context information.
#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "CTX_TEST_", post_merge_hook)]
struct ContextAwareSample {
    value: String,
    cli_was_present: bool,
    file_count: usize,
}

impl PostMergeHook for ContextAwareSample {
    fn post_merge(&mut self, ctx: &PostMergeContext) -> OrthoResult<()> {
        self.cli_was_present = ctx.has_cli_input();
        self.file_count = ctx.loaded_files().len();
        Ok(())
    }
}

fn to_anyhow<T, E: std::fmt::Display>(result: Result<T, E>) -> anyhow::Result<T> {
    result.map_err(|err| anyhow!(err.to_string()))
}

#[rstest]
fn post_merge_hook_normalizes_whitespace_preamble() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "name": "Test", "preamble": "   " }));

    let config = to_anyhow(HookSample::merge_from_layers(composer.layers()))?;
    ensure!(config.name == "Test", "unexpected name: {}", config.name);
    ensure!(
        config.preamble.is_none(),
        "expected preamble to be None after normalization, got {:?}",
        config.preamble
    );
    Ok(())
}

#[rstest]
fn post_merge_hook_preserves_valid_preamble() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "name": "Test", "preamble": "Hello!" }));

    let config = to_anyhow(HookSample::merge_from_layers(composer.layers()))?;
    ensure!(
        config.preamble.as_deref() == Some("Hello!"),
        "expected preamble to be preserved, got {:?}",
        config.preamble
    );
    Ok(())
}

#[rstest]
fn post_merge_hook_normalizes_empty_string_preamble() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "name": "Test", "preamble": "" }));

    let config = to_anyhow(HookSample::merge_from_layers(composer.layers()))?;
    ensure!(
        config.preamble.is_none(),
        "expected empty preamble to be normalized to None, got {:?}",
        config.preamble
    );
    Ok(())
}

#[rstest]
fn post_merge_context_detects_cli_input() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "value": "default",
        "cli_was_present": false,
        "file_count": 0
    }));
    composer.push_cli(json!({ "value": "from_cli" }));

    let config = to_anyhow(ContextAwareSample::merge_from_layers(composer.layers()))?;
    ensure!(
        config.value == "from_cli",
        "unexpected value: {}",
        config.value
    );
    ensure!(
        config.cli_was_present,
        "expected cli_was_present to be true when CLI layer was added"
    );
    Ok(())
}

#[rstest]
fn post_merge_context_detects_no_cli_input() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "value": "default",
        "cli_was_present": false,
        "file_count": 0
    }));
    composer.push_environment(json!({ "value": "from_env" }));

    let config = to_anyhow(ContextAwareSample::merge_from_layers(composer.layers()))?;
    ensure!(
        !config.cli_was_present,
        "expected cli_was_present to be false when no CLI layer was added"
    );
    Ok(())
}

#[rstest]
fn post_merge_context_counts_file_layers() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "value": "default",
        "cli_was_present": false,
        "file_count": 0
    }));
    composer.push_file(
        json!({ "value": "file1" }),
        Some(Utf8PathBuf::from("/etc/config.toml")),
    );
    composer.push_file(
        json!({ "value": "file2" }),
        Some(Utf8PathBuf::from("/home/user/.config.toml")),
    );

    let config = to_anyhow(ContextAwareSample::merge_from_layers(composer.layers()))?;
    ensure!(
        config.file_count == 2,
        "expected file_count to be 2, got {}",
        config.file_count
    );
    Ok(())
}

#[rstest]
fn post_merge_context_counts_zero_file_layers() -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "value": "default",
        "cli_was_present": false,
        "file_count": 0
    }));
    composer.push_environment(json!({ "value": "env" }));
    composer.push_cli(json!({ "value": "cli" }));

    let config = to_anyhow(ContextAwareSample::merge_from_layers(composer.layers()))?;
    ensure!(
        config.file_count == 0,
        "expected file_count to be 0 when no files were loaded, got {}",
        config.file_count
    );
    Ok(())
}
