//! Prove that the compiler compiles a module file whenever it compiles the root.
//!
//! A declaration inside the source tree is not by itself evidence that the
//! compiler sees it: `#[cfg]` on a `mod` declaration, a condition applied
//! through a nested `cfg_attr`, or an inner `#![cfg]` at the top of a module
//! file can make the declaring file unreachable until some condition holds.
//! The gate therefore walks `mod` declarations from the crate root and
//! certifies a file only when every hop to it is unconditional.
//!
//! A condition is never evaluated, because the gate cannot evaluate one and
//! `#[cfg(all())]` always holds. Every file below a conditional hop is instead
//! *inspected*: it may not supply the count, but the declarations it carries
//! are still audited, because a condition that holds would compile them. That
//! distinction is why the walk has two certifications rather than one: skipping
//! a conditional subtree outright would hide a `mod` declaration whose file the
//! walk never reads.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use syn::{Item, ItemMod};

use super::audit::{declares_module_path, has_presence_conditional_attribute};

/// How deep a module tree may nest before the gate refuses to certify it.
const MAX_MODULE_DEPTH: usize = 32;

/// Every file the compiler compiles whenever it compiles `crate_root`.
///
/// Only unconditional module declarations are followed, and a file whose own
/// inner attributes can remove it from the build certifies nothing. A
/// declaration reachable only through a conditional hop is therefore not
/// certified, which is the fail-closed direction.
pub(super) fn unconditionally_compiled(
    directory: &Dir,
    sources: &[Utf8PathBuf],
    crate_root: &Utf8Path,
) -> Result<BTreeSet<Utf8PathBuf>> {
    ensure!(
        sources.contains(&crate_root.to_path_buf()),
        "expected a crate root {crate_root} to locate ErrorKind from",
    );
    let mut walker = Reachability {
        directory,
        sources,
        crate_root,
        certified: BTreeSet::new(),
        inspected: BTreeSet::new(),
    };
    walker.visit_file(crate_root, 0, Certification::Unconditional)?;
    Ok(walker.certified)
}

/// Whether a visited file may supply the count or is only being audited.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Certification {
    /// Every hop from the crate root to this file is unconditional.
    Unconditional,
    /// Some hop may be conditional, so this file cannot supply the count.
    Tentative,
}

/// Where the walk stands: the directory declarations resolve in, how deep, and
/// what a file reached from here may be certified as.
#[derive(Clone, Copy)]
struct Level<'a> {
    /// The directory a `mod name;` declared at this level resolves within.
    prefix: &'a Utf8Path,
    /// The number of `mod` declarations between the crate root and here.
    depth: usize,
    /// What a file reached from here inherits from its enclosing hops.
    certification: Certification,
}

impl<'a> Level<'a> {
    /// The certification a declaration's own contents inherit.
    ///
    /// A condition on the declaration may keep it out of the build, so
    /// whatever it declares is only inspected, never certified.
    const fn children(self, is_conditional_declaration: bool) -> Certification {
        if is_conditional_declaration {
            Certification::Tentative
        } else {
            self.certification
        }
    }

    /// The level one `mod` declaration deeper, resolving below `prefix`.
    const fn descend(self, prefix: &'a Utf8Path, is_conditional_declaration: bool) -> Self {
        Self {
            prefix,
            depth: self.depth + 1,
            certification: self.children(is_conditional_declaration),
        }
    }
}

/// Walks module declarations from the crate root in both certifications.
struct Reachability<'a> {
    /// Handle on the source tree being walked.
    directory: &'a Dir,
    /// Every `.rs` file in the tree, so a hop is only followed when it resolves.
    sources: &'a [Utf8PathBuf],
    /// The root Cargo reports, which keeps its modules beside itself.
    crate_root: &'a Utf8Path,
    /// Files certified as compiled unconditionally.
    certified: BTreeSet<Utf8PathBuf>,
    /// Files visited only to audit their declarations.
    inspected: BTreeSet<Utf8PathBuf>,
}

