//! The pure vocabulary a lock-holder report is built from.
//!
//! Two conversions stand between a kernel lock record and a report a maintainer
//! can act on: splitting glibc's packed `dev_t` into the major and minor halves
//! `/proc/locks` prints, and parsing the kernel's record line into named
//! fields. Both are pure, so both are tested on every platform even where
//! `/proc/locks` does not exist — and both fail silently in the same way. A
//! mis-split device matches no record, and a mis-parsed line is skipped, so
//! either defect reads as "nothing holds this lock", which is exactly the wrong
//! answer to give a caller investigating a hold.
//!
//! Nothing here reads the filesystem. The scan that feeds these functions lives
//! in `artefact_lock_diagnostics`, and the `/proc/<pid>/stat` lookup lives in
//! `artefact_lock_process`.
//!
//! The newtypes below give the kernel's bare integers a domain language. A pid,
//! an inode, and a descriptor number all arrive as `u32` or `u64`, so a
//! signature built from bare integers cannot stop a caller transposing two of
//! them — and a wrong pid in a holder report reads as a confident accusation of
//! an innocent process.

use std::fmt;

/// A process identifier, as `/proc` names it.
///
/// A holder report must never name the wrong process: a transposed pid reads as
/// a confident accusation of an innocent one. The wrappers below give each
/// quantity its own type so the compiler rejects the swap.
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ProcessId(u32);

#[cfg(any(target_os = "linux", test))]
impl ProcessId {
    /// Names the process the kernel identifies by `raw`.
    pub(super) const fn new(raw: u32) -> Self {
        Self(raw)
    }
}

#[cfg(any(target_os = "linux", test))]
impl fmt::Display for ProcessId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raw = self.0;
        write!(formatter, "{raw}")
    }
}

/// The inode of the locked file, unique within its device.
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Inode(u64);

#[cfg(any(target_os = "linux", test))]
impl Inode {
    /// Names the inode the kernel reports as `raw`.
    pub(super) const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

#[cfg(any(target_os = "linux", test))]
impl fmt::Display for Inode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raw = self.0;
        write!(formatter, "{raw}")
    }
}

/// An open descriptor number within one process's descriptor table.
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Descriptor(u32);

#[cfg(any(target_os = "linux", test))]
impl Descriptor {
    /// Names the descriptor the `/proc/<pid>/fd` entry is called.
    pub(super) const fn new(raw: u32) -> Self {
        Self(raw)
    }
}

#[cfg(any(target_os = "linux", test))]
impl fmt::Display for Descriptor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raw = self.0;
        write!(formatter, "{raw}")
    }
}

/// A device number split into the major and minor halves `/proc/locks` prints.
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DeviceNumber {
    /// The device's major number, which selects the driver.
    major: u64,
    /// The device's minor number, which selects the unit within the driver.
    minor: u64,
}

#[cfg(any(target_os = "linux", test))]
impl DeviceNumber {
    /// Builds a device number from the halves the kernel prints.
    ///
    /// The halves come from a `/proc/locks` record, which prints them in hex,
    /// rather than from a raw `st_dev`; [`Self::from_raw`] is the other route.
    pub(super) const fn new(major: u64, minor: u64) -> Self {
        Self { major, minor }
    }

    /// Splits a raw `st_dev` value into the halves `/proc/locks` prints.
    ///
    /// glibc packs the major's low 12 bits at bit 8 and its high 20 bits at bit
    /// 44, and the minor's low 8 bits at bit 0 and its high 24 bits at bit 20.
    /// The masks below are those four fields, so each half is the masked low
    /// part shifted down plus the masked high part shifted down.
    pub(super) const fn from_raw(raw: u64) -> Self {
        /// Bits 8..20 of the raw value: the major's low 12 bits.
        const MAJOR_LOW: u64 = 0x0000_0000_000f_ff00;
        /// Bits 44..64 of the raw value: the major's high 20 bits.
        const MAJOR_HIGH: u64 = 0xffff_f000_0000_0000;
        /// Bits 0..8 of the raw value: the minor's low 8 bits.
        const MINOR_LOW: u64 = 0x0000_0000_0000_00ff;
        /// Bits 20..44 of the raw value: the minor's high 24 bits.
        const MINOR_HIGH: u64 = 0x0000_0fff_fff0_0000;
        Self {
            major: (raw & MAJOR_LOW) >> 8 | (raw & MAJOR_HIGH) >> 32,
            minor: raw & MINOR_LOW | (raw & MINOR_HIGH) >> 12,
        }
    }
}

