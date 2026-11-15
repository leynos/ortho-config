//! Temporary Cargo workspaces for compiling and running documented Rust.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;
use tempfile::TempDir;

use super::documentation_examples::{DocumentedExample, is_valid_example_id};
use crate::process_runner::{self, Operation};

#[path = "build_cache.rs"]
mod build_cache;
#[path = "build_state.rs"]
mod build_state;
#[path = "cargo_runner_test_support.rs"]
pub(super) mod cargo_runner;
#[path = "scenario.rs"]
mod scenario;

#[path = "workspace_layout.rs"]
mod layout;
use cargo_runner::cargo_dispatch_path;
use layout::{manifest, sanitize_application_environment};

/// A Cargo dependency alias used by the generated example package.
pub(super) struct DependencyAlias<'a>(pub(super) &'a str);

/// The identifier attached to a documented example.
pub(super) struct ExampleId<'a>(pub(super) &'a str);

/// One environment-variable override for a documented example process.
pub(super) struct EnvironmentVariable<'a> {
    pub(super) name: &'a str,
    pub(super) value: &'a str,
}

/// A fixture file relative to one documented example's run directory.
pub(super) struct RunFile<'a> {
    pub(super) path: &'a Path,
    pub(super) contents: &'a str,
}

/// An isolated package whose binary sources are exact Markdown fence bodies.
pub struct ExampleWorkspace {
    root: TempDir,
    directory: Dir,
    executables: BTreeMap<String, std::path::PathBuf>,
    deadline: Instant,
}

