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
    /// Effective option names mapped to their declared value types.
    options: BTreeMap<String, Option<String>>,
    /// Declared verb, when the bridge supplied one.
    pub(crate) canonical_verb: Option<String>,
}

impl ResolvedCommand {
    /// Returns whether the command accepts `option` exactly.
    pub(crate) fn accepts(&self, option: &str) -> bool {
        self.options.contains_key(option)
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
    let mut options: BTreeMap<String, Option<String>> = BTreeMap::new();
    for option in &contract.options {
        if option.long.is_empty() {
            return Err(CommandSurfaceError::EmptyOption { path });
        }
        insert_option(
            &mut options,
            &option.long,
            option.value_type.as_ref(),
            &path,
        )?;
    }
    Ok(ResolvedCommand {
        options,
        canonical_verb: contract.canonical_verb.clone(),
    })
}

/// Inserts one option, allowing a repeated identical declaration only.
fn insert_option(
    options: &mut BTreeMap<String, Option<String>>,
    long: &str,
    value_type: Option<&String>,
    path: &str,
) -> Result<(), CommandSurfaceError> {
    match options.get(long) {
        None => {
            options.insert(long.to_owned(), value_type.cloned());
            Ok(())
        }
        Some(existing) if existing.as_ref() == value_type => Ok(()),
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
