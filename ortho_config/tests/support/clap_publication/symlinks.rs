//! Staged symlink controls for the `ErrorKind` source walk.
//!
//! A committed symlink does not survive a Windows checkout, where it
//! materialises as a text file naming its target, so each tree here is staged
//! in a temporary directory at test time. The controls sit together because
//! they answer opposite halves of one question: which symlinks the walk may
//! ignore and which it must refuse.
//!
//! The walk reads regular `.rs` files only. That is what stops an alias from
//! being counted as a second declaration, and it is also why a declared module
//! file that is a symlink leaves the compiler loading source the walk never
//! parses. The first control requires the alias to stay uncounted; the others
//! require the module a symlink hides to be refused rather than silently
//! skipped, whether the declaration carrying it is conditional or sits below a
//! conditional inline module.

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

/// Stages a compiled decoy beside an inventory hidden behind `alias`.
///
/// `crate_root` is the whole `lib.rs`, so each control states the declaration
/// shape it is about. `alias` is the module file the walk must fail to read: a
/// symlink to `contained`, a non-Rust file the walk never parses. Returns the
/// refusal message, which is empty only if the walk accepted the tree.
fn refusal_for_hidden_inventory(crate_root: &str, alias: &str, contained: &str) -> Result<String> {
    let (directory, tree) = staged_tree()?;
    tree.write("lib.rs", crate_root)
        .context("write the crate root")?;
    tree.write("decoy.rs", ALIASED_ERROR_KIND)
        .context("write the compiled decoy")?;
    if let Some((parent, _)) = alias.rsplit_once('/') {
        tree.create_dir(parent)
            .with_context(|| format!("create {parent} for the nested module file"))?;
    }
    tree.write(contained, CONTAINED_ERROR_KIND)
        .context("write the contained inventory")?;
    tree.symlink(contained, alias)
        .with_context(|| format!("alias the contained inventory as {alias}"))?;
    Ok(refusal(
        count_error_kinds_under(staged_root(&directory)?).map(|_| ()),
    ))
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
    let message =
        refusal_for_hidden_inventory("mod decoy;\n\nmod error;\n", "error.rs", "inventory.txt")?;
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
    let message = refusal_for_hidden_inventory(
        "mod decoy;\n\n#[cfg(all())]\nmod error;\n",
        "error.rs",
        "inventory.txt",
    )?;
    ensure!(
        message.contains("is conditional and resolves to no file this walk reads"),
        "{message:?} should name the conditional module whose source is unreadable",
    );
    Ok(())
}

/// A module declared below a conditional inline module is still resolved.
///
/// Skipping a conditional inline module also skips every declaration in its
/// body, and `#[cfg(all())]` means the compiler compiles that body. Here the
/// skipped declaration names a module file that is a symlink, so a walk that
/// leaves the body unaudited certifies the three-variant decoy while the
/// compiler exports the four-variant inventory. The body is therefore inspected
/// even though it can never be certified.
#[test]
fn conditional_inline_module_hop_is_still_resolved() -> Result<()> {
    let message = refusal_for_hidden_inventory(
        concat!(
            "mod decoy;\n",
            "\n",
            "#[cfg(all())]\n",
            "pub mod error {\n",
            "    pub mod inventory;\n",
            "    pub use inventory::ErrorKind;\n",
            "}\n",
        ),
        "error/inventory.rs",
        "error/inventory.txt",
    )?;
    ensure!(
        message.contains("resolves to no file this walk reads"),
        "{message:?} should name the nested module file the walk cannot read",
    );
    Ok(())
}
