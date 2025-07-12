//! Support for loading configuration for individual subcommands.

#[allow(deprecated)]
use crate::merge_cli_over_defaults;
use crate::{OrthoError, load_config_file, normalize_prefix};
use clap::CommandFactory;
#[cfg(not(any(unix, target_os = "redox")))]
use directories::BaseDirs;
use figment::{Figment, providers::Env};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use uncased::Uncased;
#[cfg(any(unix, target_os = "redox"))]
use xdg::BaseDirectories;

/// Prefix used when constructing configuration paths and environment variables.
///
/// Stores the raw prefix as provided by the user alongside a normalized
/// lowercase version used for file lookups.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Prefix {
    raw: String,
    normalized: String,
}

impl Prefix {
    /// Create a new `Prefix` from `raw`. The `raw` value is kept as-is for
    /// environment variables while a normalized version is used for file paths.
    #[must_use]
    /// Creates a new `Prefix` from a raw string, storing both the original and a normalised lowercase version.
    ///
    /// The raw string is preserved for use in environment variable names, while the normalised form is used for file path lookups.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::subcommand::Prefix;
    /// let prefix = Prefix::new("MyApp");
    /// let _ = prefix;
    /// ```
    pub fn new(raw: &str) -> Self {
        Self {
            raw: raw.to_owned(),
            normalized: normalize_prefix(raw),
        }
    }

    #[must_use]
    /// Returns the normalised, lowercase form of the prefix as a string slice.
    fn as_str(&self) -> &str {
        &self.normalized
    }

    #[must_use]
    /// Returns the original, unmodified prefix string as provided by the user.
    fn raw(&self) -> &str {
        &self.raw
    }
}

/// Name of a subcommand.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CmdName(String);

impl CmdName {
    /// Create a new command name from `raw`.
    #[must_use]
    /// Creates a new `CmdName` from the provided raw string.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::subcommand::CmdName;
    /// let name = CmdName::new("my-subcommand");
    /// let _ = name;
    /// ```
    pub fn new(raw: &str) -> Self {
        Self(raw.to_owned())
    }

    #[must_use]
    /// Returns the normalised string representation of the prefix.
    fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    /// Returns the subcommand name formatted as an uppercase environment variable key.
    ///
    /// Hyphens are replaced with underscores and all characters are converted to uppercase.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ortho_config::subcommand::CmdName;
    /// let name = CmdName::new("my-cmd");
    /// let _ = name;
    /// ```
    fn env_key(&self) -> String {
        self.0.replace('-', "_").to_ascii_uppercase()
    }
}

/// Adds candidate configuration file paths with supported extensions to the provided vector.
///
/// Appends file paths with `.toml` extension, and conditionally `.json`, `.json5`, `.yaml`, and `.yml` extensions if the corresponding features are enabled. The `to_path` closure is used to construct each `PathBuf` from the filename.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// let mut paths = Vec::new();
/// paths.push(PathBuf::from("config.toml"));
/// assert!(paths.iter().any(|p| p.ends_with("config.toml")));
/// ```
fn push_candidates<F>(paths: &mut Vec<PathBuf>, base: &str, mut to_path: F)
where
    F: FnMut(String) -> PathBuf,
{
    paths.push(to_path(format!("{base}.toml")));
    #[cfg(feature = "json5")]
    for ext in ["json", "json5"] {
        paths.push(to_path(format!("{base}.{ext}")));
    }
    #[cfg(feature = "yaml")]
    for ext in ["yaml", "yml"] {
        paths.push(to_path(format!("{base}.{ext}")));
    }
}

/// Adds candidate configuration file paths under `dir` using `base` as the file stem.
///
/// The `base` string should include any desired prefix such as a leading dot.
/// Supported configuration extensions are appended and each candidate is joined
/// with `dir` before being pushed onto `paths`.
///
/// # Examples
///
/// ```rust,ignore
/// use std::path::Path;
/// let mut candidates: Vec<std::path::PathBuf> = Vec::new();
/// crate::subcommand::push_dir_candidates(Path::new("/tmp"), ".myapp", &mut candidates);
/// assert!(candidates.iter().any(|p| p.ends_with(".myapp.toml")));
/// ```
fn push_dir_candidates(dir: &Path, base: &str, paths: &mut Vec<PathBuf>) {
    push_candidates(paths, base, |f| dir.join(f));
}

