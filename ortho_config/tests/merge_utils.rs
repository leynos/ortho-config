//! Tests for merging CLI values with defaults.

use figment::{Figment, providers::Serialized};
use ortho_config::sanitize_value;
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
    let merged = merge_via_sanitized_cli(&defaults, &cli);
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

#[test]
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
    let merged = merge_via_sanitized_cli(&defaults, &cli);
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

#[test]
fn cli_none_fields_do_not_override_defaults() {
    let defaults = Sample {
        a: Some(42),
        b: Some("default".into()),
    };
    let cli = Sample { a: None, b: None };
    let merged = merge_via_sanitized_cli(&defaults, &cli);
    assert_eq!(merged, defaults);
}

#[test]
fn nested_structs_partial_none_merge() {
    let defaults = Nested {
        inner: Some(Sample {
            a: Some(1),
            b: None,
        }),
    };
    let cli = Nested {
        inner: Some(Sample {
            a: None,
            b: Some("cli".into()),
        }),
    };
    let merged = merge_via_sanitized_cli(&defaults, &cli);
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

fn merge_via_sanitized_cli<T>(defaults: &T, cli: &T) -> T
where
    T: Serialize + serde::de::DeserializeOwned + Default,
{
    let sanitized = sanitize_value(cli).expect("sanitize");
    Figment::from(Serialized::defaults(defaults))
        .merge(Serialized::defaults(&sanitized))
        .extract()
        .expect("merge")
}
