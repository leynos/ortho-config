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

    // PowerShell format is not yet implemented
    if matches!(args.format, OutputFormat::Ps) {
        return Err(OrthohelpError::UnsupportedFormat(
            args.format.as_str().to_owned(),
        ));
    }

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
    let has_multiple_locales = locales.len() > 1;

    for locale in locales {
        generate_outputs_for_locale(
            &selection.package_root,
            &doc_metadata,
            &locale,
            &out_dir,
            &args.man,
            should_generate_ir,
            should_generate_man,
            has_multiple_locales,
        )?;
    }

    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "Extracted helper keeps run() concise; grouping into a struct would over-complicate."
)]
fn generate_outputs_for_locale(
    package_root: &Utf8PathBuf,
    doc_metadata: &DocMetadata,
    locale: &ortho_config::LanguageIdentifier,
    out_dir: &Utf8PathBuf,
    man_args: &cli::ManArgs,
    should_generate_ir: bool,
    should_generate_man: bool,
    has_multiple_locales: bool,
) -> Result<(), OrthohelpError> {
    let resources = locale::load_consumer_resources(package_root, locale)?;
    let localizer = locale::build_localizer(locale, resources)?;
    let resolved_ir = ir::localize_doc(doc_metadata, locale, &localizer);

    if should_generate_ir {
        output::write_localized_ir(out_dir, &locale.to_string(), &resolved_ir)?;
    }

    if should_generate_man {
        let section = roff::ManSection::new(man_args.section)?;
        // Use locale-specific subdirectory when generating for multiple locales
        // to prevent overwrites (e.g., out/en-US/man/man1/ vs out/ja/man/man1/).
        let man_out_dir = if has_multiple_locales {
            out_dir.join(locale.to_string())
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
        roff::generate(&resolved_ir, &roff_config)?;
    }

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
