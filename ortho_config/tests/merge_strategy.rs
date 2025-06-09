#![allow(non_snake_case)]
use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize, OrthoConfig)]
struct VecConfig {
    #[ortho_config(merge_strategy = "append")]
    values: Vec<String>,
}

#[derive(Debug, Deserialize, OrthoConfig)]
struct DefaultVec {
    #[ortho_config(default = vec!["def".to_string()], merge_strategy = "append")]
    values: Vec<String>,
}

#[test]
fn append_merges_all_sources() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cfg = VecConfig::load_from_iter(["prog", "--values", "cli1", "--values", "cli2"])
            .expect("load");
        assert_eq!(cfg.values, vec!["file", "env", "cli1", "cli2"]);
        Ok(())
    });
}

#[test]
fn append_includes_defaults() {
    figment::Jail::expect_with(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cfg = DefaultVec::load_from_iter(["prog", "--values", "cli"]).expect("load");
        assert_eq!(cfg.values, vec!["def", "file", "env", "cli"]);
        Ok(())
    });
}