#[cfg(any(target_os = "linux", test))]
impl fmt::Display for DeviceNumber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The kernel prints each half as `%02x`, so single digits keep a
        // leading zero and wider values are never truncated.
        write!(formatter, "{:02x}:{:02x}", self.major, self.minor)
    }
}

/// The file a lock is held on, as the kernel reports it.
///
/// A lock record names a file by the pair (device, inode), and every question
/// the report asks about a holder — which records match, which descriptors
/// refer to it — needs both halves together. Passing them as two arguments let
/// a caller transpose them, and made every signature read as two unrelated
/// numbers rather than one file.
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LockFileIdentity {
    /// The device holding the file, split into the halves the kernel prints.
    device: DeviceNumber,
    /// The inode of the locked file, unique within that device.
    inode: Inode,
}

#[cfg(any(target_os = "linux", test))]
impl LockFileIdentity {
    /// Names the file that `device` and `inode` identify.
    pub(super) const fn of(device: DeviceNumber, inode: Inode) -> Self {
        Self { device, inode }
    }

    /// Returns whether `device` and `inode` name this same file.
    ///
    /// Not a `const fn`: the comparison is derived `PartialEq`, which is not
    /// usable in a constant context. Nothing here needs one.
    pub(super) fn holds(&self, device: DeviceNumber, inode: Inode) -> bool {
        self.device == device && self.inode == inode
    }
}

/// One record from the kernel's lock table.
///
/// The format is `N: TYPE MODE ACCESS PID MAJ:MIN:INODE START END`. A blocked
/// record repeats its blocker's ordinal behind one or more `->` markers, and
/// the kernel pads `POSIX` to six characters, so fields are split on runs of
/// whitespace rather than read from fixed columns.
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LockRecord {
    /// Whether the record describes a waiter rather than the holder.
    blocked: bool,
    /// The kernel's lock type: `FLOCK`, `POSIX`, `OFDLCK`, and so on.
    kind: String,
    /// Whether the lock is `ADVISORY` or `MANDATORY`.
    mode: String,
    /// Whether the lock is `READ` or `WRITE`.
    access: String,
    /// The acquiring process, or `None` where the kernel reports none.
    ///
    /// `OFDLCK` records carry pid `-1`, because an open-file-description lock
    /// is not owned by any single process and no pid would be truthful.
    pid: Option<ProcessId>,
    /// The device of the locked file, split into the halves the kernel prints.
    device: DeviceNumber,
    /// The inode of the locked file, in decimal.
    inode: Inode,
    /// The lock's byte range as the kernel prints it, such as `0 EOF`.
    range: String,
}

#[cfg(any(target_os = "linux", test))]
impl LockRecord {
    /// Returns whether this record describes the same file as `identity`.
    pub(super) fn matches(&self, identity: LockFileIdentity) -> bool {
        identity.holds(self.device, self.inode)
    }

    /// Reports the pid that owns this record, and whether it is a waiter.
    ///
    /// A blocked record names the process *waiting* for the lock, so it never
    /// owns anything; and an `OFDLCK` record names no process at all. Both cases
    /// answer `None`, which is what keeps a waiter from being reported as a
    /// holder.
    pub(super) fn owning_pid(&self) -> Option<ProcessId> {
        if self.blocked { None } else { self.pid }
    }

    /// Returns whether this record's lock type is `FLOCK`.
    ///
    /// The type matters because only `flock` locks belong to an open file
    /// description, and so only an `FLOCK` record can name an acquirer while a
    /// child keeps the lock alive.
    pub(super) fn is_flock(&self) -> bool {
        self.kind == "FLOCK"
    }

