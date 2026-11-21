//! Conflicting key spellings observed through real generated loaders.
//!
//! A field and its alias name the same value, so a merged configuration that
//! holds both spellings is ambiguous. Serde raises a duplicate-field error on
//! the second spelling, but only after the first one has deserialized, so an
//! incomplete first spelling fails with a missing-field error instead. Either
//! way the failure is the deserializer's, and neither can be repaired by the
//! leaf a report would have advertised: supplying it leaves both spellings in
//! place. The collector must therefore stop at such a node instead of
//! descending into it.
//!
//! The canonical-only and alias-only cases are the controls. Each supplies the
//! same values under a single spelling and recovers them, so the failures above
//! are a property of the duplication rather than of the fixture.

use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

use super::{PROGRAM, load};

/// A nested child with one required and one optional leaf.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct ConflictDatabase {
    /// Required leaf the conflict is built around.
    host: String,
    /// Optional leaf used to populate one spelling only.
    #[serde(default)]
    port: u16,
}

/// A root whose nested object Serde accepts under two spellings.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRB_")]
struct AliasedConflictRoot {
    /// The canonical spelling and the alias name the same value.
    #[serde(alias = "db")]
    #[ortho_config(nested, skip_cli)]
    database: ConflictDatabase,
}

/// Assert that a load fails with the deserializer's own error, not a report.
///
/// A missing-value report here would mean the collector descended into an
/// ambiguous node and invented a requirement from one of the two spellings.
fn assert_not_enumerated<T: OrthoConfig>(
    result: ortho_config::OrthoResult<T>,
    expected: &str,
) -> Result<()> {
    let error = result
        .err()
        .ok_or_else(|| anyhow!("two spellings of one value must not load"))?;
    ensure!(
        !matches!(
            error.as_ref(),
            ortho_config::OrthoError::MissingRequiredValues(_)
        ),
        "the ambiguity belongs to the deserializer, not to a report: {error}"
    );
    ensure!(
        error.to_string().contains(expected),
        "expected the loader's own `{expected}` error, got: {error}"
    );
    Ok(())
}

/// One spelling may hold the missing leaf while the other holds a sibling.
///
/// This is the case a report would get wrong: the canonical key is present and
/// is missing `host`, but supplying that leaf would leave both spellings in the
/// merged value and still fail. Serde fails on the incomplete value before it
/// reaches the later spelling, so the missing-field error is what surfaces.
#[test]
fn a_partial_spelling_held_beside_its_alias_is_not_enumerated() -> Result<()> {
    assert_not_enumerated(
        load::<AliasedConflictRoot>(
            &[PROGRAM],
            &[("MRB_DATABASE__PORT", "1"), ("MRB_DB__HOST", "db")],
        ),
        "missing field `host`",
    )
}

/// A conflict is the duplication itself, not an incomplete child.
#[test]
fn two_complete_spellings_still_conflict() -> Result<()> {
    assert_not_enumerated(
        load::<AliasedConflictRoot>(
            &[PROGRAM],
            &[
                ("MRB_DATABASE__HOST", "db"),
                ("MRB_DATABASE__PORT", "1"),
                ("MRB_DB__HOST", "db"),
            ],
        ),
        "duplicate field",
    )
}

/// Either spelling on its own loads, so the failures above are the
/// duplication's and not the fixture's.
#[test]
fn either_spelling_alone_recovers_the_value() -> Result<()> {
    for (parent, spelling) in [("MRB_DATABASE__", "canonical"), ("MRB_DB__", "alias")] {
        let port = format!("{parent}PORT");
        let host = format!("{parent}HOST");
        let config =
            load::<AliasedConflictRoot>(&[PROGRAM], &[(port.as_str(), "1"), (host.as_str(), "db")])
                .map_err(|error| anyhow!("the {spelling} spelling alone must load: {error}"))?;
        ensure!(
            config.database.host == "db" && config.database.port == 1,
            "the {spelling} spelling must supply both leaves, got {:?}",
            config.database
        );
    }
    Ok(())
}
