// An inode where a device belongs, and vice versa.
//
// Both are `u64` underneath. `LockFileIdentity::of` is the pair that names one
// file, and matching reads it as a whole; a swap here compares an inode against
// a device and finds no record, which reads as "nothing holds this lock" rather
// than as the defect it is.
//
// Self-contained for the reason given in `transposed_pid_and_descriptor.rs`: a
// shared helper turns every case into a privacy error and never reaches the
// transposition.

#[expect(
    dead_code,
    reason = "one include reaches the whole vocabulary; the fixture calls only part of it"
)]
#[path = "../../src/derive/generate/localization/artefact/artefact_lock_evidence.rs"]
mod evidence;

use evidence::{DeviceNumber, Inode, LockFileIdentity};

fn main() {
    let device = DeviceNumber::new(8, 1);
    let inode = Inode::new(42);
    // Both arguments are transposed: an `Inode` where a `DeviceNumber` is
    // declared, and a `DeviceNumber` where an `Inode` is.
    let _identity = LockFileIdentity::of(inode, device);
}
