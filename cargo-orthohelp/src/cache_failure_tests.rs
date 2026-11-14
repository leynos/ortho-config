//! Fingerprinting failure cases: every unreadable input has to surface.
//!
//! A fingerprint that silently ignores an input it could not read would pin a
//! stale IR to a fresh source tree, so every unreadable input has to surface as
//! an error. The cases below force each arm of that rule with a filesystem
//! state no healthy workspace produces.
//!
//! Split out of `cache_tests` once that module crossed Whitaker's 400-line
//! ceiling. Only the failure cases moved: the success-path cases stay beside
//! the fixture they share.
//!
//! Everything that manufactures an unreadable state is `cfg(unix)`, because the
//! states themselves are unix-only — `std::os::unix::fs::symlink`,
//! `/`-separated relative link targets and POSIX permission bits have no
//! portable spelling. The two absent-root cases are the exceptions: a missing
//! directory is missing on every platform, so they run everywhere.
//!
//! One case also depends on the permission bits being enforced, which a
//! privileged account bypasses. It reports the skip on standard error rather
//! than returning silently, because a silent return leaves the suite claiming
//! a pass for a case that never ran.

use super::*;

/// A temporary workspace root, kept alive by the returned handle.
///
/// Derived from [`temp_root`] rather than repeating it, so the two cannot drift
/// apart in how they name the root. It drops the capability handle and the
/// fallible signature: the cases below are plain `#[test]` functions that
/// assert rather than propagate, and threading a `Result` through each of them
/// would add a `panic_in_result_fn` expectation per case in exchange for
/// nothing they assert.
fn workspace() -> (tempfile::TempDir, Utf8PathBuf) {
    let (tempdir, root, _dir) = temp_root().expect("a temporary workspace root");
    (tempdir, root)
}

/// The I/O error a fingerprinting failure was reported with.
fn io_failure(failure: OrthohelpError) -> (Utf8PathBuf, std::io::Error) {
    match failure {
        OrthohelpError::Io { path, source } => (path, source),
        other => panic!("fingerprinting reports an I/O error, not {other:?}"),
    }
}

#[test]
fn a_missing_workspace_root_is_an_error_for_the_lockfile() {
    let (_tempdir, root) = workspace();
    let absent = root.join("absent");

    let failure = lockfile_fingerprint(&absent)
        .expect_err("a workspace root that cannot be opened must not read as a missing lockfile");

    let (path, source) = io_failure(failure);
    assert_eq!(path, absent, "the failure names the root it could not open");
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn a_missing_package_root_is_an_error_for_the_fingerprint() {
    let (_tempdir, root) = workspace();
    let absent = root.join("absent");

    let failure = fingerprint_package(&absent)
        .expect_err("an unreadable package root must not fingerprint as an empty package");

    let (path, source) = io_failure(failure);
    assert_eq!(path, absent, "the failure names the root it could not open");
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
}

#[cfg(unix)]
#[test]
fn a_lockfile_that_cannot_be_opened_is_reported() {
    let (_tempdir, root) = workspace();
    // A link that resolves to itself fails to open on every account, which
    // distinguishes "present but unreadable" from "absent, so not an input".
    std::os::unix::fs::symlink("Cargo.lock", root.join("Cargo.lock"))
        .expect("create a self-referential link");

    let failure =
        lockfile_fingerprint(&root).expect_err("an unopenable lockfile must not read as absent");

    let (path, _source) = io_failure(failure);
    assert_eq!(
        path,
        root.join("Cargo.lock"),
        "the failure names the lockfile it could not open"
    );
}

/// A filesystem state that makes one entry of the walk unreadable.
///
/// Two states rather than one: a self-referential link fails while opening,
/// which reaches the code that decides "absent, so skip" from "present, so
/// refuse"; a directory in a file's place opens and then fails while being
/// read. The whole walk has to abort for either, and for either the failure
/// has to name the entry it was working on rather than the root it started
/// from.
#[cfg(unix)]
#[derive(Clone, Copy)]
enum Broken {
    /// A link that resolves to itself, so opening it fails.
    SelfLink,
    /// A directory where a file belongs, so reading it fails.
    Directory,
}

#[cfg(unix)]
impl Broken {
    /// Create this state at `relative` under `root`.
    fn install(self, root: &Utf8Path, relative: &str) {
        match self {
            Self::SelfLink => std::os::unix::fs::symlink(relative, root.join(relative))
                .expect("create a self-referential link"),
            Self::Directory => std::fs::create_dir(root.join(relative))
                .expect("create a directory in the file's place"),
        }
    }
}

/// [`lockfile_fingerprint`] with its value discarded, for the case table.
///
/// Named rather than written as a closure: a closure literal here infers a
/// single concrete lifetime, and the parameter below is a higher-ranked
/// pointer (`for<'a> fn(&'a Utf8Path)`), which a `fn` item coerces to and a
/// closure does not.
#[cfg(unix)]
fn lockfile_fingerprint_only(root: &Utf8Path) -> Result<(), OrthohelpError> {
    lockfile_fingerprint(root).map(drop)
}

/// [`fingerprint_package`] with its value discarded, for the same reason.
#[cfg(unix)]
fn fingerprint_package_only(root: &Utf8Path) -> Result<(), OrthohelpError> {
    fingerprint_package(root).map(drop)
}

/// How a row spells the path its failure must report.
///
/// The two inputs differ in this, not just in which function they call: the
/// lockfile is opened from the workspace root and reported with that root
/// joined, while the package's own files are opened through the package
/// handle and reported relative to it. Spelling the expectation per row keeps
/// that difference visible instead of hiding it behind one shared rule.
#[cfg(unix)]
#[derive(Clone, Copy)]
enum Reported {
    /// The entry's name alone, relative to the handle it was opened through.
    Relative,
    /// The workspace root joined with the entry's name.
    UnderRoot,
}

#[cfg(unix)]
impl Reported {
    /// The path the failure must name for `relative`, under `root`.
    fn expected(self, root: &Utf8Path, relative: &str) -> Utf8PathBuf {
        match self {
            Self::Relative => Utf8PathBuf::from(relative),
            Self::UnderRoot => root.join(relative),
        }
    }
}

/// One unreadable entry, and the input it must not be lost from.
///
/// Each row discards a successful fingerprint's value: what every row asserts
/// is that there is no successful fingerprint to discard. The recovery probe
/// beside this module already proves the success case for both functions, so
/// nothing is lost by dropping the value here.
#[cfg(unix)]
#[rstest]
#[case::lockfile_read(
    Broken::Directory,
    "Cargo.lock",
    lockfile_fingerprint_only,
    Reported::UnderRoot
)]
#[case::package_file_open(
    Broken::SelfLink,
    "Cargo.toml",
    fingerprint_package_only,
    Reported::Relative
)]
#[case::package_file_read(
    Broken::Directory,
    "Cargo.toml",
    fingerprint_package_only,
    Reported::Relative
)]
#[case::source_tree_open(Broken::SelfLink, "src", fingerprint_package_only, Reported::Relative)]
fn an_unreadable_input_aborts_with_the_entry_it_could_not_read(
    #[case] broken: Broken,
    #[case] relative: &str,
    #[case] fingerprint: fn(&Utf8Path) -> Result<(), OrthohelpError>,
    #[case] reported: Reported,
) {
    let (_tempdir, root) = workspace();
    broken.install(&root, relative);

    let failure = fingerprint(&root)
        .expect_err("an input that cannot be read must abort the fingerprint, not be skipped");

    // The entry itself, not the root the walk started from: a walk that
    // reported its starting directory would name `relative` nowhere.
    let (path, _source) = io_failure(failure);
    assert_eq!(
        path,
        reported.expected(&root, relative),
        "the failure names the entry it could not read"
    );
}

