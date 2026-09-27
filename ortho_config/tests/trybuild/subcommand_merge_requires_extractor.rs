//! Compile-time contract proving the two match-aware `SubcmdConfigMerge`
//! methods are unavailable to a type with no `CliValueExtractor` impl.
//!
//! `NoExtractorArgs` satisfies the trait's own bounds — it is `OrthoConfig`,
//! `Serialize`, `Default` and `CommandFactory` — and the two calls below prove
//! it, since they only compile when `SubcmdConfigMerge` is implemented. What it
//! lacks is `CliValueExtractor`: it declares no `cli_default_as_absent` field,
//! so the derive emits no implementation. The pair that needs the match
//! metadata is therefore withheld while the plain pair stays available.

use clap::{CommandFactory, Parser};
use ortho_config::{MapEnv, OrthoConfig, SharedScanEnvSource, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Subcommand that narrows the CLI layer without asking for match metadata.
#[derive(Debug, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[command(name = "no-extractor")]
#[ortho_config(prefix = "TRYBUILD_CONTRACT_")]
struct NoExtractorArgs {
    /// A plain option, deliberately without `cli_default_as_absent`.
    #[arg(long)]
    level: Option<u32>,
}

fn main() {
    let matches = NoExtractorArgs::command().get_matches_from(["no-extractor"]);
    let cli = NoExtractorArgs::default();
    let source: SharedScanEnvSource = Arc::new(MapEnv::new());

    // Within the trait's own bounds: these must keep compiling.
    let _ = cli.load_and_merge();
    let _ = cli.load_and_merge_with_sources(Arc::clone(&source));

    // Outside them: no `CliValueExtractor`, so neither call can resolve.
    let _ = cli.load_and_merge_with_matches(&matches);
    let _ = cli.load_and_merge_with_matches_with_sources(&matches, source);
}
