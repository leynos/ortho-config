//! CLI entrypoint for `cargo-orthohelp`.

mod bridge;
mod cache;
mod cli;
mod error;
mod ir;
mod locale;
mod metadata;
mod output;

use camino::Utf8PathBuf;
use clap::Parser;
use ortho_config::ORTHO_DOCS_IR_VERSION;

use crate::bridge::BridgeConfig;
use crate::cache::CacheKey;
use crate::cli::{Args, OutputFormat};
use crate::error::OrthohelpError;
use crate::metadata::PackageSelection;

fn main() -> Result<(), OrthohelpError> {
    run()
}

fn run() -> Result<(), OrthohelpError> {
    let args = Args::parse();
    if !matches!(args.format, OutputFormat::Ir) {
        return Err(OrthohelpError::UnsupportedFormat(
            args.format.as_str().to_owned(),
        ));
    }

    let metadata = metadata::load_metadata()?;
    let selection = metadata::select_package(&metadata, &args)?;
    let locales = locale::resolve_locales(&args, &selection)?;

    let out_dir = resolve_out_dir(args.out_dir.clone(), &selection);

    let fingerprint = cache::fingerprint_package(&selection.package_root)?;
    let cache_key = CacheKey::new(
        fingerprint,
        selection.root_type.clone(),
        env!("CARGO_PKG_VERSION").to_owned(),
        ORTHO_DOCS_IR_VERSION.to_owned(),
    );

    let paths = bridge::prepare_paths(&selection, &cache_key);
    let config = build_bridge_config(&selection);

    let ir_json = bridge::load_or_build_ir(&config, &paths, args.cache.cache, args.cache.no_build)?;
    let doc_metadata: ortho_config::DocMetadata = serde_json::from_str(&ir_json)?;

    for locale in locales {
        let resources = locale::load_consumer_resources(&selection.package_root, &locale)?;
        let localizer = locale::build_localizer(&locale, resources)?;
        let resolved_ir = ir::localize_doc(&doc_metadata, &locale, &localizer);
        output::write_localized_ir(&out_dir, &locale.to_string(), &resolved_ir)?;
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
