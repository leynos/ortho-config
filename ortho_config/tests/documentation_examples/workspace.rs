//! Temporary Cargo workspaces for compiling and running documented Rust.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::ffi::OsStr;
use std::path::{Component, Path};
use std::process::{Command, Output};
use tempfile::TempDir;

use super::documentation_examples::{DocumentedExample, is_valid_example_id};

const CHILD_ENV_ALLOWLIST: &[&str] = &["SYSTEMROOT", "WINDIR"];
const CARGO_ENV_ALLOWLIST: &[&str] = &[
    "CARGO_HOME",
    "HOME",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "PATH",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "SYSTEMROOT",
    "TMPDIR",
    "USERPROFILE",
    "VCINSTALLDIR",
    "VSCMD_ARG_TGT_ARCH",
    "VSINSTALLDIR",
    "WINDIR",
    "WindowsSDKVersion",
    "WindowsSdkDir",
];

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
    pub fn add_binary(&self, example: &DocumentedExample) -> Result<()> {
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
    pub fn build(&self) -> Result<()> {
        let output = self
            .cargo_command()
            .args(["build", "--offline", "--bins"])
            .output()?;
        ensure!(
            output.status.success(),
            "documented Rust failed to compile:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }

    /// Run a built example in its own deterministic working directory.
    pub fn run<I, S>(&self, ExampleId(id): ExampleId<'_>, args: I) -> Result<Output>
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
    pub fn run_with_environment<'a, I, S, E>(
        &self,
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
        command.env_clear();
        for name in CHILD_ENV_ALLOWLIST {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .args(args)
            .current_dir(&run_dir)
            .env("HOME", &run_dir)
            .env("XDG_CONFIG_HOME", run_dir.join("xdg"))
            .envs(
                environment
                    .into_iter()
                    .map(|EnvironmentVariable { name, value }| (name, value)),
            )
            .output()
            .with_context(|| format!("run documented binary {id}"))
    }

    /// Write a file in one binary's deterministic working directory.
    pub fn write_run_file(
        &self,
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

    fn cargo_command(&self) -> Command {
        let mut command = Command::new("cargo");
        command.env_clear();
        for name in CARGO_ENV_ALLOWLIST {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .current_dir(self.root.path())
            .env("CARGO_TARGET_DIR", self.root.path().join("target"));
        command
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

    use super::{CARGO_ENV_ALLOWLIST, render_manifest};

    #[test]
    fn cargo_environment_preserves_msvc_linker_context() {
        for name in [
            "INCLUDE",
            "LIB",
            "LIBPATH",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "VCINSTALLDIR",
            "VSCMD_ARG_TGT_ARCH",
            "VSINSTALLDIR",
            "WindowsSDKVersion",
            "WindowsSdkDir",
        ] {
            assert!(
                CARGO_ENV_ALLOWLIST.contains(&name),
                "Cargo subprocess should inherit {name} on Windows"
            );
        }
    }

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
}
