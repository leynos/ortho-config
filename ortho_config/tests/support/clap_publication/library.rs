//! Which file Cargo compiles as a package's library crate root.
//!
//! `src/lib.rs` is Cargo's default, not a rule: a package may set `[lib] path`
//! and keep an unused `src/lib.rs` beside the file it really compiles. The
//! inventory therefore asks `cargo metadata` which file the library target
//! starts from. A package with no library target, or with more than one, is
//! refused rather than guessed at, because either shape would let the count
//! describe a file the compiler does not build.

use anyhow::{Result, bail, ensure};
use camino::Utf8PathBuf;
use cargo_metadata::{Package, Target, TargetKind};

/// The crate root `metadata` reports for `package`'s library target.
pub(crate) fn library_crate_root(package: &Package) -> Result<Utf8PathBuf> {
    unique_library_root(&package.targets)
}

/// Select the one reported crate root from `targets`, refusing none or several.
pub(super) fn unique_library_root(targets: &[Target]) -> Result<Utf8PathBuf> {
    let roots = targets
        .iter()
        .filter(|target| is_library_target(target))
        .map(|target| target.src_path.clone())
        .collect::<Vec<_>>();
    let Some(root) = roots.first() else {
        bail!(
            "cargo metadata reports no library target for the resolved package; the inventory \
             has no crate root to start from"
        );
    };
    ensure!(
        roots.len() == 1,
        "cargo metadata reports {} library targets for the resolved package; the inventory \
         cannot tell which crate root the count describes",
        roots.len(),
    );
    Ok(root.clone())
}

/// Whether a target is a Rust library target rather than a binary or test.
///
/// A `proc-macro` target is excluded: it cannot be the crate that defines a
/// public `ErrorKind`, and counting one would describe a different crate.
pub(super) fn is_library_target(target: &Target) -> bool {
    target.kind.iter().any(|kind| {
        matches!(
            kind,
            TargetKind::Lib
                | TargetKind::RLib
                | TargetKind::DyLib
                | TargetKind::CDyLib
                | TargetKind::StaticLib
        )
    })
}
