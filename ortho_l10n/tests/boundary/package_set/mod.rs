//! Materializes the three crates as a stranger would receive them.
//!
//! A path dependency in this workspace proves nothing about the published
//! graph. Cargo rewrites a member's `path` dependency into a registry version
//! requirement when it packages, and a stranger then resolves that version
//! requirement from crates.io with no workspace in sight. The contracts here
//! exercise the second half of that: the packaged manifests, the packaged
//! sources, and the exact edges between them.
//!
//! There is no network step. `cargo package` is run offline against the shared
//! package cache, and the resulting `.crate` files are unpacked into the same
//! target directory. That produces the *stranger's* manifests - the normalized
//! `Cargo.toml` with path dependencies rewritten - while leaving the sources
//! identical to the workspace's.
//!
//! # The two-invocation recipe
//!
//! `ortho_config` depends on `ortho_config_macros` and `ortho_l10n` by version,
//! `ortho_config_macros` depends on `ortho_l10n` by version, and no published
//! version exists yet, so every same-version edge has to be patched to the
//! sibling sources before a crate can be packaged at all. That much is solved
//! by the patch list below. What is not solved by patching is *verification*:
//! `cargo package` builds every dependency table, dev-dependencies included, so
//! [`ORTHO_L10N_AND_CONFIG`] needs the facade's dev-dependency on
//! `ortho_config_test_helpers` patched as well - a path member a clean runner
//! has never fetched an index entry for. Packaging all three crates in one
//! invocation then fails for a different reason: Cargo verifies a crate against
//! the lockfile it just generated, and the sibling that the first invocation
//! patched is not the one the second invocation's lockfile records. Two
//! invocations, each patching the edges of the crates it does not package,
//! produce all three roots with `--verify` intact.
//!
//! Verification is not optional here. It is the step that unpacks each
//! `.crate` into `package/<name>-<version>/`, which is the root every consumer
//! and manifest contract reads. `--no-verify` skips the unpacking along with
//! the compile, so the archives would be written and nothing would unpack them.
//!
//! # Cost, and why the result is shared within one run
//!
//! The two invocations verify what they package, so they compile each
//! packaged crate's dependency table from its own unpacked root. On a shared
//! host under compile admission that is not a small cost and it is not a fixed
//! one: a single verification has been measured at 216 s with every compiler
//! slot queued, and the bound has to leave room for it.
//!
//! The result is therefore shared, but only within one test run. `roots`
//! caches it per process, and `lease` extends that to every process nextest
//! starts for the same run, so six packaged contracts perform one packaging
//! pair between them rather than six. It is deliberately *not* shared between
//! runs: a later run may be reading different sources, and nothing here can
//! cheaply prove that it is not. See [`lease`] for that reasoning in full.
//!
//! # Why the patch list is per invocation
//!
//! A patch only helps if something actually resolves through it, and Cargo says
//! so loudly when it does not: "patch ... was not used in the crate graph". The
//! second invocation therefore patches `ortho_l10n` alone rather than copying
//! the first's list, and the first patches each crate that `ortho_config`
//! declares by path under *its package name*, which is not always its
//! directory name (`ortho_config_test_helpers` lives in `test_helpers`).

use super::fixture_cargo::{
    Budget, FixtureCargo, cargo_binary, inherited_toolchain, run_with_deadline, toml_string,
};
use anyhow::{Result, bail, ensure};
use camino::Utf8Path;
use std::process::Command;
use std::time::Duration;

