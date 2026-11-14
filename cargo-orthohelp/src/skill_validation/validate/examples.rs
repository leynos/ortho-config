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
use super::super::policy_check::{UseSite, option_use_advice};
use super::super::report::codes;
use super::super::resolve::{CommandPath, OptionName, join_path};
use super::manifest_index::OwnerKey;
use super::{ClaimRef, FindingData, Validator, reference_at};

/// One example option that resolved, as the policy rules need it.
#[derive(Clone, Copy)]
struct ResolvedUse<'a> {
    /// Option name the example used.
    name: &'a str,
    /// Whether the surface classifies this option as global.
    is_global: bool,
}

impl<'a> Validator<'a> {
    /// Checks one normalized example.
    ///
    /// Returns whether the example reached its option checks. An unsupported
    /// extraction, an unresolvable owner, or a broken binding chain reports its
    /// own finding and returns `false`, so the report never implies that
    /// dependent checks ran.
    pub(super) fn check_example(&mut self, example: &'a NormalizedExample) -> bool {
        if example.extraction == ExampleExtraction::Unsupported {
            return false;
        }
        let claim = Self::example_claim(usize::MAX, example);
        match self.manifests.owner(&example.manifest_id) {
            OwnerKey::Missing => {
                self.push_structural(
                    claim,
                    FindingData::structural(
                        codes::EXAMPLE_UNKNOWN_MANIFEST,
                        None,
                        format!("manifest ID '{}' is not declared", example.manifest_id),
                    ),
                );
                false
            }
            OwnerKey::Ambiguous(order) => {
                self.push_structural(
                    Self::example_claim(order, example),
                    FindingData::structural(
                        codes::EXAMPLE_AMBIGUOUS_MANIFEST,
                        None,
                        format!(
                            "manifest ID '{}' is declared more than once; \
                             the example owner is ambiguous",
                            example.manifest_id
                        ),
                    ),
                );
                false
            }
            OwnerKey::Unique(order) => {
                let manifests = self.manifests.manifests;
                manifests
                    .get(order)
                    .is_some_and(|manifest| self.check_bound_example(order, manifest, example))
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
    ///
    /// Returns whether the example reached its option checks.
    fn check_bound_example(
        &mut self,
        order: usize,
        manifest: &'a ManifestClaims,
        example: &'a NormalizedExample,
    ) -> bool {
        let claim = Self::example_claim(order, example);
        let Some(reference) = reference_at(manifest, example.command_reference_ordinal) else {
            self.push_structural(
                claim,
                FindingData::structural(
                    codes::EXAMPLE_UNBOUND_REFERENCE,
                    None,
                    "example does not select a declared command reference".to_owned(),
                ),
            );
            return false;
        };
        if example.path != reference.path {
            self.push_structural(
                claim,
                FindingData::structural(
                    codes::EXAMPLE_PATH_MISMATCH,
                    None,
                    "example path differs from its owning command reference".to_owned(),
                ),
            );
            return false;
        }
        self.check_example_options(claim, reference, &example.options)
    }

    /// Checks each distinct option name once, then applies policy to resolved
    /// uses.
    ///
    /// The checks form a chain: an option its owning reference does not declare
    /// is reported once and not re-examined, for the same reason a reference
    /// with an unknown path does not cascade into flag findings. The owning
    /// reference is the claim the example elaborates, so a second finding about
    /// the real command would repeat one offending token. A *declared* option
    /// the command does not accept is reported, and only a use that is both
    /// declared and accepted counts as resolved for policy.
    ///
    /// Returns whether every option was checked. When the surface does not
    /// declare the reference's path, no option can be accepted and the
    /// remaining checks do not run. That path is already reported once against
    /// the manifest, so this adds no second finding; it only keeps the example
    /// out of `examples_bound`, because claiming it was validated would
    /// overstate coverage.
    fn check_example_options(
        &mut self,
        claim: ClaimRef<'_>,
        reference: &'a SkillCommandRef,
        options: &'a [OptionUse],
    ) -> bool {
        let mut seen_names: BTreeSet<&str> = BTreeSet::new();
        for option in options {
            if !seen_names.insert(option.name.as_str()) {
                continue;
            }
            if !reference.flags.iter().any(|flag| flag == &option.name) {
                self.push_structural(
                    claim,
                    FindingData::structural(
                        codes::EXAMPLE_UNDECLARED_OPTION,
                        Some(option.name.clone()),
                        format!(
                            "example option '{}' is not declared by its command reference",
                            option.name
                        ),
                    ),
                );
                continue;
            }
            let Some(resolved) = self.index.command(CommandPath::new(claim.path)) else {
                return false;
            };
            let Some(resolved_option) = resolved.option(OptionName::new(&option.name)) else {
                self.push_structural(
                    claim,
                    FindingData::structural(
                        codes::EXAMPLE_UNKNOWN_OPTION,
                        Some(option.name.clone()),
                        format!(
                            "example option '{}' is not accepted by '{}'",
                            option.name,
                            join_path(CommandPath::new(claim.path))
                        ),
                    ),
                );
                continue;
            };
            let resolved_use = ResolvedUse {
                name: &option.name,
                is_global: resolved_option.is_global(),
            };
            self.apply_option_policy(claim, options, resolved_use);
        }
        true
    }

    /// Applies policy once per distinct use of one resolved option.
    ///
    /// Only a use that is both declared and accepted reaches this point, so a
    /// claim the example already contradicted never also receives advice.
    fn apply_option_policy(
        &mut self,
        claim: ClaimRef<'_>,
        options: &'a [OptionUse],
        resolved: ResolvedUse<'_>,
    ) {
        let mut seen_uses: BTreeSet<(&str, Option<&str>)> = BTreeSet::new();
        for use_site in options
            .iter()
            .filter(|candidate| candidate.name == resolved.name)
        {
            let value = use_site.value.as_deref();
            if !seen_uses.insert((use_site.name.as_str(), value)) {
                continue;
            }
            let site = UseSite {
                path: claim.path,
                name: &use_site.name,
                value,
                is_global: resolved.is_global,
            };
            self.push_advice(claim, option_use_advice(self.policy, &site));
        }
    }
}
