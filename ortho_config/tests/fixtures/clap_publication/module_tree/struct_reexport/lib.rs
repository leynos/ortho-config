//! A decoy whose exported name is a struct rather than the counted enum.
//!
//! Any type-namespace declaration of `ErrorKind` that is not the counted enum
//! makes the exported name lead somewhere the walk never counted. Here a public
//! module declares `ErrorKind` as a struct carrying the longer inventory's
//! variants as associated constants, beside a file-level seventeen-variant
//! enum decoy. The gate refuses the redeclaration instead of counting the
//! decoy. The tree is gate input rather than a compiled program.

mod decoy;

/// The module a consumer reaches `ErrorKind` through.
pub mod error {
    /// The struct that replaces the inventory type.
    pub struct ErrorKind;
}
