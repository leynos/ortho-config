//! Post-load selection surfacing (decision D14).

use super::SelectedProfile;

/// Configuration plus the profile selection that produced it.
///
/// Returned by the generated `load_with_profile_from_iter`/`load_with_profile`
/// entry points on opted-in structs. The selection is empty when no profile
/// was selected and a singleton when one was; the slice shape allows multiple
/// simultaneous profiles to arrive additively later.
#[derive(Debug)]
pub struct ProfileLoadOutcome<T> {
    config: T,
    selection: Vec<SelectedProfile>,
}

impl<T> ProfileLoadOutcome<T> {
    /// Wrap a loaded config and its selection.
    ///
    /// Public for derive-generated code; downstream code obtains outcomes only
    /// from the generated load functions.
    #[doc(hidden)]
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Moving the Vec field out of self prevents a const constructor"
    )]
    pub fn new(config: T, selection: Vec<SelectedProfile>) -> Self {
        Self { config, selection }
    }

    /// Borrow the loaded configuration.
    #[must_use]
    pub const fn config(&self) -> &T {
        &self.config
    }

    /// Consume the outcome and return the loaded configuration.
    #[must_use]
    pub fn into_config(self) -> T {
        self.config
    }

    /// The selected profiles (empty or singleton today).
    #[must_use]
    pub fn selection(&self) -> &[SelectedProfile] {
        &self.selection
    }
}
