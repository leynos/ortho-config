//! `PowerShell` help generator for `cargo-orthohelp`.

mod about;
mod maml;
mod manifest;
mod types;
mod wrapper;
mod writer;

pub use types::{PowerShellConfig, PowerShellOutput};

use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;

use crate::error::OrthohelpError;
use crate::ir::LocalizedDocMetadata;

struct ModuleWriter<'a> {
    root_dir: &'a Dir,
    module_root: &'a Utf8PathBuf,
}

impl<'a> ModuleWriter<'a> {
    const fn new(root_dir: &'a Dir, module_root: &'a Utf8PathBuf) -> Self {
        Self {
            root_dir,
            module_root,
        }
    }

    const fn module_root(&self) -> &Utf8PathBuf {
        self.module_root
    }

    fn write_file(
        &self,
        path: &Utf8PathBuf,
        content: &str,
        include_bom: bool,
    ) -> Result<Utf8PathBuf, OrthohelpError> {
        let relative = path.strip_prefix(self.module_root).map_err(|_| {
            OrthohelpError::Message("module path is not within module root".to_owned())
        })?;
        writer::write_crlf_text(
            self.root_dir,
            &writer::TextWriteRequest {
                root: self.module_root,
                relative_path: relative,
                content,
                include_bom,
            },
        )
    }

    fn ensure_subdir(&self, target: &Utf8PathBuf) -> Result<(), OrthohelpError> {
        let relative = target.strip_prefix(self.module_root).map_err(|_| {
            OrthohelpError::Message("module path is not within module root".to_owned())
        })?;
        self.root_dir
            .create_dir_all(relative)
            .map_err(|io_err| OrthohelpError::Io {
                path: self.module_root.join(relative),
                source: io_err,
            })?;
        Ok(())
    }
}

struct GenerationContext<'a> {
    writer: ModuleWriter<'a>,
    config: &'a PowerShellConfig,
    output: PowerShellOutput,
}

impl<'a> GenerationContext<'a> {
    const fn new(writer: ModuleWriter<'a>, config: &'a PowerShellConfig) -> Self {
        Self {
            writer,
            config,
            output: PowerShellOutput::new(),
        }
    }

    fn write_core_files(&mut self, metadata: &LocalizedDocMetadata) -> Result<(), OrthohelpError> {
        let functions_to_export = build_functions_to_export(metadata, self.config);
        let bin_name = wrapper::BinName::new(self.config.bin_name.clone());
        let export_aliases = self
            .config
            .export_aliases
            .iter()
            .cloned()
            .map(wrapper::Alias::new)
            .collect::<Vec<_>>();
        let wrapper_content = wrapper::render_wrapper(
            metadata,
            &bin_name,
            &export_aliases,
            self.config.split_subcommands,
        );
        let wrapper_path = self
            .writer
            .module_root()
            .join(format!("{}.psm1", self.config.module_name));
        self.output.add_file(
            self.writer
                .write_file(&wrapper_path, &wrapper_content, false)?,
        );

        let manifest_content = manifest::render_manifest(&manifest::ManifestConfig {
            module_name: &self.config.module_name,
            module_version: &self.config.module_version,
            functions_to_export: &functions_to_export,
            aliases_to_export: &self.config.export_aliases,
            help_info_uri: self.config.help_info_uri.as_deref(),
        });
        let manifest_path = self
            .writer
            .module_root()
            .join(format!("{}.psd1", self.config.module_name));
        self.output.add_file(
            self.writer
                .write_file(&manifest_path, &manifest_content, false)?,
        );

        Ok(())
    }

    fn write_locale_files(
        &mut self,
        metadata: &LocalizedDocMetadata,
        locale_name: &str,
    ) -> Result<(), OrthohelpError> {
        let locale_dir = self.writer.module_root().join(locale_name);
        self.writer.ensure_subdir(&locale_dir)?;

        let commands = build_command_specs(metadata, self.config);
        let maml_content = maml::render_help(
            &commands,
            maml::MamlOptions {
                include_common_parameters: self.config.include_common_parameters,
            },
        );
        let help_path = locale_dir.join(format!("{}-help.xml", self.config.module_name));
        self.output
            .add_file(self.writer.write_file(&help_path, &maml_content, true)?);

        let about_content = about::render_about(metadata, &self.config.module_name);
        let about_path = locale_dir.join(format!("about_{}.help.txt", self.config.module_name));
        self.output
            .add_file(self.writer.write_file(&about_path, &about_content, false)?);

        Ok(())
    }

