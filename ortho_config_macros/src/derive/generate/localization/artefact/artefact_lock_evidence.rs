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
//! in `artefact_lock_diagnostics`.

use std::fmt;

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
    /// Splits a raw `st_dev` value into the halves `/proc/locks` prints.
    ///
    /// glibc packs the major's low 12 bits at bit 8 and its high 20 bits at bit
    /// 32, and the minor's low 8 bits at bit 0 and its high 24 bits at bit 12.
    /// The masks below are those four fields, so each half is the masked low
    /// part shifted down plus the masked high part shifted down.
    pub(super) const fn from_raw(raw: u64) -> Self {
        /// Bits 8..20 of the raw value: the major's low 12 bits.
        const MAJOR_LOW: u64 = 0x0000_0000_000f_ff00;
        /// Bits 32..52 of the raw value: the major's high 20 bits.
        const MAJOR_HIGH: u64 = 0xffff_f000_0000_0000;
        /// Bits 0..8 of the raw value: the minor's low 8 bits.
        const MINOR_LOW: u64 = 0x0000_0000_0000_00ff;
        /// Bits 12..36 of the raw value: the minor's high 24 bits.
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
        write!(formatter, "{}:{}", hex(self.major), hex(self.minor))
    }
}

/// Renders a device half the way the kernel prints it: `%02x`.
#[cfg(any(target_os = "linux", test))]
fn hex(value: u64) -> String {
    let digits = hex_digits(value);
    if digits.len() >= 2 {
        digits
    } else {
        format!("0{digits}")
    }
}

/// Renders a value in lowercase hexadecimal without padding.
#[cfg(any(target_os = "linux", test))]
fn hex_digits(value: u64) -> String {
    let mut digits = String::new();
    let mut remaining = value;
    loop {
        digits.insert(0, nibble_char(remaining.rem_euclid(16)));
        remaining = remaining.div_euclid(16);
        if remaining == 0 {
            return digits;
        }
    }
}

/// Maps a four-bit value to its lowercase hexadecimal digit.
#[cfg(any(target_os = "linux", test))]
fn nibble_char(nibble: u64) -> char {
    u32::try_from(nibble)
        .ok()
        .and_then(|value| char::from_digit(value, 16))
        .unwrap_or('?')
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
    pid: Option<u32>,
    /// The major number of the locked file's device, in hexadecimal.
    major: u64,
    /// The minor number of the locked file's device, in hexadecimal.
    minor: u64,
    /// The inode of the locked file, in decimal.
    inode: u64,
    /// The lock's byte range as the kernel prints it, such as `0 EOF`.
    range: String,
}

#[cfg(any(target_os = "linux", test))]
impl LockRecord {
    /// Returns whether this record describes `device` and `inode`.
    pub(super) const fn matches(&self, device: DeviceNumber, inode: u64) -> bool {
        self.inode == inode && self.major == device.major && self.minor == device.minor
    }

    /// Reports the pid that owns this record, and whether it is a waiter.
    ///
    /// A blocked record names the process *waiting* for the lock, so it never
    /// owns anything; and an `OFDLCK` record names no process at all. Both cases
    /// answer `None`, which is what keeps a waiter from being reported as a
    /// holder.
    pub(super) fn owning_pid(&self) -> Option<u32> {
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
        let device = DeviceNumber {
            major: self.major,
            minor: self.minor,
        };
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

/// Parses one `/proc/locks` line, or reports that it is not one.
///
/// Malformed input returns `None` rather than an error. The table belongs to
/// the whole machine, so a line this parser does not recognise is a record to
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
    let (major, minor, inode) = parse_identity(fields.next()?)?;
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
        major,
        minor,
        inode,
        range,
    })
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
    Owner(u32),
    /// The kernel printed no owner, as it does for an `OFDLCK` record.
    NoOwner,
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
    field.parse().ok().map(RecordedPid::Owner)
}

/// Parses the `MAJOR:MINOR:INODE` column of a lock record.
///
/// The device halves are hexadecimal and the inode is decimal, so the three
/// parts cannot be parsed uniformly. A fourth part means this was not the
/// column expected, and is rejected rather than silently ignored.
#[cfg(any(target_os = "linux", test))]
fn parse_identity(field: &str) -> Option<(u64, u64, u64)> {
    let mut parts = field.split(':');
    let major = u64::from_str_radix(parts.next()?, 16).ok()?;
    let minor = u64::from_str_radix(parts.next()?, 16).ok()?;
    let inode = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, inode))
}

#[cfg(test)]
#[path = "artefact_lock_evidence_tests.rs"]
mod tests;
