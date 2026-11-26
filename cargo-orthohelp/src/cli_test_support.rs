//! Shared CLI and package-selection fixtures for binary-crate unit tests.
//!
//! Locale selection and generation tests construct the same argument and
//! selection values, so those builders live here rather than being copied into
//! each test module. The module is test-only and is not part of the library
//! target, whose module set intentionally omits the binary's CLI and metadata
//! adapters.

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use tempfile::TempDir;

use crate::cli::{Args, CacheArgs, ManArgs, OutputFormat, PowerShellArgs};
use crate::metadata::{OrthoConfigDependency, PackageSelection};

/// Builds CLI arguments with every optional flag at its default.
pub(crate) fn base_args() -> Args {
    Args {
        package: None,
        bin: None,
        is_lib: false,
        root_type: None,
        locale: Vec::new(),
        should_use_all_locales: false,
        out_dir: None,
        cache: CacheArgs {
            should_cache: false,
            should_skip_build: false,
        },
        format: OutputFormat::Ir,
        man: ManArgs {
            section: 1,
            date: None,
            should_split_subcommands: false,
        },
        powershell: PowerShellArgs {
            module_name: None,
            should_split_subcommands: None,
            should_include_common_parameters: None,
            help_info_uri: None,
            should_ensure_en_us: true,
        },
        check_agent_native: false,
        policy_mode: None,
    }
}

/// Builds a package selection rooted at `package_root`.
pub(crate) fn selection_with_locales(
    package_root: Utf8PathBuf,
    locales: Option<Vec<String>>,
) -> PackageSelection {
    PackageSelection {
        package_name: "demo".to_owned(),
        package_root: package_root.clone(),
        target_directory: package_root,
        package_version: "0.1.0".to_owned(),
        root_type: "demo::Config".to_owned(),
        locales,
        windows: None,
        policy: None,
        ortho_config_dependency: OrthoConfigDependency {
            requirement: "^0.7.0".to_owned(),
            path: None,
        },
    }
}

/// An isolated package root that owns its temporary directory.
///
/// The guard keeps the tree alive until the test ends, so fixtures can hand out
/// a borrowed path while callers add catalogues or bare locale directories.
pub(crate) struct PackageRoot {
    /// Kept until the test ends so the package tree survives every assertion.
    _temp_dir: TempDir,
    /// UTF-8 path to the package root.
    pub(crate) path: Utf8PathBuf,
}

impl PackageRoot {
    /// Creates an empty package root under a fresh temporary directory.
    pub(crate) fn new() -> Self {
        let temp_dir = TempDir::new().expect("create a temporary package root");
        let path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .expect("temporary package root should be UTF-8");
        Self {
            _temp_dir: temp_dir,
            path,
        }
    }

    /// Writes one Fluent catalogue below `locales/<locale>`.
    pub(crate) fn add_catalogue(&self, locale: &str, body: &str) {
        let dir = self.open();
        dir.create_dir_all(format!("locales/{locale}"))
            .expect("create the locale directory");
        dir.write(format!("locales/{locale}/messages.ftl"), body)
            .expect("write the Fluent catalogue");
    }

    /// Creates an empty directory below `locales/`.
    pub(crate) fn add_locale_dir(&self, locale: &str) {
        self.open()
            .create_dir_all(format!("locales/{locale}"))
            .expect("create the locale directory");
    }

    /// Writes a regular file at `path` relative to the package root.
    pub(crate) fn write_file(&self, path: &str, body: &str) {
        self.open()
            .write(path, body)
            .expect("write the package file");
    }

    /// Opens the package root through a capability handle.
    fn open(&self) -> Dir {
        Dir::open_ambient_dir(self.path.as_path(), ambient_authority())
            .expect("open the temporary package root")
    }
}

/// One captured `tracing` event, reduced to its recorded fields.
#[derive(Debug, Default)]
pub(crate) struct CapturedEvent {
    /// Recorded fields, including `message` and any structured fields.
    fields: std::collections::BTreeMap<String, String>,
}

impl CapturedEvent {
    /// Reads a field, or the empty string when the event did not record it.
    pub(crate) fn field(&self, name: &str) -> &str {
        self.fields.get(name).map_or("", String::as_str)
    }

    /// Reports whether the event's message contains `needle`.
    pub(crate) fn message_contains(&self, needle: &str) -> bool {
        self.field("message").contains(needle)
    }
}

/// Process-wide capture state shared by one global subscriber.
#[derive(Default)]
struct CaptureSink {
    /// Threads currently inside [`capture_events`].
    active: std::sync::Mutex<std::collections::HashSet<std::thread::ThreadId>>,
    /// Recorded events, tagged with the emitting thread.
    events: std::sync::Mutex<Vec<(std::thread::ThreadId, CapturedEvent)>>,
}

static CAPTURE: std::sync::OnceLock<std::sync::Arc<CaptureSink>> = std::sync::OnceLock::new();

/// Installs the process-wide capture subscriber if it is not installed yet.
///
/// A global subscriber is required because `tracing` caches callsite interest
/// globally: a callsite first reached without a subscriber can stay disabled
/// for later thread-local subscribers. Recording is still scoped by thread, so
/// concurrent tests cannot observe one another's events.
pub(crate) fn ensure_capture_subscriber() {
    capture_sink();
}

/// Returns the shared capture state, installing the subscriber on first use.
fn capture_sink() -> &'static std::sync::Arc<CaptureSink> {
    use std::sync::{Arc, PoisonError};
    use tracing_subscriber::layer::Context;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{Layer, Registry};

    struct CaptureLayer(Arc<CaptureSink>);

    struct Visitor<'a>(&'a mut std::collections::BTreeMap<String, String>);

    impl tracing::field::Visit for Visitor<'_> {
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            self.0.insert(field.name().to_owned(), value.to_owned());
        }

        fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
            self.0.insert(field.name().to_owned(), value.to_string());
        }

        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.0.insert(field.name().to_owned(), format!("{value:?}"));
        }
    }

    impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
        fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
            let thread = std::thread::current().id();
            let active = self.0.active.lock().unwrap_or_else(PoisonError::into_inner);
            if !active.contains(&thread) {
                return;
            }
            drop(active);
            let mut captured = CapturedEvent::default();
            event.record(&mut Visitor(&mut captured.fields));
            self.0
                .events
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((thread, captured));
        }
    }

    CAPTURE.get_or_init(|| {
        let sink = Arc::new(CaptureSink::default());
        let subscriber = Registry::default().with(CaptureLayer(Arc::clone(&sink)));
        drop(tracing::subscriber::set_global_default(subscriber));
        sink
    })
}

/// Captures every event emitted by `emit` on the calling thread.
///
/// Events from concurrent tests are left for their own captures. The global
/// subscriber is installed before `emit` runs, so callsite interest is enabled.
pub(crate) fn capture_events(emit: impl FnOnce()) -> Vec<CapturedEvent> {
    use std::sync::PoisonError;

    let sink = capture_sink();
    let thread = std::thread::current().id();
    sink.active
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(thread);
    emit();
    sink.active
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&thread);
    let mut events = sink.events.lock().unwrap_or_else(PoisonError::into_inner);
    let mut captured = Vec::new();
    let mut remaining = Vec::new();
    for (event_thread, event) in events.drain(..) {
        if event_thread == thread {
            captured.push(event);
        } else {
            remaining.push((event_thread, event));
        }
    }
    *events = remaining;
    captured
}
