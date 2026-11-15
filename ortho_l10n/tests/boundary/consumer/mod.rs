//! Renders the consumer roots the boundary contracts resolve, build, and test.
//!
//! Three shapes are needed:
//!
//! - a *runtime* consumer of `ortho_l10n` alone, which is the graph issue #566
//!   exists to make possible;
//! - a *facade* consumer of `ortho_config` alone, which must keep working
//!   unchanged for existing applications;
//! - a *mixed* consumer importing both, which is the only shape that can show
//!   the two import paths name the same nominal types.
//!
//! Each shape can resolve from a *source* workspace (path dependencies, as an
//! unpublished dependent sees it) or from *packaged* sources (`cargo package`
//! output reached through `[patch.crates-io]`, as a stranger sees it).
//!
//! Every manifest closes with its own `[workspace]` table. Without it Cargo
//! walks up from `target/` into the repository workspace, and the fixture stops
//! being a stranger: feature unification, the lockfile, and the patch table
//! would all come from this repository instead.

mod library;

use super::fixture_cargo::{absolute_member, manifest_text, repository_root, toml_string};
use super::package_set::PACKAGED_VERSION;
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;

/// The package name every rendered consumer root uses.
pub(crate) const CONSUMER_PACKAGE: &str = "boundary-consumer";

/// Which crates a consumer root depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dependency {
    /// The standalone runtime crate alone.
    Runtime,
    /// The configuration facade alone, which re-exports the runtime.
    Configuration,
    /// Both crates, to compare their import paths.
    Mixed,
}

impl Dependency {
    /// Every shape, so a new variant cannot be forgotten.
    pub(crate) const ALL: [Self; 3] = [Self::Runtime, Self::Configuration, Self::Mixed];

    /// Spelling for test names and failure messages.
    pub(crate) const fn describe(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Configuration => "configuration facade",
            Self::Mixed => "mixed",
        }
    }

    /// The internal crates this shape spells in `[dependencies]`.
    ///
    /// A table rather than a predicate per crate: the two writers below differ
    /// only in how a crate is spelled, so both iterate this list. It also keeps
    /// each writer free of the clustered conditionals the `bumpy_road_function`
    /// lint rejects, and a fourth shape adds a row rather than a branch.
    const fn crates(self) -> &'static [&'static str] {
        match self {
            Self::Runtime => &["ortho_l10n"],
            Self::Configuration => &["ortho_config"],
            Self::Mixed => &["ortho_l10n", "ortho_config"],
        }
    }

    /// The unpacked roots this shape's dependency graph reaches.
    ///
    /// A packaged root declares its own internal edges by version, so the patch
    /// table has to name them even when this shape does not. The runtime stands
    /// alone; the facade reaches the runtime and the derive macros, which in
    /// turn reach the runtime. Naming only the direct dependency would leave
    /// the transitive edge resolving from the registry, where no version of
    /// these crates exists yet.
    const fn patched_crates(self) -> &'static [&'static str] {
        match self {
            Self::Runtime => &["ortho_l10n"],
            Self::Configuration | Self::Mixed => {
                &["ortho_l10n", "ortho_config", "ortho_config_macros"]
            }
        }
    }
}

/// Which source a consumer root resolves its dependencies from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DependencySource {
    /// Path dependencies into this workspace.
    Workspace,
    /// A published version, patched to the unpacked package roots.
    Packaged {
        /// Directory `cargo package` unpacked the crates into.
        unpacked: camino::Utf8PathBuf,
    },
}

impl DependencySource {
    /// Spelling for test names.
    pub(crate) const fn describe(&self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Packaged { .. } => "packaged",
        }
    }
}

/// One rendered consumer root: its manifest and library body.
pub(crate) struct Consumer {
    /// The manifest, ready to be written as `Cargo.toml`.
    pub(crate) manifest: String,
    /// The library body, ready to be written as `src/lib.rs`.
    pub(crate) library: String,
}

