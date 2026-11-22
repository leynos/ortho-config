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
    "INFERRED-DEFAULT-SENTINEL",
    "INFERRED-EXAMPLE-SENTINEL",
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
    //! Checks the catalogue's own invariants.
    //!
    //! Each integration test target includes this module through
    //! `tests::fixtures` and reads only the entries its suite needs, so these
    //! cross-references are the one place the catalogue is checked as a whole.
    //! They test the asset rather than generated output: the suites reason
    //! about membership and attribution across these lists, and a drifted list
    //! would weaken a scan without failing it.

    use std::collections::BTreeSet;

    use super::*;

    /// A sentinel that reads like prose could match a public description, so a
    /// scan reporting it absent would not prove the projection ran.
    fn assert_reads_as_a_sentinel(sentinel: &str) {
        assert!(
            sentinel.contains("-SENTINEL") || sentinel.contains("-ENUM-"),
            "{sentinel} does not follow the sentinel naming convention"
        );
    }

    /// Every protected literal must be attributable to exactly one
    /// classification.
    fn assert_classifications_are_disjoint_and_distinct() {
        let secret: BTreeSet<&str> = SECRET_SENTINELS.iter().copied().collect();
        let reference: BTreeSet<&str> = REFERENCE_SENTINELS.iter().copied().collect();
        assert_eq!(
            secret.len(),
            SECRET_SENTINELS.len(),
            "the secret list repeats a literal, so a scan could miss one"
        );
        assert_eq!(
            reference.len(),
            REFERENCE_SENTINELS.len(),
            "the reference-only list repeats a literal, so a scan could miss one"
        );
        assert!(
            secret.is_disjoint(&reference),
            "a literal cannot be both secret and reference-only"
        );
        for sentinel in SECRET_SENTINELS.iter().chain(REFERENCE_SENTINELS.iter()) {
            assert_reads_as_a_sentinel(sentinel);
        }
        for sentinel in UNSUPPORTED_SENTINELS {
            assert_reads_as_a_sentinel(sentinel);
        }
    }

    #[test]
    fn catalogue_is_internally_consistent() {
        assert_classifications_are_disjoint_and_distinct();

        let protected = all_protected_sentinels();
        assert_eq!(
            protected.len(),
            SECRET_SENTINELS.len() + REFERENCE_SENTINELS.len(),
            "the aggregate view must hold every protected literal exactly once"
        );
        for example in COMMAND_EXAMPLE_SENTINELS {
            assert!(
                protected.contains(example),
                "{example} sits in command examples but is not a protected literal"
            );
        }
        for sentinel in UNSUPPORTED_SENTINELS {
            assert!(
                !protected.contains(sentinel),
                "{sentinel} belongs to the unsupported document, which opts into no profiles"
            );
        }
    }

    /// A scenario is selected by root type, so two roots sharing a name would
    /// silently scan the same document twice.
    #[test]
    fn root_types_and_locales_identify_distinct_documents() {
        let roots = [
            REDACTION_ROOT_TYPE,
            UNSUPPORTED_ROOT_TYPE,
            ORDINARY_ROOT_TYPE,
        ];
        let distinct: BTreeSet<&str> = roots.into_iter().collect();
        assert_eq!(
            distinct.len(),
            roots.len(),
            "each scenario needs its own root type: {roots:?}"
        );

        let locales: BTreeSet<&str> = ARTEFACT_LOCALES.into_iter().collect();
        assert!(locales.len() > 1, "the multi-locale scan needs two or more");
        assert_eq!(
            locales.len(),
            ARTEFACT_LOCALES.len(),
            "a repeated locale would scan the same artefact twice: {ARTEFACT_LOCALES:?}"
        );
    }
}
