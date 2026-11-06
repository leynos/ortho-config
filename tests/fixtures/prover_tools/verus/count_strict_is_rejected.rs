//! Negative Verus fixture: a false strict bound that must fail verification.
//!
//! This file is an integration-test input for the pinned Verus verifier. It is
//! deliberately outside the Cargo workspace: `verus` reads it as a standalone
//! crate, so it never participates in ordinary Rust builds or tests.
//!
//! The claim `count(s) < s.len()` is false for the all-true sequence and for
//! the empty sequence, and no precondition excludes either witness. A genuine
//! verification failure here is the negative control: it proves the runner
//! reports a rejected proof rather than a compiler error or a timeout.

use vstd::prelude::*;

verus! {

/// Counts the `true` elements of a finite Boolean sequence.
spec fn count(s: Seq<bool>) -> nat
    decreases s.len(),
{
    if s.len() == 0 {
        0
    } else {
        (if s[0] { 1nat } else { 0nat }) + count(s.subrange(1, s.len() as int))
    }
}

/// The false claim: every sequence has strictly fewer `true` elements than
/// its length. The empty sequence alone refutes it.
proof fn count_is_strictly_less(s: Seq<bool>)
    ensures
        count(s) < s.len(),
    decreases s.len(),
{
    if s.len() == 0 {
    } else {
        count_is_strictly_less(s.subrange(1, s.len() as int));
    }
}

fn main() {}

} // verus!