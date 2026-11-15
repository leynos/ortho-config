//! Steps for the combined one-diagnostic recovery scenarios.
//!
//! These scenarios carry the plan's end-to-end contract. One load must name a
//! root requirement and both nested requirements together, every advertised
//! route must be one the real loader reads, and a mixed round of corrections
//! must then succeed without a second diagnostic. Each load injects its
//! sources, so the process environment is never mutated.
//!
//! The `When` wording departs from the plan in one respect: step texts are
//! registered globally, and [`super::env_steps`] already owns "the
//! configuration is loaded", so each scenario here names its own load.

use anyhow::{Result, anyhow, bail, ensure};
use ortho_config::{
    ConfigPath, MapEnv, MissingRequiredValues, OrthoConfig, OrthoError, RequiredConfigSchema,
    SchemaChildren, SchemaField, SupplyRoute,
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use serde::{Deserialize, Serialize};

use super::common::{config_fixture, shared_sources};

mod scenarios;

/// A nested object whose declared leaves are both required.
#[derive(Debug, Deserialize, Serialize, RequiredConfigSchema)]
struct EndpointSettings {
    /// Required leaf, reachable from the environment and the file.
    host: String,
    /// Required numeric leaf, which proves a non-string route works.
    port: u16,
}

/// One root requirement beside a required nested object.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRQC_")]
struct CombinedConfig {
    /// Required scalar, so the parser offers a flag for it.
    reference: String,
    /// Required object: no flag can carry a whole table.
    #[ortho_config(nested, skip_cli)]
    database: EndpointSettings,
}

/// A configuration whose nested object is optional.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRQD_")]
struct OptionalParentConfig {
    /// Required scalar, supplied by the scenario.
    reference: String,
    /// Optional subtree: an absent object deactivates its children.
    #[ortho_config(nested, skip_cli)]
    database: Option<EndpointSettings>,
}

/// The values a successful load produces, reduced to what the scenarios assert.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LoadedValues {
    /// The root scalar the load resolved.
    reference: String,
    /// The nested leaves, when the object was present.
    database: Option<(String, u16)>,
}

/// The observable outcome of one scenario load.
#[derive(Clone, Debug)]
enum LoadOutcome {
    /// The load succeeded with the values under test.
    Succeeded(LoadedValues),
    /// The load failed with enumerated missing values.
    Missing(MissingRequiredValues),
    /// The load failed with an error that is not absence.
    Invalid(String),
}

/// Scenario state for the combined recovery scenarios.
#[derive(Default, ScenarioState)]
pub struct RecoveryContext {
    /// Contents the combined load reads from its configuration file.
    file_contents: Slot<String>,
    /// Outcome of the combined load.
    combined_result: Slot<LoadOutcome>,
    /// Outcome of the optional-parent load.
    optional_result: Slot<LoadOutcome>,
    /// Outcome of the scalar-parent load.
    scalar_result: Slot<LoadOutcome>,
    /// Every route the combined diagnostic advertised, keyed by its path.
    routes: Slot<Vec<(ConfigPath, SupplyRoute)>>,
}

/// Provides a clean context for the combined recovery scenarios.
#[fixture]
pub fn recovery_context() -> RecoveryContext {
    RecoveryContext::default()
}

/// Convert a load result into the scenario's observable outcome.
fn outcome_of<T>(
    result: ortho_config::OrthoResult<T>,
    values: impl FnOnce(T) -> LoadedValues,
) -> LoadOutcome {
    match result {
        Ok(config) => LoadOutcome::Succeeded(values(config)),
        Err(error) => match error.as_ref() {
            OrthoError::MissingRequiredValues(report) => LoadOutcome::Missing((**report).clone()),
            other => LoadOutcome::Invalid(other.to_string()),
        },
    }
}

/// Load `T` from one fixture file plus explicit arguments and environment.
///
/// The fixture directory is dropped once the load has read the file, so every
/// call starts from the same contents and no earlier load can leak values into
/// a later one.
fn load_config<T>(
    file_name: &str,
    contents: &str,
    config_path_variable: &str,
    args: &[String],
    assignments: &[(String, String)],
) -> Result<ortho_config::OrthoResult<T>>
where
    T: OrthoConfig,
{
    let (_directory, path) = config_fixture(file_name, Some(contents))?;
    let mut environment = MapEnv::new().with_var(config_path_variable, path);
    for (key, value) in assignments {
        environment = environment.with_var(key.clone(), value.clone());
    }
    let (discovery, merge) = shared_sources(environment);
    let mut command_line = vec![String::from("consumer")];
    command_line.extend_from_slice(args);
    Ok(T::load_from_iter_with_sources(
        command_line,
        discovery,
        merge,
    ))
}

