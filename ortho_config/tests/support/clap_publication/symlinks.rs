//! Staged symlink controls for the `ErrorKind` source walk.
//!
//! A committed symlink does not survive a Windows checkout, where it
//! materialises as a text file naming its target, so each tree here is staged
//! in a temporary directory at test time. The two controls sit together
//! because they answer opposite halves of one question: which symlinks the walk
//! may ignore and which it must refuse.
//!
//! The walk reads regular `.rs` files only. That is what stops an alias from
//! being counted as a second declaration, and it is also why a declared module
//! file that is a symlink leaves the compiler loading source the walk never
//! parses. The first control requires the alias to stay uncounted; the second
//! requires the module it hides to be refused rather than silently skipped.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use tempfile::TempDir;

use super::source::count_error_kinds_under;

/// The declaring module the alias control points at.
const ALIASED_ERROR_KIND: &str = concat!(
    "//! The declaring module of a synthetic crate.\n",
    "\n",
    "/// Kinds of error produced while parsing command-line arguments.\n",
    "#[non_exhaustive]\n",
    "pub enum ErrorKind {\n",
    "    /// A value was supplied that failed validation.\n",
    "    InvalidValue,\n",
    "    /// An argument was supplied that the command does not define.\n",
    "    UnknownArgument,\n",
    "    /// A subcommand was supplied that the command does not define.\n",
    "    InvalidSubcommand,\n",
    "}\n",
);

/// The inventory a declared module is a symlink to.
const CONTAINED_ERROR_KIND: &str = concat!(
    "//! The inventory a symlinked module file would contribute.\n",
    "\n",
    "/// Kinds of error produced while parsing command-line arguments.\n",
    "pub enum ErrorKind {\n",
    "    InvalidValue,\n",
    "    UnknownArgument,\n",
    "    InvalidSubcommand,\n",
    "    ValueValidation,\n",
    "}\n",
);

/// A tree staged in a fresh temporary directory.
///
/// The directory handle owns the temporary directory for as long as the test
/// needs it, so the tree is removed even when an assertion fails.
fn staged_tree() -> Result<(TempDir, Dir)> {
    let directory = TempDir::new().context("create a directory for a symlink tree")?;
    let root = Utf8Path::from_path(directory.path())
        .context("the temporary directory should be valid UTF-8")?;
    let tree = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open {root} to stage a symlink tree"))?;
    Ok((directory, tree))
}

/// The directory the staged tree sits in, for a walk that needs a path.
fn staged_root(directory: &TempDir) -> Result<&Utf8Path> {
    Utf8Path::from_path(directory.path()).context("the temporary directory should be valid UTF-8")
}

/// The refusal message, or an empty string when the check passed.
fn refusal(result: Result<()>) -> String {
    match result {
        Ok(()) => String::new(),
        Err(error) => error.to_string(),
    }
}

/// A symlink alias is not read as a second declaration.
///
/// The inventory covers every regular `.rs` file under the crate root. A
/// symlink named `alias.rs` pointing at `error.rs` is not one of them: reading
/// it would report the same `ErrorKind` twice and refuse a package that is
/// perfectly valid.
#[test]
fn symlink_alias_is_counted_once() -> Result<()> {
    let (directory, tree) = staged_tree()?;
    tree.write("lib.rs", "mod error;\n")
        .context("write the crate root")?;
    tree.write("error.rs", ALIASED_ERROR_KIND)
        .context("write the declaring module")?;
    tree.symlink("error.rs", "alias.rs")
        .context("alias the declaring module")?;

    let count = count_error_kinds_under(staged_root(&directory)?)?;
    ensure!(
        count == 3,
        "the alias must not be counted as a second declaration, found {count}",
    );
    Ok(())
}

/// A declared module file the walk cannot read is refused, not skipped.
///
/// `error.rs` is a symlink to a non-Rust file, so the compiler loads an
/// inventory the walk never parses while `decoy.rs` declares only the three
/// variants it can see. Certifying no file for `mod error;` is the fail-closed
/// direction: the count would otherwise describe a crate whose exported
/// inventory is four variants long.
#[test]
fn symlinked_module_file_is_refused() -> Result<()> {
    let (directory, tree) = staged_tree()?;
    tree.write("lib.rs", "mod decoy;\n\nmod error;\n")
        .context("write the crate root")?;
    tree.write("decoy.rs", ALIASED_ERROR_KIND)
        .context("write the compiled decoy")?;
    tree.write("inventory.txt", CONTAINED_ERROR_KIND)
        .context("write the contained inventory")?;
    tree.symlink("inventory.txt", "error.rs")
        .context("alias the contained inventory as a module file")?;

    let message = refusal(count_error_kinds_under(staged_root(&directory)?).map(|_| ()));
    ensure!(
        message.contains("resolves to no file this walk reads"),
        "{message:?} should name the unresolved module file",
    );
    Ok(())
}

/// A conditional module file the walk cannot read is refused, not skipped.
///
/// The condition does not make the declaration unreachable: `#[cfg(all())]`
/// always holds, so the compiler loads `error.rs` exactly as it would an
/// unconditional hop. A walk that skips every conditional declaration before
/// resolving its file never notices that `error.rs` is a symlink to the
/// four-variant inventory, and would certify the three-variant decoy beside it.
#[test]
fn conditional_symlinked_module_file_is_refused() -> Result<()> {
    let (directory, tree) = staged_tree()?;
    tree.write("lib.rs", "mod decoy;\n\n#[cfg(all())]\nmod error;\n")
        .context("write the crate root")?;
    tree.write("decoy.rs", ALIASED_ERROR_KIND)
        .context("write the compiled decoy")?;
    tree.write("inventory.txt", CONTAINED_ERROR_KIND)
        .context("write the contained inventory")?;
    tree.symlink("inventory.txt", "error.rs")
        .context("alias the contained inventory as a conditional module file")?;

    let message = refusal(count_error_kinds_under(staged_root(&directory)?).map(|_| ()));
    ensure!(
        message.contains("is conditional and resolves to no file this walk reads"),
        "{message:?} should name the conditional module whose source is unreadable",
    );
    Ok(())
}
