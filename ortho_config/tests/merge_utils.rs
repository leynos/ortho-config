//! Tests for merging CLI values with defaults.
use anyhow::{Result, anyhow, ensure};
use figment::{Figment, providers::Serialized};
use ortho_config::sanitized_provider;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct Sample {
    a: Option<u32>,
    b: Option<String>,
}

#[test]
fn cli_overrides_defaults() -> Result<()> {
    let defaults = Sample {
        a: Some(1),
        b: Some("def".into()),
    };
    let cli = Sample {
        a: None,
        b: Some("cli".into()),
    };
    let merged = merge_via_sanitized_cli(&defaults, &cli)?;
    let expected = Sample {
        a: Some(1),
        b: Some("cli".into()),
    };
    ensure!(
        merged == expected,
        "expected {:?}, got {:?}",
        expected,
        merged
    );
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct Nested {
    inner: Option<Sample>,
}

#[test]
fn nested_structs_merge_deeply() -> Result<()> {
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
    let merged = merge_via_sanitized_cli(&defaults, &cli)?;
    let expected = Nested {
        inner: Some(Sample {
            a: Some(1),
            b: Some("cli".into()),
        }),
    };
    ensure!(
        merged == expected,
        "expected {:?}, got {:?}",
        expected,
        merged
    );
    Ok(())
}

#[test]
fn cli_none_fields_do_not_override_defaults() -> Result<()> {
    let defaults = Sample {
        a: Some(42),
        b: Some("default".into()),
    };
    let cli = Sample { a: None, b: None };
    let merged = merge_via_sanitized_cli(&defaults, &cli)?;
    ensure!(
        merged == defaults,
        "expected defaults {:?}, got {:?}",
        defaults,
        merged
    );
    Ok(())
}

#[test]
fn nested_structs_partial_none_merge() -> Result<()> {
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
    let merged = merge_via_sanitized_cli(&defaults, &cli)?;
    let expected = Nested {
        inner: Some(Sample {
            a: Some(1),
            b: Some("cli".into()),
        }),
    };
    ensure!(
        merged == expected,
        "expected {:?}, got {:?}",
        expected,
        merged
    );
    Ok(())
}

#[test]
fn arrays_nulls_are_pruned_and_replace_defaults_in_cli_layer() -> Result<()> {
    #[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
    struct WithVec {
        #[serde(default)]
        items: Vec<Option<u32>>,
    }

    let defaults = WithVec {
        items: vec![Some(1)],
    };
    let cli = WithVec {
        items: vec![None, Some(2), None, Some(3)],
    };
    let merged = merge_via_sanitized_cli(&defaults, &cli)?;
    // Arrays are replaced at the CLI layer, not appended.
    let expected = vec![Some(2), Some(3)];
    ensure!(
        merged.items == expected,
        "expected {:?}, got {:?}",
        expected,
        merged.items
    );
    Ok(())
}

fn merge_via_sanitized_cli<T>(defaults: &T, cli: &T) -> Result<T>
where
    T: Serialize + serde::de::DeserializeOwned,
{
    let sanitized = sanitized_provider(cli).map_err(|err| anyhow!(err))?;
    let merged = Figment::from(Serialized::defaults(defaults))
        .merge(sanitized)
        .extract()
        .map_err(|err| anyhow!(err))?;
    Ok(merged)
}
