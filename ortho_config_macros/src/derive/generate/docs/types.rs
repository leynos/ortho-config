//! Newtype wrappers for documentation metadata inputs.

use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppName(String);

impl AppName {
    pub(crate) const fn new(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for AppName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl From<String> for AppName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigFileName(String);

impl ConfigFileName {
    pub(crate) const fn new(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for ConfigFileName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl From<String> for ConfigFileName {
    fn from(value: String) -> Self {
        Self(value)
    }
}