    fn into_output(self) -> PowerShellOutput {
        self.output
    }
}

/// Generates `PowerShell` wrapper modules and MAML help from localised metadata.
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
///     include_common_parameters: true,
///     split_subcommands: false,
///     help_info_uri: None,
///     ensure_en_us: true,
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
    let writer = ModuleWriter::new(&root_dir, &module_root);
    let mut context = GenerationContext::new(writer, config);

    context.write_core_files(root_metadata)?;

    let (locales_to_write, fallback_locale) = resolve_locales(locales, config.ensure_en_us);

    for locale in &locales_to_write {
        context.write_locale_files(locale, &locale.locale)?;
    }
    if let Some(locale) = fallback_locale {
        context.write_locale_files(locale, "en-US")?;
    }

    Ok(context.into_output())
}

fn build_functions_to_export(
    metadata: &LocalizedDocMetadata,
    config: &PowerShellConfig,
) -> Vec<String> {
    let mut functions = Vec::new();
    functions.push(config.bin_name.clone());
    if config.split_subcommands {
        for subcommand in &metadata.subcommands {
            let sub_name = subcommand
                .bin_name
                .as_deref()
                .unwrap_or(&subcommand.app_name);
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
    if config.split_subcommands {
        for subcommand in &metadata.subcommands {
            let sub_name = subcommand
                .bin_name
                .as_deref()
                .unwrap_or(&subcommand.app_name);
            commands.push(maml::CommandSpec {
                name: format!("{}_{}", config.bin_name, sub_name),
                metadata: subcommand,
            });
        }
    }
    commands
}

fn resolve_locales(
    locales: &[LocalizedDocMetadata],
    ensure_en_us: bool,
) -> (Vec<&LocalizedDocMetadata>, Option<&LocalizedDocMetadata>) {
    let mut list = Vec::new();
    let mut has_en_us = false;
    for locale in locales {
        if locale.locale == "en-US" {
            has_en_us = true;
        }
        list.push(locale);
    }

    let fallback = if ensure_en_us && !has_en_us {
        locales.first()
    } else {
        None
    };

    (list, fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{LocalizedHeadings, LocalizedSectionsMetadata};
    use rstest::rstest;

    fn minimal_doc() -> LocalizedDocMetadata {
        LocalizedDocMetadata {
            ir_version: "1.1".to_owned(),
            locale: "fr-FR".to_owned(),
            app_name: "fixture".to_owned(),
            bin_name: None,
            about: "Fixture".to_owned(),
            synopsis: None,
            sections: LocalizedSectionsMetadata {
                headings: LocalizedHeadings {
                    name: "NAME".to_owned(),
                    synopsis: "SYNOPSIS".to_owned(),
                    description: "DESCRIPTION".to_owned(),
                    options: "OPTIONS".to_owned(),
                    environment: "ENVIRONMENT".to_owned(),
                    files: "FILES".to_owned(),
                    precedence: "PRECEDENCE".to_owned(),
                    exit_status: "EXIT STATUS".to_owned(),
                    examples: "EXAMPLES".to_owned(),
                    see_also: "SEE ALSO".to_owned(),
                    commands: "COMMANDS".to_owned(),
                },
                discovery: None,
                precedence: None,
                examples: vec![],
                links: vec![],
                notes: vec![],
            },
            fields: vec![],
            subcommands: vec![],
            windows: None,
        }
    }

    #[rstest]
    fn resolve_locales_falls_back_to_en_us() {
        let locale = minimal_doc();
        let locales = vec![locale];
        let (resolved, fallback) = resolve_locales(&locales, true);
        assert_eq!(resolved.len(), 1);
        assert!(fallback.is_some());
    }
}
