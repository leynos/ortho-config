//! Item-macro auditing for the `ErrorKind` source inventory.
//!
//! Rust parses a macro invocation as one item and substitutes what it declares
//! only afterwards, so an `enum ErrorKind` a macro generates never appears in
//! the walk's parse. This module answers the one question the attribute audit
//! cannot: could an item-level invocation declare the inventory after the
//! parse that counted the file?
//!
//! An expansion is established only for a macro the same item-list tree
//! defines with `macro_rules!`. Those tokens are visible, so the transcription
//! and the call's arguments can both be read. Any other invocation is refused
//! without further analysis: an imported macro's body is not in this file at
//! all, and `include!` splices source the walk never reads.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use camino::Utf8Path;
use syn::{Item, ItemMacro};

/// Item-level macros that cannot declare anything the gate would count.
///
/// `compile_error!` expands to a diagnostic and no item, so a crate that
/// compiles with one in place declares exactly what the parse shows. Every
/// other invocation is refused below, including `include!`, whose contents are
/// a file this walk never reads.
const INERT_ITEM_MACROS: [&str; 1] = ["compile_error"];

/// Macros a transcription may invoke without this file defining them.
///
/// Every entry expands only to an expression, statement, or literal, so none
/// can contribute the item that declares the inventory. The list is closed on
/// purpose: an unresolved name that is not here is refused, because an imported
/// macro's transcription is not in this file at all and `include!` above all
/// can splice source the walk never reads.
const INERT_NESTED_MACROS: [&str; 35] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "cfg",
    "column",
    "compile_error",
    "concat",
    "dbg",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "env",
    "eprint",
    "eprintln",
    "file",
    "format",
    "format_args",
    "include_bytes",
    "include_str",
    "line",
    "macro_rules",
    "matches",
    "module_path",
    "option_env",
    "panic",
    "print",
    "println",
    "stringify",
    "todo",
    "try",
    "unimplemented",
    "unreachable",
    "vec",
    "write",
    "writeln",
];

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
/// A nested invocation must resolve to a transcription in `scope` or be one of
/// [`INERT_NESTED_MACROS`]. Accepting an unresolved name because it "only
/// receives tokens the transcription supplied" is not sound: a wrapper whose
/// transcription calls `include!` hands that macro one string literal, and the
/// file it names supplies the items. The wrapper's own transcription never
/// spells `ErrorKind`, so only the refusal of the unresolved nested name stops
/// the decoy beside it from being counted.
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
        is_inert_nested(nested)
            || scope.get(nested).is_some_and(|nested_body| {
                transcription_is_clean(nested, nested_body, scope, proven)
            })
    })
}

/// Whether a nested invocation provably contributes no item.
fn is_inert_nested(name: &str) -> bool {
    INERT_NESTED_MACROS.contains(&name)
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
