//! Shared helpers for BDD step definitions.
//!
//! Several step modules repeat the same "guard, validate, and populate a
//! scalar slot" shape, and the same "take the slot's value or fail with a
//! descriptive error" shape. Centralising both here keeps individual step
//! modules focused on scenario wiring rather than boilerplate.

use anyhow::{Result, anyhow, ensure};
use rstest_bdd::Slot;

/// Populates `slot` with `value`, failing when the slot already holds a
/// value.
///
/// Use [`set_nonblank_scalar_once`] instead when blank (whitespace-only)
/// input must also be rejected.
pub(crate) fn set_scalar_once(
    slot: &Slot<String>,
    value: impl Into<String>,
    label: &str,
) -> Result<()> {
    ensure!(slot.is_empty(), "{label} already initialised");
    slot.set(value.into());
    Ok(())
}

/// As [`set_scalar_once`], but first rejects blank (whitespace-only) input.
pub(crate) fn set_nonblank_scalar_once(
    slot: &Slot<String>,
    value: impl Into<String>,
    label: &str,
) -> Result<()> {
    let value = value.into();
    ensure!(!value.trim().is_empty(), "{label} must not be empty");
    set_scalar_once(slot, value, label)
}

/// Extension trait collapsing the repeated
/// `slot.take().ok_or_else(|| anyhow!(...))` boilerplate found throughout
/// the step definitions.
pub(crate) trait SlotTakeOrExt<T> {
    /// Takes the slot's value, or fails with `message` when the slot is
    /// empty.
    fn take_or(&self, message: &str) -> Result<T>;
}

impl<T> SlotTakeOrExt<T> for Slot<T> {
    fn take_or(&self, message: &str) -> Result<T> {
        self.take().ok_or_else(|| anyhow!("{message}"))
    }
}
