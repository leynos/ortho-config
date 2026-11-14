//! Example-binding checks for supplied skill claims.
//!
//! An example is only checkable once its manifest ID resolves to exactly one
//! supplied manifest and its ordinal selects one declared command reference.
//! Anything that breaks that chain is reported against the example itself, and
//! the remaining checks do not run: a path-mismatched example says nothing
//! trustworthy about option names.

use std::collections::BTreeSet;

use ortho_config::SkillCommandRef;

use super::super::input::{ExampleExtraction, ManifestClaims, NormalizedExample, OptionUse};
use super::super::policy_check::option_use_advice;
use super::super::report::codes;
use super::super::resolve::join_path;
use super::manifest_index::OwnerKey;
use super::{ClaimRef, FindingData, Validator, reference_at};

impl<'a> Validator<'a> {
    /// Checks one normalized example.
    pub(super) fn check_example(&mut self, example: &'a NormalizedExample) {
        if example.extraction == ExampleExtraction::Unsupported {
            return;
        }
        let claim = Self::example_claim(usize::MAX, example);
        match self.manifests.owner(&example.manifest_id) {
            OwnerKey::Missing => self.push_structural(
                claim,
                FindingData {
                    code: codes::EXAMPLE_UNKNOWN_MANIFEST,
                    option: None,
                    message: format!("manifest ID '{}' is not declared", example.manifest_id),
                },
            ),
            OwnerKey::Ambiguous(order) => self.push_structural(
                Self::example_claim(order, example),
                FindingData {
                    code: codes::EXAMPLE_AMBIGUOUS_MANIFEST,
                    option: None,
                    message: format!(
                        "manifest ID '{}' is declared more than once; \
                         the example owner is ambiguous",
                        example.manifest_id
                    ),
                },
            ),
            OwnerKey::Unique(order) => {
                let manifests = self.manifests.manifests;
                if let Some(manifest) = manifests.get(order) {
                    self.check_bound_example(order, manifest, example);
                }
            }
        }
    }

    /// Builds a claim anchor for an example.
    fn example_claim(order: usize, example: &'a NormalizedExample) -> ClaimRef<'a> {
        ClaimRef {
            order,
            manifest_id: &example.manifest_id,
            source_ordinal: example.source_ordinal,
            path: &example.path,
        }
    }

    /// Checks an example whose owning manifest resolved uniquely.
    fn check_bound_example(
        &mut self,
        order: usize,
        manifest: &'a ManifestClaims,
        example: &'a NormalizedExample,
    ) {
        let claim = Self::example_claim(order, example);
        let Some(reference) = reference_at(manifest, example.command_reference_ordinal) else {
            self.push_structural(
                claim,
                FindingData {
                    code: codes::EXAMPLE_UNBOUND_REFERENCE,
                    option: None,
                    message: "example does not select a declared command reference".to_owned(),
                },
            );
            return;
        };
        if example.path != reference.path {
            self.push_structural(
                claim,
                FindingData {
                    code: codes::EXAMPLE_PATH_MISMATCH,
                    option: None,
                    message: "example path differs from its owning command reference".to_owned(),
                },
            );
            return;
        }
        self.check_option_ownership(claim, reference, &example.options);
        self.check_example_option_reality(claim, &example.options);
    }

    /// Reports example options the owning reference does not declare.
    fn check_option_ownership(
        &mut self,
        claim: ClaimRef<'_>,
        reference: &'a SkillCommandRef,
        options: &'a [OptionUse],
    ) {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for option in options {
            if seen.insert(option.name.as_str())
                && !reference.flags.iter().any(|flag| flag == &option.name)
            {
                self.push_structural(
                    claim,
                    FindingData {
                        code: codes::EXAMPLE_UNDECLARED_OPTION,
                        option: Some(option.name.clone()),
                        message: format!(
                            "example option '{}' is not declared by its command reference",
                            option.name
                        ),
                    },
                );
            }
        }
    }

    /// Reports example options the real command does not accept, then policy.
    fn check_example_option_reality(&mut self, claim: ClaimRef<'_>, options: &'a [OptionUse]) {
        let mut seen_names: BTreeSet<&str> = BTreeSet::new();
        let mut seen_uses: BTreeSet<(&str, Option<&str>)> = BTreeSet::new();
        for option in options {
            let accepted = self
                .index
                .command(claim.path)
                .is_some_and(|command| command.accepts(&option.name));
            if seen_names.insert(option.name.as_str()) && !accepted {
                self.push_structural(
                    claim,
                    FindingData {
                        code: codes::EXAMPLE_UNKNOWN_OPTION,
                        option: Some(option.name.clone()),
                        message: format!(
                            "example option '{}' is not accepted by '{}'",
                            option.name,
                            join_path(claim.path)
                        ),
                    },
                );
            }
            if accepted && seen_uses.insert((option.name.as_str(), option.value.as_deref())) {
                self.push_advice(
                    claim,
                    option_use_advice(
                        self.policy,
                        claim.path,
                        &option.name,
                        option.value.as_deref(),
                    ),
                );
            }
        }
    }
}
