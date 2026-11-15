// A pid where a descriptor belongs, and vice versa.
//
// Both are `u32` underneath, which is exactly why they are separate types: a
// report that swapped them would name an innocent process in place of a
// descriptor, and read as a confident accusation.
//
// The fixture includes the real source and calls it locally, with no shared
// helper module. An earlier draft put the call behind a `mod` shared with the
// control and the cases, and every case then failed on `E0603`/`E0624` privacy
// errors instead — the transposition was never reached, so the test proved
// nothing. Keeping each case self-contained makes the transposition the only
// error rustc can report.

#[expect(
    dead_code,
    reason = "one include reaches the whole vocabulary; the fixture calls only part of it"
)]
#[path = "../../src/derive/generate/localization/artefact/artefact_lock_evidence.rs"]
mod evidence;

use evidence::{Descriptor, DeviceNumber, Inode, LockFileIdentity, ProcessId};

fn describe(pid: ProcessId, descriptor: Descriptor) -> String {
    format!("pid {pid} fd {descriptor}")
}

fn main() {
    let device = DeviceNumber::new(8, 1);
    let inode = Inode::new(42);
    let identity = LockFileIdentity::of(device, inode);
    let _ = (identity, device, inode);
    // Both arguments are transposed: a `ProcessId` where a `Descriptor` is
    // declared, and a `Descriptor` where a `ProcessId` is.
    let _ = describe(Descriptor::new(3), ProcessId::new(1234));
}
