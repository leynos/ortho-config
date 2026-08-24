//! Generation-phase helpers for the CLI entrypoint.
//!
//! The binary crate keeps `main.rs` within the 400-line repository cap by
//! splitting the multi-format output pipeline into this module: agent-context
//! transformation, locale localisation, and per-format artefact generation
//! (IR, man pages, `PowerShell`) live here, while `main.rs` orchestrates
//! them.

use camino::Utf8PathBuf;

use crate::cli::{Args, OutputFormat};
use crate::error::OrthohelpError;
use crate::ir::LocalizedDocMetadata;
use crate::metadata::PackageSelection;
use crate::schema::DocMetadata;
use crate::{agent_context, ir, locale, metadata, output, policy, powershell, roff};
use ortho_config::{FluentLocalizer, LanguageIdentifier, Localizer};
use std::str::FromStr;

/// Run-scoped inputs borrowed by the output-generation phases.
pub struct GenerationContext<'a> {
    /// Cargo package selection for the run.
    pub selection: &'a PackageSelection,
    /// Parsed bridge IR document metadata.
    pub doc_metadata: &'a DocMetadata,
    /// Resolved output directory.
    pub out_dir: &'a Utf8PathBuf,
    /// Optional en-US localizer shared by agent-context and localized output.
    pub en_us_localizer: Option<&'a (LanguageIdentifier, FluentLocalizer)>,
}

/// Generates the agent-context JSON when the requested format needs it.
///
/// # Errors
///
/// Returns an I/O error when the agent-context file cannot be written.
pub fn generate_agent_context_if_requested(
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
    let mut agent_context = agent_context::bridge_ir_to_agent_context(
        context.doc_metadata,
        &context.selection.package_name,
        summary_localizer,
    );
    let policy_config = context
        .selection
        .policy
        .as_ref()
        .map(policy::config::PolicyConfig::from);
    agent_context::apply_policy_to_context(&mut agent_context, policy_config.as_ref());
    tracing::debug!(
        package = %agent_context.package,
        command_count = agent_context.commands.len(),
        "agent-context transformation complete",
    );
    output::write_agent_context(context.out_dir.as_path(), &agent_context)?;
    Ok(())
}

/// Builds the optional en-US localizer shared by agent-context and localized output.
pub fn build_agent_context_localizer_if_requested(
    args: &Args,
    selection: &PackageSelection,
) -> Option<(LanguageIdentifier, FluentLocalizer)> {
    if !matches!(args.format, OutputFormat::AgentContext | OutputFormat::All) {
        return None;
    }
    match build_en_us_localizer(&selection.package_root) {
        Ok(localizer) => Some(localizer),
        Err(error) => {
            tracing::warn!(error = %error, "no en-US localizer; agent-context summaries omitted");
            None
        }
    }
}

/// Builds an en-US localizer from the package's consumer resources.
///
/// # Errors
///
/// Returns a locale or I/O error when the resources cannot be resolved.
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

/// Localizes the document when the requested formats need localized output.
///
/// # Errors
///
/// Returns an error when a locale's resources cannot be resolved.
pub fn localize_docs_if_requested(
    should_generate_localized_docs: bool,
    context: &GenerationContext<'_>,
    locales: &[LanguageIdentifier],
) -> Result<Vec<LocalizedDocMetadata>, OrthohelpError> {
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

/// Localizes the document metadata for every requested locale.
///
/// # Errors
///
/// Returns an error when a locale's resources cannot be resolved.
fn localize_docs(
    package_root: &Utf8PathBuf,
    doc_metadata: &DocMetadata,
    locales: &[LanguageIdentifier],
    en_us_localizer: Option<&(LanguageIdentifier, FluentLocalizer)>,
) -> Result<Vec<LocalizedDocMetadata>, OrthohelpError> {
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

/// Builds the `PowerShell` generation configuration for the requested output.
pub fn build_powershell_config(
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

/// Writes one localized IR JSON artefact per locale.
///
/// # Errors
///
/// Returns an I/O error when an IR artefact cannot be written.
pub fn generate_ir(
    localized_docs: &[LocalizedDocMetadata],
    out_dir: &Utf8PathBuf,
) -> Result<(), OrthohelpError> {
    for doc in localized_docs {
        output::write_localized_ir(out_dir.as_path(), &doc.locale, doc)?;
    }
    Ok(())
}

/// Generates and writes man pages for every localized document.
///
/// # Errors
///
/// Returns an error when a man section is invalid or man pages cannot be
/// written.
pub fn generate_man(
    localized_docs: &[LocalizedDocMetadata],
    out_dir: &Utf8PathBuf,
    man_args: &crate::cli::ManArgs,
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

/// Generates `PowerShell` wrapper modules and MAML help.
///
/// # Errors
///
/// Returns an error when the `PowerShell` artefacts cannot be generated.
pub fn generate_powershell(
    localized_docs: &[LocalizedDocMetadata],
    ps_config: &powershell::PowerShellConfig,
) -> Result<(), OrthohelpError> {
    // Keep the generated artefact list binding for future CLI reporting.
    let _generated_output = powershell::generate(localized_docs, ps_config)?;
    Ok(())
}

/// Resolves the output directory, defaulting to the selected package's
/// conventional `orthohelp/out` directory.
pub fn resolve_out_dir(out_dir: Option<Utf8PathBuf>, selection: &PackageSelection) -> Utf8PathBuf {
    out_dir.unwrap_or_else(|| selection.target_directory.join("orthohelp").join("out"))
}
