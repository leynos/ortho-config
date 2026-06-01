//! Tests for ephemeral bridge crate orchestration.

use std::ffi::OsStr;
use std::time::{Duration, Instant, SystemTime};

use camino::{Utf8Path, Utf8PathBuf};
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

fn read_mtime(path: &Utf8Path) -> SystemTime {
    std::fs::metadata(path.as_std_path())
        .and_then(|metadata| metadata.modified())
        .expect("read cache file mtime")
}

fn poll_mtime_until(
    path: &Utf8Path,
    timeout: Duration,
    matches: impl Fn(SystemTime) -> bool,
) -> SystemTime {
    let deadline = Instant::now() + timeout;
    let mut mtime = read_mtime(path);
    while !matches(mtime) {
        assert!(
            Instant::now() < deadline,
            "cache file mtime did not reach the expected value before timeout: {mtime:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
        mtime = read_mtime(path);
    }
    mtime
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
fn build_bridge_command_strips_coverage_env_vars() {
    let vars = [
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTC_WRAPPER",
        "LLVM_PROFILE_FILE",
        "CARGO_LLVM_COV_TARGET_DIR",
        "CARGO_TARGET_DIR",
    ];

    let cmd = build_bridge_command(&dummy_paths());
    let removed: Vec<&OsStr> = cmd
        .get_envs()
        .filter_map(|(key, value)| if value.is_none() { Some(key) } else { None })
        .collect();

    for var in vars {
        assert!(
            removed.iter().any(|key| *key == OsStr::new(var)),
            "build_bridge_command should remove {var} from the child environment"
        );
    }
}

#[test]
fn build_bridge_command_does_not_set_coverage_env_vars() {
    let vars = [
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTC_WRAPPER",
        "LLVM_PROFILE_FILE",
        "CARGO_LLVM_COV_TARGET_DIR",
        "CARGO_TARGET_DIR",
    ];

    let cmd = build_bridge_command(&dummy_paths());
    let set: Vec<&OsStr> = cmd
        .get_envs()
        .filter_map(|(key, value)| value.map(|_| key))
        .collect();

    for var in vars {
        assert!(
            !set.iter().any(|key| *key == OsStr::new(var)),
            "build_bridge_command should not set {var} in the child environment"
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
    std::fs::create_dir_all(bridge_dir.join("src")).expect("create src");
    let paths = BridgePaths {
        bridge_dir: bridge_dir.clone(),
        manifest_path: bridge_dir.join("Cargo.toml"),
        target_dir: bridge_dir.join("target"),
        ir_path: bridge_dir.join("ir.json"),
    };

    write_ir_cache(&paths, CONTENT).expect("first write");
    let mtime1 = read_mtime(&paths.ir_path);

    write_ir_cache(&paths, CONTENT).expect("idempotent write");
    let mtime2 = poll_mtime_until(&paths.ir_path, MTIME_TIMEOUT, |mtime| mtime == mtime1);

    assert_eq!(
        mtime1, mtime2,
        "mtime should not change when content is identical"
    );

    let next_filesystem_tick = mtime2 + Duration::from_secs(1);
    poll_clock_after(next_filesystem_tick, MTIME_TIMEOUT);
    write_ir_cache(&paths, OTHER).expect("write new content");
    let mtime3 = poll_mtime_until(&paths.ir_path, MTIME_TIMEOUT, |mtime| mtime > mtime2);

    assert!(mtime3 > mtime2, "mtime should advance when content changes");
}
