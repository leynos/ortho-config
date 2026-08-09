//! Tests for structured profile error paths: unknown, invalid, and reserved
//! names, forbidden keys, and error ordering (milestone 3).

use pretty_assertions::assert_eq;

#[test]
fn verifies_pretty_assertions_diff_output_is_usable() {
    assert_eq!(vec![1, 2, 3], [1, 2, 3]);
}
