//! Guards that the documentation IR and the runtime localizer agree.
//!
//! The IR is consumed by external tooling that writes Fluent catalogues, while
//! `clap_command` builds its lookup key at runtime from
//! `message_id_for(command_path, "args.{arg_id}.long_help")`. If the two
//! derivations disagree, the emitted catalogue simply never matches the lookup:
//! the miss is silent, with no error and no changed output.

use anyhow::{Result, anyhow, ensure};
use ortho_config::{OrthoConfigLocalization, message_id_for};
use rstest::rstest;

use crate::DocsConfig;
use crate::docs_metadata;
use crate::field_by_name;

/// Verifies the IR key for a field equals the runtime lookup key.
///
/// `log_level` is the discriminating case: it is `snake_case` in Rust, but
/// `clap_derive` keeps the raw field name as the argument id, so a kebab-cased
/// or otherwise re-spelled id would silently miss. The suffix is equally
/// load-bearing, because `normalize_segment` preserves `-` and `_`, so a
/// hyphenated suffix is a distinct and unreachable key.
#[rstest]
fn test_field_long_help_id_matches_runtime_lookup(
    docs_metadata: ortho_config::docs::DocMetadata,
) -> Result<()> {
    let log_level = field_by_name(&docs_metadata, "log_level")?;
    let command_path = DocsConfig::LOCALIZATION_BASE.split('.').collect::<Vec<_>>();
    let expected = message_id_for(&command_path, "args.log_level.long_help");

    ensure!(
        log_level.long_help_id.as_deref() == Some(expected.as_str()),
        "IR long_help_id {:?} must equal the runtime lookup key {expected:?}",
        log_level.long_help_id,
    );

    let Some(arg) = DocsConfig::ARG_IDS
        .iter()
        .find(|arg| arg.name == "log_level")
    else {
        return Err(anyhow!("ARG_IDS should contain the log_level argument"));
    };
    ensure!(
        arg.long_help_id == expected,
        "ARG_IDS long_help_id {:?} must equal the runtime lookup key {expected:?}",
        arg.long_help_id,
    );
    Ok(())
}