    /// Renders the record for a human reader.
    ///
    /// The fields are reproduced rather than the original line so that a record
    /// whose columns were reflowed by the parser still reads as the kernel
    /// meant it.
    pub(super) fn describe(&self) -> String {
        let owner = self
            .pid
            .map_or_else(|| String::from("-1"), |pid| pid.to_string());
        let waiting = if self.blocked { " (waiting) " } else { " " };
        let device = self.device;
        let inode = self.inode;
        let range = &self.range;
        format!(
            "{kind}{waiting}{mode} {access} pid {owner} {device}:{inode} the file range {range}",
            kind = self.kind,
            mode = self.mode,
            access = self.access,
        )
    }
}

/// What a lock record's pid column says about ownership.
///
/// A plain `Option<u32>` cannot distinguish "the kernel named no process" from
/// "this column is not a pid at all", and those two need opposite treatment: the
/// former is a reportable record, the latter a line to skip.
#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecordedPid {
    /// The pid of the process the kernel associates with the lock.
    Owner(ProcessId),
    /// The kernel printed no owner, as it does for an `OFDLCK` record.
    NoOwner,
}

/// Parses one `/proc/locks` line, or reports that it is not one.
///
/// Malformed input returns `None` rather than an error. The table belongs to
/// the whole machine, so a line this parser does not recognize is a record to
/// skip — including any future lock type — not a reason to abandon the
/// diagnosis of the one record that matters.
#[cfg(any(target_os = "linux", test))]
pub(super) fn parse_lock_record(line: &str) -> Option<LockRecord> {
    let mut fields = line.split_whitespace();
    let ordinal = fields.next()?;
    if !ordinal.ends_with(':') {
        return None;
    }
    let mut blocked = false;
    let kind = loop {
        let field = fields.next()?;
        if field == "->" {
            blocked = true;
            continue;
        }
        break field.to_owned();
    };
    let mode = fields.next()?.to_owned();
    let access = fields.next()?.to_owned();
    let pid = match parse_pid(fields.next()?)? {
        RecordedPid::Owner(pid) => Some(pid),
        RecordedPid::NoOwner => None,
    };
    let (device, inode) = parse_identity(fields.next()?)?;
    // The byte range is always two tokens. A record that stops after the start
    // offset is not one this parser can report faithfully.
    let start = fields.next()?;
    let end = fields.next()?;
    let range = format!("{start} {end}");
    Some(LockRecord {
        blocked,
        kind,
        mode,
        access,
        pid,
        device,
        inode,
        range,
    })
}

/// Parses the pid column, mapping the kernel's `-1` to no owner.
///
/// Malformed input returns `None` rather than [`RecordedPid::NoOwner`], so a
/// corrupt column never reads as a truthful "nobody holds this".
#[cfg(any(target_os = "linux", test))]
fn parse_pid(field: &str) -> Option<RecordedPid> {
    if field == "-1" {
        return Some(RecordedPid::NoOwner);
    }
    field
        .parse()
        .ok()
        .map(|raw| RecordedPid::Owner(ProcessId::new(raw)))
}

/// Parses the `MAJOR:MINOR:INODE` column of a lock record.
///
/// The device halves are hexadecimal and the inode is decimal, so the three
/// parts cannot be parsed uniformly. A fourth part means this was not the
/// column expected, and is rejected rather than silently ignored.
#[cfg(any(target_os = "linux", test))]
fn parse_identity(field: &str) -> Option<(DeviceNumber, Inode)> {
    let mut parts = field.split(':');
    let major = u64::from_str_radix(parts.next()?, 16).ok()?;
    let minor = u64::from_str_radix(parts.next()?, 16).ok()?;
    let inode = Inode::new(parts.next()?.parse().ok()?);
    if parts.next().is_some() {
        return None;
    }
    Some((DeviceNumber::new(major, minor), inode))
}

#[cfg(test)]
#[path = "artefact_lock_evidence_tests.rs"]
mod tests;
