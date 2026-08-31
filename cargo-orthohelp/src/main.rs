//! CLI entrypoint for `cargo-orthohelp`.
//!
//! The binary accepts Cargo's external-subcommand dispatch shape through
//! [`cli::Cli`], then delegates to the metadata, locale, cache, bridge, and
//! output modules to build localized documentation artefacts.

pub mod agent_context;
mod bridge;
mod cache;
mod cli;
mod error;
mod fs_helpers;
mod generation;
mod hex;
mod ir;
mod locale;
mod metadata;
mod output;
pub mod policy;
pub mod powershell;
pub mod roff;
mod rustflags;
pub mod schema;
#[cfg(test)]
mod test_support;
use crate::bridge::BridgeConfig;
use crate::cache::CacheKey;
use crate::cli::{Args, CargoSubcommand, Cli, OutputFormat};
use crate::error::OrthohelpError;
use crate::generation::{
    GenerationContext, build_agent_context_localizer_if_requested, build_powershell_config,
    generate_agent_context_if_requested, generate_ir, generate_man, generate_powershell,
    localize_docs_if_requested, resolve_out_dir,
};
use crate::metadata::PackageSelection;
use crate::schema::{DocMetadata, ORTHO_DOCS_IR_VERSION};
use clap::parser::ValueSource;
use clap::{CommandFactory, Error as ClapError, FromArgMatches, error::ErrorKind};
use std::io::Write;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), OrthohelpError> {
    init_tracing();
    let (cli, format_was_explicit) = match parse_cli() {
        Ok(parsed) => parsed,
        Err(error) => exit_for_clap_error(&error),
    };
    run(cli, format_was_explicit)
}

fn init_tracing() {
    let _result = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}

fn parse_cli() -> Result<(Cli, bool), ClapError> {
    let matches = Cli::command().try_get_matches()?;
    let cli = Cli::from_arg_matches(&matches)?;
    let format_was_explicit = matches
        .subcommand_matches("orthohelp")
        .is_some_and(|sub| sub.value_source("format") == Some(ValueSource::CommandLine));
    Ok((cli, format_was_explicit))
}

fn exit_for_clap_error(error: &ClapError) -> ! {
    let kind = error.kind();
    let exit_code = error.exit_code();
    if matches!(
        kind,
        ErrorKind::UnknownArgument | ErrorKind::MissingSubcommand
    ) {
        drop(write_augmented_clap_error(error));
        std::process::exit(exit_code);
    }
    error.exit();
}

fn write_augmented_clap_error(error: &ClapError) -> std::io::Result<()> {
    let mut stderr = std::io::stderr().lock();
    write!(stderr, "{error}")?;
    writeln!(
        stderr,
        "note: invoke this tool via `cargo orthohelp` or as `cargo-orthohelp orthohelp [OPTIONS]`"
    )
}

/// Runs the agent-native policy check when requested and reports whether the
/// generator pipeline should be skipped (Decision D11's `--format` rule).
fn run_policy_check_if_requested(
    args: &Args,
    metadata: &cargo_metadata::Metadata,
    format_was_explicit: bool,
) -> Result<bool, OrthohelpError> {
    if !args.check_agent_native {
        return Ok(false);
    }
    let out_dir = args
        .out_dir
        .clone()
        .unwrap_or_else(|| metadata.target_directory.join("orthohelp").join("out"));
    let package = metadata::select_policy_package(metadata, args)?;
    policy::check::run_policy_check(package, args.policy_mode, &out_dir)?;
    Ok(!format_was_explicit)
}

fn run(cli: Cli, format_was_explicit: bool) -> Result<(), OrthohelpError> {
    let Cli {
        command: CargoSubcommand::Orthohelp(args),
    } = cli;
    tracing::debug!("cargo-orthohelp dispatched via Cargo external-subcommand");

    let metadata = metadata::load_metadata()?;
    if run_policy_check_if_requested(&args, &metadata, format_was_explicit)? {
        return Ok(());
    }
    let selection = metadata::select_package(&metadata, &args)?;

    let out_dir = resolve_out_dir(args.out_dir.clone(), &selection);

    let fingerprint = cache::fingerprint_package(&selection.package_root)?;
    let lockfile_hash = cache::lockfile_fingerprint(&metadata.workspace_root)?;
    let cache_key = CacheKey {
        fingerprint,
        root_type: selection.root_type.clone(),
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        ir_version: ORTHO_DOCS_IR_VERSION.to_owned(),
        lockfile_hash,
    };

    let paths = bridge::prepare_paths(&selection, &cache_key);
    let config = build_bridge_config(&selection);

    let should_use_cache = args.cache.should_cache;
    let should_skip_build = args.cache.should_skip_build;
    let ir_json = bridge::load_or_build_ir(&config, &paths, should_use_cache, should_skip_build)?;
    let doc_metadata: DocMetadata = serde_json::from_str(&ir_json)?;

    let should_generate_ir = matches!(args.format, OutputFormat::Ir | OutputFormat::All);
    let should_generate_man = matches!(args.format, OutputFormat::Man | OutputFormat::All);
    let should_generate_ps = matches!(args.format, OutputFormat::Ps | OutputFormat::All);
    let should_generate_localized_docs =
        should_generate_ir || should_generate_man || should_generate_ps;

    let en_us_localizer = build_agent_context_localizer_if_requested(&args, &selection);
    let generation_context = GenerationContext {
        selection: &selection,
        doc_metadata: &doc_metadata,
        out_dir: &out_dir,
        en_us_localizer: en_us_localizer.as_ref(),
    };
    generate_agent_context_if_requested(&args, &generation_context)?;

    let locales = if should_generate_localized_docs {
        locale::resolve_locales(&args, &selection)?
    } else {
        Vec::new()
    };

    let localized_docs = localize_docs_if_requested(
        should_generate_localized_docs,
        &generation_context,
        &locales,
    )?;

    if should_generate_ir {
        generate_ir(&localized_docs, &out_dir)?;
    }

    if should_generate_man {
        generate_man(&localized_docs, &out_dir, &args.man)?;
    }

    if should_generate_ps {
        let ps_config = build_powershell_config(&args, &selection, &doc_metadata, &out_dir);
        generate_powershell(&localized_docs, &ps_config)?;
    }

    Ok(())
}

fn build_bridge_config(selection: &PackageSelection) -> BridgeConfig {
    BridgeConfig {
        package_root: selection.package_root.clone(),
        package_name: selection.package_name.clone(),
        root_type: selection.root_type.clone(),
        ortho_config_dependency: selection.ortho_config_dependency.clone(),
    }
}
