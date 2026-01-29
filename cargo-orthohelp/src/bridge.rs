//! Ephemeral bridge build pipeline for `cargo-orthohelp`.

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use std::fmt::Write as FmtWrite;
use std::io::{Read, Write};
use std::process::Command;

use crate::cache::CacheKey;
use crate::error::OrthohelpError;
use crate::metadata::{OrthoConfigDependency, PackageSelection};

/// Paths used when building the ephemeral bridge crate.
pub struct BridgePaths {
    /// Root directory for the bridge crate.
    pub bridge_dir: Utf8PathBuf,
    /// Path to the generated `Cargo.toml`.
    pub manifest_path: Utf8PathBuf,
    /// Target directory for bridge build artefacts.
    pub target_dir: Utf8PathBuf,
    /// Cached IR JSON path.
    pub ir_path: Utf8PathBuf,
}

/// Inputs needed to generate the bridge crate source.
pub struct BridgeConfig {
    /// Root directory of the target package.
    pub package_root: Utf8PathBuf,
    /// Cargo package name of the target crate.
    pub package_name: String,
    /// Normalised root type path for the config.
    pub root_type: String,
    /// `ortho_config` dependency metadata for the bridge.
    pub ortho_config_dependency: OrthoConfigDependency,
}

/// Constructs bridge paths for the provided cache key.
pub fn prepare_paths(selection: &PackageSelection, cache_key: &CacheKey) -> BridgePaths {
    let bridge_dir = selection
        .target_directory
        .join("orthohelp")
        .join(cache_key.hash());
    let manifest_path = bridge_dir.join("Cargo.toml");
    let target_dir = bridge_dir.join("target");
    let ir_path = bridge_dir.join("ir.json");

    BridgePaths {
        bridge_dir,
        manifest_path,
        target_dir,
        ir_path,
    }
}

/// Loads cached IR or builds the bridge to produce fresh IR JSON.
pub fn load_or_build_ir(
    config: &BridgeConfig,
    paths: &BridgePaths,
    cache: bool,
    no_build: bool,
) -> Result<String, OrthohelpError> {
    if cache || no_build {
        if let Some(cached) = read_cached_ir(paths) {
            return cached;
        }
        if no_build {
            return Err(OrthohelpError::MissingCache(paths.ir_path.clone()));
        }
    }

    ensure_bridge_layout(paths)?;
    write_bridge_manifest(config, paths)?;
    write_bridge_main(config, paths)?;
    build_bridge(paths)?;
    let ir_json = run_bridge(paths)?;
    write_ir_cache(paths, &ir_json)?;

    Ok(ir_json)
}

fn read_cached_ir(paths: &BridgePaths) -> Option<Result<String, OrthohelpError>> {
    let dir = Dir::open_ambient_dir(&paths.bridge_dir, ambient_authority()).ok()?;
    let mut file = match dir.open("ir.json") {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            return Some(Err(OrthohelpError::Io {
                path: paths.ir_path.clone(),
                source: err,
            }));
        }
    };

    let mut buffer = String::new();
    if let Err(err) = file.read_to_string(&mut buffer) {
        return Some(Err(OrthohelpError::Io {
            path: paths.ir_path.clone(),
            source: err,
        }));
    }

    Some(Ok(buffer))
}

fn ensure_bridge_layout(paths: &BridgePaths) -> Result<(), OrthohelpError> {
    match Dir::open_ambient_dir(&paths.bridge_dir, ambient_authority()) {
        Ok(dir) => {
            dir.create_dir_all("src")
                .map_err(|io_err| OrthohelpError::Io {
                    path: paths.bridge_dir.clone(),
                    source: io_err,
                })?;
            Ok(())
        }
        Err(open_err) if open_err.kind() == std::io::ErrorKind::NotFound => {
            Dir::create_ambient_dir_all(&paths.bridge_dir, ambient_authority()).map_err(
                |io_err| OrthohelpError::Io {
                    path: paths.bridge_dir.clone(),
                    source: io_err,
                },
            )?;
            let dir = Dir::open_ambient_dir(&paths.bridge_dir, ambient_authority()).map_err(
                |io_err| OrthohelpError::Io {
                    path: paths.bridge_dir.clone(),
                    source: io_err,
                },
            )?;
            dir.create_dir_all("src")
                .map_err(|io_err| OrthohelpError::Io {
                    path: paths.bridge_dir.clone(),
                    source: io_err,
                })?;
            Ok(())
        }
        Err(open_err) => Err(OrthohelpError::Io {
            path: paths.bridge_dir.clone(),
            source: open_err,
        }),
    }
}

