//! Declarative transforms replayed by the injected `CsvEnv` path.

use crate::SharedScanEnvSource;

#[derive(Clone)]
pub(super) struct Options {
    pub(super) prefix: Option<String>,
    pub(super) split_pattern: Option<String>,
    pub(super) lowercase: Lowercase,
    pub(super) uppercase: Uppercase,
    pub(super) csv: Csv,
    pub(super) key_transform: KeyTransform,
    pub(super) source: Option<SharedScanEnvSource>,
}

impl Options {
    pub(super) fn new(prefix: Option<String>) -> Self {
        Self {
            prefix,
            split_pattern: None,
            lowercase: Lowercase::Enabled,
            uppercase: Uppercase::Disabled,
            csv: Csv::Enabled,
            key_transform: KeyTransform::Declarative,
            source: None,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Lowercase {
    Enabled,
    Disabled,
}

impl Lowercase {
    pub(super) const fn from_bool(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }

    pub(super) const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Clone, Copy)]
pub(super) enum Uppercase {
    Enabled,
    Disabled,
}

impl Uppercase {
    pub(super) const fn from_bool(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }

    pub(super) const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Clone, Copy)]
pub(super) enum Csv {
    Enabled,
    Disabled,
}

impl Csv {
    pub(super) const fn from_bool(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }

    pub(super) const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Clone, Copy)]
pub(super) enum KeyTransform {
    Declarative,
    Opaque,
}
