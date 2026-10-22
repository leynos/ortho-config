//! Reading a rendered or packaged manifest as data rather than as text.
//!
//! Both halves of the dependency-closure claim need to know what a manifest
//! *declares*, and neither can answer that by searching the file. A crate name
//! in a comment, in Cargo's normalized header, or in a feature table is not a
//! declared edge, and a text search cannot tell the difference. Parsing the
//! document into TOML also reaches the platform-conditional tables, which is
//! what lets a Linux runner assert something true of a Windows build.
//!
//! # Every kind, every platform, and the package identity
//!
//! The walk is deliberately blind to three things a weaker reader would
//! filter on:
//!
//! - **Kind.** A `[dev-dependencies]` or `[build-dependencies]` edge is still
//!   an edge Cargo resolves for the crate's own tests and build script, so a
//!   check that kept only normal edges would report nothing while a forbidden
//!   crate sat in the same manifest.
//! - **Platform.** A `cfg(windows)` table is not compiled on Linux, so a
//!   host-narrowed resolve *omits* it entirely - measured, not assumed: Cargo
//!   reports such an edge in `packages[].dependencies` and in no resolve node.
//!   Manifest text is the only place where every platform is visible at once.
//! - **The key.** `alias = { package = "real-name" }` declares the *real*
//!   name; the key is local spelling. Comparing keys against a forbidden list
//!   would miss a renamed dependency, and a rename is the cheapest way to
//!   bring a crate back.
//!
//! This module owns that reading and nothing else. It does not resolve a graph
//! ([`super::FixtureCargo`] does) and it does not judge one
//! ([`super::super::closure`] does); it answers "what edges does this document
//! name, and under which table".

use super::metadata::{DependencyEdge, EdgeKind};
use anyhow::{Context, Result};
use std::collections::BTreeSet;

/// Every dependency edge a manifest *declares*, as its tables name them.
///
/// Each edge is named by the package it will resolve to, not by the key it is
/// written under, so a renamed dependency is reported under the identity a
/// forbidden list can match. Target-conditional tables are walked too, which
/// is the only way a single-platform run can see another platform's edge.
///
/// The kind is the table the edge sits under, which is what distinguishes a
/// published edge from a test-only one. A dependency declared in more than one
/// table is reported once per table, matching how it was declared.
///
/// # Errors
///
/// Returns an error when `manifest` is not a TOML document.
pub(crate) fn declared_dependencies(manifest: &str) -> Result<Vec<DependencyEdge>> {
    let declared = parse(manifest)?;
    Ok(declarations(&declared)
        .iter()
        .map(Declaration::edge)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

/// Every dependency a manifest declares by `path`, as package identities.
///
/// Cargo rewrites such an edge into a version requirement when it packages, so
/// a surviving `path` in a *packaged* manifest means the measured root still
/// points into this worktree. The check reads dependency *entries* rather than
/// text, because `[lib] path` and `[[test]] path` name files inside the package
/// and are expected to survive.
///
/// # Errors
///
/// Returns an error when `manifest` is not a TOML document.
pub(crate) fn declared_path_dependencies(manifest: &str) -> Result<Vec<String>> {
    let document = parse(manifest)?;
    Ok(declarations(&document)
        .into_iter()
        .filter(|declaration| declaration.entry.get("path").is_some())
        .map(|declaration| declaration.package().to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

/// Parse `manifest` into the tables the walker reads.
///
/// # Errors
///
/// Returns an error when `manifest` is not a TOML document.
fn parse(manifest: &str) -> Result<toml::Table> {
    toml::from_str(manifest).context("parse a manifest as a TOML document")
}

/// The manifest tables whose keys are dependency names, with their kinds.
const DEPENDENCY_TABLES: [(&str, EdgeKind); 3] = [
    ("dependencies", EdgeKind::Normal),
    ("dev-dependencies", EdgeKind::Dev),
    ("build-dependencies", EdgeKind::Build),
];

/// One declared dependency, as the table that carries it names it.
///
/// The entry value is kept rather than only its key, because both facts the
/// closure claim needs live there: the `package` field names the real
/// dependency, and the `path` field says whether the edge survives packaging.
struct Declaration<'a> {
    /// The key the dependency is declared under, which is local spelling.
    name: &'a str,
    /// The dependency's value: a version string or a detail table.
    entry: &'a toml::Value,
    /// The kind implied by the table the entry sits under.
    kind: EdgeKind,
}

impl<'a> Declaration<'a> {
    /// The package identity this entry resolves to.
    ///
    /// `package` wins when present: `alias = { package = "clap" }` is a
    /// dependency on `clap`, and the alias is not a package any registry knows.
    fn package(&self) -> &'a str {
        self.entry
            .get("package")
            .and_then(toml::Value::as_str)
            .unwrap_or(self.name)
    }

    /// This declaration as a comparable edge.
    fn edge(&self) -> DependencyEdge {
        DependencyEdge {
            package: self.package().to_owned(),
            kind: self.kind,
        }
    }
}

/// Every dependency entry `document` declares, target tables included.
fn declarations(document: &toml::Table) -> Vec<Declaration<'_>> {
    let mut found = Vec::new();
    collect_declarations(document, &mut found);
    found
}

/// Walk `document`, collecting dependency entries from it and its targets.
fn collect_declarations<'a>(document: &'a toml::Table, found: &mut Vec<Declaration<'a>>) {
    for (heading, kind) in DEPENDENCY_TABLES {
        for (name, entry) in dependency_entries(document, heading) {
            found.push(Declaration { name, entry, kind });
        }
    }
    for section in target_tables(document) {
        collect_declarations(section, found);
    }
}

/// The entries of one dependency table, or nothing when it is not declared.
fn dependency_entries<'a>(
    table: &'a toml::Table,
    heading: &str,
) -> impl Iterator<Item = (&'a String, &'a toml::Value)> {
    table
        .get(heading)
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::iter)
}

