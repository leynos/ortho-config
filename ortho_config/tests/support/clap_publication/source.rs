//! Locate and count the resolved `ErrorKind` variants.
//!
//! The variant count must come from the source Cargo actually built, not from
//! a guessed registry path or a hard-coded file name. The gate therefore walks
//! the resolved package's `src/` tree, parses every Rust file, and requires
//! exactly one top-level `enum ErrorKind`. Anything the count cannot see
//! safely — a conditional variant, a duplicate definition, a parse failure, a
//! file or module the compiler may exclude — fails the gate instead of
//! producing a count that does not describe the resolved build.
//!
//! A declaration inside `src/` is not by itself evidence that the compiler
//! sees it: `#[cfg]` on a `mod` declaration, or an inner `#![cfg]` at the top
//! of a module file, can make the declaring file unreachable. The walk
//! therefore follows `mod` declarations from `lib.rs` and certifies the
//! declaring file only when every hop is unconditional.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use cargo_metadata::Package;
use syn::punctuated::Punctuated;
use syn::{Attribute, Item, ItemEnum, ItemMod, Meta, Token};

/// The crate root a library package compiles first.
const CRATE_ROOT: &str = "lib.rs";

/// Count the `ErrorKind` variants declared by `package`'s resolved source.
pub(crate) fn count_resolved_error_kinds(package: &Package) -> Result<usize> {
    let source_root = package_source_root(package)?;
    let directory = Dir::open_ambient_dir(&source_root, ambient_authority())
        .with_context(|| format!("open {source_root} to inventory ErrorKind"))?;
    count_under(&directory, &source_root)
}

/// Count the `ErrorKind` variants declared by a source tree rooted at `root`.
///
/// The fixture controls use this directly, so a committed tree can prove each
/// refusal without touching the real registry cache.
pub(super) fn count_error_kinds_under(root: &Utf8Path) -> Result<usize> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open {root} to inventory ErrorKind"))?;
    count_under(&directory, root)
}

/// Count the one unconditional `ErrorKind` declared below `directory`.
fn count_under(directory: &Dir, source_root: &Utf8Path) -> Result<usize> {
    let sources = rust_sources(directory, Utf8Path::new(""))?;
    let mut definitions = Vec::new();
    for relative in &sources {
        let source = directory
            .read_to_string(relative)
            .with_context(|| format!("read {source_root}/{relative}"))?;
        if let Some(count) = analyze_error_kind(&source, relative)? {
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
    ensure_unconditional(directory, &sources, declaring)?;
    Ok(*count)
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
    reject_cfg_attributes(&file.attrs, "the whole file", origin)?;
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

/// Whether any attribute can remove its item from the build.
///
/// `#[cfg(..)]` always can. `#[cfg_attr(cond, ..)]` cannot by itself — it only
/// applies attributes — unless one of the attributes it applies is another
/// `cfg`. The distinction matters here: `clap_builder` conditions its crate
/// root with `#![cfg_attr(docsrs, feature(doc_cfg))]`, which leaves the file
/// in every build, so refusing every `cfg_attr` would fail the real package.
fn has_presence_conditional_attribute(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            || (attribute.path().is_ident("cfg_attr") && cfg_attr_applies_a_cfg(attribute))
    })
}

/// Whether a `cfg_attr` applies a nested `cfg`, and so can remove its item.
///
/// Anything this cannot parse is treated as conditional: an unreadable
/// `cfg_attr` must not be the reason a count is trusted.
fn cfg_attr_applies_a_cfg(attribute: &Attribute) -> bool {
    let Meta::List(list) = &attribute.meta else {
        return true;
    };
    let Ok(arguments) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
    else {
        return true;
    };
    arguments.iter().skip(1).any(|applied| {
        matches!(applied, Meta::List(inner) if inner.path.is_ident("cfg"))
            || matches!(applied, Meta::Path(path) if path.is_ident("cfg"))
    })
}

/// Refuse a declaring file the compiler does not unconditionally compile.
fn ensure_unconditional(
    directory: &Dir,
    sources: &[Utf8PathBuf],
    declaring: &Utf8Path,
) -> Result<()> {
    let reachable = unconditionally_compiled(directory, sources)?;
    ensure!(
        reachable.contains(declaring),
        "{declaring} declares ErrorKind but the compiler does not unconditionally compile it \
         from {CRATE_ROOT}; a conditional `mod` hop or an inner `#![cfg]` makes the inventory \
         unprovable, so the gate fails closed",
    );
    Ok(())
}

/// Every file the compiler compiles whenever it compiles `lib.rs`.
///
/// Only unconditional module declarations are followed, and a file whose own
/// inner attributes can remove it from the build certifies nothing. A
/// declaration reachable only through a conditional hop is therefore not
/// certified, which is the fail-closed direction.
fn unconditionally_compiled(
    directory: &Dir,
    sources: &[Utf8PathBuf],
) -> Result<BTreeSet<Utf8PathBuf>> {
    ensure!(
        sources.contains(&Utf8PathBuf::from(CRATE_ROOT)),
        "expected a crate root {CRATE_ROOT} to locate ErrorKind from",
    );
    let mut walker = Reachability {
        directory,
        sources,
        reachable: BTreeSet::new(),
    };
    walker.visit_file(Utf8Path::new(CRATE_ROOT), 0)?;
    Ok(walker.reachable)
}

/// Walks unconditional module declarations from the crate root.
struct Reachability<'a> {
    /// Handle on the source tree being walked.
    directory: &'a Dir,
    /// Every `.rs` file in the tree, so a hop is only followed when it resolves.
    sources: &'a [Utf8PathBuf],
    /// Files certified as compiled unconditionally.
    reachable: BTreeSet<Utf8PathBuf>,
}

