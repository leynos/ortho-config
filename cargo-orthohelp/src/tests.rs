//! Tests for ephemeral bridge crate orchestration.

use std::ffi::OsStr;
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
        ir_path: Utf8PathBuf::from("/tmp/bridge/ir.json"),
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

const COVERAGE_VARS: &[&str] = &[
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_WRAPPER",
    "LLVM_PROFILE_FILE",
    "CARGO_LLVM_COV_TARGET_DIR",
    "CARGO_TARGET_DIR",
];

#[test]
fn build_bridge_command_removes_coverage_env_vars() {
    let cmd = build_bridge_command(&dummy_paths());
    let env_overrides: Vec<(&OsStr, Option<&OsStr>)> = cmd.get_envs().collect();

    for var in COVERAGE_VARS {
        let entry = env_overrides
            .iter()
            .find(|(key, _)| *key == OsStr::new(var));
        assert!(
            matches!(entry, Some((_, None))),
            "build_bridge_command should mark {var} for removal \
             (must appear in get_envs() with value None, not absent or set)"
        );
    }
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
    let paths = BridgePaths {
        bridge_dir: bridge_dir.clone(),
        manifest_path: bridge_dir.join("Cargo.toml"),
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
