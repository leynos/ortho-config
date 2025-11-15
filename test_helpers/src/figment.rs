//! Shared helpers for working with `figment::Jail` in tests.
//!
//! These utilities centralise the common pattern of initialising a jail,
//! running a closure that performs setup work (creating files, injecting
//! environment variables, etc.), and propagating the closure's return value as
//! an `anyhow::Result`. They eliminate the repetitive `Option` plumbing that
//! callers previously had to write by hand.

use anyhow::{Result, anyhow};

/// Executes `f` inside a [`figment::Jail`], returning the closure's output.
///
/// The jail is torn down automatically once the closure completes, even when
/// the closure returns an error. Failures are converted into `anyhow::Error`
/// values so callers can use the `?` operator without extra boilerplate.
///
/// # Errors
///
/// Returns an error if the jail initialisation fails or the closure returns a
/// [`figment::error::Error`].
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

/// Converts any error implementing [`ToString`] into a [`figment::Error`].
///
/// Helpful when bridging between `anyhow::Error` and APIs that expect a
/// figment-specific error type.
#[expect(
    clippy::needless_pass_by_value,
    reason = "callers often own the error and passing by value avoids extra clones"
)]
pub fn figment_error<E: ToString>(err: E) -> figment::Error {
    figment::Error::from(err.to_string())
}
