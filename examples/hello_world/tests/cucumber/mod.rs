//! Cucumber test harness for the `hello_world` example.

use std::time::Duration;

use camino::Utf8PathBuf;
use cucumber::{World as _, gherkin};

mod config;
#[path = "../steps/mod.rs"]
mod steps;
mod world;

pub use config::SampleConfigError;
pub use world::World;

pub(crate) const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const CONFIG_FILE: &str = ".hello_world.toml";
pub(crate) const ENV_PREFIX: &str = "HELLO_WORLD_";

fn binary_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!(
        "CARGO_BIN_EXE_hello_world",
        "Cargo must set the hello_world binary path for integration tests",
    ))
}

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
fn requires_yaml(
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

#[tokio::main]
async fn main() {
    let yaml_enabled = cfg!(feature = "yaml");
    #[cfg(test)]
    requires_yaml_tests::run();
    World::cucumber()
        .filter_run("tests/features", move |feature, rule, scenario| {
            yaml_enabled || !requires_yaml(feature, rule, scenario)
        })
        .await;
}

#[cfg(test)]
mod requires_yaml_tests {
    use super::*;

    pub(super) fn run() {
        detects_multi_tag_yaml_scenarios();
        detects_yaml_tag_on_rule();
        returns_false_when_no_yaml_tags_present();
    }

    fn feature_with_tags(tags: &[&str]) -> gherkin::Feature {
        gherkin::Feature {
            keyword: String::new(),
            name: String::new(),
            description: None,
            background: None,
            scenarios: Vec::new(),
            rules: Vec::new(),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            span: gherkin::Span::default(),
            position: gherkin::LineCol::default(),
            path: None,
        }
    }

    fn rule_with_tags(tags: &[&str]) -> gherkin::Rule {
        gherkin::Rule {
            keyword: String::new(),
            name: String::new(),
            description: None,
            background: None,
            scenarios: Vec::new(),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            span: gherkin::Span::default(),
            position: gherkin::LineCol::default(),
        }
    }

    fn scenario_with_tags(tags: &[&str]) -> gherkin::Scenario {
        gherkin::Scenario {
            keyword: String::new(),
            name: String::new(),
            description: None,
            steps: Vec::new(),
            examples: Vec::new(),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            span: gherkin::Span::default(),
            position: gherkin::LineCol::default(),
        }
    }

    fn detects_multi_tag_yaml_scenarios() {
        let feature = feature_with_tags(&[]);
        let scenario = scenario_with_tags(&["slow", "requires.yaml"]);
        assert!(requires_yaml(&feature, None, &scenario));
    }

    fn detects_yaml_tag_on_rule() {
        let feature = feature_with_tags(&[]);
        let rule = rule_with_tags(&["requires.yaml", "other"]);
        let scenario = scenario_with_tags(&["fast"]);
        assert!(requires_yaml(&feature, Some(&rule), &scenario));
    }

    fn returns_false_when_no_yaml_tags_present() {
        let feature = feature_with_tags(&["external"]);
        let scenario = scenario_with_tags(&["slow"]);
        assert!(!requires_yaml(&feature, None, &scenario));
    }
}
