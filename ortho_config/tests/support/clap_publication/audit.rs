//! Attribute auditing for the `ErrorKind` source inventory.
//!
//! The inventory is trustworthy only when the declaration it counts is the
//! declaration the compiler builds. Four questions decide that, and each is
//! answered here rather than in the walk that follows modules across files:
//! whether an item's compilation is conditional, whether a `mod` declaration
//! redirects the compiler to a file a default-name walk would not open, whether
//! the inventory type is declared somewhere a file-level scan cannot bind to
//! the item the crate exports, and whether an item-level macro could declare it
//! after this parse.
//!
//! Keeping these predicates together is deliberate. Each of them fails closed,
//! and a control that reverts one of them must fail for the reason it names,
//! so they are audited as one subject rather than scattered through the walk.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail, ensure};
use camino::Utf8Path;
use syn::punctuated::Punctuated;
use syn::{Attribute, Item, ItemEnum, ItemMacro, Meta, MetaList, Token};

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

/// Item-level macros that cannot declare anything the gate would count.
///
/// `compile_error!` expands to a diagnostic and no item, so a crate that
/// compiles with one in place declares exactly what the parse shows. Every
/// other invocation is refused below, including `include!`, whose contents are
/// a file this walk never reads.
const INERT_ITEM_MACROS: [&str; 1] = ["compile_error"];

/// The identifier a generated inventory would have to spell.
const INVENTORY: &str = "ErrorKind";

/// Refuse an item-level macro invocation the walk cannot expand.
///
/// Rust parses a macro invocation as one item and only later substitutes the
/// items it produces. A crate can therefore compile a seventeen-variant decoy
/// beside `pub mod error { include!("inventory.inc"); }`, where the included
/// file declares the exported eighteen-variant inventory: the file-level count
/// sees the decoy, the gate accepts it, and the number describes the wrong
/// enum.
///
/// The gate establishes an expansion only for a macro this file defines with
/// `macro_rules!` in an enclosing item list. Those tokens are visible, so the
/// gate walks the transcription and the call's arguments and refuses when
/// either can name the inventory. Any other invocation is refused without
/// further analysis: `include!` splices a file the source walk does not map,
/// and an imported macro's body is not in this file at all.
///
/// A `macro_rules!` definition is not an invocation — it declares nothing until
/// a call site is audited in turn — and `compile_error!` produces a diagnostic
/// and no item, so both are allowed.
pub(super) fn refuse_item_macros(items: &[Item], origin: &Utf8Path) -> Result<()> {
    audit_item_macros(items, &MacroBodies::new(), origin)
}

/// The `macro_rules!` transcriptions a set of item lists has in scope.
type MacroBodies = BTreeMap<String, String>;

/// Audit one item list, which inherits the macros its enclosing lists define.
fn audit_item_macros(items: &[Item], inherited: &MacroBodies, origin: &Utf8Path) -> Result<()> {
    let scope = local_macro_bodies(items, inherited);
    for item in items {
        match item {
            Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    audit_item_macros(nested, &scope, origin)?;
                }
            }
            Item::Macro(invocation) => audit_invocation(invocation, &scope, origin)?,
            _ => {}
        }
    }
    Ok(())
}

/// The inherited macros, overridden by the ones this item list defines.
fn local_macro_bodies(items: &[Item], inherited: &MacroBodies) -> MacroBodies {
    let mut scope = inherited.clone();
    scope.extend(items.iter().filter_map(macro_rules_definition));
    scope
}

/// The name and transcription of an item's `macro_rules!` definition.
fn macro_rules_definition(item: &Item) -> Option<(String, String)> {
    let Item::Macro(definition) = item else {
        return None;
    };
    if !definition.mac.path.is_ident("macro_rules") {
        return None;
    }
    let name = definition.ident.as_ref()?;
    Some((name.to_string(), definition.mac.tokens.to_string()))
}

/// Refuse one invocation unless its expansion provably cannot name the enum.
fn audit_invocation(invocation: &ItemMacro, scope: &MacroBodies, origin: &Utf8Path) -> Result<()> {
    let path = &invocation.mac.path;
    let is_inert = INERT_ITEM_MACROS.iter().any(|name| path.is_ident(name));
    if path.is_ident("macro_rules") || is_inert {
        return Ok(());
    }
    let provable = path.get_ident().is_some_and(|name| {
        let mut proven = BTreeSet::new();
        expands_without_inventory(
            &name.to_string(),
            &invocation.mac.tokens.to_string(),
            scope,
            &mut proven,
        )
    });
    ensure!(
        provable,
        "{origin} invokes `{}!` at item level; its expansion can declare items this walk cannot \
         read, so the inventory is unprovable and the gate fails closed",
        quote_path(path),
    );
    Ok(())
}

/// Whether `name` expands from tokens that can never spell the inventory's name.
///
/// A generated `enum ErrorKind` has to carry the identifier `ErrorKind` in the
/// transcription that produces it, and a macro can only receive tokens from its
/// own transcription or the call's arguments. A name this file does not define
/// has no transcription to read, which is why the answer is `false` here and
/// [`audit_invocation`] refuses it.
fn expands_without_inventory(
    name: &str,
    arguments: &str,
    scope: &MacroBodies,
    proven: &mut BTreeSet<String>,
) -> bool {
    let Some(body) = scope.get(name) else {
        return false;
    };
    !mentions_inventory(arguments) && transcription_is_clean(name, body, scope, proven)
}

/// Whether a transcription and everything it invokes stay clear of the name.
///
/// An invocation inside the transcription may name a macro this file does not
/// define. That macro can only receive tokens the transcription already
/// supplied, and those were scanned with it, so an unresolved nested name is
/// accepted rather than refused.
fn transcription_is_clean(
    name: &str,
    body: &str,
    scope: &MacroBodies,
    proven: &mut BTreeSet<String>,
) -> bool {
    if mentions_inventory(body) {
        return false;
    }
    if !proven.insert(name.to_owned()) {
        return true;
    }
    invoked_macro_names(body).iter().all(|nested| {
        scope
            .get(nested)
            .is_none_or(|nested_body| transcription_is_clean(nested, nested_body, scope, proven))
    })
}

/// Whether the tokens can spell the inventory's identifier.
fn mentions_inventory(tokens: &str) -> bool {
    tokens
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .any(|token| token == INVENTORY)
}

/// The macros a rendered transcription invokes.
///
/// Rendering separates tokens with spaces, so the identifier immediately before
/// a `!` is an invocation. A name this scope cannot resolve is returned as well
/// and fails the audit by resolving to no transcription.
fn invoked_macro_names(tokens: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = tokens;
    while let Some((before, after)) = rest.split_once('!') {
        if let Some(name) = before
            .split_whitespace()
            .next_back()
            .filter(|word| is_identifier(word))
        {
            names.push(name.to_owned());
        }
        rest = after;
    }
    names
}

/// Whether a rendered token is a bare identifier.
fn is_identifier(word: &str) -> bool {
    let Some(first) = word.chars().next() else {
        return false;
    };
    let starts_ident = first.is_alphabetic() || first == '_';
    starts_ident
        && word
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
}

/// Render a macro path the way the source spells it.
fn quote_path(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
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
