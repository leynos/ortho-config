//! Refusals for an export of the inventory name the walk cannot trace.
//!
//! The count is bound to a declaration the compiler builds, and the crate then
//! chooses what its exported `ErrorKind` means. When that choice is the
//! declaration itself, the count describes the export. Every other way of
//! binding the name breaks that link without changing the parse, so each is
//! refused here rather than guessed at:
//!
//! - a renamed re-export gives the name to an item declared under another one;
//! - an unrenamed path rooted outside this crate reaches another crate's own
//!   inventory;
//! - a type alias, struct, union, trait or module declared under the name
//!   replaces the type the name resolves to; and
//! - any of these behind a condition the gate cannot evaluate.
//!
//! The plain re-export the resolved source uses, such as `pub use
//! kind::ErrorKind;`, names the declaration itself and stays accepted.

use anyhow::{Result, bail};
use camino::Utf8Path;
use syn::Item;

use super::audit::is_error_kind_name;
use super::imports::{CrateScope, Imports};
use super::tokens::{identifier_name, module_path};

/// Refuse a `use` item that binds the inventory name by renaming another item.
///
/// The count is bound to a declaration whose file the compiler compiles, not to
/// the path a consumer resolves. Those coincide only when every binding of the
/// name `ErrorKind` names an item that is itself called `ErrorKind`. A renamed
/// re-export breaks that: `pub use crate::real::ActualErrorKind as ErrorKind;`
/// makes the exported inventory an enum this walk never counts, so a file-level
/// `ErrorKind` decoy beside it would pass with the wrong total. A plain
/// re-export such as the reviewed `pub use kind::ErrorKind;` keeps the
/// guarantee and stays accepted.
///
/// The check descends inline modules, conditional or not, because the re-export
/// is normally written in a public module rather than at the crate root, and
/// the gate cannot evaluate a condition that would decide whether the compiler
/// sees it.
pub(super) fn refuse_error_kind_renames(
    items: &[Item],
    scope: &CrateScope,
    origin: &Utf8Path,
) -> Result<()> {
    if let Some(place) = renamed_error_kind(items, "", scope) {
        bail!(
            "{origin} binds the inventory name in {place} to something the gate cannot trace \
             to the declaration it counts; the export may reach an inventory this walk never \
             reads, so it fails closed"
        );
    }
    Ok(())
}

/// Refuse any declaration of the inventory name that is not the counted enum.
///
/// The count is certified for one enum, so every other type-namespace
/// declaration of `ErrorKind` is a way to make the exported name lead
/// somewhere the walk never counted: a type alias
/// (`pub type ErrorKind = crate::real::ActualErrorKind;`), a struct, a union, a
/// trait, or a module. Resolving each of those to whatever it ultimately names
/// would repeat the same binding question at every hop, so the gate refuses
/// them instead. The declaration is refused wherever it appears, conditionally
/// or not, because the gate cannot evaluate a condition that decides whether
/// the compiler sees it.
pub(super) fn refuse_error_kind_redeclarations(items: &[Item], origin: &Utf8Path) -> Result<()> {
    if let Some(place) = redeclared_error_kind(items, "") {
        bail!(
            "{origin} declares the inventory name in {place} as something other than the counted \
             enum; the gate cannot bind that export to the declaration it counts, so it fails \
             closed"
        );
    }
    Ok(())
}

/// Where the first non-enum `ErrorKind` declaration appears, if any.
fn redeclared_error_kind(items: &[Item], prefix: &str) -> Option<String> {
    for item in items {
        if non_enum_declaration_ident(item).is_some_and(is_error_kind_name) {
            return Some(described_place(prefix));
        }
        if let Some(deeper) = redeclared_in_module(item, prefix) {
            return Some(deeper);
        }
    }
    None
}

/// The identifier one item declares in the type namespace, if it does.
///
/// Only these item kinds can make the exported name resolve to something other
/// than the counted enum; a function or a constant of the same name occupies a
/// namespace a type path never reads.
const fn non_enum_declaration_ident(item: &Item) -> Option<&syn::Ident> {
    match item {
        Item::Type(alias) => Some(&alias.ident),
        Item::Struct(item_struct) => Some(&item_struct.ident),
        Item::Union(union) => Some(&union.ident),
        Item::Trait(trait_item) => Some(&trait_item.ident),
        Item::Mod(module) => Some(&module.ident),
        _ => None,
    }
}

