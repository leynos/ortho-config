//! Serde-visible keys in generated CLI, defaults, and collection merge paths.
//!
//! A renamed field must enter every merge layer under the key the final
//! configuration type reads. These cases exercise the generated CLI struct,
//! the defaults struct, and declarative collection append across the defaults,
//! file, environment, and CLI rungs.

use anyhow::{Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoConfig};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Load `T` through both injected source capabilities.
fn load_config<T: OrthoConfig>(args: &[&str], env: MapEnv) -> Result<T> {
    let source = Arc::new(env);
    let resolved_args: Vec<OsString> = args.iter().map(OsString::from).collect();
    T::load_from_iter_with_sources(resolved_args, source.clone(), source)
        .map_err(|err| anyhow!(err))
}

/// Write `contents` to `config.toml` beneath `dir`, returning its full path.
fn write_config(dir: &Path, contents: &str) -> Result<PathBuf> {
    let cap = Dir::open_ambient_dir(dir, ambient_authority())?;
    cap.write("config.toml", contents.as_bytes())?;
    Ok(dir.join("config.toml"))
}

/// Build an injected environment pinned to `root` and free of host sources.
///
/// `MapEnv::with_current_dir` closes the project-root rung, and
/// `XDG_CONFIG_DIRS` closes the platform `/etc/xdg` fallback, so only fixtures
/// this case names can contribute.
fn layered_env(root: &Path, vars: &[(&str, &str)]) -> MapEnv {
    vars.iter()
        .fold(MapEnv::new().with_current_dir(root), |env, (key, value)| {
            env.with_var(*key, *value)
        })
        .with_var("XDG_CONFIG_DIRS", root.as_os_str())
}

/// A scalar renamed by the container rule, with a declared default.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "SVK_")]
#[serde(rename_all = "kebab-case")]
struct RenamedScalar {
    #[ortho_config(default = String::from("def"))]
    greeting_text: String,
}

/// Load `RenamedScalar` with the supplied file and environment layers.
fn load_scalar(
    file: Option<&str>,
    env_vars: &[(&str, &str)],
    cli_args: &[&str],
) -> Result<RenamedScalar> {
    let temp_dir = tempfile::tempdir()?;
    let mut vars: Vec<(String, String)> = env_vars
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    if let Some(contents) = file {
        let path = write_config(temp_dir.path(), contents)?;
        vars.push((
            String::from("SVK_CONFIG_PATH"),
            path.to_string_lossy().into_owned(),
        ));
    }
    let borrowed: Vec<(&str, &str)> = vars
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let env = layered_env(temp_dir.path(), &borrowed);
    load_config::<RenamedScalar>(cli_args, env)
}

/// Every rung supplies a value; the CLI must win under the renamed key.
#[test]
fn renamed_scalar_cli_override_wins_across_layers() -> Result<()> {
    let config = load_scalar(
        Some("greeting-text = \"file\"\n"),
        &[("SVK_GREETING-TEXT", "env")],
        &["prog", "--greeting-text", "cli"],
    )?;
    ensure!(
        config.greeting_text == "cli",
        "expected the CLI override to win under the kebab-case key, got {:?}",
        config.greeting_text
    );
    Ok(())
}

/// The defaults layer and the file layer use the renamed key too.
#[rstest]
#[case::defaults_only(None, &[], "def")]
#[case::file_over_defaults(Some("greeting-text = \"file\"\n"), &[], "file")]
#[case::environment_over_file(
    Some("greeting-text = \"file\"\n"),
    &[("SVK_GREETING-TEXT", "env")],
    "env"
)]
fn renamed_scalar_layer_precedence(
    #[case] file: Option<&str>,
    #[case] env_vars: &[(&str, &str)],
    #[case] expected: &str,
) -> Result<()> {
    let config = load_scalar(file, env_vars, &["prog"])?;
    ensure!(
        config.greeting_text == expected,
        "expected {expected:?} from the layered sources, got {:?}",
        config.greeting_text
    );
    Ok(())
}

/// A `Vec` appended across every layer under a per-field rename.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "SVA_")]
struct RenamedVec {
    #[serde(rename = "appended")]
    #[ortho_config(default = vec![String::from("def")], merge_strategy = "append")]
    extra_values: Vec<String>,
}

/// A `Vec` appended across every layer under a container rename rule.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "SVC_")]
#[serde(rename_all = "kebab-case")]
struct ContainerRenamedVec {
    #[ortho_config(default = vec![String::from("def")], merge_strategy = "append")]
    extra_values: Vec<String>,
}

trait HasValues {
    fn values(&self) -> &[String];
}

/// Compare a merged vector against the expected layer order.
fn ensure_values(actual: &[String], expected: &[&str], context: &str) -> Result<()> {
    let expected_values: Vec<String> = expected.iter().map(|value| (*value).to_owned()).collect();
    ensure!(
        actual == expected_values.as_slice(),
        "{context}: expected {expected_values:?}, got {actual:?}"
    );
    Ok(())
}

