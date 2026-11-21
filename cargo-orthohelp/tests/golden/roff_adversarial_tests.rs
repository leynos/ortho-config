//! Adversarial renderer tests for roff man page generation.
//!
//! Issue #435 reported that a metadata value carrying a line break could start
//! a roff request of its own, and that command names were written without any
//! macro-safe escaping. These tests render complete pages from hostile
//! metadata, so a regression in any single sink is caught at the layer a
//! reader would see rather than only inside the encoder.
//!
//! The payload names a marker that only a *request* could execute: `.tm`
//! writes to the processor's standard error and `.so` tries to include a file.
//! A page that merely renders the payload as text executes neither, so the
//! check is that every request line belongs to the renderer's own vocabulary.

use cargo_orthohelp::ir::{
    LocalizedConfigDiscoveryMeta, LocalizedDeprecation, LocalizedDocMetadata, LocalizedExample,
    LocalizedFieldMetadata, LocalizedHeadings, LocalizedLink, LocalizedPathPattern,
    LocalizedPrecedenceMeta,
};
use cargo_orthohelp::roff::{RoffConfig, generate_to_string};
use cargo_orthohelp::schema::{
    CliMetadata, ConfigFormat, DefaultValue, EnvMetadata, FileMetadata, SourceKind, ValueType,
};
use insta::assert_snapshot;
use rstest::rstest;

use super::roff_tests::{default_headings, make_test_metadata};

/// Marker that distinguishes an executed request from rendered text.
const INJECTION_MARKER: &str = "ROFF-INJECTION-MARKER";

/// The request names the renderer is allowed to emit.
const RENDERED_REQUESTS: &[&str] = &[
    ".TH", ".SH", ".SS", ".TP", ".PP", ".RS", ".RE", ".IP", ".B", ".BR", ".UR", ".UE", ".nf",
    ".fi", ".br",
];

/// A value that tries to start requests with LF, CR, and CRLF separators.
fn injection_payload() -> String {
    format!("value\r\u{0}.tm {INJECTION_MARKER}\n\u{0}.so /nonexistent-{INJECTION_MARKER}")
}

/// Renders a page from default metadata after the case applies its payload.
fn render_adversarial(mutate: impl FnOnce(&mut LocalizedDocMetadata, &mut RoffConfig)) -> String {
    let mut metadata = make_test_metadata(default_headings(), "test-app", "A test application.");
    let mut config = RoffConfig::default();
    mutate(&mut metadata, &mut config);
    generate_to_string(&metadata, &config)
}

/// Returns whether `ch` is a control character groff discards while reading.
///
/// This mirrors the encoder's own rule. The oracle must apply it, because a
/// line that begins with a discarded byte is judged by groff on its *next*
/// character: `\0.so /etc/passwd` is read as the `.so` request. A naive
/// first-character check would accept that line and the test would pass while
/// the page was exploitable.
fn is_discarded_by_groff(ch: char) -> bool {
    ch.is_control() && !matches!(ch, '\t' | '\n' | '\r')
}

/// Returns the line as groff would see it, with discarded characters removed.
fn as_groff_reads_it(line: &str) -> String {
    line.chars()
        .filter(|ch| !is_discarded_by_groff(*ch))
        .collect()
}

/// Lists request lines that the renderer does not itself emit.
///
/// Returning the offending lines rather than a boolean lets a failure quote
/// exactly what escaped, which is what distinguishes a neutralized payload
/// from one that became `.TM` after the name was uppercased.
fn unexpected_request_lines(rendered: &str) -> Vec<String> {
    rendered
        .lines()
        .filter_map(|line| {
            let seen = as_groff_reads_it(line);
            if !seen.starts_with(['.', '\'']) {
                return None;
            }
            let name = seen.split_whitespace().next().unwrap_or(&seen).to_owned();
            (!RENDERED_REQUESTS.contains(&name.as_str())).then_some(seen)
        })
        .collect()
}

/// Asserts that every request line belongs to the renderer's own vocabulary.
fn assert_only_expected_requests(rendered: &str) {
    let injected = unexpected_request_lines(rendered);
    assert!(
        injected.is_empty(),
        "an untrusted value became a roff request: {injected:?}\n{rendered}"
    );
}