/// The platform-conditional tables a manifest nests its edges in.
fn target_tables(table: &toml::Table) -> impl Iterator<Item = &toml::Table> {
    table
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::values)
        .filter_map(toml::Value::as_table)
}

#[cfg(test)]
mod tests {
    //! Negative controls for the three ways a manifest check goes vacuous.
    //!
    //! Each case is the mutation that the previous version of this reader
    //! failed to catch, written as text and asserted through the same function
    //! the packaged-manifest contract calls. A reader that regressed to keys,
    //! or to normal edges, or to the top-level tables, fails here without a
    //! Cargo subprocess - so the evidence does not depend on the shared package
    //! cache holding the right crates.

    use super::{declared_dependencies, declared_path_dependencies};
    use crate::boundary::closure::FORBIDDEN_PACKAGES;
    use crate::boundary::fixture_cargo::{DependencyEdge, EdgeKind};
    use anyhow::{Result, ensure};

    /// A manifest header every fixture below shares.
    const HEADER: &str = concat!(
        "[package]\n",
        "name = \"sample\"\n",
        "version = \"0.0.0\"\n",
        "edition = \"2024\"\n",
    );

    /// Assert that `manifest` declares `expected` as an edge.
    ///
    /// The expectation is borrowed rather than taken by value: the helper only
    /// compares against it, and a `Result`-returning test helper that panicked
    /// on mismatch would hide the graph it was given instead of reporting it.
    fn assert_declares(manifest: &str, expected: &DependencyEdge) -> Result<()> {
        let declared = declared_dependencies(manifest)?;
        ensure!(
            declared.contains(expected),
            "the reader reported {declared:?}, which does not contain {expected}",
        );
        Ok(())
    }

    #[rstest::rstest]
    #[case::dev("[dev-dependencies]", EdgeKind::Dev)]
    #[case::build("[build-dependencies]", EdgeKind::Build)]
    #[case::normal("[dependencies]", EdgeKind::Normal)]
    fn a_clap_edge_is_a_declared_edge(#[case] heading: &str, #[case] kind: EdgeKind) -> Result<()> {
        let manifest = format!("{HEADER}\n{heading}\nclap = \"4\"\n");
        assert_declares(
            &manifest,
            &DependencyEdge {
                package: String::from("clap"),
                kind,
            },
        )
    }

    /// The target table is where a host-narrowed resolve cannot look, so this
    /// case is a different question from the three above rather than a fourth
    /// instance of it.
    #[test]
    fn a_target_specific_dev_dependency_is_a_declared_edge() -> Result<()> {
        let manifest = concat!(
            "[package]\n",
            "name = \"sample\"\n",
            "version = \"0.0.0\"\n",
            "edition = \"2024\"\n",
            "\n",
            "[target.'cfg(target_os = \"none\")'.dev-dependencies]\n",
            "clap = \"4\"\n",
        );
        assert_declares(
            manifest,
            &DependencyEdge {
                package: String::from("clap"),
                kind: EdgeKind::Dev,
            },
        )
    }

    /// The alias is the local spelling; the `package` field is the identity a
    /// forbidden list matches. This is the case a key-reading walker fails.
    #[test]
    fn a_renamed_dependency_is_declared_under_its_package_name() -> Result<()> {
        let manifest = format!(
            "{HEADER}\n[dependencies]\nlocal_alias = {{ package = \"clap\", version = \"4\" }}\n"
        );
        assert_declares(
            &manifest,
            &DependencyEdge {
                package: String::from("clap"),
                kind: EdgeKind::Normal,
            },
        )
    }

    /// The fixture above must be one the forbidden list can match, or the
    /// assertions prove only that an arbitrary name is read.
    #[test]
    fn the_controls_use_a_forbidden_name() {
        assert!(FORBIDDEN_PACKAGES.contains(&"clap"));
    }

    #[test]
    fn a_path_dependency_is_reported_by_package_name() -> Result<()> {
        let manifest = concat!(
            "[package]\n",
            "name = \"sample\"\n",
            "version = \"0.0.0\"\n",
            "edition = \"2024\"\n",
            "\n",
            "[dev-dependencies]\n",
            "helpers = { package = \"ortho_config_test_helpers\", path = \"../test_helpers\" }\n",
        );
        ensure!(
            declared_path_dependencies(manifest)?
                == vec![String::from("ortho_config_test_helpers")],
            "the path dependency was not reported under its package name",
        );
        Ok(())
    }

    /// A file path inside the package is not a dependency edge.
    #[test]
    fn library_and_test_paths_are_not_dependencies() -> Result<()> {
        let manifest = concat!(
            "[package]\n",
            "name = \"sample\"\n",
            "version = \"0.0.0\"\n",
            "edition = \"2024\"\n",
            "\n",
            "[lib]\n",
            "path = \"src/lib.rs\"\n",
            "\n",
            "[[test]]\n",
            "name = \"boundary\"\n",
            "path = \"tests/boundary.rs\"\n",
        );
        ensure!(
            declared_path_dependencies(manifest)?.is_empty(),
            "a package's own source paths were reported as dependencies",
        );
        Ok(())
    }
}
