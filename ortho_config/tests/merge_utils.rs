use ortho_config::merge_cli_over_defaults;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct Sample {
    a: Option<u32>,
    b: Option<String>,
}

#[test]
fn cli_overrides_defaults() {
    let defaults = Sample {
        a: Some(1),
        b: Some("def".into()),
    };
    let cli = Sample {
        a: None,
        b: Some("cli".into()),
    };
    let merged = merge_cli_over_defaults(defaults, cli);
    assert_eq!(
        merged,
        Sample {
            a: Some(1),
            b: Some("cli".into())
        }
    );
}
