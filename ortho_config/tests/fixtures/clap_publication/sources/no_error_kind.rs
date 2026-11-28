//! Irrelevant inventory: a file with no `ErrorKind` definition at all.
//!
//! Walking a real source tree sees files like this constantly, so the gate
//! treats "not here" as a normal result rather than a failure.

/// A type unrelated to command-line errors.
pub enum Other {
    /// The only variant.
    Only,
}