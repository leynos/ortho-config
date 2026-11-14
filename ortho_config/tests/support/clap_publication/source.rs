//! Locate and count the resolved `ErrorKind` variants.
//!
//! The variant count must come from the source Cargo actually built, not from
//! a guessed registry path or a hard-coded file name. The gate therefore walks
//! the resolved package's `src/` tree, parses every Rust file, and requires
//! exactly one top-level `enum ErrorKind`. Anything the count cannot see
//! safely — a conditional variant, a duplicate definition, a parse failure —
//! fails the gate instead of producing a smaller number.

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use cargo_metadata::Package;
use syn::{Attribute, Item, ItemEnum};

/// Count the `ErrorKind` variants declared by `package`'s resolved source.
pub(crate) fn count_resolved_error_kinds(package: &Package) -> Result<usize> {
    let source_root = package_source_root(package)?;
    let directory = Dir::open_ambient_dir(&source_root, ambient_authority())
        .with_context(|| format!("open {source_root} to inventory ErrorKind"))?;
    let mut definitions = Vec::new();
    for relative in rust_sources(&directory, Utf8Path::new(""))? {
        let source = directory
            .read_to_string(&relative)
            .with_context(|| format!("read {source_root}/{relative}"))?;
        if let Some(count) = analyze_error_kind(&source, &relative)? {
            definitions.push((relative, count));
        }
    }
    match definitions.as_slice() {
        [(_, count)] => Ok(*count),
        _ => bail!(
            "expected exactly one top-level ErrorKind definition under {source_root}, found {:?}",
            definitions
                .iter()
                .map(|(path, _)| path.as_str())
                .collect::<Vec<_>>(),
        ),
    }
}

/// Count the variants of the single top-level `ErrorKind` in `source`.
///
/// The fixture controls use this directly, so a synthetic source can prove
/// each refusal without touching the real registry cache.
pub(super) fn count_error_kind_variants(source: &str, origin: &Utf8Path) -> Result<usize> {
    analyze_error_kind(source, origin)?
        .with_context(|| format!("{origin} declares no top-level ErrorKind enum to inventory"))
}

/// The `<package>/src` directory containing the package's Rust sources.
fn package_source_root(package: &Package) -> Result<Utf8PathBuf> {
    package
        .manifest_path
        .parent()
        .map(|root| root.join("src"))
        .with_context(|| {
            format!(
                "{} should sit in a package directory",
                package.manifest_path
            )
        })
}

/// Every `.rs` file below `directory`, as paths relative to the root.
fn rust_sources(directory: &Dir, prefix: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let mut sources = Vec::new();
    collect_sources(directory, prefix, &mut sources)?;
    sources.sort();
    Ok(sources)
}

/// Recursively collect Rust sources, skipping symlinked entries.
///
/// `file_type` reports a symlink as such, so a symlink cycle cannot make the
/// walk unbounded; a skipped symlink would still surface as a missing variant.
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
        } else if relative.extension() == Some("rs") {
            sources.push(relative);
        }
    }
    Ok(())
}

/// Count one file's top-level `ErrorKind` variants, refusing conditional ones.
fn analyze_error_kind(source: &str, origin: &Utf8Path) -> Result<Option<usize>> {
    let file = syn::parse_file(source).with_context(|| format!("{origin} should be valid Rust"))?;
    let mut definitions = file.items.iter().filter_map(|item| match item {
        Item::Enum(definition) if definition.ident == "ErrorKind" => Some(definition),
        _ => None,
    });
    let Some(definition) = definitions.next() else {
        return Ok(None);
    };
    ensure!(
        definitions.next().is_none(),
        "{origin} declares more than one top-level ErrorKind enum",
    );
    audit_definition(definition, origin)?;
    Ok(Some(definition.variants.len()))
}

/// Refuse an enum whose own or whose variants' compilation is conditional.
fn audit_definition(definition: &ItemEnum, origin: &Utf8Path) -> Result<()> {
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
fn reject_cfg_attributes(attributes: &[Attribute], subject: &str, origin: &Utf8Path) -> Result<()> {
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
