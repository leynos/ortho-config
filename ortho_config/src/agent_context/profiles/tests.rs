//! Tests for the extended `ProfilesDeclaration` contracts.

use std::collections::BTreeSet;

use pretty_assertions::assert_eq;
use proptest::prelude::*;
use serde_json::json;

use super::{ProfileFieldRedaction, ProfileSelectionContract, ProfilesDeclaration};
use crate::docs::ProfileFieldExposure;
use crate::profile::ProfileName;

fn selection() -> ProfileSelectionContract {
    ProfileSelectionContract {
        flag: "profile".to_owned(),
        env_var: "DEMO_PROFILE".to_owned(),
    }
}

fn supported() -> ProfilesDeclaration {
    ProfilesDeclaration::supported(selection())
}

fn record(command: &[&str], field: &str, exposure: ProfileFieldExposure) -> ProfileFieldRedaction {
    ProfileFieldRedaction::new(
        command.iter().map(ToString::to_string).collect(),
        field,
        Some(format!("{field}.key")),
        exposure,
    )
}

fn name(value: &str) -> ProfileName {
    ProfileName::new(value).expect("test names satisfy the profile grammar")
}

#[test]
fn unsupported_declaration_keeps_the_legacy_wire_shape() {
    let encoded = serde_json::to_value(ProfilesDeclaration::unsupported())
        .expect("unsupported declaration serializes");

    assert_eq!(
        encoded,
        json!({ "supported": false }),
        "an unsupported declaration must serialize to the legacy shape"
    );
}

#[test]
fn empty_metadata_is_omitted_from_the_wire() {
    let encoded = serde_json::to_value(supported()).expect("supported declaration serializes");

    assert_eq!(
        encoded,
        json!({
            "supported": true,
            "selection": { "flag": "profile", "env_var": "DEMO_PROFILE" },
        }),
        "no redaction or names metadata may appear when both are empty"
    );
}

#[test]
fn redaction_records_are_canonicalised_by_identity() {
    let declaration = supported()
        .with_redaction(vec![
            record(&["tool", "login"], "token", ProfileFieldExposure::Secret),
            record(&["tool"], "api_key", ProfileFieldExposure::ReferenceOnly),
        ])
        .expect("distinct identities are accepted");

    let identities = declaration
        .redaction()
        .iter()
        .map(|record| (record.command_path.clone(), record.field_name.clone()))
        .collect::<Vec<_>>();

    assert_eq!(
        identities,
        vec![
            (vec!["tool".to_owned()], "api_key".to_owned()),
            (
                vec!["tool".to_owned(), "login".to_owned()],
                "token".to_owned()
            ),
        ],
        "the records must be canonicalised into identity order"
    );
}

#[test]
fn duplicate_redaction_identity_is_rejected() {
    let result = supported().with_redaction(vec![
        record(&["tool"], "token", ProfileFieldExposure::Secret),
        record(&["tool"], "token", ProfileFieldExposure::ReferenceOnly),
    ]);

    let error = result.expect_err("duplicate identities are rejected");
    assert!(
        matches!(
            error.as_ref(),
            crate::OrthoError::DuplicateProfileRedaction { field_name, .. } if field_name == "token"
        ),
        "a repeated identity must be rejected naming the colliding field"
    );
}

#[test]
fn redaction_on_an_unsupported_declaration_is_rejected() {
    let result = ProfilesDeclaration::unsupported().with_redaction(vec![record(
        &["tool"],
        "token",
        ProfileFieldExposure::Secret,
    )]);

    assert!(
        matches!(
            result
                .expect_err("unsupported declarations carry no redaction")
                .as_ref(),
            crate::OrthoError::ProfileMetadataOnUnsupported { .. }
        ),
        "attaching redaction to an unsupported declaration must be refused"
    );
}

#[test]
fn empty_redaction_is_accepted_on_an_unsupported_declaration() {
    let declaration = ProfilesDeclaration::unsupported()
        .with_redaction(Vec::new())
        .expect("an empty list changes nothing");

    assert_eq!(
        declaration,
        ProfilesDeclaration::unsupported(),
        "an empty redaction list must leave the declaration unchanged"
    );
}

#[test]
fn available_names_are_sorted_and_deduplicated() {
    let declaration = supported().with_available_names([
        name("zeta"),
        name("alpha"),
        name("alpha"),
        name("beta"),
    ]);

    let names = declaration
        .available_names()
        .iter()
        .map(ProfileName::as_str)
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["alpha", "beta", "zeta"],
        "the catalogue must be sorted and deduplicated"
    );
}

