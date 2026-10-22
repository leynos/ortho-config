//! Reading a rendered or packaged manifest as data rather than as text.
//!
//! Both halves of the dependency-closure claim need to know what a manifest
//! *declares*, and neither can answer that by searching the file. A crate name
//! in a comment, in Cargo's normalized header, or in a feature table is not a
//! declared edge, and a text search cannot tell the difference. Parsing the
//! document into TOML also reaches the platform-conditional tables, which is
//! what lets a Linux runner assert something true of a Windows build.
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
/// This reads the packaged manifest as data rather than as text, so a crate
/// that a comment or a feature table merely mentions is not mistaken for a
/// declared edge. Target-conditional tables are walked too: an edge declared
/// for one platform is still an edge a stranger's platform may resolve, so a
/// Linux runner still sees a `cfg(windows)` table and a Windows runner still
/// sees a `cfg(unix)` one.
///
/// The kind is the table the edge sits under, which is what distinguishes a
/// published edge from a test-only one. A dependency that Cargo inherited into
/// a platform table is reported once per table, matching how it was declared.
///
/// # Errors
///
/// Returns an error when `manifest` is not a TOML document.
pub(crate) fn declared_dependencies(manifest: &str) -> Result<Vec<DependencyEdge>> {
    let document: toml::Table =
        toml::from_str(manifest).context("parse a packaged manifest as a TOML document")?;
    let mut edges = BTreeSet::new();
    collect_declared_dependencies(&document, &mut edges);
    Ok(edges
        .into_iter()
        .map(|(package, kind)| DependencyEdge { package, kind })
        .collect())
}

/// The manifest tables whose keys are dependency names, with their kinds.
const DEPENDENCY_TABLES: [(&str, EdgeKind); 3] = [
    ("dependencies", EdgeKind::Normal),
    ("dev-dependencies", EdgeKind::Dev),
    ("build-dependencies", EdgeKind::Build),
];

/// The keys of one dependency table, or nothing when it is not declared.
fn dependency_names<'a>(table: &'a toml::Table, heading: &str) -> impl Iterator<Item = &'a String> {
    table
        .get(heading)
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::keys)
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

/// Walk `table`, collecting declared edges from it and its target sections.
fn collect_declared_dependencies(table: &toml::Table, edges: &mut BTreeSet<(String, EdgeKind)>) {
    for (heading, kind) in DEPENDENCY_TABLES {
        for name in dependency_names(table, heading) {
            edges.insert((name.clone(), kind));
        }
    }
    for section in target_tables(table) {
        collect_declared_dependencies(section, edges);
    }
}
