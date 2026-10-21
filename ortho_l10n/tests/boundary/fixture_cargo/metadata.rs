//! The parts of Cargo's `cargo metadata` document the contracts read.
//!
//! Kept in its own module because the document's shape is an external tool
//! contract: Cargo may add fields, and a field this suite silently ignored is
//! one whose meaning could change without any contract noticing. Everything
//! here is a deliberate reading of the format, small enough to audit against
//! `cargo metadata --format-version=1` output in one sitting.
//!
//! Two spellings of a dependency name appear in the document. A resolve node's
//! `deps[].name` is the name the *source* uses, with hyphens folded to
//! underscores; a package's `name` is the crate's identity on the registry.
//! Every edge here is named by identity, taken from the `pkg` id, so a
//! comparison against a package name cannot fail on a spelling difference that
//! means nothing.
//!
//! Edge kinds are likewise named rather than compared as strings. Cargo spells
//! a normal edge as `null` and the other two as `"build"` and `"dev"`; a future
//! vocabulary addition would silently read as "not build, not dev" in a string
//! comparison, so [`EdgeKind::from_cargo`] refuses it instead.

use anyhow::{Context, Result, bail};

/// Part of Cargo's `cargo metadata --format-version=1` document.
///
/// Fields the contracts do not read are deliberately absent; `serde` ignores
/// them, which keeps this description small enough to audit.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct Metadata {
    /// Every package in the resolved graph.
    pub(crate) packages: Vec<MetadataPackage>,
    /// Resolution graph: one node per package, with classified dependency edges.
    pub(crate) resolve: Resolve,
}

impl Metadata {
    /// Parse a `cargo metadata` document from standard output.
    ///
    /// # Errors
    ///
    /// Returns an error when `stdout` is not the JSON document Cargo emits.
    pub(crate) fn parse(stdout: &str) -> Result<Self> {
        serde_json::from_str(stdout).context("parse cargo metadata document")
    }

    /// Return the names of every package in the resolved graph.
    pub(crate) fn package_names(&self) -> Vec<&str> {
        self.packages
            .iter()
            .map(|package| package.name.as_str())
            .collect()
    }

    /// Look up one package by name.
    ///
    /// # Errors
    ///
    /// Returns an error when the package is absent from the graph.
    pub(crate) fn package(&self, name: &str) -> Result<&MetadataPackage> {
        self.packages
            .iter()
            .find(|package| package.name == name)
            .with_context(|| format!("the resolved graph has no package named {name}"))
    }

    /// The id of the package called `name`.
    ///
    /// # Errors
    ///
    /// Returns an error when the graph holds no package by that name.
    fn id_of(&self, name: &str) -> Result<&str> {
        self.packages
            .iter()
            .find(|package| package.name == name)
            .map(|package| package.id.as_str())
            .with_context(|| format!("the resolved graph has no package named {name}"))
    }

    /// Describe the dependency edges of the package with `name`.
    ///
    /// Edge targets are named by looking their `pkg` id up among the
    /// [`MetadataPackage`] entries. The id itself is not a reliable source of
    /// the name: a registry id ends `#<name>@<version>`, but a path id ends
    /// `#<version>` with no name at all, so a parser that read the fragment
    /// would work for crates.io and fail for exactly the packaged roots these
    /// contracts resolve.
    ///
    /// # Errors
    ///
    /// Returns an error when the package has no resolve node, when an edge
    /// names an id the package list does not contain, or when Cargo reports an
    /// edge kind this suite does not recognize.
    pub(crate) fn edges(&self, name: &str) -> Result<Vec<DependencyEdge>> {
        let id = self.id_of(name)?;
        let node = self
            .resolve
            .nodes
            .iter()
            .find(|node| node.id == id)
            .with_context(|| format!("resolve graph has no node for {name}"))?;
        let mut edges = Vec::new();
        for dependency in &node.deps {
            let package = self.name_of_id(&dependency.pkg)?;
            for kind in &dependency.dep_kinds {
                edges.push(DependencyEdge {
                    package: package.to_owned(),
                    kind: EdgeKind::from_cargo(kind.kind.as_deref())?,
                });
            }
        }
        Ok(edges)
    }

    /// The name of the package whose id is `id`.
    ///
    /// # Errors
    ///
    /// Returns an error when no package carries that id.
    fn name_of_id(&self, id: &str) -> Result<&str> {
        self.packages
            .iter()
            .find(|package| package.id == id)
            .map(|package| package.name.as_str())
            .with_context(|| format!("the resolved graph has no package with id {id:?}"))
    }
}

/// One package entry from `cargo metadata`.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct MetadataPackage {
    /// Package id, of the form `source#<name>@<version>` for a registry
    /// package and `source#<version>` for a path package.
    pub(crate) id: String,
    /// Package name.
    pub(crate) name: String,
    /// Package version.
    pub(crate) version: String,
    /// Source identifier; `None` for a path package.
    pub(crate) source: Option<String>,
}

/// The `resolve` object from `cargo metadata`.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct Resolve {
    /// One node per resolved package.
    pub(crate) nodes: Vec<ResolveNode>,
}

/// One package's classified dependency edges.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct ResolveNode {
    /// Package id, of the form `source#name@version`.
    pub(crate) id: String,
    /// Dependencies with their edge classifications.
    pub(crate) deps: Vec<ResolveDependency>,
}

/// One dependency entry on a resolve node.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct ResolveDependency {
    /// Package id of the dependency, from which the identity is read.
    pub(crate) pkg: String,
    /// Edge classifications; one entry per kind the manifest declares.
    pub(crate) dep_kinds: Vec<DepKind>,
}

/// One edge classification, as Cargo reports it.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct DepKind {
    /// `null` for a normal edge, `"build"`, or `"dev"`.
    pub(crate) kind: Option<String>,
}

/// A dependency edge with its kind rendered for assertions and messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyEdge {
    /// Dependency package name, as the registry spells it.
    pub(crate) package: String,
    /// `normal`, `build`, or `dev`.
    pub(crate) kind: EdgeKind,
}

/// The three edge kinds Cargo classifies, named rather than spelled as `null`.
///
/// The variants are ordered normal, build, dev so that a sorted edge set is
/// stable and reads in dependency-strength order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum EdgeKind {
    /// A normal runtime dependency.
    Normal,
    /// A build-script dependency.
    Build,
    /// A test-only dependency of the root package.
    Dev,
}

impl EdgeKind {
    /// Map Cargo's `dep_kinds[].kind` spelling onto an edge kind.
    ///
    /// # Errors
    ///
    /// Returns an error for a kind outside Cargo's closed set, so a future
    /// Cargo vocabulary change fails loudly instead of being misclassified as
    /// a normal edge.
    fn from_cargo(kind: Option<&str>) -> Result<Self> {
        match kind {
            None => Ok(Self::Normal),
            Some("build") => Ok(Self::Build),
            Some("dev") => Ok(Self::Dev),
            Some(other) => bail!("unrecognized Cargo dependency edge kind {other:?}"),
        }
    }
}

impl std::fmt::Display for DependencyEdge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} ({})", self.package, self.kind)
    }
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Normal => "normal",
            Self::Build => "build",
            Self::Dev => "dev",
        };
        formatter.write_str(name)
    }
}
