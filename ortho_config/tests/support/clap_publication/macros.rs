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
//!
//! A `macro_rules!` name is resolved against the definitions textually in
//! scope at the call, so an item list is walked in source order and its scope
//! grows as each definition is met. Reading every definition in the list first
//! would let a later redefinition describe an earlier call that the compiler
//! expands with the earlier transcription.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use camino::Utf8Path;
use syn::{Item, ItemMacro, ItemMod};

/// Item-level macros that cannot declare anything the gate would count.
///
/// `compile_error!` expands to a diagnostic and no item, so a crate that
/// compiles with one in place declares exactly what the parse shows. Every
/// other invocation is refused below, including `include!`, whose contents are
/// a file this walk never reads. The list decides only names the declaring file
/// does not define itself: a local `macro_rules!` shadows a prelude macro of the
/// same name, so a local `compile_error!` is audited through its transcription
/// instead of being trusted here.
const INERT_ITEM_MACROS: [&str; 1] = ["compile_error"];

/// Macros a transcription may invoke without this file defining them.
///
/// Every entry expands only to an expression, statement, or literal, so none
/// can contribute the item that declares the inventory. The list is closed on
/// purpose: an unresolved name that is not here is refused, because an imported
/// macro's transcription is not in this file at all and `include!` above all
/// can splice source the walk never reads. It is consulted only after the
/// declaring file's own definitions, because a local `macro_rules!` shadows the
/// standard macro the name would otherwise resolve to.
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
/// `macro_rules!` and that is in scope at the call. Those tokens are visible,
/// so the gate walks the transcription and the call's arguments and refuses
/// when either can name the inventory or invoke a macro it cannot resolve. Any
/// other invocation is refused without further analysis: `include!` splices a
/// file the source walk does not map, and an imported macro's body is not in
/// this file at all.
///
/// A `macro_rules!` definition is not an invocation — it declares nothing until
/// a call site is audited in turn — and `compile_error!` produces a diagnostic
/// and no item, so both are allowed.
pub(super) fn refuse_item_macros(items: &[Item], origin: &Utf8Path) -> Result<()> {
    audit_item_macros(items, &MacroBodies::new(), origin)
}

/// The `macro_rules!` transcriptions in scope at one point in an item list.
type MacroBodies = BTreeMap<String, String>;

/// Audit one item list, which inherits the macros its enclosing lists define.
///
/// The list is walked in source order because that is how a `macro_rules!`
/// name resolves: a definition reaches the items that follow it, and a later
/// redefinition does not reach backwards to an earlier call. A nested inline
/// module is audited against the scope in force where it is declared, and the
/// definitions it adds stay inside it.
fn audit_item_macros(items: &[Item], inherited: &MacroBodies, origin: &Utf8Path) -> Result<()> {
    let mut scope = inherited.clone();
    for item in items {
        match item {
            Item::Mod(module) => audit_inline_module(module, &scope, origin)?,
            Item::Macro(invocation) => audit_item_macro(invocation, &mut scope, origin)?,
            _ => {}
        }
    }
    Ok(())
}

/// Audit a module declared inline; a file module is left to the source walk.
fn audit_inline_module(module: &ItemMod, scope: &MacroBodies, origin: &Utf8Path) -> Result<()> {
    if let Some((_, nested)) = &module.content {
        audit_item_macros(nested, scope, origin)?;
    }
    Ok(())
}

/// Record a definition for the items after it, or audit an invocation now.
///
/// A definition declares nothing until a call site is audited in turn, and it
/// becomes visible only to the items that follow it.
fn audit_item_macro(
    invocation: &ItemMacro,
    scope: &mut MacroBodies,
    origin: &Utf8Path,
) -> Result<()> {
    match macro_rules_definition(invocation) {
        Some((name, body)) => {
            scope.insert(name, body);
            Ok(())
        }
        None => audit_invocation(invocation, scope, origin),
    }
}

/// The name and transcription of an item macro's `macro_rules!` definition.
fn macro_rules_definition(invocation: &ItemMacro) -> Option<(String, String)> {
    if !invocation.mac.path.is_ident("macro_rules") {
        return None;
    }
    let name = invocation.ident.as_ref()?;
    Some((name.to_string(), invocation.mac.tokens.to_string()))
}

/// Refuse one invocation unless its expansion provably cannot name the enum.
///
/// A name this file defines is audited through its own transcription even when
/// it also names a standard macro: Rust lets a `macro_rules!` definition shadow
/// a prelude or `std` macro of the same name, so `assert_eq!` here may be a
/// local wrapper around `include!` rather than the assertion macro. Resolution
/// therefore comes first and the inert list only decides names this file leaves
/// to the prelude.
fn audit_invocation(invocation: &ItemMacro, scope: &MacroBodies, origin: &Utf8Path) -> Result<()> {
    let path = &invocation.mac.path;
    if path.is_ident("macro_rules") {
        return Ok(());
    }
    let provable = path.get_ident().is_some_and(|name| {
        let invoked = name.to_string();
        if scope.contains_key(&invoked) {
            let mut proven = BTreeSet::new();
            expands_without_inventory(
                &invoked,
                &invocation.mac.tokens.to_string(),
                scope,
                &mut proven,
            )
        } else {
            INERT_ITEM_MACROS.contains(&invoked.as_str())
        }
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
    !mentions_inventory(arguments)
        && arguments_are_clean(arguments, scope, proven)
        && transcription_is_clean(name, body, scope, proven)
}

/// Whether every macro the call's arguments invoke provably contributes no item.
///
/// A transcription with an item-position parameter pastes its argument into the
/// expansion, so the arguments are a second route into the item list:
/// `paste! { include!("inventory.inc") }` hands `include!` to a body that is
/// nothing but its parameter, and neither the argument nor the transcription
/// spells `ErrorKind`. Resolving the argument's names is the only way to see
/// that splice, and an unresolvable one fails the audit as it does inside a
/// transcription.
fn arguments_are_clean(
    arguments: &str,
    scope: &MacroBodies,
    proven: &mut BTreeSet<String>,
) -> bool {
    invoked_macro_names(arguments)
        .iter()
        .all(|nested| nested_is_clean(nested, scope, proven))
}

/// Whether a transcription and everything it invokes stay clear of the name.
///
/// A nested invocation is resolved against `scope` first and only then against
/// [`INERT_NESTED_MACROS`]. The order matters because a local `macro_rules!`
/// shadows a prelude macro of the same name: treating `assert_eq!` as inert
/// whenever it appears would let a local definition of that name hide an
/// `include!` behind the name of an assertion. Accepting an unresolved name
/// because it "only receives tokens the transcription supplied" is not sound
/// either: a wrapper whose transcription calls `include!` hands that macro one
/// string literal, and the file it names supplies the items.
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
    invoked_macro_names(body)
        .iter()
        .all(|nested| nested_is_clean(nested, scope, proven))
}

/// Whether a nested invocation provably contributes no item.
///
/// A local definition wins over the inert list, because it shadows the standard
/// macro the name would otherwise resolve to.
fn nested_is_clean(nested: &str, scope: &MacroBodies, proven: &mut BTreeSet<String>) -> bool {
    scope.get(nested).map_or_else(
        || is_inert_nested(nested),
        |body| transcription_is_clean(nested, body, scope, proven),
    )
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
