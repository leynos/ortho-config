//! Regression coverage for explicit CLI values that equal configured defaults.
//!
//! The generated loader must decide whether a field was supplied from clap's
//! value sources, not by comparing the serialized CLI object with the defaults
//! object. These cases pin the distinction for scalar, boolean, optional, and
//! collection fields on both the declarative and the profile-enabled compose
//! paths: an explicit value equal to the configured default still outranks a
//! file or environment value, while an omitted argument leaves the lower layer
//! in charge and must not add a CLI layer of its own.
//!
//! Each case selects `app.toml` through the prefixed configuration-path
//! variable rather than `--config-path`. A command-line selector would itself
//! count as explicit input and mask the per-field behaviour under test.

#[path = "support/hermetic_env.rs"]
mod hermetic_env;

use anyhow::{Context as _, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use hermetic_env::hermetic;
use ortho_config::declarative::LayerComposition;
use ortho_config::{
    EnvSource, MapEnv, MergeLayer, MergeProvenance, OrthoConfig, OrthoResult, ScanEnvSource,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Declarative configuration carrying one field of each relevant shape.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "EQD_")]
struct DeclarativeDefaults {
    /// Scalar whose configured default the explicit cases restate.
    #[ortho_config(default = 443)]
    port: u16,
    /// Boolean whose configured default the explicit cases restate.
    #[ortho_config(default = false)]
    verbose: bool,
    /// Optional value whose configured default the explicit cases restate.
    #[ortho_config(default = String::from("info"))]
    label: Option<String>,
    /// Collection replaced wholesale by an explicit command-line value.
    #[ortho_config(default = vec![String::from("alpha")], merge_strategy = "replace")]
    tags: Vec<String>,
}

/// Profile-enabled configuration with the same field shapes.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "EQP_", profiles)]
struct ProfiledDefaults {
    /// Scalar whose configured default the explicit cases restate.
    #[ortho_config(default = 443)]
    port: u16,
    /// Boolean whose configured default the explicit cases restate.
    #[ortho_config(default = false)]
    verbose: bool,
    /// Optional value whose configured default the explicit cases restate.
    #[ortho_config(default = String::from("info"))]
    label: Option<String>,
    /// Collection replaced wholesale by an explicit command-line value.
    #[ortho_config(default = vec![String::from("alpha")], merge_strategy = "replace")]
    tags: Vec<String>,
}

/// Values expected after a case has composed and merged its layers.
#[derive(Clone, Copy, Debug)]
struct Expected {
    /// Expected scalar value.
    port: u16,
    /// Expected boolean value.
    verbose: bool,
    /// Expected optional value.
    label: &'static str,
    /// Expected collection value.
    tags: &'static [&'static str],
}

/// The default used by both fixtures' collection fields.
const DEFAULT_TAGS: &[&str] = &["alpha"];

impl Default for Expected {
    /// The configured defaults shared by both fixtures.
    fn default() -> Self {
        Self {
            port: 443,
            verbose: false,
            label: "info",
            tags: DEFAULT_TAGS,
        }
    }
}

/// One row of the explicit-versus-absent matrix.
struct Case {
    /// TOML body written to `app.toml` for the file layer.
    file_body: &'static str,
    /// Environment entries with the fixture prefix omitted.
    env: &'static [(&'static str, &'static str)],
    /// Command-line arguments, including the program name.
    cli_args: &'static [&'static str],
    /// Values the merged configuration must hold.
    expected: Expected,
    /// Whether the composition must include a CLI layer.
    expect_cli_layer: bool,
}

/// A configuration type the cases can compose and inspect.
trait CaseConfig: OrthoConfig {
    /// Prefix shared by the fixture's environment variables.
    const PREFIX: &'static str;

    /// Compose the layers through the injected-environment entry point.
    fn compose<I, T, E>(iter: I, env: Arc<E>) -> LayerComposition
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
        E: EnvSource + ScanEnvSource + 'static;

    /// Merge the composed layers through the generated implementation.
    fn merge(layers: Vec<MergeLayer<'static>>) -> OrthoResult<Self>;

    /// Compare the merged configuration with the expected values.
    fn assert_values(&self, expected: &Expected) -> Result<()>;
}

impl CaseConfig for DeclarativeDefaults {
    const PREFIX: &'static str = "EQD_";

