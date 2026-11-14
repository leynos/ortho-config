// The control: every newtype at its own position.
//
// This must compile. It proves the include reaches the real declarations and
// that the calls the failing cases transpose are otherwise well formed, so the
// only difference between this file and theirs is the transposition.

#[allow(dead_code)]
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
    // `LockFileIdentity` has no `Display` impl by design — a file identity is
    // compared, not printed — so the control formats it as `Debug`.
    let _ = format!("{identity:?} {device} {inode}");
    let _ = describe(ProcessId::new(1234), Descriptor::new(3));
}
