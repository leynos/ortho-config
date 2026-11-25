//! Audit that a reviewed glob's crate cannot supply an inventory.
//!
//! The classifier trusts a glob rooted at a reviewed crate because that crate's
//! `*` cannot supply an inventory the walk never counts. Taking the spelling on
//! trust is what earlier review rounds kept finding holes in, so the property
//! is checked instead: the crate is resolved from the same graph and refused if
//! it declares the inventory name at any depth, including inside an inline
//! module or a re-export chain. Globs and macros are reported as unproven
//! unless their shape provably carries only names this walk already reads.

use anyhow::{Context, Result, bail};
use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use cargo_metadata::Package;
use syn::Item;

use super::audit::is_error_kind_name;
use super::library::library_crate_root;
use super::source::{parse_sources, rust_sources};

/// Whether a reviewed glob's crate declares the inventory name at all.
///
/// The gate trusts a glob rooted at a reviewed crate because that crate's `*`
/// cannot supply an inventory the walk never counts. Taking the spelling on
/// trust is what earlier rounds kept finding holes in, so the property is
/// checked: resolve the reviewed crate from the same graph and refuse the gate
/// if any of its names is `ErrorKind`, at any depth.
pub(crate) fn reviewed_glob_supplies_no_inventory(package: &Package) -> Result<()> {
    let crate_root = library_crate_root(package)?;
    let Some(source_root) = crate_root.parent() else {
        bail!("{crate_root} should sit in a source directory");
    };
    let directory = Dir::open_ambient_dir(source_root, ambient_authority())
        .with_context(|| format!("open {source_root} to audit {} exports", package.name))?;
    let sources = rust_sources(&directory, Utf8Path::new(""))?;
    let parsed = parse_sources(&directory, source_root, &sources)?;
    for (relative, file) in &parsed {
        if let Some(place) = mentions_error_kind(&file.items, "") {
            bail!(
                "the reviewed glob crate {} declares `ErrorKind` in {relative} {place}, so its \
                 glob can supply an inventory the count never certifies",
                package.name,
            );
        }
    }
    Ok(())
}

/// Where a file's declarations mention `ErrorKind`, if they do.
///
/// A re-exported inventory can sit in an inline module and reach the crate root
/// through a glob, so the walk descends inline modules and treats an unproven
/// glob or item-producing macro as a hazard rather than assuming it harmless.
fn mentions_error_kind(items: &[Item], prefix: &str) -> Option<String> {
    items
        .iter()
        .find_map(|item| unproven_item(item, items).map(|why| format!("{}{why}", in_place(prefix))))
        .or_else(|| {
            items.iter().find_map(|item| {
                let Item::Mod(module) = item else {
                    return None;
                };
                let (_, nested) = module.content.as_ref()?;
                let path = join_module(prefix, &module.ident.to_string());
                mentions_error_kind(nested, &path)
            })
        })
}

/// Why one item makes the file unproven, or `None` when it is accounted for.
fn unproven_item(item: &Item, items: &[Item]) -> Option<&'static str> {
    match item {
        Item::Enum(definition) => names_inventory(&definition.ident),
        Item::Struct(item_struct) => names_inventory(&item_struct.ident),
        Item::Union(union) => names_inventory(&union.ident),
        Item::Type(alias) => names_inventory(&alias.ident),
        Item::Trait(trait_item) => names_inventory(&trait_item.ident),
        Item::Use(declaration) if written_use_binds_error_kind(&declaration.tree) => Some(""),
        Item::Use(declaration) if unproven_glob(&declaration.tree, items) => {
            Some(" a glob re-export")
        }
        Item::Macro(invocation) if macro_may_produce_inventory(invocation) => {
            Some(" an item-producing macro")
        }
        _ => None,
    }
}

/// The diagnostic suffix for an item declared under the inventory name.
fn names_inventory(ident: &syn::Ident) -> Option<&'static str> {
    if is_error_kind_name(ident) {
        Some("")
    } else {
        None
    }
}

/// Whether an item macro's transcription could declare the inventory.
///
/// A `macro_rules!` body can only produce what its own tokens spell, so the
/// transcription is read for the name; `include!` splices a file the walk never
/// opens, so it is unproven whatever its argument.
fn macro_may_produce_inventory(invocation: &syn::ItemMacro) -> bool {
    invocation.mac.path.is_ident("include")
        || invocation.mac.tokens.to_string().contains("ErrorKind")
}

/// Join one inline module name onto the path of its enclosing modules.
fn join_module(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}::{name}")
    }
}

/// Name the place a declaration was found for the diagnostic.
fn in_place(prefix: &str) -> String {
    if prefix.is_empty() {
        "at the file level".to_owned()
    } else {
        format!("inside `mod {prefix}`")
    }
}

/// Whether one glob re-export can carry a name this walk never reads.
///
/// A glob of a sibling module declared in the same item list cannot: the walk
/// reads that module's own source next. Anything else — a deeper path, an
/// external crate, or a glob inside a group — is unproven.
fn unproven_glob(tree: &syn::UseTree, items: &[Item]) -> bool {
    let syn::UseTree::Path(path) = tree else {
        return matches!(tree, syn::UseTree::Glob(_));
    };
    let syn::UseTree::Glob(_) = &*path.tree else {
        return matches!(tree, syn::UseTree::Group(_) | syn::UseTree::Glob(_));
    };
    // `super`, `self` and `crate` reach only modules this walk already reads.
    if matches!(path.ident.to_string().as_str(), "crate" | "self" | "super") {
        return false;
    }
    let declared = items
        .iter()
        .any(|item| matches!(item, Item::Mod(module) if module.ident == path.ident));
    !declared
}

/// Whether one `use` tree names `ErrorKind` at any leaf.
fn written_use_binds_error_kind(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Name(name) => is_error_kind_name(&name.ident),
        syn::UseTree::Rename(rename) => {
            is_error_kind_name(&rename.ident) || is_error_kind_name(&rename.rename)
        }
        syn::UseTree::Path(path) => written_use_binds_error_kind(&path.tree),
        syn::UseTree::Group(group) => group.items.iter().any(written_use_binds_error_kind),
        syn::UseTree::Glob(_) => false,
    }
}
