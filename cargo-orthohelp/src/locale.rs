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
    let has_resources = !leaked.is_empty();
    let base_builder = FluentLocalizer::builder(locale.clone());
    let builder = if has_resources {
        base_builder.with_consumer_resources(leaked.iter().copied())
    } else {
        base_builder
    };
    build_localizer_with_fallback(locale, &leaked, has_resources, builder)
}

/// Loads Fluent resources from `locales/<locale>` in the target package.
pub fn load_consumer_resources(
    package_root: &Utf8Path,
    locale: &LanguageIdentifier,
) -> Result<Vec<String>, OrthohelpError> {
    let locale_dir = package_root.join("locales").join(locale.to_string());
    let Some(dir) = open_optional_dir(locale_dir.as_path())? else {
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
mod tests {
    //! Unit tests for locale resolution helpers.

    use super::*;
    use camino::Utf8PathBuf;
    use cap_std::ambient_authority;
    use cap_std::fs_utf8::Dir;
    use rstest::rstest;
    use tempfile::TempDir;

    #[rstest]
    fn defaults_to_en_us() {
        let args = base_args();
        let selection = selection_with_locales(Utf8PathBuf::from("."), None);

        let locales = resolve_locales(&args, &selection).expect("resolve locales");
        assert_eq!(locales.len(), 1);
        let locale = locales.first().expect("expected one locale");
        assert_eq!(locale.to_string(), "en-US");
    }

    #[rstest]
    fn uses_metadata_locales_when_requested() {
        let mut args = base_args();
        args.should_use_all_locales = true;
        let selection =
            selection_with_locales(Utf8PathBuf::from("."), Some(vec!["fr-FR".to_owned()]));

        let locales = resolve_locales(&args, &selection).expect("resolve locales");
        assert_eq!(locales.len(), 1);
        let locale = locales.first().expect("expected one locale");
        assert_eq!(locale.to_string(), "fr-FR");
    }

    #[rstest]
    #[case(
        vec!["ja-JP".to_owned()],
        vec!["ja-JP".to_owned()]
    )]
    #[case(
        vec!["en-US".to_owned(), "en-US".to_owned(), "fr-FR".to_owned()],
        vec!["en-US".to_owned(), "fr-FR".to_owned()]
    )]
    fn uses_cli_locales_and_dedupes(#[case] requested: Vec<String>, #[case] expected: Vec<String>) {
        let mut args = base_args();
        args.locale = requested;
        let selection = selection_with_locales(Utf8PathBuf::from("."), None);

        let locales = resolve_locales(&args, &selection).expect("resolve locales");
        let resolved = locales
            .into_iter()
            .map(|locale| locale.to_string())
            .collect::<Vec<_>>();
        assert_eq!(resolved, expected);
    }

    #[rstest]
    fn discovers_locales_when_metadata_missing() {
        let temp_dir = TempDir::new().expect("temp dir");
        let package_root =
            Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).expect("temp dir is UTF-8");
        let dir = Dir::open_ambient_dir(package_root.as_path(), ambient_authority())
            .expect("open temp dir");
        dir.create_dir_all("locales/en-US")
            .expect("create locale dir");
        dir.create_dir_all("locales/fr-FR")
            .expect("create locale dir");

        let mut args = base_args();
        args.should_use_all_locales = true;
        let selection = selection_with_locales(package_root, None);

        let locales = resolve_locales(&args, &selection).expect("resolve locales");
        let resolved = locales
            .into_iter()
            .map(|locale| locale.to_string())
            .collect::<Vec<_>>();
        assert_eq!(resolved, vec!["en-US".to_owned(), "fr-FR".to_owned()]);
    }

    fn base_args() -> Args {
        Args {
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
            man: crate::cli::ManArgs {
                section: 1,
                date: None,
                should_split_subcommands: false,
            },
            powershell: crate::cli::PowerShellArgs {
                module_name: None,
                should_split_subcommands: None,
                should_include_common_parameters: None,
                help_info_uri: None,
                should_ensure_en_us: true,
            },
        }
    }

    fn selection_with_locales(
        package_root: Utf8PathBuf,
        locales: Option<Vec<String>>,
    ) -> PackageSelection {
        PackageSelection {
            package_name: "demo".to_owned(),
            package_root: package_root.clone(),
            target_directory: package_root,
            package_version: "0.1.0".to_owned(),
            root_type: "demo::Config".to_owned(),
            locales,
            windows: None,
            ortho_config_dependency: crate::metadata::OrthoConfigDependency {
                requirement: "^0.7.0".to_owned(),
                path: None,
            },
        }
    }
}
