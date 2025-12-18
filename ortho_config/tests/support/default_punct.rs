//! Shared fixtures for `cli_default_as_absent` tests.

/// Default punctuation used when neither file nor CLI provides a value.
#[must_use]
pub fn default_punct() -> String {
    "!".into()
}