/// A backslash in a command name must reach the page as a literal backslash.
///
/// A macro argument is interpreted once more than body text, so a doubled
/// backslash would be re-read as the start of an escape: `.B tool\\fBtail`
/// switches the font mid-name. The `\(rs` glyph survives that extra layer.
#[test]
fn backslash_in_command_name_survives_as_a_glyph() {
    let rendered = render_adversarial(|metadata, _| {
        metadata.bin_name = Some(r"tool\fBtail".to_owned());
    });

    assert!(
        rendered.contains(".B tool\\(rsfBtail\n"),
        "the synopsis should carry the glyph-encoded name: {rendered}"
    );
    assert!(
        !rendered.contains(r"tool\\fBtail"),
        "a doubled backslash would be re-read as an escape: {rendered}"
    );
}

/// One untrusted metadata sink exercised by the adversarial cases.
///
/// A sink is wherever a metadata value is stored before the renderer encodes
/// it, so the table below covers each syntactic slot the encoders serve.
#[derive(Clone, Copy, Debug)]
enum Sink {
    AppName,
    BinName,
    About,
    Synopsis,
    Headings,
    LinkUri,
    TitleDate,
    TitleSource,
    TitleManual,
}

impl Sink {
    /// Writes `payload` into this sink.
    fn apply(self, metadata: &mut LocalizedDocMetadata, config: &mut RoffConfig, payload: &str) {
        match self {
            Self::AppName => payload.clone_into(&mut metadata.app_name),
            Self::BinName => metadata.bin_name = Some(payload.to_owned()),
            Self::About => payload.clone_into(&mut metadata.about),
            Self::Synopsis => metadata.synopsis = Some(payload.to_owned()),
            Self::Headings => {
                payload.clone_into(&mut metadata.sections.headings.name);
                payload.clone_into(&mut metadata.sections.headings.commands);
            }
            Self::LinkUri => {
                metadata.sections.links = vec![LocalizedLink {
                    text: Some("link text".to_owned()),
                    uri: format!("https://example.invalid/{payload}"),
                }];
            }
            Self::TitleDate => config.date = Some(payload.to_owned()),
            Self::TitleSource => config.source = Some(payload.to_owned()),
            Self::TitleManual => config.manual = Some(payload.to_owned()),
        }
    }
}

#[rstest]
#[case(Sink::AppName)]
#[case(Sink::BinName)]
#[case(Sink::About)]
#[case(Sink::Synopsis)]
#[case(Sink::Headings)]
#[case(Sink::LinkUri)]
#[case(Sink::TitleDate)]
#[case(Sink::TitleSource)]
#[case(Sink::TitleManual)]
fn each_sink_encodes_the_payload(#[case] sink: Sink) {
    let payload = injection_payload();
    let rendered = render_adversarial(|metadata, config| sink.apply(metadata, config, &payload));

    assert_only_expected_requests(&rendered);
    assert!(
        rendered.contains(INJECTION_MARKER),
        "the {sink:?} sink should keep the payload as escaped text: {rendered}"
    );
}

/// Metadata whose every rendered field carries the injection payload.
///
/// The whole page is rendered as one case so no sink is left untested by
/// accident, and so the snapshot pins the encoded form of each one.
fn adversarial_metadata() -> LocalizedDocMetadata {
    let payload = injection_payload();
    let mut metadata = make_test_metadata(default_headings(), &payload, &payload);
    metadata.bin_name = Some(payload.clone());
    metadata.synopsis = Some(payload.clone());
    metadata.sections.headings = LocalizedHeadings {
        name: payload.clone(),
        synopsis: payload.clone(),
        description: payload.clone(),
        options: payload.clone(),
        environment: payload.clone(),
        files: payload.clone(),
        precedence: payload.clone(),
        exit_status: payload.clone(),
        examples: payload.clone(),
        see_also: payload.clone(),
        commands: payload.clone(),
    };
    metadata.sections.discovery = Some(LocalizedConfigDiscoveryMeta {
        formats: vec![ConfigFormat::Toml],
        search_paths: vec![LocalizedPathPattern {
            pattern: payload.clone(),
            note: Some(payload.clone()),
        }],
        override_flag_long: None,
        override_env: None,
        xdg_compliant: true,
    });
    metadata.sections.precedence = Some(LocalizedPrecedenceMeta {
        order: vec![SourceKind::Defaults, SourceKind::Cli],
        rationale: Some(payload.clone()),
    });
    metadata.sections.examples = vec![LocalizedExample {
        title: Some(payload.clone()),
        code: payload.clone(),
        body: Some(payload.clone()),
    }];
    metadata.sections.links = vec![LocalizedLink {
        text: Some(payload.clone()),
        uri: format!("https://example.invalid/{payload}"),
    }];
    metadata.fields = vec![adversarial_field(&payload)];
    metadata
}

