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
    let chain_is_empty = layers.is_empty();
    let mut file_layers = Vec::with_capacity(layers.len());
    let mut profile_layers = Vec::new();
    let mut available = Vec::new();
    let mut selected_found = false;

    for layer in layers {
        let path = layer.path().map(Utf8Path::to_path_buf);
        let mut value = layer.into_value();
        let (selected_body, found) = take_profile_tables(&mut value, selected, &mut available)?;
        selected_found |= found;

        file_layers.push(MergeLayer::file(Cow::Owned(value), path.clone()));
        // `map` over the optional body keeps the loop body free of a
        // conditional: zero or one profile layer falls out of each file.
        let profile_layer = selected_body.map(|body| MergeLayer::profile(Cow::Owned(body), path));
        profile_layers.extend(profile_layer);
    }

    if let Some(selected_profile) = selected.filter(|_| !selected_found) {
        return Err(unknown_profile_error(
            selected_profile,
            chain_is_empty,
            available,
        ));
    }

    Ok(ExtractionOutcome {
        file_layers,
        profile_layers,
    })
}

/// Build the error for a selection that no file defines.
///
/// The two ways an empty name list can arise are reported distinctly: a chain
/// with no files at all is a discovery problem, while a non-empty chain that
/// defines no profile tables is a selector problem. Reporting the former for
/// the latter would send the operator hunting for files that are already
/// present.
fn unknown_profile_error(
    selected: &SelectedProfile,
    chain_is_empty: bool,
    available: Vec<String>,
) -> Arc<OrthoError> {
    let reported = if chain_is_empty {
        AvailableProfileNames::no_files_discovered()
    } else {
        AvailableProfileNames::new(available)
    };
    Arc::new(OrthoError::UnknownProfile {
        selected: selected.name.to_string(),
        selection_source: selected.source,
        available: reported,
    })
}

/// Strip the reserved `profile` root key from one file layer and validate it.
///
/// Returns the selected profile's table (when the selection matches a name
/// defined here) and whether the selection was found. Candidate names are
/// appended to `available` for the unknown-profile error.
///
/// A `profile` key that is present but not a table is still stripped: the key
/// is reserved across all three projections once a struct opts in, so it never
/// merges as an ordinary value (decision D12).
fn take_profile_tables(
    value: &mut Value,
    selected: Option<&SelectedProfile>,
    available: &mut Vec<String>,
) -> OrthoResult<(Option<Value>, bool)> {
    let Some(object) = value.as_object_mut() else {
        return Ok((None, false));
    };
    // One pattern handles all three misses at once: no `profile` key, a
    // `profile` key holding a non-table, and a non-object file layer. The key
    // is removed either way, so it never survives into the file layer.
    let Some(Value::Object(profile_map)) = object.remove("profile") else {
        return Ok((None, false));
    };
    let (selected_body, found) = collect_profile_tables(&profile_map, selected, available)?;
    Ok((selected_body.cloned(), found))
}

/// Validate every profile table in one file and return the selected body.
///
/// Returns the selected profile's table (when selected matches a name) and
/// whether the selected profile was found.
fn collect_profile_tables<'a>(
    profile_map: &'a serde_json::Map<String, Value>,
    selected: Option<&SelectedProfile>,
    available: &mut Vec<String>,
) -> OrthoResult<(Option<&'a Value>, bool)> {
    let mut selected_body = None;
    let mut found = false;
    for (raw_name, body) in profile_map {
        let name = super::ProfileName::new(raw_name)?;
        validate_profile_body(&name, body)?;
        if selected.is_some_and(|sel| sel.name == name) {
            found = true;
            selected_body = Some(body);
        }
        available.push(raw_name.clone());
    }
    Ok((selected_body, found))
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
