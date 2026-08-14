//! CLI entrypoint for `cargo-orthohelp`.
//!
//! The binary accepts Cargo's external-subcommand dispatch shape through
//! [`cli::Cli`], then delegates to the metadata, locale, cache, bridge, and
//! output modules to build localized documentation artefacts. `main` keeps the
//! process boundary thin by forwarding all fallible work through `run`, where
//! parsed `orthohelp` arguments are converted into package selection, bridge
//! configuration, localized IR, and renderer-specific outputs.

pub mod agent_context;
mod bridge;
mod cache;
mod cli;
mod error;
mod fs_helpers;
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
use crate::bridge::BridgeConfig;
use crate::cache::CacheKey;
use crate::cli::{Args, CargoSubcommand, Cli, OutputFormat};
use crate::error::OrthohelpError;
use crate::metadata::PackageSelection;
use crate::schema::{DocMetadata, ORTHO_DOCS_IR_VERSION};
use camino::Utf8PathBuf;
use clap::{CommandFactory, Error as ClapError, FromArgMatches, error::ErrorKind};
use ortho_config::{FluentLocalizer, LanguageIdentifier, Localizer};
use std::io::Write;
use std::str::FromStr;
use tracing_subscriber::EnvFilter;

/// Run-scoped inputs borrowed by the output-generation phases.
struct GenerationContext<'a> {
    selection: &'a PackageSelection,
    doc_metadata: &'a DocMetadata,
    out_dir: &'a Utf8PathBuf,
    en_us_localizer: Option<&'a (LanguageIdentifier, FluentLocalizer)>,
}

/// Decides which artefact families a run should generate.
///
/// The five booleans each gate one distinct artefact family (IR, man page,
/// `PowerShell`, agent context, localized docs), so collapsing them into
/// two-variant enums would obscure the per-family skip decisions made in
/// [`GenerationPlan::for_run`]. The lint is suppressed with that rationale.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each boolean gates one distinct artefact family; collapsing them into enums would obscure the per-family skip decisions"
)]
struct GenerationPlan {
    should_generate_ir: bool,
    should_generate_man: bool,
    should_generate_ps: bool,
    should_generate_agent_context: bool,
    should_generate_localized_docs: bool,
}

impl GenerationPlan {
    /// Builds the plan for a run.
    ///
    /// When only the lint flag is present and the default `--format ir` was
    /// not explicitly requested, artefact generation is skipped entirely: the
    /// answer to the check is on stdout and no files were asked for.
    const fn for_run(args: &Args, check_flag_present: bool, format_was_explicit: bool) -> Self {
        let should_skip_artefacts = check_flag_present && !format_was_explicit;
        let should_generate_ir =
            !should_skip_artefacts && matches!(args.format, OutputFormat::Ir | OutputFormat::All);
        let should_generate_man =
            !should_skip_artefacts && matches!(args.format, OutputFormat::Man | OutputFormat::All);
        let should_generate_ps =
            !should_skip_artefacts && matches!(args.format, OutputFormat::Ps | OutputFormat::All);
        let should_generate_agent_context = !should_skip_artefacts
            && matches!(args.format, OutputFormat::AgentContext | OutputFormat::All);
        let should_generate_localized_docs =
            should_generate_ir || should_generate_man || should_generate_ps;
        Self {
            should_generate_ir,
            should_generate_man,
            should_generate_ps,
            should_generate_agent_context,
            should_generate_localized_docs,
        }
    }
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
    let matches = Cli::command().get_matches();
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

    let check_flag_present = args.check_agent_native.is_some();
    if let Some(mode) = args
        .check_agent_native
        .map(cargo_orthohelp::policy::PolicyMode::from)
    {
        run_agent_native_check(&doc_metadata, &selection, mode, &out_dir)?;
    }

    let plan = GenerationPlan::for_run(&args, check_flag_present, format_was_explicit);

    let en_us_localizer = build_agent_context_localizer_if_requested(&args, &selection);
    let generation_context = GenerationContext {
        selection: &selection,
        doc_metadata: &doc_metadata,
        out_dir: &out_dir,
        en_us_localizer: en_us_localizer.as_ref(),
    };
    if plan.should_generate_agent_context {
        generate_agent_context_if_requested(&args, &generation_context)?;
    }

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

