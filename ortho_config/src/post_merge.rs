//! Post-merge hook support for custom subcommand configuration logic.
//!
//! This module provides the [`PostMergeHook`] trait and [`PostMergeContext`]
//! struct that enable advanced merge customisation without manual glue code.

use crate::OrthoResult;
use camino::Utf8PathBuf;

/// Hook invoked after declarative layer merging completes.
///
/// Implement this trait to inject custom logic after the standard merge
/// pipeline has combined defaults, file, environment, and CLI layers.
/// The hook receives the merged struct and a context containing information
/// about the merge process.
///
/// # When to Use
///
/// Most configuration structs do not need a post-merge hook. The standard
/// merge pipeline handles layered precedence correctly for the vast majority
/// of use cases. Consider implementing this trait when you need to:
///
/// - Apply validation that depends on multiple fields being merged
/// - Normalize values after all layers have been applied
/// - Perform conditional transformations based on which sources contributed
/// - Clean up or adjust fields that interact in complex ways
///
/// # Examples
///
/// ```rust
/// use ortho_config::{OrthoConfig, OrthoResult, PostMergeContext, PostMergeHook};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Default, Deserialize, Serialize, OrthoConfig)]
/// #[ortho_config(prefix = "APP_", post_merge_hook)]
/// struct GreetArgs {
///     #[ortho_config(default = String::from("!"))]
///     punctuation: String,
///     preamble: Option<String>,
/// }
///
/// impl PostMergeHook for GreetArgs {
///     fn post_merge(&mut self, _ctx: &PostMergeContext) -> OrthoResult<()> {
///         // Normalize empty preambles to None
///         if self.preamble.as_ref().is_some_and(|p| p.trim().is_empty()) {
///             self.preamble = None;
///         }
///         Ok(())
///     }
/// }
/// ```
pub trait PostMergeHook: Sized {
    /// Adjusts the merged configuration after all layers have been applied.
    ///
    /// This method is called by the generated `merge_from_layers` function
    /// when the `#[ortho_config(post_merge_hook)]` attribute is present on
    /// the struct. The context provides metadata about the merge process
    /// that hooks may use for decision-making or diagnostics.
    ///
    /// # Errors
    ///
    /// Returns an error via [`crate::OrthoResult`] if post-merge adjustments
    /// fail. Errors are propagated to the caller of `merge_from_layers` or
    /// `load_and_merge`.
    fn post_merge(&mut self, ctx: &PostMergeContext) -> OrthoResult<()>;
}

/// Context provided to post-merge hooks.
///
/// Contains metadata about the merge process that hooks may use for
/// decision-making or diagnostics. The context is constructed by the
/// generated merge code and passed to [`PostMergeHook::post_merge`].
///
/// # Examples
///
/// ```rust
/// use ortho_config::PostMergeContext;
/// use camino::Utf8PathBuf;
///
/// let mut ctx = PostMergeContext::new("APP_");
/// ctx.with_file(Utf8PathBuf::from("/etc/app/config.toml"))
///    .with_cli_input();
///
/// assert_eq!(ctx.prefix(), "APP_");
/// assert!(ctx.has_cli_input());
/// assert_eq!(ctx.loaded_files().len(), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct PostMergeContext {
    prefix: String,
    loaded_files: Vec<Utf8PathBuf>,
    has_cli_input: bool,
}

impl PostMergeContext {
    /// Creates a new context with the given prefix.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::PostMergeContext;
    ///
    /// let ctx = PostMergeContext::new("MY_APP_");
    /// assert_eq!(ctx.prefix(), "MY_APP_");
    /// ```
    #[must_use]
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            loaded_files: Vec::new(),
            has_cli_input: false,
        }
    }

    /// Adds a loaded file path to the context.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::PostMergeContext;
    /// use camino::Utf8PathBuf;
    ///
    /// let mut ctx = PostMergeContext::new("APP_");
    /// ctx.with_file(Utf8PathBuf::from("/etc/app.toml"));
    /// assert_eq!(ctx.loaded_files().len(), 1);
    /// ```
    pub fn with_file(&mut self, path: Utf8PathBuf) -> &mut Self {
        self.loaded_files.push(path);
        self
    }

    /// Marks that CLI input was present in the merge.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::PostMergeContext;
    ///
    /// let mut ctx = PostMergeContext::new("APP_");
    /// ctx.with_cli_input();
    /// assert!(ctx.has_cli_input());
    /// ```
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Not const for API consistency with `with_file`, which cannot be const due to Vec::push"
    )]
    pub fn with_cli_input(&mut self) -> &mut Self {
        self.has_cli_input = true;
        self
    }

    /// Returns the prefix used during configuration loading.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::PostMergeContext;
    ///
    /// let ctx = PostMergeContext::new("MY_APP_");
    /// assert_eq!(ctx.prefix(), "MY_APP_");
    /// ```
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Returns the paths of configuration files that contributed to the merge.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::PostMergeContext;
    /// use camino::Utf8PathBuf;
    ///
    /// let mut ctx = PostMergeContext::new("APP_");
    /// ctx.with_file(Utf8PathBuf::from("/etc/app.toml"))
    ///    .with_file(Utf8PathBuf::from("~/.config/app.toml"));
    /// assert_eq!(ctx.loaded_files().len(), 2);
    /// ```
    #[must_use]
    pub fn loaded_files(&self) -> &[Utf8PathBuf] {
        &self.loaded_files
    }

    /// Returns whether CLI arguments were present in the merge.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::PostMergeContext;
    ///
    /// let ctx_without_cli = PostMergeContext::new("APP_");
    /// assert!(!ctx_without_cli.has_cli_input());
    ///
    /// let ctx_with_cli = PostMergeContext::new("APP_").with_cli_input();
    /// assert!(ctx_with_cli.has_cli_input());
    /// ```
    #[must_use]
    pub const fn has_cli_input(&self) -> bool {
        self.has_cli_input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_new_creates_empty_context() {
        let ctx = PostMergeContext::new("TEST_");
        assert_eq!(ctx.prefix(), "TEST_");
        assert!(ctx.loaded_files().is_empty());
        assert!(!ctx.has_cli_input());
    }

    #[test]
    fn context_with_file_adds_path() {
        let mut ctx = PostMergeContext::new("TEST_");
        ctx.with_file(Utf8PathBuf::from("/etc/test.toml"))
            .with_file(Utf8PathBuf::from("/home/user/.test.toml"));
        assert_eq!(ctx.loaded_files().len(), 2);
        assert_eq!(
            ctx.loaded_files().first().map(|p| p.as_str()),
            Some("/etc/test.toml")
        );
        assert_eq!(
            ctx.loaded_files().get(1).map(|p| p.as_str()),
            Some("/home/user/.test.toml")
        );
    }

    #[test]
    fn context_with_cli_input_sets_flag() {
        let mut ctx = PostMergeContext::new("TEST_");
        ctx.with_cli_input();
        assert!(ctx.has_cli_input());
    }

    #[test]
    fn context_builders_chain() {
        let mut ctx = PostMergeContext::new("APP_");
        ctx.with_file(Utf8PathBuf::from("/etc/app.toml"))
            .with_cli_input()
            .with_file(Utf8PathBuf::from("~/.app.toml"));

        assert_eq!(ctx.prefix(), "APP_");
        assert_eq!(ctx.loaded_files().len(), 2);
        assert!(ctx.has_cli_input());
    }
}
