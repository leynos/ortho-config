// The contained inventory this fixture splices into `mod error`.
//
// The `.inc` extension keeps the file outside the gate's Rust source walk,
// exactly as a real include target can sit outside it. Eighteen variants: one
// more than the reviewed total, so a count that reached this file would fail
// the comparison while a count of the decoy beside it would not. These are
// ordinary comments rather than inner docs because the file is spliced into a
// module body this way.

/// The exported inventory, one variant longer than the reviewed total.
pub enum ErrorKind {
    Real01,
    Real02,
    Real03,
    Real04,
    Real05,
    Real06,
    Real07,
    Real08,
    Real09,
    Real10,
    Real11,
    Real12,
    Real13,
    Real14,
    Real15,
    Real16,
    Real17,
    Real18,
}
