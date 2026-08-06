//! Minimal thread-local tracing capture for discovery unit tests.
//!
//! The integration suites have a fuller harness under `tests/support`; unit
//! tests cannot reach it, and only need "which events fired with which
//! bounded fields", so this stays deliberately small. `with_default` is
//! thread-local, so tests using it run concurrently without a lock.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Layer, Registry};

/// One captured event, reduced to its recorded fields.
#[derive(Debug, Default, Clone)]
pub(super) struct CapturedEvent {
    fields: BTreeMap<String, String>,
}

impl CapturedEvent {
    /// Read a field, or the empty string when the event did not record it.
    pub(super) fn field(&self, name: &str) -> &str {
        self.fields.get(name).map_or("", String::as_str)
    }
}

struct CaptureLayer(Arc<Mutex<Vec<CapturedEvent>>>);

impl<S> Layer<S> for CaptureLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        let mut captured = CapturedEvent::default();
        event.record(&mut FieldVisitor {
            fields: &mut captured.fields,
        });
        if let Ok(mut events) = self.0.lock() {
            events.push(captured);
        }
    }
}

struct FieldVisitor<'fields> {
    fields: &'fields mut BTreeMap<String, String>,
}

impl Visit for FieldVisitor<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), value.to_owned());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}"));
    }
}

/// Run `f` with a capturing subscriber installed on this thread only.
pub(super) fn capture_events<R>(f: impl FnOnce() -> R) -> Vec<CapturedEvent> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = Registry::default().with(CaptureLayer(Arc::clone(&events)));
    tracing::subscriber::with_default(subscriber, f);
    // Moved out rather than cloned, matching the integration harness: the
    // subscriber is gone, so nothing else can observe the vector again.
    events
        .lock()
        .map(|mut guard| std::mem::take(&mut *guard))
        .unwrap_or_default()
}
