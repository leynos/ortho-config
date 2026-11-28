//! The reviewed export shape: a plain re-export of a local module's enum.
//!
//! This mirrors the resolved `clap_builder` source, whose `error` module
//! declares `mod kind;` and `pub use kind::ErrorKind;`. The re-export names the
//! declaration the walk counts, so the refusal for an untraceable export must
//! not fire here: `kind` is one of this crate's own modules.

pub mod error {
    mod kind;

    pub use kind::ErrorKind;
}
