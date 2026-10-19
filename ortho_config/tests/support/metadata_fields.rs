//! Metadata-level assertions for the discovery event contract.
//!
//! The neighbouring capture harnesses record only *values*, so they cannot see
//! the defect this module guards. `tracing` stores a callsite's field *names*
//! in its static metadata at the macro, whether or not a value follows:
//! recording a `None` suppresses the value, not the name. A subscriber
//! filtering on `Metadata::fields` would therefore observe a collapsed emitter
//! as though the field were always present.
//!
//! `docs/design.md` promises the opposite for three events — `scope` is
//! "omitted on the flat first-wins walk", `source` is "omitted for
//! `policy_resolve`", and `selector_class` is "omitted when no selector
//! matched". Each case asserts on the callsite's name set, which is the only
//! layer at which the promise is either kept or broken.
//!
//! The set is read from `event.metadata()`, not from what the visitor records.
//! Two calls to the same emitter share one callsite, so the set is stable
//! across runs and between the arms of a conditional.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Layer, Registry};

/// One captured event: its `event` name and its callsite's declared names.
#[derive(Debug)]
pub struct CapturedMetadata {
    event: Option<String>,
    names: BTreeSet<&'static str>,
}

impl CapturedMetadata {
    /// Whether the callsite declares `name` at all.
    pub fn declares(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// The declared field names, for failure messages that show what was seen.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.names.iter().copied()
    }
}

/// Reads the `event` field's value so captured events can be matched by name.
#[derive(Default)]
struct NameVisitor {
    event: Option<String>,
}

impl Visit for NameVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "event" {
            self.event = Some(value.to_owned());
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}
}

#[derive(Default)]
struct Events(Mutex<Vec<CapturedMetadata>>);

impl Events {
    fn lock(&self) -> MutexGuard<'_, Vec<CapturedMetadata>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

struct CaptureLayer(Arc<Events>);

impl<S> Layer<S> for CaptureLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        let mut visitor = NameVisitor::default();
        event.record(&mut visitor);
        let record = CapturedMetadata {
            event: visitor.event,
            names: event
                .metadata()
                .fields()
                .iter()
                .map(|field| field.name())
                .collect(),
        };
        self.0.lock().push(record);
    }
}

/// Keeps a dispatcher registered so the global max level cannot drop to `OFF`.
///
/// The same hazard the value-capture harness documents: `tracing::debug!`
/// consults a global maximum before reaching the thread-local subscriber, and
/// a concurrent thread leaving its scope can lower it. Holding one dispatcher
/// for the binary's lifetime pins the level without installing it as a default.
fn pin_global_max_level() {
    static PINNED: std::sync::OnceLock<tracing::Dispatch> = std::sync::OnceLock::new();
    let _ = PINNED.get_or_init(|| tracing::Dispatch::new(Registry::default()));
}

/// Run `f` with a metadata-capturing subscriber installed on this thread only.
pub fn capture_metadata<R>(f: impl FnOnce() -> R) -> Vec<CapturedMetadata> {
    pin_global_max_level();
    let events = Arc::new(Events::default());
    let subscriber = Registry::default().with(CaptureLayer(Arc::clone(&events)));
    tracing::subscriber::with_default(subscriber, f);
    std::mem::take(&mut *events.lock())
}

/// Exactly one event named `name`; panics with the observed set otherwise.
pub fn only_metadata<'a>(events: &'a [CapturedMetadata], name: &str) -> &'a CapturedMetadata {
    let matching: Vec<_> = events
        .iter()
        .filter(|event| event.event.as_deref() == Some(name))
        .collect();
    match matching.as_slice() {
        [event] => event,
        other => panic!("expected exactly one `{name}` event, got {}", other.len()),
    }
}

/// Every event named `name`.
pub fn all_metadata<'a>(events: &'a [CapturedMetadata], name: &str) -> Vec<&'a CapturedMetadata> {
    events
        .iter()
        .filter(|event| event.event.as_deref() == Some(name))
        .collect()
}

/// Write `content` to `dir/name` through a capability handle.
///
/// The repository's lint suite forbids `std::fs` writes, and a capability
/// handle additionally names the directory it may touch, so a fixture cannot
/// escape the temporary tree it was given.
pub fn write_fixture(
    dir: &std::path::Path,
    name: &str,
    content: &str,
) -> anyhow::Result<std::path::PathBuf> {
    use cap_std::{ambient_authority, fs::Dir};
    let cap = Dir::open_ambient_dir(dir, ambient_authority())?;
    cap.write(name, content.as_bytes())?;
    Ok(dir.join(name))
}

/// Create `relative` beneath `base`, returning the resulting directory.
///
/// Goes through the same capability handle as [`write_fixture`] for the same
/// reason.
pub fn make_dir(base: &std::path::Path, relative: &str) -> anyhow::Result<std::path::PathBuf> {
    use cap_std::{ambient_authority, fs::Dir};
    let cap = Dir::open_ambient_dir(base, ambient_authority())?;
    cap.create_dir_all(relative)?;
    Ok(base.join(relative))
}
