//! Locate and count the resolved `ErrorKind` variants.
//!
//! The variant count must come from the source Cargo actually built, not from
//! a guessed registry path or a hard-coded file name. The gate therefore walks
//! the tree below the crate root Cargo reports, parses every Rust file, and
//! requires exactly one top-level `enum ErrorKind`. Anything the count cannot
//! see safely — a conditional variant, a duplicate definition, a parse
//! failure, a file or module the compiler may exclude — fails the gate instead
//! of producing a count that does not describe the resolved build.
//!
//! A declaration inside the source tree is not by itself evidence that the
//! compiler sees it: `#[cfg]` on a `mod` declaration, a condition applied
//! through a nested `cfg_attr`, or an inner `#![cfg]` at the top of a module
//! file can make the declaring file unreachable. A `#[path]` is the opposite
//! hazard: it can make the compiler load a file this walk would not follow, and
//! a module file that is a symlink — or that exists only in a form the walk
//! skips — hides the same way. A macro invocation is a third: Rust parses the
//! call as one item and substitutes what it declares afterwards, so a
//! declaration a macro generates never appears in this parse. A `use` item is
//! the fourth: it can bind an imported macro to a name the audit would
//! otherwise trust, so the walk mines every parsed file for the macro and
//! import names the crate declares before it audits any invocation.
//!
//! The count is certified by [`super::reachability`], which follows `mod`
//! declarations from the crate root and certifies the declaring file only when
//! every hop to it is unconditional. A declaration nested inside an inline
//! module is refused before the scan: a file-level count cannot bind it to the
//! inventory the crate exports, and counting the file-level decoy beside it
//! would report a number that describes the wrong enum.
//!
//! The crate root is the one Cargo reports for the resolved package's library
//! target, not `src/lib.rs` by convention: [`library_crate_root`] settles that
//! first, because a package may keep an unused `src/lib.rs` beside the file it
//! really compiles.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use cargo_metadata::Package;
use syn::Item;

use super::audit::{
    audit_definition, is_error_kind_name, refuse_error_kind_redeclarations,
    refuse_error_kind_renames, refuse_nested_error_kinds, reject_cfg_attributes,
};
use super::expansion::refuse_unverified_attributes;
use super::imports::CrateScope;
use super::library::library_crate_root;
use super::macros::{collect_crate_facts, refuse_item_macros};
use super::reachability::unconditionally_compiled;

/// The crate root a library package compiles first when it declares no path.
const CRATE_ROOT: &str = "lib.rs";

/// Count the `ErrorKind` variants declared by `package`'s resolved source.
///
/// The walk starts at the crate root Cargo reports for the package's library
/// target, so a package that moves its root with `[lib] path` is counted from
/// the file the compiler builds rather than from the default location.
pub(crate) fn count_resolved_error_kinds(package: &Package) -> Result<usize> {
    let crate_root = library_crate_root(package)?;
    let (Some(source_root), Some(name)) = (crate_root.parent(), crate_root.file_name()) else {
        bail!("{crate_root} should sit in a source directory and name a file");
    };
    count_error_kinds_from(source_root, Utf8Path::new(name))
}

/// Count the `ErrorKind` variants declared by a source tree rooted at `root`.
///
/// The fixture controls use this directly, so a committed tree can prove each
/// refusal without touching the real registry cache.
pub(super) fn count_error_kinds_under(root: &Utf8Path) -> Result<usize> {
    count_error_kinds_from(root, Utf8Path::new(CRATE_ROOT))
}

/// Count from `root`, starting at `crate_root` rather than the default name.
pub(super) fn count_error_kinds_from(root: &Utf8Path, crate_root: &Utf8Path) -> Result<usize> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open {root} to inventory ErrorKind"))?;
    count_from(&directory, root, crate_root)
}

/// Count the one unconditional `ErrorKind` declared below `directory`.
fn count_from(directory: &Dir, source_root: &Utf8Path, crate_root: &Utf8Path) -> Result<usize> {
    let sources = rust_sources(directory, Utf8Path::new(""))?;
    let parsed = parse_sources(directory, source_root, &sources)?;
    let crate_scope = crate_scope_of(parsed.iter().map(|(_, file)| file));
    let mut definitions = Vec::new();
    for (relative, file) in &parsed {
        if let Some(count) = analyze_error_kind(file, &crate_scope, relative)? {
            definitions.push((relative.clone(), count));
        }
    }
    let [(declaring, count)] = definitions.as_slice() else {
        bail!(
            "expected exactly one top-level ErrorKind definition under {source_root}, found {:?}",
            definitions
                .iter()
                .map(|(path, _)| path.as_str())
                .collect::<Vec<_>>(),
        );
    };
    ensure_unconditional(directory, &sources, declaring, crate_root)?;
    Ok(*count)
}

/// Count the variants of the single top-level `ErrorKind` in `source`.
///
/// The fixture controls use this directly, so a synthetic source can prove
/// each refusal without touching the real registry cache.
pub(super) fn count_error_kind_variants(source: &str, origin: &Utf8Path) -> Result<usize> {
    let file = syn::parse_file(source).with_context(|| format!("{origin} should be valid Rust"))?;
    let crate_scope = crate_scope_of(std::iter::once(&file));
    analyze_error_kind(&file, &crate_scope, origin)?
        .with_context(|| format!("{origin} declares no top-level ErrorKind enum to inventory"))
}

