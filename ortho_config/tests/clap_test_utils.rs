//! Helpers specific to the CLI integration test suite.

use anyhow::Result;

/// Trait for types that can validate their `sample_value` and `other` fields.
///
/// # Errors
///
/// Implementations should return an error when observed values do not match
/// the expected configuration.
pub trait ConfigValueAssertions {
    /// Checks the captured values against the expected ones.
    ///
    /// # Errors
    ///
    /// Returns an error when the implementation reports a mismatch.
    fn assert_values(
        &self,
        expected_sample: Option<&'static str>,
        expected_other: Option<&'static str>,
    ) -> Result<()>;
}

/// Validates the sampled configuration values via [`ConfigValueAssertions`].
///
/// # Errors
///
/// Returns an error if the underlying implementation signals a mismatch.
pub fn assert_config_values<T>(
    config: &T,
    expected_sample: Option<&'static str>,
    expected_other: Option<&'static str>,
) -> Result<()>
where
    T: ConfigValueAssertions,
{
    config.assert_values(expected_sample, expected_other)
}
