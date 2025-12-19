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
    #[expect(
        dead_code,
        reason = "Required for JSON deserialization but not read in tests"
    )]
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
    #[expect(
        dead_code,
        reason = "Required for JSON deserialization but not read in tests"
    )]
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

fn merge_hook_sample(composer: MergeComposer) -> Result<HookSample> {
    to_anyhow(HookSample::merge_from_layers(composer.layers()))
}

fn merge_context_aware_sample(composer: MergeComposer) -> Result<ContextAwareSample> {
    to_anyhow(ContextAwareSample::merge_from_layers(composer.layers()))
}

#[rstest]
#[case::whitespace("   ", None, "whitespace-only preamble should be normalized to None")]
#[case::empty("", None, "empty string preamble should be normalized to None")]
#[case::valid("Hello!", Some("Hello!"), "valid preamble should be preserved")]
fn post_merge_hook_normalizes_preamble(
    #[case] input_preamble: &str,
    #[case] expected: Option<&str>,
    #[case] error_msg: &str,
) -> Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "name": "Test", "preamble": input_preamble }));

    let config = merge_hook_sample(composer)?;
    ensure!(
        config.preamble.as_deref() == expected,
        "{error_msg}, got {:?}",
        config.preamble
    );
    Ok(())
}

/// Layer configuration for context-aware tests.
#[derive(Clone, Copy)]
enum LayerConfig {
    CliOnly,
    EnvOnly,
    TwoFiles,
    EnvAndCli,
}

fn setup_context_aware_composer(layer_config: LayerConfig) -> MergeComposer {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({
        "value": "default",
        "cli_was_present": false,
        "file_count": 0
    }));
    match layer_config {
        LayerConfig::CliOnly => {
            composer.push_cli(json!({ "value": "from_cli" }));
        }
        LayerConfig::EnvOnly => {
            composer.push_environment(json!({ "value": "from_env" }));
        }
        LayerConfig::TwoFiles => {
            composer.push_file(
                json!({ "value": "file1" }),
                Some(Utf8PathBuf::from("/etc/config.toml")),
            );
            composer.push_file(
                json!({ "value": "file2" }),
                Some(Utf8PathBuf::from("/home/user/.config.toml")),
            );
        }
        LayerConfig::EnvAndCli => {
            composer.push_environment(json!({ "value": "env" }));
            composer.push_cli(json!({ "value": "cli" }));
        }
    }
    composer
}

#[rstest]
#[case::with_cli(
    LayerConfig::CliOnly,
    true,
    "cli_was_present should be true when CLI layer is added"
)]
#[case::without_cli(
    LayerConfig::EnvOnly,
    false,
    "cli_was_present should be false when no CLI layer is added"
)]
fn post_merge_context_detects_cli_input(
    #[case] layer_config: LayerConfig,
    #[case] expected: bool,
    #[case] error_msg: &str,
) -> Result<()> {
    let composer = setup_context_aware_composer(layer_config);
    let config = merge_context_aware_sample(composer)?;
    ensure!(config.cli_was_present == expected, "{error_msg}");
    Ok(())
}

#[rstest]
#[case::two_files(
    LayerConfig::TwoFiles,
    2,
    "file_count should be 2 when two files are loaded"
)]
#[case::zero_files(
    LayerConfig::EnvAndCli,
    0,
    "file_count should be 0 when no files are loaded"
)]
fn post_merge_context_counts_file_layers(
    #[case] layer_config: LayerConfig,
    #[case] expected: usize,
    #[case] error_msg: &str,
) -> Result<()> {
    let composer = setup_context_aware_composer(layer_config);
    let config = merge_context_aware_sample(composer)?;
    ensure!(
        config.file_count == expected,
        "{error_msg}, got {}",
        config.file_count
    );
    Ok(())
}
