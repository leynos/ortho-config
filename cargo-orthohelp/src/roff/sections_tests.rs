//! Unit tests for roff section rendering.

use super::*;
use rstest::{fixture, rstest};

#[fixture]
fn headings() -> LocalizedHeadings {
    LocalizedHeadings {
        name: "NAME".to_owned(),
        synopsis: "SYNOPSIS".to_owned(),
        description: "DESCRIPTION".to_owned(),
        options: "OPTIONS".to_owned(),
        environment: "ENVIRONMENT".to_owned(),
        files: "FILES".to_owned(),
        precedence: "PRECEDENCE".to_owned(),
        exit_status: "EXIT STATUS".to_owned(),
        examples: "EXAMPLES".to_owned(),
        see_also: "SEE ALSO".to_owned(),
        commands: "COMMANDS".to_owned(),
    }
}

#[test]
fn title_header_formats_correctly() {
    let metadata = TitleMetadata::new(Some("2026-01-31"), Some("v1.0"), Some("User Commands"));
    let section = ManSection::new(1).expect("valid section");
    let result = title_header("my-app", section, &metadata);
    assert!(result.starts_with(".TH \"MY-APP\" \"1\""));
    assert!(result.contains("2026-01-31"));
    assert!(result.contains("v1.0"));
    assert!(result.contains("User Commands"));
}

#[rstest]
fn name_section_escapes_description(headings: LocalizedHeadings) {
    let result = name_section(&headings, "my-app", "A -test application");
    assert!(result.contains("my-app \\- A -test application"));

    let leading_dash_result = name_section(&headings, "my-app", "-starts with dash");
    assert!(leading_dash_result.contains("my-app \\- \\-starts with dash"));
}

#[rstest]
fn precedence_section_orders_sources(headings: LocalizedHeadings) {
    let prec = LocalizedPrecedenceMeta {
        order: vec![
            SourceKind::Defaults,
            SourceKind::File,
            SourceKind::Env,
            SourceKind::Cli,
        ],
        rationale: None,
    };
    let result = precedence_section(&headings, Some(&prec));
    assert!(result.contains(".IP 1. 4\nBuilt-in defaults"));
    assert!(result.contains(".IP 4. 4\nCommand-line arguments"));
}

/// A profile-opted-in command reports the profile overlay as tier three,
/// between configuration files and the environment.
#[rstest]
fn precedence_section_places_profiles_third(headings: LocalizedHeadings) {
    let prec = LocalizedPrecedenceMeta {
        order: vec![
            SourceKind::Defaults,
            SourceKind::File,
            SourceKind::Profile,
            SourceKind::Env,
            SourceKind::Cli,
        ],
        rationale: None,
    };
    let result = precedence_section(&headings, Some(&prec));
    assert!(result.contains(".IP 3. 4\nProfile overlays"), "{result}");
    assert!(
        result.contains(".IP 4. 4\nEnvironment variables"),
        "{result}"
    );
    assert!(
        result.contains(".IP 5. 4\nCommand-line arguments"),
        "{result}"
    );
}
