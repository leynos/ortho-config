//! Profile support declarations (roadmap 9.1.1, extended by 9.1.2).
//!
//! Split from `mod` so each module stays beneath the 400-line cap.
//!
//! The declaration carries only public identity: the selection contract, the
//! optional listing command, the redaction records that name protected fields,
//! and the consumer-supplied catalogue of available profile names. It never
//! carries profile bodies or resolved configuration.

use std::cmp::Ordering;
use std::sync::Arc;

use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::docs::ProfileFieldExposure;
use crate::profile::ProfileName;
use crate::{OrthoError, OrthoResult};

/// Profile support declared by an application.
///
/// The unsupported case serializes byte-identically to the legacy
/// `{ "supported": false }` because the optional fields are omitted when
/// absent (decision D7). Prefer the constructors over struct literals so
/// downstream construction survives future field additions; the metadata
/// fields are private and can only be set through the validating builders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfilesDeclaration {
    /// Whether the application supports profiles.
    pub supported: bool,
    /// The canonical selection contract (flag and environment variable names).
    pub selection: Option<ProfileSelectionContract>,
    /// Command path listing available profiles, populated by roadmap 9.1.3.
    ///
    /// Matches [`crate::agent_context::AgentCommand::path`] token-for-token.
    pub list_command: Option<Vec<String>>,
    /// Protected fields declared by the command tree, sorted by identity.
    redaction: Vec<ProfileFieldRedaction>,
    /// Public profile names supplied by the consumer, sorted and deduplicated.
    available_names: Vec<ProfileName>,
}

/// One protected field's public identity and classification.
///
/// The record deliberately carries no value-bearing metadata. `command_path`
/// is the existing tokenised command path, `field_name` the emitted metadata
/// name, and `key_path` the optional existing file key spelling retained
/// verbatim rather than re-parsed from a dotted string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileFieldRedaction {
    /// Tokenised command path whose field is protected.
    pub command_path: Vec<String>,
    /// Emitted metadata field name.
    pub field_name: String,
    /// Existing file configuration key, when the field has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    /// The declared disclosure classification.
    pub exposure: ProfileFieldExposure,
}

impl ProfileFieldRedaction {
    /// Builds one redaction record from its public identity and label.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::ProfileFieldRedaction;
    /// use ortho_config::docs::ProfileFieldExposure;
    ///
    /// let record = ProfileFieldRedaction::new(
    ///     vec!["tool".to_owned(), "login".to_owned()],
    ///     "token",
    ///     Some("auth.token".to_owned()),
    ///     ProfileFieldExposure::Secret,
    /// );
    /// assert_eq!(record.field_name, "token");
    /// ```
    #[must_use]
    pub fn new(
        command_path: Vec<String>,
        field_name: impl Into<String>,
        key_path: Option<String>,
        exposure: ProfileFieldExposure,
    ) -> Self {
        Self {
            command_path,
            field_name: field_name.into(),
            key_path,
            exposure,
        }
    }

    /// Orders records by the `(command_path, field_name)` identity.
    fn identity_cmp(&self, other: &Self) -> Ordering {
        (&self.command_path, &self.field_name).cmp(&(&other.command_path, &other.field_name))
    }

    /// Reports whether two records describe the same field.
    fn has_same_identity(&self, other: &Self) -> bool {
        self.command_path == other.command_path && self.field_name == other.field_name
    }
}

impl ProfilesDeclaration {
    /// The legacy unsupported declaration.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self {
            supported: false,
            selection: None,
            list_command: None,
            redaction: Vec::new(),
            available_names: Vec::new(),
        }
    }

    /// A supported declaration carrying the selection contract.
    #[must_use]
    pub const fn supported(selection: ProfileSelectionContract) -> Self {
        Self {
            supported: true,
            selection: Some(selection),
            list_command: None,
            redaction: Vec::new(),
            available_names: Vec::new(),
        }
    }

    /// Attaches redaction records, canonicalising their order.
    ///
    /// # Errors
    ///
    /// Returns [`OrthoError::DuplicateProfileRedaction`] when two records name
    /// the same `(command_path, field_name)` identity, and
    /// [`OrthoError::ProfileMetadataOnUnsupported`] when records are attached
    /// to an unsupported declaration.
    pub fn with_redaction(mut self, records: Vec<ProfileFieldRedaction>) -> OrthoResult<Self> {
        if records.is_empty() {
            return Ok(self);
        }
        ensure_supported(self.supported, "redaction records")?;
        self.redaction = canonicalise_redaction(records)?;
        Ok(self)
    }

    /// Attaches the consumer-supplied catalogue of public profile names.
    ///
    /// Names are sorted and deduplicated. An unsupported declaration is
    /// returned unchanged: supplying a catalogue does not opt an application
    /// into profiles.
    #[must_use]
    pub fn with_available_names(mut self, names: impl IntoIterator<Item = ProfileName>) -> Self {
        if self.supported {
            self.available_names = canonicalise_names(names);
        }
        self
    }

    /// Returns the protected-field records in deterministic identity order.
    #[must_use]
    pub fn redaction(&self) -> &[ProfileFieldRedaction] {
        &self.redaction
    }

    /// Returns the sorted, deduplicated public profile names.
    #[must_use]
    pub fn available_names(&self) -> &[ProfileName] {
        &self.available_names
    }

    /// Rebuilds a declaration from deserialized wire data, enforcing every
    /// invariant that the constructors enforce.
    fn from_wire(wire: ProfilesDeclarationWire) -> OrthoResult<Self> {
        if !wire.supported && carries_metadata(&wire.redaction, &wire.available_names) {
            ensure_supported(false, "redaction records or available names")?;
        }
        Ok(Self {
            supported: wire.supported,
            selection: wire.selection,
            list_command: wire.list_command,
            redaction: canonicalise_redaction(wire.redaction)?,
            available_names: canonicalise_names(wire.available_names),
        })
    }
}

