//! Temporary Cargo workspaces for compiling and running documented Rust.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::ffi::OsStr;
use std::path::{Component, Path};
use std::process::{Command, Output};
use tempfile::TempDir;

use super::documentation_examples::{DocumentedExample, is_valid_example_id};
use crate::process_runner::{self, Operation};

pub(super) mod cargo_runner;

const CHILD_ENV_ALLOWLIST: &[&str] = &["SYSTEMROOT", "WINDIR"];

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
        let root = tempfile::tempdir().context("create documentation example workspace")?;
        let directory = Dir::open_ambient_dir(root.path(), ambient_authority())
            .context("open documentation example workspace")?;
        directory
            .create_dir_all("src/bin")
            .context("create bin directory")?;
        directory
            .write("Cargo.toml", manifest(dependency_alias))
            .context("write example manifest")?;
        Ok(Self { root, directory })
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
        ensure!(example.language == "rust", "{} is not Rust", example.id);
        ensure!(
            is_valid_example_id(&example.id),
            "{} is not a safe documented example identifier",
            example.id
        );
        self.directory
            .write(format!("src/bin/{}.rs", example.id), &example.body)
            .with_context(|| format!("write {} binary", example.id))
    }

    /// Build every documented binary in the package.
    ///
    /// `Ok(())` means every added binary compiled successfully in the isolated
    /// target directory.
    ///
    /// ```no_run
    /// # let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
    /// # workspace.add_binary(documented_example("readme-main")?)?;
    /// workspace.build()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn build(&mut self) -> Result<()> {
        let mut command = self.cargo_command()?;
        command.args(["build", "--offline", "--bins"]);
        let output = process_runner::run_command(&mut command, Operation("build documented Rust"))?;
        ensure!(
            output.status.success(),
            "documented Rust failed to compile:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
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
            .root
            .path()
            .join("target/debug")
            .join(format!("{id}{}", std::env::consts::EXE_SUFFIX));
        let mut command = Command::new(binary);
        cargo_runner::sanitize_environment(&mut command, CHILD_ENV_ALLOWLIST);
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
        process_runner::run_command(&mut command, Operation(&operation))
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
        run_dir.write(path, contents)?;
        Ok(())
    }

    fn cargo_command(&mut self) -> Result<Command> {
        cargo_runner::prepare_cargo_command(self.root.path(), self.root.path())
    }
}

fn manifest(DependencyAlias(dependency_name): DependencyAlias<'_>) -> String {
    let crate_path = toml::Value::String(env!("CARGO_MANIFEST_DIR").to_owned()).to_string();
    render_manifest(dependency_name, &crate_path)
}

/// Render the generated manifest from a serialized Cargo dependency path.
///
/// This stays private to the documentation workspace and its path regression
/// test; callers must serialize the path as a TOML value before using it.
fn render_manifest(dependency_name: &str, serialized_crate_path: &str) -> String {
    format!(
        concat!(
            "[package]\n",
            "name = \"documented-examples\"\n",
            "version = \"0.0.0\"\n",
            "edition = \"2024\"\n\n",
            "[dependencies]\n",
            "{} = {{ package = \"ortho_config\", path = {} }}\n",
            "clap = {{ version = \"4.5\", features = [\"derive\"] }}\n",
            "serde = {{ version = \"1.0\", features = [\"derive\"] }}\n",
            "tracing-subscriber = {{ version = \"0.3\", features = [\"env-filter\"] }}\n",
        ),
        dependency_name, serialized_crate_path,
    )
}

#[cfg(test)]
mod tests {
    //! Regression coverage for generated workspace manifests.

    use super::{
        DependencyAlias, DocumentedExample, ExampleId, ExampleWorkspace, RunFile, render_manifest,
    };
    use anyhow::{Context, Result, ensure};
    use std::path::Path;

    #[test]
    fn windows_dependency_path_produces_valid_toml() {
        let windows_path = r#"D:\a\"quoted\"\ortho-config\ortho_config"#;
        let serialized_path = toml::Value::String(windows_path.to_owned()).to_string();
        let generated = render_manifest("ortho_config", &serialized_path);
        let parsed = toml::from_str::<toml::Value>(&generated)
            .expect("serialized documentation manifest should parse as TOML");
        let parsed_path = parsed
            .get("dependencies")
            .and_then(|dependencies| dependencies.get("ortho_config"))
            .and_then(|dependency| dependency.get("path"))
            .and_then(toml::Value::as_str);
        assert_eq!(parsed_path, Some(windows_path));
    }

    #[test]
    fn independently_owned_workspaces_support_concurrent_interleavings() -> Result<()> {
        std::thread::scope(|scope| {
            let first = scope.spawn(|| exercise_workspace_interleavings("first", "updated-first"));
            let second =
                scope.spawn(|| exercise_workspace_interleavings("second", "updated-second"));

            first
                .join()
                .map_err(|_| anyhow::anyhow!("first workspace thread panicked"))??;
            second
                .join()
                .map_err(|_| anyhow::anyhow!("second workspace thread panicked"))??;
            Ok(())
        })
    }

    fn exercise_workspace_interleavings(initial: &str, updated: &str) -> Result<()> {
        let mut workspace = ExampleWorkspace::new(DependencyAlias("ortho_config"))?;
        workspace.add_binary(&file_probe())?;
        workspace.build()?;

        write_probe_value(&mut workspace, initial)?;
        assert_probe_output(&mut workspace, initial)?;
        write_probe_value(&mut workspace, updated)?;
        assert_probe_output(&mut workspace, updated)
    }

    fn file_probe() -> DocumentedExample {
        DocumentedExample {
            id: "workspace-probe".to_owned(),
            language: "rust".to_owned(),
            body: concat!(
                "fn main() -> std::io::Result<()> {\n",
                "    print!(\"{}\", std::fs::read_to_string(\"value.txt\")?);\n",
                "    Ok(())\n",
                "}\n",
            )
            .to_owned(),
            source: "workspace ownership probe",
            line: 1,
        }
    }

    fn write_probe_value(workspace: &mut ExampleWorkspace, contents: &str) -> Result<()> {
        workspace.write_run_file(
            ExampleId("workspace-probe"),
            RunFile {
                path: Path::new("value.txt"),
                contents,
            },
        )
    }

    fn assert_probe_output(workspace: &mut ExampleWorkspace, expected: &str) -> Result<()> {
        let output = workspace.run(ExampleId("workspace-probe"), std::iter::empty::<&str>())?;
        ensure!(output.status.success(), "workspace probe should succeed");
        let stdout = String::from_utf8(output.stdout).context("workspace probe output is UTF-8")?;
        ensure!(stdout == expected, "workspace probe output differed");
        Ok(())
    }
}
