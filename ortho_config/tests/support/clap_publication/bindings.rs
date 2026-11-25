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
use super::tokens::module_path;
use super::use_tree::binds_unproven_inventory;

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

/// Name the place a refusal was found for the diagnostic.
fn described_place(prefix: &str) -> String {
    if prefix.is_empty() {
        "the file level".to_owned()
    } else {
        format!("`mod {prefix}`")
    }
}
