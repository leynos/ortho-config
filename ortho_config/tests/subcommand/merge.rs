//! Tests for merge helpers in subcommand flows.

use anyhow::{Context as _, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory as _, FromArgMatches as _, Parser};
use ortho_config::subcommand::Prefix;
use ortho_config::{
    MapEnv, OrthoConfig, SubcommandCliMatches, SubcommandFileContext, SupplyRoute,
    load_and_merge_subcommand_for_with_matches_with_sources_at,
    load_and_merge_subcommand_for_with_sources_at,
    load_and_merge_subcommand_with_matches_with_sources_at,
    load_and_merge_subcommand_with_sources_at,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::fixtures::{IsolatedRoot, isolated_root};

#[derive(Debug, Deserialize, Serialize, Default, PartialEq, Parser)]
#[command(name = "test")]
struct MergeArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bar: Option<bool>,
}

/// Tests that merging CLI arguments with configuration file values prioritizes CLI values and preserves unset fields.
///
/// This test creates a configuration file with a default value for `foo`, then merges it with CLI arguments that override `foo` and leave `bar` unset. It asserts that the merged configuration uses the CLI value for `foo` and retains `None` for `bar`.
///
/// # Examples
///
/// ```
/// merge_helper_combines_defaults_and_cli();
/// ```
#[rstest]
fn merge_helper_combines_defaults_and_cli(isolated_root: Result<IsolatedRoot>) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let directory = Dir::open_ambient_dir(isolated.path(), ambient_authority())
        .context("open merge helper fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nfoo = \"file\"")
        .context("write merge helper fixture")?;
    let cli = MergeArgs {
        foo: Some("cli".into()),
        bar: None,
    };
    let merged = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &cli,
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge explicit CLI and file defaults")?;
    ensure!(
        merged.foo.as_deref() == Some("cli"),
        "expected cli, got {:?}",
        merged.foo
    );
    ensure!(merged.bar.is_none(), "expected None, got {:?}", merged.bar);
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default, PartialEq, Parser)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
struct MergePrefixed {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
}

/// Verifies that `MergePrefixed` respects the configuration prefix and
/// prefers file values when the CLI field is unset.
#[rstest]
fn merge_wrapper_respects_prefix(isolated_root: Result<IsolatedRoot>) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let directory = Dir::open_ambient_dir(isolated.path(), ambient_authority())
        .context("open prefixed merge fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nfoo = \"file\"")
        .context("write prefixed merge fixture")?;
    let cli = MergePrefixed { foo: None };
    let merged = load_and_merge_subcommand_for_with_sources_at(
        &cli,
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge prefixed explicit file defaults")?;
    ensure!(
        merged.foo.as_deref() == Some("file"),
        "expected file, got {:?}",
        merged.foo
    );
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default, PartialEq, Parser)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
struct MergeNonFinite {
    #[serde(skip_serializing_if = "Option::is_none")]
    ratio: Option<f64>,
}

/// A configured non-finite float is data, not absence.
///
/// The requirement enumeration projects the merged figment into JSON, which
/// cannot represent `nan` or `inf`, so the projection sees `null`. Reading the
/// final value back out of that projection would report `None` for a value the
/// user really configured, and would fail outright for a plain `f64`. This
/// case therefore pins the extraction to the original figment.
#[rstest]
fn a_non_finite_float_survives_the_schema_aware_subcommand_merge(
    isolated_root: Result<IsolatedRoot>,
) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let directory = Dir::open_ambient_dir(isolated.path(), ambient_authority())
        .context("open non-finite merge fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nratio = nan")
        .context("write non-finite merge fixture")?;
    let merged = load_and_merge_subcommand_for_with_sources_at(
        &MergeNonFinite::default(),
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("merge a configured non-finite value")?;
    ensure!(
        matches!(merged.ratio, Some(ratio) if ratio.is_nan()),
        "a configured nan must survive the merge, got {:?}",
        merged.ratio
    );
    Ok(())
}

mod wide_u128 {
    //! A `u128` field deserializer for values JSON and Figment would reject.
    //!
    //! Figment exposes a wide number only through `deserialize_any`; a derived
    //! `u128` field asks for `deserialize_u128`, which Figment does not
    //! implement. A type in this test therefore needs its own visitor.

    use serde::{Deserializer, de::Visitor};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct WideVisitor;

        impl Visitor<'_> for WideVisitor {
            type Value = u128;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an unsigned integer")
            }

            fn visit_u64<E>(self, value: u64) -> Result<u128, E> {
                Ok(u128::from(value))
            }

            fn visit_u128<E>(self, value: u128) -> Result<u128, E> {
                Ok(value)
            }
        }

        deserializer.deserialize_any(WideVisitor)
    }
}

/// A subcommand whose `Default` holds a `u128` above `u64::MAX`.
///
/// `wide` is marked `cli_default_as_absent`, so clap's own default does not
/// reach the CLI layer unless the user names the flag. The merged figment then
/// carries the type's `Default` verbatim: the one shape where it holds a value
/// JSON cannot represent even though the raw loader accepts it.
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Parser, PartialEq)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
struct MergeWideDefault {
    #[serde(deserialize_with = "wide_u128::deserialize")]
    #[arg(long, default_value_t = 0)]
    #[ortho_config(cli_default_as_absent)]
    wide: u128,
}

impl Default for MergeWideDefault {
    fn default() -> Self {
        Self { wide: u128::MAX }
    }
}

