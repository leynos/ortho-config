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
///
/// # Example
/// ```rust,ignore
/// use cucumber::gherkin;
///
/// let feature = gherkin::Feature::default();
/// let scenario = gherkin::Scenario::default();
/// assert!(!has_yaml_requirement(&feature, None, &scenario));
/// ```
pub(crate) fn has_yaml_requirement(
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
        .any(|tag| tag.as_str() == TAG)
}
