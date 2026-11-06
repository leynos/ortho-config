//! Positive Verus fixture: a recursive count over a finite Boolean sequence.
//!
//! This file is an integration-test input for the pinned Verus verifier. It is
//! deliberately outside the Cargo workspace: `verus` reads it as a standalone
//! crate, so it never participates in ordinary Rust builds or tests.
//!
//! The proof establishes `0 <= count(s) <= s.len()` by induction on length,
//! rather than assuming the bound. None of the obligations is closed by
//! `assume`, `admit`, or an external proof body.

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

/// Every sequence's count lies between zero and its length.
proof fn count_bounds(s: Seq<bool>)
    ensures
        0 <= count(s) && count(s) <= s.len(),
    decreases s.len(),
{
    if s.len() == 0 {
        assert(count(s) == 0);
    } else {
        let rest = s.subrange(1, s.len() as int);
        assert(rest.len() == s.len() - 1);
        count_bounds(rest);
        assert(count(s) == (if s[0] { 1nat } else { 0nat }) + count(rest));
    }
}

/// Inhabited witnesses: the empty, singleton, and mixed cases.
proof fn inhabited_witnesses() {
    assert(count(Seq::<bool>::empty()) == 0) by (compute);
    assert(count(seq![true]) == 1) by (compute);
    assert(count(seq![true, false, true]) == 2) by (compute);
}

fn main() {}

} // verus!