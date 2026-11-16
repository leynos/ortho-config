//! Shared catalogue for the profile-field disclosure boundary.
//!
//! Every literal here is declared by `orthohelp_fixture::redaction` in a
//! value-bearing slot: a default, an enum member, or an example. The strings
//! appear nowhere in a public description, so finding one in a generated
//! artefact is proof that the projection failed rather than a coincidence of
//! shared wording.
//!
//! The behavioural, golden, and contract suites import this module so a new
//! fixture literal cannot be scanned by one suite and missed by another.

/// Root type whose fields carry both disclosure classifications.
pub(crate) const REDACTION_ROOT_TYPE: &str = "orthohelp_fixture::redaction::RedactionFixtureConfig";

/// Root type that classifies a field without opting into profiles.
pub(crate) const UNSUPPORTED_ROOT_TYPE: &str =
    "orthohelp_fixture::redaction::UnsupportedRedactionFixtureConfig";

/// Root type with no disclosure classification at all.
pub(crate) const ORDINARY_ROOT_TYPE: &str = "orthohelp_fixture::redaction::OrdinaryFixtureConfig";

/// Locales every multi-locale artefact scan visits.
pub(crate) const ARTEFACT_LOCALES: [&str; 2] = ["en-US", "fr-FR"];

/// Protected literals attached to fields classified `secret`.
pub(crate) const SECRET_SENTINELS: &[&str] = &[
    "SECRET-DEFAULT-SENTINEL",
    "SECRET-ENUM-ALPHA",
    "SECRET-ENUM-BETA",
    "SECRET-EXAMPLE-SENTINEL",
    "HIDDEN-DEFAULT-SENTINEL",
    "HIDDEN-EXAMPLE-SENTINEL",
    "NESTED-SECRET-DEFAULT-SENTINEL",
    "NESTED-SECRET-ENUM-ALPHA",
    "NESTED-SECRET-ENUM-BETA",
    "NESTED-SECRET-EXAMPLE-SENTINEL",
];

/// Protected literals attached to fields classified `reference_only`.
pub(crate) const REFERENCE_SENTINELS: &[&str] = &[
    "REFERENCE-DEFAULT-SENTINEL",
    "REFERENCE-ENUM-ALPHA",
    "REFERENCE-ENUM-BETA",
    "REFERENCE-EXAMPLE-SENTINEL",
    "FILE-ONLY-DEFAULT-SENTINEL",
    "FILE-ONLY-EXAMPLE-SENTINEL",
];

/// The one literal the fixture also places in parent and sibling examples.
pub(crate) const COMMAND_EXAMPLE_SENTINELS: &[&str] = &["NESTED-SECRET-DEFAULT-SENTINEL"];

/// The literal on the document that never opted into profiles.
pub(crate) const UNSUPPORTED_SENTINELS: &[&str] = &["UNSUPPORTED-DEFAULT-SENTINEL"];

/// Every protected literal the profile-enabled fixture declares.
pub(crate) fn all_protected_sentinels() -> Vec<&'static str> {
    SECRET_SENTINELS
        .iter()
        .chain(REFERENCE_SENTINELS.iter())
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    //! Checks the catalogue is internally consistent.
    //!
    //! Each integration test target includes this module through
    //! `tests::fixtures` but reads only the constants its own suite needs.
    //! Reading all of them here keeps every target free of dead-code warnings
    //! and proves the aggregate view has not drifted from the per-classification
    //! lists.

    use super::*;

    #[test]
    fn catalogue_matches_its_aggregate_view() {
        let protected = all_protected_sentinels();
        assert_eq!(
            protected.len(),
            SECRET_SENTINELS.len() + REFERENCE_SENTINELS.len()
        );
        for sentinel in SECRET_SENTINELS.iter().chain(REFERENCE_SENTINELS.iter()) {
            assert!(protected.contains(sentinel), "{sentinel} missing from view");
        }

        for root in [
            REDACTION_ROOT_TYPE,
            UNSUPPORTED_ROOT_TYPE,
            ORDINARY_ROOT_TYPE,
        ] {
            assert!(
                root.starts_with("orthohelp_fixture::redaction::"),
                "{root} should name the redaction fixture module"
            );
        }
        assert_eq!(ARTEFACT_LOCALES, ["en-US", "fr-FR"]);
        // Named membership reads better than an emptiness check and proves the
        // lists hold the literal each scenario is about, not merely something.
        assert!(
            COMMAND_EXAMPLE_SENTINELS.contains(&"NESTED-SECRET-DEFAULT-SENTINEL"),
            "the command-level example list should hold the nested sentinel"
        );
        assert!(
            UNSUPPORTED_SENTINELS.contains(&"UNSUPPORTED-DEFAULT-SENTINEL"),
            "the unsupported list should hold the fixture's default"
        );
    }
}
