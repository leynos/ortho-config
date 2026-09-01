//! Declarative transforms replayed by the injected `CsvEnv` path.

use crate::SharedScanEnvSource;

/// Declarative state retained so injected loading can replay provider setup.
#[derive(Clone)]
pub(super) struct Options {
    /// Prefix removed before the remaining key transforms run.
    pub(super) prefix: Option<String>,
    /// Key mappings replayed in the precise order in which builders added them.
    pub(super) mappings: Vec<KeyMapping>,
    /// Whether final keys use ASCII lowercase.
    pub(super) lowercase: Lowercase,
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
            mappings: Vec::new(),
            lowercase: Lowercase::Enabled,
            csv: Csv::Enabled,
            key_transform: KeyTransform::Declarative,
            source: None,
        }
    }
}

/// A replayable key transformation that Figment represents as a mapping closure.
///
/// Each mapping resets Figment's lowercase mode. `CsvEnv` records that reset
/// when the builder adds the mapping, so injected replay needs only apply this
/// sequence in order before the final case conversion.
#[derive(Clone)]
pub(super) enum KeyMapping {
    /// Convert the whole key to ASCII uppercase at this point in the sequence.
    Uppercase(Uppercase),
    /// Replace this literal pattern with dots to create nested dictionary keys.
    Split(String),
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

/// Whether one replayed mapping converts a key to ASCII uppercase.
#[derive(Clone, Copy)]
pub(super) enum Uppercase {
    /// Apply ASCII uppercase at this point in the mapping sequence.
    Enabled,
    /// Leave the key unchanged at this point in the mapping sequence.
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
