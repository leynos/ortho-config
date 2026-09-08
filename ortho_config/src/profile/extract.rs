//! Profile-table extraction from the resolved file chain.
//!
//! Extraction is written against the minimal ordered `(path, value)` view of
//! each file layer (decision D13), so it can be re-seated unchanged under RFC
//! 0002's `FileLayerOutcome` if that lands.

use std::borrow::Cow;
use std::sync::Arc;

use camino::Utf8Path;
use serde_json::Value;

use crate::OrthoError;
use crate::OrthoResult;
use crate::declarative::MergeLayer;

use super::AvailableProfileNames;
use super::SelectedProfile;

/// Outcome of extracting profile tables from the file chain.
///
/// Public because the derive-generated loader consumes it; consumers treat it
/// as the two layer vectors the generated code pushes.
#[derive(Debug)]
pub struct ExtractionOutcome {
    /// File layers with the reserved `profile` root key stripped.
    pub file_layers: Vec<MergeLayer<'static>>,
    /// One profile layer per file that defines the selected profile.
    pub profile_layers: Vec<MergeLayer<'static>>,
}

/// Extract and validate `[profile.<name>]` tables from the ordered file chain.
///
/// For every file layer the reserved `profile` root key is stripped (opt-in
/// structs never merge it as an ordinary value), every profile table's name
/// and body are validated, and one profile layer is produced per file that
/// defines the selected profile, in chain order (decision D12). When a
/// profile is selected but no file defines it, loading fails with
/// [`OrthoError::UnknownProfile`] carrying the sorted available names.
///
/// # Errors
///
/// Returns [`OrthoError::ReservedProfileName`] for `[profile.default]`,
/// [`OrthoError::InvalidProfileName`] for names outside the grammar,
/// [`OrthoError::ProfileForbiddenKey`] for `cmds` or `inherits` inside a
/// profile body, and [`OrthoError::UnknownProfile`] when the selected profile
/// is not defined by any file.
pub fn extract_profile_layers(
    layers: Vec<MergeLayer<'static>>,
    selected: Option<&SelectedProfile>,
) -> OrthoResult<ExtractionOutcome> {
    let mut file_layers = Vec::with_capacity(layers.len());
    let mut profile_layers = Vec::new();
    let mut available = Vec::new();
    let mut selected_found = false;

    for layer in layers {
        let path = layer.path().map(Utf8Path::to_path_buf);
        let mut value = layer.into_value();
        let mut file_profile_layer = None;

        if let Some(profile_value) = value.get("profile") {
            if let Some(profile_map) = profile_value.as_object() {
                let (selected_body, found, names) = collect_profile_tables(profile_map, selected)?;
                selected_found |= found;
                file_profile_layer = selected_body.cloned();
                available.extend(names);
            }
            if let Some(object) = value.as_object_mut() {
                object.remove("profile");
            }
        }

        file_layers.push(MergeLayer::file(Cow::Owned(value), path.clone()));
        if let Some(body) = file_profile_layer {
            profile_layers.push(MergeLayer::profile(Cow::Owned(body), path));
        }
    }

    if let Some(selected_profile) = selected.filter(|_| !selected_found) {
        return Err(Arc::new(OrthoError::UnknownProfile {
            selected: selected_profile.name.to_string(),
            selection_source: selected_profile.source,
            available: AvailableProfileNames::new(available),
        }));
    }

    Ok(ExtractionOutcome {
        file_layers,
        profile_layers,
    })
}

/// Validate every profile table in one file and return the selected body.
///
/// Returns the selected profile's table (when selected matches a name), whether
/// the selected profile was found, and the sorted-candidate names for the
/// unknown-profile error.
fn collect_profile_tables<'a>(
    profile_map: &'a serde_json::Map<String, Value>,
    selected: Option<&SelectedProfile>,
) -> OrthoResult<(Option<&'a Value>, bool, Vec<String>)> {
    let mut selected_body = None;
    let mut found = false;
    let mut available = Vec::new();
    for (raw_name, body) in profile_map {
        let name = super::ProfileName::new(raw_name)?;
        validate_profile_body(&name, body)?;
        if selected.is_some_and(|sel| sel.name == name) {
            found = true;
            selected_body = Some(body);
        }
        available.push(raw_name.clone());
    }
    Ok((selected_body, found, available))
}

/// Reject profile-body keys `OrthoConfig` reserves for future work.
///
/// `cmds` is forbidden because subcommand loading ignores profiles (decision
/// D11); `inherits` is reserved for future single-parent inheritance (decision
/// D5). Both are checked so no configuration is silently dead.
fn validate_profile_body(profile: &super::ProfileName, body: &Value) -> OrthoResult<()> {
    let Some(map) = body.as_object() else {
        return Ok(());
    };
    for key in ["cmds", "inherits"] {
        if map.contains_key(key) {
            return Err(Arc::new(OrthoError::ProfileForbiddenKey {
                profile: profile.to_string(),
                key: (*key).to_owned(),
            }));
        }
    }
    Ok(())
}