impl Consumer {
    /// Render a consumer root for `dependency` resolved from `source`.
    ///
    /// # Errors
    ///
    /// Returns an error when the repository root cannot be located or a
    /// packaged source is not an unpacked crate root.
    pub(crate) fn render(dependency: Dependency, source: &DependencySource) -> Result<Self> {
        let manifest = manifest_for(dependency, source)?;
        Ok(Self {
            manifest,
            library: library::body(dependency),
        })
    }
}

/// Render the whole manifest.
fn manifest_for(dependency: Dependency, source: &DependencySource) -> Result<String> {
    let dependencies = match source {
        DependencySource::Workspace => workspace_dependencies(dependency)?,
        DependencySource::Packaged { unpacked } => packaged_dependencies(dependency, unpacked)?,
    };
    Ok(manifest_header() + &dependencies)
}

/// The package, workspace and `[dependencies]` heading every consumer starts with.
fn manifest_header() -> String {
    manifest_text([
        "[package]".to_owned(),
        format!("name = \"{CONSUMER_PACKAGE}\""),
        "version = \"0.0.0\"".to_owned(),
        "edition = \"2024\"".to_owned(),
        String::new(),
        "# A consumer root is a workspace of its own.".to_owned(),
        "[workspace]".to_owned(),
        String::new(),
        "[dependencies]".to_owned(),
    ])
}

/// Spell the required crates as path dependencies into this workspace.
///
/// The paths are absolute. A relative spelling would be resolved against the
/// fixture root under `target/`, which is not inside the repository tree, so
/// `ortho_l10n` would be looked for beside the fixture instead of in the
/// worktree. Absolute paths also keep the two source shapes symmetric: the
/// packaged root is patched at an absolute unpacked path for the same reason.
fn workspace_dependencies(dependency: Dependency) -> Result<String> {
    let repository = repository_root()?;
    let mut lines = Vec::new();
    for crate_name in dependency.crates() {
        let member = absolute_member(&repository, Utf8Path::new(crate_name))?;
        lines.push(format!(
            "{crate_name} = {{ path = {} }}",
            toml_string(&member)
        ));
    }
    Ok(manifest_text(lines))
}

/// Spell the required crates by version, patched to unpacked package roots.
///
/// The patch table covers the transitive internal edges as well as the ones
/// this shape names: a packaged facade declares its runtime and derive-macro
/// edges by version, and those have to resolve to the unpacked roots too or the
/// consumer would be resolving them from the registry. Only the crates this
/// shape actually reaches are patched, so a runtime-only consumer does not
/// silently acquire a derive-macro root it never asked for.
fn packaged_dependencies(dependency: Dependency, unpacked: &Utf8Path) -> Result<String> {
    let mut lines: Vec<String> = dependency
        .crates()
        .iter()
        .map(|crate_name| format!("{crate_name} = \"{PACKAGED_VERSION}\""))
        .collect();
    lines.push(String::new());
    lines
        .push("# Point every same-version internal edge at the unpacked package roots.".to_owned());
    lines.push("[patch.crates-io]".to_owned());
    for crate_name in dependency.patched_crates() {
        lines.push(patch_entry(unpacked, crate_name)?);
    }
    Ok(manifest_text(lines))
}

/// One `[patch.crates-io]` entry pointing at an unpacked package root.
fn patch_entry(unpacked: &Utf8Path, crate_name: &str) -> Result<String> {
    let crate_root = unpacked.join(format!("{crate_name}-{PACKAGED_VERSION}"));
    let absolute = crate_root
        .canonicalize_utf8()
        .with_context(|| format!("canonicalize packaged root {crate_root}"))?;
    ensure!(
        absolute.join("Cargo.toml").is_file(),
        "{absolute} is not an unpacked package root",
    );
    Ok(format!(
        "{crate_name} = {{ path = {} }}",
        toml_string(absolute.as_str()),
    ))
}
