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
use super::tokens::identifier_name;

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
        Item::Macro(invocation) if macro_may_produce_inventory(invocation, items) => {
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
fn macro_may_produce_inventory(invocation: &syn::ItemMacro, items: &[Item]) -> bool {
    // A `macro_rules!` definition declares nothing at item level; only a call
    // to it can.
    if invocation.mac.path.is_ident("macro_rules") {
        return false;
    }
    // `include!` splices a file this walk never opens, so it is unproven
    // whatever its argument. Any other item macro is unproven unless a
    // `macro_rules!` of the same name is declared here and its whole
    // transcription — including a wrapper that re-emits another macro — spells
    // no `ErrorKind` and calls nothing this walk cannot read.
    if invocation.mac.path.is_ident("include")
        || invocation.mac.tokens.to_string().contains("ErrorKind")
    {
        return true;
    }
    local_transcription(items, &invocation.mac.path)
        .is_none_or(|tokens| transcription_is_unproven(&tokens))
}

/// The tokens of a local `macro_rules!` definition with this name, if any.
fn local_transcription(items: &[Item], path: &syn::Path) -> Option<String> {
    let name = path.get_ident()?;
    items.iter().find_map(|item| {
        let Item::Macro(definition) = item else {
            return None;
        };
        if !definition.mac.path.is_ident("macro_rules") {
            return None;
        }
        let tokens = definition.mac.tokens.to_string();
        tokens.starts_with(&name.to_string()).then_some(tokens)
    })
}

/// Whether a local transcription's tokens can reach an unproven shape.
fn transcription_is_unproven(tokens: &str) -> bool {
    tokens.contains("ErrorKind") || tokens.contains("include") || tokens.contains("!(")
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
    match tree {
        syn::UseTree::Glob(_) | syn::UseTree::Group(_) => true,
        syn::UseTree::Rename(_) | syn::UseTree::Name(_) => false,
        syn::UseTree::Path(path) => {
            let root = identifier_name(&path.ident);
            // `super`, `self` and `crate` reach only modules this walk reads.
            if matches!(root.as_str(), "crate" | "self" | "super") {
                return false;
            }
            // A sibling-module glob carries only names read from that module's
            // own source, which is the reviewed `anstyle` shape.
            let is_sibling = matches!(&*path.tree, syn::UseTree::Glob(_));
            !(is_sibling && declares_module(items, &root))
        }
    }
}

/// Whether one item list declares a module with this name.
fn declares_module(items: &[Item], name: &str) -> bool {
    items
        .iter()
        .any(|item| matches!(item, Item::Mod(module) if identifier_name(&module.ident) == name))
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
