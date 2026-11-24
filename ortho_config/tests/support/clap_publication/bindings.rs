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
use super::imports::CrateScope;
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
    for item in items {
        match item {
            Item::Use(declaration) => {
                if binds_unproven_inventory(&declaration.tree, scope) {
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

/// Whether one `use` tree can bind an inventory the walk never counted.
///
/// A rename is the reported redirection: `… as ErrorKind` gives the name to an
/// item declared under another. An unrenamed path is accepted only when its
/// root is this crate — the reviewed `pub use kind::ErrorKind;` shape — because
/// a path rooted at another crate can name that crate's own inventory while a
/// file-level decoy in this one supplies the count.
fn binds_unproven_inventory(tree: &syn::UseTree, scope: &CrateScope) -> bool {
    tree_renames_error_kind(tree) || tree_reaches_foreign_error_kind(tree, true, scope)
}

/// Whether an unrenamed path under `tree` reaches a foreign `ErrorKind`.
///
/// The walk carries whether every segment so far is still a local path: the
/// first segment decides it, by naming one of this crate's modules or something
/// else.
fn tree_reaches_foreign_error_kind(
    tree: &syn::UseTree,
    is_local_so_far: bool,
    scope: &CrateScope,
) -> bool {
    match tree {
        syn::UseTree::Path(path) => {
            let is_local = is_local_segment(&identifier_name(&path.ident), is_local_so_far, scope);
            if !is_local && tree_names_error_kind(&path.tree) {
                return true;
            }
            tree_reaches_foreign_error_kind(&path.tree, is_local, scope)
        }
        syn::UseTree::Group(group) => group
            .items
            .iter()
            .any(|item| tree_reaches_foreign_error_kind(item, is_local_so_far, scope)),
        syn::UseTree::Name(_) | syn::UseTree::Rename(_) | syn::UseTree::Glob(_) => false,
    }
}

/// Whether one path segment keeps the import inside this crate.
///
/// `crate`, `self`, `super` and `Self` always do. A segment that names one of
/// this crate's modules does too, and any other segment leaves the crate.
fn is_local_segment(segment: &str, is_local_so_far: bool, scope: &CrateScope) -> bool {
    if matches!(segment, "crate" | "self" | "super" | "Self") {
        return true;
    }
    is_local_so_far && scope.is_local_module(segment)
}

/// Whether one `use` tree's leaf is the unrenamed name `ErrorKind`.
fn tree_names_error_kind(tree: &syn::UseTree) -> bool {
    any_leaf(tree, |_, _, to| to == "ErrorKind")
}

/// Whether one `use` tree renames a differently named item to `ErrorKind`.
///
/// A rename from `ErrorKind` to `ErrorKind` changes nothing and is therefore
/// not reported here; only the rename that gives the inventory name to an item
/// declared under another one is a hazard.
fn tree_renames_error_kind(tree: &syn::UseTree) -> bool {
    any_leaf(tree, |is_rename, from, to| {
        is_rename && to == "ErrorKind" && from != "ErrorKind"
    })
}

/// Whether any leaf of one `use` tree satisfies `predicate`.
///
/// The predicate receives the leaf's normalized names: whether it is a rename,
/// the name it binds from, and the name it binds to. A plain name passes itself
/// as both, so a caller that only cares what the leaf ends up called needs one
/// comparison. Paths and groups are descended, so a decision never depends on
/// where in the tree the interesting leaf sits.
fn any_leaf(tree: &syn::UseTree, predicate: impl Fn(bool, &str, &str) -> bool + Copy) -> bool {
    match tree {
        syn::UseTree::Path(path) => any_leaf(&path.tree, predicate),
        syn::UseTree::Group(group) => group.items.iter().any(|item| any_leaf(item, predicate)),
        syn::UseTree::Name(name) => {
            let ident = identifier_name(&name.ident);
            predicate(false, &ident, &ident)
        }
        syn::UseTree::Rename(rename) => {
            let from = identifier_name(&rename.ident);
            let to = identifier_name(&rename.rename);
            predicate(true, &from, &to)
        }
        syn::UseTree::Glob(_) => false,
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