#[cfg(unix)]
#[test]
fn a_source_entry_that_is_not_valid_utf8_is_reported() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let (_tempdir, root) = workspace();
    std::fs::create_dir(root.join("src")).expect("create the source tree");
    let unnamed = root
        .join("src")
        .as_std_path()
        .join(OsStr::from_bytes(b"raw-\xff"));
    std::fs::write(unnamed, b"x").expect("create a file whose name is not UTF-8");

    let failure = fingerprint_package(&root)
        .expect_err("an entry the digest cannot name must abort the walk");

    let (path, source) = io_failure(failure);
    assert_eq!(
        path,
        Utf8Path::new("src"),
        "the failure names the tree holding the unnamed entry"
    );
    assert_ne!(
        source.kind(),
        std::io::ErrorKind::NotFound,
        "an entry the digest cannot name aborts the walk rather than reading as absent"
    );
}

/// Say on standard error why the unreadable-directory case is not running.
///
/// A silent early return leaves `cargo test` reporting a pass for a case that
/// never executed, so an account that bypasses permission bits would look the
/// same as one where the walk was proven to abort. The suite's output is the
/// only place that difference can be seen.
///
/// `print_stderr` is denied workspace-wide because production code should log
/// through `tracing`; this is a test process announcing a skip, and there is no
/// subscriber to receive an event.
#[expect(
    clippy::print_stderr,
    reason = "a skipped test has no other way to report that it did not run"
)]
fn report_mode_bits_not_enforced() {
    eprintln!(
        "SKIP an_unreadable_source_directory_aborts_the_walk: \
         this account can read a mode-000 directory"
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_source_directory_aborts_the_walk() {
    use std::os::unix::fs::PermissionsExt;

    let (_tempdir, root) = workspace();
    let private = root.join("src").join("private");
    std::fs::create_dir_all(&private).expect("create a nested source directory");
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o000))
        .expect("make the directory unreadable");

    // Privileged accounts bypass permission bits, so the walk would enter the
    // directory and fingerprint it instead of failing. Detect that the way
    // `ortho_config`'s probe tests do — by asking whether the locked path is
    // still readable — rather than by inspecting the uid.
    let reaches_through_mode = std::fs::read_dir(&private).is_ok();
    // One `match` rather than a `let ... else` rebind: the workspace denies
    // `clippy::shadow_reuse`, which any shadowing rebind of the outcome trips.
    let reached = if reaches_through_mode {
        None
    } else {
        Some(fingerprint_package(&root))
    };

    // Restore access before asserting so the temporary tree can be removed.
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o755))
        .expect("restore directory access");
    let Some(attempted) = reached else {
        report_mode_bits_not_enforced();
        return;
    };
    let (path, source) =
        io_failure(attempted.expect_err(
            "a directory the walk cannot enter must abort fingerprinting, not be skipped",
        ));
    assert_eq!(
        path,
        Utf8Path::new("src/private"),
        "the failure names the directory it could not open"
    );
    assert_eq!(source.kind(), std::io::ErrorKind::PermissionDenied);
}
