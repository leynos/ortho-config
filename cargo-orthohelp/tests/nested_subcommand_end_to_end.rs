//! End-to-end bridge smoke tests for nested subcommand documentation.
//!
//! These tests validate that nested subcommands survive the bridge into IR
//! JSON, split roff man pages, and `PowerShell` wrapper functions. Downstream
//! tooling depends on that shape to keep the documented CLI surface, generated
//! manuals, and shell wrappers aligned.

mod fixtures;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest::rstest;
use serde_json::Value;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::Read;
use std::process::Command;
use std::sync::{LazyLock, Mutex, PoisonError};
use tempfile::TempDir;

type TestError = Box<dyn Error + Send + Sync>;

const FIXTURE_PACKAGE: &str = "orthohelp_fixture";
const FIXTURE_ROOT_TYPE: &str = "orthohelp_fixture::NestedFixtureConfig";

// The rstest cases share a content-addressed bridge directory whose manifest is
// rewritten before each build, so their subprocesses must not overlap.
static BRIDGE_BUILD_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

#[rstest]
#[case::ir(
    "ir",
    &["--format", "ir"],
    assert_ir_contains_nested_tree
)]
#[case::man(
    "man",
    &[
        "--format",
        "man",
        "--man-date",
        "2026-06-04",
        "--man-split-subcommands",
    ],
    assert_man_contains_nested_pages
)]
#[case::powershell(
    "PowerShell",
    &[
        "--format",
        "ps",
        "--ps-split-subcommands",
        "true",
        "--ensure-en-us",
        "true",
    ],
    assert_powershell_contains_subcommand_functions
)]
fn nested_subcommand_tree_survives_bridge_outputs(
    #[case] name: &str,
    #[case] args: &[&str],
    #[case] assertion: fn(&Utf8PathBuf) -> Result<(), TestError>,
) -> Result<(), TestError> {
    let (_temp, out_dir) = temp_out_dir()?;
    {
        let _bridge_build_guard = BRIDGE_BUILD_MUTEX
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        run_orthohelp(&out_dir, args)?;
    }
    assertion(&out_dir).map_err(|err| assertion_failed(name, err))?;
    Ok(())
}

#[derive(Debug)]
struct AssertionFailure {
    name: String,
    source: TestError,
}

impl Display for AssertionFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} assertion failed", self.name)
    }
}

impl Error for AssertionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

fn assertion_failed(name: &str, source: TestError) -> TestError {
    Box::new(AssertionFailure {
        name: name.to_owned(),
        source,
    })
}

/// Creates an output directory and keeps its temporary owner alive.
///
/// The returned path must be UTF-8 because `cargo-orthohelp` and the
/// capability filesystem helpers use `Utf8PathBuf` throughout these tests.
fn temp_out_dir() -> Result<(TempDir, Utf8PathBuf), TestError> {
    let temp_dir = tempfile::tempdir()?;
    let out_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .map_err(|path| format!("temporary path is not UTF-8: {}", path.display()))?;
    Ok((temp_dir, out_dir))
}

/// Runs `cargo-orthohelp orthohelp` against the nested fixture root type.
///
/// A non-zero process exit is returned as an error containing captured stderr
/// so bridge build and renderer failures stay visible in test diagnostics.
fn run_orthohelp(out_dir: &Utf8PathBuf, format_args: &[&str]) -> Result<(), TestError> {
    let exe = fixtures::cargo_orthohelp_exe()?;
    let workspace_root = fixtures::workspace_root()?;
    let output = Command::new(exe.as_str())
        .current_dir(workspace_root.as_str())
        .arg("orthohelp")
        .arg("--out-dir")
        .arg(out_dir.as_str())
        .arg("--package")
        .arg(FIXTURE_PACKAGE)
        .arg("--root-type")
        .arg(FIXTURE_ROOT_TYPE)
        .arg("--locale")
        .arg("en-US")
        .args(format_args)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("cargo-orthohelp failed: {stderr}").into())
    }
}

/// Asserts that localized IR preserves top-level and nested command ordering.
fn assert_ir_contains_nested_tree(out_dir: &Utf8PathBuf) -> Result<(), TestError> {
    let ir = read_output(out_dir, Utf8Path::new("ir/en-US.json"))?;
    let value: Value = serde_json::from_str(&ir)?;
    let subcommands = array_field(&value, "subcommands")?;
    assert_app_names(subcommands, &["greet", "version", "admin"])?;

    let admin = subcommands
        .iter()
        .find(|command| command.get("app_name").and_then(Value::as_str) == Some("admin"))
        .ok_or("admin subcommand missing from IR")?;
    assert_app_names(
        array_field(admin, "subcommands")?,
        &["audit", "grant-access"],
    )?;
    Ok(())
}

