//! Bounded precedence property test for the profile merge layer.
//!
//! Generates small per-layer values over a fixed key alphabet and asserts the
//! five-tier order `defaults < file < profile < environment < flags` holds for
//! every key. The strategy deliberately generates CLI values equal to the
//! default so the flag-equals-default rule is exercised (risk 3).

use proptest::prelude::*;
use serde_json::{Map, Value, json};

use crate::declarative::{MergeComposer, merge_value};

const KEYS: [&str; 3] = ["alpha", "beta", "gamma"];
const TIERS: [&str; 5] = ["cli", "environment", "profile", "file", "defaults"];

/// A bounded strategy over a subset of the fixed key alphabet.
fn layer_strategy() -> impl Strategy<Value = Value> {
    let keys = proptest::sample::select(vec![
        String::from("alpha"),
        String::from("beta"),
        String::from("gamma"),
    ]);
    let values = 0u32..3u32;
    prop::collection::hash_map(keys, values, 0..KEYS.len()).prop_map(|entries| {
        let mut object = Map::new();
        for (key, value) in entries {
            object.insert(key, Value::from(value));
        }
        Value::Object(object)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn five_tier_precedence_holds(
        defaults in layer_strategy(),
        file in layer_strategy(),
        profile in layer_strategy(),
        environment in layer_strategy(),
        cli in layer_strategy(),
    ) {
        let mut composer = MergeComposer::new();
        composer.push_defaults(defaults.clone());
        composer.push_file(file.clone(), None);
        composer.push_profile(profile.clone(), None);
        composer.push_environment(environment.clone());
        composer.push_cli(cli.clone());

        let mut buffer = json!(null);
        for layer in composer.layers() {
            merge_value(&mut buffer, layer.into_value());
        }

        for key in KEYS.iter().copied() {
            let winner = TIERS.iter().find_map(|tier| {
                let layer = match *tier {
                    "cli" => &cli,
                    "environment" => &environment,
                    "profile" => &profile,
                    "file" => &file,
                    _ => &defaults,
                };
                layer.get(key)
            });
            prop_assert_eq!(
                buffer.get(key),
                winner,
                "key {} must take the highest-precedence value",
                key
            );
        }
    }
}
