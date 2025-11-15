//! Sample configuration and file helpers for the behavioural harness.
use super::config::{ensure_simple_filename, parse_extends, ConfigCopyParams, SampleConfigError};
use super::{Harness, CONFIG_FILE};
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use cap_std::fs::Dir;
use std::collections::BTreeSet;

impl Harness {
    pub(crate) fn write_config(&self, contents: &str) -> Result<()> {
        let dir = self
            .scenario_dir()
            .context("open hello_world workdir for config write")?;
        dir.write(CONFIG_FILE, contents)
            .with_context(|| format!("write {CONFIG_FILE}"))?;
        Ok(())
    }

    pub(crate) fn write_named_file<S>(&self, name: S, contents: &str) -> Result<()>
    where
        S: AsRef<str>,
    {
        let name_ref = name.as_ref();
        ensure_simple_filename(name_ref)?;
        let dir = self
            .scenario_dir()
            .context("open hello_world workdir for named config write")?;
        dir.write(name_ref, contents)
            .with_context(|| format!("write hello_world named config {name_ref}"))?;
        Ok(())
    }

    pub(crate) fn write_sample_config<S>(&self, sample: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        self.try_write_sample_config(sample).map_err(Into::into)
    }

    pub(crate) fn try_write_sample_config<S>(&self, sample: S) -> Result<(), SampleConfigError>
    where
        S: AsRef<str>,
    {
        let sample_name = sample.as_ref();
        ensure_simple_filename(sample_name)?;
        let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let config_dir = manifest_dir.join("config");
        let source = Dir::open_ambient_dir(config_dir.as_std_path(), cap_std::ambient_authority())
            .map_err(|source| SampleConfigError::OpenConfigDir {
                path: config_dir.as_str().to_owned(),
                source,
            })?;
        let mut visited = BTreeSet::new();
        let params = ConfigCopyParams {
            source: &source,
            source_name: sample_name,
            target_name: CONFIG_FILE,
        };
        self.copy_sample_config(params, &mut visited)
    }

    pub(crate) fn copy_sample_config(
        &self,
        params: ConfigCopyParams<'_>,
        visited: &mut BTreeSet<String>,
    ) -> Result<(), SampleConfigError> {
        ensure_simple_filename(params.source_name)?;
        ensure_simple_filename(params.target_name)?;
        if !visited.insert(params.source_name.to_owned()) {
            return Ok(());
        }
        let contents = params
            .source
            .read_to_string(params.source_name)
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::NotFound {
                    SampleConfigError::OpenSample {
                        name: params.source_name.to_owned(),
                        source,
                    }
                } else {
                    SampleConfigError::ReadSample {
                        name: params.source_name.to_owned(),
                        source,
                    }
                }
            })?;
        let scenario = self
            .scenario_dir()
            .map_err(|source| SampleConfigError::WriteSample {
                name: params.target_name.to_owned(),
                source,
            })?;
        scenario
            .write(params.target_name, &contents)
            .map_err(|source| SampleConfigError::WriteSample {
                name: params.target_name.to_owned(),
                source,
            })?;
        for base in parse_extends(&contents) {
            let base_name = base.as_str();
            ensure_simple_filename(base_name)?;
            let base_params = ConfigCopyParams {
                source: params.source,
                source_name: base_name,
                target_name: base_name,
            };
            self.copy_sample_config(base_params, visited)?;
        }
        Ok(())
    }
}
