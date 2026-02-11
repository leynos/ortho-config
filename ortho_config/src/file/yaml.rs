//! YAML provider support backed by `serde-saphyr`.

use figment::{
    Metadata, Profile, Provider,
    error::Kind,
    value::{Dict, Value as FigmentValue},
};
use serde_saphyr::Options;

use std::path::PathBuf;

#[derive(Debug, Clone)]
enum YamlInput {
    File,
    Inline(String),
}

#[derive(Debug, Clone)]
/// Figment provider that reads YAML using `serde-saphyr`.
pub struct SaphyrYaml {
    path: PathBuf,
    input: YamlInput,
    profile: Option<Profile>,
}

impl SaphyrYaml {
    /// Construct a provider that reads configuration from `path` when queried.
    #[must_use]
    pub fn file<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            path: path.into(),
            input: YamlInput::File,
            profile: None,
        }
    }

    /// Construct a provider from in-memory YAML, primarily used by tests.
    #[must_use]
    pub fn string<P, S>(path: P, contents: S) -> Self
    where
        P: Into<PathBuf>,
        S: Into<String>,
    {
        Self {
            path: path.into(),
            input: YamlInput::Inline(contents.into()),
            profile: None,
        }
    }

    /// Override the profile this provider emits values into.
    #[must_use]
    pub fn profile<P: Into<Profile>>(mut self, profile: P) -> Self {
        self.profile = Some(profile.into());
        self
    }

    /// Read the provider input into a `String`.
    fn read_contents(&self) -> std::io::Result<String> {
        match &self.input {
            YamlInput::File => std::fs::read_to_string(&self.path),
            YamlInput::Inline(contents) => Ok(contents.clone()),
        }
    }

    /// Parse YAML contents into a Figment `Value` using strict boolean semantics.
    fn parse_value(contents: &str) -> Result<FigmentValue, serde_saphyr::Error> {
        serde_saphyr::from_str_with_options(
            contents,
            Options {
                strict_booleans: true,
                ..Options::default()
            },
        )
    }
}

impl Provider for SaphyrYaml {
    fn metadata(&self) -> Metadata {
        Metadata::from("Saphyr YAML", self.path.as_path())
    }

    fn data(&self) -> Result<std::collections::BTreeMap<Profile, Dict>, figment::Error> {
        let contents = self.read_contents().map_err(|err| {
            figment::Error::from(format!("failed to read {}: {err}", self.path.display()))
        })?;
        let value = Self::parse_value(&contents).map_err(|err| {
            figment::Error::from(Kind::Message(format!(
                "failed to parse {}: {err}",
                self.path.display()
            )))
        })?;
        let actual = value.to_actual();
        let dict = value
            .into_dict()
            .ok_or_else(|| figment::Error::from(Kind::InvalidType(actual, "map".into())))?;
        let profile = self.profile.clone().unwrap_or(Profile::Default);
        Ok(profile.collect(dict))
    }
}
