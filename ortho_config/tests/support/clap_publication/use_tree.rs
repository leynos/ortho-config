//! How a `use` tree binds the inventory name.
//!
//! The count is certified for one declaration, so the crate must bind the name
//! to that declaration and nowhere else. Approximating Rust's scope resolution
//! — per-module scopes, alias hops, globs, re-export chains — is a resolver,
//! and each attempt to approximate it left another shape for a later review to
//! find. Two closed allow-lists replace the approximation instead:
//!
//! - a `crate::`-rooted binding of the name must be one of
//!   [`REVIEWED_CRATE_BINDINGS`]; and
//! - an external glob must be rooted at one of [`REVIEWED_EXTERNAL_GLOBS`] and
//!   that root must not be rebound anywhere in the crate.
//!
//! Everything else that touches the name — a rename, an alias hop, an absolute
//! path, an unreviewed glob — fails closed without having to be enumerated.

use super::imports::{CrateScope, Imports};
use super::tokens::identifier_name;

/// What one `use` tree binds, in the terms the refusal needs to decide.
///
/// The root of the path decides whether an external inventory is even
/// reachable, so the root is resolved before the leaf is interpreted: a rename
/// from `ErrorKind` to `ErrorKind` only keeps the declaration when its path
/// stays in this crate, and an external glob can bring in an `ErrorKind` the
/// walk never reads even though its own leaf is not a name at all.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Binding {
    /// The tree does not touch the inventory name.
    Unrelated,
    /// The tree aliases another item to `ErrorKind` in this crate.
    LocalRename,
    /// The tree re-exports or renames a name from another crate.
    Foreign,
}

/// Whether one `use` tree can bind an inventory the walk never counted.
pub(super) fn binds_unproven_inventory(
    tree: &syn::UseTree,
    is_absolute: bool,
    scope: &CrateScope,
    imports: &Imports,
) -> bool {
    if is_reviewed_crate_path(tree, is_absolute, scope) {
        return false;
    }
    if is_unreviewed_crate_path(tree, is_absolute) {
        return true;
    }
    matches!(
        classify_binding(tree, !is_absolute, scope, imports),
        Binding::LocalRename | Binding::Foreign
    )
}

/// Whether one `use` tree is a `crate::` path that binds the inventory name.
///
/// Such a tree is judged as a whole against the reviewed list, because a
/// per-segment walk would have to reproduce module scope resolution and would
/// still miss an alias declared inside an intermediate module.
fn is_unreviewed_crate_path(tree: &syn::UseTree, is_absolute: bool) -> bool {
    !is_absolute && starts_with_crate(tree) && binds_inventory_name(tree)
}

/// Whether one `use` tree is a reviewed `crate::` binding of the name.
///
/// The reviewed spelling alone is not enough: `pub use std::io as error;` binds
/// the reviewed path's first segment to another crate, so `crate::error::
/// ErrorKind` then matches the spelling while exporting a different inventory.
/// The path is trusted only when the crate root declares `error` as a module.
fn is_reviewed_crate_path(tree: &syn::UseTree, is_absolute: bool, scope: &CrateScope) -> bool {
    is_unreviewed_crate_path(tree, is_absolute)
        && is_reviewed_crate_binding(tree)
        && scope.root_declares_module("error")
}

/// Whether one `use` tree binds the inventory name anywhere in it.
fn binds_inventory_name(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Name(name) => identifier_name(&name.ident) == "ErrorKind",
        syn::UseTree::Rename(rename) => {
            identifier_name(&rename.rename) == "ErrorKind"
                || identifier_name(&rename.ident) == "ErrorKind"
        }
        syn::UseTree::Path(path) => binds_inventory_name(&path.tree),
        syn::UseTree::Group(group) => group.items.iter().any(binds_inventory_name),
        syn::UseTree::Glob(_) => false,
    }
}

/// Whether one `use` tree's path begins with `crate`.
fn starts_with_crate(tree: &syn::UseTree) -> bool {
    matches!(tree, syn::UseTree::Path(path) if identifier_name(&path.ident) == "crate")
}

/// The `crate::`-rooted bindings of the inventory name the source ships.
///
/// This is a closed allow-list, because a per-segment name check cannot see
/// that `crate::actual::helper` resolves to an alias declared inside `actual`.
/// The resolved `clap_builder` 4.6.7 source binds the name this way from its
/// builder and error modules (`use crate::error::ErrorKind;`), so any other
/// crate-rooted binding fails the gate until a maintainer reviews it.
const REVIEWED_CRATE_BINDINGS: [&[&str]; 1] = [&["crate", "error", "ErrorKind"]];

