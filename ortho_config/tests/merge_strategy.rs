//! Tests for the append merge strategy on vectors.

#![allow(non_snake_case)]
use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig)]
struct VecConfig {
    #[ortho_config(merge_strategy = "append")]
    #[arg(long)]
    values: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig)]
struct DefaultVec {
    #[ortho_config(default = vec!["def".to_string()], merge_strategy = "append")]
    #[arg(long)]
    values: Vec<String>,
}

#[test]
fn append_merges_all_sources() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cli = VecConfig::parse_from(["prog", "--values", "cli1", "--values", "cli2"]);
        let cfg = cli.load_and_merge().expect("load");
        assert_eq!(cfg.values, vec!["file", "env", "cli1", "cli2"]);
        Ok(())
    });
}

#[test]
fn append_includes_defaults() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cli = DefaultVec::parse_from(["prog", "--values", "cli"]);
        let cfg = cli.load_and_merge().expect("load");
        assert_eq!(cfg.values, vec!["def", "file", "env", "cli"]);
        Ok(())
    });
}