/// A field whose every rendered facet carries the payload.
fn adversarial_field(payload: &str) -> LocalizedFieldMetadata {
    LocalizedFieldMetadata {
        name: "mode".to_owned(),
        help: payload.to_owned(),
        long_help: Some(payload.to_owned()),
        value: Some(ValueType::Enum {
            variants: vec![payload.to_owned()],
        }),
        default: Some(DefaultValue {
            display: payload.to_owned(),
        }),
        required: false,
        deprecated: Some(LocalizedDeprecation {
            note: payload.to_owned(),
        }),
        cli: Some(CliMetadata {
            long: Some(payload.to_owned()),
            short: Some('"'),
            value_name: Some(payload.to_owned()),
            multiple: false,
            takes_value: true,
            value_optional: false,
            possible_values: vec![payload.to_owned()],
            hide_in_help: false,
        }),
        env: Some(EnvMetadata {
            var_name: payload.to_owned(),
        }),
        file: Some(FileMetadata {
            key_path: payload.to_owned(),
        }),
        examples: vec![],
        links: vec![],
        notes: vec![],
    }
}

/// Configuration whose title-header fields all carry the payload.
fn adversarial_config() -> RoffConfig {
    let payload = injection_payload();
    RoffConfig {
        date: Some(payload.clone()),
        source: Some(payload.clone()),
        manual: Some(payload.clone()),
        ..RoffConfig::default()
    }
}

#[test]
fn every_sink_together_cannot_inject_requests() {
    let rendered = generate_to_string(&adversarial_metadata(), &adversarial_config());
    assert_only_expected_requests(&rendered);
    assert!(
        rendered.contains(INJECTION_MARKER),
        "the payload should survive as escaped text: {rendered}"
    );
}

/// Asserts that the oracle catches a raw, unescaped request.
///
/// Without this control the oracle could accept everything and the sink cases
/// would prove nothing.
#[test]
fn oracle_rejects_a_raw_request_line() {
    let injected = unexpected_request_lines(".TH \"T\" \"1\"\n.so /etc/passwd\n");

    assert_eq!(
        injected,
        vec![".so /etc/passwd".to_owned()],
        "the oracle should name the unescaped request"
    );
}

/// Asserts that the oracle catches a request hidden behind a discarded byte.
///
/// Groff drops the NUL and then reads `.so`, so the oracle must do the same or
/// this exact bypass would pass unnoticed.
#[test]
fn oracle_rejects_a_nul_prefixed_request_line() {
    let injected = unexpected_request_lines(".TH \"T\" \"1\"\n\u{0}.so /etc/passwd\n");

    assert_eq!(
        injected,
        vec![".so /etc/passwd".to_owned()],
        "a discarded byte must not hide the request that follows it"
    );
}

/// Asserts that escaped text is accepted, so the oracle fails closed only.
#[test]
fn oracle_accepts_neutralized_text() {
    let injected = unexpected_request_lines(".TH \"T\" \"1\"\n\\&.so /etc/passwd\n");

    assert!(
        injected.is_empty(),
        "neutralized text should not be reported as a request: {injected:?}"
    );
}

#[test]
fn adversarial_page_snapshot() {
    let rendered = generate_to_string(&adversarial_metadata(), &adversarial_config());
    assert_snapshot!("adversarial_roff_page", rendered);
}
