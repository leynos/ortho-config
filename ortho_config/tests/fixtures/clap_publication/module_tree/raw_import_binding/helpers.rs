//! The exported macro a raw-spelled `use` alias binds to an inert name.
//!
//! Its transcription spells no `ErrorKind`, so nothing visible in the call or
//! the macro body names the inventory. Only the included file does, which is
//! why the audit must record the binding under the name Rust resolves it by.

/// Splices the contained inventory into the invoking module.
#[macro_export]
macro_rules! error_inject {
    () => {
        include!("inventory.inc");
    };
}
