//! Ordered explicit path selectors and the selection they resolve to.
//!
//! A selector names a configuration file directly — from a CLI adapter or an
//! environment variable — and ordered resolution picks the first one that
//! yields a path. [`super::policy::ConfigFilePolicy`] is built from a chain of
//! these, and [`super::policy::FileLayerOutcome`] reports which one won as a
//! [`ResolvedSelection`].

use std::path::PathBuf;

use super::{ConfigDiscovery, telemetry};

/// An ordered explicit configuration-path selector.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ConfigPathSelector {
    path: Option<PathBuf>,
    environment_variable: Option<String>,
    label: String,
    legacy: bool,
    /// The constructor family this selector came from, for telemetry: a
    /// `&'static str` from the closed `SELECTOR_CLASS_*` set, never the
    /// caller-supplied [`label`](Self::label) or variable name.
    class: &'static str,
}

impl ConfigPathSelector {
    /// Select a path supplied by a CLI adapter.
    #[must_use]
    pub fn cli(path: Option<PathBuf>) -> Self {
        Self {
            path,
            environment_variable: None,
            label: String::from("cli"),
            legacy: false,
            class: telemetry::SELECTOR_CLASS_CLI,
        }
    }

    /// Select a path named by an environment variable.
    #[must_use]
    pub fn env(variable_name: impl Into<String>) -> Self {
        let environment_variable = variable_name.into();
        Self {
            path: None,
            environment_variable: Some(environment_variable.clone()),
            label: environment_variable,
            legacy: false,
            class: telemetry::SELECTOR_CLASS_ENVIRONMENT,
        }
    }

    /// Give the selector a diagnostics label.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Mark the selector as a legacy compatibility rung.
    #[must_use]
    pub const fn legacy_alias(mut self) -> Self {
        self.legacy = true;
        self
    }

    /// Resolve this selector against the discovery environment, if it names a
    /// path.
    ///
    /// Visible to the parent module so the ordered chain can be walked from
    /// `ConfigFilePolicy`, which owns the ordering.
    pub(super) fn resolve(&self, discovery: &ConfigDiscovery) -> Option<PathBuf> {
        self.path.clone().or_else(|| {
            self.environment_variable.as_ref().and_then(|name| {
                discovery
                    .env_source
                    .get(name)
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
            })
        })
    }

    /// Returns the telemetry class of this selector's constructor family.
    pub(super) const fn class(&self) -> &'static str {
        self.class
    }

    /// Returns the selector's diagnostics label.
    pub(super) fn label_text(&self) -> &str {
        &self.label
    }

    /// Returns whether the selector is a legacy compatibility rung.
    pub(super) const fn is_legacy(&self) -> bool {
        self.legacy
    }
}

/// Behaviour when an explicit selector yields a path.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExplicitMode {
    /// A selected path is required and suppresses automatic discovery.
    #[default]
    RequiredExclusive,
    /// A missing selected path is ignored, while still suppressing automatic discovery.
    Optional,
}

/// The selector that won ordered explicit resolution.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ResolvedSelection {
    label: String,
    path: PathBuf,
    legacy: bool,
}

impl ResolvedSelection {
    /// Build the winning selection from a resolved selector.
    ///
    /// Fields stay private so the label, path, and legacy flag can only be
    /// produced by resolution rather than assembled by a caller.
    pub(super) const fn new(label: String, path: PathBuf, legacy: bool) -> Self {
        Self {
            label,
            path,
            legacy,
        }
    }

    /// Returns the winning selector label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the selected path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Returns an owned copy of the selected path.
    ///
    /// `FileLayerOutcome::selected` loads the chain before it stores the
    /// selection, so it needs the path to outlive the borrow `path()` hands
    /// out. Kept `pub(super)` because a caller outside `discovery` has no need
    /// to copy what `path()` already lends.
    pub(super) fn path_buf(&self) -> PathBuf {
        self.path.clone()
    }

    /// Returns whether the selector is a legacy alias.
    #[must_use]
    pub const fn legacy(&self) -> bool {
        self.legacy
    }
}