impl Default for ProfilesDeclaration {
    fn default() -> Self {
        Self::unsupported()
    }
}

/// Reports whether a declaration carries profile-only metadata.
///
/// Redaction records and the names catalogue may only accompany a supported
/// declaration; both the deserializer and the serializer ask this one question
/// rather than repeating the disjunction.
const fn carries_metadata(
    redaction: &[ProfileFieldRedaction],
    available_names: &[ProfileName],
) -> bool {
    !redaction.is_empty() || !available_names.is_empty()
}

/// Rejects profile-only metadata on an unsupported declaration.
fn ensure_supported(supported: bool, kind: &'static str) -> OrthoResult<()> {
    if supported {
        Ok(())
    } else {
        Err(Arc::new(OrthoError::ProfileMetadataOnUnsupported { kind }))
    }
}

/// Sorts records by identity and rejects duplicates rather than picking one
/// label arbitrarily.
fn canonicalise_redaction(
    mut records: Vec<ProfileFieldRedaction>,
) -> OrthoResult<Vec<ProfileFieldRedaction>> {
    records.sort_by(ProfileFieldRedaction::identity_cmp);
    // Sorting groups equal identities together, so inspecting each adjacent
    // pair finds every duplicate. A one-step offset zip does that without
    // indexing into the vector.
    let adjacent_duplicate = records
        .iter()
        .zip(records.iter().skip(1))
        .find(|(left, right)| left.has_same_identity(right))
        .map(|(left, _right)| left);
    if let Some(duplicate) = adjacent_duplicate {
        return Err(Arc::new(OrthoError::DuplicateProfileRedaction {
            command_path: duplicate.command_path.join(" "),
            field_name: duplicate.field_name.clone(),
        }));
    }
    Ok(records)
}

/// Sorts names and removes repeated spellings.
fn canonicalise_names(names: impl IntoIterator<Item = ProfileName>) -> Vec<ProfileName> {
    let mut unique = names.into_iter().collect::<Vec<_>>();
    unique.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    unique.dedup_by(|left, right| left.as_str() == right.as_str());
    unique
}

/// Deserialization view that lets the declaration validate before it exists.
#[derive(Deserialize)]
struct ProfilesDeclarationWire {
    /// Whether the application supports profiles.
    supported: bool,
    /// Declared profile-selection contract, when supported.
    #[serde(default)]
    selection: Option<ProfileSelectionContract>,
    /// Command a caller runs to list profile names, when supported.
    #[serde(default)]
    list_command: Option<Vec<String>>,
    /// Protected field records, absent on legacy documents.
    #[serde(default)]
    redaction: Vec<ProfileFieldRedaction>,
    /// Public profile names, absent on legacy documents.
    #[serde(default)]
    available_names: Vec<ProfileName>,
}

impl<'de> Deserialize<'de> for ProfilesDeclaration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = ProfilesDeclarationWire::deserialize(deserializer)?;
        Self::from_wire(wire).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ProfilesDeclaration {
    /// Serializes the declaration, refusing an inconsistent one.
    ///
    /// `supported` is a public mutable flag, so a caller can flip it after
    /// attaching metadata. Emitting that declaration would produce a document
    /// that consumers must reject, so serialization fails instead.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !self.supported && carries_metadata(&self.redaction, &self.available_names) {
            return Err(serde::ser::Error::custom(
                OrthoError::ProfileMetadataOnUnsupported {
                    kind: "redaction records or available names",
                },
            ));
        }
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("supported", &self.supported)?;
        if let Some(selection) = &self.selection {
            map.serialize_entry("selection", selection)?;
        }
        if let Some(list_command) = &self.list_command {
            map.serialize_entry("list_command", list_command)?;
        }
        if !self.redaction.is_empty() {
            map.serialize_entry("redaction", &self.redaction)?;
        }
        if !self.available_names.is_empty() {
            map.serialize_entry("available_names", &self.available_names)?;
        }
        map.end()
    }
}

/// The canonical profile selection contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSelectionContract {
    /// The selector flag name following the `AgentInput::long` convention
    /// (no leading `--`).
    pub flag: String,
    /// The selector environment variable name (for example `APP_PROFILE`).
    pub env_var: String,
}

#[cfg(test)]
mod tests;
