//! The narrow procfs boundary the artefact-lock report reads through.
//!
//! Every kernel observation the diagnostic report makes goes through
//! [`ProcFs`]: the lock table, a process's descriptor table, one descriptor's
//! identity, and a process's `/proc/<pid>/stat` line. Confining those reads to
//! one trait is what lets a test drive the report's *decisions* — what a hold
//! means, which scan counts as complete — without needing a real held lock, a
//! real fork, or a real unreadable `/proc` entry. Before this boundary existed
//! those decisions could only be reached by arranging filesystem topology, so
//! the failure branches that matter most were the ones hardest to exercise.
//!
//! [`RealProcFs`] is the only implementation that touches `std::fs` on `/proc`.
//! It is not a general-purpose filesystem abstraction and it is not public: it
//! exists for the test-only diagnostics described in
//! [`super::lock_diagnostics`], and its scope is recorded in the developers'
//! guide. The production path constructs one [`RealProcFs`] and hands it to the
//! reporting functions; nothing else chooses an implementation.
//!
//! The methods answer in the vocabulary of their *callers* rather than of the
//! kernel. [`ProcFs::descriptor_entries`] returns the descriptor numbers and
//! the names it could not parse, because "there is no descriptor here" and "I
//! could not tell what this was" are different answers and the scan must not
//! confuse them. A failure carries whatever was read before it, for the same
//! reason: an unrelated descriptor closing mid-scan must not discard the
//! holder the scan had already matched.

use std::fs;
use std::path::Path;

use super::lock_evidence::{
    Descriptor, DeviceNumber, Inode, LockFileIdentity, LockRecord, ProcessId, parse_lock_record,
};

/// Where the kernel publishes its lock table on Linux.
///
/// Shared with `lock_diagnostics`, which names the file in the lines it writes
/// when the table has no record for the inode it is explaining.
#[cfg(target_os = "linux")]
pub(super) const PROC_LOCKS: &str = "/proc/locks";

/// The kernel observations the artefact-lock report is built from.
///
/// One method per observation, so a test can fail exactly the read it needs to
/// fail and leave the others answering normally. A single "read a path" method
/// would be simpler to implement and would make the report's branches
/// indistinguishable from each other in a fake.
pub(super) trait ProcFs {
    /// Reads and parses the kernel's lock table.
    ///
    /// The failure is text rather than an error type: the caller is already
    /// composing a failure report, and a diagnosis that could not be gathered
    /// must appear *inside* that report rather than replacing it.
    fn lock_table(&self) -> Result<Vec<LockRecord>, String>;

    /// Lists the descriptor numbers under `pid`'s descriptor table.
    ///
    /// Names that are not descriptor numbers are returned in the second half
    /// rather than dropped. Every entry the kernel writes there is a decimal
    /// number, so a name that will not parse is evidence about the *scan*, not
    /// about the lock: the scan reports it and still examines the rest.
    ///
    /// A listing that stops part-way is returned with the entries it yielded
    /// and the reason it stopped, never as a bare failure: the entries were
    /// really read, and the same reasoning that keeps a matched descriptor in
    /// an incomplete scan keeps them here.
    ///
    /// The directory's own handle is handed back too. When the scan is of
    /// *this* process, `/proc/self/fd` lists the very descriptor the listing
    /// was read through; dropping the listing before the descriptors are
    /// identified closes it, and its entry then reads as unreadable. A
    /// self-scan would report an incomplete scan with no application
    /// descriptor having changed — the intermittent failure this module
    /// exists to diagnose, manufactured by the diagnostic itself. The caller
    /// holds the handle until it has read every identity.
    fn descriptor_entries(&self, pid: ProcessId) -> Result<DescriptorListing, String>;

    /// Reads the file identity one descriptor of `pid` refers to.
    ///
    /// A failure is text for the same reason [`ProcFs::lock_table`]'s is.
    fn descriptor_identity(
        &self,
        pid: ProcessId,
        descriptor: Descriptor,
    ) -> Result<LockFileIdentity, String>;

