//! Global configuration loading helpers for the `hello_world` CLI.
//!
//! The functions in this module handle merging CLI overrides with discovered
//! configuration layers, keeping the main CLI module focused on types and
//! parsing concerns.

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use clap::CommandFactory;
use ortho_config::{
    MergeLayer, MergeProvenance, OrthoError, SharedEnvSource, SharedScanEnvSource,
    SubcmdConfigMerge, SubcommandCliMatches, SubcommandFileContext,
    load_and_merge_subcommand_for_with_matches_with_sources_at,
};

use super::{GlobalArgs, HelloWorldCli};
use crate::error::HelloWorldError;

use super::GreetCommand;
use super::config_loading;
use super::overrides::Overrides;

/// Environment capabilities used by source-aware global configuration loading.
///
/// This context is only for application-level global and greeting composition.
/// It keeps lookup-only discovery separate from the explicitly scanning merge
/// layer.
#[derive(Clone)]
pub struct GlobalConfigSources {
    discovery: SharedEnvSource,
    merge: SharedScanEnvSource,
}

impl GlobalConfigSources {
    /// Bundle the source capabilities used for one global configuration load.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::sync::Arc;
    ///
    /// use hello_world::cli::GlobalConfigSources;
    /// use ortho_config::{MapEnv, SharedEnvSource, SharedScanEnvSource};
    ///
    /// let environment = Arc::new(MapEnv::new().with_var("HELLO_WORLD_RECIPIENT", "Ada"));
    /// let discovery: SharedEnvSource = environment.clone();
    /// let merge: SharedScanEnvSource = environment;
    /// let _sources = GlobalConfigSources::new(discovery, merge);
    /// ```
    #[must_use]
    pub fn new(discovery: SharedEnvSource, merge: SharedScanEnvSource) -> Self {
        Self { discovery, merge }
    }
}

/// Resolves the global configuration by layering defaults with CLI overrides.
///
/// # Parameters
///
/// * `globals` - Global CLI arguments containing recipient, salutations, and
///   delivery flags.
/// * `config_override` - Optional explicit path used to override configuration
///   file discovery.
/// * `program_name` - Program name forwarded as `argv[0]` to the composition
///   layer.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when discovery fails or configuration cannot
/// be deserialized.
pub fn load_global_config(
    globals: &GlobalArgs,
    config_override: Option<&Path>,
    program_name: impl AsRef<std::ffi::OsStr>,
) -> Result<HelloWorldCli, HelloWorldError> {
    load_global_config_with_composition(
        globals,
        config_override,
        program_name,
        HelloWorldCli::compose_layers_from_iter,
    )
}

/// Resolves global configuration using the supplied environment capabilities.
///
/// This preserves the default loader's CLI provenance filtering and global
/// override handling while allowing tests and embedders to avoid process state.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
///
/// use hello_world::cli::{
///     GlobalArgs, GlobalConfigSources, load_global_config_with_sources,
/// };
/// use ortho_config::{MapEnv, SharedEnvSource, SharedScanEnvSource};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let environment = Arc::new(MapEnv::new().with_var("HELLO_WORLD_RECIPIENT", "Ada"));
/// let discovery: SharedEnvSource = environment.clone();
/// let merge: SharedScanEnvSource = environment;
/// let config = load_global_config_with_sources(
///     &GlobalArgs::default(),
///     None,
///     "hello-world",
///     GlobalConfigSources::new(discovery, merge),
/// )?;
/// assert_eq!(config.recipient, "Ada");
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when discovery fails or configuration cannot
/// be deserialized.
pub fn load_global_config_with_sources(
    globals: &GlobalArgs,
    config_override: Option<&Path>,
    program_name: impl AsRef<std::ffi::OsStr>,
    sources: GlobalConfigSources,
) -> Result<HelloWorldCli, HelloWorldError> {
    load_global_config_with_composition(globals, config_override, program_name, |args| {
        HelloWorldCli::compose_layers_from_iter_with_sources(args, sources.discovery, sources.merge)
    })
}

