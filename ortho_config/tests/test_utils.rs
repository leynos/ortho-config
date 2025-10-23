//! Shared helpers for integration tests that rely on `figment::Jail`.

use anyhow::{Result, anyhow};
use figment::Jail;
use std::cell::RefCell;

/// Runs `f` inside a `figment::Jail`, returning any propagated error as an
/// [`anyhow::Result`].
///
/// # Errors
///
/// Returns an error when either the inner closure fails or when the jailed
/// execution cannot be initialised.
pub fn with_jail<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut Jail) -> Result<T>,
{
    let output = RefCell::new(None);
    figment::Jail::try_with(|j| {
        let result = f(j).map_err(|err| figment::Error::from(err.to_string()))?;
        output.replace(Some(result));
        Ok(())
    })
    .map_err(|err| anyhow!(err.to_string()))?;
    output
        .into_inner()
        .ok_or_else(|| anyhow!("jail closure did not produce a result"))
}