/// Where a redeclaration inside one item's inline module appears, if any.
fn redeclared_in_module(item: &Item, prefix: &str) -> Option<String> {
    let Item::Mod(module) = item else {
        return None;
    };
    let (_, nested) = module.content.as_ref()?;
    redeclared_error_kind(nested, &module_path(prefix, &module.ident.to_string()))
}

/// Where the first `ErrorKind`-rebinding `use` appears, if any.
fn renamed_error_kind(items: &[Item], prefix: &str, scope: &CrateScope) -> Option<String> {
    let imports = Imports::of(items);
    for item in items {
        match item {
            Item::Use(declaration) => {
                // A leading `::` resolves through the extern prelude in edition
                // 2018 and later, so the path is external whatever the crate's
                // own module names are.
                let is_absolute = declaration.leading_colon.is_some();
                if binds_unproven_inventory(&declaration.tree, is_absolute, scope, &imports) {
                    return Some(described_place(prefix));
                }
            }
            Item::Mod(module) => {
                let Some((_, nested)) = &module.content else {
                    continue;
                };
                let path = module_path(prefix, &module.ident.to_string());
                if let Some(deeper) = renamed_error_kind(nested, &path, scope) {
                    return Some(deeper);
                }
            }
            _ => {}
        }
    }
    None
}

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
fn binds_unproven_inventory(
    tree: &syn::UseTree,
    is_absolute: bool,
    scope: &CrateScope,
    imports: &Imports,
) -> bool {
    if is_reviewed_crate_path(tree, is_absolute) {
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
fn is_reviewed_crate_path(tree: &syn::UseTree, is_absolute: bool) -> bool {
    is_unreviewed_crate_path(tree, is_absolute) && is_reviewed_crate_binding(tree)
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
            leaf_binding(&ident, None, walk.is_local)
        }
        syn::UseTree::Rename(rename) => {
            let from = identifier_name(&rename.ident);
            let to = identifier_name(&rename.rename);
            leaf_binding(&from, Some(&to), walk.is_local)
        }
        // A glob reaches whatever its module exports, so an external glob may
        // bring in an `ErrorKind` this walk never reads. The resolved source does
        // carry one — `pub use anstyle::*;` — so the reviewed roots are an
        // allow-list, in the same spirit as the closed attribute lists: a new
        // external glob fails the gate until it has been reviewed.
        syn::UseTree::Glob(_) => classify_glob(walk.root, walk.is_local, scope),
    }
}

/// Whether one `crate::`-rooted tree is one of the reviewed bindings.
///
/// The tree is rendered as its written names, so the comparison is against the
/// exact path the source uses rather than against any resolution the walk
/// would have to reproduce.
fn is_reviewed_crate_binding(tree: &syn::UseTree) -> bool {
    // Only a tree whose leaves all reach a reviewed path is reviewed; a group
    // carrying one reviewed and one unreviewed path is not.
    let paths = written_paths(tree);
    !paths.is_empty()
        && paths.iter().all(|path| {
            let names: Vec<&str> = path.iter().map(String::as_str).collect();
            REVIEWED_CRATE_BINDINGS.contains(&names.as_slice())
        })
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
fn classify_glob(root: Option<&str>, is_local: bool, scope: &CrateScope) -> Binding {
    let Some(glob_root) = root.filter(|_| !is_local) else {
        return Binding::Unrelated;
    };
    let is_reviewed = REVIEWED_EXTERNAL_GLOBS.contains(&glob_root);
    if is_reviewed && !scope.is_imported_somewhere(glob_root) {
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

/// Name the place a refusal was found for the diagnostic.
fn described_place(prefix: &str) -> String {
    if prefix.is_empty() {
        "the file level".to_owned()
    } else {
        format!("`mod {prefix}`")
    }
}