fn push_home_from_env(vars: &[&str], prefix: &Prefix, paths: &mut Vec<PathBuf>) -> bool {
    for var in vars {
        if let Some(home) = std::env::var_os(var) {
            push_dir_candidates(Path::new(&home), &format!(".{}", prefix.as_str()), paths);
            return true;
        }
    }
    false
}

/// Returns a list of possible configuration file paths for a subcommand, based on the provided prefix.
///
/// The search includes standard locations such as the user's home directory, platform-specific configuration directories, and the current working directory. The function considers multiple file formats and adapts its search strategy according to the operating system and enabled features.
///
/// # Parameters
/// - `prefix`: The configuration prefix, which determines the subdirectory or filename pattern used in path generation.
///
/// # Returns
/// A vector of candidate `PathBuf`s representing possible configuration file locations.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// use ortho_config::subcommand::Prefix;
/// let prefix = Prefix::new("myapp");
/// let paths: Vec<PathBuf> = Vec::new();
/// let _ = (prefix, paths);
/// ```
#[cfg(any(unix, target_os = "redox"))]
fn collect_unix_paths(prefix: &Prefix, paths: &mut Vec<PathBuf>) {
    let _ = push_home_from_env(&["HOME"], prefix, paths);

    let xdg_dirs = if prefix.as_str().is_empty() {
        BaseDirectories::new()
    } else {
        BaseDirectories::with_prefix(prefix.as_str())
    };

    if let Some(p) = xdg_dirs.find_config_file("config.toml") {
        paths.push(p);
    }

    #[cfg(feature = "json5")]
    for ext in ["json", "json5"] {
        if let Some(p) = xdg_dirs.find_config_file(format!("config.{ext}")) {
            paths.push(p);
        }
    }

    #[cfg(feature = "yaml")]
    for ext in ["yaml", "yml"] {
        if let Some(p) = xdg_dirs.find_config_file(format!("config.{ext}")) {
            paths.push(p);
        }
    }
}

#[cfg(not(any(unix, target_os = "redox")))]
fn collect_non_unix_paths(prefix: &Prefix, paths: &mut Vec<PathBuf>) {
    // Prefer an explicit HOME or USERPROFILE if provided. These variables allow
    // callers to override the detected home directory on Windows where
    // `BaseDirs` otherwise queries the system API.
    let env_set = push_home_from_env(&["HOME", "USERPROFILE"], prefix, paths);

    if let Some(dirs) = BaseDirs::new() {
        // If the home directory wasn't overridden above, include the one
        // reported by `BaseDirs` as well.
        if !env_set {
            push_dir_candidates(dirs.home_dir(), &format!(".{}", prefix.as_str()), paths);
        }

        let cfg_dir = if prefix.as_str().is_empty() {
            dirs.config_dir().to_path_buf()
        } else {
            dirs.config_dir().join(prefix.as_str())
        };
        push_dir_candidates(&cfg_dir, "config", paths);
    }
}

fn candidate_paths(prefix: &Prefix) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(any(unix, target_os = "redox"))]
    collect_unix_paths(prefix, &mut paths);

    #[cfg(not(any(unix, target_os = "redox")))]
    collect_non_unix_paths(prefix, &mut paths);

    push_dir_candidates(Path::new(""), &format!(".{}", prefix.as_str()), &mut paths);
    paths
}

