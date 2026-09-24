//! Compile-pass test for the public policy API.

use cargo_orthohelp::policy::{
    ExceptionKind, PolicyConfig, PolicyException, PolicyInputs, evaluate,
};

fn main() {
    let config = PolicyConfig {
        exceptions: vec![PolicyException {
            kind: ExceptionKind::Verb,
            name: "list".to_owned(),
            reason: "compatibility".to_owned(),
            command_path: None,
        }],
        ..PolicyConfig::default()
    };
    let report = evaluate(&config, &PolicyInputs::default());
    assert_eq!(report.exceptions, config.exceptions);
}
