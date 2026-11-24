//! Procedural-expansion auditing for attribute paths and derive names.
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
//! A derive name is resolved the other way round. An explicit `use` binding
//! shadows the prelude's built-in derive: `use inventory_macros::Inject as
//! Debug;` makes `#[derive(Debug)]` invoke the imported procedural macro, whose
//! output is appended to the annotated item's scope and can declare the
//! inventory this walk counts from the parse. Nothing reports the substitution,
//! so the name alone is not evidence: a built-in derive is trusted only where
//! the module's own declarations cannot rebind it.
//!
//! The lists below are closed. A built-in this module omits refuses a crate
//! that uses it, which costs a human review; trusting a name the compiler does
//! not own is the bypass this audit exists to close, so the smaller list is the
//! safe one.

use std::collections::BTreeSet;

use anyhow::{Result, bail, ensure};
use camino::Utf8Path;
use syn::punctuated::Punctuated;
use syn::{Attribute, Item, Meta, MetaList, Path, Token};

use super::audit::{path_names, sole_identifier};
use super::imports::Imports;
use super::tokens::{identifier_name, quote_path};

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

/// Roots whose macro namespace only the compiler defines.
///
/// A `use` path rooted at one of these cannot supply a procedural derive: the
/// compiler's built-in derives are the only derives the standard library
/// exports. A module that rebinds the root name itself voids the exemption,
/// because a local `mod std` shadows the extern crate for that module.
const COMPILER_DERIVE_ROOTS: [&str; 3] = ["std", "core", "alloc"];

/// Refuse an attribute whose expansion the walk cannot read.
///
/// The whole file is audited, not only the item the count reads: an attribute
/// macro can replace any annotated item, including a `mod` that would otherwise
/// put the inventory somewhere the file-level scan never looks, so no parsed
/// shape is trusted until every attribute path is bounded. Inline modules are
/// descended for the same reason.
pub(super) fn refuse_unverified_attributes(file: &syn::File, origin: &Utf8Path) -> Result<()> {
    let shadowed = shadowed_derives(&file.items);
    audit_attributes(&file.attrs, origin, &shadowed)?;
    audit_items(&file.items, origin, &shadowed)
}

/// The built-in derive names this module's own declarations may rebind.
///
/// An explicit `use` binding shadows the prelude's built-in derive of the same
/// name, so `#[derive(Debug)]` can invoke an imported procedural macro and
/// nothing in the parse says so. A glob cannot: the compiler reports the name
/// as ambiguous (E0659) instead of choosing the import, so a crate that
/// compiles is not redirected by one. A `macro_rules!` definition cannot
/// either: bang macros are a separate macro kind, and `#[derive(Debug)]` beside
/// a local `macro_rules! Debug` still invokes the built-in derive.
///
/// A binding rooted at `std`, `core` or `alloc` cannot name a procedural
/// derive, so it is trusted unless the module rebinds the root name itself.
/// Every other binding refuses its name in the module.
fn shadowed_derives(items: &[Item]) -> BTreeSet<String> {
    let imports = Imports::of(items);
    let locals = local_names(items);
    let mut shadowed = BTreeSet::new();
    for (name, roots) in imports.explicit_roots() {
        if is_builtin_derive(name) && !every_binding_is_compiler(roots, &locals) {
            shadowed.insert(name.clone());
        }
    }
    shadowed
}

/// Whether every binding of one name can only name a compiler-supplied derive.
///
/// A name is trusted only when all of its bindings are: one import that could
/// reach a procedural macro is enough to redirect `#[derive(Name)]`, even with
/// a trustworthy import of the same spelling beside it, because a type and a
/// derive macro occupy different namespaces and both imports then compile. An
/// empty list cannot arise from a recorded binding, and is treated as untrusted
/// so the decision fails closed if that ever changes.
fn every_binding_is_compiler(roots: &[Option<String>], locals: &BTreeSet<String>) -> bool {
    !roots.is_empty()
        && roots
            .iter()
            .all(|root| is_compiler_binding(root.as_deref(), locals))
}

/// Whether one explicit binding can only name a compiler-supplied derive.
///
/// The root decides: `std`, `core` and `alloc` export no procedural derive, so
/// a binding through one is the built-in item the name suggests. A module that
/// declares a `mod std`, or an `extern crate … as std`, gives that root a local
/// meaning instead, and then the path reaches whatever the local declaration
/// exports. A bare `use Name;` names no root at all, so it is untrusted.
fn is_compiler_binding(root: Option<&str>, locals: &BTreeSet<String>) -> bool {
    root.is_some_and(|path_root| {
        COMPILER_DERIVE_ROOTS.contains(&path_root) && !locals.contains(path_root)
    })
}