#[test]
fn available_names_do_not_opt_an_unsupported_declaration_in() {
    let declaration = ProfilesDeclaration::unsupported().with_available_names([name("alpha")]);

    assert_eq!(
        declaration.available_names(),
        &[] as &[ProfileName],
        "an unsupported declaration must discard the supplied names"
    );
    assert!(
        !declaration.supported,
        "supplying a catalogue must not opt the declaration into profiles"
    );
}

#[test]
fn deserialization_canonicalises_the_metadata() {
    let wire = json!({
        "supported": true,
        "selection": { "flag": "profile", "env_var": "DEMO_PROFILE" },
        "redaction": [
            {
                "command_path": ["tool", "login"],
                "field_name": "token",
                "key_path": "auth.token",
                "exposure": "secret"
            },
            {
                "command_path": ["tool"],
                "field_name": "api_key",
                "exposure": "reference_only"
            }
        ],
        "available_names": ["zeta", "alpha", "alpha"]
    });

    let declaration: ProfilesDeclaration =
        serde_json::from_value(wire).expect("valid declaration deserializes");

    let redaction = declaration.redaction();
    assert_eq!(
        redaction
            .first()
            .expect("two records were declared")
            .field_name,
        "api_key",
        "deserialization must canonicalise the shorter command path first"
    );
    assert_eq!(
        redaction
            .get(1)
            .expect("two records were declared")
            .field_name,
        "token",
        "deserialization must canonicalise the longer command path second"
    );
    assert_eq!(
        declaration
            .available_names()
            .iter()
            .map(ProfileName::as_str)
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"],
        "deserialization must sort and deduplicate the names catalogue"
    );
}

#[test]
fn deserialization_rejects_invalid_names() {
    let wire = json!({
        "supported": true,
        "available_names": ["not a name"]
    });

    assert!(
        serde_json::from_value::<ProfilesDeclaration>(wire).is_err(),
        "the invalid profile name must be rejected during deserialization"
    );
}

#[test]
fn deserialization_rejects_metadata_on_an_unsupported_declaration() {
    let wire = json!({
        "supported": false,
        "available_names": ["alpha"]
    });

    assert!(
        serde_json::from_value::<ProfilesDeclaration>(wire).is_err(),
        "metadata on an unsupported declaration must be rejected during deserialization"
    );
}

#[test]
fn deserialization_rejects_a_duplicate_redaction_identity() {
    let wire = json!({
        "supported": true,
        "redaction": [
            {
                "command_path": ["tool"],
                "field_name": "token",
                "exposure": "secret"
            },
            {
                "command_path": ["tool"],
                "field_name": "token",
                "exposure": "reference_only"
            }
        ]
    });

    assert!(
        serde_json::from_value::<ProfilesDeclaration>(wire).is_err(),
        "a duplicate redaction identity must be rejected during deserialization"
    );
}

proptest! {
    /// V3: the catalogue is a deterministic function of the public key set.
    ///
    /// `with_available_names` takes only validated names, so no profile body
    /// is available to leak. Reordering and repeating the input must not change
    /// the sorted, deduplicated result.
    #[test]
    fn available_names_depend_only_on_the_public_key_set(
        raw in prop::collection::vec("p[A-Za-z0-9_-]{0,12}", 0..=8),
        rotation in any::<usize>(),
    ) {
        let parsed = raw.iter().map(|value| name(value)).collect::<Vec<_>>();
        let distinct = raw.iter().cloned().collect::<BTreeSet<_>>();
        let expected = distinct.into_iter().collect::<Vec<_>>();

        let first = supported().with_available_names(parsed.clone());
        let mut reordered = parsed.clone();
        if !reordered.is_empty() {
            let offset = rotation
                .checked_rem(reordered.len())
                .expect("the branch guards against a zero length");
            reordered.rotate_left(offset);
        }
        reordered.extend(parsed);
        let second = supported().with_available_names(reordered);

        prop_assert_eq!(
            &first,
            &second,
            "reordering and repeating the input must not change the declaration"
        );
        let observed = first
            .available_names()
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect::<Vec<_>>();
        prop_assert_eq!(
            observed,
            expected,
            "the catalogue must be the sorted distinct spellings of the input"
        );
    }
}

#[test]
fn serialization_refuses_a_flipped_supported_flag() {
    let mut declaration = supported().with_available_names([name("alpha")]);
    declaration.supported = false;

    let result = serde_json::to_value(declaration);

    assert!(
        result.is_err(),
        "inconsistent declarations must not be emitted"
    );
}
