//! Locale selection and Fluent resource loading for `cargo-orthohelp`.

use std::io::Read;
use std::str::FromStr;

use camino::{Utf8Path, Utf8PathBuf};
use ortho_config::{FluentLocalizer, FluentLocalizerError, LanguageIdentifier};

use crate::cli::Args;
use crate::error::OrthohelpError;
use crate::fs_helpers::open_optional_dir;
use crate::metadata::PackageSelection;

/// Resolves the locales to generate based on CLI flags and package metadata.
pub fn resolve_locales(
    args: &Args,
    selection: &PackageSelection,
) -> Result<Vec<LanguageIdentifier>, OrthohelpError> {
    if !args.locale.is_empty() {
        return parse_locales(&args.locale);
    }

    if args.should_use_all_locales {
        let configured = selection.locales.clone().unwrap_or_default();
        if !configured.is_empty() {
            return parse_locales(&configured);
        }
        let discovered = discover_locale_dirs(&selection.package_root)?;
        if !discovered.is_empty() {
            return parse_locales(&discovered);
        }
    }

    parse_locales(&["en-US".to_owned()])
}

/// Builds a `FluentLocalizer` from embedded defaults and consumer resources.
pub fn build_localizer(
    locale: &LanguageIdentifier,
    resources: Vec<String>,
) -> Result<FluentLocalizer, OrthohelpError> {
    let leaked = leak_resources(resources);
    build_localizer_from_resources(locale, &leaked)
}

