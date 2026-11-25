//! `use` bindings for the item-macro audit.
//!
//! A `macro_rules!` definition is not the only thing that decides what a name
//! means at a call site: a `use` item can bind an imported macro to that name,
//! and the imported macro's transcription is not in this file at all.
//! `use crate::error_inject as compile_error;` is the sharpest example, because
//! it hides a splice behind a name the audit would otherwise trust as a
//! diagnostic-only prelude macro.
//!
//! A glob cannot be enumerated from the source alone, so it is judged by reach.
//! `use crate::…::*` and its `self`/`super` forms can only bind a name this
//! crate already declares, so the audit consults the crate-wide name set. Any
//! other glob may name an external crate, whose macro of any name it can bring
//! in, so no unresolved name is trusted in a module carrying one.
//!
//! A `#[macro_use]` module is the third route. It imports every `macro_rules!`
//! name its target declares into the enclosing scope and the modules below it,
//! so an inert name is trusted only where the crate declares no macro of that
//! name or `#[macro_use]` appears nowhere. The attribute can itself be applied
//! through `cfg_attr`, including a nested one, and Rust expands it before the
//! import happens, so the attribute is resolved recursively rather than by its
//! immediate spelling. `#[macro_use] extern crate` imports names from another
//! crate entirely, which no local set can bound.

use std::collections::{BTreeMap, BTreeSet};

use syn::{Attribute, Ident, Item, ItemMod, UseTree};

use super::audit::applies_attribute;
use super::tokens::identifier_name;

/// What the `use` items of one item list can bind.
#[derive(Debug, Default)]
pub(super) struct Imports {
    /// Names bound by an explicit `use` path, including `as` renames, and every
    /// root those bindings are imported through.
    ///
    /// The root matters beyond the macro audit: a derive name has to know
    /// whether an explicit binding could name a procedural macro, and a path
    /// rooted at `std`, `core` or `alloc` cannot. Every binding is kept rather
    /// than the last one seen, because a type and a derive macro occupy
    /// different namespaces: `use injector::Inject as Debug;` beside
    /// `use std::…::DefaultHasher as Debug;` compiles, and `#[derive(Debug)]`
    /// still invokes the procedural macro. A single root per name would let the
    /// later, trustworthy binding hide the earlier one.
    explicit: BTreeMap<String, Vec<Option<String>>>,
    /// Every module this item list declares directly.
    ///
    /// A `use` path is local only when its first segment names one of these,
    /// because a same-named module elsewhere in the crate is out of scope and
    /// Rust would resolve the path through the extern prelude instead.
    modules: BTreeSet<String>,
    /// `use crate::…::*`, `use self::…::*` and `use super::…::*`.
    has_internal_glob: bool,
    /// A glob whose path may name an external crate.
    has_external_glob: bool,
}

impl Imports {
    /// Record every binding the item list's `use` items declare.
    pub(super) fn of(items: &[Item]) -> Self {
        let mut imports = Self::default();
        for item in items {
            match item {
                Item::Use(declaration) => imports.add_tree(&declaration.tree, None),
                Item::Mod(module) => {
                    imports.modules.insert(identifier_name(&module.ident));
                }
                _ => {}
            }
        }
        imports
    }

    /// Whether the module may bind `name` to an import rather than the prelude.
    ///
    /// An explicit binding is exact. A glob and a `#[macro_use]` module are
    /// judged against the crate-wide name set, because neither can introduce a
    /// name the crate does not declare. An external glob or a `#[macro_use]`
    /// of another crate can supply a macro of any name, so either refuses every
    /// unresolved name in the crate.
    pub(super) fn may_bind(&self, name: &str, crate_scope: &CrateScope) -> bool {
        self.explicit.contains_key(name)
            || self.has_external_glob
            || crate_scope.has_external_macro_use
            || ((crate_scope.has_macro_use || self.has_internal_glob)
                && crate_scope.names.contains(name))
    }

    /// The names this module binds explicitly.
    pub(super) fn explicit_names(&self) -> impl Iterator<Item = &String> {
        self.explicit.keys()
    }

    /// Whether `root` names a module this item list itself declares.
    ///
    /// A `use inventory_provider as helper;` in this scope does not count: it
    /// binds an alias, and `crate::helper::…` then reaches whatever the alias
    /// names rather than a module of this crate.
    pub(super) fn declares_module(&self, root: &str) -> bool {
        self.modules.contains(root) && !self.explicit.contains_key(root)
    }

    /// Every explicit binding with the roots its paths are imported through.
    ///
    /// A binding whose root is `None` is a bare `use Name;`, so it resolves in
    /// the crate's own scope rather than through a named crate. One name can
    /// carry several roots, because two imports of it may compile when they
    /// occupy different namespaces.
    pub(super) const fn explicit_roots(&self) -> &BTreeMap<String, Vec<Option<String>>> {
        &self.explicit
    }