    /// Reads `pid`'s command name and parent from `/proc/<pid>/stat`.
    ///
    /// Returns `None` where the kernel has nothing to say about that pid, which
    /// is ordinary: a process can exit between the record that names it and the
    /// read that would describe it. The caller degrades the sentence it writes
    /// rather than failing the report.
    fn process_stat(&self, pid: ProcessId) -> Option<ProcessStat>;
}

/// What one process's descriptor table yielded.
///
/// The split between `descriptors` and `unparsed` is the point of the type.
/// A descriptor number is a fact the scan can act on; a name the scan could not
/// parse is a gap in its coverage. Reporting the second as the first would
/// claim a descriptor that never existed, and dropping it silently would hide
/// that the scan saw something it did not understand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DescriptorEntries {
    /// The descriptor numbers the directory listed.
    descriptors: Vec<Descriptor>,
    /// Names that are not descriptor numbers, kept so a report can say so.
    unparsed: Vec<String>,
}

impl DescriptorEntries {
    /// Names the descriptor numbers and unparsed names one listing yielded.
    pub(super) const fn new(descriptors: Vec<Descriptor>, unparsed: Vec<String>) -> Self {
        Self {
            descriptors,
            unparsed,
        }
    }

    /// Returns the descriptor numbers, in ascending order.
    pub(super) fn descriptors(&self) -> &[Descriptor] {
        &self.descriptors
    }

    /// Returns the names that are not descriptor numbers.
    ///
    /// The scan reports these rather than failing on them: a name the kernel
    /// would not have written is a gap in the scan's *coverage*, and a report
    /// that stayed silent about it would claim a complete scan it did not
    /// perform. It is not a *fault*, so it does not make the scan incomplete
    /// either — the descriptors it did read are still all of them.
    pub(super) fn unparsed(&self) -> &[String] {
        &self.unparsed
    }
}

/// A listing, or what it yielded before it failed.
///
/// `registry` is the iterator the listing was read through, held past the
/// exchange for the reason [`ProcFs::descriptor_entries`] describes.
pub(super) struct DescriptorListing {
    /// The entries the directory yielded, whether or not it finished.
    pub(super) entries: DescriptorEntries,
    /// Why the enumeration stopped early; `None` when it reached the end.
    pub(super) fault: Option<String>,
    /// The open directory iterator, which owns the descriptor a self-scan sees.
    pub(super) registry: Option<fs::ReadDir>,
}

/// What `/proc/<pid>/stat` says about one process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ProcessStat {
    /// The command name the kernel records, truncated to 15 bytes by `comm`.
    command: String,
    /// The pid of the process that forked this one.
    parent: ProcessId,
}

impl ProcessStat {
    /// Names the command and parent one stat line yielded.
    pub(super) const fn new(command: String, parent: ProcessId) -> Self {
        Self { command, parent }
    }

    /// Returns the process's command name.
    pub(super) fn command(&self) -> &str {
        &self.command
    }

    /// Returns the pid that forked the process.
    pub(super) const fn parent(&self) -> ProcessId {
        self.parent
    }
}

/// The Linux implementation: the only place that reads procfs.
///
/// A unit struct rather than a struct with state because the reads are
/// stateless; it exists as a value so the reporting functions can take *an*
/// implementation rather than reach for the filesystem themselves.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RealProcFs;

