//! A subcommand whose only requirement cannot arrive from the command line.
//!
//! `skip_cli` drops the generated flag and serialization omits the empty
//! default, so neither the CLI nor the defaults layer carries a value for
//! `reference`. That leaves an absent `reference` a genuine requirement of the
//! merged value, which the loader must enumerate instead of leaving to the
//! deserializer. Scenarios that assert that report share this fixture.

use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// The variable the subcommand's environment layer reads for `reference`.
pub const STRICT_REFERENCE_VAR: &str = "CWDSEAM_CMDS_STRICT_REFERENCE";

/// Subcommand whose only requirement cannot arrive from the command line.
///
/// `retries` is needed because the matches-aware entry point requires the
/// generated `CliValueExtractor`; it doubles as a control, since a value its
/// declared default supplies is never reported.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, Default, PartialEq)]
#[command(name = "strict")]
#[ortho_config(prefix = "CWDSEAM_")]
pub struct StrictCmd {
    /// Required, and reachable only through a file or the environment.
    #[arg(skip)]
    #[ortho_config(skip_cli)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reference: String,
    /// Optional through its declared default.
    #[arg(long, default_value = "2")]
    #[ortho_config(default = 2, cli_default_as_absent)]
    pub retries: u32,
}

/// Assert a load failed, and only because `reference` is absent.
///
/// The environment route must be offered as well: a report that named the
/// value without naming a truthful route would be no more actionable than the
/// deserializer error it replaces. A `skip_cli` field must advertise no
/// command-line route, and it must still offer a scalar file route.
pub fn assert_reports_the_absent_reference<T>(result: ortho_config::OrthoResult<T>) -> Result<()> {
    let error = result
        .err()
        .ok_or_else(|| anyhow!("an absent required value must fail the load"))?;
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
        "the report must name only the absent value, got {rendered:?}"
    );

    let item = report
        .items()
        .first()
        .ok_or_else(|| anyhow!("the report is empty"))?;
    let offers_environment = item.routes().iter().any(|route| {
        matches!(
            route,
            ortho_config::SupplyRoute::Environment { variable }
                if variable == STRICT_REFERENCE_VAR
        )
    });
    ensure!(
        offers_environment,
        "the report must offer {STRICT_REFERENCE_VAR}, got {:?}",
        item.routes()
    );
    ensure!(
        !item
            .routes()
            .iter()
            .any(|route| matches!(route, ortho_config::SupplyRoute::CommandLine { .. })),
        "a skip_cli field must not advertise a command-line route, got {:?}",
        item.routes()
    );
    ensure!(
        item.routes().iter().any(|route| matches!(
            route,
            ortho_config::SupplyRoute::ConfigurationFile {
                expects_object: false,
                ..
            }
        )),
        "the report must offer a scalar configuration-file route, got {:?}",
        item.routes()
    );
    Ok(())
}
