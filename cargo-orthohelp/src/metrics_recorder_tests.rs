//! Check that the `tracing` recorder exports every metric kind.
//!
//! The recorder is the CLI's only metrics export path, so its contract is the
//! event each registered handle emits. These cases register a handle with the
//! recorder and read what it logged, which pins the `kind` verb, the metric
//! identity, and the label rendering. They drive the recorder directly rather
//! than installing it process-wide: the `metrics` facade has one global slot,
//! and stealing it here would decide whether another test's export works.

use super::{TracingRecorder, render_labels};

use metrics::{
    Counter, Gauge, Histogram, Key, Label, Level, Metadata, Recorder, SharedString, Unit,
};
use std::io;
use std::sync::{Arc, Mutex};

/// Metric name the bridge cache emits, reused so the events name a real metric.
const CACHE_EVENT_METRIC: &str = "cargo_orthohelp.bridge_cache.events";

/// Writer that collects a subscriber's rendered output for assertions.
#[derive(Clone)]
struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl CapturedWriter {
    /// An empty capture buffer.
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    /// The text rendered so far.
    fn text(&self) -> String {
        let captured = self
            .0
            .lock()
            .expect("captured tracing output is readable")
            .clone();
        String::from_utf8(captured).expect("tracing output is UTF-8")
    }
}

impl io::Write for CapturedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        // A poisoned lock means another test thread panicked mid-write; report
        // it as a write failure rather than panicking the subscriber.
        let mut captured = self
            .0
            .lock()
            .map_err(|_| io::Error::other("captured tracing output is poisoned"))?;
        captured.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Run `emit` under a capturing subscriber and return what it logged.
///
/// The subscriber is thread-local, so a concurrent case cannot capture these
/// events and these cases cannot see another's.
fn captured(emit: impl FnOnce()) -> String {
    let writer = CapturedWriter::new();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer({
            // The subscriber needs its own handle; the case reads the capture
            // through the original once the events have been emitted.
            let sink = writer.clone();
            move || sink.clone()
        })
        .finish();

    tracing::subscriber::with_default(subscriber, emit);
    writer.text()
}

/// The key the bridge cache registers its event counter with.
fn cache_event_key() -> Key {
    Key::from_parts(
        CACHE_EVENT_METRIC,
        vec![
            Label::from_static_parts("operation", "publish"),
            Label::from_static_parts("outcome", "hit"),
        ],
    )
}

/// Metadata the recorder ignores but must accept.
fn metadata() -> Metadata<'static> {
    Metadata::new("cargo_orthohelp::metrics", Level::INFO, None)
}

/// A counter registered through the recorder under test.
fn registered_counter() -> Counter {
    TracingRecorder.register_counter(&cache_event_key(), &metadata())
}

/// A gauge registered through the recorder under test.
fn registered_gauge() -> Gauge {
    TracingRecorder.register_gauge(&cache_event_key(), &metadata())
}

/// A histogram registered through the recorder under test.
fn registered_histogram() -> Histogram {
    TracingRecorder.register_histogram(&cache_event_key(), &metadata())
}

#[test]
fn a_registered_counter_reports_an_increment() {
    let output = captured(|| registered_counter().increment(3));

    assert!(
        output.contains(CACHE_EVENT_METRIC),
        "the event names the metric: {output}"
    );
    assert!(
        output.contains("kind=\"counter_increment\""),
        "the event names the verb that produced it: {output}"
    );
    assert!(
        output.contains("value=3"),
        "the counter keeps its exact value: {output}"
    );
}

#[test]
fn a_registered_counter_reports_an_absolute_value() {
    let output = captured(|| registered_counter().absolute(9));

    assert!(
        output.contains("kind=\"counter_absolute\""),
        "an absolute set is reported as itself, not as an increment: {output}"
    );
    assert!(
        output.contains("value=9"),
        "the value is reported: {output}"
    );
}

#[test]
fn a_registered_gauge_reports_increment_decrement_and_set() {
    let output = captured(|| {
        let gauge = registered_gauge();
        gauge.increment(1.5);
        gauge.decrement(0.5);
        gauge.set(2.0);
    });

    for (kind, value) in [
        ("gauge_increment", "value=1.5"),
        ("gauge_decrement", "value=0.5"),
        ("gauge_set", "value=2"),
    ] {
        assert!(
            output.contains(&format!("kind=\"{kind}\"")),
            "a gauge update reports {kind}: {output}"
        );
        assert!(
            output.contains(value),
            "a gauge update reports its value: {output}"
        );
    }
}

#[test]
fn a_registered_histogram_reports_its_record() {
    let output = captured(|| registered_histogram().record(0.25));

    assert!(
        output.contains("kind=\"histogram_record\""),
        "a recorded observation names its verb: {output}"
    );
    assert!(
        output.contains("value=0.25"),
        "the recorded value survives rendering: {output}"
    );
}

#[test]
fn an_exported_event_carries_the_registered_labels() {
    let output = captured(|| registered_counter().increment(1));

    assert!(
        output.contains("operation=publish") && output.contains("outcome=hit"),
        "the event carries the labels the metric was registered with: {output}"
    );
}

#[test]
fn descriptions_are_accepted_and_log_nothing() {
    let recorder = TracingRecorder;
    let key = Key::from_name(CACHE_EVENT_METRIC);
    let name = key.name_shared();
    let description = SharedString::from("bridge cache outcomes");

    let output = captured(|| {
        recorder.describe_counter(name.clone(), Some(Unit::Count), description.clone());
        recorder.describe_gauge(name.clone(), Some(Unit::Count), description.clone());
        recorder.describe_histogram(name.clone(), None, description.clone());
    });

    assert!(
        output.is_empty(),
        "the emitter logs updates; a description is not one: {output}"
    );
}

#[test]
fn labels_render_as_space_separated_pairs() {
    assert_eq!(
        render_labels(&cache_event_key()),
        "operation=publish outcome=hit",
        "the exported label field is a flat, parseable string"
    );
}

#[test]
fn a_key_without_labels_renders_an_empty_field() {
    assert!(
        render_labels(&Key::from_name(CACHE_EVENT_METRIC)).is_empty(),
        "an unlabelled metric must not render a placeholder label"
    );
}