/// The names one module binds directly, which can redirect a compiler root.
///
/// A `use` binding, a module declared in this module, or an `extern crate`
/// rename can give `std`, `core` or `alloc` a local meaning, so the
/// compiler-root exemption is void in a module that binds one of the names.
/// Other items cannot redirect a path: a struct, function or type of the same
/// name occupies a namespace a path's first segment cannot resolve through.
fn local_names(items: &[Item]) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = Imports::of(items).explicit_names().cloned().collect();
    for item in items {
        match item {
            Item::Mod(module) => {
                names.insert(identifier_name(&module.ident));
            }
            Item::ExternCrate(krate) => {
                names.insert(identifier_name(&krate.ident));
                if let Some((_, rename)) = &krate.rename {
                    names.insert(identifier_name(rename));
                }
            }
            _ => {}
        }
    }
    names
}

/// Audit one attribute, following the one route that applies more attributes.
fn audit_meta(meta: &Meta, origin: &Utf8Path, shadowed: &BTreeSet<String>) -> Result<()> {
    if let Meta::List(list) = meta
        && path_names(&list.path, "cfg_attr")
    {
        return audit_applied_attributes(list, origin, shadowed);
    }
    if path_names(meta.path(), "derive") {
        return audit_derives(meta, origin, shadowed);
    }
    ensure!(
        is_inert_attribute(meta.path()),
        "{} carries `#[{}]`, which can be an attribute macro; its expansion can declare items this \
         walk cannot read, so the inventory is unprovable and the gate fails closed",
        origin,
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
fn audit_applied_attributes(
    list: &MetaList,
    origin: &Utf8Path,
    shadowed: &BTreeSet<String>,
) -> Result<()> {
    let Ok(arguments) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
    else {
        bail!(
            "{origin} carries a `#[cfg_attr]` this walk cannot parse; an attribute it applies can \
             be a macro whose expansion declares items this walk cannot read, so the inventory is \
             unprovable and the gate fails closed",
        );
    };
    arguments
        .iter()
        .skip(1)
        .try_for_each(|applied| audit_meta(applied, origin, shadowed))
}

/// Audit the derives one attribute names.
///
/// A derive macro receives the annotated item and appends its output to the
/// item's scope, so an unknown derive is an item-producing expansion like an
/// attribute macro. The arguments are paths rather than generic meta, because
/// that is the grammar `derive` accepts. A built-in name is still refused where
/// the module's own declarations rebind it, because then the name denotes
/// whatever the binding imports, not the compiler's derive.
fn audit_derives(meta: &Meta, origin: &Utf8Path, shadowed: &BTreeSet<String>) -> Result<()> {
    let Meta::List(list) = meta else {
        // A bare `#[derive]` names nothing, so it expands nothing.
        return Ok(());
    };
    let Ok(derives) = list.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated) else {
        bail!(
            "{origin} carries a `#[derive]` this walk cannot parse; a derive macro can declare \
             items this walk cannot read, so the inventory is unprovable and the gate fails closed",
        );
    };
    for derive in &derives {
        ensure!(
            is_inert_derive(derive),
            "{} derives `{}`, which is not one of the compiler's built-in derives; a derive macro \
             can declare items this walk cannot read, so the inventory is unprovable and the gate \
             fails closed",
            origin,
            quote_path(derive),
        );
        if let Some(name) = sole_identifier(derive)
            && shadowed.contains(&name)
        {
            bail!(
                "{origin} binds a derive named `{name}`, which can be a procedural macro; its \
                 expansion can declare items this walk cannot read, so the inventory is unprovable \
                 and the gate fails closed",
            );
        }
    }
    Ok(())
}

/// Audit every item in one list, descending into inline modules.
///
/// A nested module is audited against its own declarations rather than its
/// parent's: Rust modules do not inherit `use` bindings, so a binding beside
/// the module says nothing about the names inside it.
fn audit_items(items: &[Item], origin: &Utf8Path, shadowed: &BTreeSet<String>) -> Result<()> {
    for item in items {
        audit_attributes(item_attributes(item), origin, shadowed)?;
        if let Item::Mod(module) = item
            && let Some((_, nested)) = &module.content
        {
            let nested_shadowed = shadowed_derives(nested);
            audit_items(nested, origin, &nested_shadowed)?;
        }
    }
    Ok(())
}

/// Audit every attribute in one list.
fn audit_attributes(
    attributes: &[Attribute],
    origin: &Utf8Path,
    shadowed: &BTreeSet<String>,
) -> Result<()> {
    attributes
        .iter()
        .try_for_each(|attribute| audit_meta(&attribute.meta, origin, shadowed))
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
    sole_identifier(path).is_some_and(|name| is_builtin_derive(&name))
}

/// Whether `name` is one of the compiler's built-in derive names.
fn is_builtin_derive(name: &str) -> bool {
    INERT_DERIVES.contains(&name)
}
