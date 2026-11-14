//! Exact command-surface indexing for skill validation.
//!
//! The index is built once per validation call so claim checking is a lookup,
//! never a scan of the whole command tree. Paths and option names are compared
//! exactly: no case folding, alias expansion, punctuation stripping, or `--`
//! prefix removal, because 6.3.1 defines both contracts as exact.
//!
//! Paths and names travel as small domain wrappers rather than bare strings.
//! The wrappers state which `String` is a command path, an option name, or a
//! rendered diagnostic, so a lookup cannot silently swap one for another and
//! no signature has to spell out a generic `&str`.

use std::collections::BTreeMap;
use std::fmt;

use super::input::{CommandContract, CommandSurfaceError, EffectiveCommandSurface};

/// One exact command path, borrowed from the surface or a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandPath<'a> {
    /// Path segments in the order the bridge supplied them.
    segments: &'a [String],
}

impl<'a> CommandPath<'a> {
    /// Borrows path segments as one exact path.
    pub(crate) const fn new(segments: &'a [String]) -> Self {
        Self { segments }
    }

    /// Returns the segments this path was built from.
    const fn segments(self) -> &'a [String] {
        self.segments
    }
}

/// One exact option name, without the leading `--`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OptionName<'a> {
    /// Name as the surface or claim states it.
    name: &'a str,
}

impl<'a> OptionName<'a> {
    /// Borrows an option name.
    pub(crate) const fn new(name: &'a str) -> Self {
        Self { name }
    }

    /// Returns the name without alteration.
    const fn as_str(self) -> &'a str {
        self.name
    }
}

/// A command path rendered for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedPath {
    /// Segments joined by single spaces.
    rendered: String,
}

impl RenderedPath {
    /// Returns the rendered path.
    pub(crate) fn as_str(&self) -> &str {
        &self.rendered
    }

    /// Consumes the rendered path, yielding the diagnostic string.
    pub(crate) fn into_inner(self) -> String {
        self.rendered
    }
}

impl fmt::Display for RenderedPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.rendered)
    }
}

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
    pub(crate) fn option(&self, option: OptionName<'_>) -> Option<&ResolvedOption> {
        self.options.get(option.as_str())
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
                let path = join_path(CommandPath::new(&contract.path)).into_inner();
                return Err(CommandSurfaceError::DuplicatePath { path });
            }
        }
        Ok(Self { commands })
    }

    /// Resolves one exact path.
    pub(crate) fn command(&self, path: CommandPath<'_>) -> Option<&ResolvedCommand> {
        self.commands.get(path.segments())
    }
}

/// Options accumulated for one command while its contract is resolved.
struct CommandOptions {
    /// Options accepted so far.
    options: BTreeMap<String, ResolvedOption>,
    /// The command's path, for diagnostics.
    path: RenderedPath,
}

impl CommandOptions {
    /// Starts an empty option set for one command path.
    const fn new(path: RenderedPath) -> Self {
        Self {
            options: BTreeMap::new(),
            path,
        }
    }

    /// Adds one option, allowing a repeated identical declaration only.
    fn insert(
        &mut self,
        long: OptionName<'_>,
        resolved: ResolvedOption,
    ) -> Result<(), CommandSurfaceError> {
        if long.as_str().is_empty() {
            return Err(CommandSurfaceError::EmptyOption {
                path: self.path.as_str().to_owned(),
            });
        }
        match self.options.get(long.as_str()) {
            None => {
                self.options.insert(long.as_str().to_owned(), resolved);
                Ok(())
            }
            // A repeated declaration that disagrees on *either* field is
            // ambiguous: the surface would make policy advice depend on list
            // order.
            Some(existing) if *existing == resolved => Ok(()),
            Some(_) => Err(CommandSurfaceError::ConflictingOption {
                path: self.path.as_str().to_owned(),
                option: long.as_str().to_owned(),
            }),
        }
    }

    /// Consumes the accumulator, yielding the accepted options.
    fn into_options(self) -> BTreeMap<String, ResolvedOption> {
        self.options
    }
}

/// Resolves one command contract.
fn resolve_command(contract: &CommandContract) -> Result<ResolvedCommand, CommandSurfaceError> {
    let path = join_path(CommandPath::new(&contract.path));
    let mut options = CommandOptions::new(path);
    for option in &contract.options {
        let resolved = ResolvedOption {
            value_type: option.value_type.clone(),
            is_global: option.is_global,
        };
        options.insert(OptionName::new(&option.long), resolved)?;
    }
    Ok(ResolvedCommand {
        options: options.into_options(),
        canonical_verb: contract.canonical_verb.clone(),
    })
}

/// Renders a path for diagnostics using single spaces.
pub(crate) fn join_path(path: CommandPath<'_>) -> RenderedPath {
    RenderedPath {
        rendered: path.segments().join(" "),
    }
}
