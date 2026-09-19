//! CLI entrypoint for `cargo-orthohelp`.
//!
//! The binary accepts Cargo's external-subcommand dispatch shape through
//! [`cli::Cli`], then delegates to the metadata, locale, cache, bridge, and
//! output modules to build localized documentation artefacts. `main` keeps the
//! process boundary thin by forwarding all fallible work through `run`, where
//! parsed `orthohelp` arguments are converted into package selection, bridge
//! configuration, localized IR, and renderer-specific outputs.

pub mod agent_context;
mod agent_native;
mod bridge;
mod cache;
mod cli;
mod error;
mod fs_helpers;
mod hex;
mod ir;
mod locale;
mod metadata;
mod output;
pub mod powershell;
pub mod roff;
mod rustflags;
pub mod schema;
#[cfg(test)]
mod test_support;
use crate::agent_native::{
    AgentContextResources, GenerationPlan, build_resources, run_check, write_agent_context,
};
use crate::bridge::BridgeConfig;
use crate::cache::CacheKey;
use crate::cli::{Args, CargoSubcommand, Cli, OutputFormat};
use crate::error::OrthohelpError;
use crate::metadata::PackageSelection;
use crate::schema::{DocMetadata, ORTHO_DOCS_IR_VERSION};
use camino::Utf8PathBuf;
use clap::{CommandFactory, Error as ClapError, FromArgMatches, error::ErrorKind};
use ortho_config::{FluentLocalizer, LanguageIdentifier};
use std::io::Write;
use tracing_subscriber::EnvFilter;

/// Run-scoped inputs borrowed by the output-generation phases.
struct GenerationContext<'a> {
    args: &'a Args,
    selection: &'a PackageSelection,
    doc_metadata: &'a DocMetadata,
    out_dir: &'a Utf8PathBuf,
    en_us_localizer: Option<&'a (LanguageIdentifier, FluentLocalizer)>,
}

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
        .with_writer(std::io::stderr)
        .try_init();
}