    fn compose<I, T, E>(iter: I, env: Arc<E>) -> LayerComposition
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
        E: EnvSource + ScanEnvSource + 'static,
    {
        Self::compose_layers_from_iter_with_env(iter, env)
    }

    fn merge(layers: Vec<MergeLayer<'static>>) -> OrthoResult<Self> {
        Self::merge_from_layers(layers)
    }

    fn assert_values(&self, expected: &Expected) -> Result<()> {
        assert_merged_values(expected, &Actual::from(self))
    }
}

impl CaseConfig for ProfiledDefaults {
    const PREFIX: &'static str = "EQP_";

    fn compose<I, T, E>(iter: I, env: Arc<E>) -> LayerComposition
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
        E: EnvSource + ScanEnvSource + 'static,
    {
        Self::compose_layers_from_iter_with_env(iter, env)
    }

    fn merge(layers: Vec<MergeLayer<'static>>) -> OrthoResult<Self> {
        Self::merge_from_layers(layers)
    }

    fn assert_values(&self, expected: &Expected) -> Result<()> {
        assert_merged_values(expected, &Actual::from(self))
    }
}

/// The values observed in a merged fixture.
struct Actual<'a> {
    /// Observed scalar value.
    port: u16,
    /// Observed boolean value.
    verbose: bool,
    /// Observed optional value.
    label: Option<&'a str>,
    /// Observed collection value.
    tags: &'a [String],
}

impl<'a> From<&'a DeclarativeDefaults> for Actual<'a> {
    fn from(config: &'a DeclarativeDefaults) -> Self {
        Self {
            port: config.port,
            verbose: config.verbose,
            label: config.label.as_deref(),
            tags: &config.tags,
        }
    }
}

impl<'a> From<&'a ProfiledDefaults> for Actual<'a> {
    fn from(config: &'a ProfiledDefaults) -> Self {
        Self {
            port: config.port,
            verbose: config.verbose,
            label: config.label.as_deref(),
            tags: &config.tags,
        }
    }
}

/// Compare every merged field with `expected`, naming the first mismatch.
fn assert_merged_values(expected: &Expected, actual: &Actual<'_>) -> Result<()> {
    ensure!(
        actual.port == expected.port,
        "expected port {}, got {}",
        expected.port,
        actual.port
    );
    ensure!(
        actual.verbose == expected.verbose,
        "expected verbose {}, got {}",
        expected.verbose,
        actual.verbose
    );
    ensure!(
        actual.label == Some(expected.label),
        "expected label {:?}, got {:?}",
        expected.label,
        actual.label
    );
    let expected_tags: Vec<&str> = expected.tags.to_vec();
    let actual_tags: Vec<&str> = actual.tags.iter().map(String::as_str).collect();
    ensure!(
        actual_tags == expected_tags,
        "expected tags {expected_tags:?}, got {actual_tags:?}"
    );
    Ok(())
}

/// Write `body` to `app.toml` beneath `dir`, returning its UTF-8 path.
///
/// The capability handle names the directory it may touch, and the returned
/// path is `camino`'s UTF-8 type so a non-UTF-8 fixture root is reported rather
/// than silently carried into the environment map.
fn write_config(dir: &Utf8Path, body: &str) -> Result<Utf8PathBuf> {
    let cap = Dir::open_ambient_dir(dir, ambient_authority()).context("open fixture dir")?;
    cap.write("app.toml", body.as_bytes())
        .context("write app.toml")?;
    Ok(dir.join("app.toml"))
}

