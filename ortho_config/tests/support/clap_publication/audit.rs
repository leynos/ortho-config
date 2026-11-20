//! Attribute auditing for the `ErrorKind` source inventory.
//!
//! The inventory is trustworthy only when the declaration it counts is the
//! declaration the compiler builds. Four questions decide that: whether an
//! item's compilation is conditional, whether a `mod` declaration redirects the
//! compiler to a file a default-name walk would not open, whether the inventory
//! type is declared somewhere a file-level scan cannot bind to the item the
//! crate exports, and whether an item-level macro could declare it after this
//! parse. The first three are answered here; [`super::macros`] answers the
//! fourth.
//!
//! The predicates here stay together because each of them fails closed, and a
//! control that reverts one must fail for the reason it names, so they are
//! audited as one subject rather than scattered through the module walk.

use anyhow::{Result, bail, ensure};
use camino::Utf8Path;
use syn::punctuated::Punctuated;
use syn::{Attribute, Item, ItemEnum, Meta, MetaList, Token};

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
    attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
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
    attributes
        .iter()
        .any(|attribute| meta_applies(&attribute.meta, "cfg"))
}

/// Whether a `mod` declaration names the file to load instead of the default.
///
/// This walk resolves default file names only. A `#[path]` — or a `cfg_attr`
/// that applies one — can make the compiler load a different file, so the
/// declaration is refused rather than followed as if the default held.
pub(super) fn declares_module_path(attributes: &[Attribute]) -> bool {
    attributes
        .iter()
        .any(|attribute| meta_applies(&attribute.meta, "path"))
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
        Meta::Path(path) => path.is_ident(name),
        Meta::NameValue(entry) => entry.path.is_ident(name),
        Meta::List(list) if list.path.is_ident("cfg_attr") => applies_through_cfg_attr(list, name),
        Meta::List(list) => list.path.is_ident(name),
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
pub(super) fn is_error_kind_enum(item: &Item) -> bool {
    matches!(item, Item::Enum(definition) if definition.ident == "ErrorKind")
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
