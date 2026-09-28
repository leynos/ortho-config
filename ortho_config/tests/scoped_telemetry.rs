//! Scope attribution for the scoped walk's telemetry.
//!
//! `StackScopes` made the same `source` label reachable from more than one
//! scope: a candidate under `XDG_CONFIG_DIRS` is `source = "xdg"` in the system
//! scope, and one under `XDG_CONFIG_HOME` is `source = "xdg"` in the user scope.
//! Without a scope field the two are indistinguishable in a log, so an operator
//! seeing an `xdg` failure cannot tell which scope produced it — the exact
//! question the scoped mode introduces.
//!
//! These cases pin the field, and the case that matters most is the negative
//! one: a candidate that failed in one scope must not be reported against
//! another. A label that were merely present, and always the same, would pass a
//! weaker test while telling an operator nothing.
//!
//! A top-level test file rather than a module of `scoped_stacking`, because the
//! capture harness is shared and two suites would compile two copies of it —
//! the same split `discovery_telemetry` made.

use anyhow::{Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{AutomaticMode, ConfigDiscovery, DiscoveryScope, MapEnv};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[path = "support/tracing_capture.rs"]
#[expect(
    dead_code,
    reason = "This suite reads fields directly, so it needs only capture and the two fixture writers; the remaining helpers serve the discovery suites."
)]
mod capture_support;

use capture_support::{Captured, capture, write_fixture, write_fixture_with};

/// A builder over `env` with no project root, so only XDG candidates arise.
///
/// A project root is deliberately absent: it would add a `project`-scope
/// candidate to every case, and these cases are about telling the two `xdg`
/// scopes apart.
fn discovery_over(env: MapEnv) -> ConfigDiscovery {
    ConfigDiscovery::builder("demo")
        .config_file_name("config.toml")
        .clear_project_roots()
        .env_source(Arc::new(env))
        .build()
}

/// Every event named `name`, as `(source, scope)` pairs.
///
/// Returned as pairs rather than inspected one at a time because the assertion
/// is about the *pairing*: the same `source` label under two scopes is the fact
/// under test, so reading the fields in isolation would miss it.
fn scopes_of(events: &[Captured], name: &str) -> Vec<(String, String)> {
    events
        .iter()
        .filter(|event| event.field("event") == name)
        .map(|event| {
            (
                String::from(event.field("source")),
                String::from(event.field("scope")),
            )
        })
        .collect()
}

/// Every `discovery.candidate` event, as `(source, scope)` pairs.
fn candidate_scopes(events: &[Captured]) -> Vec<(String, String)> {
    scopes_of(events, "discovery.candidate")
}

/// Stage a directory where a configuration file is expected.
///
/// A candidate is consulted whenever its path *exists*, and one naming a
/// directory fails: the loader refuses it with "configuration path is not a
/// regular file". That is how a case produces a failure *inside* the walk,
/// rather than the silence of an absent candidate, which emits no event at all.
///
/// Created through a `cap_std::fs::Dir` handle opened on the temporary
/// directory the caller already holds, so the capability names the whole tree
/// this may touch and the repository's filesystem policy is honoured.
///
/// # Errors
///
/// Returns an error when the temporary directory cannot be opened, when `path`
/// lies outside it, or when the directory cannot be created.
fn dir_where_file_expected(temp: &tempfile::TempDir, path: &Path) -> Result<()> {
    let cap = Dir::open_ambient_dir(temp.path(), ambient_authority())?;
    let relative = path.strip_prefix(temp.path()).map_err(|_| {
        anyhow::anyhow!(
            "staged path {} escaped the temporary directory",
            path.display()
        )
    })?;
    cap.create_dir_all(relative)?;
    Ok(())
}

