//! Compile-fail test for the policy evaluator's `#[must_use]` report.

#![deny(unused_must_use)]

use cargo_orthohelp::policy::{PolicyConfig, PolicyInputs, evaluate};

fn main() {
    evaluate(&PolicyConfig::default(), &PolicyInputs::default());
}
