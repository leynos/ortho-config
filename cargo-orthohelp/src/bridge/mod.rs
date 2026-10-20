//! Ephemeral bridge build pipeline for `cargo-orthohelp`.

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use std::fmt::Write as FmtWrite;
use std::io::{Read, Write};
use std::process::Command;

use crate::cache::CacheKey;
use crate::error::OrthohelpError;
use crate::fs_helpers::open_optional_dir;
use crate::metadata::{OrthoConfigDependency, PackageSelection};
#[path = "../bridge_build.rs"]
mod bridge_build;
#[path = "../bridge_build_environment.rs"]
mod bridge_build_environment;
#[path = "../bridge_workspace.rs"]
mod workspace;
mod telemetry;

use bridge_build_environment::BuildEnvironment;
use workspace::BridgeWorkspace;

/// Stable IR cache identity and Cargo-owned compiler state.
pub struct BridgeCache {
    /// Directory containing the cached IR and private invocation directories.
    pub cache_dir: Utf8PathBuf,
    /// Cargo-resolved target shared across fingerprints and invocations.
    pub target_dir: Utf8PathBuf,
    /// Stable cached IR JSON path.
    pub ir_path: Utf8PathBuf,
}

/// Private generated crate paths for exactly one invocation.
pub struct BridgePaths {
    /// Root directory of the invocation-owned bridge crate.
    pub bridge_dir: Utf8PathBuf,
    /// Path to its generated `Cargo.toml`.
    pub manifest_path: Utf8PathBuf,
    /// Independently selected Cargo target directory for build artefacts.
    pub target_dir: Utf8PathBuf,
    /// Generated target name that is distinct from a workspace's default bin.
    pub binary_name: String,
}

/// Inputs needed to generate the bridge crate source.
pub struct BridgeConfig {
    /// Workspace owning the dependency lockfile used to seed private builds.
    pub workspace_root: Utf8PathBuf,
    /// Root directory of the target package.
    pub package_root: Utf8PathBuf,
    /// Cargo package name of the target crate.
    pub package_name: String,
    /// Normalized root type path for the config.
    pub root_type: String,
    /// `ortho_config` dependency metadata for the bridge.
    pub ortho_config_dependency: OrthoConfigDependency,
}

/// Constructs bridge paths for the provided cache key.
pub fn prepare_cache(selection: &PackageSelection, cache_key: &CacheKey) -> BridgeCache {
    let cache_key_hash = cache_key.hash();
    let cache_dir = selection
        .target_directory
        .join("orthohelp")
        .join(cache_key_hash);
    BridgeCache {
        ir_path: cache_dir.join("ir.json"),
        cache_dir,
        target_dir: selection.target_directory.clone(),
    }
}

/// Loads cached IR or builds the bridge to produce fresh IR JSON.
pub fn load_or_build_ir(
    config: &BridgeConfig,
    paths: &BridgeCache,
    should_use_cache: bool,
    should_skip_build: bool,
) -> Result<String, OrthohelpError> {
    if should_use_cache || should_skip_build {
        if let Some(cached) = read_cached_ir(paths)? {
            tracing::debug!(
                cache_dir = %paths.cache_dir,
                ir_path = %paths.ir_path,
                "reusing cached bridge IR"
            );
            return Ok(cached);
        }
        if should_skip_build {
            return Err(OrthohelpError::MissingCache(paths.ir_path.clone()));
        }
    }

    let workspace = BridgeWorkspace::new(paths, &config.package_root)?;
    let private_paths = &workspace.paths;
    ensure_bridge_layout(private_paths)?;
    workspace.seed_lockfile(config)?;
    write_bridge_manifest(config, private_paths)?;
    write_bridge_main(config, private_paths)?;
    let environment = BuildEnvironment::from_process();
    let executable = bridge_build::build_and_snapshot(
        &mut build_bridge_command(private_paths, &environment),
        &private_paths.target_dir,
        &private_paths.bridge_dir.join("run"),
        &private_paths.binary_name,
    )?;
    let ir_json = run_bridge(&executable)?;
    write_ir_cache(paths, &ir_json)?;

    Ok(ir_json)
}

