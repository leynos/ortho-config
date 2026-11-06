//! Fixture-only bounded counter kernel for Kani integration tests.
//!
//! This crate is an integration-test input for the pinned Kani verifier. It is
//! deliberately outside the Cargo workspace: `cargo kani` reads it as a
//! standalone package, so it never participates in ordinary Rust builds or
//! tests.
//!
//! The kernel counts selected inputs in a four-element Boolean array. The
//! harnesses below prove the count against an independently expressed sum,
//! check the reachable range, and include a deliberately wrong mutant that
//! must produce a counterexample. They establish fixture behaviour within four
//! inputs, not production code or unbounded behaviour.

/// Counts the selected inputs in a four-element Boolean array.
///
/// # Examples
///
/// ```
/// assert_eq!(
///     prover_tools_kani_fixture::count_selected([true, false, true, false]),
///     2,
/// );
/// ```
pub fn count_selected(inputs: [bool; 4]) -> u8 {
    inputs.iter().filter(|selected| **selected).count() as u8
}

#[cfg(kani)]
mod proofs {
    use super::count_selected;

    /// The counter agrees with an independently expressed sum.
    #[kani::proof]
    #[kani::unwind(5)]
    fn count_matches_independent_sum() {
        let inputs: [bool; 4] = kani::any();
        let expected: u8 = inputs.iter().map(|&selected| u8::from(selected)).sum();
        assert_eq!(count_selected(inputs), expected);
    }

    /// The counter stays in range and every count class is reachable.
    #[kani::proof]
    #[kani::unwind(5)]
    fn reachable_zero_full_and_mixed() {
        let inputs: [bool; 4] = kani::any();
        let count = count_selected(inputs);
        assert!(count <= 4);
        kani::cover!(count == 0);
        kani::cover!(count == 4);
        kani::cover!(count == 2);
    }

    /// The mutant omits the last input, so a selected last input refutes it.
    ///
    /// No assumption excludes that witness: the harness is expected to fail.
    #[kani::proof]
    #[kani::unwind(5)]
    fn mutant_omitting_last_input_is_rejected() {
        let inputs: [bool; 4] = kani::any();
        let mutant = inputs[..3].iter().filter(|selected| **selected).count() as u8;
        let expected: u8 = inputs.iter().map(|&selected| u8::from(selected)).sum();
        assert_eq!(mutant, expected);
    }

    /// The concrete witness the mutant must miss: only the last input is set.
    #[kani::proof]
    #[kani::unwind(5)]
    fn mutant_witness_is_a_counterexample() {
        let inputs = [false, false, false, true];
        let mutant = inputs[..3].iter().filter(|selected| **selected).count() as u8;
        let expected: u8 = inputs.iter().map(|&selected| u8::from(selected)).sum();
        assert_ne!(mutant, expected);
    }
}
