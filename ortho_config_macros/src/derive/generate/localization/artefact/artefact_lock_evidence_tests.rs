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

use super::{DeviceNumber, LockRecord, RecordedPid, parse_identity, parse_lock_record, parse_pid};
use rstest::rstest;

/// Builds the record a granted exclusive `flock` produces.
fn granted_flock(inode: u64) -> LockRecord {
    LockRecord {
        blocked: false,
        kind: String::from("FLOCK"),
        mode: String::from("ADVISORY"),
        access: String::from("WRITE"),
        pid: Some(2_181_913),
        major: 0x09,
        minor: 0x02,
        inode,
        range: String::from("0 EOF"),
    }
}

#[test]
fn parses_a_granted_flock_record() {
    let record = parse_lock_record("13: FLOCK  ADVISORY  WRITE 2181913 09:02:9476791 0 EOF");
    assert_eq!(record, Some(granted_flock(9_476_791)));
}

#[test]
fn parses_a_blocked_record_as_blocked_not_as_a_holder() {
    let record = parse_lock_record("13: -> FLOCK  ADVISORY  WRITE 2181920 09:02:9476791 0 EOF");
    let parsed = record.expect("a blocked record must parse");
    assert!(parsed.blocked, "a `->` record describes a waiter");
    assert_eq!(
        parsed.pid,
        Some(2_181_920),
        "the waiting pid is still reported, so a reader sees who is stuck",
    );
}

#[test]
fn parses_nested_blocked_records() {
    let record = parse_lock_record("7: -> -> FLOCK  ADVISORY  WRITE 42 09:02:9476791 0 EOF");
    let parsed = record.expect("a nested waiter must parse");
    assert!(parsed.blocked);
    assert_eq!(parsed.pid, Some(42));
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
#[case::granted_flock(false, Some(2_181_913), Some(2_181_913))]
#[case::blocked_waiter(true, Some(2_181_920), None)]
#[case::ownerless_record(false, None, None)]
fn reports_only_a_granted_record_as_an_owner(
    #[case] blocked: bool,
    #[case] pid: Option<u32>,
    #[case] expected: Option<u32>,
) {
    let record = LockRecord {
        blocked,
        pid,
        ..granted_flock(9_476_791)
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
        ..granted_flock(9_476_791)
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
        Some((0x09, 0x02, 9_476_791)),
        "the device halves are hexadecimal while the inode is decimal",
    );
}

#[test]
fn parses_an_unknown_lock_type_as_a_record() {
    let expected = LockRecord {
        kind: String::from("LEASE"),
        pid: Some(1),
        ..granted_flock(1)
    };
    assert_eq!(
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
    assert_eq!(parse_pid("13"), Some(RecordedPid::Owner(13)));
    assert_eq!(
        parse_pid("-12"),
        None,
        "a negative pid other than the -1 sentinel is not an owner",
    );
}

#[rstest]
#[case::tmpfs(0x902, 0x09, 0x02)]
#[case::small_minor(0x19, 0x00, 0x19)]
#[case::small_major(0x801, 0x08, 0x01)]
#[case::zero(0x0, 0x00, 0x00)]
#[case::wide_major_high_bits(0x0000_1000_0562_3478, 0x1234, 0x5678)]
#[case::major_high_minor_low(0x001f_ff00, 0xfff, 0x100)]
#[case::minor_high_bits(0x120_fe34, 0xfe, 0x1234)]
fn splits_device_numbers_by_the_glibc_layout(
    #[case] raw: u64,
    #[case] major: u64,
    #[case] minor: u64,
) {
    assert_eq!(
        DeviceNumber::from_raw(raw),
        DeviceNumber { major, minor },
        "the raw st_dev {raw:#x} must split as glibc's gnu_dev_major/minor do",
    );
}

#[rstest]
#[case::tmpfs(0x09, 0x02, "09:02")]
#[case::single_digit_minor(0x00, 0x19, "00:19")]
#[case::wide(0x1234, 0x5678, "1234:5678")]
#[case::zero(0x00, 0x00, "00:00")]
fn renders_devices_the_way_the_kernel_does(
    #[case] major: u64,
    #[case] minor: u64,
    #[case] expected: &str,
) {
    assert_eq!(
        DeviceNumber { major, minor }.to_string(),
        expected,
        "the kernel prints each half as %02x, so single digits keep a leading zero",
    );
}

#[rstest]
#[case::same_device_and_inode(0x09, 0x02, 9_476_791, true)]
#[case::different_inode(0x09, 0x02, 9_476_792, false)]
#[case::different_major(0x0a, 0x02, 9_476_791, false)]
#[case::different_minor(0x09, 0x03, 9_476_791, false)]
fn matches_records_by_device_and_inode(
    #[case] major: u64,
    #[case] minor: u64,
    #[case] inode: u64,
    #[case] expected: bool,
) {
    let record = granted_flock(9_476_791);
    assert_eq!(
        record.matches(DeviceNumber { major, minor }, inode),
        expected,
        "a record must only be attributed to this lock when device and inode both match",
    );
}

#[test]
fn describes_a_record_without_its_ordinal() {
    let described = granted_flock(9_476_791).describe();
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
    let mut waiting = granted_flock(9_476_791);
    waiting.blocked = true;
    let described = waiting.describe();
    assert!(
        described.contains("(waiting)"),
        "a waiter must be distinguishable from a holder: {described}",
    );
}

#[test]
fn describes_an_open_file_description_lock_as_unowned() {
    let mut record = granted_flock(9_476_791);
    record.kind = String::from("OFDLCK");
    record.pid = None;
    let described = record.describe();
    assert!(
        described.contains("pid -1"),
        "an unowned lock must not be rendered as no pid at all: {described}",
    );
}
