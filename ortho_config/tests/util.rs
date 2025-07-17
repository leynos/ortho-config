//! Utilities for integration tests using `figment::Jail`.
//!
//! These helpers run setup code inside a jailed environment before
//! loading a subcommand configuration. They reduce boilerplate in
//! tests by encapsulating the jail creation and configuration loading.
#![allow(deprecated)]

use ortho_config::subcommand::{CmdName, Prefix};
use ortho_config::{OrthoConfig, load_subcommand_config, load_subcommand_config_for};
use serde::de::DeserializeOwned;

/// Runs `setup` in a jailed environment then loads a subcommand
/// configuration for the `test` command using the `APP_` prefix.
///
/// # Panics
///
/// Panics if configuration loading fails.
pub fn with_cfg<F, T>(setup: F) -> T
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    T: DeserializeOwned + Default,
{
    use std::cell::RefCell;

    let result = RefCell::new(None);
    figment::Jail::expect_with(|j| {
        setup(j)?;
        let cfg =
            load_subcommand_config::<T>(&Prefix::new("APP_"), &CmdName::new("test")).expect("load");
        result.replace(Some(cfg));
        Ok(())
    });
    result.into_inner().unwrap()
}

/// Runs `setup` in a jailed environment and loads a subcommand
/// configuration for the `test` command using `T`'s prefix.
///
/// # Panics
///
/// Panics if configuration loading fails.
pub fn with_cfg_wrapper<F, T>(setup: F) -> T
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    T: OrthoConfig + Default,
{
    use std::cell::RefCell;

    let result = RefCell::new(None);
    figment::Jail::expect_with(|j| {
        setup(j)?;
        let cfg = load_subcommand_config_for::<T>(&CmdName::new("test")).expect("load");
        result.replace(Some(cfg));
        Ok(())
    });
    result.into_inner().unwrap()
}
