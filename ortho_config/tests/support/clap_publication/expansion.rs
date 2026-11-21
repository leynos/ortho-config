//! Procedural-expansion auditing for attribute paths.
//!
//! `syn` parses an attribute as a path and a token stream, and it does not say
//! whether the compiler reads that path as one of its own annotations or invokes
//! a procedural macro with the annotated item. The difference decides whether
//! the parse describes the build: `#[inventory_macros::inject] pub enum
//! ErrorKind { .. }` parses as the seventeen-variant decoy it spells, while the
//! macro returns the items that replace that enum, so the crate can export an
//! eighteen-variant inventory the walk never counted.
//!
//! The gate cannot expand a procedural macro, so it bounds every route it
//! cannot follow. The compiler's own attributes are single identifiers, so a
//! multi-segment path names either a procedural macro or a tool attribute, and
//! both are refused. A single identifier is trusted only when the compiler owns
//! that name: a macro bound to a built-in attribute's name makes the attribute
//! ambiguous (E0659), so a crate that compiles has already resolved every
//! one-segment attribute path to the compiler's meaning. `derive` is the one
//! compiler attribute whose arguments reach a procedural macro, so its list is
//! read and only the compiler's built-in derives are trusted. `cfg_attr`
//! applies its attributes when its condition holds, so the audit recurses with
//! it.
//!
//! The lists below are closed. A built-in this module omits refuses a crate
//! that uses it, which costs a human review; trusting a name the compiler does
//! not own is the bypass this audit exists to close, so the smaller list is the
//! safe one.

use anyhow::{Result, bail, ensure};
use camino::Utf8Path;
use syn::punctuated::Punctuated;
use syn::{Attribute, Item, Meta, MetaList, Path, Token};

use super::audit::{path_names, sole_identifier};
use super::tokens::quote_path;

/// Attributes the compiler defines, so their expansion declares nothing.
///
/// Every name here is resolved by the compiler itself; none can be a
/// procedural macro, because a macro of the same spelling makes the attribute
/// ambiguous instead of shadowing it. The list covers the names the reviewed
/// `clap_builder` source really uses, plus the compiler attributes a library
/// plausibly adopts next: a name missing from it refuses a crate for review
/// rather than trusting an expansion the walk cannot read.
const INERT_ATTRIBUTES: [&str; 37] = [
    // Conditional compilation.
    "cfg",
    "cfg_attr",
    // Lint levels and diagnostics.
    "allow",
    "warn",
    "deny",
    "forbid",
    "expect",
    "deprecated",
    "must_use",
    // Derivation and the helper its built-in derives read.
    "derive",
    "automatically_derived",
    "default",
    "non_exhaustive",
    // Testing.
    "test",
    "ignore",
    "should_panic",
    // Documentation and code generation.
    "doc",
    "inline",
    "cold",
    "track_caller",
    "target_feature",
    "repr",
    "coverage",
    "no_sanitize",
    "debugger_visualizer",
    // Modules, macros, and linkage.
    "path",
    "macro_export",
    "macro_use",
    "link",
    "link_name",
    "link_section",
    "no_mangle",
    "export_name",
    "used",
    // Preludes and unstable features.
    "no_std",
    "no_implicit_prelude",
    "feature",
];

/// The derives the compiler implements itself.
///
/// Every other derive names a procedural macro, whose output is appended to the
/// annotated item's scope and can therefore declare the inventory. The list is
/// the complete set the compiler provides.
const INERT_DERIVES: [&str; 9] = [
    "Clone",
    "Copy",
    "Debug",
    "Default",
    "Eq",
    "Hash",
    "Ord",
    "PartialEq",
    "PartialOrd",
];

/// Refuse an attribute whose expansion the walk cannot read.
///
/// The whole file is audited, not only the item the count reads: an attribute
/// macro can replace any annotated item, including a `mod` that would otherwise
/// put the inventory somewhere the file-level scan never looks, so no parsed
/// shape is trusted until every attribute path is bounded. Inline modules are
/// descended for the same reason.
pub(super) fn refuse_unverified_attributes(file: &syn::File, origin: &Utf8Path) -> Result<()> {
    audit_attributes(&file.attrs, origin)?;
    audit_items(&file.items, origin)
}

