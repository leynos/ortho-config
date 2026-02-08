//! `PowerShell` help generator for `cargo-orthohelp`.

mod about;
mod maml;
mod manifest;
#[cfg(test)]
mod test_fixtures;
mod text;
mod types;
mod wrapper;
mod writer;

pub use types::{PowerShellConfig, PowerShellOutput};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;

use crate::error::OrthohelpError;
use crate::ir::LocalizedDocMetadata;

struct GenerationPaths<'a> {
    root_dir: &'a Dir,
    module_root: &'a Utf8PathBuf,
}

struct LocaleWriteRequest<'a> {
    metadata: &'a LocalizedDocMetadata,
    locale_name: &'a str,
}

/// Generates `PowerShell` wrapper modules and MAML help from localized metadata.
///
/// # Errors
///
/// Returns an error if the output directory cannot be created or any help
/// artefact fails to write.
///
/// # Examples
///
/// ```no_run
/// use camino::Utf8PathBuf;
/// use cargo_orthohelp::powershell::{generate, PowerShellConfig};
/// # use cargo_orthohelp::ir::LocalizedDocMetadata;
/// # fn build_doc() -> LocalizedDocMetadata { todo!() }
/// let doc = build_doc();
/// let config = PowerShellConfig {
///     out_dir: Utf8PathBuf::from("target/orthohelp"),
///     module_name: "Demo".to_owned(),
///     module_version: "0.1.0".to_owned(),
///     bin_name: "demo".to_owned(),
///     export_aliases: Vec::new(),
///     should_include_common_parameters: true,
///     should_split_subcommands: false,
///     help_info_uri: None,
///     should_ensure_en_us: true,
/// };
/// generate(&[doc], &config)?;
/// # Ok::<(), cargo_orthohelp::error::OrthohelpError>(())
/// ```
pub fn generate(
    locales: &[LocalizedDocMetadata],
    config: &PowerShellConfig,
) -> Result<PowerShellOutput, OrthohelpError> {
    let root_metadata = locales
        .first()
        .ok_or_else(|| OrthohelpError::Message("no locales provided".to_owned()))?;

    let module_root = config.module_root();
    let root_dir = writer::ensure_dir(&module_root)?;
    let paths = GenerationPaths {
        root_dir: &root_dir,
        module_root: &module_root,
    };
    let mut output = PowerShellOutput::new();

    write_core_files(&paths, config, root_metadata, &mut output)?;

    let (locales_to_write, fallback_locale) = resolve_locales(locales, config.should_ensure_en_us);

    for locale in locales_to_write {
        let request = LocaleWriteRequest {
            metadata: locale,
            locale_name: &locale.locale,
        };
        write_locale_files(&paths, config, &request, &mut output)?;
    }

    if let Some(locale) = fallback_locale {
        let request = LocaleWriteRequest {
            metadata: locale,
            locale_name: "en-US",
        };
        write_locale_files(&paths, config, &request, &mut output)?;
    }

    Ok(output)
}

fn write_core_files(
    paths: &GenerationPaths<'_>,
    config: &PowerShellConfig,
    metadata: &LocalizedDocMetadata,
    output: &mut PowerShellOutput,
) -> Result<(), OrthohelpError> {
    let functions_to_export = build_functions_to_export(metadata, config);
    let bin_name = wrapper::BinName::new(config.bin_name.clone());
    let export_aliases = config
        .export_aliases
        .iter()
        .cloned()
        .map(wrapper::Alias::new)
        .collect::<Vec<_>>();
    let wrapper_content = wrapper::render_wrapper(
        metadata,
        &bin_name,
        &export_aliases,
        config.should_split_subcommands,
    );
    let wrapper_relative = Utf8PathBuf::from(format!("{}.psm1", config.module_name));
    output.add_file(write_module_file(
        paths,
        &wrapper_relative,
        &wrapper_content,
        false,
    )?);

    let manifest_content = manifest::render_manifest(&manifest::ManifestConfig {
        module_name: &config.module_name,
        module_version: &config.module_version,
        functions_to_export: &functions_to_export,
        aliases_to_export: &config.export_aliases,
        help_info_uri: config.help_info_uri.as_deref(),
    });
    let manifest_relative = Utf8PathBuf::from(format!("{}.psd1", config.module_name));
    output.add_file(write_module_file(
        paths,
        &manifest_relative,
        &manifest_content,
        false,
    )?);

    Ok(())
}

