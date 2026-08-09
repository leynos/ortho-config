//! Tests for the first-class profile merge layer (milestone 2).
//!
//! Pins the five-tier ordering contract from decision D2 of execplan 9-1-1:
//! a layer pushed via `push_profile` merges above the file layer and below
//! the environment layer and reports `MergeProvenance::Profile`. The merge
//! steps use a hand-written [`DeclarativeMerge`] state machine because the
//! derive macro cannot be invoked inside this crate (its generated code
//! references the consumer crate by name).

use camino::Utf8PathBuf;
use pretty_assertions::assert_eq;
use rstest::rstest;
use serde::Deserialize;
use serde_json::json;
use std::borrow::Cow;

use crate::declarative::{from_value, merge_value};
use crate::{DeclarativeMerge, OrthoResult};

use super::{MergeComposer, MergeLayer, MergeProvenance};

#[derive(Debug, Deserialize, PartialEq)]
struct ProfileSample {
    retries: u32,
}

#[derive(Default)]
struct ProfileSampleMerge {
    buffer: serde_json::Value,
}

impl DeclarativeMerge for ProfileSampleMerge {
    type Output = ProfileSample;

    fn merge_layer(&mut self, layer: MergeLayer<'_>) -> OrthoResult<()> {
        merge_value(&mut self.buffer, layer.into_value());
        Ok(())
    }

    fn finish(self) -> OrthoResult<Self::Output> {
        from_value(self.buffer)
    }
}

fn merge_profile_layers(layers: Vec<MergeLayer<'static>>) -> anyhow::Result<ProfileSample> {
    let mut merge = ProfileSampleMerge::default();
    for layer in layers {
        merge.merge_layer(layer)?;
    }
    Ok(merge.finish()?)
}

#[test]
fn profile_layer_orders_above_file_and_below_environment() {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "retries": 1 }));
    composer.push_file(json!({ "retries": 3 }), None);
    composer.push_profile(json!({ "retries": 7 }), None);
    composer.push_environment(json!({ "retries": 9 }));
    composer.push_cli(json!({ "retries": 11 }));

    let provenances: Vec<MergeProvenance> = composer
        .layers()
        .iter()
        .map(MergeLayer::provenance)
        .collect();
    assert_eq!(
        provenances,
        vec![
            MergeProvenance::Defaults,
            MergeProvenance::File,
            MergeProvenance::Profile,
            MergeProvenance::Environment,
            MergeProvenance::Cli,
        ]
    );
}

#[rstest]
#[case::environment_beats_profile(Some(json!({ "retries": 9 })), 9)]
#[case::profile_beats_file(None, 7)]
fn profile_layer_merges_above_file_and_below_environment(
    #[case] environment: Option<serde_json::Value>,
    #[case] expected: u32,
) -> anyhow::Result<()> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(json!({ "retries": 1 }));
    composer.push_file(json!({ "retries": 3 }), None);
    composer.push_profile(json!({ "retries": 7 }), None);
    if let Some(environment_value) = environment {
        composer.push_environment(environment_value);
    }
    let config = merge_profile_layers(composer.layers())?;
    assert_eq!(config.retries, expected);
    Ok(())
}

#[test]
fn profile_layer_carries_profile_provenance_and_path() {
    let layer = MergeLayer::profile(
        Cow::Owned(json!({ "retries": 7 })),
        Some(Utf8PathBuf::from("config.toml")),
    );
    assert_eq!(layer.provenance(), MergeProvenance::Profile);
    assert_eq!(
        layer.path().map(camino::Utf8Path::as_str),
        Some("config.toml")
    );
}

#[test]
fn profile_layer_value_is_borrowable_without_consuming() {
    let layer = MergeLayer::profile(Cow::Owned(json!({ "retries": 7 })), None);
    assert_eq!(layer.value(), &json!({ "retries": 7 }));
    // the layer remains usable after borrowing its value
    assert_eq!(layer.provenance(), MergeProvenance::Profile);
}