impl HasValues for RenamedVec {
    fn values(&self) -> &[String] {
        &self.extra_values
    }
}

impl HasValues for ContainerRenamedVec {
    fn values(&self) -> &[String] {
        &self.extra_values
    }
}

/// Load `T` after staging the same four layers for each renamed vector case.
fn load_renamed_vec<T: OrthoConfig + HasValues>(
    env_prefix: &str,
    env_key: &str,
    file_key: &str,
    cli_args: &[&str],
) -> Result<T> {
    let temp_dir = tempfile::tempdir()?;
    let path = write_config(temp_dir.path(), &format!("{file_key} = [\"file\"]\n"))?;
    let config_path = format!("{env_prefix}CONFIG_PATH");
    let env = layered_env(
        temp_dir.path(),
        &[
            (config_path.as_str(), path.to_string_lossy().as_ref()),
            (env_key, "[\"env\"]"),
        ],
    );
    load_config::<T>(cli_args, env)
}

/// A per-field rename still enters declarative append handling.
///
/// Without the configuration key in the merge strategy the renamed field falls
/// through to whole-array replacement, leaving only the highest layer's value.
#[rstest]
fn renamed_vec_appends_across_layers() -> Result<()> {
    let config = load_renamed_vec::<RenamedVec>(
        "SVA_",
        "SVA_APPENDED",
        "appended",
        &["prog", "--extra-values", "cli"],
    )?;
    ensure_values(
        config.values(),
        &["def", "file", "env", "cli"],
        "expected every layer to append under the renamed key",
    )
}

/// A container `rename_all` rule reaches append handling as well.
#[rstest]
fn container_renamed_vec_appends_across_layers() -> Result<()> {
    let config = load_renamed_vec::<ContainerRenamedVec>(
        "SVC_",
        "SVC_EXTRA-VALUES",
        "extra-values",
        &["prog", "--extra-values", "cli"],
    )?;
    ensure_values(
        config.values(),
        &["def", "file", "env", "cli"],
        "expected the kebab-case key to append across layers",
    )
}

/// A serialization-only rename must not move the merge key.
///
/// The final configuration type reads `actual_name`, so a layer keyed by the
/// serialization spelling would be ignored by the CLI and defaults rungs.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "SVS_")]
struct SerializeOnlyRename {
    #[serde(rename(serialize = "serialized-name"))]
    #[ortho_config(default = String::from("def"))]
    actual_name: String,
}

#[rstest]
fn serialization_only_rename_keeps_the_deserialization_key() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = write_config(temp_dir.path(), "actual_name = \"file\"\n")?;
    let env = layered_env(
        temp_dir.path(),
        &[
            ("SVS_CONFIG_PATH", path.to_string_lossy().as_ref()),
            ("SVS_ACTUAL_NAME", "env"),
        ],
    );
    let config = load_config::<SerializeOnlyRename>(&["prog", "--actual-name", "cli"], env)?;
    ensure!(
        config.actual_name == "cli",
        "expected the CLI to layer under `actual_name`, got {:?}",
        config.actual_name
    );
    Ok(())
}

/// A renamed boolean whose clap default must be pruned from the CLI layer.
///
/// The sibling `--note` value forces the CLI layer to be pushed, so an
/// unpruned default on the boolean would mask the file value. The string
/// `default_value` form is replayed onto the generated argument, which is the
/// shape the pruning exists for.
#[derive(Debug, Deserialize, Serialize, clap::Parser, OrthoConfig)]
#[command(name = "renamed-bool")]
#[ortho_config(prefix = "SVB_")]
struct RenamedBool {
    #[serde(rename = "is-enabled")]
    #[arg(long = "enabled", default_value = "false")]
    #[ortho_config(cli_long = "enabled", cli_default_as_absent)]
    flag_state: bool,
    #[arg(long)]
    note: Option<String>,
}

#[rstest]
#[case::file_survives_pruned_default(&["prog", "--note", "x"], true)]
#[case::explicit_false_wins(&["prog", "--note", "x", "--enabled=false"], false)]
fn renamed_bool_default_does_not_mask_lower_layers(
    #[case] cli_args: &[&str],
    #[case] expected: bool,
) -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = write_config(temp_dir.path(), "is-enabled = true\n")?;
    let env = layered_env(
        temp_dir.path(),
        &[("SVB_CONFIG_PATH", path.to_string_lossy().as_ref())],
    );
    let config = load_config::<RenamedBool>(cli_args, env)?;
    ensure!(
        config.flag_state == expected,
        "expected flag_state to be {expected}, got {}",
        config.flag_state
    );
    Ok(())
}
