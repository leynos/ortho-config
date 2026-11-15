//! Behavioural steps that drive the combined recovery scenarios.
//!
//! Split from the module root to keep both files within the repository's
//! 400-line ceiling. The fixtures, the shared loader helpers, and the scenario
//! state live in the parent module; each step here asserts one observable
//! outcome of a single load.

use anyhow::{Result, anyhow, bail, ensure};
use ortho_config::{RequiredConfigSchema, RequiredSchemaStack, SupplyRoute};
use rstest_bdd_macros::{given, then, when};

use super::{
    CombinedConfig, FixtureFile, LoadOutcome, LoadedValues, OptionalParentConfig, RecoveryContext,
    RouteKind, apply_route, file_snippet, load_combined, load_config, outcome_of, route_of,
    schema_paths,
};

#[given("a schema-enabled configuration with reference, database.host and database.port required")]
fn schema_enabled_configuration(recovery_context: &RecoveryContext) -> Result<()> {
    let mut stack = RequiredSchemaStack::new();
    let fields = CombinedConfig::collect_required_schema(&mut stack)
        .map_err(|error| anyhow!("compose the combined schema: {error}"))?;
    let declared = schema_paths(&fields);
    ensure!(
        declared == ["reference", "database", "database.host", "database.port"],
        "the fixture must declare exactly the scenario's requirements: {declared:?}"
    );
    recovery_context.file_contents.set(String::new());
    Ok(())
}

#[given("no source supplies those values")]
fn no_source_supplies_them(recovery_context: &RecoveryContext) -> Result<()> {
    recovery_context.file_contents.set(String::new());
    Ok(())
}

#[when("the combined configuration is loaded")]
fn load_combined_configuration(recovery_context: &RecoveryContext) -> Result<()> {
    let contents = recovery_context.file_contents.get().unwrap_or_default();
    let outcome = load_combined(&contents, &[], &[])?;
    recovery_context.combined_result.set(outcome);
    Ok(())
}

#[then("one missing-values error names all three paths exactly once")]
fn one_error_names_three_paths(recovery_context: &RecoveryContext) -> Result<()> {
    let outcome = recovery_context
        .combined_result
        .get()
        .ok_or_else(|| anyhow!("the combined load was not recorded"))?;
    let LoadOutcome::Missing(report) = outcome else {
        bail!("expected enumerated missing values, got {outcome:?}");
    };
    let paths: Vec<String> = report
        .items()
        .iter()
        .map(|item| item.path().render())
        .collect();
    ensure!(
        paths == ["reference", "database.host", "database.port"],
        "every required path must appear exactly once, in order: {paths:?}"
    );
    let routes = report
        .items()
        .iter()
        .flat_map(|item| {
            let path = item.path().clone();
            item.routes()
                .iter()
                .map(move |route| (path.clone(), route.clone()))
        })
        .collect();
    recovery_context.routes.set(routes);
    Ok(())
}

#[then("every suggested route is accepted by its provider")]
fn every_route_is_accepted(recovery_context: &RecoveryContext) -> Result<()> {
    let routes = recovery_context
        .routes
        .get()
        .ok_or_else(|| anyhow!("no routes were recorded"))?;
    ensure!(!routes.is_empty(), "the diagnostic must advertise routes");
    for (path, route) in &routes {
        let remaining = apply_route(path, route)?;
        let still_missing = remaining
            .as_ref()
            .is_some_and(|report| report.items().iter().any(|item| item.path() == path));
        ensure!(
            !still_missing,
            "the route {route:?} must supply `{}`, but it is still reported: {remaining:?}",
            path.render()
        );
    }
    Ok(())
}

