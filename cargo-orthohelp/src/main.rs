//! CLI entrypoint for `cargo-orthohelp`.

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
pub mod schema;

use camino::Utf8PathBuf;
use clap::Parser;

use crate::bridge::BridgeConfig;
use crate::cache::CacheKey;
use crate::cli::{Args, OutputFormat};
use crate::error::OrthohelpError;
use crate::metadata::PackageSelection;
use crate::schema::{DocMetadata, ORTHO_DOCS_IR_VERSION};

fn main() -> Result<(), OrthohelpError> {
    run()
}

fn run() -> Result<(), OrthohelpError> {
    let args = Args::parse();

    let metadata = metadata::load_metadata()?;
    let selection = metadata::select_package(&metadata, &args)?;
    let locales = locale::resolve_locales(&args, &selection)?;

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

    let localized_docs = localize_docs(&selection.package_root, &doc_metadata, &locales)?;

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

fn localize_docs(
    package_root: &Utf8PathBuf,
    doc_metadata: &DocMetadata,
    locales: &[ortho_config::LanguageIdentifier],
) -> Result<Vec<ir::LocalizedDocMetadata>, OrthohelpError> {
    let mut localized_docs = Vec::new();
    for locale in locales {
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
        .or_else(|| windows.module_name.clone())
        .unwrap_or_else(|| bin_name.clone());

    if let Some(split_subcommands) = args.powershell.should_split_subcommands {
        windows.should_split_subcommands_into_functions = split_subcommands;
    }
    if let Some(include_common_parameters) = args.powershell.should_include_common_parameters {
        windows.should_include_common_parameters = include_common_parameters;
    }
    if let Some(help_info_uri) = args.powershell.help_info_uri.clone() {
        windows.help_info_uri = Some(help_info_uri);
    }

    powershell::PowerShellConfig {
        out_dir: out_dir.clone(),
        module_name: module_name.into(),
        module_version: selection.package_version.clone().into(),
        bin_name: bin_name.into(),
        export_aliases: windows
            .export_aliases
            .iter()
            .cloned()
            .map(Into::into)
            .collect(),
        should_include_common_parameters: windows.should_include_common_parameters,
        should_split_subcommands: windows.should_split_subcommands_into_functions,
        help_info_uri: windows.help_info_uri.clone().map(Into::into),
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