fn parse_cli() -> Result<(Cli, bool), ClapError> {
    let matches = Cli::command().try_get_matches()?;
    let cli = Cli::from_arg_matches(&matches)?;
    let format_was_explicit = matches
        .subcommand()
        .and_then(|(_, sub_matches)| sub_matches.value_source("format"))
        .is_some_and(|source| source != clap::parser::ValueSource::DefaultValue);
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

fn run(cli: Cli, format_was_explicit: bool) -> Result<(), OrthohelpError> {
    let Cli {
        command: CargoSubcommand::Orthohelp(args),
    } = cli;
    tracing::debug!(
        "cargo-orthohelp dispatched via Cargo external-subcommand (orthohelp token present)"
    );

    let metadata = metadata::load_metadata()?;
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

    let check_mode = args
        .check_agent_native
        .map(cargo_orthohelp::policy::PolicyMode::from)
        .filter(|mode| *mode != cargo_orthohelp::policy::PolicyMode::Off);
    let check_flag_present = check_mode.is_some();
    let requests_agent_context =
        matches!(args.format, OutputFormat::AgentContext | OutputFormat::All);
    let agent_context_resources = (check_flag_present || requests_agent_context)
        .then(|| build_resources(&doc_metadata, &selection));
    let check_has_deny_findings = match (check_mode, agent_context_resources.as_ref()) {
        (Some(mode), Some(resources)) => run_check(resources.context(), mode)?,
        _ => false,
    };

    let plan = GenerationPlan::for_run(args.format, check_flag_present, format_was_explicit);

    let en_us_localizer = agent_context_resources
        .as_ref()
        .and_then(AgentContextResources::en_us_localizer);
    let generation_context = GenerationContext {
        args: &args,
        selection: &selection,
        doc_metadata: &doc_metadata,
        out_dir: &out_dir,
        en_us_localizer,
    };

    let locales = if plan.should_generate_localized_docs {
        locale::resolve_locales(&args, &selection)?
    } else {
        Vec::new()
    };

    let localized_docs = localize_docs_if_requested(
        plan.should_generate_localized_docs,
        &generation_context,
        &locales,
    )?;

    generate_requested_artefacts(
        &plan,
        &generation_context,
        &localized_docs,
        agent_context_resources.as_ref(),
    )?;

    // The lint's report is emitted before generation so a CI pipeline can
    // parse it regardless of the exit path. The failure exit for deny-level
    // findings must not pre-empt explicitly requested artefact generation
    // (milestone E composition contract): when `--format` is explicit, the
    // command still writes its artefacts and only then exits 3.
    if check_has_deny_findings {
        std::process::exit(3);
    }

    Ok(())
}

fn generate_requested_artefacts(
    plan: &GenerationPlan,
    context: &GenerationContext<'_>,
    localized_docs: &[ir::LocalizedDocMetadata],
    agent_context_resources: Option<&AgentContextResources>,
) -> Result<(), OrthohelpError> {
    if plan.should_generate_agent_context
        && let Some(resources) = agent_context_resources
    {
        write_agent_context(context.out_dir, resources)?;
    }
    if plan.should_generate_ir {
        generate_ir(localized_docs, context.out_dir)?;
    }
    if plan.should_generate_man {
        generate_man(localized_docs, context.out_dir, &context.args.man)?;
    }
    if plan.should_generate_ps {
        let ps_config = build_powershell_config(
            context.args,
            context.selection,
            context.doc_metadata,
            context.out_dir,
        );
        generate_powershell(localized_docs, &ps_config)?;
    }
    Ok(())
}

fn localize_docs_if_requested(
    should_generate_localized_docs: bool,
    context: &GenerationContext<'_>,
    locales: &[ortho_config::LanguageIdentifier],
) -> Result<Vec<ir::LocalizedDocMetadata>, OrthohelpError> {
    if should_generate_localized_docs {
        localize_docs(
            &context.selection.package_root,
            context.doc_metadata,
            locales,
            context.en_us_localizer,
        )
    } else {
        Ok(Vec::new())
    }
}

fn localize_docs(
    package_root: &Utf8PathBuf,
    doc_metadata: &DocMetadata,
    locales: &[ortho_config::LanguageIdentifier],
    en_us_localizer: Option<&(LanguageIdentifier, FluentLocalizer)>,
) -> Result<Vec<ir::LocalizedDocMetadata>, OrthohelpError> {
    let mut localized_docs = Vec::new();
    for locale in locales {
        if let Some((cached_locale, cached_localizer)) = en_us_localizer
            && locale == cached_locale
        {
            localized_docs.push(ir::localize_doc(doc_metadata, locale, cached_localizer));
            continue;
        }
        let resources = locale::load_consumer_resources(package_root, locale)?;
        let doc_localizer = locale::build_localizer(locale, resources)?;
        localized_docs.push(ir::localize_doc(doc_metadata, locale, &doc_localizer));
    }
    Ok(localized_docs)
}

fn build_powershell_config(
    args: &Args,
    selection: &PackageSelection,
    doc_metadata: &DocMetadata,
    out_dir: &Utf8PathBuf,
) -> powershell::PowerShellConfig {
    let base_windows = selection.windows.as_ref().map_or_else(
        || {
            doc_metadata
                .windows
                .clone()
                .map(metadata::ResolvedWindowsMetadata::from)
                .unwrap_or_default()
        },
        |metadata| metadata.resolve(doc_metadata.windows.as_ref()),
    );
    let mut windows = base_windows;

    let bin_name = doc_metadata
        .bin_name
        .as_ref()
        .unwrap_or(&doc_metadata.app_name)
        .clone();
    let module_name = args
        .powershell
        .module_name
        .clone()
        .map(Into::into)
        .or_else(|| windows.module_name.clone())
        .unwrap_or_else(|| bin_name.as_str().into());

    if let Some(split_subcommands) = args.powershell.should_split_subcommands {
        windows.should_split_subcommands_into_functions = split_subcommands;
    }
    if let Some(include_common_parameters) = args.powershell.should_include_common_parameters {
        windows.should_include_common_parameters = include_common_parameters;
    }
    if let Some(help_info_uri) = args.powershell.help_info_uri.clone() {
        windows.help_info_uri = Some(help_info_uri.into());
    }

    powershell::PowerShellConfig {
        out_dir: out_dir.clone(),
        module_name,
        module_version: selection.package_version.clone().into(),
        bin_name: bin_name.into(),
        export_aliases: windows.export_aliases.clone(),
        should_include_common_parameters: windows.should_include_common_parameters,
        should_split_subcommands: windows.should_split_subcommands_into_functions,
        help_info_uri: windows.help_info_uri.clone(),
        should_ensure_en_us: args.powershell.should_ensure_en_us,
    }
}

fn generate_ir(
    localized_docs: &[ir::LocalizedDocMetadata],
    out_dir: &Utf8PathBuf,
) -> Result<(), OrthohelpError> {
    for doc in localized_docs {
        output::write_localized_ir(out_dir.as_path(), &doc.locale, doc)?;
    }
    Ok(())
}

fn generate_man(
    localized_docs: &[ir::LocalizedDocMetadata],
    out_dir: &Utf8PathBuf,
    man_args: &cli::ManArgs,
) -> Result<(), OrthohelpError> {
    let has_multiple_locales = localized_docs.len() > 1;
    for doc in localized_docs {
        let section = roff::ManSection::new(man_args.section)?;
        // Use locale-specific subdirectory when generating for multiple locales
        // to prevent overwrites (e.g., out/en-US/man/man1/ vs out/ja/man/man1/).
        let man_out_dir = if has_multiple_locales {
            out_dir.join(&doc.locale)
        } else {
            out_dir.clone()
        };
        let roff_config = roff::RoffConfig {
            out_dir: man_out_dir,
            section,
            date: man_args.date.clone(),
            should_split_subcommands: man_args.should_split_subcommands,
            source: None,
            manual: None,
        };
        roff::generate(doc, &roff_config)?;
    }
    Ok(())
}

fn generate_powershell(
    localized_docs: &[ir::LocalizedDocMetadata],
    ps_config: &powershell::PowerShellConfig,
) -> Result<(), OrthohelpError> {
    // Keep the generated artefact list available for future CLI reporting while
    // the command currently only signals success/failure via exit status.
    let _generated_output = powershell::generate(localized_docs, ps_config)?;
    Ok(())
}

fn resolve_out_dir(out_dir: Option<Utf8PathBuf>, selection: &PackageSelection) -> Utf8PathBuf {
    out_dir.unwrap_or_else(|| selection.target_directory.join("orthohelp").join("out"))
}

fn build_bridge_config(selection: &PackageSelection) -> BridgeConfig {
    BridgeConfig {
        package_root: selection.package_root.clone(),
        package_name: selection.package_name.clone(),
        root_type: selection.root_type.clone(),
        ortho_config_dependency: selection.ortho_config_dependency.clone(),
    }
}
