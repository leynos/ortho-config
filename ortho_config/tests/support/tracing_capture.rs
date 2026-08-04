//! Shared tracing-capture harness for the discovery test suites.
//!
//! Extracted so the telemetry suite stays under the repository's 400-line
//! module limit, and so neighbouring discovery tests can capture events
//! without growing a second implementation — two copies of a capture path
//! drift in field rendering and lock handling.

use anyhow::Context as _;
use cap_std::{ambient_authority, fs::Dir};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Layer, Registry};

/// One captured `tracing` event, reduced to its recorded fields.
#[derive(Debug, Default)]
pub struct Captured {
    pub fields: BTreeMap<String, String>,
}

impl Captured {
    /// Read a field, or the empty string when the event did not record it.
    pub fn field(&self, name: &str) -> &str {
        self.fields.get(name).map_or("", String::as_str)
    }
}

#[derive(Default)]
pub struct Events(Mutex<Vec<Captured>>);

impl Events {
    fn lock(&self) -> MutexGuard<'_, Vec<Captured>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub struct CaptureLayer(Arc<Events>);

impl<S> Layer<S> for CaptureLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        let mut captured = Captured::default();
        event.record(&mut FieldVisitor {
            fields: &mut captured.fields,
        });
        self.0.lock().push(captured);
    }
}

pub struct FieldVisitor<'fields> {
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

/// Keep one registered dispatcher alive for the lifetime of the test binary.
///
/// `tracing::debug!` consults a *global* maximum level before it ever reaches
/// the thread-local subscriber, and that maximum is recomputed as dispatchers
/// register and drop. With tests running concurrently, one thread leaving its
/// `with_default` scope could drop the global maximum to `OFF` while another
/// was still inside its own — and the second thread's events would vanish.
///
/// This was observed as a genuine intermittent failure, not a theoretical one.
/// Holding a dispatcher that is never dropped pins the maximum at the registry
/// default, so no thread can lower it out from under another. It is deliberately
/// never installed as the default subscriber: each test still captures through
/// its own thread-local one.
pub fn pin_global_max_level() {
    static PINNED: OnceLock<tracing::Dispatch> = OnceLock::new();
    let _ = PINNED.get_or_init(|| tracing::Dispatch::new(Registry::default()));
}

/// Run `f` with a capturing subscriber installed on this thread only.
///
/// `with_default` is thread-local, so these tests need no lock and no
/// `#[serial]` — the same property the injected environment source provides
/// for the environment itself.
pub fn capture<R>(f: impl FnOnce() -> R) -> Vec<Captured> {
    pin_global_max_level();
    let events = Arc::new(Events::default());
    let subscriber = Registry::default().with(CaptureLayer(Arc::clone(&events)));
    tracing::subscriber::with_default(subscriber, f);
    std::mem::take(&mut *events.lock())
}

/// Every captured event carrying the given `event` name.
pub fn named<'a>(events: &'a [Captured], name: &str) -> Vec<&'a Captured> {
    events
        .iter()
        .filter(|event| event.field("event") == name)
        .collect()
}

/// Exactly one event with `name` must have been emitted; return it.
pub fn only<'a>(events: &'a [Captured], name: &str) -> &'a Captured {
    let matching = named(events, name);
    match matching.as_slice() {
        [event] => event,
        other => panic!("expected exactly one `{name}` event, got {other:?}"),
    }
}

/// Write a minimal loadable configuration file into `dir`, returning its path.
///
/// The write goes through a `cap_std::fs::Dir` handle rather than `std::fs`,
/// which the repository's lint suite requires: a capability handle names the
/// directory it may touch, so a fixture cannot accidentally write relative to
/// the process's working directory.
pub fn write_fixture(dir: &std::path::Path, name: &str) -> anyhow::Result<std::path::PathBuf> {
    let cap =
        Dir::open_ambient_dir(dir, ambient_authority()).context("open the temporary directory")?;
    cap.write(name, b"value = 1\n")
        .context("write the fixture")?;
    Ok(dir.join(name))
}
