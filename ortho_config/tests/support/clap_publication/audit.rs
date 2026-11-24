//! Attribute auditing for the `ErrorKind` source inventory.
//!
//! The inventory is trustworthy only when the declaration it counts is the
//! declaration the compiler builds. Five questions decide that: whether an
//! item's compilation is conditional, whether a `mod` declaration redirects the
//! compiler to a file a default-name walk would not open, whether the inventory
//! type is declared somewhere a file-level scan cannot bind to the item the
//! crate exports, whether an item-level macro could declare it after this
//! parse, and whether an attribute path is a procedural macro whose expansion
//! replaces the item it annotates. The first three are answered here;
//! [`super::macros`] answers the fourth and [`super::expansion`] the fifth.
//!
//! The predicates here stay together because each of them fails closed, and a
//! control that reverts one must fail for the reason it names, so they are
//! audited as one subject rather than scattered through the module walk.

use anyhow::{Result, bail, ensure};
use camino::Utf8Path;
use syn::punctuated::Punctuated;
use syn::{Attribute, Item, ItemEnum, Meta, MetaList, Path, Token};

use super::imports::CrateScope;
use super::tokens::identifier_name;

/// Refuse an enum whose own or whose variants' compilation is conditional.
pub(super) fn audit_definition(definition: &ItemEnum, origin: &Utf8Path) -> Result<()> {
    reject_cfg_attributes(&definition.attrs, "the ErrorKind enum", origin)?;
    for variant in &definition.variants {
        reject_cfg_attributes(&variant.attrs, &variant.ident.to_string(), origin)?;
    }
    Ok(())
}

/// Refuse `cfg`/`cfg_attr` on an inventory-relevant item.
///
/// A conditional variant would make the source count depend on features the
/// gate does not model, so the gate fails closed rather than reporting a count
/// that may not describe the resolved build.
pub(super) fn reject_cfg_attributes(
    attributes: &[Attribute],
    subject: &str,
    origin: &Utf8Path,
) -> Result<()> {
    ensure!(
        !attributes.iter().any(is_cfg_attribute),
        "{origin} conditionally compiles {subject}; the gate counts unconditional variants only",
    );
    Ok(())
}

/// Whether an attribute is `cfg` or `cfg_attr`.
fn is_cfg_attribute(attribute: &Attribute) -> bool {
    path_names(attribute.path(), "cfg") || path_names(attribute.path(), "cfg_attr")
}

/// Whether `path` spells exactly `name`, with a raw identifier normalized.
///
/// `syn::Path::is_ident` compares the rendered spelling, and `proc-macro2`
/// keeps the `r#` prefix, so `#[r#cfg]` would not match `"cfg"`. Rust applies
/// the two as one attribute, and a check that missed the raw spelling would read
/// a conditional item as unconditional and trust a shape the compiler may
/// exclude — so every attribute comparison goes through here.
pub(super) fn path_names(path: &Path, name: &str) -> bool {
    sole_identifier(path).is_some_and(|spelled| spelled == name)
}

/// The single identifier `path` spells, with a raw identifier normalized.
///
/// Only a one-segment path can name an attribute the compiler defines; a
/// multi-segment path names a procedural macro or a tool attribute.
/// [`super::expansion`] therefore asks this question to decide whether a path is
/// even a candidate for the compiler's own names, and relies on the same
/// normalization `#[r#cfg]` needs.
pub(super) fn sole_identifier(path: &Path) -> Option<String> {
    path.get_ident().map(identifier_name)
}

/// Whether any attribute applies `name`, directly or through `cfg_attr`.
///
/// The search recurses because Rust expands a `cfg_attr` into the attributes
/// it names, and a nested `cfg_attr` can apply the same attribute again. A
/// `cfg_attr` condition the gate cannot evaluate counts as a match, because an
/// unreadable attribute must not be the reason a module or a binding is
/// trusted. Each caller decides what the match costs it: `cfg` and `path` make
/// a module refuse, while `macro_use` makes an inert name ambiguous.
pub(super) fn applies_attribute(attributes: &[Attribute], name: &str) -> bool {
    attributes
        .iter()
        .any(|attribute| meta_applies(&attribute.meta, name))
}

/// Whether any attribute can remove its item from the build.
///
/// `#[cfg(..)]` always can. `#[cfg_attr(cond, ..)]` cannot by itself — it only
/// applies attributes — unless one of the attributes it applies is a `cfg`,
/// directly or through a further `cfg_attr`, which Rust expands recursively.
/// The distinction matters here: `clap_builder` conditions its crate root with
/// `#![cfg_attr(docsrs, feature(doc_cfg))]`, which leaves the file in every
/// build, so refusing every `cfg_attr` would fail the real package.
pub(super) fn has_presence_conditional_attribute(attributes: &[Attribute]) -> bool {
    applies_attribute(attributes, "cfg")
}

