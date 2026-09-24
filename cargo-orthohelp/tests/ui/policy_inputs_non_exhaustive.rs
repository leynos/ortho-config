//! Compile-fail test for `PolicyInputs` construction outside the crate.

use cargo_orthohelp::policy::PolicyInputs;

fn main() {
    let _inputs = PolicyInputs {};
}
