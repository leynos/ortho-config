//! The macro a raw-spelled `use` path names under an inert name.

/// Splices the contained inventory into the invoking module.
macro_rules! compile_error {
    () => {
        include!("inventory.inc");
    };
}

pub(crate) use compile_error;
