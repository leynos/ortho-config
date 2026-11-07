//! Unit tests for the lock diagnostics' pure evidence vocabulary.
//!
//! The parser and the device split are what turn a kernel line into evidence a
//! maintainer can act on, and both are wrong in ways a green end-to-end test
//! would not notice: a mis-split device silently reports *no* records, which
//! reads exactly like "the lock was already free". These tests run on every
//! platform, so the evidence format is pinned even where `/proc/locks` is
//! absent.
//!
//! Every fixture line below is transcribed from this host's `/proc/locks`, and
//! each transcription was checked against the raw bytes: the `POSIX` type is
//! padded to six characters and `OFDLCK` reports pid `-1`. That padding is why
//! the parser splits on whitespace runs rather than on fixed columns.
//!
//! The device, inode, and pid fixtures are built from the newtypes rather than
//! from bare integers, so a swap of two same-width quantities fails to compile
//! here as well as in the production module.

use super::{
    DeviceNumber, Inode, LockFileIdentity, LockRecord, ProcessId, RecordedPid, parse_identity,
    parse_lock_record, parse_pid,
};
use rstest::rstest;

/// The device every fixture record names, from this host's `/proc/locks`.
const LOCK_DEVICE: DeviceNumber = DeviceNumber::new(0x09, 0x02);

/// The inode every fixture record names, from this host's `/proc/locks`.
const LOCK_INODE: Inode = Inode::new(9_476_791);

/// Builds the record a granted exclusive `flock` produces.
fn granted_flock() -> LockRecord {
    LockRecord {
        blocked: false,
        kind: String::from("FLOCK"),
        mode: String::from("ADVISORY"),
        access: String::from("WRITE"),
        pid: Some(ProcessId::new(2_181_913)),
        device: LOCK_DEVICE,
        inode: LOCK_INODE,
        range: String::from("0 EOF"),
    }
}

#[test]
fn parses_a_granted_flock_record() {
    let record = parse_lock_record("13: FLOCK  ADVISORY  WRITE 2181913 09:02:9476791 0 EOF");
    assert_eq!(record, Some(granted_flock()));
}

#[test]
fn parses_a_blocked_record_as_blocked_not_as_a_holder() {
    let record = parse_lock_record("13: -> FLOCK  ADVISORY  WRITE 2181920 09:02:9476791 0 EOF");
    let parsed = record.expect("a blocked record must parse");
    assert!(parsed.blocked, "a `->` record describes a waiter");
    assert_eq!(
        parsed.pid,
        Some(ProcessId::new(2_181_920)),
        "the waiting pid is still reported, so a reader sees who is stuck",
    );
}

#[test]
fn parses_nested_blocked_records() {
    let record = parse_lock_record("7: -> -> FLOCK  ADVISORY  WRITE 42 09:02:9476791 0 EOF");
    let parsed = record.expect("a nested waiter must parse");
    assert!(parsed.blocked);
    assert_eq!(parsed.pid, Some(ProcessId::new(42)));
}

#[test]
fn parses_a_posix_record_despite_its_padded_type() {
    let record = parse_lock_record("6: POSIX  ADVISORY  READ 2188420 09:02:9477589 0 9");
    let parsed = record.expect("the padded POSIX type must not shift the columns");
    assert_eq!(parsed.kind, "POSIX");
    assert_eq!(parsed.access, "READ");
    assert_eq!(parsed.range, "0 9");
}

#[test]
fn parses_an_open_file_description_lock_without_an_owner() {
    let record = parse_lock_record("1: OFDLCK ADVISORY  WRITE -1 09:02:9477589 0 EOF");
    let parsed = record.expect("an OFDLCK record must parse");
    assert_eq!(
        parsed.pid, None,
        "the kernel reports -1 for OFDLCK, which is no owner rather than pid -1",
    );
}

#[rstest]
#[case::granted_flock(
    false,
    Some(ProcessId::new(2_181_913)),
    Some(ProcessId::new(2_181_913))
)]
#[case::blocked_waiter(true, Some(ProcessId::new(2_181_920)), None)]
#[case::ownerless_record(false, None, None)]
fn reports_only_a_granted_record_as_an_owner(
    #[case] blocked: bool,
    #[case] pid: Option<ProcessId>,
    #[case] expected: Option<ProcessId>,
) {
    let record = LockRecord {
        blocked,
        pid,
        ..granted_flock()
    };
    assert_eq!(
        record.owning_pid(),
        expected,
        "a waiter and an OFDLCK record own nothing, so neither may be reported as the holder",
    );
}