/// A wide default is data the requirement enumeration must not reject.
///
/// The matches-aware figment carries `T::default()` verbatim, so a `u128`
/// above `u64::MAX` reaches the merge even though no CLI, file, or environment
/// route can carry one. Projecting that figment into JSON fails, while the raw
/// extraction accepts it through the field's own deserializer; the schema-aware
/// loader must not turn a loadable configuration into an error.
#[rstest]
fn a_wide_default_survives_the_schema_aware_matches_merge(
    isolated_root: Result<IsolatedRoot>,
) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let matches = MergeWideDefault::command().get_matches_from(["test"]);
    let cli = MergeWideDefault::from_arg_matches(&matches).context("parse wide-default CLI")?;
    ensure!(
        cli.wide == 0,
        "clap must leave the absent flag at its declared default, got {}",
        cli.wide
    );
    let cli_matches = SubcommandCliMatches::new(&cli, &matches);

    let raw = load_and_merge_subcommand_with_matches_with_sources_at(
        &Prefix::new("APP_"),
        &cli_matches,
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("raw matches merge of a wide default")?;
    ensure!(
        raw.wide == u128::MAX,
        "the raw loader must accept the wide default, got {}",
        raw.wide
    );

    let merged = load_and_merge_subcommand_for_with_matches_with_sources_at(
        &cli_matches,
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("schema-aware matches merge of a wide default")?;
    ensure!(
        merged.wide == u128::MAX,
        "the schema-aware loader must accept the wide default, got {}",
        merged.wide
    );
    Ok(())
}

/// A subcommand whose declared default only its own root loader materializes.
///
/// `reference` declares a default, which the generated defaults struct supplies
/// when the type is loaded as the root. The selected-subcommand loaders build
/// their figment from file, environment, and CLI layers instead, and the
/// serialization skips the empty value the parser leaves behind, so the key is
/// genuinely absent from the document they merge.
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default, Parser, PartialEq)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
struct MergeDeclaredDefault {
    /// Absent from every scoped layer despite its declared default.
    #[arg(skip)]
    #[ortho_config(skip_cli, default = String::from("fallback"))]
    #[serde(skip_serializing_if = "String::is_empty")]
    reference: String,
    /// Needed by the matches-aware entry point, and present through clap's
    /// default, so it must never appear in the report.
    #[arg(long, default_value = "2")]
    #[ortho_config(default = 2, cli_default_as_absent)]
    retries: u32,
}

/// The variable the subcommand's environment layer reads for `reference`.
const DECLARED_REFERENCE_VAR: &str = "APP_CMDS_TEST_REFERENCE";

/// Assert a scoped load reported `reference`, and nothing else, as absent.
fn assert_reports_the_absent_declared_default<T>(
    result: ortho_config::OrthoResult<T>,
) -> Result<()> {
    let error = result
        .err()
        .ok_or_else(|| anyhow!("a declared default is not a scoped supply layer"))?;
    let ortho_config::OrthoError::MissingRequiredValues(report) = error.as_ref() else {
        return Err(anyhow!("expected an enumerated report, got: {error}"));
    };
    let rendered: Vec<String> = report
        .items()
        .iter()
        .map(|item| item.path().render())
        .collect();
    ensure!(
        rendered == ["reference"],
        "the report must name the absent declared default only, got {rendered:?}"
    );
    let item = report
        .items()
        .first()
        .ok_or_else(|| anyhow!("the report is empty"))?;
    ensure!(
        item.routes().iter().any(|route| matches!(
            route,
            SupplyRoute::Environment { variable } if variable == DECLARED_REFERENCE_VAR
        )),
        "the report must offer {DECLARED_REFERENCE_VAR}, got {:?}",
        item.routes()
    );
    ensure!(
        item.routes().iter().any(|route| matches!(
            route,
            SupplyRoute::ConfigurationFile {
                expects_object: false,
                ..
            }
        )),
        "the report must offer a scalar configuration-file route, got {:?}",
        item.routes()
    );
    Ok(())
}

/// A declared default is a requirement of the scoped loaders, not a supply.
#[rstest]
fn a_declared_default_is_not_a_scoped_subcommand_supply(
    isolated_root: Result<IsolatedRoot>,
) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    assert_reports_the_absent_declared_default(load_and_merge_subcommand_for_with_sources_at(
        &MergeDeclaredDefault::default(),
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    ))?;

    let matches = MergeDeclaredDefault::command().get_matches_from(["test"]);
    let cli = MergeDeclaredDefault::from_arg_matches(&matches).context("parse declared CLI")?;
    assert_reports_the_absent_declared_default(
        load_and_merge_subcommand_for_with_matches_with_sources_at(
            &SubcommandCliMatches::new(&cli, &matches),
            SubcommandFileContext::new(isolated.path(), &discovery),
            Arc::new(MapEnv::new()),
        ),
    )
}

/// Every advertised route repairs the load the report described.
#[rstest]
fn the_absent_declared_default_recovers_from_its_file_route(
    isolated_root: Result<IsolatedRoot>,
) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let directory = Dir::open_ambient_dir(isolated.path(), ambient_authority())
        .context("open declared-default fixture directory")?;
    directory
        .write(".app.toml", b"[cmds.test]\nreference = \"supplied\"")
        .context("write declared-default fixture")?;
    let merged = load_and_merge_subcommand_for_with_sources_at(
        &MergeDeclaredDefault::default(),
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .context("a supplied declared default must load")?;
    ensure!(
        merged.reference == "supplied",
        "the file route must supply the value, got {:?}",
        merged.reference
    );
    Ok(())
}
