//! Man page file writer using `cap_std` for filesystem operations.

#![allow(
    clippy::too_many_arguments,
    reason = "writer needs path components for nesting"
)]
#![allow(
    clippy::option_if_let_else,
    reason = "match expression clearer for filename"
)]

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use std::io::Write;

use crate::error::OrthohelpError;

/// Writes man page content to the appropriate file path.
///
/// Creates the directory structure `man/man<section>/` and writes
/// `<name>.<section>` or `<name>-<subcommand>.<section>` for split pages.
pub fn write_man_page(
    out_dir: &Utf8Path,
    name: &str,
    subcommand: Option<&str>,
    section: u8,
    content: &str,
) -> Result<Utf8PathBuf, OrthohelpError> {
    let dir = ensure_dir(out_dir)?;

    // Create man/man<section>/ directory
    let section_dir = format!("man/man{section}");
    dir.create_dir_all(&section_dir)
        .map_err(|io_err| OrthohelpError::Io {
            path: out_dir.join(&section_dir),
            source: io_err,
        })?;

    let section_dir_handle = dir
        .open_dir(&section_dir)
        .map_err(|io_err| OrthohelpError::Io {
            path: out_dir.join(&section_dir),
            source: io_err,
        })?;

    // Determine filename
    let filename = match subcommand {
        Some(sub) => format!("{name}-{sub}.{section}"),
        None => format!("{name}.{section}"),
    };

    let file_path = out_dir.join(&section_dir).join(&filename);

    let mut file = section_dir_handle
        .open_with(
            &filename,
            OpenOptions::new().write(true).create(true).truncate(true),
        )
        .map_err(|io_err| OrthohelpError::Io {
            path: file_path.clone(),
            source: io_err,
        })?;

    file.write_all(content.as_bytes())
        .map_err(|io_err| OrthohelpError::Io {
            path: file_path.clone(),
            source: io_err,
        })?;

    Ok(file_path)
}

fn ensure_dir(path: &Utf8Path) -> Result<Dir, OrthohelpError> {
    match Dir::open_ambient_dir(path, ambient_authority()) {
        Ok(dir) => Ok(dir),
        Err(open_err) if open_err.kind() == std::io::ErrorKind::NotFound => {
            Dir::create_ambient_dir_all(path, ambient_authority()).map_err(|io_err| {
                OrthohelpError::Io {
                    path: path.to_path_buf(),
                    source: io_err,
                }
            })?;
            Dir::open_ambient_dir(path, ambient_authority()).map_err(|io_err| OrthohelpError::Io {
                path: path.to_path_buf(),
                source: io_err,
            })
        }
        Err(open_err) => Err(OrthohelpError::Io {
            path: path.to_path_buf(),
            source: open_err,
        }),
    }
}
