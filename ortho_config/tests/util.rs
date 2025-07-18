//! Utilities for integration tests using `figment::Jail`.
//!
//! These helpers run setup code inside a jailed environment before
//! loading a subcommand configuration. They reduce boilerplate in
//! tests by encapsulating the jail creation and configuration loading.
#![expect(
    deprecated,
    reason = "figment's Jail uses deprecated APIs for test isolation"
)]

use ortho_config::subcommand::{CmdName, Prefix};
use ortho_config::{OrthoConfig, load_subcommand_config, load_subcommand_config_for};
use serde::de::DeserializeOwned;

fn with_jail<F, L, T>(setup: F, loader: L) -> T
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    L: FnOnce() -> T,
{
    use std::cell::RefCell;

    let result = RefCell::new(None);
    figment::Jail::expect_with(|j| {
        setup(j)?;
        result.replace(Some(loader()));
        Ok(())
    });
    result.into_inner().unwrap()
}

/// Runs `setup` in a jailed environment then loads a subcommand
/// configuration for the `test` command using the `APP_` prefix.
///
/// # Panics
///
/// Panics if configuration loading fails.
pub fn with_subcommand_config<F, T>(setup: F) -> T
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    T: DeserializeOwned + Default,
{
    with_jail(setup, || {
        load_subcommand_config::<T>(&Prefix::new("APP_"), &CmdName::new("test")).expect("load")
    })
}

/// Runs `setup` in a jailed environment and loads a subcommand
/// configuration for the `test` command using `T`'s prefix.
///
/// # Panics
///
/// Panics if configuration loading fails.
pub fn with_typed_subcommand_config<F, T>(setup: F) -> T
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    T: OrthoConfig + Default,
{
    with_jail(setup, || {
        load_subcommand_config_for::<T>(&CmdName::new("test")).expect("load")
    })
}