fn write_locale_files(
    paths: &GenerationPaths<'_>,
    config: &PowerShellConfig,
    request: &LocaleWriteRequest<'_>,
    output: &mut PowerShellOutput,
) -> Result<(), OrthohelpError> {
    let locale_dir_relative = Utf8PathBuf::from(request.locale_name);
    ensure_module_subdir(paths, &locale_dir_relative)?;

    let commands = build_command_specs(request.metadata, config);
    let maml_content = maml::render_help(
        &commands,
        maml::MamlOptions {
            should_include_common_parameters: config.should_include_common_parameters,
        },
    );
    let help_relative = locale_dir_relative.join(format!("{}-help.xml", config.module_name));
    output.add_file(write_module_file(
        paths,
        &help_relative,
        &maml_content,
        true,
    )?);

    let about_content = about::render_about(request.metadata, &config.module_name);
    let about_relative = locale_dir_relative.join(format!("about_{}.help.txt", config.module_name));
    output.add_file(write_module_file(
        paths,
        &about_relative,
        &about_content,
        false,
    )?);

    Ok(())
}

fn write_module_file(
    paths: &GenerationPaths<'_>,
    relative_path: &Utf8Path,
    content: &str,
    include_bom: bool,
) -> Result<Utf8PathBuf, OrthohelpError> {
    writer::write_crlf_text(
        paths.root_dir,
        &writer::WriteTarget {
            root: paths.module_root,
            relative_path,
        },
        content,
        include_bom,
    )
}

fn ensure_module_subdir(
    paths: &GenerationPaths<'_>,
    relative_path: &Utf8Path,
) -> Result<(), OrthohelpError> {
    paths
        .root_dir
        .create_dir_all(relative_path)
        .map_err(|io_err| OrthohelpError::Io {
            path: paths.module_root.join(relative_path),
            source: io_err,
        })?;
    Ok(())
}

fn build_functions_to_export(
    metadata: &LocalizedDocMetadata,
    config: &PowerShellConfig,
) -> Vec<String> {
    let mut functions = Vec::new();
    functions.push(config.bin_name.clone());
    if config.should_split_subcommands {
        for (sub_name, _) in iter_subcommands(metadata) {
            functions.push(format!("{}_{}", config.bin_name, sub_name));
        }
    }
    functions
}

fn build_command_specs<'a>(
    metadata: &'a LocalizedDocMetadata,
    config: &PowerShellConfig,
) -> Vec<maml::CommandSpec<'a>> {
    let mut commands = Vec::new();
    commands.push(maml::CommandSpec {
        name: config.bin_name.clone(),
        metadata,
    });
    if config.should_split_subcommands {
        for (sub_name, subcommand) in iter_subcommands(metadata) {
            commands.push(maml::CommandSpec {
                name: format!("{}_{}", config.bin_name, sub_name),
                metadata: subcommand,
            });
        }
    }
    commands
}

fn iter_subcommands(
    metadata: &LocalizedDocMetadata,
) -> impl Iterator<Item = (&str, &LocalizedDocMetadata)> {
    metadata.subcommands.iter().map(|subcommand| {
        (
            subcommand
                .bin_name
                .as_deref()
                .unwrap_or(&subcommand.app_name),
            subcommand,
        )
    })
}

fn resolve_locales(
    locales: &[LocalizedDocMetadata],
    should_ensure_en_us: bool,
) -> (&[LocalizedDocMetadata], Option<&LocalizedDocMetadata>) {
    let has_en_us = locales.iter().any(|locale| locale.locale == "en-US");
    let fallback = if should_ensure_en_us && !has_en_us {
        locales.first()
    } else {
        None
    };
    (locales, fallback)
}

#[cfg(test)]
mod tests {
    //! Unit tests for locale resolution in the `PowerShell` generator.

    use super::*;
    use crate::powershell::test_fixtures;
    use rstest::rstest;

    #[rstest]
    #[case(&["fr-FR"], true, true)]
    #[case(&["en-US"], true, false)]
    fn resolve_locales_handles_en_us_fallback(
        #[case] locale_names: &[&str],
        #[case] should_ensure_en_us: bool,
        #[case] should_have_fallback: bool,
    ) {
        let locales = locale_names
            .iter()
            .map(|locale| test_fixtures::minimal_doc(locale, "Fixture"))
            .collect::<Vec<_>>();
        let (resolved, fallback) = resolve_locales(&locales, should_ensure_en_us);
        assert_eq!(resolved.len(), locales.len());
        assert_eq!(fallback.is_some(), should_have_fallback);
    }
}