/// Builds a localizer from already-leaked consumer resource strings.
///
/// This is the second half of [`build_localizer`]: resource loading and
/// lifetime extension happen there, while this function owns the builder
/// configuration and its fallback rules.
fn build_localizer_from_resources(
    locale: &LanguageIdentifier,
    leaked: &[&'static str],
) -> Result<FluentLocalizer, OrthohelpError> {
    let has_resources = !leaked.is_empty();
    let base_builder = FluentLocalizer::builder(locale.clone());
    let builder = if has_resources {
        base_builder.with_consumer_resources(leaked.iter().copied())
    } else {
        base_builder
    };
    build_localizer_with_fallback(locale, leaked, has_resources, builder)
}

/// Loads Fluent resources from the package directory for `locale`.
///
/// Discovery preserves the directory names an author chose, so a canonical
/// `en-GB` identifier can name a `locales/en-gb` directory. The exact spelling
/// is preferred; otherwise the first case-insensitive match in sorted
/// discovery order wins, which keeps the choice deterministic on a
/// case-sensitive filesystem.
pub fn load_consumer_resources(
    package_root: &Utf8Path,
    locale: &LanguageIdentifier,
) -> Result<Vec<String>, OrthohelpError> {
    let Some(directory_name) = resolve_locale_directory(package_root, &locale.to_string())? else {
        return Ok(Vec::new());
    };
    let locale_dir = package_root.join("locales").join(directory_name);
    let Some(dir) = open_optional_dir(locale_dir.as_path())? else {
        return Ok(Vec::new());
    };

    let mut files = ftl_file_names(&dir, &locale_dir)?;
    files.sort();
    files
        .iter()
        .map(|file| read_resource(&dir, file, &locale_dir))
        .collect()
}

/// Resolves the on-disk directory name for a requested locale.
///
/// Returns `None` when no directory matches. Only the directory name is
/// tolerant of case; the locale identifier itself keeps its canonical form for
/// matching, logging, and localizer construction.
fn resolve_locale_directory(
    package_root: &Utf8Path,
    requested: &str,
) -> Result<Option<String>, OrthohelpError> {
    let exact = package_root.join("locales").join(requested);
    if open_optional_dir(exact.as_path())?.is_some() {
        return Ok(Some(requested.to_owned()));
    }
    let discovered = discover_locale_dirs(package_root)?;
    if let Some(name) = discovered
        .iter()
        .find(|name| name.eq_ignore_ascii_case(requested))
    {
        return Ok(Some(name.clone()));
    }
    let Ok(requested_identifier) = parse_locale(requested) else {
        return Ok(None);
    };
    Ok(discovered
        .into_iter()
        .find(|name| parse_locale(name).is_ok_and(|parsed| parsed == requested_identifier)))
}

/// List the `.ftl` files directly inside `dir`, ignoring other entries.
///
/// Every failure is reported against `locale_dir`, the directory being listed.
fn ftl_file_names(
    dir: &cap_std::fs_utf8::Dir,
    locale_dir: &Utf8Path,
) -> Result<Vec<Utf8PathBuf>, OrthohelpError> {
    let io_error = |source: std::io::Error| OrthohelpError::Io {
        path: locale_dir.to_path_buf(),
        source,
    };
    let mut files = Vec::new();
    for entry_result in dir.read_dir(".").map_err(io_error)? {
        let file_name = entry_result
            .map_err(io_error)?
            .file_name()
            .map_err(io_error)?;
        let is_ftl = Utf8Path::new(&file_name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ftl"));
        if is_ftl {
            files.push(Utf8PathBuf::from(file_name));
        }
    }
    Ok(files)
}

/// Read one Fluent resource, reporting failures against `locale_dir`.
fn read_resource(
    dir: &cap_std::fs_utf8::Dir,
    file: &Utf8Path,
    locale_dir: &Utf8Path,
) -> Result<String, OrthohelpError> {
    let io_error = |source: std::io::Error| OrthohelpError::Io {
        path: locale_dir.to_path_buf(),
        source,
    };
    let mut buffer = String::new();
    dir.open(file)
        .map_err(io_error)?
        .read_to_string(&mut buffer)
        .map_err(io_error)?;
    Ok(buffer)
}

/// Parses locale names and keeps the first occurrence of each normalized identifier.
///
/// Parsing happens before duplicate comparison, so equivalent spellings resolve
/// to one `LanguageIdentifier` while preserving the caller's requested order.
/// An invalid value fails the whole selection rather than being skipped.
fn parse_locales(values: &[String]) -> Result<Vec<LanguageIdentifier>, OrthohelpError> {
    let mut output = Vec::new();
    for value in values {
        let locale = parse_locale(value)?;
        if output.iter().any(|existing| existing == &locale) {
            continue;
        }
        output.push(locale);
    }
    Ok(output)
}

/// Parses one locale value, keeping the caller's message for diagnostics.
pub(crate) fn parse_locale(value: &str) -> Result<LanguageIdentifier, OrthohelpError> {
    LanguageIdentifier::from_str(value).map_err(|err| OrthohelpError::InvalidLocale {
        value: value.to_owned(),
        message: err.to_string(),
    })
}

/// Lists package locale subdirectories in sorted order for `--all-locales` fallback.
///
/// A missing `locales` directory means no locales were discovered; directory
/// enumeration and entry-inspection errors retain the directory path in the
/// returned error.
pub(crate) fn discover_locale_dirs(package_root: &Utf8Path) -> Result<Vec<String>, OrthohelpError> {
    let locales_root = package_root.join("locales");
    let Some(dir) = open_optional_dir(locales_root.as_path())? else {
        return Ok(Vec::new());
    };

    let mut locales = Vec::new();
    for entry_result in dir.read_dir(".").map_err(|err| OrthohelpError::Io {
        path: locales_root.clone(),
        source: err,
    })? {
        let entry = entry_result.map_err(|err| OrthohelpError::Io {
            path: locales_root.clone(),
            source: err,
        })?;
        let file_name = entry.file_name().map_err(|err| OrthohelpError::Io {
            path: locales_root.clone(),
            source: err,
        })?;
        let file_type = entry.file_type().map_err(|err| OrthohelpError::Io {
            path: locales_root.clone(),
            source: err,
        })?;
        if !file_type.is_dir() {
            continue;
        }
        locales.push(file_name);
    }

    locales.sort();
    Ok(locales)
}

/// Gives resource strings the process-lifetime references required by Fluent.
///
/// The allocations are intentionally leaked because the builder accepts only
/// `&'static str`; `cargo-orthohelp` is a short-lived command-line process, so
/// those strings are reclaimed when the process exits.
fn leak_resources(resources: Vec<String>) -> Vec<&'static str> {
    // Fluent requires resource strings with a `'static` lifetime; `cargo-orthohelp`
    // is a short-lived CLI, so leaking these allocations is acceptable here.
    resources
        .into_iter()
        .map(|resource| Box::leak(resource.into_boxed_str()) as &'static str)
        .collect()
}

/// Builds with embedded defaults, retrying consumer-only resources if needed.
///
/// The retry is limited to `UnsupportedLocale` when consumer resources exist:
/// this permits a package-specific locale absent from the embedded defaults
/// while preserving all other builder failures. If consumer resources are
/// absent, or the retry fails, the error is returned with the locale context.
fn build_localizer_with_fallback(
    locale: &LanguageIdentifier,
    leaked: &[&'static str],
    has_resources: bool,
    builder: ortho_config::FluentLocalizerBuilder,
) -> Result<FluentLocalizer, OrthohelpError> {
    match builder.try_build() {
        Ok(localizer) => Ok(localizer),
        Err(FluentLocalizerError::UnsupportedLocale { .. }) if has_resources => {
            FluentLocalizer::builder(locale.clone())
                .disable_defaults()
                .with_consumer_resources(leaked.iter().copied())
                .try_build()
                .map_err(|err| {
                    OrthohelpError::Message(format!(
                        "failed to build localizer for {locale}: {err}"
                    ))
                })
        }
        Err(err) => Err(OrthohelpError::Message(format!(
            "failed to build localizer for {locale}: {err}"
        ))),
    }
}

#[cfg(test)]
#[path = "locale_tests.rs"]
mod tests;
