//! Production metrics export for the standalone `cargo-orthohelp` binary.
//!
//! The `metrics` facade discards every update unless the process installs a
//! recorder, so the bridge cache counters would be silent in the CLI without
//! this module. It installs a recorder that turns each update into one
//! structured `tracing` event, which reaches the same stderr writer and
//! `RUST_LOG` filter the rest of the binary already uses.
//!
//! A richer sink, such as an OpenTelemetry or Prometheus exporter, is a host
//! decision rather than a CLI one: [`install`] claims the process-wide slot
//! only when it is free, so a host that installs its own recorder first keeps
//! it. The labels carried by these events come from the bridge cache's closed
//! enums, so the exported label set stays bounded.

use std::fmt;
use std::sync::Arc;

use metrics::{
    Counter, CounterFn, Gauge, GaugeFn, Histogram, HistogramFn, Key, KeyName, Metadata, Recorder,
    SharedString, Unit,
};

/// Install the `tracing`-backed recorder as the process-wide metrics sink.
///
/// The `metrics` facade has one global recorder slot. A refused install means
/// a host already owns that slot, so no export is lost and there is nothing to
/// report.
pub(crate) fn install() {
    let _refused = metrics::set_global_recorder(TracingRecorder);
}

/// Metrics recorder that reports each update as a `tracing` event.
struct TracingRecorder;

impl Recorder for TracingRecorder {
    fn describe_counter(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn describe_gauge(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn register_counter(&self, key: &Key, _metadata: &Metadata<'_>) -> Counter {
        Counter::from_arc(Arc::new(TracingMetric::new(key)))
    }

    fn register_gauge(&self, key: &Key, _metadata: &Metadata<'_>) -> Gauge {
        Gauge::from_arc(Arc::new(TracingMetric::new(key)))
    }

    fn register_histogram(&self, key: &Key, _metadata: &Metadata<'_>) -> Histogram {
        Histogram::from_arc(Arc::new(TracingMetric::new(key)))
    }
}

/// One registered metric, holding the identity it was registered with.
///
/// Registration repeats per emission because this recorder keeps no cache, so
/// the identity is copied into the handle rather than borrowed from the key.
struct TracingMetric {
    /// Metric name as the emitter registered it.
    metric: String,
    /// Rendered `name=value` label set, bounded by the emitters' closed enums.
    labels: String,
}

impl TracingMetric {
    /// Capture the name and labels of a registered metric.
    fn new(key: &Key) -> Self {
        Self {
            metric: key.name().to_owned(),
            labels: render_labels(key),
        }
    }

    /// Report one update, naming the verb that produced it.
    ///
    /// Every metric kind shares one event shape, so a consumer parses `kind`
    /// rather than guessing the payload type from the metric family. The value
    /// is rendered exactly as the emitter supplied it: counters keep integer
    /// precision without a lossy conversion to `f64`.
    fn report(&self, kind: &'static str, value: impl fmt::Display) {
        tracing::debug!(
            target: "cargo_orthohelp::metrics",
            metric = %self.metric,
            labels = %self.labels,
            kind,
            value = %value,
            "metric updated"
        );
    }
}

impl CounterFn for TracingMetric {
    fn increment(&self, value: u64) {
        self.report("counter_increment", value);
    }

    fn absolute(&self, value: u64) {
        self.report("counter_absolute", value);
    }
}

impl GaugeFn for TracingMetric {
    fn increment(&self, value: f64) {
        self.report("gauge_increment", value);
    }

    fn decrement(&self, value: f64) {
        self.report("gauge_decrement", value);
    }

    fn set(&self, value: f64) {
        self.report("gauge_set", value);
    }
}

impl HistogramFn for TracingMetric {
    fn record(&self, value: f64) {
        self.report("histogram_record", value);
    }
}

/// Render a key's labels as space-separated `name=value` pairs.
fn render_labels(key: &Key) -> String {
    key.labels()
        .map(|label| format!("{}={}", label.key(), label.value()))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[path = "metrics_recorder_tests.rs"]
mod tests;
