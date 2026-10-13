//! Profile layering through an `extends` chain.
//!
//! `extends` resolves at the figment layer, so the parent and the child each
//! arrive at profile extraction as their own file layer. These cases pin where
//! that leaves the profile tier relative to the file tiers.

use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use ortho_config::declarative::MergeProvenance;
use serde::{Deserialize, Serialize};

#[path = "support/profile_chain_assertions.rs"]
mod profile_chain_assertions;
#[path = "test_utils.rs"]
mod test_utils;

use profile_chain_assertions::ensure_eq;
use test_utils::with_jail;

/// An `extends` chain that defines the same profile in both files.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(profiles)]
struct ProfiledExtendsCfg {
    app_name: String,
    retries: u8,
    notes: String,
}

/// A profile defined in two files of an `extends` chain layers per file.
///
/// `extends` resolves at the figment layer, so the parent and the child each
/// arrive at profile extraction as their own file layer. The selected profile
/// therefore contributes one layer per defining file — parent first, child
/// second — and both are pushed after every file layer and before the
/// environment, which is what makes the profile tier sit strictly between
/// configuration files and the environment.
#[test]
fn extends_chain_profile_layers_in_chain_order() -> Result<()> {
    with_jail(|j| {
        j.create_file(
            "base.toml",
            concat!(
                "app_name = \"base\"\n",
                "retries = 1\n",
                "notes = \"base\"\n",
                "[profile.ci]\n",
                "retries = 5\n",
                "notes = \"base-ci\"\n",
            ),
        )?;
        j.create_file(
            ".config.toml",
            concat!(
                "extends = \"base.toml\"\n",
                "app_name = \"child\"\n",
                "retries = 2\n",
                "[profile.ci]\n",
                "retries = 7\n",
            ),
        )?;

        let outcome = ProfiledExtendsCfg::load_with_profile_from_iter(["prog", "--profile", "ci"])
            .map_err(|err| anyhow!(err))?;
        let selection = outcome.selection();
        let [selected] = selection else {
            return Err(anyhow!(
                "expected exactly one selected profile, got {selection:?}"
            ));
        };
        ensure!(
            selected.name.as_str() == "ci",
            "expected profile `ci` to be selected, got {selected:?}"
        );
        let cfg = outcome.into_config();

        // The selected profile's retries come from the child layer, the last
        // one pushed; `notes` is set only by the parent's profile layer and
        // survives, proving both profile layers contributed.
        ensure_eq(&cfg.retries, &7, "retries")?;
        ensure_eq(&cfg.notes.as_str(), &"base-ci", "notes")?;
        // Base file values still win where no profile overrides them.
        ensure_eq(&cfg.app_name.as_str(), &"child", "app_name")?;
        Ok(())
    })
}

/// Profile layers are ordered after file layers and carry their source paths.
#[test]
fn extends_chain_profile_layers_follow_every_file_layer() -> Result<()> {
    with_jail(|j| {
        j.create_file(
            "base.toml",
            concat!(
                "app_name = \"base\"\n",
                "retries = 1\n",
                "notes = \"base\"\n",
                "[profile.ci]\n",
                "retries = 5\n",
            ),
        )?;
        j.create_file(
            ".config.toml",
            concat!(
                "extends = \"base.toml\"\n",
                "app_name = \"child\"\n",
                "retries = 2\n",
                "[profile.ci]\n",
                "retries = 7\n",
            ),
        )?;

        let composition = ProfiledExtendsCfg::compose_layers_from_iter([
            "prog".to_owned(),
            "--profile".to_owned(),
            "ci".to_owned(),
        ]);
        let (layers, errors) = composition.into_parts();
        ensure!(
            errors.is_empty(),
            "composition should not record errors, got {errors:?}"
        );

        let provenance: Vec<MergeProvenance> = layers
            .iter()
            .map(ortho_config::declarative::MergeLayer::provenance)
            .collect();
        let last_file = provenance
            .iter()
            .rposition(|p| *p == MergeProvenance::File)
            .ok_or_else(|| anyhow!("composition should contain file layers"))?;
        let first_profile = provenance
            .iter()
            .position(|p| *p == MergeProvenance::Profile)
            .ok_or_else(|| anyhow!("composition should contain profile layers"))?;
        ensure!(
            first_profile > last_file,
            "every profile layer should be pushed after every file layer, got {provenance:?}"
        );

        let profile_paths: Vec<String> = layers
            .iter()
            .filter(|layer| layer.provenance() == MergeProvenance::Profile)
            .filter_map(|layer| layer.path())
            .filter_map(|path| path.file_name())
            .map(str::to_owned)
            .collect();
        ensure_eq(
            &profile_paths,
            &vec!["base.toml".to_owned(), ".config.toml".to_owned()],
            "profile layer source paths in chain order",
        )?;
        Ok(())
    })
}
