//! Tests ensuring flattened CLI structs merge without overriding defaults.
#![allow(
    unfulfilled_lint_expectations,
    reason = "clippy::expect_used is denied globally; tests may not hit those branches"
)]
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]

use figment::{Figment, providers::Serialized};
use ortho_config::sanitized_provider;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct Inner {
    val: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct Outer {
    inner: Inner,
}

fn merge(defaults: &Outer, cli: &Outer) -> Outer {
    Figment::from(Serialized::defaults(defaults))
        .merge(sanitized_provider(cli).expect("sanitize"))
        .extract()
        .expect("merge")
}

#[rstest]
fn empty_flatten_like_struct_preserves_defaults() {
    let defaults = Outer {
        inner: Inner { val: Some(7) },
    };
    let cli = Outer::default();
    let merged = merge(&defaults, &cli);
    assert_eq!(merged, defaults);
}
