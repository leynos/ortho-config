//! Guards that `rstest-bdd` 0.6.0 really dispatches step returns by type.
//!
//! The sibling [`step_return_regression`](super::step_return_regression)
//! module drives real registered steps, so it shows what this repository
//! observes but cannot separate "the new dispatch works" from "the new
//! dispatch is absent and nothing here notices". This module supplies that
//! missing control by reaching the classification bridge the 0.6.0 step
//! wrapper is built on, and by emulating what the pre-migration wrapper did.
//!
//! The comparison is exact rather than approximate. In `rstest-bdd-macros`
//! 0.5.0 `ReturnKind::Value` emitted
//! `Ok(__rstest_bdd_payload_from_value(#call))` — see
//! `codegen/wrapper/emit/call_expr.rs`. A step returning a user-defined alias
//! such as `anyhow::Result<()>` fell into that arm, because
//! `return_classifier.rs` resolved only syntactic `Result<..>` paths and did
//! not resolve type aliases. The alias was therefore boxed as a success value
//! and its `Err` was never propagated. The emulation below reproduces the
//! 0.5.0 helper and wrapper directly.
//!
//! The two behaviour checks are differential: they ask that 0.6.0 and 0.5.0
//! *disagree*, so they fail both when the new bridge stops classifying a
//! `Result` alias as a `Result` and when the recorded old behaviour is no
//! longer what produced the false green. A check that only asserted the new
//! behaviour would still pass if that behaviour were never exercised. The two
//! tag checks then pin the discrimination those comparisons depend on, so a
//! probe that answered with one tag for every input could not pass unnoticed.

use rstest_bdd::step_return::{
    StepReturnNormalize, StepReturnProbe, StepReturnResultTag, StepReturnValueKind,
    StepReturnValueTag,
};

/// The exact alias both this crate's steps and its regression module use.
type StepAlias<T> = anyhow::Result<T>;

/// A non-unit payload that must not be mistaken for the alias itself.
#[derive(Debug, PartialEq, Eq)]
struct Level(u8);

/// Reproduces `rstest_bdd` 0.5.0's payload helper.
///
/// Kept as a local copy so the emulation is checked against the shape the
/// pre-migration crate actually had, not against the 0.6.0 helper, which is
/// the behaviour under test.
fn pre_migration_payload_from_value<T: std::any::Any>(value: T) -> Option<Box<dyn std::any::Any>> {
    if std::any::TypeId::of::<T>() == std::any::TypeId::of::<()>() {
        None
    } else {
        Some(Box::new(value) as Box<dyn std::any::Any>)
    }
}

/// Reproduces `rstest-bdd-macros` 0.5.0's `ReturnKind::Value` wrapper arm.
fn pre_migration_value_arm<T: std::any::Any>(
    returned: T,
) -> std::result::Result<Option<Box<dyn std::any::Any>>, String> {
    Ok(pre_migration_payload_from_value(returned))
}

/// Probes one return value through the 0.6.0 classification bridge.
///
/// This is the same two-step path the generated wrapper takes: select the
/// kind, then normalize through the selected tag.
fn post_migration_normalize<T: std::any::Any, E: std::fmt::Display>(
    returned: std::result::Result<T, E>,
) -> std::result::Result<Option<Box<dyn std::any::Any>>, String> {
    StepReturnProbe(&returned)
        .__rstest_bdd_step_return_kind()
        .normalize(returned)
}

/// A failing aliased step is propagated by 0.6.0 and swallowed by 0.5.0.
///
/// This is the regression the migration had to prevent, stated as the
/// difference between the two wrappers rather than as an expectation about
/// either one alone.
#[test]
fn a_failing_alias_is_propagated_only_after_migration() {
    let legacy = pre_migration_value_arm::<StepAlias<()>>(Err(anyhow::anyhow!("alias failed")))
        .expect("the 0.5.0 value arm never produced a step error");
    assert!(
        legacy.is_some(),
        "the 0.5.0 value arm must box the whole alias as a success payload"
    );

    let error = post_migration_normalize::<(), _>(Err(anyhow::anyhow!("alias failed")))
        .expect_err("0.6.0 must propagate a failing Result alias as a step error");
    assert_eq!(error, "alias failed", "the handler message must survive");
}

/// A succeeding aliased step injects its inner payload only after migration.
///
/// The two arms disagree about *what* is injected: 0.5.0 boxes the alias
/// itself, so the payload is `Result<Level, Error>`, while 0.6.0 boxes the
/// inner `Level`. Asserting that difference prevents the check from passing if
/// the bridge silently stopped unwrapping.
#[test]
fn a_succeeding_alias_injects_the_inner_value_only_after_migration() {
    let legacy = pre_migration_value_arm::<StepAlias<Level>>(Ok(Level(7)))
        .expect("the 0.5.0 value arm never produced a step error")
        .expect("the 0.5.0 value arm must box the alias");
    assert!(
        legacy.downcast_ref::<StepAlias<Level>>().is_some(),
        "the 0.5.0 payload must be the boxed alias, not its inner value"
    );

    let migrated = post_migration_normalize::<Level, anyhow::Error>(Ok(Level(7)))
        .expect("0.6.0 must not fail a successful Result alias")
        .expect("0.6.0 must inject the inner payload");
    assert_eq!(
        migrated.downcast_ref::<Level>(),
        Some(&Level(7)),
        "the 0.6.0 payload must be the inner value, not the boxed alias"
    );
}

/// The bridge is the type-directed one this module assumes it is.
///
/// [`StepReturnProbe`] resolves a `Result` through inherent-method precedence;
/// if a `StepReturnResultTag` were ever produced for something that is not a
/// `Result`, the checks above would be testing the wrong arm. Comparing tag
/// type names makes the discrimination explicit in the test file rather than
/// implied by the upstream implementation, and the companion case below is
/// what stops a probe that answered `StepReturnResultTag` unconditionally from
/// satisfying every check in this module.
#[test]
fn the_result_probe_selects_the_result_tag() {
    let value: StepAlias<()> = Ok(());
    let tag = StepReturnProbe(&value).__rstest_bdd_step_return_kind();
    assert_eq!(
        std::any::type_name_of_val(&tag),
        std::any::type_name_of_val(&StepReturnResultTag),
        "an aliased Result must select the result arm"
    );
}

/// A non-`Result` payload still selects the fallback value arm.
///
/// The regression steps above only ever return aliased `Result`s, so without
/// this case nothing in the module would notice a probe that stopped
/// discriminating at all.
#[test]
fn a_plain_value_selects_the_value_tag() {
    let value = Level(7);
    let tag = StepReturnProbe(&value).__rstest_bdd_step_return_kind();
    assert_eq!(
        std::any::type_name_of_val(&tag),
        std::any::type_name_of_val(&StepReturnValueTag),
        "a plain payload must select the value arm"
    );
}