/// The external glob roots the reviewed source ships and a reviewer accepted.
///
/// `clap_builder` 4.6.7 re-exports `anstyle` wholesale from its styling module,
/// and no other external glob appears in that source. A glob is a hazard
/// because it can supply an inventory this walk never reads, so the list is
/// closed: an upstream release that adds one fails the gate until a maintainer
/// reviews what the new crate exports.
const REVIEWED_EXTERNAL_GLOBS: [&str; 1] = ["anstyle"];

/// The state one `use` tree's walk carries from segment to segment.
#[derive(Clone, Copy)]
struct PathWalk<'a> {
    /// Whether every segment so far stays inside this crate.
    is_local: bool,
    /// The first segment seen, which decides how a glob is judged.
    root: Option<&'a str>,
    /// How many segments the path has, which bounds what a local path reaches.
    ///
    /// A one-segment local path such as `use super::*;` or `use kind::…` reaches
    /// only the enclosing module, which this walk has already audited. A longer
    /// local path descends into a module whose own `use` items this walk never
    /// reads, so an alias there can point at another crate and the path cannot
    /// be trusted by name alone.
    segments: usize,
}

/// Classify one `use` tree, starting as a local path with no root yet.
fn classify_binding(
    tree: &syn::UseTree,
    is_local: bool,
    scope: &CrateScope,
    imports: &Imports,
) -> Binding {
    classify_with_root(
        tree,
        PathWalk {
            is_local,
            root: None,
            segments: 0,
        },
        scope,
        imports,
    )
}

/// Classify one `use` tree, carrying whether its path is still local.
fn classify_with_root(
    tree: &syn::UseTree,
    walk: PathWalk<'_>,
    scope: &CrateScope,
    imports: &Imports,
) -> Binding {
    match tree {
        syn::UseTree::Path(path) => {
            let segment = identifier_name(&path.ident);
            // A `crate::`-rooted path may name a module declared in another
            // file, but every later segment has to resolve in the module before
            // it. Rather than model each scope, the reviewed crate paths are a
            // closed list: `crate::error::ErrorKind` is the source's only one, so
            // any other `crate::` path fails closed. A per-segment name check
            // cannot see that `crate::actual::helper` resolves to an alias
            // declared inside `actual`.
            let is_local_here = is_local_segment(&segment, walk.is_local, imports)
                || (walk.root == Some("crate") && scope.root_declares_module(&segment));
            let next = PathWalk {
                is_local: is_local_here,
                root: walk.root.or(Some(segment.as_str())),
                segments: walk.segments + 1,
            };
            classify_with_root(&path.tree, next, scope, imports)
        }
        syn::UseTree::Group(group) => group
            .items
            .iter()
            .map(|item| classify_with_root(item, walk, scope, imports))
            .fold(Binding::Unrelated, stronger_binding),
        syn::UseTree::Name(name) => {
            let ident = identifier_name(&name.ident);
            deep_local_leaf(&walk, &ident)
                .unwrap_or_else(|| leaf_binding(&ident, None, walk.is_local))
        }
        syn::UseTree::Rename(rename) => {
            let from = identifier_name(&rename.ident);
            let to = identifier_name(&rename.rename);
            deep_local_leaf(&walk, &to)
                .unwrap_or_else(|| leaf_binding(&from, Some(&to), walk.is_local))
        }
        // The same depth rule covers a glob: a deeper local path descends into
        // a module whose own `use` items this walk never reads.
        syn::UseTree::Glob(_) => classify_glob(&walk, scope),
    }
}

/// The refusal for a local leaf reached through more than one segment.
///
/// `self::local::alias::ErrorKind` resolves `alias` in `local`'s scope, where it
/// may be a `use` alias for another crate, while this walk checks segments
/// against the enclosing item list. A one-segment local path reaches only the
/// enclosing module, which the walk audits; a deeper one cannot be trusted by
/// name, so its inventory leaf fails closed.
fn deep_local_leaf(walk: &PathWalk<'_>, leaf: &str) -> Option<Binding> {
    let is_deep_local = walk.is_local && walk.segments > 1;
    (is_deep_local && leaf == "ErrorKind").then_some(Binding::Foreign)
}

/// Whether one `crate::`-rooted tree is one of the reviewed bindings.
///
/// The tree is rendered as its written names, so the comparison is against the
/// exact path the source uses rather than against any resolution the walk
/// would have to reproduce.
fn is_reviewed_crate_binding(tree: &syn::UseTree) -> bool {
    // Every leaf has to be a reviewed path. `written_paths` reports only the
    // name leaves it can render, so a group carrying a reviewed path beside a
    // glob or a rename would otherwise pass on its reviewed leaf alone; the
    // leaf count catches that.
    let paths = written_paths(tree);
    !paths.is_empty()
        && leaf_count(tree) == paths.len()
        && paths.iter().all(|path| {
            let names: Vec<&str> = path.iter().map(String::as_str).collect();
            REVIEWED_CRATE_BINDINGS.contains(&names.as_slice())
        })
}