/// Every Rust source below `directory`, parsed before anything is decided.
///
/// The crate-wide name set needs every file, not just the one being audited, so
/// a glob that re-exports a macro declared elsewhere in the crate is not
/// mistaken for one that could import an unknown name. Parsing once also keeps
/// the later audit from reading the tree twice.
fn parse_sources(
    directory: &Dir,
    source_root: &Utf8Path,
    sources: &[Utf8PathBuf],
) -> Result<Vec<(Utf8PathBuf, syn::File)>> {
    sources
        .iter()
        .map(|relative| {
            let source = directory
                .read_to_string(relative)
                .with_context(|| format!("read {source_root}/{relative}"))?;
            let file = syn::parse_file(&source)
                .with_context(|| format!("{relative} should be valid Rust"))?;
            Ok((relative.clone(), file))
        })
        .collect()
}

/// The crate-wide names and `#[macro_use]` facts the macro audit resolves against.
fn crate_scope_of<'a>(files: impl IntoIterator<Item = &'a syn::File>) -> CrateScope {
    let parsed: Vec<&syn::File> = files.into_iter().collect();
    let mut names = BTreeSet::new();
    let mut modules = BTreeSet::new();
    for file in &parsed {
        collect_crate_facts(&file.items, &mut names, &mut modules);
    }
    let mut scope = CrateScope::new(names, modules);
    for file in parsed {
        scope.observe(&file.items);
    }
    scope
}

/// Every `.rs` file below `directory`, as paths relative to the root.
fn rust_sources(directory: &Dir, prefix: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let mut sources = Vec::new();
    collect_sources(directory, prefix, &mut sources)?;
    sources.sort();
    Ok(sources)
}

/// Recursively collect regular Rust sources, skipping symlinked entries.
///
/// `file_type` reports a symlink as such, so a symlink cycle cannot make the
/// walk unbounded. The regular-file test is what stops an alias from being
/// read as a second module: `alias.rs` pointing at `error.rs` would otherwise
/// be parsed twice and report the same `ErrorKind` as a duplicate definition.
/// A skipped symlink would still surface as a missing variant.
fn collect_sources(
    directory: &Dir,
    prefix: &Utf8Path,
    sources: &mut Vec<Utf8PathBuf>,
) -> Result<()> {
    let entries = directory
        .entries()
        .with_context(|| format!("list {prefix}"))?;
    for listed in entries {
        let entry = listed.with_context(|| format!("read an entry under {prefix}"))?;
        let name = entry
            .file_name()
            .with_context(|| format!("read a file name under {prefix}"))?;
        let relative = prefix.join(&name);
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspect {relative}"))?;
        if file_type.is_dir() {
            let child = directory
                .open_dir(&name)
                .with_context(|| format!("open {relative}"))?;
            collect_sources(&child, &relative, sources)?;
        } else if file_type.is_file() && relative.extension() == Some("rs") {
            sources.push(relative);
        }
    }
    Ok(())
}

/// Count one file's top-level `ErrorKind` variants, refusing other shapes.
///
/// An attribute is audited before anything the parse shows is trusted, because
/// an attribute macro can replace any annotated item, including the enum this
/// count reads. A declaration nested in an inline module is refused next,
/// because the file-level scan could otherwise count a compiled decoy while
/// the exported inventory sat in a module it never looked inside.
fn analyze_error_kind(
    file: &syn::File,
    crate_scope: &CrateScope,
    origin: &Utf8Path,
) -> Result<Option<usize>> {
    refuse_unverified_attributes(file, origin)?;
    refuse_nested_error_kinds(&file.items, origin)?;
    refuse_error_kind_renames(&file.items, crate_scope, origin)?;
    refuse_error_kind_redeclarations(&file.items, origin)?;
    refuse_item_macros(&file.items, origin, crate_scope)?;
    let mut definitions = file.items.iter().filter_map(|item| match item {
        Item::Enum(definition) if is_error_kind_name(&definition.ident) => Some(definition),
        _ => None,
    });
    let Some(definition) = definitions.next() else {
        return Ok(None);
    };
    ensure!(
        definitions.next().is_none(),
        "{origin} declares more than one top-level ErrorKind enum",
    );
    reject_cfg_attributes(&file.attrs, "the whole file", origin)?;
    audit_definition(definition, origin)?;
    Ok(Some(definition.variants.len()))
}

/// Refuse a declaring file the compiler does not unconditionally compile.
fn ensure_unconditional(
    directory: &Dir,
    sources: &[Utf8PathBuf],
    declaring: &Utf8Path,
    crate_root: &Utf8Path,
) -> Result<()> {
    let reachable = unconditionally_compiled(directory, sources, crate_root)?;
    ensure!(
        reachable.contains(declaring),
        "{declaring} declares ErrorKind but the compiler does not unconditionally compile it \
         from {crate_root}; a conditional `mod` hop or an inner `#![cfg]` makes the inventory \
         unprovable, so the gate fails closed",
    );
    Ok(())
}