impl ExampleWorkspace {
    /// Create a package using `dependency_alias` for the local `OrthoConfig` crate.
    ///
    /// The returned workspace contains a generated manifest and an empty binary
    /// source directory.
    ///
    /// ```no_run
    /// let workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(dependency_alias: DependencyAlias<'_>) -> Result<Self> {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .context("find documentation repository")?;
        let workspaces = repository.join("target/documentation-workspaces");
        Dir::create_ambient_dir_all(&workspaces, ambient_authority())?;
        let root = tempfile::Builder::new()
            .prefix("example-")
            .tempdir_in(workspaces)
            .context("create documentation example workspace")?;
        let directory = Dir::open_ambient_dir(root.path(), ambient_authority())
            .context("open documentation example workspace")?;
        directory
            .create_dir_all("src/bin")
            .context("create bin directory")?;
        directory
            .write("Cargo.toml", manifest(dependency_alias))
            .context("write example manifest")?;
        layout::seed_lockfile(repository, &directory)?;
        Ok(Self {
            root,
            directory,
            executables: BTreeMap::new(),
            deadline: scenario::deadline(),
        })
    }

    /// Add an example as a binary without altering its published source.
    ///
    /// A successful result writes the exact fence body to `src/bin/<id>.rs`.
    ///
    /// ```no_run
    /// let workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    /// let example = documented_example("readme-main")?;
    /// workspace.add_binary(example)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn add_binary(&mut self, example: &DocumentedExample) -> Result<()> {
        self.add_binary_as(&example.id, example)
    }

    /// Add an example as a binary with a Cargo-dispatchable name.
    ///
    /// A successful result writes the exact fence body to
    /// `src/bin/<binary_name>.rs`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    /// # let example = documented_example("guide-cargo-external-subcommand")?;
    /// workspace.add_binary_as("cargo-demo", example)?;
    /// // The example body is now `src/bin/cargo-demo.rs`.
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the example is not Rust, the binary name is
    /// unsafe, or the fixture source cannot be written.
    pub fn add_binary_as(&mut self, binary_name: &str, example: &DocumentedExample) -> Result<()> {
        ensure!(example.language == "rust", "{} is not Rust", example.id);
        ensure!(
            is_valid_example_id(binary_name),
            "{binary_name} is not a safe documented binary name"
        );
        self.directory
            .write(format!("src/bin/{binary_name}.rs"), &example.body)
            .with_context(|| format!("write {binary_name} binary"))
    }

    /// Build every documented binary in the package.
    ///
    /// `Ok(())` means every added binary compiled successfully in Cargo's
    /// resolved shared target and was snapshotted into this workspace.
    ///
    /// ```no_run
    /// # let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    /// # workspace.add_binary(documented_example("readme-main")?)?;
    /// workspace.build()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn build(&mut self) -> Result<()> {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .context("find documentation repository")?;
        self.executables = build_cache::BuildCache::build_before(
            repository,
            build_cache::BuildInputs {
                package: self.root.path(),
                destination: &self.root.path().join("executables"),
                arguments: &["--bins"],
            },
            self.deadline,
        )?;
        Ok(())
    }

    /// Run a built example in its own deterministic working directory.
    ///
    /// The returned [`std::process::Output`] contains the binary's exit status
    /// and captured standard streams.
    ///
    /// ```no_run
    /// # let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    /// let output = workspace.run(ExampleId("readme-main"), ["--port", "3000"])?;
    /// assert!(output.status.success());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn run<I, S>(&mut self, ExampleId(id): ExampleId<'_>, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_with_environment(
            ExampleId(id),
            args,
            std::iter::empty::<EnvironmentVariable<'_>>(),
        )
    }

    /// Run a built example with explicit environment-variable overrides.
    ///
    /// Overrides are visible to the child alongside the deterministic home
    /// directories; unrelated host variables remain absent.
    ///
    /// ```no_run
    /// # let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    /// let output = workspace.run_with_environment(
    ///     ExampleId("guide-first-cli"),
    ///     ["--port", "3000"],
    ///     [EnvironmentVariable { name: "ACME_HOST", value: "api.internal" }],
    /// )?;
    /// assert!(output.status.success());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn run_with_environment<'a, I, S, E>(
        &mut self,
        ExampleId(id): ExampleId<'_>,
        args: I,
        environment: E,
    ) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
        E: IntoIterator<Item = EnvironmentVariable<'a>>,
    {
        ensure!(
            is_valid_example_id(id),
            "{id} is not a safe documented example identifier"
        );
        let run_dir_name = format!("run-{id}");
        self.directory
            .create_dir_all(&run_dir_name)
            .with_context(|| format!("create working directory for {id}"))?;
        let run_dir = self.root.path().join(&run_dir_name);
        let binary = self
            .executables
            .get(id)
            .with_context(|| format!("no Cargo executable snapshot for {id}"))?;
        let mut command = Command::new(binary);
        sanitize_application_environment(&mut command);
        command
            .args(args)
            .current_dir(&run_dir)
            .env("HOME", &run_dir)
            .env("XDG_CONFIG_HOME", run_dir.join("xdg"))
            .envs(
                environment
                    .into_iter()
                    .map(|EnvironmentVariable { name, value }| (name, value)),
            );
        let operation = format!("run documented binary {id}");
        process_runner::run_with_policy(
            &mut command,
            Operation(&operation),
            crate::process_runner_execution_policy::policy()
                .bounded_by(scenario::remaining(self.deadline)?),
        )
    }

    /// Run a built `cargo-<subcommand>` binary through Cargo dispatch.
    ///
    /// The workspace's private executable directory is prepended to the child
    /// `PATH`, so Cargo discovers only the fixture binary added to this
    /// workspace before any host installation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    /// # let example = documented_example("guide-cargo-external-subcommand")?;
    /// # workspace.add_binary_as("cargo-demo", example)?;
    /// # workspace.build()?;
    /// let output = workspace.run_cargo_subcommand("demo", ["--verbose"])?;
    /// assert!(output.status.success());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the subcommand name is unsafe, the dispatch
    /// path cannot be constructed, or Cargo cannot run the fixture.
    pub fn run_cargo_subcommand<I, S>(&mut self, subcommand: &str, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        ensure!(
            is_valid_example_id(subcommand),
            "{subcommand} is not a safe Cargo subcommand name"
        );
        let fixture_bin_dir = self.root.path().join("executables");
        let inherited_path = std::env::var_os("PATH");
        let path = cargo_dispatch_path(&fixture_bin_dir, inherited_path.as_deref())?;
        let operation = format!("run Cargo-dispatched documented binary {subcommand}");
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .context("find documentation repository")?;
        build_cache::BuildCache::with_owned_state(
            repository,
            self.deadline,
            |cache, target, remaining| {
                let mut command = cargo_runner::prepare_cargo_command_for_target_before(
                    self.root.path(),
                    cache,
                    target,
                    self.deadline,
                )?;
                command.env("PATH", path).arg(subcommand).args(args);
                process_runner::run_with_policy(
                    &mut command,
                    Operation(&operation),
                    crate::process_runner_execution_policy::policy()
                        .bounded_by(scenario::remaining(self.deadline)?.min(remaining)),
                )
            },
        )
    }

    /// Write a file in one binary's deterministic working directory.
    ///
    /// A successful result creates the run directory when necessary and writes
    /// the requested relative file within it.
    ///
    /// ```no_run
    /// # let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    /// workspace.write_run_file(
    ///     ExampleId("guide-first-cli"),
    ///     RunFile { path: Path::new(".acme.toml"), contents: "port = 3000\n" },
    /// )?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn write_run_file(
        &mut self,
        ExampleId(id): ExampleId<'_>,
        RunFile { path, contents }: RunFile<'_>,
    ) -> Result<()> {
        ensure!(
            is_valid_example_id(id),
            "{id} is not a safe documented example identifier"
        );
        ensure!(
            !path.as_os_str().is_empty()
                && path
                    .components()
                    .all(|component| matches!(component, Component::Normal(_))),
            "run file path must stay within the example directory"
        );
        let run_dir_name = format!("run-{id}");
        self.directory.create_dir_all(&run_dir_name)?;
        let run_dir = self.directory.open_dir(&run_dir_name)?;
        if let Some(parent) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            run_dir
                .create_dir_all(parent)
                .with_context(|| format!("create fixture directory {}", parent.display()))?;
        }
        run_dir.write(path, contents)?;
        Ok(())
    }

    /// Resolve a relative path within this workspace.
    ///
    /// Tests use this when a documented application expects a path in its
    /// environment rather than opening a run-directory fixture directly.
    ///
    /// ```no_run
    /// # let workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    /// let config = workspace.path_in_root("run-example/config.toml")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when `relative` contains a root or parent component.
    pub fn path_in_root(&self, relative: &str) -> Result<PathBuf> {
        let relative_path = Path::new(relative);
        ensure!(
            !relative_path.as_os_str().is_empty()
                && relative_path
                    .components()
                    .all(|component| matches!(component, Component::Normal(_))),
            "workspace-relative path must stay within the workspace root"
        );
        Ok(self.root.path().join(relative_path))
    }
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
