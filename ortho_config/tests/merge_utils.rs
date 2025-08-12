//! Tests for merging CLI values with defaults.

#![allow(deprecated)]
use ortho_config::merge_cli_over_defaults;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct Sample {
    a: Option<u32>,
    b: Option<String>,
}

#[rstest]
fn cli_overrides_defaults() {
    let defaults = Sample {
        a: Some(1),
        b: Some("def".into()),
    };
    let cli = Sample {
        a: None,
        b: Some("cli".into()),
    };
    let merged = merge_cli_over_defaults(&defaults, &cli).expect("merge");
    assert_eq!(
        merged,
        Sample {
            a: Some(1),
            b: Some("cli".into())
        }
    );
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct Nested {
    inner: Option<Sample>,
}

#[rstest]
fn nested_structs_merge_deeply() {
    let defaults = Nested {
        inner: Some(Sample {
            a: Some(1),
            b: Some("def".into()),
        }),
    };
    let cli = Nested {
        inner: Some(Sample {
            a: None,
            b: Some("cli".into()),
        }),
    };
    let merged = merge_cli_over_defaults(&defaults, &cli).expect("merge");
    assert_eq!(
        merged,
        Nested {
            inner: Some(Sample {
                a: Some(1),
                b: Some("cli".into()),
            })
        }
    );
}
