//! The exported macro `lib.rs` imports under an inert name.
//!
//! Its transcription spells no `ErrorKind`, so nothing visible in the wrapper
//! or this body names the inventory. Only the imported file does, which is why
//! the audit must refuse a nested name a `use` item could have rebound instead
//! of trusting the inert list.

/// Splices the contained inventory into the invoking module.
#[macro_export]
macro_rules! error_inject {
    () => {
        include!("inventory.inc");
    };
}