/// Reads one directory's entries, keeping what it yielded if it stops early.
///
/// An enumeration can fail after it has already yielded entries, and returning
/// only the error would discard every entry the read did produce — including,
/// potentially, the one the report exists to name. The entries are therefore
/// returned alongside the fault, and the caller marks the scan incomplete
/// rather than carrying on as though the table were empty.
///
/// Sorted because `read_dir` yields entries in no defined order. A scan that
/// failed part-way through an unsorted listing would report whichever
/// descriptors happened to precede the failure, making both the report and any
/// test of it depend on filesystem enumeration order.
///
/// The open iterator is returned with the entries, because the descriptor it
/// holds is the one a self-scan would otherwise find closed. Opening a second
/// handle to keep alive would not do: the iterator opens its own, and *that* is
/// the number the listing reports. See [`ProcFs::descriptor_entries`].
#[cfg(target_os = "linux")]
fn read_listing(directory: &Path) -> Result<DescriptorListing, String> {
    let mut walker = fs::read_dir(directory)
        .map_err(|error| format!("{}: unreadable ({error})", directory.display()))?;
    let mut entries = Vec::new();
    let mut fault = None;
    loop {
        match walker.next() {
            Some(Ok(entry)) => entries.push(entry),
            Some(Err(error)) => {
                fault = Some(format!(
                    "{}: an entry could not be read ({error})",
                    directory.display()
                ));
                break;
            }
            None => break,
        }
    }
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(DescriptorListing {
        entries: split_descriptor_names(&entries),
        fault,
        registry: Some(walker),
    })
}

/// Returns the descriptor numbers an entry list names, and the names it cannot.
///
/// A name that is not valid UTF-8, or is not a decimal number, is recorded in
/// the second half rather than treated as a fault or dropped. The scan decides
/// what to do with that; this function only reports what it saw.
#[cfg(target_os = "linux")]
fn split_descriptor_names(entries: &[fs::DirEntry]) -> DescriptorEntries {
    let mut descriptors = Vec::new();
    let mut unparsed = Vec::new();
    for entry in entries {
        match entry.file_name().into_string() {
            Ok(name) => match name.parse().map(Descriptor::new) {
                Ok(descriptor) => descriptors.push(descriptor),
                Err(_) => unparsed.push(name),
            },
            // Not valid UTF-8, so it cannot name a descriptor. Reported by its
            // lossy rendering: the raw bytes would not survive the report.
            Err(name) => unparsed.push(name.to_string_lossy().into_owned()),
        }
    }
    DescriptorEntries::new(descriptors, unparsed)
}

#[cfg(target_os = "linux")]
impl ProcFs for RealProcFs {
    fn lock_table(&self) -> Result<Vec<LockRecord>, String> {
        fs::read_to_string(PROC_LOCKS)
            .map(|table| table.lines().filter_map(parse_lock_record).collect())
            .map_err(|error| format!("{PROC_LOCKS}: unreadable ({error})"))
    }

    fn descriptor_entries(&self, pid: ProcessId) -> Result<DescriptorListing, String> {
        read_listing(Path::new(&format!("/proc/{pid}/fd")))
    }

    fn descriptor_identity(
        &self,
        pid: ProcessId,
        descriptor: Descriptor,
    ) -> Result<LockFileIdentity, String> {
        use std::os::unix::fs::MetadataExt;

        // The entries under `/proc/<pid>/fd` are symlinks, and `metadata`
        // follows them, so this reads the identity of the open file rather than
        // of the link that points at it.
        let path = format!("/proc/{pid}/fd/{descriptor}");
        let metadata =
            fs::metadata(&path).map_err(|error| format!("{path}: unreadable ({error})"))?;
        Ok(LockFileIdentity::of(
            DeviceNumber::from_raw(metadata.dev()),
            Inode::new(metadata.ino()),
        ))
    }

    fn process_stat(&self, pid: ProcessId) -> Option<ProcessStat> {
        parse_stat(&fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
    }
}

/// Parses one `/proc/<pid>/stat` line into its command and parent.
///
/// The command name sits in parentheses and may itself contain spaces or
/// parentheses, so the fields after it are located from the *last* closing
/// parenthesis rather than by splitting the whole line: splitting would make a
/// process called `a) b` look like a different process entirely.
#[cfg(target_os = "linux")]
fn parse_stat(stat: &str) -> Option<ProcessStat> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let command = stat.get(open.saturating_add(1)..close)?.to_owned();
    let mut trailing = stat.get(close.saturating_add(1)..)?.split_whitespace();
    // Past the command name the first field is the state and the second the
    // parent pid.
    let _state = trailing.next()?;
    let parent = ProcessId::new(trailing.next()?.parse().ok()?);
    Some(ProcessStat::new(command, parent))
}
