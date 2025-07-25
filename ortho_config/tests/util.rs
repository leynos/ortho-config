//! Utilities for integration tests using `figment::Jail`.
//!
//! These helpers run setup code inside a jailed environment before
//! loading a subcommand configuration. They reduce boilerplate in
//! tests by encapsulating the jail creation and configuration loading.
#![expect(
    deprecated,
    reason = "figment's Jail uses deprecated APIs for test isolation"
)]

use clap::CommandFactory;
use ortho_config::subcommand::{CmdName, Prefix};
use ortho_config::{
    OrthoConfig, OrthoError, load_and_merge_subcommand, load_and_merge_subcommand_for,
    load_subcommand_config, load_subcommand_config_for,
};
use serde::de::DeserializeOwned;

#[expect(
    clippy::result_large_err,
    reason = "tests need full error details for assertions"
)]
fn with_jail<F, L, T>(setup: F, loader: L) -> Result<T, OrthoError>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    L: FnOnce() -> Result<T, OrthoError>,
{
    use std::cell::RefCell;

    let result = RefCell::new(None);
    figment::Jail::try_with(|j| {
        setup(j)?;
        let cfg = loader().map_err(|e| figment::error::Error::from(e.to_string()))?;
        result.replace(Some(cfg));
        Ok(())
    })?;
    Ok(result.into_inner().expect("loader executed"))
}

/// Runs `setup` in a jailed environment then loads a subcommand
/// configuration for the `test` command using the `APP_` prefix.
///
/// # Errors
///
/// Returns an error if configuration loading fails.
#[expect(
    clippy::result_large_err,
    reason = "tests need full error details for assertions"
)]
pub fn with_subcommand_config<F, T>(setup: F) -> Result<T, OrthoError>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    T: DeserializeOwned + Default,
{
    with_jail(setup, || {
        load_subcommand_config::<T>(&Prefix::new("APP_"), &CmdName::new("test"))
    })
}

/// Runs `setup` in a jailed environment and loads a subcommand
/// configuration for the `test` command using `T`'s prefix.
///
/// # Errors
///
/// Returns an error if configuration loading fails.
#[expect(
    clippy::result_large_err,
    reason = "tests need full error details for assertions"
)]
pub fn with_typed_subcommand_config<F, T>(setup: F) -> Result<T, OrthoError>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    T: OrthoConfig + Default,
{
    with_jail(setup, || {
        load_subcommand_config_for::<T>(&CmdName::new("test"))
    })
}

/// Runs `setup` in a jailed environment, then loads defaults for the `test`
/// subcommand and merges them with `cli` using the `APP_` prefix.
///
/// # Errors
///
/// Returns an error if configuration loading or merging fails.
#[expect(
    clippy::result_large_err,
    reason = "tests need full error details for assertions"
)]
pub fn with_merged_subcommand_cli<F, T>(setup: F, cli: &T) -> Result<T, OrthoError>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory,
{
    with_jail(setup, || {
        load_and_merge_subcommand(&Prefix::new("APP_"), cli)
    })
}

/// Runs `setup` in a jailed environment, then loads defaults for the `test`
/// subcommand using `T`'s prefix and merges them with `cli`.
///
/// # Errors
///
/// Returns an error if configuration loading or merging fails.
#[expect(
    clippy::result_large_err,
    reason = "tests need full error details for assertions"
)]
pub fn with_merged_subcommand_cli_for<F, T>(setup: F, cli: &T) -> Result<T, OrthoError>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    T: OrthoConfig + serde::Serialize + Default + CommandFactory,
{
    with_jail(setup, || load_and_merge_subcommand_for(cli))
}
