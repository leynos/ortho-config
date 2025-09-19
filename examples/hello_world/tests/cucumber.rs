use camino::Utf8PathBuf;
use cucumber::World as _;
use tokio::process::Command;

mod steps;

/// Shared state threaded through Cucumber steps.
#[derive(Debug, Default, cucumber::World)]
pub struct World {
    /// Result captured after invoking the binary.
    pub result: Option<CommandResult>,
}

/// Output captured from executing the CLI.
#[derive(Debug, Default)]
pub struct CommandResult {
    pub status: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl From<std::process::Output> for CommandResult {
    fn from(output: std::process::Output) -> Self {
        Self {
            status: output.status.code(),
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

impl World {
    /// Runs the example binary with the provided arguments.
    ///
    /// # Panics
    ///
    /// Panics if spawning or awaiting the example binary fails.
    pub async fn run_example(&mut self, args: Vec<String>) {
        let binary = binary_path();
        let mut command = Command::new(binary.as_std_path());
        command.args(&args);
        let output = command.output().await.expect("execute hello_world binary");
        self.result = Some(CommandResult::from(output));
    }

    /// Returns the captured command output from the most recent run.
    ///
    /// # Panics
    ///
    /// Panics if the example has not been executed in the current scenario.
    #[must_use]
    pub fn result(&self) -> &CommandResult {
        self.result
            .as_ref()
            .expect("command execution result available")
    }
}

fn binary_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!(
        "CARGO_BIN_EXE_hello_world",
        "Cargo must set the hello_world binary path for integration tests",
    ))
}

#[tokio::main]
async fn main() {
    World::run("tests/features").await;
}