fn load_global_config_with_composition(
    globals: &GlobalArgs,
    config_override: Option<&Path>,
    program_name: impl AsRef<std::ffi::OsStr>,
    compose_layers: impl FnOnce(Vec<std::ffi::OsString>) -> ortho_config::declarative::LayerComposition,
) -> Result<HelloWorldCli, HelloWorldError> {
    let args = build_composition_args(program_name.as_ref(), config_override);
    resolve_global_composition(globals, compose_layers(args))
}

fn resolve_global_composition(
    globals: &GlobalArgs,
    composition: ortho_config::declarative::LayerComposition,
) -> Result<HelloWorldCli, HelloWorldError> {
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

/// Loads greeting defaults using explicit discovery and merge capabilities.
///
/// The explicit `file_base` bounds subcommand file lookup; discovery uses only
/// `sources.discovery`, while the greeting environment layer uses
/// `sources.merge`. The two remain separate so lookup cannot enumerate
/// unrelated variables or fall back to the caller's working directory.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
/// use std::path::Path;
/// use hello_world::cli::{GlobalConfigSources, load_greet_defaults_with_sources};
/// use ortho_config::{MapEnv, SharedEnvSource, SharedScanEnvSource};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let environment = Arc::new(MapEnv::new());
/// let discovery: SharedEnvSource = environment.clone();
/// let merge: SharedScanEnvSource = environment;
/// let greet = load_greet_defaults_with_sources(
///     Path::new("/isolated/fixtures"),
///     GlobalConfigSources::new(discovery, merge),
/// )?;
/// assert_eq!(greet.punctuation, "!");
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when greeting or file overrides cannot be loaded.
pub fn load_greet_defaults_with_sources(
    file_base: &Path,
    sources: GlobalConfigSources,
) -> Result<GreetCommand, HelloWorldError> {
    let files = SubcommandFileContext::new(file_base, sources.discovery.as_ref());
    let defaults = GreetCommand::default();
    // Preserve cli_default_as_absent: clap's punctuation default is not a
    // user-provided override of an injected file or environment value.
    let matches = GreetCommand::command().try_get_matches_from(["greet"])?;
    let cli_matches = SubcommandCliMatches::new(&defaults, &matches);
    let mut command = load_and_merge_subcommand_for_with_matches_with_sources_at(
        &cli_matches,
        files,
        sources.merge,
    )?;
    apply_greet_overrides_with_source(&mut command, sources.discovery)?;
    Ok(command)
}

/// Applies greeting-specific overrides derived from configuration defaults.
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when greeting overrides cannot be loaded.
pub fn apply_greet_overrides(command: &mut GreetCommand) -> Result<(), HelloWorldError> {
    apply_file_greet_overrides(command, config_loading::load_config_overrides()?);
    Ok(())
}

/// Applies greeting file overrides discovered through an explicit lookup source.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
/// use hello_world::cli::{GreetCommand, apply_greet_overrides_with_source};
/// use ortho_config::MapEnv;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut command = GreetCommand::default();
/// apply_greet_overrides_with_source(&mut command, Arc::new(MapEnv::new()))?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns a [`HelloWorldError`] when file discovery or deserialisation fails.
pub fn apply_greet_overrides_with_source(
    command: &mut GreetCommand,
    discovery: SharedEnvSource,
) -> Result<(), HelloWorldError> {
    apply_file_greet_overrides(
        command,
        config_loading::load_config_overrides_with_source(discovery)?,
    );
    Ok(())
}

fn apply_file_greet_overrides(
    command: &mut GreetCommand,
    loaded: Option<(super::overrides::FileOverrides, Option<camino::Utf8PathBuf>)>,
) {
    if let Some((overrides, _)) = loaded
        && let Some(greet) = overrides.cmds.greet
    {
        if let Some(preamble) = greet.preamble {
            command.preamble = Some(preamble);
        }
        if let Some(punctuation) = greet.punctuation {
            command.punctuation = punctuation;
        }
    }
}