#[rstest]
#[case::flock("FLOCK", true)]
#[case::posix("POSIX", false)]
#[case::ofdlck("OFDLCK", false)]
#[case::unknown_type("LEASE", false)]
fn identifies_only_flock_records(#[case] kind: &str, #[case] expected: bool) {
    let record = LockRecord {
        kind: String::from(kind),
        ..granted_flock()
    };
    assert_eq!(
        record.is_flock(),
        expected,
        "only flock locks belong to an open file description, so only FLOCK explains an inherited hold",
    );
}

#[test]
fn parses_device_and_inode_from_the_identity_column() {
    assert_eq!(
        parse_identity("09:02:9476791"),
        Some((DeviceNumber::new(0x09, 0x02), LOCK_INODE)),
        "the device halves are hexadecimal while the inode is decimal",
    );
}

#[test]
fn parses_an_unknown_lock_type_as_a_record() {
    let expected = LockRecord {
        kind: String::from("LEASE"),
        pid: Some(ProcessId::new(1)),
        inode: Inode::new(1),
        ..granted_flock()
    };
    assert_eq!(
        // The line keeps the fixture's device but names a shorter inode, so the
        // expectation has to follow it: deriving everything from `granted_flock`
        // would assert the parser had echoed the fixture rather than this line.
        parse_lock_record("13: LEASE ADVISORY WRITE 1 09:02:1 0 EOF"),
        Some(expected),
        "a never-seen lock type is still a record to report, not a line to skip",
    );
}

#[rstest]
#[case::empty("")]
#[case::no_ordinal("FLOCK ADVISORY WRITE 1 09:02:1 0 EOF")]
#[case::ordinal_without_colon("13 FLOCK ADVISORY WRITE 1 09:02:1 0 EOF")]
#[case::truncated_after_ordinal("13:")]
#[case::truncated_before_pid("13: FLOCK ADVISORY WRITE")]
#[case::non_numeric_pid("13: FLOCK ADVISORY WRITE x 09:02:1 0 EOF")]
#[case::padded_pid_placeholder("13: FLOCK ADVISORY WRITE - 09:02:1 0 EOF")]
#[case::non_numeric_major("13: FLOCK ADVISORY WRITE 1 zz:02:1 0 EOF")]
#[case::non_numeric_minor("13: FLOCK ADVISORY WRITE 1 09:zz:1 0 EOF")]
#[case::non_numeric_inode("13: FLOCK ADVISORY WRITE 1 09:02:x 0 EOF")]
#[case::four_part_identity("13: FLOCK ADVISORY WRITE 1 09:02:1:0 0 EOF")]
#[case::identity_without_inode("13: FLOCK ADVISORY WRITE 1 09:02 0 EOF")]
#[case::missing_range("13: FLOCK ADVISORY WRITE 1 09:02:1")]
fn rejects_malformed_lines(#[case] line: &str) {
    assert_eq!(
        parse_lock_record(line),
        None,
        "a line this parser does not understand must be skipped, not guessed at",
    );
}

#[test]
fn parses_the_kernels_no_owner_sentinel_only_as_a_whole_field() {
    assert_eq!(parse_pid("-1"), Some(RecordedPid::NoOwner));
    assert_eq!(
        parse_pid("13"),
        Some(RecordedPid::Owner(ProcessId::new(13)))
    );
    assert_eq!(
        parse_pid("-12"),
        None,
        "a negative pid other than the -1 sentinel is not an owner",
    );
}