impl Reachability<'_> {
    /// Audits `file` under `certification` and everything it declares.
    fn visit_file(
        &mut self,
        file: &Utf8Path,
        depth: usize,
        certification: Certification,
    ) -> Result<()> {
        ensure!(
            depth <= MAX_MODULE_DEPTH,
            "{file} nests modules more than {MAX_MODULE_DEPTH} deep; the gate will not certify it",
        );
        if !self.mark_visited(file, certification) {
            return Ok(());
        }
        let source = self
            .directory
            .read_to_string(file)
            .with_context(|| format!("read {file} while locating ErrorKind"))?;
        let parsed =
            syn::parse_file(&source).with_context(|| format!("{file} should be valid Rust"))?;
        let inherited = self.child_certification(file, &parsed.attrs, certification);
        let prefix = module_directory(file, self.crate_root);
        self.visit_items(
            &parsed.items,
            Level {
                prefix: &prefix,
                depth,
                certification: inherited,
            },
        )
    }

    /// Records the visit, reporting whether this certification is new.
    ///
    /// The two sets are tracked apart so a file reached tentatively and then
    /// unconditionally is walked again under the stronger certification; only
    /// that second walk can certify it.
    fn mark_visited(&mut self, file: &Utf8Path, certification: Certification) -> bool {
        match certification {
            Certification::Unconditional => self.certified.insert(file.to_path_buf()),
            Certification::Tentative => self.inspected.insert(file.to_path_buf()),
        }
    }

    /// How `file`'s own declarations are certified, given its inner attributes.
    ///
    /// An inner `#![cfg]` can remove the whole file from the build, so a file
    /// that carries one loses any certification it had and is only inspected
    /// from here on.
    fn child_certification(
        &mut self,
        file: &Utf8Path,
        attributes: &[syn::Attribute],
        certification: Certification,
    ) -> Certification {
        let is_conditional_file = has_presence_conditional_attribute(attributes);
        if certification == Certification::Unconditional && is_conditional_file {
            self.certified.remove(file);
        }
        if certification == Certification::Unconditional && !is_conditional_file {
            Certification::Unconditional
        } else {
            Certification::Tentative
        }
    }

    /// Follows the modules among `items`, declared at `level`.
    fn visit_items(&mut self, items: &[Item], level: Level<'_>) -> Result<()> {
        for item in items {
            if let Item::Mod(module) = item {
                self.visit_module(module, level)?;
            }
        }
        Ok(())
    }

    /// Follows one `mod` declaration according to its condition and shape.
    ///
    /// A conditional declaration is not skipped: its body, and the file it
    /// names, are still audited, because the condition may hold.
    fn visit_module(&mut self, module: &ItemMod, level: Level<'_>) -> Result<()> {
        if declares_module_path(&module.attrs) {
            bail!(
                "`mod {name}` below {prefix} carries a `#[path]` attribute; the compiler may \
                 load a file this walk does not follow, so the inventory is unprovable",
                name = module.ident,
                prefix = level.prefix,
            );
        }
        let is_conditional_declaration = has_presence_conditional_attribute(&module.attrs);
        match &module.content {
            Some((_, nested)) => {
                let inline = level.prefix.join(module.ident.to_string());
                self.visit_items(nested, level.descend(&inline, is_conditional_declaration))
            }
            None => self.visit_declared_files(module, level, is_conditional_declaration),
        }
    }

    /// Follows the files one `mod name;` declaration below `level` resolves to.
    fn visit_declared_files(
        &mut self,
        module: &ItemMod,
        level: Level<'_>,
        is_conditional_declaration: bool,
    ) -> Result<()> {
        let candidates = module_files(level.prefix, module, self.sources);
        ensure!(
            !candidates.is_empty(),
            "{reason}",
            reason = unreadable_declaration(module, level.prefix, is_conditional_declaration),
        );
        let certification = level.children(is_conditional_declaration);
        for candidate in candidates {
            self.visit_file(&candidate, level.depth + 1, certification)?;
        }
        Ok(())
    }
}

/// The explanation for a declaration whose compiled file this walk cannot read.
///
/// A condition is not evidence that the compiler skips the declaration:
/// `#[cfg(all())]` always holds, and the gate cannot evaluate any condition.
/// The two texts differ only in whether the condition is on this
/// declaration or inherited from an enclosing hop, because that is what
/// tells a maintainer which line to look at.
fn unreadable_declaration(
    module: &ItemMod,
    prefix: &Utf8Path,
    is_conditional_declaration: bool,
) -> String {
    let name = &module.ident;
    if is_conditional_declaration {
        format!(
            "`mod {name}` below {prefix} is conditional and resolves to no file this walk \
             reads; the condition may hold, so the compiler can load source the gate has not \
             inspected and the inventory is unprovable",
        )
    } else {
        format!(
            "`mod {name}` below {prefix} resolves to no file this walk reads; the compiler \
             may load a symlinked or otherwise unreachable module file, so the inventory is \
             unprovable and the gate fails closed",
        )
    }
}

/// The files a `mod name;` declared below `prefix` may resolve to, when present.
///
/// Rust resolves a file module relative to the module's own directory: beside
/// `lib.rs`, below `foo/` for both `foo.rs` and `foo/mod.rs`, and below the
/// inline module's name for a nested declaration. A `#[path]` overrides all of
/// that, which is why [`declares_module_path`] refuses it instead of guessing.
fn module_files(prefix: &Utf8Path, module: &ItemMod, sources: &[Utf8PathBuf]) -> Vec<Utf8PathBuf> {
    let name = module.ident.to_string();
    [
        prefix.join(format!("{name}.rs")),
        prefix.join(name).join("mod.rs"),
    ]
    .into_iter()
    .filter(|candidate| sources.contains(candidate))
    .collect()
}

/// The directory a `mod name;` declared in `file` resolves within.
///
/// The crate root Cargo reports and a `mod.rs` keep their modules beside
/// themselves; any other `foo.rs` keeps them below `foo/`. The root is
/// compared by path, not by file name: a package may compile `lib/entry.rs`,
/// and a nested `lib.rs` of its own is then an ordinary module file.
fn module_directory(file: &Utf8Path, crate_root: &Utf8Path) -> Utf8PathBuf {
    let parent = file.parent().unwrap_or_else(|| Utf8Path::new(""));
    let is_crate_root = file == crate_root || file.file_name() == Some("mod.rs");
    if is_crate_root {
        return parent.to_path_buf();
    }
    file.file_stem()
        .map_or_else(|| parent.to_path_buf(), |stem| parent.join(stem))
}
