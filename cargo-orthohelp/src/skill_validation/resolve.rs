//! Exact command-surface indexing for skill validation.
//!
//! The index is built once per validation call so claim checking is a lookup,
//! never a scan of the whole command tree. Paths and option names are compared
//! exactly: no case folding, alias expansion, punctuation stripping, or `--`
//! prefix removal, because 6.3.1 defines both contracts as exact.

use std::collections::BTreeMap;

use super::input::{CommandContract, CommandSurfaceError, EffectiveCommandSurface};

/// One command's resolved option metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedCommand {
    /// Effective option names mapped to their resolved metadata.
    options: BTreeMap<String, ResolvedOption>,
    /// Declared verb, when the bridge supplied one.
    pub(crate) canonical_verb: Option<String>,
}

/// One option as the supplied surface declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedOption {
    /// Declared value type, when the surface states one.
    value_type: Option<String>,
    /// Whether the surface classifies the option as global.
    is_global: bool,
}

impl ResolvedOption {
    /// Returns whether this option is a global option on its command.
    ///
    /// Policy asks this to decide whether the shared glossary's rules apply at
    /// all; a domain option that happens to share a canonical name is not one
    /// of the glossary's options.
    pub(crate) const fn is_global(&self) -> bool {
        self.is_global
    }
}

impl ResolvedCommand {
    /// Returns the resolved metadata for `option`, when it is accepted.
    ///
    /// `None` is the exact-membership answer: the command does not accept the
    /// option at all.
    pub(crate) fn option(&self, option: &str) -> Option<&ResolvedOption> {
        self.options.get(option)
    }
}

/// A path-keyed index of the effective command surface.
#[derive(Debug, Clone, Default)]
pub(crate) struct CommandIndex {
    /// Commands keyed by their exact path.
    commands: BTreeMap<Vec<String>, ResolvedCommand>,
}

impl CommandIndex {
    /// Builds the index, rejecting an ambiguous surface before any claim is
    /// evaluated.
    pub(crate) fn build(surface: &EffectiveCommandSurface) -> Result<Self, CommandSurfaceError> {
        let mut commands = BTreeMap::new();
        for (index, contract) in surface.commands.iter().enumerate() {
            if contract.path.is_empty() {
                return Err(CommandSurfaceError::EmptyPath { index });
            }
            let resolved = resolve_command(contract)?;
            if commands.insert(contract.path.clone(), resolved).is_some() {
                return Err(CommandSurfaceError::DuplicatePath {
                    path: join_path(&contract.path),
                });
            }
        }
        Ok(Self { commands })
    }

    /// Resolves one exact path.
    pub(crate) fn command(&self, path: &[String]) -> Option<&ResolvedCommand> {
        self.commands.get(path)
    }
}

/// Resolves one command contract, rejecting empty or conflicting option names.
fn resolve_command(contract: &CommandContract) -> Result<ResolvedCommand, CommandSurfaceError> {
    let path = join_path(&contract.path);
    let mut options: BTreeMap<String, ResolvedOption> = BTreeMap::new();
    for option in &contract.options {
        if option.long.is_empty() {
            return Err(CommandSurfaceError::EmptyOption { path });
        }
        let resolved = ResolvedOption {
            value_type: option.value_type.clone(),
            is_global: option.is_global,
        };
        insert_option(&mut options, &option.long, resolved, &path)?;
    }
    Ok(ResolvedCommand {
        options,
        canonical_verb: contract.canonical_verb.clone(),
    })
}

/// Inserts one option, allowing a repeated identical declaration only.
fn insert_option(
    options: &mut BTreeMap<String, ResolvedOption>,
    long: &str,
    resolved: ResolvedOption,
    path: &str,
) -> Result<(), CommandSurfaceError> {
    match options.get(long) {
        None => {
            options.insert(long.to_owned(), resolved);
            Ok(())
        }
        // A repeated declaration that disagrees on *either* field is
        // ambiguous: the surface would make policy advice depend on list
        // order.
        Some(existing) if *existing == resolved => Ok(()),
        Some(_) => Err(CommandSurfaceError::ConflictingOption {
            path: path.to_owned(),
            option: long.to_owned(),
        }),
    }
}

/// Joins a path for diagnostics using spaces.
pub(crate) fn join_path(path: &[String]) -> String {
    path.join(" ")
}