    /// Record one declaration, tracking the path a nested glob reaches through.
    fn add_tree(&mut self, tree: &UseTree, root: Option<&Ident>) {
        match tree {
            UseTree::Path(path) => self.add_tree(&path.tree, root.or(Some(&path.ident))),
            // `use crate::error_inject as r#compile_error;` binds the imported
            // macro to `compile_error`, so the binding is recorded under the name
            // Rust resolves rather than the spelling the source renders.
            UseTree::Name(name) => self.record_binding(identifier_name(&name.ident), root),
            UseTree::Rename(rename) => {
                self.record_binding(identifier_name(&rename.rename), root);
            }
            // The glob's root needs no normalization: only `crate`, `self` and
            // `super` are classified here, and Rust forbids a raw spelling of a
            // path keyword, so no other name can become one.
            UseTree::Glob(_) => match root.map(ToString::to_string).as_deref() {
                Some("crate" | "self" | "super") => self.has_internal_glob = true,
                _ => self.has_external_glob = true,
            },
            UseTree::Group(group) => {
                for item in &group.items {
                    self.add_tree(item, root);
                }
            }
        }
    }

    /// Add one root to a name's bindings without discarding an earlier one.
    ///
    /// A repeated identical root is recorded once; a distinct root is kept
    /// beside it, because the two imports may compile in different namespaces
    /// and only one of them decides what a derive of that name invokes.
    fn record_binding(&mut self, name: String, root: Option<&Ident>) {
        let root_name = root.map(identifier_name);
        let roots = self.explicit.entry(name).or_default();
        if !roots.contains(&root_name) {
            roots.push(root_name);
        }
    }
}

/// The crate-wide facts that decide what an unresolved name could be.
#[derive(Debug)]
pub(super) struct CrateScope {
    /// Every name the crate declares as a macro or an import binding.
    names: BTreeSet<String>,
    /// Every module the crate root itself declares, by its bare name.
    ///
    /// `crate::X` resolves in the crate root's scope, so only a module declared
    /// there makes the first segment local. A module of the same name nested in
    /// another module is out of scope, and a `use` alias bound at the crate
    /// root can redirect the name to a dependency; both must not count.
    root_modules: BTreeSet<String>,
    /// Every module any item list in the crate declares.
    ///
    /// A reviewed glob root is trusted by spelling, so a local `mod anstyle`
    /// anywhere in the crate has to void it: that module can re-export another
    /// crate's items under the reviewed name.
    declared_modules: BTreeSet<String>,
    /// Every name any `use` item in the crate binds.
    ///
    /// A reviewed external glob is trusted by its root spelling, so an alias
    /// that reuses that spelling has to be visible here: `use std::io as
    /// anstyle;` would otherwise borrow the reviewed name for a different
    /// crate's exports.
    imported_names: BTreeSet<String>,
    /// Whether any module is annotated `#[macro_use]`.
    has_macro_use: bool,
    /// Whether any `extern crate` is annotated `#[macro_use]`.
    has_external_macro_use: bool,
}

impl CrateScope {
    /// Build the scope from the crate-wide name and module sets.
    pub(super) const fn new(
        names: BTreeSet<String>,
        root_modules: BTreeSet<String>,
        declared_modules: BTreeSet<String>,
        imported_names: BTreeSet<String>,
    ) -> Self {
        Self {
            names,
            root_modules,
            declared_modules,
            imported_names,
            has_macro_use: false,
            has_external_macro_use: false,
        }
    }

    /// Whether any item list in the crate declares a module with this name.
    pub(super) fn declares_module_anywhere(&self, name: &str) -> bool {
        self.declared_modules.contains(name)
    }

    /// Whether the crate root declares a module with this bare name.
    pub(super) fn root_declares_module(&self, name: &str) -> bool {
        self.root_modules.contains(name)
    }

    /// Replace the root-module set with the crate root file's own top level.
    ///
    /// `crate::X` resolves in the crate root's own scope, so only a `mod X`
    /// written at that file's top level makes the segment local. The crate-wide
    /// walk collects nested modules too, which is what a name-only check would
    /// wrongly trust.
    pub(super) fn set_root_modules(&mut self, root_file: &syn::File) {
        self.root_modules = root_file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Mod(module) => Some(identifier_name(&module.ident)),
                _ => None,
            })
            .collect();
    }

    /// Whether any `use` item in the crate binds this name.
    pub(super) fn is_imported_somewhere(&self, name: &str) -> bool {
        self.imported_names.contains(name)
    }

    /// Record the `#[macro_use]` attributes one item list carries.
    ///
    /// Only a `mod` or an `extern crate` can meaningfully carry the attribute,
    /// and inline modules are walked because the attribute can sit anywhere in
    /// the tree. A file module's own attributes are in its declaring list, so
    /// every file contributes by being observed in turn.
    pub(super) fn observe(&mut self, items: &[Item]) {
        for item in items {
            match item {
                Item::Mod(module) => self.observe_module(module),
                Item::ExternCrate(declaration) => {
                    self.has_external_macro_use |= has_macro_use(&declaration.attrs);
                }
                _ => {}
            }
        }
    }

    /// Record one module's attribute and, for an inline module, its contents.
    fn observe_module(&mut self, module: &ItemMod) {
        self.has_macro_use |= has_macro_use(&module.attrs);
        if let Some((_, nested)) = &module.content {
            self.observe(nested);
        }
    }
}

/// Whether the attributes apply `#[macro_use]`, directly or through `cfg_attr`.
///
/// `#[cfg_attr(all(), macro_use)]` names `cfg_attr` on the item, so a check of
/// the immediate path would miss an import Rust really performs: `all()` holds,
/// the attribute is applied, and the helper's macros reach the modules below.
/// The shared attribute search expands the condition's consequence, which is
/// the same reason `cfg` and `path` are resolved that way.
fn has_macro_use(attributes: &[Attribute]) -> bool {
    applies_attribute(attributes, "macro_use")
}