/// An empty directory to stand in for a home or XDG base that holds nothing.
///
/// An unset variable is not the same as one naming an empty directory: leaving
/// `HOME` unset would let the platform fallback resolve the real home, so the
/// candidate list would depend on the host.
///
/// # Errors
///
/// Returns an error when the directory cannot be created.
fn empty_directory(temp: &tempfile::TempDir, name: &str) -> Result<PathBuf> {
    let path = temp.path().join(name);
    dir_where_file_expected(temp, &path)?;
    Ok(path)
}

/// A system-scope failure names the system scope, not the user one.
///
/// `XDG_CONFIG_DIRS` is pinned to a directory holding `demo/config.toml` as a
/// directory, so the system scope's XDG candidate exists and fails, while the
/// user scope's is absent: unset `XDG_CONFIG_HOME` means the user scope looks
/// under `HOME`, which is an empty temporary directory.
#[test]
fn a_system_scope_failure_reports_the_system_scope() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dirs = temp.path().join("dirs");
    dir_where_file_expected(&temp, &dirs.join("demo/config.toml"))?;
    let empty_home = empty_directory(&temp, "home")?;

    let events = capture(|| {
        discovery_over(
            MapEnv::new()
                .with_var("XDG_CONFIG_DIRS", &dirs)
                .with_var("HOME", &empty_home),
        )
        .compose_scoped_layers(
            AutomaticMode::StackScopes,
            &[DiscoveryScope::System, DiscoveryScope::User],
        )
    });

    let scopes = candidate_scopes(&events);
    ensure!(
        scopes.contains(&(String::from("xdg"), String::from("system"))),
        "an xdg failure under XDG_CONFIG_DIRS must report the system scope, got {scopes:?}"
    );
    Ok(())
}

/// The same `source` label in two scopes is attributed to each, not to one.
///
/// This is the case the scope field exists for. A user-scope and a system-scope
/// candidate both fail, both report `source = "xdg"`, and the two events are
/// told apart by `scope` alone. Before the field existed these two events were
/// byte-identical, which is precisely the ambiguity the row named.
#[test]
fn an_xdg_failure_in_each_scope_is_attributed_to_its_own_scope() -> Result<()> {
    let temp = tempfile::tempdir()?;

    // A system candidate: `$XDG_CONFIG_DIRS/demo/config.toml` is a directory.
    let dirs = temp.path().join("dirs");
    dir_where_file_expected(&temp, &dirs.join("demo/config.toml"))?;
    // A user candidate: `$XDG_CONFIG_HOME/demo/config.toml` is a directory too.
    let xdg_home = temp.path().join("xdg-home");
    dir_where_file_expected(&temp, &xdg_home.join("demo/config.toml"))?;
    let empty_home = empty_directory(&temp, "home")?;

    let events = capture(|| {
        discovery_over(
            MapEnv::new()
                .with_var("XDG_CONFIG_DIRS", &dirs)
                .with_var("XDG_CONFIG_HOME", &xdg_home)
                .with_var("HOME", &empty_home),
        )
        .compose_scoped_layers(
            AutomaticMode::StackScopes,
            &[DiscoveryScope::System, DiscoveryScope::User],
        )
    });

    let scopes = candidate_scopes(&events);
    let xdg_scopes: Vec<&str> = scopes
        .iter()
        .filter(|(source, _)| source == "xdg")
        .map(|(_, scope)| scope.as_str())
        .collect();

    ensure!(
        xdg_scopes.contains(&"system") && xdg_scopes.contains(&"user"),
        "an xdg candidate failed in each scope, so both scopes must appear; got {scopes:?}"
    );
    ensure!(
        xdg_scopes
            .iter()
            .filter(|scope| **scope == "system")
            .count()
            == 1
            && xdg_scopes.iter().filter(|scope| **scope == "user").count() == 1,
        "each xdg failure belongs to exactly one scope; got {scopes:?}"
    );
    Ok(())
}

