//! Tests for configuration inheritance using the `extends` key.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig)]
struct ExtendsCfg {
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
}

#[test]
fn extended_file_overrides_base() {
    figment::Jail::expect_with(|j| {
        j.create_file("base.toml", "foo = \"base\"")?;
        j.create_file(".config.toml", "extends = \"base.toml\"\nfoo = \"child\"")?;
        let cli = ExtendsCfg::parse_from(["prog"]);
        let cfg = cli.load_and_merge().expect("load");
        assert_eq!(cfg.foo.as_deref(), Some("child"));
        Ok(())
    });
}

#[test]
fn env_and_cli_override_extended_file() {
    figment::Jail::expect_with(|j| {
        j.create_file("base.toml", "foo = \"base\"")?;
        j.create_file(".config.toml", "extends = \"base.toml\"\nfoo = \"file\"")?;
        j.set_env("FOO", "env");
        let cli = ExtendsCfg::parse_from(["prog", "--foo", "cli"]);
        let cfg = cli.load_and_merge().expect("load");
        assert_eq!(cfg.foo.as_deref(), Some("cli"));
        Ok(())
    });
}