/// Longest either packaging invocation may run.
///
/// Each invocation is bounded on its own, and every entry in
/// [`PACKAGING_INVOCATIONS`] shares one test, so all of them have to fit
/// inside nextest's per-test allowance with room for the root checks around
/// them. The figure is derived from the allowance rather than chosen for
/// comfort, because a bound that does not fit never fires: nextest stops the
/// test first, and the error naming the invocation that hung is never printed.
///
/// The pair took 244 s on this host with the packaging target removed, which is
/// an upper bound rather than a build time: it includes waiting for
/// compile-admission slots, which the child inherits. The bounds were 240 s
/// when six packaged contracts timed out on a shared host with 340 compile
/// requests queued behind them; the failure named the deadline but discarded
/// the queue lines the child had printed, so the wait could not be told apart
/// from a hang. The tail is now reported with the error.
///
/// # Why one invocation needs most of the allowance
///
/// The first invocation verifies two crates in sequence, and a single
/// verification has been measured at 216 s on a host whose every compiler slot
/// was queued. Two of those plus their packaging step is the pair, and the
/// pair is one *test*, so the bound on a single invocation has to be most of
/// the nextest allowance rather than a share of it. The serialization lease
/// means the pair now runs once per run rather than once per contract, so this
/// is the cost of the whole suite's packaging and not a cost paid six times.
///
/// `boundary_packaging_test.py` reads this figure and the invocation count, and
/// fails if the pair no longer fits the allowance the override grants.
const PACKAGE_DEADLINE: Duration = Duration::from_secs(440);

/// The `cargo package` invocations that together produce all three roots.
///
/// Each entry names the crates it packages and the workspace members it
/// patches to their sources, as `(package name, directory)` pairs. The two
/// differ for `ortho_config_test_helpers`, which lives in `test_helpers`: a
/// patch is keyed by package name, so spelling the directory there would
/// silently patch nothing.
///
/// They are data rather than two call statements because the per-test
/// allowance is shared between them: `boundary_packaging_test.py` reads this
/// array's length and [`PACKAGE_DEADLINE`] to check that the whole set can
/// reach its own deadlines before nextest stops the test. The array length is
/// part of the type, so rustc checks it against the initializers.
pub(super) const PACKAGING_INVOCATIONS: [PackageInvocation; 2] = [
    PackageInvocation {
        crates: ORTHO_L10N_AND_CONFIG,
        patched: ORTHO_CONFIG_DEPENDENCIES,
    },
    PackageInvocation {
        crates: &["ortho_config_macros"],
        patched: &[ORTHO_L10N],
    },
];

/// One `cargo package` invocation: what it packages, and what it patches.
///
/// A struct rather than a tuple because the two fields are both slices of
/// small tuples and the nesting loses the reader: `(&[&str], &[(&str, &str)])`
/// says nothing about which slice is which, and Clippy rejects the spelling
/// outright as a complex type.
pub(super) struct PackageInvocation {
    /// Crates to package, by package name.
    pub(super) crates: &'static [&'static str],
    /// Workspace members to patch, as `(package name, directory)`.
    pub(super) patched: &'static [(&'static str, &'static str)],
}

/// The crates the first invocation packages: the runtime and the facade.
const ORTHO_L10N_AND_CONFIG: &[&str] = &["ortho_l10n", "ortho_config"];

/// A workspace member as a `cargo patch` needs it: package name, then
/// directory. The two differ when a member's package is renamed, as
/// `ortho_config_test_helpers` is.
const ORTHO_L10N: (&str, &str) = ("ortho_l10n", "ortho_l10n");

/// Every same-version edge of `ortho_config`, by package name.
///
/// The runtime and the derive macros are normal dependencies; the test helpers
/// are a dev-dependency. All three are path dependencies in the workspace, so
/// `cargo package` rewrites each into a version requirement, and verification
/// resolves each from them. Leaving the helpers out is what made this
/// invocation fail on a clean runner: the facade's dev-dependency was looked
/// up in an index that has never seen it.
const ORTHO_CONFIG_DEPENDENCIES: &[(&str, &str)] = &[
    ORTHO_L10N,
    ("ortho_config_macros", "ortho_config_macros"),
    ("ortho_config_test_helpers", "test_helpers"),
];

mod lease;
mod record;
mod roots;
mod run_guard;

#[cfg(test)]
#[path = "scratch.rs"]
mod scratch;

// Re-exported so the contracts keep naming one module for the packaged set.
pub(crate) use roots::{
    PACKAGED_CRATES, PACKAGED_VERSION, assert_packaged_set_complete, packaged_crate,
    prepare_packaged_set, unpacked_directory,
};

