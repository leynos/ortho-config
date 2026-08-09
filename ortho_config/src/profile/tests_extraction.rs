//! Tests for `[profile.<name>]` table extraction from the file chain:
//! per-file layers, chain order, base-layer stripping (milestone 3).

use googletest::prelude::*;

#[test]
fn verifies_googletest_matchers_are_usable() {
    assert_that!("profile", contains_substring("file"));
}
