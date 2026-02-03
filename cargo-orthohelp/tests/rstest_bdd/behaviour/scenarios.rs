//! Binds `cargo-orthohelp` feature files to the step registry.

use rstest_bdd_macros::scenarios;

scenarios!("tests/features/orthohelp_ir.feature");
scenarios!("tests/features/orthohelp_roff.feature");
