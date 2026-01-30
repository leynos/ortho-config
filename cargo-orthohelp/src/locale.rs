//! Locale selection and Fluent resource loading for `cargo-orthohelp`.

use std::io::Read;
use std::str::FromStr;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use ortho_config::{FluentLocalizer, FluentLocalizerError, LanguageIdentifier};

use crate::cli::Args;
use crate::error::OrthohelpError;
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
    let has_resources = !leaked.is_empty();
    let base_builder = FluentLocalizer::builder(locale.clone());
    let builder = if has_resources {
        base_builder.with_consumer_resources(leaked.clone())
    } else {
        base_builder
    };
    build_localizer_with_fallback(locale, leaked, has_resources, builder)
}

/// Loads Fluent resources from `locales/<locale>` in the target package.
pub fn load_consumer_resources(
    package_root: &Utf8Path,
    locale: &LanguageIdentifier,
) -> Result<Vec<String>, OrthohelpError> {
    let locale_dir = package_root.join("locales").join(locale.to_string());
    let Some(dir) = open_optional_dir(&locale_dir)? else {
        return Ok(Vec::new());
    };

    let mut files = Vec::new();
    for entry_result in dir.read_dir(".").map_err(|err| OrthohelpError::Io {
        path: locale_dir.clone(),
        source: err,
    })? {
        let entry = entry_result.map_err(|err| OrthohelpError::Io {
            path: locale_dir.clone(),
            source: err,
        })?;
        let file_name = entry.file_name().map_err(|err| OrthohelpError::Io {
            path: locale_dir.clone(),
            source: err,
        })?;
        if !Utf8Path::new(&file_name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ftl"))
        {
            continue;
        }
        files.push(Utf8PathBuf::from(file_name));
    }

    files.sort();
    let mut resources = Vec::new();
    for file in files {
        let mut handle = dir.open(&file).map_err(|err| OrthohelpError::Io {
            path: locale_dir.clone(),
            source: err,
        })?;
        let mut buffer = String::new();
        handle
            .read_to_string(&mut buffer)
            .map_err(|err| OrthohelpError::Io {
                path: locale_dir.clone(),
                source: err,
            })?;
        resources.push(buffer);
    }

    Ok(resources)
}

fn parse_locales(values: &[String]) -> Result<Vec<LanguageIdentifier>, OrthohelpError> {
    let mut output = Vec::new();
    for value in values {
        let locale =
            LanguageIdentifier::from_str(value).map_err(|err| OrthohelpError::InvalidLocale {
                value: value.clone(),
                message: err.to_string(),
            })?;
        if output.iter().any(|existing| existing == &locale) {
            continue;
        }
        output.push(locale);
    }
    Ok(output)
}

fn discover_locale_dirs(package_root: &Utf8Path) -> Result<Vec<String>, OrthohelpError> {
    let locales_root = package_root.join("locales");
    let Some(dir) = open_optional_dir(&locales_root)? else {
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

fn leak_resources(resources: Vec<String>) -> Vec<&'static str> {
    // Fluent requires resource strings with a `'static` lifetime; `cargo-orthohelp`
    // is a short-lived CLI, so leaking these allocations is acceptable here.
    resources
        .into_iter()
        .map(|resource| Box::leak(resource.into_boxed_str()) as &'static str)
        .collect()
}

fn build_localizer_with_fallback(
    locale: &LanguageIdentifier,
    leaked: Vec<&'static str>,
    has_resources: bool,
    builder: ortho_config::FluentLocalizerBuilder,
) -> Result<FluentLocalizer, OrthohelpError> {
    match builder.try_build() {
        Ok(localizer) => Ok(localizer),
        Err(FluentLocalizerError::UnsupportedLocale { .. }) if has_resources => {
            FluentLocalizer::builder(locale.clone())
                .disable_defaults()
                .with_consumer_resources(leaked)
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

fn open_optional_dir(path: &Utf8Path) -> Result<Option<Dir>, OrthohelpError> {
    match Dir::open_ambient_dir(path, ambient_authority()) {
        Ok(dir) => Ok(Some(dir)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(OrthohelpError::Io {
            path: path.to_path_buf(),
            source: err,
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for locale resolution helpers.

    use super::*;
    use camino::Utf8PathBuf;
    use rstest::rstest;

    #[rstest]
    fn defaults_to_en_us() {
        let args = Args {
            package: None,
            bin: None,
            is_lib: false,
            root_type: None,
            locale: Vec::new(),
            should_use_all_locales: false,
            out_dir: None,
            cache: crate::cli::CacheArgs {
                should_cache: false,
                should_skip_build: false,
            },
            format: crate::cli::OutputFormat::Ir,
        };
        let selection = PackageSelection {
            package_name: "demo".to_owned(),
            package_root: Utf8PathBuf::from("."),
            target_directory: Utf8PathBuf::from("."),
            root_type: "demo::Config".to_owned(),
            locales: None,
            ortho_config_dependency: crate::metadata::OrthoConfigDependency {
                requirement: "^0.7.0".to_owned(),
                path: None,
            },
        };

        let locales = resolve_locales(&args, &selection).expect("resolve locales");
        assert_eq!(locales.len(), 1);
        let locale = locales.first().expect("expected one locale");
        assert_eq!(locale.to_string(), "en-US");
    }

    #[rstest]
    fn uses_metadata_locales_when_requested() {
        let args = Args {
            package: None,
            bin: None,
            is_lib: false,
            root_type: None,
            locale: Vec::new(),
            should_use_all_locales: true,
            out_dir: None,
            cache: crate::cli::CacheArgs {
                should_cache: false,
                should_skip_build: false,
            },
            format: crate::cli::OutputFormat::Ir,
        };
        let selection = PackageSelection {
            package_name: "demo".to_owned(),
            package_root: Utf8PathBuf::from("."),
            target_directory: Utf8PathBuf::from("."),
            root_type: "demo::Config".to_owned(),
            locales: Some(vec!["fr-FR".to_owned()]),
            ortho_config_dependency: crate::metadata::OrthoConfigDependency {
                requirement: "^0.7.0".to_owned(),
                path: None,
            },
        };

        let locales = resolve_locales(&args, &selection).expect("resolve locales");
        assert_eq!(locales.len(), 1);
        let locale = locales.first().expect("expected one locale");
        assert_eq!(locale.to_string(), "fr-FR");
    }
}
