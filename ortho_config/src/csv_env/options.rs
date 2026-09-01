//! Declarative transforms replayed by the injected `CsvEnv` path.

use crate::SharedScanEnvSource;

/// Declarative state retained so injected loading can replay provider setup.
#[derive(Clone)]
pub(super) struct Options {
    /// Prefix removed before the remaining key transforms run.
    pub(super) prefix: Option<String>,
    /// Literal pattern converted to dots for nested configuration keys.
    pub(super) split_pattern: Option<String>,
    /// Whether final keys use ASCII lowercase.
    pub(super) lowercase: Lowercase,
    /// Whether keys are uppercased before split processing.
    pub(super) uppercase: Uppercase,
    /// Whether comma-containing scalar values become arrays.
    pub(super) csv: Csv,
    /// Whether the provider's key transform can be replayed declaratively.
    pub(super) key_transform: KeyTransform,
    /// Optional source used instead of Figment's process enumeration.
    pub(super) source: Option<SharedScanEnvSource>,
}

impl Options {
    /// Construct default transform state for a raw or prefixed provider.
    ///
    /// These defaults mirror Figment: lowercase keys, no uppercase pass, and
    /// CSV parsing enabled. They are retained independently of `Env` because
    /// its stored closures cannot be inspected later.
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

/// Whether the final replayed key is converted to ASCII lowercase.
#[derive(Clone, Copy)]
pub(super) enum Lowercase {
    /// Apply ASCII lowercase after splitting and trimming.
    Enabled,
    /// Preserve the post-split key casing.
    Disabled,
}

impl Lowercase {
    /// Convert the public boolean builder argument into explicit state.
    pub(super) const fn from_bool(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }

    /// Report whether this transform must run during injected replay.
    pub(super) const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Whether injected replay uppercases a key before it is split.
#[derive(Clone, Copy)]
pub(super) enum Uppercase {
    /// Apply ASCII uppercase before the split transform.
    Enabled,
    /// Leave the prefixed key unchanged before splitting.
    Disabled,
}

impl Uppercase {
    /// Convert the public boolean builder argument into explicit state.
    pub(super) const fn from_bool(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }

    /// Report whether the uppercase transform must run during replay.
    pub(super) const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Whether comma-containing scalar values are interpreted as CSV lists.
#[derive(Clone, Copy)]
pub(super) enum Csv {
    /// Parse eligible comma-containing values as arrays.
    Enabled,
    /// Preserve every value as a single scalar string or structured value.
    Disabled,
}

impl Csv {
    /// Convert the public boolean builder argument into explicit state.
    pub(super) const fn from_bool(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }

    /// Report whether CSV parsing is active for the provider.
    pub(super) const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Whether an injected source can reproduce the configured key transformation.
#[derive(Clone, Copy)]
pub(super) enum KeyTransform {
    /// The key transform consists only of retained declarative options.
    Declarative,
    /// A `map` or `filter_map` closure hides transformation behaviour.
    Opaque,
}