/// Raw `st_dev` values and the halves glibc's `gnu_dev_major`/`minor` give them.
///
/// The wide rows are why the split cannot be two low-bit masks: the major's
/// high 20 bits sit at bit 44 and the minor's high 24 bits at bit 20, so a
/// split that reads only the low halves passes every small case and then finds
/// no records on a real filesystem. Naming each row keeps a failure pointing at
/// the layout case that broke rather than at an anonymous input.
const DEVICE_SPLITS: [(&str, u64, DeviceNumber); 8] = [
    ("tmpfs", 0x902, DeviceNumber::new(0x09, 0x02)),
    ("small_minor", 0x19, DeviceNumber::new(0x00, 0x19)),
    ("small_major", 0x801, DeviceNumber::new(0x08, 0x01)),
    ("zero", 0x0, DeviceNumber::new(0x00, 0x00)),
    (
        "wide_major_high_bits",
        0x0000_1000_0562_3478,
        DeviceNumber::new(0x1234, 0x5678),
    ),
    (
        "major_high_minor_low",
        0x001f_ff00,
        DeviceNumber::new(0xfff, 0x100),
    ),
    (
        "minor_high_bits",
        0x120_fe34,
        DeviceNumber::new(0xfe, 0x1234),
    ),
    // The minor's high field runs to bit 43, but no other row sets a bit past
    // 31 there, so a split that stopped reading at bit 32 would still pass
    // them. This row reaches bits 32..44, the range only the widest minor
    // field occupies.
    (
        "minor_high_bits_above_32",
        0x0000_0abc_0000_0000,
        DeviceNumber::new(0x00, 0xabc0_0000),
    ),
];

#[test]
fn splits_device_numbers_by_the_glibc_layout() {
    for (case, raw, expected) in DEVICE_SPLITS {
        assert_eq!(
            DeviceNumber::from_raw(raw),
            expected,
            "the {case} st_dev {raw:#x} must split as glibc's gnu_dev_major/minor do",
        );
    }
}

/// Device halves and the text the kernel prints for them, by case name.
const DEVICE_RENDERS: [(&str, u64, u64, &str); 4] = [
    ("tmpfs", 0x09, 0x02, "09:02"),
    ("single_digit_minor", 0x00, 0x19, "00:19"),
    ("wide", 0x1234, 0x5678, "1234:5678"),
    ("zero", 0x00, 0x00, "00:00"),
];

#[test]
fn renders_devices_the_way_the_kernel_does() {
    for (case, major, minor, expected) in DEVICE_RENDERS {
        assert_eq!(
            DeviceNumber::new(major, minor).to_string(),
            expected,
            "the {case} halves {major:#x}:{minor:#x} must print as the kernel does; \
             each half is two lowercase hex digits, so a single digit keeps a leading zero",
        );
    }
}

/// A device and inode compared against the recorded lock's, and the verdict.
const IDENTITY_MATCHES: [(&str, DeviceNumber, Inode, bool); 4] = [
    ("same_device_and_inode", LOCK_DEVICE, LOCK_INODE, true),
    ("different_inode", LOCK_DEVICE, Inode::new(9_476_792), false),
    (
        "different_major",
        DeviceNumber::new(0x0a, 0x02),
        LOCK_INODE,
        false,
    ),
    (
        "different_minor",
        DeviceNumber::new(0x09, 0x03),
        LOCK_INODE,
        false,
    ),
];

#[test]
fn matches_records_by_device_and_inode() {
    let record = granted_flock();
    for (case, device, inode, expected) in IDENTITY_MATCHES {
        assert_eq!(
            record.matches(LockFileIdentity::of(device, inode)),
            expected,
            "the {case} case must only be attributed to this lock when device and \
             inode both match",
        );
    }
}

#[test]
fn describes_a_record_without_its_ordinal() {
    let described = granted_flock().describe();
    assert!(
        described.contains("FLOCK ADVISORY WRITE pid 2181913 09:02:9476791"),
        "the description must name the type, owner, and file: {described}",
    );
    assert!(
        !described.starts_with("13:"),
        "the ordinal is per-reader and would mislead: {described}",
    );
}

#[test]
fn marks_a_waiting_record_as_waiting() {
    let mut waiting = granted_flock();
    waiting.blocked = true;
    let described = waiting.describe();
    assert!(
        described.contains("(waiting)"),
        "a waiter must be distinguishable from a holder: {described}",
    );
}

#[test]
fn describes_an_open_file_description_lock_as_unowned() {
    let mut record = granted_flock();
    record.kind = String::from("OFDLCK");
    record.pid = None;
    let described = record.describe();
    assert!(
        described.contains("pid -1"),
        "an unowned lock must not be rendered as no pid at all: {described}",
    );
}
