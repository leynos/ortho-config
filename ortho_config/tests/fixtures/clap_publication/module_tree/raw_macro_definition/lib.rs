//! A compiled decoy beside an inventory a raw-spelled definition splices in.
//!
//! `r#compile_error` and `compile_error` are one identifier, so a definition
//! carrying the prefix shadows the prelude macro of the plain name. The call
//! below therefore reaches a transcription that calls `include!` and splices
//! the eighteen-variant inventory beside the seventeen-variant decoy. An audit
//! that keys its definitions by the rendered spelling and its calls by the
//! plain one resolves the call to no local definition at all and trusts the
//! prelude macro the name only appears to reach.

mod decoy;

/// The module that really declares the inventory this crate exports.
pub mod error {
    /// Splices the contained inventory into the enclosing module.
    macro_rules! r#compile_error {
        () => {
            include!("inventory.inc");
        };
    }

    compile_error!();
}