/// How many leaves one `use` tree has, counting globs and renames too.
fn leaf_count(tree: &syn::UseTree) -> usize {
    match tree {
        syn::UseTree::Path(path) => leaf_count(&path.tree),
        syn::UseTree::Group(group) => group.items.iter().map(leaf_count).sum(),
        syn::UseTree::Name(_) | syn::UseTree::Rename(_) | syn::UseTree::Glob(_) => 1,
    }
}

/// Every complete path one `use` tree writes, with names normalized.
///
/// A group contributes one path per item; a rename or glob writes no complete
/// path, because neither names a single resolvable item.
fn written_paths(tree: &syn::UseTree) -> Vec<Vec<String>> {
    fn walk(tree: &syn::UseTree, prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(identifier_name(&path.ident));
                walk(&path.tree, prefix, out);
                prefix.pop();
            }
            syn::UseTree::Name(name) => {
                prefix.push(identifier_name(&name.ident));
                out.push(prefix.clone());
                prefix.pop();
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    walk(item, prefix, out);
                }
            }
            syn::UseTree::Rename(_) | syn::UseTree::Glob(_) => {}
        }
    }

    let mut paths = Vec::new();
    walk(tree, &mut Vec::new(), &mut paths);
    paths
}

/// Classify one glob by whether its root is a reviewed external crate.
///
/// The reviewed names are accepted only when nothing in the crate rebinds them.
/// A `use std::io as anstyle;` makes `anstyle` an alias for a different crate,
/// whose `*` can then export an `ErrorKind` the walk never reads, so an
/// imported name that matches a reviewed root is refused rather than trusted.
fn classify_glob(walk: &PathWalk<'_>, scope: &CrateScope) -> Binding {
    if walk.is_local {
        // One segment reaches only the enclosing module, which this walk
        // audits; a deeper internal glob descends into a module whose own `use`
        // items it never reads, so an alias there could reach another crate.
        return if walk.segments <= 1 {
            Binding::Unrelated
        } else {
            Binding::Foreign
        };
    }
    let Some(glob_root) = walk.root else {
        return Binding::Unrelated;
    };
    // A reviewed root is trusted only when nothing in the crate reuses its
    // name: a `use` or `extern crate` alias binds it to another crate, and a
    // local `mod anstyle` can re-export another crate's items under it.
    let is_reviewed = REVIEWED_EXTERNAL_GLOBS.contains(&glob_root)
        && !scope.is_imported_somewhere(glob_root)
        && !scope.declares_module_anywhere(glob_root);
    if is_reviewed {
        Binding::Unrelated
    } else {
        Binding::Foreign
    }
}

/// Whether one path segment keeps the import inside this crate.
///
/// `crate`, `self`, `super` and `Self` always do. A segment that names one of
/// this crate's modules does too, and any other segment leaves the crate.
fn is_local_segment(segment: &str, is_local_so_far: bool, imports: &Imports) -> bool {
    if matches!(segment, "crate" | "self" | "super" | "Self") {
        return true;
    }
    // Only the enclosing scope decides a bare first segment, because that is the
    // scope Rust resolves it in. A module of the same name elsewhere in the
    // crate is out of scope, and a leading `::` reaches the extern prelude
    // instead of any local module at all.
    is_local_so_far && imports.declares_module(segment)
}

/// Classify one leaf bound by a `use` tree.
///
/// `to` is the renamed binding's new name, or `None` for a plain import. A
/// plain name or a same-name rename keeps the declaration only when the path
/// stayed local; any other binding of the inventory name either aliases a local
/// item or reaches a foreign one.
fn leaf_binding(from: &str, to: Option<&str>, is_local: bool) -> Binding {
    let bound = to.unwrap_or(from);
    if bound != "ErrorKind" {
        return Binding::Unrelated;
    }
    if !is_local {
        return Binding::Foreign;
    }
    if to.is_some() && from != "ErrorKind" {
        return Binding::LocalRename;
    }
    Binding::Unrelated
}

/// Keep the stronger of two bindings found in one group.
const fn stronger_binding(current: Binding, next: Binding) -> Binding {
    match (current, next) {
        (Binding::Foreign, _) | (_, Binding::Foreign) => Binding::Foreign,
        (Binding::LocalRename, _) | (_, Binding::LocalRename) => Binding::LocalRename,
        _ => Binding::Unrelated,
    }
}