/// Asserts that split man pages include the nested admin command sections.
fn assert_man_contains_nested_pages(out_dir: &Utf8PathBuf) -> Result<(), TestError> {
    read_output(out_dir, Utf8Path::new("man/man1/nested_fixture.1"))?;

    read_output(out_dir, Utf8Path::new("man/man1/nested_fixture-greet.1"))?;
    read_output(out_dir, Utf8Path::new("man/man1/nested_fixture-version.1"))?;
    let admin_page = read_output(out_dir, Utf8Path::new("man/man1/nested_fixture-admin.1"))?;
    let artefact = OutputArtefact::new(&admin_page, "admin man page");
    ensure_contains(artefact, ".SH COMMANDS")?;
    ensure_contains(artefact, ".SS audit")?;
    ensure_contains(artefact, ".SS grant-access")?;
    Ok(())
}

/// Asserts that generated `PowerShell` wrappers expose the expected functions.
///
/// `out_dir` is the directory containing generated files; returning `Result`
/// lets the test report file and assertion failures without panicking.
fn assert_powershell_contains_subcommand_functions(out_dir: &Utf8PathBuf) -> Result<(), TestError> {
    let module = read_output(
        out_dir,
        Utf8Path::new("powershell/NestedFixture/NestedFixture.psm1"),
    )?;
    let artefact = OutputArtefact::new(&module, "PowerShell wrapper");
    ensure_contains(artefact, "function nested_fixture_greet")?;
    ensure_contains(artefact, "function nested_fixture_version")?;
    ensure_contains(artefact, "function nested_fixture_admin")?;
    // PowerShell wrapper splitting is intentionally one level deep: the
    // generated admin wrapper delegates nested dispatch to the executable
    // rather than exposing `nested_fixture_admin_audit` style functions.
    ensure_excludes(artefact, "function nested_fixture_admin_audit")?;
    ensure_excludes(artefact, "function nested_fixture_admin_grant_access")?;
    Ok(())
}

#[derive(Clone, Copy)]
struct OutputArtefact<'a> {
    content: &'a str,
    description: &'a str,
}

impl<'a> OutputArtefact<'a> {
    const fn new(content: &'a str, description: &'a str) -> Self {
        Self {
            content,
            description,
        }
    }
}

/// Reads a generated UTF-8 text artefact through a capability directory.
///
/// The ambient authority is scoped to the temporary output root produced by
/// this test; I/O errors or invalid UTF-8 are reported to the caller.
fn read_output(out_dir: &Utf8PathBuf, relative_path: &Utf8Path) -> Result<String, TestError> {
    let dir = Dir::open_ambient_dir(out_dir, ambient_authority())?;
    let mut file = dir.open(relative_path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

/// Returns a JSON object field when it is present and array-shaped.
///
/// The error text names the missing or incorrectly typed field to make IR
/// shape regressions easier to diagnose.
fn array_field<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, TestError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{field} should be an array").into())
}

/// Compares command `app_name` values in declaration order.
///
/// Ordering is part of the bridge contract because downstream renderers use
/// the emitted sequence directly when presenting command lists.
fn assert_app_names(commands: &[Value], expected: &[&str]) -> Result<(), TestError> {
    let actual = commands
        .iter()
        .filter_map(|command| command.get("app_name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("expected app names {expected:?}, got {actual:?}").into())
    }
}

/// Requires generated text to contain a marker and names the artefact on error.
fn ensure_contains(artefact: OutputArtefact<'_>, needle: &str) -> Result<(), TestError> {
    if artefact.content.contains(needle) {
        Ok(())
    } else {
        Err(format!("{} should contain {needle:?}", artefact.description).into())
    }
}

/// Requires generated text to omit a marker and names the artefact on error.
fn ensure_excludes(artefact: OutputArtefact<'_>, needle: &str) -> Result<(), TestError> {
    if artefact.content.contains(needle) {
        Err(format!("{} should not contain {needle:?}", artefact.description).into())
    } else {
        Ok(())
    }
}