impl Reachability<'_> {
    /// Certifies `file` and everything it declares unconditionally.
    fn visit_file(&mut self, file: &Utf8Path, depth: usize) -> Result<()> {
        ensure!(
            depth <= MAX_MODULE_DEPTH,
            "{file} nests modules more than {MAX_MODULE_DEPTH} deep; the gate will not certify it",
        );
        if !self.reachable.insert(file.to_path_buf()) {
            return Ok(());
        }
        let source = self
            .directory
            .read_to_string(file)
            .with_context(|| format!("read {file} while locating ErrorKind"))?;
        let parsed =
            syn::parse_file(&source).with_context(|| format!("{file} should be valid Rust"))?;
        if has_presence_conditional_attribute(&parsed.attrs) {
            self.reachable.remove(file);
            return Ok(());
        }
        let prefix = module_directory(file);
        self.visit_items(&parsed.items, &prefix, depth)
    }

    /// Follows the unconditional modules among `items`, declared below `prefix`.
    fn visit_items(&mut self, items: &[Item], prefix: &Utf8Path, depth: usize) -> Result<()> {
        for item in items {
            let Item::Mod(module) = item else { continue };
            if has_presence_conditional_attribute(&module.attrs) {
                continue;
            }
            match &module.content {
                Some((_, nested)) => {
                    let inline = prefix.join(module.ident.to_string());
                    self.visit_items(nested, &inline, depth + 1)?;
                }
                None => self.visit_declared_files(module, prefix, depth)?,
            }
        }
        Ok(())
    }

    /// Follows the files one `mod name;` declaration below `prefix` resolves to.
    fn visit_declared_files(
        &mut self,
        module: &ItemMod,
        prefix: &Utf8Path,
        depth: usize,
    ) -> Result<()> {
        for candidate in module_files(prefix, module, self.sources) {
            self.visit_file(&candidate, depth + 1)?;
        }
        Ok(())
    }
}

/// How deep a module tree may nest before the gate refuses to certify it.
const MAX_MODULE_DEPTH: usize = 32;

/// The files a `mod name;` declared below `prefix` may resolve to, when present.
///
/// Rust resolves a file module relative to the module's own directory: beside
/// `lib.rs`, below `foo/` for both `foo.rs` and `foo/mod.rs`, and below the
/// inline module's name for a nested declaration.
fn module_files(prefix: &Utf8Path, module: &ItemMod, sources: &[Utf8PathBuf]) -> Vec<Utf8PathBuf> {
    let name = module.ident.to_string();
    [
        prefix.join(format!("{name}.rs")),
        prefix.join(name).join("mod.rs"),
    ]
    .into_iter()
    .filter(|candidate| sources.contains(candidate))
    .collect()
}

/// The directory a `mod name;` declared in `file` resolves within.
///
/// A crate root (`lib.rs`, `main.rs`) and a `mod.rs` keep their modules beside
/// themselves; any other `foo.rs` keeps them below `foo/`.
fn module_directory(file: &Utf8Path) -> Utf8PathBuf {
    let parent = file.parent().unwrap_or_else(|| Utf8Path::new(""));
    let is_crate_root = matches!(file.file_name(), Some("mod.rs" | "lib.rs" | "main.rs"));
    if is_crate_root {
        return parent.to_path_buf();
    }
    file.file_stem()
        .map_or_else(|| parent.to_path_buf(), |stem| parent.join(stem))
}
