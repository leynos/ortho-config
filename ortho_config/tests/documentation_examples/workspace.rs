//! Temporary Cargo workspaces for compiling and running documented Rust.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::ffi::OsStr;
use std::process::{Command, Output};
use tempfile::TempDir;

use super::documentation_examples::DocumentedExample;

/// An isolated package whose binary sources are exact Markdown fence bodies.
pub struct ExampleWorkspace {
    root: TempDir,
    directory: Dir,
}

impl ExampleWorkspace {
    /// Create a package using `dependency_name` for the local `OrthoConfig` crate.
    pub fn new(dependency_name: &str) -> Result<Self> {
        let root = tempfile::tempdir().context("create documentation example workspace")?;
        let directory = Dir::open_ambient_dir(root.path(), ambient_authority())
            .context("open documentation example workspace")?;
        directory
            .create_dir_all("src/bin")
            .context("create bin directory")?;
        directory
            .write("Cargo.toml", manifest(dependency_name))
            .context("write example manifest")?;
        Ok(Self { root, directory })
    }

    /// Add an example as a binary without altering its published source.
    pub fn add_binary(&self, example: &DocumentedExample) -> Result<()> {
        ensure!(example.language == "rust", "{} is not Rust", example.id);
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
    pub fn run<I, S>(&self, id: &str, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
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
        Command::new(binary)
            .args(args)
            .current_dir(&run_dir)
            .env("HOME", &run_dir)
            .env("XDG_CONFIG_HOME", run_dir.join("xdg"))
            .env_remove("ACME_CONFIG_PATH")
            .env_remove("ACME_HOST")
            .env_remove("ACME_PORT")
            .env_remove("ACME_LOG_LEVEL")
            .env_remove("HELLO_HOST")
            .env_remove("HELLO_PORT")
            .output()
            .with_context(|| format!("run documented binary {id}"))
    }

    /// Write a file in one binary's deterministic working directory.
    pub fn write_run_file(&self, id: &str, name: &str, contents: &str) -> Result<()> {
        let run_dir_name = format!("run-{id}");
        self.directory.create_dir_all(&run_dir_name)?;
        let run_dir = self.directory.open_dir(&run_dir_name)?;
        run_dir.write(name, contents)?;
        Ok(())
    }

    fn cargo_command(&self) -> Command {
        let mut command = Command::new("cargo");
        command
            .current_dir(self.root.path())
            .env("CARGO_TARGET_DIR", self.root.path().join("target"));
        command
    }
}

fn manifest(dependency_name: &str) -> String {
    let crate_path = env!("CARGO_MANIFEST_DIR");
    format!(
        concat!(
            "[package]\n",
            "name = \"documented-examples\"\n",
            "version = \"0.0.0\"\n",
            "edition = \"2024\"\n\n",
            "[dependencies]\n",
            "{} = {{ package = \"ortho_config\", path = \"{}\" }}\n",
            "clap = {{ version = \"4.5\", features = [\"derive\"] }}\n",
            "serde = {{ version = \"1.0\", features = [\"derive\"] }}\n",
            "tracing-subscriber = {{ version = \"0.3\", features = [\"env-filter\"] }}\n",
        ),
        dependency_name, crate_path,
    )
}
