//! Newtype wrappers for documentation metadata inputs.

use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppName(String);

impl AppName {
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Newtype constructor is not used in const context."
    )]
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }
}

impl Deref for AppName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for AppName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for AppName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for AppName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigFileName(String);

impl ConfigFileName {
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Newtype constructor is not used in const context."
    )]
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }
}

impl Deref for ConfigFileName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for ConfigFileName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ConfigFileName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for ConfigFileName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
