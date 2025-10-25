//! Utilities for integration tests using `figment::Jail`.
//! These helpers run setup code inside a jailed environment before
//! loading a subcommand configuration. They reduce boilerplate in
//! tests by encapsulating the jail creation and configuration loading.

use std::{path::Path, sync::Arc};

use clap::CommandFactory;
use figment::Error as FigmentError;
use ortho_config::subcommand::Prefix;
use ortho_config::{
    OrthoConfig, OrthoError, OrthoResult, OrthoResultExt, ResultIntoFigment, SubcmdConfigMerge,
    load_and_merge_subcommand,
};
use serde::de::DeserializeOwned;

fn with_jail<F, L, T>(setup: F, loader: L) -> OrthoResult<T>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    L: FnOnce() -> OrthoResult<T>,
{
    use std::cell::RefCell;

    let result = RefCell::new(None);
    figment::Jail::try_with(|j| {
        setup(j)?;
        let cfg = loader().to_figment()?;
        result.replace(Some(cfg));
        Ok(())
    })
    .into_ortho()?;
    result.into_inner().ok_or_else(|| {
        Arc::new(OrthoError::Validation {
            key: "subcommand_loader".into(),
            message: "loader did not run".into(),
        })
    })
}

/// Converts `path` into an owned UTF-8 [`String`] for use in environment
/// variables.
///
/// # Errors
///
/// Returns an error when the provided path cannot be represented as UTF-8.
///
/// # Examples
///
/// ```ignore
/// use std::path::Path;
///
/// let dir = Path::new("/tmp");
/// let utf8 = path_to_utf8_string(dir, "tmp")?;
/// assert_eq!(utf8, "/tmp");
/// ```
#[expect(
    clippy::result_large_err,
    reason = "figment::Error must be returned directly to integrate with Jail closures"
)]
pub fn path_to_utf8_string(path: &Path, context: &str) -> Result<String, FigmentError> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        let display = path.display();
        FigmentError::from(format!("{context} path not valid UTF-8: {display}"))
    })
}

/// Runs `setup` in a jailed environment, then loads defaults for the `test`
/// subcommand and merges them with `cli` using the `APP_` prefix.
///
/// # Errors
///
/// Returns an error if configuration loading or merging fails.
pub fn with_merged_subcommand_cli<F, T>(setup: F, cli: &T) -> OrthoResult<T>
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
pub fn with_merged_subcommand_cli_for<F, T>(setup: F, cli: &T) -> OrthoResult<T>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<()>,
    T: OrthoConfig + serde::Serialize + Default + CommandFactory,
{
    with_jail(setup, || cli.load_and_merge())
}

// These helpers keep tests focused on the unified subcommand loaders without
// legacy-specific branches.
