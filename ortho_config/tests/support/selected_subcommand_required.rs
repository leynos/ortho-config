//! Required-value enumeration through the selected-subcommand merge.
//!
//! A selected subcommand load reads `PREFIX_CMDS_<NAME>_*` variables and the
//! `[cmds.<name>]` file section, not the struct's own prefix and file root.
//! These cases therefore check two things at once: that the derived merge
//! enumerates the selected variant's requirements, and that every advertised
//! route is one the loader really reads. Driving the real merge is what makes
//! the route spelling falsifiable — an unscoped report would name
//! `APP_REFERENCE` and `reference`, which the loader never consults.

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use clap::{CommandFactory, FromArgMatches};
use ortho_config::{
    MapEnv, MissingRequiredValues, OrthoError, SelectedSubcommandMerge,
    SelectedSubcommandMergeError, SubcommandFileContext, SupplyRoute,
};
use rstest::rstest;
use std::sync::Arc;

use super::{Cli, Commands, selected_file};

/// Render one supply route as a compact label.
fn render_route(route: &SupplyRoute) -> String {
    match route {
        SupplyRoute::CommandLine { flag, .. } => format!("cli:{flag}"),
        SupplyRoute::Environment { variable } => format!("env:{variable}"),
        SupplyRoute::ConfigurationFile { path, .. } => format!("file:{}", path.render()),
        _ => String::from("other"),
    }
}

/// Merge the selected command against one file fixture and one environment.
fn merge_selected(
    command: &str,
    contents: &str,
    environment: MapEnv,
) -> Result<Result<Commands, SelectedSubcommandMergeError>> {
    let root = selected_file(contents)?;
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    let files = SubcommandFileContext::new(root.path(), &discovery);
    let matches = Cli::command()
        .try_get_matches_from(["prog", command])
        .with_context(|| format!("parse {command} arguments"))?;
    let cli = Cli::from_arg_matches(&matches).context("decode selected arguments")?;
    Ok(cli
        .cmd
        .load_and_merge_selected_with_sources(&matches, files, Arc::new(environment)))
}

/// Extract the enumerated report from a selected merge failure.
fn missing_report(error: SelectedSubcommandMergeError) -> Result<MissingRequiredValues> {
    let SelectedSubcommandMergeError::Merge(source) = error else {
        bail!("expected a merge failure, got {error:?}");
    };
    match source.as_ref() {
        OrthoError::MissingRequiredValues(report) => Ok((**report).clone()),
        other => Err(anyhow!("expected enumerated missing values, got {other:?}")),
    }
}

/// Report the selected command's requirement with loader-true routes.
#[rstest]
fn the_selected_command_reports_its_own_requirements() -> Result<()> {
    let merged = merge_selected("serve", "# no command defaults\n", MapEnv::new())?;
    let error = merged.expect_err("no layer supplies serve's requirement");
    let report = missing_report(error)?;

    ensure!(
        report.len() == 1,
        "only serve's single requirement is absent: {report}"
    );
    let item = report.items().first().expect("the report is non-empty");
    ensure!(
        item.path().render() == "reference",
        "the report must name the declared key: {item:?}"
    );
    let routes: Vec<String> = item.routes().iter().map(render_route).collect();
    ensure!(
        routes == ["env:APP_CMDS_SERVE_REFERENCE", "file:cmds.serve.reference"],
        "routes must name the subcommand namespaces the loader reads: {routes:?}"
    );
    Ok(())
}

/// Merge `serve` against one supplying layer and read back its reference.
///
/// A failure to select the variant is an error here, because a load that
/// produced another command would not have read the layer under test at all.
fn recovered_reference(contents: &str, environment: MapEnv) -> Result<String> {
    let merged = merge_selected("serve", contents, environment)?;
    let Commands::Serve(config) = merged.context("the advertised source must be read")? else {
        bail!("expected serve command");
    };
    Ok(config.reference)
}

/// Recover the missing value through the advertised environment route.
#[rstest]
fn the_environment_route_recovers_the_missing_value() -> Result<()> {
    let environment = MapEnv::new().with_var("APP_CMDS_SERVE_REFERENCE", "env reference");
    let reference = recovered_reference("# no command defaults\n", environment)?;
    ensure!(
        reference == "env reference",
        "the environment route must populate the field, got {reference:?}"
    );
    Ok(())
}

/// Recover the missing value through the advertised file section.
#[rstest]
fn the_file_route_recovers_the_missing_value() -> Result<()> {
    let reference = recovered_reference(
        "[cmds.serve]\nreference = \"file reference\"\n",
        MapEnv::new(),
    )?;
    ensure!(
        reference == "file reference",
        "the file route must populate the field, got {reference:?}"
    );
    Ok(())
}

/// A non-selected variant must never contribute another variant's requirements.
///
/// `serve` declares a requirement that no layer supplies, so this case fails
/// if the merge consults any variant other than the selected one.
#[rstest]
fn an_unselected_command_contributes_no_requirements() -> Result<()> {
    let merged = merge_selected("run", "# no command defaults\n", MapEnv::new())?;
    let Commands::Run(config) = merged.context("run declares no requirement")? else {
        bail!("expected run command");
    };
    ensure!(
        config.option.is_none(),
        "no layer supplied run's optional value, got {:?}",
        config.option
    );
    Ok(())
}
