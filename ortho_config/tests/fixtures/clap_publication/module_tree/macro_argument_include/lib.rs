//! A compiled decoy beside an inventory handed to a macro as an argument.
//!
//! `paste!` has an item-position parameter, so the call's arguments are a
//! second route into the item list: the argument is `include!("inventory.inc")`,
//! which spells no `ErrorKind` itself and appears in the arguments rather than
//! in the transcription. A walk that reads only the transcription accepts the
//! call and counts `decoy.rs`, seventeen variants that equal the reviewed
//! total, while the expansion splices in the eighteen-variant inventory.

mod decoy;

/// Re-emits its item argument, whatever that argument is.
macro_rules! paste {
    ($item:item) => {
        $item
    };
}

paste! { include!("inventory.inc") }