/// Reads the cached IR JSON from the bridge directory, returning `None` when
/// no cache file exists.
fn read_cached_ir(paths: &BridgeCache) -> Result<Option<String>, OrthohelpError> {
    let Some(dir) = open_optional_dir(paths.cache_dir.as_path())? else {
        return Ok(None);
    };
    let mut file = match dir.open("ir.json") {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(OrthohelpError::Io {
                path: paths.ir_path.clone(),
                source: err,
            });
        }
    };

    let mut buffer = String::new();
    file.read_to_string(&mut buffer)
        .map_err(|err| OrthohelpError::Io {
            path: paths.ir_path.clone(),
            source: err,
        })?;

    Ok(Some(buffer))
}

/// Creates the bridge crate directory tree (bridge root and `src/`) if absent.
fn ensure_bridge_layout(paths: &BridgePaths) -> Result<(), OrthohelpError> {
    Dir::create_ambient_dir_all(&paths.bridge_dir, ambient_authority()).map_err(|io_err| {
        OrthohelpError::Io {
            path: paths.bridge_dir.clone(),
            source: io_err,
        }
    })?;
    let dir = open_bridge_dir(paths)?;
    ensure_bridge_src(&dir, paths)?;
    Ok(())
}

/// Appends the `ortho_config` dependency line to the bridge manifest.
fn append_ortho_config_dependency(
    manifest: &mut String,
    dependency: &OrthoConfigDependency,
) -> Result<(), OrthohelpError> {
    let render = match &dependency.path {
        Some(path) => writeln!(
            manifest,
            "ortho_config = {{ path = {:?}, version = \"{}\" }}",
            path.as_str(),
            dependency.requirement,
        ),
        None => writeln!(manifest, "ortho_config = \"{}\"", dependency.requirement),
    };
    render.map_err(|_| OrthohelpError::Message("failed to render bridge manifest".to_owned()))
}

/// Generates and writes the ephemeral bridge `Cargo.toml`.
fn write_bridge_manifest(config: &BridgeConfig, paths: &BridgePaths) -> Result<(), OrthohelpError> {
    let mut manifest = format!(
        concat!(
            "[package]\n",
            "name = \"orthohelp_bridge\"\n",
            "version = \"0.1.0\"\n",
            "edition = \"2024\"\n",
            "autobins = false\n",
            "publish = false\n",
            "\n",
            "[[bin]]\n",
            "name = {:?}\n",
            "path = \"src/main.rs\"\n",
            "\n",
            "[workspace]\n",
            "\n",
            "[dependencies]\n",
            "serde_json = \"1\"\n",
        ),
        paths.binary_name
    );

    writeln!(
        manifest,
        "{} = {{ path = {:?} }}",
        config.package_name,
        config.package_root.as_str()
    )
    .map_err(|_| OrthohelpError::Message("failed to render bridge manifest".to_owned()))?;

    append_ortho_config_dependency(&mut manifest, &config.ortho_config_dependency)?;

    let mut file = open_bridge_file(paths, "Cargo.toml", &paths.manifest_path)?;
    file.write_all(manifest.as_bytes())
        .map_err(|io_err| OrthohelpError::Io {
            path: paths.manifest_path.clone(),
            source: io_err,
        })?;
    Ok(())
}

/// Generates and writes `src/main.rs` for the ephemeral bridge crate.
fn write_bridge_main(config: &BridgeConfig, paths: &BridgePaths) -> Result<(), OrthohelpError> {
    let content = format!(
        concat!(
            "use ortho_config::docs::OrthoConfigDocs;\n",
            "\n",
            "fn main() -> Result<(), Box<dyn std::error::Error>> {{\n",
            "    let metadata = <{} as OrthoConfigDocs>::get_doc_metadata();\n",
            "    serde_json::to_writer(std::io::stdout(), &metadata)?;\n",
            "    Ok(())\n",
            "}}\n",
        ),
        config.root_type
    );

    let src_dir = paths.bridge_dir.join("src");
    let dir = Dir::open_ambient_dir(&src_dir, ambient_authority()).map_err(|io_err| {
        OrthohelpError::Io {
            path: src_dir.clone(),
            source: io_err,
        }
    })?;
    let main_rs = src_dir.join("main.rs");
    let mut file = dir
        .open_with(
            "main.rs",
            OpenOptions::new().write(true).create(true).truncate(true),
        )
        .map_err(|io_err| OrthohelpError::Io {
            path: main_rs.clone(),
            source: io_err,
        })?;
    file.write_all(content.as_bytes())
        .map_err(|io_err| OrthohelpError::Io {
            path: main_rs,
            source: io_err,
        })?;
    Ok(())
}