/// Load and merge `[cmds.<name>]` sections from the given paths.
#[allow(clippy::result_large_err)]
/// Loads and merges configuration for a subcommand from the specified files.
///
/// For each provided path, loads the configuration file and merges the `[cmds.<name>]` section into a single `Figment` instance. Only the focused section for the given subcommand is merged from each file.
///
/// # Parameters
/// - `paths`: Slice of file paths to search for configuration.
/// - `name`: The subcommand name whose configuration section to load.
///
/// # Returns
/// A `Figment` instance containing the merged configuration for the subcommand, or an error if loading fails.
fn load_from_files(paths: &[PathBuf], name: &CmdName) -> Result<Figment, OrthoError> {
    let mut fig = Figment::new();
    for p in paths {
        if let Some(file_fig) = load_config_file(p)? {
            fig = fig.merge(file_fig.focus(&format!("cmds.{}", name.as_str())));
        }
    }
    Ok(fig)
}

/// Load configuration for a specific subcommand.
///
/// The configuration is sourced from:
///   * `[cmds.<name>]` sections in configuration files
///   * environment variables following the pattern `<PREFIX>CMDS_<NAME>_`.
///
/// Values from environment variables override those from files.
///
/// # Errors
///
/// Returns an [`OrthoError`] if file loading or deserialization fails.
///
/// # Deprecated
///
/// Use [`load_and_merge_subcommand`] or [`load_and_merge_subcommand_for`] instead
/// to load defaults and apply CLI overrides in one step.
#[allow(clippy::result_large_err)]
#[deprecated(note = "use `load_and_merge_subcommand` or `load_and_merge_subcommand_for` instead")]
/// Loads configuration for a specific subcommand from files and environment variables.
///
/// Searches for configuration files using the provided prefix, loads the `[cmds.<name>]` section from each file, and merges them. Then overlays environment variables prefixed with `<PREFIX>CMDS_<NAME>_` (case-insensitive, double underscore for nesting). Returns the merged configuration as type `T`.
///
/// # Deprecated
///
/// This function is deprecated. Use the newer combined loading and merging functions instead.
///
/// # Errors
///
/// Returns an error if configuration files cannot be loaded or if deserialisation fails.
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::subcommand::{Prefix, CmdName, load_subcommand_config};
/// # type MyServeConfig = ();
/// # fn main() -> Result<(), ortho_config::OrthoError> {
/// let prefix = Prefix::new("myapp");
/// let name = CmdName::new("serve");
/// let config: MyServeConfig = load_subcommand_config(&prefix, &name)?;
/// # let _ = config;
/// # Ok(())
/// # }
/// ```
pub fn load_subcommand_config<T>(prefix: &Prefix, name: &CmdName) -> Result<T, OrthoError>
where
    T: DeserializeOwned + Default,
{
    let paths = candidate_paths(prefix);
    let mut fig = load_from_files(&paths, name)?;

    let env_name = name.env_key();
    let env_prefix = format!("{}CMDS_{env_name}_", prefix.raw());
    let env_provider = Env::prefixed(&env_prefix)
        .map(|k| Uncased::from(k))
        .split("__");
    fig = fig.merge(env_provider);

    fig.extract().map_err(OrthoError::Gathering)
}

/// Load default values for a subcommand using `T`'s configured prefix.
///
/// The prefix is provided by [`OrthoConfig::prefix`]. If the struct does not
/// specify `#[ortho_config(prefix = "...")]`, the default empty prefix is used.
/// Combine the returned defaults with CLI arguments using
/// [`merge_cli_over_defaults`](crate::merge_cli_over_defaults).
///
/// # Errors
///
/// Returns an [`OrthoError`] if file loading or deserialization fails.
#[allow(clippy::result_large_err)]
/// Loads configuration defaults for a subcommand using the prefix defined by the type.
///
/// This function retrieves the configuration for the specified subcommand, using the prefix provided by the `OrthoConfig` implementation of `T`. It loads and merges configuration from files and environment variables, returning the resulting configuration as type `T`.
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::subcommand::{CmdName, load_subcommand_config_for};
/// #[derive(serde::Deserialize, Default)]
/// struct MySubcommandConfig;
/// impl ortho_config::OrthoConfig for MySubcommandConfig {
///     fn load() -> Result<Self, ortho_config::OrthoError> { todo!() }
///     fn prefix() -> &'static str { "" }
/// }
/// # fn main() -> Result<(), ortho_config::OrthoError> {
/// let config = load_subcommand_config_for::<MySubcommandConfig>(&CmdName::new("serve"))?;
/// # let _ = config;
/// # Ok(())
/// # }
/// ```
pub fn load_subcommand_config_for<T>(name: &CmdName) -> Result<T, OrthoError>
where
    T: crate::OrthoConfig + Default,
{
    #[allow(deprecated)]
    {
        load_subcommand_config(&Prefix::new(T::prefix()), name)
    }
}