/// Audit one attribute, following the one route that applies more attributes.
fn audit_meta(meta: &Meta, origin: &Utf8Path) -> Result<()> {
    if let Meta::List(list) = meta
        && path_names(&list.path, "cfg_attr")
    {
        return audit_applied_attributes(list, origin);
    }
    if path_names(meta.path(), "derive") {
        return audit_derives(meta, origin);
    }
    ensure!(
        is_inert_attribute(meta.path()),
        "{origin} carries `#[{}]`, which can be an attribute macro; its expansion can declare \
         items this walk cannot read, so the inventory is unprovable and the gate fails closed",
        quote_path(meta.path()),
    );
    Ok(())
}

/// Audit the attributes a `cfg_attr` applies when its condition holds.
///
/// The first argument is the condition, which the gate does not evaluate; every
/// later argument is an attribute the compiler applies, so each is audited as
/// if it were written on the item directly. A `cfg_attr` the audit cannot parse
/// is refused rather than skipped, because an unreadable attribute must not be
/// the reason an unknown expansion is trusted.
fn audit_applied_attributes(list: &MetaList, origin: &Utf8Path) -> Result<()> {
    let Ok(arguments) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
    else {
        bail!(
            "{origin} carries a `#[cfg_attr]` this walk cannot parse; an attribute it applies can \
             be a macro whose expansion declares items this walk cannot read, so the inventory is \
             unprovable and the gate fails closed"
        );
    };
    arguments
        .iter()
        .skip(1)
        .try_for_each(|applied| audit_meta(applied, origin))
}

/// Audit the derives one attribute names.
///
/// A derive macro receives the annotated item and appends its output to the
/// item's scope, so an unknown derive is an item-producing expansion like an
/// attribute macro. The arguments are paths rather than generic meta, because
/// that is the grammar `derive` accepts.
fn audit_derives(meta: &Meta, origin: &Utf8Path) -> Result<()> {
    let Meta::List(list) = meta else {
        // A bare `#[derive]` names nothing, so it expands nothing.
        return Ok(());
    };
    let Ok(derives) = list.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated) else {
        bail!(
            "{origin} carries a `#[derive]` this walk cannot parse; a derive macro can declare \
             items this walk cannot read, so the inventory is unprovable and the gate fails closed"
        );
    };
    for derive in &derives {
        ensure!(
            is_inert_derive(derive),
            "{origin} derives `{}`, which is not one of the compiler's built-in derives; a derive \
             macro can declare items this walk cannot read, so the inventory is unprovable and \
             the gate fails closed",
            quote_path(derive),
        );
    }
    Ok(())
}

/// Audit every item in one list, descending into inline modules.
fn audit_items(items: &[Item], origin: &Utf8Path) -> Result<()> {
    for item in items {
        audit_attributes(item_attributes(item), origin)?;
        if let Item::Mod(module) = item
            && let Some((_, nested)) = &module.content
        {
            audit_items(nested, origin)?;
        }
    }
    Ok(())
}

/// Audit every attribute in one list.
fn audit_attributes(attributes: &[Attribute], origin: &Utf8Path) -> Result<()> {
    attributes
        .iter()
        .try_for_each(|attribute| audit_meta(&attribute.meta, origin))
}

/// The outer attributes of one item.
///
/// Variants and fields are not audited: a non-built-in attribute there needs a
/// non-built-in derive on the item to consume it, and every such derive is
/// refused above. The wildcard covers the item syntax `syn` keeps as verbatim
/// tokens, such as `macro` definitions and `default impl` blocks. Both are
/// unstable, so a stable crate cannot contain one: the file would not compile,
/// and an attribute on an uncompilable item cannot change the inventory this
/// gate certifies.
fn item_attributes(declaration: &Item) -> &[Attribute] {
    match declaration {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

/// Whether `path` names one of the compiler's own attributes.
fn is_inert_attribute(path: &Path) -> bool {
    sole_identifier(path).is_some_and(|name| INERT_ATTRIBUTES.contains(&name.as_str()))
}

/// Whether `path` names one of the compiler's own derives.
fn is_inert_derive(path: &Path) -> bool {
    sole_identifier(path).is_some_and(|name| INERT_DERIVES.contains(&name.as_str()))
}
