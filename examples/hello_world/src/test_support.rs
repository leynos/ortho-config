//! Shared helpers for unit and integration tests within the `hello_world` crate.
//!
//! This module is compiled only for tests.

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