    if plan.should_generate_ir {
        generate_ir(&localized_docs, &out_dir)?;
    }

    if plan.should_generate_man {
        generate_man(&localized_docs, &out_dir, &args.man)?;
    }

    if plan.should_generate_ps {
        let ps_config = build_powershell_config(&args, &selection, &doc_metadata, &out_dir);
        generate_powershell(&localized_docs, &ps_config)?;
    }

    Ok(())
}

fn generate_agent_context_if_requested(
    args: &Args,
    context: &GenerationContext<'_>,
) -> Result<(), OrthohelpError> {
    if !matches!(args.format, OutputFormat::AgentContext | OutputFormat::All) {
        tracing::debug!(
            package = %context.selection.package_name,
            format = ?args.format,
            "agent-context generation skipped for requested format",
        );
        return Ok(());
    }
    tracing::debug!(
        package = %context.selection.package_name,
        format = "agent-context",
        "starting agent-context transformation",
    );
    let summary_localizer = context
        .en_us_localizer
        .map(|(_, resolved_localizer)| resolved_localizer as &dyn Localizer);
    let agent_context = agent_context::bridge_ir_to_agent_context(
        context.doc_metadata,
        &context.selection.package_name,
        summary_localizer,
    );
    tracing::debug!(
        package = %agent_context.package,
        command_count = agent_context.commands.len(),
        "agent-context transformation complete",
    );
    output::write_agent_context(context.out_dir.as_path(), &agent_context)?;
    Ok(())
}

/// Runs the agent-native behaviour lint and emits its report.
///
/// The policy report is written to stdout as exactly one JSON document, a
/// human-readable summary goes to stderr, and the process exits with code 3
/// if and only if the report contains at least one `deny` finding. Runtime
/// errors keep exit code 1; clap usage errors keep exit code 2.
fn run_agent_native_check(
    doc_metadata: &DocMetadata,
    selection: &metadata::PackageSelection,
    mode: cargo_orthohelp::policy::PolicyMode,
    out_dir: &Utf8PathBuf,
) -> Result<(), OrthohelpError> {
    let maybe_localizer = build_en_us_localizer(&selection.package_root)
        .ok()
        .map(|(_, localizer)| localizer);
    let summary_localizer = maybe_localizer
        .as_ref()
        .map(|localizer| localizer as &dyn ortho_config::Localizer);
    let agent_context = agent_context::bridge_ir_to_agent_context(
        doc_metadata,
        &selection.package_name,
        summary_localizer,
    );
    let report = cargo_orthohelp::policy::rules::behaviour::check_behaviour(&agent_context, mode);
    let report_json = serde_json::to_string(&report)?;
    {
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{report_json}").map_err(|source| OrthohelpError::Io {
            path: Utf8PathBuf::from("<stdout>"),
            source,
        })?;
    }

    {
        let mut stderr = std::io::stderr().lock();
        writeln!(
            stderr,
            "agent-native behaviour check: {} finding(s) ({} deny)",
            report.summary.total, report.summary.deny
        )
        .map_err(|source| OrthohelpError::Io {
            path: Utf8PathBuf::from("<stderr>"),
            source,
        })?;
    }
    if report.summary.deny > 0 {
        std::process::exit(3);
    }
    let _ = out_dir;
    Ok(())
}

/// Builds the optional en-US localizer shared by agent-context and localized output.
fn build_agent_context_localizer_if_requested(
    args: &Args,
    selection: &PackageSelection,
) -> Option<(LanguageIdentifier, FluentLocalizer)> {
    if !matches!(args.format, OutputFormat::AgentContext | OutputFormat::All) {
        return None;
    }
    match build_en_us_localizer(&selection.package_root) {
        Ok(localizer) => Some(localizer),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "no en-US localizer available; agent-context summaries will be omitted",
            );
            None
        }
    }
}

fn build_en_us_localizer(
    package_root: &Utf8PathBuf,
) -> Result<(LanguageIdentifier, FluentLocalizer), OrthohelpError> {
    let locale =
        LanguageIdentifier::from_str("en-US").map_err(|err| OrthohelpError::InvalidLocale {
            value: "en-US".to_owned(),
            message: err.to_string(),
        })?;
    let resources = locale::load_consumer_resources(package_root, &locale)?;
    let localizer = locale::build_localizer(&locale, resources)?;
    Ok((locale, localizer))
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
