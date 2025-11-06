//! Cucumber test harness for the `hello_world` example.

use std::time::Duration;

use camino::Utf8PathBuf;
use cucumber::World as _;
use tag_filters::has_yaml_requirement;

mod config;
#[path = "../steps/mod.rs"]
mod steps;
mod tag_filters;
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

#[tokio::main]
async fn main() {
    let yaml_enabled = cfg!(feature = "yaml");
    World::cucumber()
        .filter_run("tests/features", move |feature, rule, scenario| {
            yaml_enabled || !has_yaml_requirement(feature, rule, scenario)
        })
        .await;
}

#[cfg(test)]
mod tests {
    use cucumber::gherkin;
    use rstest::{fixture, rstest};

    #[fixture]
    fn feature(#[default(&[])] tags: &[&str]) -> gherkin::Feature {
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

    #[fixture]
    fn rule(#[default(&[])] tags: &[&str]) -> gherkin::Rule {
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

    #[fixture]
    fn scenario(#[default(&[])] tags: &[&str]) -> gherkin::Scenario {
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

    #[rstest]
    fn detects_multi_tag_yaml_scenarios(
        feature: gherkin::Feature,
        #[with(&["slow", "requires.yaml"])] scenario: gherkin::Scenario,
    ) {
        assert!(super::has_yaml_requirement(&feature, None, &scenario));
    }

    #[rstest]
    fn detects_yaml_tag_on_rule(
        feature: gherkin::Feature,
        #[with(&["requires.yaml", "other"])] rule: gherkin::Rule,
        scenario: gherkin::Scenario,
    ) {
        assert!(super::has_yaml_requirement(
            &feature,
            Some(&rule),
            &scenario
        ));
    }

    #[rstest]
    fn returns_false_when_no_yaml_tags_present(
        #[with(&["external"])] feature: gherkin::Feature,
        #[with(&["slow"])] scenario: gherkin::Scenario,
    ) {
        assert!(!super::has_yaml_requirement(&feature, None, &scenario));
    }
}
