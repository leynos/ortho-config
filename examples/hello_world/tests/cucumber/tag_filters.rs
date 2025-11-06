//! Tag filters used by the Cucumber harness.

use cucumber::gherkin;

/// Detect whether a feature, rule, or scenario includes the `@requires.yaml`
/// tag so YAML-dependent scenarios can be skipped when that Cargo feature is
/// disabled.
///
/// # Parameters
/// - `feature`: Feature whose tags may enable YAML requirements.
/// - `rule`: Optional rule that may contribute additional tags.
/// - `scenario`: Scenario under evaluation.
///
/// # Returns
/// `true` when any supplied tags equal `requires.yaml`, otherwise `false`.
pub(crate) fn requires_yaml(
    feature: &gherkin::Feature,
    rule: Option<&gherkin::Rule>,
    scenario: &gherkin::Scenario,
) -> bool {
    const TAG: &str = "requires.yaml";
    feature
        .tags
        .iter()
        .chain(rule.into_iter().flat_map(|r| r.tags.iter()))
        .chain(scenario.tags.iter())
        .any(|tag| tag == TAG)
}