/// Load [`CombinedConfig`] under the given layers and reduce it to its outcome.
fn load_combined(
    contents: &str,
    args: &[String],
    assignments: &[(String, String)],
) -> Result<LoadOutcome> {
    let result = load_config::<CombinedConfig>(
        "mrqc.toml",
        contents,
        "MRQC_CONFIG_PATH",
        args,
        assignments,
    )?;
    Ok(outcome_of(result, |config| LoadedValues {
        reference: config.reference,
        database: Some((config.database.host, config.database.port)),
    }))
}

/// Render every path a composed schema declares, in declaration order.
fn schema_paths(fields: &[SchemaField]) -> Vec<String> {
    fn walk(prefix: &[String], fields: &[SchemaField], found: &mut Vec<String>) {
        for field in fields {
            let mut segments = prefix.to_vec();
            segments.push(field.key.clone());
            found.push(ConfigPath::new(segments.iter().cloned()).render());
            if let SchemaChildren::Nested(children) = &field.children {
                walk(&segments, children, found);
            }
        }
    }
    let mut found = Vec::new();
    walk(&[], fields, &mut found);
    found
}

/// Build a one-key TOML document that supplies `path`.
///
/// The fixture's port is numeric, so its route needs a bare integer while the
/// string-valued routes take a quoted value. The key and header are rendered
/// through [`ConfigPath`] so they keep the same quoting rules the diagnostic
/// uses.
fn file_snippet(path: &ConfigPath, value: &str) -> String {
    let Some((last, parent)) = path.segments().split_last() else {
        return String::new();
    };
    let key = ConfigPath::new([last.to_owned()]).render();
    let assignment = if last == "port" {
        format!("{key} = {value}")
    } else {
        format!("{key} = \"{value}\"")
    };
    let header = ConfigPath::new(parent.iter().cloned()).render();
    if header.is_empty() {
        format!("{assignment}\n")
    } else {
        format!("[{header}]\n{assignment}\n")
    }
}

/// The value a scenario supplies for `path` when it exercises a route.
fn value_for(path: &ConfigPath) -> &'static str {
    if path.last() == Some("port") {
        "8080"
    } else {
        "supplied"
    }
}

/// Load the combined configuration through `route` alone.
///
/// The returned report is the load's own enumeration, so a caller can prove
/// that `path` is no longer among the missing values when it is absent from it.
fn apply_route(path: &ConfigPath, route: &SupplyRoute) -> Result<Option<MissingRequiredValues>> {
    let value = value_for(path);
    let (contents, args, assignments) = match route {
        SupplyRoute::CommandLine { flag, .. } => (
            String::new(),
            vec![flag.clone(), String::from(value)],
            Vec::new(),
        ),
        SupplyRoute::Environment { variable } => (
            String::new(),
            Vec::new(),
            vec![(variable.clone(), String::from(value))],
        ),
        SupplyRoute::ConfigurationFile {
            path: file_path,
            expects_object: false,
        } => (file_snippet(file_path, value), Vec::new(), Vec::new()),
        other => bail!("the scenario cannot apply the route {other:?}"),
    };
    match load_combined(&contents, &args, &assignments)? {
        LoadOutcome::Missing(report) => Ok(Some(report)),
        LoadOutcome::Succeeded(_) => Ok(None),
        LoadOutcome::Invalid(message) => {
            bail!("the route {route:?} produced an invalid configuration: {message}")
        }
    }
}

/// Report whether `route` is of the kind named by `kind`.
fn is_route_kind(route: &SupplyRoute, kind: &str) -> bool {
    match kind {
        "command-line" => matches!(route, SupplyRoute::CommandLine { .. }),
        "environment" => matches!(route, SupplyRoute::Environment { .. }),
        "configuration-file" => matches!(
            route,
            SupplyRoute::ConfigurationFile {
                expects_object: false,
                ..
            }
        ),
        _ => false,
    }
}

/// Select the single reported route of `kind` for the rendered `path`.
fn route_of(routes: &[(ConfigPath, SupplyRoute)], path: &str, kind: &str) -> Result<SupplyRoute> {
    let mut matching = routes
        .iter()
        .filter(|(reported, _)| reported.render() == path)
        .map(|(_, route)| route)
        .filter(|route| is_route_kind(route, kind));
    let route = matching
        .next()
        .ok_or_else(|| anyhow!("`{path}` advertises no {kind} route"))?;
    ensure!(
        matching.next().is_none(),
        "`{path}` advertises more than one {kind} route"
    );
    Ok(route.clone())
}
