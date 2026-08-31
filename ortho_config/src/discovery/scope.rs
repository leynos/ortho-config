//! Scope labels and automatic-discovery modes for file configuration layers.

/// A platform-discovery scope.
///
/// Scopes describe automatic locations only. Explicit paths and selector rungs
/// intentionally have no scope because a policy resolves them before automatic
/// discovery begins.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum DiscoveryScope {
    /// Machine-wide configuration, such as `/etc/xdg`.
    System,
    /// Per-user configuration, such as XDG home and application-data folders.
    User,
    /// Configuration rooted in the current project.
    Project,
}

/// How automatic candidates become file layers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum AutomaticMode {
    /// Retain the historic first-successful-file behaviour.
    #[default]
    FirstWins,
    /// Load the first successful extends chain in every requested scope.
    StackScopes,
}
