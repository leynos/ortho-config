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
    ///
    /// The candidate list is scanned most-preferred first and the scan stops at
    /// the first file that loads, whichever scope it belongs to.
    #[default]
    FirstWins,
    /// Load every applicable file in every requested scope.
    ///
    /// Scopes are applied in the requested order, so a later scope overrides an
    /// earlier one. Within a scope the candidate list is a preference order:
    /// the least-preferred candidate that loads is applied first and the
    /// most-preferred is applied last, so the historic winner still wins. That
    /// reversal is what lets one rule — later applied wins — serve both the
    /// layer stack and the historical preference order; without it a fallback
    /// such as `~/.demo.toml` would override `$XDG_CONFIG_HOME/demo/config.toml`
    /// as soon as a second location started loading.
    StackScopes,
}