/// Run one `cargo package` invocation.
///
/// The invocation is bounded by [`PACKAGE_DEADLINE`] or by what the contract's
/// shared [`Budget`] has left, whichever is sooner.
pub(super) fn package_sources(
    repository: &Utf8Path,
    target: &Utf8Path,
    invocation: &PackageInvocation,
    budget: Budget,
) -> Result<()> {
    // The subcommand comes first: `--manifest-path` is a per-command flag.
    let mut args = vec![String::from("package")];
    args.push(String::from("--manifest-path"));
    args.push(repository.join("Cargo.toml").into_string());
    args.push(String::from("--offline"));
    // The worktree is usually dirty while a milestone is being built; the
    // contract is about the sources as they are, not about what is committed.
    args.push(String::from("--allow-dirty"));
    for crate_name in invocation.crates {
        args.push(String::from("-p"));
        args.push((*crate_name).to_owned());
    }
    for (package, directory) in invocation.patched {
        args.push(String::from("--config"));
        args.push(format!(
            "patch.crates-io.{package}.path={}",
            toml_string(&sibling_path(repository, directory)?),
        ));
    }

    let mut command = Command::new(cargo_binary());
    command
        .current_dir(repository)
        .args(&args)
        .env_clear()
        .envs(inherited_toolchain())
        .env("CARGO_TARGET_DIR", target);
    let description = format!("cargo package for {}", invocation.crates.join(", "));
    let output = run_with_deadline(command, budget, PACKAGE_DEADLINE, &description)?;
    if !output.status.success() {
        bail!(
            "{description} failed with {}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
    }
    Ok(())
}

/// Spell `directory` as a path relative to the repository, for a Cargo patch.
///
/// Cargo resolves a `--config` patch path against the working directory, so a
/// relative spelling is both correct and stable across checkouts.
fn sibling_path(repository: &Utf8Path, directory: &str) -> Result<String> {
    let path = repository.join(directory);
    ensure!(
        path.join("Cargo.toml").is_file(),
        "the sibling patch source {path} is not a crate root",
    );
    Ok(directory.to_owned())
}

/// A foreign consumer root that can be written and resolved.
///
/// Wraps [`FixtureCargo`] with the consumer root's own files, so a contract
/// reads as "render, write, resolve" rather than repeating the layout each
/// time.
pub(crate) struct ConsumerFixture {
    cargo: FixtureCargo,
}

impl ConsumerFixture {
    /// Create an empty consumer fixture under `budget`.
    ///
    /// Every nested Cargo call the fixture makes shares that allowance, so the
    /// contract's calls are bounded as a sequence rather than one at a time.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture root cannot be created.
    pub(crate) fn new(budget: Budget) -> Result<Self> {
        Ok(Self {
            cargo: FixtureCargo::new(budget)?,
        })
    }

    /// Write a rendered consumer and return its fixture.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest or library cannot be written.
    pub(crate) fn write_consumer(self, consumer: &super::consumer::Consumer) -> Result<Self> {
        self.cargo
            .write(Utf8Path::new("Cargo.toml"), &consumer.manifest)?;
        self.cargo
            .write(Utf8Path::new("src/lib.rs"), &consumer.library)?;
        Ok(self)
    }

    /// Spawn `cargo metadata` for the consumer root and parse the result.
    ///
    /// Named for the command it runs, not for the document it returns. A
    /// `metadata()` accessor would read as a field lookup; this spawns Cargo,
    /// takes the package-cache lock, and can fail for reasons that have
    /// nothing to do with the consumer.
    ///
    /// # Errors
    ///
    /// Returns an error when Cargo fails or emits an unreadable document.
    pub(crate) fn resolve_metadata(&self) -> Result<super::fixture_cargo::Metadata> {
        self.cargo.resolve_metadata()
    }

    /// Build and test the consumer root.
    ///
    /// # Errors
    ///
    /// Returns an error carrying both streams when the build or its tests fail.
    pub(crate) fn test(&self) -> Result<()> {
        self.cargo
            .cargo_checked(&["test", "--all-targets", "--all-features"])
            .map(|_stdout| ())
    }
}
