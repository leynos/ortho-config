//! Shared helpers for unit and integration tests within the `hello_world` crate.
//!
//! This module is compiled only for tests.

use anyhow::{Result, anyhow};
use ortho_config::figment;

/// Converts a type implementing [`ToString`] into a [`figment::Error`].
///
/// # Examples
///
/// ```
/// # use hello_world::test_support::figment_error;
/// let err = figment_error("configuration failed");
/// assert_eq!(err.to_string(), "configuration failed");
/// ```
#[expect(
    clippy::needless_pass_by_value,
    reason = "tests map errors by value without borrowing"
)]
pub fn figment_error<E: ToString>(err: E) -> figment::Error {
    figment::Error::from(err.to_string())
}

/// Executes the provided closure inside a [`figment::Jail`], capturing its
/// return value.
///
/// # Examples
///
/// ```
/// # use hello_world::test_support::with_jail;
/// # use ortho_config::figment;
/// let value = with_jail(|jail| {
///     jail.set_var("HELLO_WORLD_TEST", "value");
///     Ok::<_, figment::Error>(jail.var("HELLO_WORLD_TEST")?.to_owned())
/// }).unwrap();
/// assert_eq!(value, "value");
/// ```
///
/// # Errors
///
/// Returns an error when the closure fails or when it does not return a value
/// before the jail scope exits.
pub fn with_jail<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut figment::Jail) -> figment::error::Result<T>,
{
    let mut output = None;
    figment::Jail::try_with(|j| {
        output = Some(f(j)?);
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    output.ok_or_else(|| anyhow!("jail closure did not return a value"))
}
