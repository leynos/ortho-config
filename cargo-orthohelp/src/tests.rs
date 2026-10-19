//! Tests for ephemeral bridge crate orchestration.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::time::{Duration, Instant, SystemTime};

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use tempfile::tempdir;

use super::*;

fn dummy_paths() -> BridgePaths {
    BridgePaths {
        bridge_dir: Utf8PathBuf::from("/tmp/bridge"),
        manifest_path: Utf8PathBuf::from("/tmp/bridge/Cargo.toml"),
        target_dir: Utf8PathBuf::from("/tmp/bridge/target"),
    }
}

fn read_mtime(dir: &Dir, relative_path: &str) -> std::io::Result<SystemTime> {
    dir.metadata(relative_path)
        .and_then(|metadata| metadata.modified())
        .map(cap_std::time::SystemTime::into_std)
}

fn poll_mtime_until(
    dir: &Dir,
    relative_path: &str,
    timeout: Duration,
    matches: impl Fn(SystemTime) -> bool,
) -> std::io::Result<SystemTime> {
    let deadline = Instant::now() + timeout;
    let mut mtime = read_mtime(dir, relative_path)?;
    while !matches(mtime) {
        if Instant::now() >= deadline {
            return Err(std::io::Error::other(format!(
                "cache file mtime did not reach the expected value before timeout: {mtime:?}"
            )));
        }
        std::thread::sleep(Duration::from_millis(5));
        mtime = read_mtime(dir, relative_path)?;
    }
    Ok(mtime)
}

fn poll_clock_after(reference: SystemTime, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while SystemTime::now() <= reference {
        assert!(
            Instant::now() < deadline,
            "system clock did not advance beyond {reference:?} before timeout"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn build_bridge_command_applies_the_selected_build_environment() {
    let supplied = HashMap::from([
        ("RUSTC_WRAPPER", OsString::from("/tools/build-limits-rustc")),
        ("RUSTDOC", OsString::from("/toolchain/bin/rustdoc")),
        ("CARGO_BUILD_JOBS", OsString::from("4")),
        ("APPLICATION_SECRET", OsString::from("must-not-leak")),
    ]);
    let environment = BuildEnvironment::from_lookup(|name| supplied.get(name).cloned());
    let command = build_bridge_command(&dummy_paths(), &environment);
    let selected = command
        .get_envs()
        .filter_map(|(name, environment_value)| {
            environment_value.map(|selected_value| (name, selected_value))
        })
        .collect::<HashMap<_, _>>();

    assert_eq!(
        selected.get(OsStr::new("RUSTC_WRAPPER")),
        Some(&OsStr::new("/tools/build-limits-rustc"))
    );
    assert_eq!(
        selected.get(OsStr::new("RUSTDOC")),
        Some(&OsStr::new("/toolchain/bin/rustdoc"))
    );
    assert_eq!(
        selected.get(OsStr::new("CARGO_BUILD_JOBS")),
        Some(&OsStr::new("4"))
    );
    assert!(!selected.contains_key(OsStr::new("APPLICATION_SECRET")));
}

#[test]
fn write_ir_cache_is_idempotent() {
    const CONTENT: &str = r#"{"ir_version":"1.0.0"}"#;
    const OTHER: &str = r#"{"ir_version":"2.0.0"}"#;
    const MTIME_TIMEOUT: Duration = Duration::from_millis(1_500);

    let tmp = tempdir().expect("temp dir");
    let bridge_dir = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("UTF-8 path");
    let bridge = Dir::open_ambient_dir(&bridge_dir, ambient_authority()).expect("open bridge dir");
    bridge.create_dir_all("src").expect("create src");
    let paths = BridgeCache {
        cache_dir: bridge_dir.clone(),
        target_dir: bridge_dir.join("target"),
        ir_path: bridge_dir.join("ir.json"),
    };

    write_ir_cache(&paths, CONTENT).expect("first write");
    let mtime1 = read_mtime(&bridge, "ir.json").expect("read cache file mtime");

    write_ir_cache(&paths, CONTENT).expect("idempotent write");
    let mtime2 = poll_mtime_until(&bridge, "ir.json", MTIME_TIMEOUT, |mtime| mtime == mtime1)
        .expect("poll mtime until unchanged");

    assert_eq!(
        mtime1, mtime2,
        "mtime should not change when content is identical"
    );

    let next_filesystem_tick = mtime2 + Duration::from_secs(1);
    poll_clock_after(next_filesystem_tick, MTIME_TIMEOUT);
    write_ir_cache(&paths, OTHER).expect("write new content");
    let mtime3 = poll_mtime_until(&bridge, "ir.json", MTIME_TIMEOUT, |mtime| mtime > mtime2)
        .expect("poll mtime until changed");

    assert!(mtime3 > mtime2, "mtime should advance when content changes");
}