/// Whether a `mod` declaration names the file to load instead of the default.
///
/// This walk resolves default file names only. A `#[path]` — or a `cfg_attr`
/// that applies one — can make the compiler load a different file, so the
/// declaration is refused rather than followed as if the default held.
pub(super) fn declares_module_path(attributes: &[Attribute]) -> bool {
    applies_attribute(attributes, "path")
}

/// Whether `meta` names `name`, or a `cfg_attr` applies something that does.
///
/// A `cfg_attr` is expanded by the compiler, so the search has to recurse
/// through it: `#[cfg_attr(all(), cfg_attr(all(), cfg(any())))]` can still
/// remove its item even though no immediate attribute names `cfg`. Anything
/// this cannot parse counts as a match, because an unreadable attribute must
/// not be the reason a count is trusted.
fn meta_applies(meta: &Meta, name: &str) -> bool {
    match meta {
        Meta::Path(path) => path_names(path, name),
        Meta::NameValue(entry) => path_names(&entry.path, name),
        Meta::List(list) if path_names(&list.path, "cfg_attr") => {
            applies_through_cfg_attr(list, name)
        }
        Meta::List(list) => path_names(&list.path, name),
    }
}

/// Whether a `cfg_attr` applies an attribute matching `name` at any depth.
fn applies_through_cfg_attr(list: &MetaList, name: &str) -> bool {
    let Ok(arguments) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
    else {
        return true;
    };
    arguments
        .iter()
        .skip(1)
        .any(|applied| meta_applies(applied, name))
}

/// Whether `item` declares the inventory type.
///
/// `pub enum r#ErrorKind` declares `ErrorKind`, so the raw spelling is the same
/// declaration and has to be refused as a nested one exactly the same way.
pub(super) fn is_error_kind_enum(item: &Item) -> bool {
    matches!(item, Item::Enum(definition) if is_error_kind_name(&definition.ident))
}

/// Whether an identifier names the inventory type, raw spelling normalized.
pub(super) fn is_error_kind_name(ident: &syn::Ident) -> bool {
    identifier_name(ident) == "ErrorKind"
}

/// Refuse an `ErrorKind` declared inside an inline module.
///
/// The walk counts one file-level declaration and proves the compiler compiles
/// its file. That proof does not extend to a declaration nested in an inline
/// module: a crate could compile a file-level seventeen-variant decoy while its
/// public `error` module declared the eighteen-variant inventory that is
/// actually exported, and a file-level count would report the decoy and pass.
/// The gate therefore refuses the nested shape instead of choosing between the
/// two declarations it can see.
///
/// A condition on the module does not exempt it. `#[cfg(..)]` is evaluated by
/// the compiler, and a condition can hold — `#[cfg(all())]` always does — so a
/// conditional module may be compiled and its inventory may be the one the
/// crate exports. The gate cannot evaluate the condition, and refusing only the
/// unconditional shape would let a decoy pass behind a condition that is true.
pub(super) fn refuse_nested_error_kinds(items: &[Item], origin: &Utf8Path) -> Result<()> {
    match nested_error_kind(items, "") {
        Some(path) => bail!(
            "{origin} declares `ErrorKind` inside `mod {path}`, not at file level; the gate \
             cannot bind a nested declaration to the inventory the crate exports, so it fails \
             closed"
        ),
        None => Ok(()),
    }
}

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
    match tree {
        syn::UseTree::Path(path) => tree_names_error_kind(&path.tree),
        syn::UseTree::Name(name) => identifier_name(&name.ident) == "ErrorKind",
        syn::UseTree::Group(group) => group.items.iter().any(tree_names_error_kind),
        syn::UseTree::Rename(_) | syn::UseTree::Glob(_) => false,
    }
}

/// Whether one `use` tree renames a differently named item to `ErrorKind`.
///
/// A rename from `ErrorKind` to `ErrorKind` changes nothing, so it stays
/// accepted; the hazard is the rename that gives the inventory name to an item
/// declared under another one.
fn tree_renames_error_kind(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Path(path) => tree_renames_error_kind(&path.tree),
        syn::UseTree::Rename(rename) => {
            identifier_name(&rename.rename) == "ErrorKind"
                && identifier_name(&rename.ident) != "ErrorKind"
        }
        syn::UseTree::Group(group) => group.items.iter().any(tree_renames_error_kind),
        syn::UseTree::Name(_) | syn::UseTree::Glob(_) => false,
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

/// The module path of the first nested `ErrorKind`, if any.
///
/// Every inline module is searched, conditional or not: the condition decides
/// whether the compiler sees the declaration, and the gate cannot decide it.
fn nested_error_kind(items: &[Item], prefix: &str) -> Option<String> {
    for item in items {
        let Item::Mod(module) = item else {
            continue;
        };
        let Some((_, nested)) = &module.content else {
            continue;
        };
        let path = module_path(prefix, &module.ident.to_string());
        if nested.iter().any(is_error_kind_enum) {
            return Some(path);
        }
        if let Some(deeper) = nested_error_kind(nested, &path) {
            return Some(deeper);
        }
    }
    None
}

/// Join one inline module name onto the path of its enclosing modules.
fn module_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}::{name}")
    }
}
