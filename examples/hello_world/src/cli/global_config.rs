//! Global configuration loading helpers for the `hello_world` CLI.
//!
//! The functions in this module handle merging CLI overrides with discovered
//! configuration layers, keeping the main CLI module focused on types and
//! parsing concerns.

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use ortho_config::{MergeLayer, MergeProvenance, OrthoError, SubcmdConfigMerge};

use super::{GlobalArgs, HelloWorldCli};
use crate::error::HelloWorldError;

use super::GreetCommand;
use super::config_loading;
use super::overrides::Overrides;

/// Resolves the global configuration by layering defaults with CLI overrides.
///
/// # Parameters
///
/// * `globals` - Global CLI arguments containing recipient, salutations, and
///   delivery flags.
/// * `config_override` - Optional explicit path used to override configuration
///   file discovery.
/// * `program_name` - Programme name forwarded as `argv[0]` to the composition
///   layer.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when discovery fails or configuration cannot
/// be deserialised.
pub fn load_global_config(
    globals: &GlobalArgs,
    config_override: Option<&Path>,
    program_name: impl AsRef<std::ffi::OsStr>,
) -> Result<HelloWorldCli, HelloWorldError> {
    let args = build_composition_args(program_name.as_ref(), config_override);
    let composition = HelloWorldCli::compose_layers_from_iter(args);
    let (mut layers, mut errors) = composition.into_parts();

    layers.retain(|layer| layer.provenance() != MergeProvenance::Cli);
    push_cli_overrides(globals, &mut layers, &mut errors);

    let resolved = ortho_config::declarative::LayerComposition::new(layers, errors)
        .into_merge_result(HelloWorldCli::merge_from_layers)
        .map_err(HelloWorldError::Configuration)?;
    resolved.validate()?;
    Ok(resolved)
}

fn build_composition_args(
    program_name: &std::ffi::OsStr,
    config_override: Option<&Path>,
) -> Vec<std::ffi::OsString> {
    let mut args = Vec::new();
    args.push(program_name.to_owned());

    if let Some(path) = config_override {
        args.push(std::ffi::OsString::from("--config"));
        args.push(path.as_os_str().to_owned());
    }

    args
}

fn push_cli_overrides(
    globals: &GlobalArgs,
    layers: &mut Vec<MergeLayer<'static>>,
    errors: &mut Vec<Arc<OrthoError>>,
) {
    let salutations = globals.trimmed_salutations();
    if !salutations.is_empty() {
        layers.push(MergeLayer::cli(Cow::Owned(
            ortho_config::serde_json::json!({ "salutations": null }),
        )));
    }

    let overrides = Overrides {
        recipient: globals.recipient.as_ref(),
        salutations: (!salutations.is_empty()).then_some(salutations),
        is_excited: globals.is_excited,
        is_quiet: globals.is_quiet,
    };

    match ortho_config::sanitize_value(&overrides) {
        Ok(value) => layers.push(MergeLayer::cli(Cow::Owned(value))),
        Err(err) => errors.push(err),
    }
}

/// Loads the greet defaults and applies configuration overrides.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when loading greeting overrides fails.
pub fn load_greet_defaults() -> Result<GreetCommand, HelloWorldError> {
    let mut command = GreetCommand::default().load_and_merge()?;
    apply_greet_overrides(&mut command)?;
    Ok(command)
}

/// Applies greeting-specific overrides derived from configuration defaults.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when greeting defaults cannot be loaded.
pub fn apply_greet_overrides(command: &mut GreetCommand) -> Result<(), HelloWorldError> {
    if let Some((overrides, _)) = config_loading::load_config_overrides()?
        && let Some(greet) = overrides.cmds.greet
    {
        if let Some(preamble) = greet.preamble {
            command.preamble = Some(preamble);
        }
        if let Some(punctuation) = greet.punctuation {
            command.punctuation = punctuation;
        }
    }
    Ok(())
}
