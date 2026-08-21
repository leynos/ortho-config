//! Helpers for Cargo external-subcommand (`cargo-<name>`) entry points.
//!
//! Cargo dispatches `cargo <name>` by locating a binary named `cargo-<name>`
//! on `PATH` and executing it with the subcommand name injected as the
//! second argument: argv becomes `["<path-to>/cargo-<name>", "<name>",
//! OPTIONS...]`. A hand-built [`clap::Command`] that models only
//! `cargo-<name> [OPTIONS]` rejects that injected `<name>` token before
//! application logic can run. [`external_subcommand`] wraps an existing
//! command in the standard wrapper shape: a synthetic `cargo` parent that
//! requires a single subcommand carrying the tool's real options, so both
//! `cargo <name> [OPTIONS]` (Cargo dispatch) and `cargo-<name> <name>
//! [OPTIONS]` (direct invocation with the same injected token) parse without
//! duplicating parser setup.
//!
//! The wrapper is CLI entry-point structure only. It performs no
//! configuration loading, reads no environment state, and does not change
//! `OrthoConfig`'s merge precedence: options simply move one level down, and
//! callers read them through
//! [`subcommand_matches`](clap::ArgMatches::subcommand_matches) on the
//! injected subcommand name rather than from the top-level matches.
//!
//! The helper deliberately leaves styling, a parent-level version flag,
//! invocation-hint error text, and tracing setup to the binary; see the
//! users' guide for those binary-level obligations. Transparently accepting
//! a bare `cargo-<name> [OPTIONS]` invocation (without the injected token)
//! by normalizing argv is a different pattern and is out of scope for this
//! module.
//!
//! # Examples
//!
//! ```rust
//! use ortho_config::cargo::external_subcommand;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let args_command = clap::Command::new("demo")
//!     .version("1.2.3")
//!     .arg(
//!         clap::Arg::new("verbose")
//!             .long("verbose")
//!             .action(clap::ArgAction::SetTrue),
//!     );
//! let cli = external_subcommand("cargo-demo", "demo", args_command);
//! let matches = cli.try_get_matches_from(["cargo-demo", "demo", "--verbose"])?;
//! // The inner options live one level down, under the subcommand:
//! let demo = matches
//!     .subcommand_matches("demo")
//!     .expect("subcommand_required guarantees a subcommand");
//! assert!(demo.get_flag("verbose"));
//! # Ok(())
//! # }
//! ```

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;

/// Wraps a hand-built `clap::Command` in the standard Cargo
/// external-subcommand shape.
///
/// Cargo runs `cargo <name>` by executing `cargo-<name>` with the
/// subcommand name injected as the second argument. The returned command
/// models that protocol: a synthetic `cargo` parent that requires the
/// `<name>` subcommand, so both `cargo <name> [OPTIONS]` and
/// `cargo-<name> <name> [OPTIONS]` parse with the caller's original
/// options, which callers read through
/// [`matches.subcommand_matches("<name>")`](clap::ArgMatches::subcommand_matches).
/// The wrapper performs no configuration loading and reads no environment
/// state.
///
/// The inner command is renamed to `subcommand_name`, its `bin_name` is
/// reset so usage renders `cargo <name>` on both the help and parse paths,
/// and its `display_name` is set to `installed_bin_name` so `--version`
/// output names the installed binary. All other inner options, help text,
/// and the inner version are preserved verbatim. The returned command is an
/// ordinary `clap::Command` that the caller may customize further. Styling,
/// top-level version flags, invocation-hint error text, and tracing setup
/// remain the caller's responsibility (see the users' guide).
///
/// # Preconditions
///
/// `installed_bin_name` must equal `"cargo-"` followed by
/// `subcommand_name` (Cargo derives the injected token from the binary file
/// name), and `subcommand_name` must be non-empty and must not be `help`,
/// which is reserved by clap's auto-generated help subcommand. Violations
/// are programming errors and trip debug assertions.
///
/// # Examples
///
/// ```rust
/// use ortho_config::cargo::external_subcommand;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let args_command = clap::Command::new("demo")
///     .arg(clap::Arg::new("all").long("all"));
/// let cli = external_subcommand("cargo-demo", "demo", args_command);
///
/// // Cargo dispatch: `cargo demo --all` injects the `demo` token.
/// let matches = cli.try_get_matches_from(["cargo-demo", "demo", "--all"])?;
/// assert!(matches.subcommand_matches("demo").is_some());
/// # Ok(())
/// # }
/// ```
#[must_use]
pub fn external_subcommand(
    installed_bin_name: impl Into<String>,
    subcommand_name: impl Into<clap::builder::Str>,
    command: clap::Command,
) -> clap::Command {
    let installed = installed_bin_name.into();
    let name = subcommand_name.into();
    debug_assert!(!name.as_str().is_empty(), "subcommand name is empty");
    debug_assert_ne!(
        name.as_str(),
        "help",
        "subcommand name must not be clap's reserved help command",
    );
    debug_assert_eq!(
        installed,
        format!("cargo-{name}"),
        "installed binary name must be cargo-<subcommand name>",
    );
    // `Resettable::Reset` clears a caller-set inner `bin_name`; clap 4.6
    // exposes no `Option<String>` conversion for `IntoResettable<String>`,
    // so the explicit reset value is the supported form.
    clap::Command::new("cargo")
        .bin_name("cargo")
        .subcommand_required(true)
        .subcommand(
            command
                .name(name)
                .bin_name(clap::builder::Resettable::Reset)
                .display_name(installed),
        )
}