fn write_bridge_manifest(config: &BridgeConfig, paths: &BridgePaths) -> Result<(), OrthohelpError> {
    let mut manifest = String::from(
        "[package]\nname = \"orthohelp_bridge\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n",
    );
    manifest.push_str("[dependencies]\n");
    manifest.push_str("serde_json = \"1\"\n");

    writeln!(
        manifest,
        "{} = {{ path = \"{}\" }}",
        config.package_name,
        config.package_root.as_str()
    )
    .map_err(|_| OrthohelpError::Message("failed to render bridge manifest".to_owned()))?;

    match &config.ortho_config_dependency.path {
        Some(path) => {
            writeln!(
                manifest,
                "ortho_config = {{ path = \"{}\", version = \"{}\" }}",
                path.as_str(),
                config.ortho_config_dependency.requirement,
            )
            .map_err(|_| OrthohelpError::Message("failed to render bridge manifest".to_owned()))?;
        }
        None => {
            writeln!(
                manifest,
                "ortho_config = \"{}\"",
                config.ortho_config_dependency.requirement
            )
            .map_err(|_| OrthohelpError::Message("failed to render bridge manifest".to_owned()))?;
        }
    }

    let dir = Dir::open_ambient_dir(&paths.bridge_dir, ambient_authority()).map_err(|io_err| {
        OrthohelpError::Io {
            path: paths.bridge_dir.clone(),
            source: io_err,
        }
    })?;
    let mut file = dir
        .open_with(
            "Cargo.toml",
            OpenOptions::new().write(true).create(true).truncate(true),
        )
        .map_err(|io_err| OrthohelpError::Io {
            path: paths.manifest_path.clone(),
            source: io_err,
        })?;
    file.write_all(manifest.as_bytes())
        .map_err(|io_err| OrthohelpError::Io {
            path: paths.manifest_path.clone(),
            source: io_err,
        })?;
    Ok(())
}

fn write_bridge_main(config: &BridgeConfig, paths: &BridgePaths) -> Result<(), OrthohelpError> {
    let content = format!(
        "use ortho_config::OrthoConfigDocs;\n\nfn main() -> Result<(), Box<dyn std::error::Error>> {{\n    let metadata = <{} as OrthoConfigDocs>::get_doc_metadata();\n    serde_json::to_writer(std::io::stdout(), &metadata)?;\n    Ok(())\n}}\n",
        config.root_type
    );

    let src_dir = paths.bridge_dir.join("src");
    let dir = Dir::open_ambient_dir(&src_dir, ambient_authority()).map_err(|io_err| {
        OrthohelpError::Io {
            path: src_dir.clone(),
            source: io_err,
        }
    })?;
    let mut file = dir
        .open_with(
            "main.rs",
            OpenOptions::new().write(true).create(true).truncate(true),
        )
        .map_err(|io_err| OrthohelpError::Io {
            path: src_dir.join("main.rs"),
            source: io_err,
        })?;
    file.write_all(content.as_bytes())
        .map_err(|io_err| OrthohelpError::Io {
            path: src_dir.join("main.rs"),
            source: io_err,
        })?;
    Ok(())
}

fn build_bridge(paths: &BridgePaths) -> Result<(), OrthohelpError> {
    let output = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(paths.manifest_path.as_str())
        .arg("--target-dir")
        .arg(paths.target_dir.as_str())
        .output()
        .map_err(|io_err| OrthohelpError::Io {
            path: paths.manifest_path.clone(),
            source: io_err,
        })?;

    if output.status.success() {
        return Ok(());
    }

    let status = output.status.code().unwrap_or(-1);
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Err(OrthohelpError::BridgeBuildFailure { status, message })
}

fn run_bridge(paths: &BridgePaths) -> Result<String, OrthohelpError> {
    let exe_name = format!("orthohelp_bridge{}", std::env::consts::EXE_SUFFIX);
    let exe_path = paths.target_dir.join("debug").join(exe_name);

    let output = Command::new(exe_path.as_str())
        .output()
        .map_err(|io_err| OrthohelpError::Io {
            path: exe_path,
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

fn write_ir_cache(paths: &BridgePaths, json: &str) -> Result<(), OrthohelpError> {
    let dir = Dir::open_ambient_dir(&paths.bridge_dir, ambient_authority()).map_err(|io_err| {
        OrthohelpError::Io {
            path: paths.bridge_dir.clone(),
            source: io_err,
        }
    })?;
    let mut file = dir
        .open_with(
            "ir.json",
            OpenOptions::new().write(true).create(true).truncate(true),
        )
        .map_err(|io_err| OrthohelpError::Io {
            path: paths.ir_path.clone(),
            source: io_err,
        })?;
    file.write_all(json.as_bytes())
        .map_err(|io_err| OrthohelpError::Io {
            path: paths.ir_path.clone(),
            source: io_err,
        })?;
    Ok(())
}