/// Compose one case's layers, assert the CLI-layer presence, and merge them.
fn run_case<T: CaseConfig>(case: &Case) -> Result<()> {
    let dir = tempfile::tempdir().context("create fixture dir")?;
    let root = Utf8Path::from_path(dir.path())
        .ok_or_else(|| anyhow!("temporary fixture path is not UTF-8"))?
        .to_owned();
    let config_path = write_config(&root, case.file_body)?;

    let mut environment = MapEnv::new()
        .with_var("HOME", root.join("home-empty").as_std_path())
        .with_var(
            format!("{}CONFIG_PATH", T::PREFIX),
            config_path.as_std_path(),
        );
    for (key, value) in case.env {
        environment.insert(format!("{}{key}", T::PREFIX), value);
    }
    let env = hermetic(environment, root.as_std_path());

    let composition = T::compose(case.cli_args.iter().copied(), Arc::new(env));
    let (layers, errors) = composition.into_parts();
    ensure!(errors.is_empty(), "composition errors: {errors:?}");

    let has_cli_layer = layers
        .iter()
        .any(|layer| layer.provenance() == MergeProvenance::Cli);
    ensure!(
        has_cli_layer == case.expect_cli_layer,
        "expected CLI layer present = {}, got layer stack {:?}",
        case.expect_cli_layer,
        layer_provenances(&layers)
    );

    let merged = T::merge(layers)
        .map_err(|err| anyhow!(err))
        .context("merge the composed layers")?;
    merged.assert_values(&case.expected)
}

/// The provenance labels of a layer stack, for failure diagnostics.
fn layer_provenances(layers: &[MergeLayer<'static>]) -> Vec<MergeProvenance> {
    layers.iter().map(MergeLayer::provenance).collect()
}

/// A scalar explicitly restated as its configured default outranks the
/// environment, while an omitted scalar leaves the environment in charge.
#[rstest]
#[case::explicit_scalar_restates_default(Case {
    file_body: "port = 8443\n",
    env: &[("PORT", "9443")],
    cli_args: &["prog", "--port", "443"],
    expected: Expected { port: 443, ..Expected::default() },
    expect_cli_layer: true,
})]
#[case::absent_scalar_defers_to_environment(Case {
    file_body: "port = 8443\n",
    env: &[("PORT", "9443")],
    cli_args: &["prog"],
    expected: Expected { port: 9443, ..Expected::default() },
    expect_cli_layer: false,
})]
#[case::explicit_bool_restates_default(Case {
    file_body: "verbose = true\n",
    env: &[("VERBOSE", "true")],
    cli_args: &["prog", "--verbose=false"],
    expected: Expected { verbose: false, ..Expected::default() },
    expect_cli_layer: true,
})]
#[case::absent_bool_defers_to_environment(Case {
    file_body: "verbose = true\n",
    env: &[("VERBOSE", "true")],
    cli_args: &["prog"],
    expected: Expected { verbose: true, ..Expected::default() },
    expect_cli_layer: false,
})]
#[case::explicit_option_restates_default(Case {
    file_body: "label = \"file\"\n",
    env: &[("LABEL", "env")],
    cli_args: &["prog", "--label", "info"],
    expected: Expected { label: "info", ..Expected::default() },
    expect_cli_layer: true,
})]
#[case::absent_option_defers_to_environment(Case {
    file_body: "label = \"file\"\n",
    env: &[("LABEL", "env")],
    cli_args: &["prog"],
    expected: Expected { label: "env", ..Expected::default() },
    expect_cli_layer: false,
})]
#[case::explicit_collection_restates_default(Case {
    file_body: "tags = [\"file\"]\n",
    env: &[("TAGS", "[\"env\"]")],
    cli_args: &["prog", "--tags", "alpha"],
    expected: Expected { tags: &["alpha"], ..Expected::default() },
    expect_cli_layer: true,
})]
#[case::absent_collection_defers_to_environment(Case {
    file_body: "tags = [\"file\"]\n",
    env: &[("TAGS", "[\"env\"]")],
    cli_args: &["prog"],
    expected: Expected { tags: &["env"], ..Expected::default() },
    expect_cli_layer: false,
})]
fn explicit_values_equal_to_defaults_preserve_the_cli_layer(
    #[case] case: Case,
    #[values(FixturePath::Declarative, FixturePath::Profiled)] path: FixturePath,
) -> Result<()> {
    match path {
        FixturePath::Declarative => run_case::<DeclarativeDefaults>(&case),
        FixturePath::Profiled => run_case::<ProfiledDefaults>(&case),
    }
}

/// Which generated compose path a case exercises.
#[derive(Clone, Copy, Debug)]
enum FixturePath {
    /// The compatibility path without profile support.
    Declarative,
    /// The profile-enabled path, whose CLI layer previously compared whole
    /// objects before pushing.
    Profiled,
}