/// Load defaults for a subcommand and merge CLI-provided values over them.
///
/// This convenience function combines [`load_subcommand_config`] and
/// [`merge_cli_over_defaults`](crate::merge_cli_over_defaults) to reduce
/// boilerplate when working with `clap` subcommands.
///
/// # Errors
///
/// Returns an [`OrthoError`] if file loading or deserialization fails.
#[allow(clippy::result_large_err)]
/// Loads configuration defaults for a subcommand and merges CLI-provided values over them.
///
/// This function determines the subcommand name from the type `T`, loads its default configuration from files and environment variables using the given prefix, and overlays values provided via the CLI. The resulting configuration is returned as type `T`.
///
/// # Returns
///
/// The merged configuration for the subcommand, or an error if loading or merging fails.
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::subcommand::load_and_merge_subcommand;
/// use ortho_config::subcommand::Prefix;
/// # use clap::Parser;
/// #[derive(clap::Parser, serde::Serialize, serde::Deserialize, Default)]
/// struct MyCmd {
///     #[clap(long)]
///     value: Option<String>,
/// }
///
/// # fn main() -> Result<(), ortho_config::OrthoError> {
/// let prefix = Prefix::new("MYAPP");
/// let cli = MyCmd { value: Some("cli".to_string()) };
/// let config = load_and_merge_subcommand(&prefix, &cli)?;
/// assert_eq!(config.value.as_deref(), Some("cli"));
/// # Ok(())
/// # }
/// ```
pub fn load_and_merge_subcommand<T>(prefix: &Prefix, cli: &T) -> Result<T, OrthoError>
where
    T: serde::Serialize + DeserializeOwned + Default + CommandFactory,
{
    let name = CmdName::new(T::command().get_name());
    #[allow(deprecated)]
    let defaults: T = load_subcommand_config(prefix, &name)?;
    #[allow(deprecated)]
    merge_cli_over_defaults(&defaults, cli).map_err(OrthoError::Gathering)
}

/// Wrapper around [`load_and_merge_subcommand`] using the struct's configured prefix.
///
/// # Errors
///
/// Returns an [`OrthoError`] if file loading or deserialization fails.
#[allow(clippy::result_large_err)]
/// Loads and merges configuration for a subcommand using the prefix defined by its type.
///
/// Loads default configuration values for the subcommand from files and environment variables, then merges CLI-provided values over these defaults. The prefix is determined by the `OrthoConfig` implementation of the type.
///
/// # Returns
///
/// The merged configuration for the subcommand, or an error if loading or merging fails.
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::subcommand::load_and_merge_subcommand_for;
/// use clap::Parser;
/// #[derive(clap::Parser, serde::Serialize, serde::Deserialize, Default)]
/// struct MyCmd { /* fields */ }
/// impl ortho_config::OrthoConfig for MyCmd {
///     fn load() -> Result<Self, ortho_config::OrthoError> { todo!() }
///     fn prefix() -> &'static str { "myapp" }
/// }
/// # fn main() -> Result<(), ortho_config::OrthoError> {
/// let cli = MyCmd::parse_from(&["mycmd"]);
/// let config = load_and_merge_subcommand_for(&cli)?;
/// # let _ = config;
/// # Ok(())
/// # }
/// ```
pub fn load_and_merge_subcommand_for<T>(cli: &T) -> Result<T, OrthoError>
where
    T: crate::OrthoConfig + serde::Serialize + Default + CommandFactory,
{
    load_and_merge_subcommand(&Prefix::new(T::prefix()), cli)
}