/// Build with the caller's wrapper, job, warning, and instrumentation policy.
///
/// Cargo resolves configuration from the original working directory. Keeping
/// the compiler flags also lets compatible dependency fingerprints be reused.
fn build_bridge_command(paths: &BridgePaths, environment: &BuildEnvironment) -> Command {
    let mut command = Command::new("cargo");
    command
        .args(["build", "--message-format=json-render-diagnostics"])
        .arg("--bin")
        .arg(&paths.binary_name)
        .arg("--manifest-path")
        .arg(paths.manifest_path.as_str())
        .arg("--target-dir")
        .arg(paths.target_dir.as_str());
    environment.apply(&mut command);
    command
}

/// Executes the compiled bridge binary and returns its stdout as a JSON string.
fn run_bridge(executable: &camino::Utf8Path) -> Result<String, OrthohelpError> {
    let output = Command::new(executable.as_str())
        .output()
        .map_err(|io_err| OrthohelpError::Io {
            path: executable.to_path_buf(),
            source: io_err,
        })?;

    if !output.status.success() {
        let status = output.status.code().unwrap_or(-1);
        let message = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(OrthohelpError::BridgeExecutionFailure { status, message });
    }

    let json = String::from_utf8_lossy(&output.stdout).to_string();
    let value: serde_json::Value = serde_json::from_str(&json)?;
    serde_json::to_string_pretty(&value).map_err(OrthohelpError::IrJson)
}

/// Writes `json` to the cached IR file, skipping the write when the file
/// already contains identical content.
///
/// Idempotent writes preserve the file's modification time, keeping
/// mtime-based or content-based cache-validity checks stable across repeated
/// invocations even when path-normalization differences cause `read_cached_ir`
/// to return `None` spuriously.
fn write_ir_cache(paths: &BridgeCache, json: &str) -> Result<(), OrthohelpError> {
    // Skip writing if the existing cache file already holds identical content.
    // This preserves the file's mtime, making cache-validity checks robust
    // against spurious cache misses (e.g. Windows path-normalization
    // differences that cause read_cached_ir to return Ok(None) even when the
    // file exists) and prevents unnecessary I/O on repeated invocations.
    if let Ok(Some(existing)) = read_cached_ir(paths)
        && existing == json
    {
        telemetry::ir_cache_unchanged(paths);
        return Ok(());
    }
    let mut file =
        tempfile::NamedTempFile::new_in(paths.cache_dir.as_std_path()).map_err(|source| {
            OrthohelpError::Io {
                path: paths.cache_dir.clone(),
                source,
            }
        })?;
    telemetry::ir_cache_write(paths, json.len());
    file.write_all(json.as_bytes())
        .map_err(|io_err| OrthohelpError::Io {
            path: paths.ir_path.clone(),
            source: io_err,
        })?;
    file.persist(paths.ir_path.as_std_path())
        .map_err(|error| OrthohelpError::Io {
            path: paths.ir_path.clone(),
            source: error.error,
        })?;
    Ok(())
}

/// Opens the bridge root directory using cap-std ambient authority.
fn open_bridge_dir(paths: &BridgePaths) -> Result<Dir, OrthohelpError> {
    Dir::open_ambient_dir(&paths.bridge_dir, ambient_authority()).map_err(|io_err| {
        OrthohelpError::Io {
            path: paths.bridge_dir.clone(),
            source: io_err,
        }
    })
}

/// Creates the `src/` subdirectory inside the bridge root if it does not
/// already exist.
fn ensure_bridge_src(dir: &Dir, paths: &BridgePaths) -> Result<(), OrthohelpError> {
    dir.create_dir_all("src")
        .map_err(|io_err| OrthohelpError::Io {
            path: paths.bridge_dir.clone(),
            source: io_err,
        })
}

/// Opens a file inside the bridge root for writing, creating or truncating it.
fn open_bridge_file(
    paths: &BridgePaths,
    relative: &str,
    path: &Utf8PathBuf,
) -> Result<cap_std::fs_utf8::File, OrthohelpError> {
    let dir = open_bridge_dir(paths)?;
    dir.open_with(
        relative,
        OpenOptions::new().write(true).create(true).truncate(true),
    )
    .map_err(|io_err| OrthohelpError::Io {
        path: path.clone(),
        source: io_err,
    })
}

#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
