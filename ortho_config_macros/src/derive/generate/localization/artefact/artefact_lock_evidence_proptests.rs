//! Property tests for the device split and the identity match.
//!
//! `artefact_lock_evidence_tests` pins the layout with named examples, each
//! reasoned about by hand. Those examples are necessary and not sufficient: the
//! split and the packing are a pair of shift-and-mask functions over 64 bits, so
//! the interesting question is what they do everywhere else, particularly at the
//! field boundaries the examples do not reach. These properties cover the
//! interior of each field's width, which is where a shift that happens to pass
//! the examples can still be wrong.
//!
//! The packing here is written from the documented glibc layout rather than from
//! `DeviceNumber::from_raw`. A property that reused the implementation would
//! agree with it by construction and prove nothing, so the two are deliberately
//! independent readings of the same specification: the test fails when they
//! disagree, which is the only way it can fail.
//!
//! The module is separate from the example tests rather than appended to them
//! because together the two exceed the 400-line module cap, and the split is a
//! real seam: examples document the cases a maintainer reasoned about, while
//! properties assert the invariant that reasoning was meant to establish.

use proptest::prelude::*;

use super::tests::granted_flock;
use super::{DeviceNumber, Inode, LockFileIdentity, LockRecord};

/// Packs a device exactly as glibc's `makedev` does, independently of the split.
///
/// This is transcribed from glibc's own `gnu_dev_makedev` rather than derived
/// from `DeviceNumber::from_raw`, so the two are independent readings of the
/// same specification and the property fails only when they disagree. Using the
/// implementation to build the input would make every input agree with it by
/// construction.
///
/// The high halves are masked as `!0xfff` and `!0xff` and shifted *after*
/// masking, which is how glibc writes them: the shift that places a half in the
/// packed value is not the shift that extracts it again, and conflating the two
/// is the mistake this property found in its first draft.
fn pack_device(major: u64, minor: u64) -> u64 {
    (minor & 0x0000_00ff)
        | (major & 0x0fff) << 8
        | (minor & !0xff_u64) << 12
        | (major & !0xfff_u64) << 32
}

/// A device whose halves are drawn from their full field widths.
///
/// The ranges are the widths glibc actually packs, not an arbitrary sample. The
/// major spans 32 bits (`12` low at bit 8, `20` high at bit 44) and the minor
/// spans 32 bits (`8` low at bit 0, `24` high at bit 20), so `0..=u32::MAX` is
/// the whole space either half can occupy.
///
/// The range matters: the high fields are exactly what a two-mask split drops,
/// so a generator that stayed below 32 bits would leave the defect these
/// properties exist to catch unexercised. An earlier version capped the halves
/// at `2^20` and `2^24`, which never set a major bit above 20 or a minor bit
/// above 24, leaving precisely those high fields unexercised.
///
/// The halves are drawn as `u64` directly rather than as a `u32` widened
/// afterwards: a `u32` generator cannot express the boundary above 32 bits, and
/// widening an already-clamped value would hide the same gap.
fn any_device() -> impl Strategy<Value = (u64, u64)> {
    (0_u64..=u64::from(u32::MAX), 0_u64..=u64::from(u32::MAX))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Verifies the split inverts the packing across every representable device.
    #[test]
    fn split_inverts_the_glibc_packing((major, minor) in any_device()) {
        let raw = pack_device(major, minor);
        prop_assert_eq!(
            DeviceNumber::from_raw(raw),
            DeviceNumber::new(major, minor),
            "st_dev {:#x} must split back to {:#x}:{:#x}",
            raw,
            major,
            minor,
        );
    }

    /// Verifies a packed device is recognised through the whole record path.
    ///
    /// This is the path the report actually takes: a raw `st_dev` is compared
    /// against a `/proc/locks` record whose identity column the kernel prints in
    /// hex. `DeviceNumber::from_raw` reads the raw value and the record carries
    /// the printed one, so the two must agree — otherwise a real hold matches no
    /// record and reads as "nothing holds this lock", which is the failure this
    /// whole module exists to make impossible.
    ///
    /// The three negative cases are what keep the positive one from being
    /// vacuous: a matcher that returned `true` unconditionally would satisfy it
    /// alone. Each changes exactly one of the three quantities that name a file,
    /// so a match that ignored any of them is caught.
    #[test]
    fn a_packed_device_matches_the_record_the_kernel_prints(
        (major, minor) in any_device(),
        inode in 1_u64..=u64::from(u32::MAX),
    ) {
        let identity = LockFileIdentity::of(
            DeviceNumber::from_raw(pack_device(major, minor)),
            Inode::new(inode),
        );
        let record = LockRecord {
            device: DeviceNumber::new(major, minor),
            inode: Inode::new(inode),
            ..granted_flock()
        };
        prop_assert!(
            record.matches(identity),
            "{major:#x}:{minor:#x}:{inode} must match the record naming the same file",
        );
        prop_assert!(
            !record.matches(LockFileIdentity::of(
                DeviceNumber::new(major.wrapping_add(1) & u64::from(u32::MAX), minor),
                Inode::new(inode),
            )),
            "a different major must not match",
        );
        prop_assert!(
            !record.matches(LockFileIdentity::of(
                DeviceNumber::new(major, minor.wrapping_add(1) & u64::from(u32::MAX)),
                Inode::new(inode),
            )),
            "a different minor must not match",
        );
        prop_assert!(
            !record.matches(LockFileIdentity::of(
                DeviceNumber::new(major, minor),
                Inode::new(inode.wrapping_add(1)),
            )),
            "a different inode must not match",
        );
    }
}