#[when("valid values are supplied using the reported routes")]
fn supply_using_reported_routes(recovery_context: &RecoveryContext) -> Result<()> {
    let routes = recovery_context
        .routes
        .get()
        .ok_or_else(|| anyhow!("no routes were recorded"))?;
    let cli = route_of(&routes, "reference", RouteKind::CommandLine)?;
    let environment = route_of(&routes, "database.host", RouteKind::Environment)?;
    let file = route_of(&routes, "database.port", RouteKind::ConfigurationFile)?;
    let (
        SupplyRoute::CommandLine { flag, .. },
        SupplyRoute::Environment { variable },
        SupplyRoute::ConfigurationFile {
            path: file_path, ..
        },
    ) = (cli, environment, file)
    else {
        bail!("the selected routes changed kind between assertions");
    };
    let args = vec![flag, String::from("cli reference")];
    let assignments = vec![(variable, String::from("database.example"))];
    let contents = file_snippet(&file_path, "8080");
    let outcome = load_combined(&contents, &args, &assignments)?;
    recovery_context.combined_result.set(outcome);
    Ok(())
}

#[then("the next load succeeds without another missing-values error")]
fn next_load_succeeds(recovery_context: &RecoveryContext) -> Result<()> {
    let outcome = recovery_context
        .combined_result
        .get()
        .ok_or_else(|| anyhow!("the recovery load was not recorded"))?;
    let LoadOutcome::Succeeded(values) = outcome else {
        bail!("expected a successful recovery load, got {outcome:?}");
    };
    ensure!(
        values.reference == "cli reference",
        "the reported flag must populate the root scalar, got {:?}",
        values.reference
    );
    ensure!(
        values.database == Some((String::from("database.example"), 8080)),
        "the nested routes must populate the object, got {:?}",
        values.database
    );
    Ok(())
}

#[given("an absent optional parent and a supplied required value")]
fn absent_optional_parent(recovery_context: &RecoveryContext) -> Result<()> {
    recovery_context.file_contents.set(String::new());
    Ok(())
}

#[when("the configuration with an optional parent is loaded")]
fn load_optional_parent(recovery_context: &RecoveryContext) -> Result<()> {
    let contents = recovery_context.file_contents.get().unwrap_or_default();
    let args = vec![String::from("--reference"), String::from("supplied")];
    let result = load_config::<OptionalParentConfig>(
        FixtureFile::new("mrqd.toml", &contents, "MRQD_CONFIG_PATH"),
        &args,
        &[],
    )?;
    let outcome = outcome_of(result, |config| LoadedValues {
        reference: config.reference,
        database: config.database.map(|db| (db.host, db.port)),
    });
    recovery_context.optional_result.set(outcome);
    Ok(())
}

#[then("neither value nor the optional parent's children are reported")]
fn neither_value_nor_children(recovery_context: &RecoveryContext) -> Result<()> {
    let outcome = recovery_context
        .optional_result
        .get()
        .ok_or_else(|| anyhow!("the optional-parent load was not recorded"))?;
    let LoadOutcome::Succeeded(values) = outcome else {
        bail!("an absent optional parent must not be reported, got {outcome:?}");
    };
    ensure!(
        values.reference == "supplied",
        "the supplied root value must be retained, got {:?}",
        values.reference
    );
    ensure!(
        values.database.is_none(),
        "an absent optional parent must not report its children, got {:?}",
        values.database
    );
    Ok(())
}

#[given("a scalar where a nested configuration object is required")]
fn scalar_where_object_required(recovery_context: &RecoveryContext) -> Result<()> {
    recovery_context
        .file_contents
        .set(String::from("database = \"not-an-object\"\n"));
    Ok(())
}

#[when("the configuration with a scalar parent is loaded")]
fn load_scalar_parent(recovery_context: &RecoveryContext) -> Result<()> {
    let contents = recovery_context.file_contents.get().unwrap_or_default();
    let args = vec![String::from("--reference"), String::from("supplied")];
    let outcome = load_combined(&contents, &args, &[])?;
    recovery_context.scalar_result.set(outcome);
    Ok(())
}

#[then("the existing invalid-value error is retained")]
fn invalid_value_error_retained(recovery_context: &RecoveryContext) -> Result<()> {
    let outcome = recovery_context
        .scalar_result
        .get()
        .ok_or_else(|| anyhow!("the scalar-parent load was not recorded"))?;
    let LoadOutcome::Invalid(message) = outcome else {
        bail!("an invalid nested value must not be reported as absence, got {outcome:?}");
    };
    ensure!(
        !message.is_empty(),
        "the invalid-value error must carry a diagnostic"
    );
    Ok(())
}
