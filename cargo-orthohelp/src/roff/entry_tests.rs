//! Check the discovery section renderer.
//!
//! `render_discovery_section` is the only place a man page states where the
//! application looks for configuration, so each of its three parts has to
//! appear when the metadata supplies it and stay out when it does not. The
//! cases below pin both directions, including the order the formats are
//! listed in, because that order is the documented precedence.

use super::render_discovery_section;

use crate::ir::{LocalizedConfigDiscoveryMeta, LocalizedPathPattern};
use crate::schema::ConfigFormat;

/// Discovery metadata carrying only the parts a case cares about.
fn discovery(
    formats: Vec<ConfigFormat>,
    search_paths: Vec<LocalizedPathPattern>,
    xdg_compliant: bool,
) -> LocalizedConfigDiscoveryMeta {
    LocalizedConfigDiscoveryMeta {
        formats,
        search_paths,
        override_flag_long: None,
        override_env: None,
        xdg_compliant,
    }
}

/// A search path with an optional explanatory note.
fn pattern(pattern: &str, note: Option<&str>) -> LocalizedPathPattern {
    LocalizedPathPattern {
        pattern: pattern.to_owned(),
        note: note.map(str::to_owned),
    }
}

#[test]
fn every_search_path_is_rendered_with_its_note() {
    let rendered = render_discovery_section(&discovery(
        Vec::new(),
        vec![
            pattern("$XDG_CONFIG_HOME/demo/config.toml", Some("Per-user config")),
            pattern("./demo.toml", None),
        ],
        false,
    ));

    assert!(
        rendered.contains("\\fI$XDG_CONFIG_HOME/demo/config.toml\\fR"),
        "the first pattern is rendered in italics: {rendered}"
    );
    assert!(
        rendered.contains("Per-user config"),
        "a pattern's note follows it: {rendered}"
    );
    assert!(
        // A leading dot would be read as a roff request, so the escaping
        // inserts `\&` before it; the path itself must survive intact.
        rendered.contains("\\fI\\&./demo.toml\\fR"),
        "a pattern without a note is still rendered: {rendered}"
    );
    assert_eq!(
        rendered.matches(".TP\n").count(),
        2,
        "each search path opens its own tagged paragraph: {rendered}"
    );
}

#[test]
fn supported_formats_are_listed_in_metadata_order() {
    let rendered = render_discovery_section(&discovery(
        vec![ConfigFormat::Yaml, ConfigFormat::Toml, ConfigFormat::Json],
        Vec::new(),
        false,
    ));

    assert!(
        rendered.contains("Supported formats: YAML, TOML, JSON."),
        "the formats are named in the order the metadata lists them: {rendered}"
    );
    assert!(
        !rendered.contains("XDG"),
        "a package that does not claim XDG compliance must not say it does: {rendered}"
    );
}

#[test]
fn xdg_compliance_is_stated_only_when_claimed() {
    let rendered = render_discovery_section(&discovery(Vec::new(), Vec::new(), true));

    assert!(
        rendered.contains("XDG Base Directory specification"),
        "a compliant package states what discovery follows: {rendered}"
    );
    assert!(
        !rendered.contains("Supported formats"),
        "no format sentence is emitted when the metadata names no format: {rendered}"
    );
}

#[test]
fn empty_discovery_metadata_renders_nothing() {
    let rendered = render_discovery_section(&discovery(Vec::new(), Vec::new(), false));

    assert!(
        rendered.is_empty(),
        "metadata that claims nothing produces no prose: {rendered}"
    );
}