/// A mixed-scope run does not attribute one scope's failure to another.
///
/// The user scope alone produces a failure — its `XDG_CONFIG_HOME` candidate is
/// a directory — while the system scope's `XDG_CONFIG_DIRS` directory holds no
/// configuration at all and therefore yields no candidate. A label merely echoed
/// from the outer request, the first scope named, would report the user failure
/// as `system` and still pass a test that only asked for *a* scope.
#[test]
fn a_user_failure_is_not_attributed_to_the_system_scope() -> Result<()> {
    let temp = tempfile::tempdir()?;

    // The system scope is given a directory that exists but holds no
    // configuration, so it yields no candidate at all.
    let dirs = empty_directory(&temp, "dirs")?;
    // The user scope's candidate exists and cannot be read as a file.
    let xdg_home = temp.path().join("xdg-home");
    dir_where_file_expected(&temp, &xdg_home.join("demo/config.toml"))?;
    let empty_home = empty_directory(&temp, "home")?;

    let events = capture(|| {
        discovery_over(
            MapEnv::new()
                .with_var("XDG_CONFIG_DIRS", &dirs)
                .with_var("XDG_CONFIG_HOME", &xdg_home)
                .with_var("HOME", &empty_home),
        )
        .compose_scoped_layers(
            AutomaticMode::StackScopes,
            &[DiscoveryScope::System, DiscoveryScope::User],
        )
    });

    let scopes = candidate_scopes(&events);
    ensure!(
        !scopes.is_empty(),
        "the user scope's candidate must fail, or this case proves nothing"
    );
    ensure!(
        scopes.iter().all(|(_, scope)| scope == "user"),
        "only the user scope produced a candidate, so no event may claim another \
         scope; got {scopes:?}"
    );
    Ok(())
}

/// A scope is named on the terminal per-scope event too, not only on failures.
///
/// A successful scoped walk reports, once per scope that found a winner, which
/// scope's search produced it. The `source` label cannot say: `xdg` is consulted
/// in the user scope and in the system scope, so a reader cannot tell from the
/// label alone which walk the winner came from.
#[test]
fn a_scoped_winner_reports_its_scope_on_the_terminal_event() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let xdg_home = temp.path().join("xdg-home");
    // `write_fixture` writes only the leaf, so the directory above it is staged
    // through the capability helper.
    dir_where_file_expected(&temp, &xdg_home.join("demo"))?;
    write_fixture(&xdg_home.join("demo"), "config.toml")?;
    let empty_home = empty_directory(&temp, "home")?;

    let events = capture(|| {
        discovery_over(
            MapEnv::new()
                .with_var("XDG_CONFIG_HOME", &xdg_home)
                .with_var("HOME", &empty_home),
        )
        .compose_scoped_layers(AutomaticMode::StackScopes, &[DiscoveryScope::User])
    });

    let loads = scopes_of(&events, "discovery.load");
    ensure!(
        loads
            .iter()
            .any(|(source, scope)| source == "xdg" && scope == "user"),
        "the user scope's winning load must name its scope; got {loads:?}"
    );
    Ok(())
}

/// A malformed file furnishes a failure the fixture writer cannot produce.
///
/// Kept here rather than elsewhere because it is the only case that needs a file
/// which exists, is readable, and is not valid configuration.
#[test]
fn a_parse_failure_still_carries_its_scope() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let xdg_home = temp.path().join("xdg-home");
    dir_where_file_expected(&temp, &xdg_home.join("demo"))?;
    write_fixture_with(&xdg_home.join("demo"), "config.toml", "value = = 1\n")?;
    let empty_home = empty_directory(&temp, "home")?;

    let events = capture(|| {
        discovery_over(
            MapEnv::new()
                .with_var("XDG_CONFIG_HOME", &xdg_home)
                .with_var("HOME", &empty_home),
        )
        .compose_scoped_layers(AutomaticMode::StackScopes, &[DiscoveryScope::User])
    });

    let scopes = candidate_scopes(&events);
    ensure!(
        scopes.iter().any(|(_, scope)| scope == "user"),
        "a malformed user candidate must still name its scope; got {scopes:?}"
    );
    Ok(())
}
