//! Parser-faithful clap default inference and conversion tests.

use super::*;

/// Parser used to prove inferred string defaults reuse a field's clap parser.
fn parse_tcp_port(value: &str) -> Result<u16, String> {
    value
        .strip_prefix("tcp:")
        .ok_or_else(|| String::from("expected a tcp: port"))?
        .parse::<u16>()
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    Fast,
    Safe,
}

/// Subcommand that exercises parser-faithful string default inference.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "default-parity")]
#[ortho_config(prefix = "APP_")]
struct DefaultParityArgs {
    #[arg(long, default_value = "8")]
    #[ortho_config(cli_default_as_absent)]
    count: u16,

    #[arg(long, default_value = "FAST", value_enum, ignore_case = true)]
    #[ortho_config(cli_default_as_absent)]
    mode: Mode,

    #[arg(long, default_value = "tcp:7", value_parser = parse_tcp_port)]
    #[ortho_config(cli_default_as_absent)]
    port: u16,

    #[arg(long, default_value = "default")]
    #[ortho_config(cli_default_as_absent)]
    label: Option<String>,
}

impl Default for DefaultParityArgs {
    /// Mirrors the clap defaults so struct defaults and inferred defaults agree.
    fn default() -> Self {
        Self {
            count: 8,
            mode: Mode::Fast,
            port: 7,
            label: Some(String::from("default")),
        }
    }
}

/// Inferred string defaults, file values, and explicit CLI values all pass
/// through each field's clap parser and respect source precedence.
#[rstest]
#[serial]
fn inferred_default_value_preserves_clap_parsers() -> Result<()> {
    assert_inferred_default_parity()?;
    assert_file_overrides_inferred_default_parity()?;
    assert_explicit_cli_overrides_default_parity()?;
    Ok(())
}

/// Field values a parity case expects after loading or merging.
#[derive(Clone, Copy)]
struct DefaultParityExpected {
    count: u16,
    mode: Mode,
    port: u16,
    label: &'static str,
}

/// Checks every parity field, naming the first mismatched field on failure.
fn assert_default_parity(
    actual: &DefaultParityArgs,
    expected: DefaultParityExpected,
) -> Result<()> {
    ensure!(
        actual.count == expected.count,
        "expected count {}, got {}",
        expected.count,
        actual.count
    );
    ensure!(
        actual.mode == expected.mode,
        "expected mode {:?}, got {:?}",
        expected.mode,
        actual.mode
    );
    ensure!(
        actual.port == expected.port,
        "expected port {}, got {}",
        expected.port,
        actual.port
    );
    ensure!(
        actual.label.as_deref() == Some(expected.label),
        "expected label {:?}, got {:?}",
        expected.label,
        actual.label,
    );
    Ok(())
}

/// With no file or environment, the parsed clap defaults are loaded.
fn assert_inferred_default_parity() -> Result<()> {
    let no_file_dir = tempfile::tempdir().context("create no-file config dir")?;
    let _cwd_guard = cwd::set_dir(no_file_dir.path())?;
    let inferred =
        DefaultParityArgs::load_from_iter(["default-parity"]).context("load inferred defaults")?;
    assert_default_parity(
        &inferred,
        DefaultParityExpected {
            count: 8,
            mode: Mode::Fast,
            port: 7,
            label: "default",
        },
    )
}

/// Configuration file shared by the file-backed default parity cases.
const DEFAULT_PARITY_FILE: &str =
    "[cmds.default-parity]\ncount = 5\nmode = \"safe\"\nport = 6\nlabel = \"file\"\n";

/// Parses `cli_args` and merges them over [`DEFAULT_PARITY_FILE`].
///
/// The temporary directory and current-directory guard live until the merge
/// completes, so discovery always sees the shared configuration file.
fn merge_default_parity_over_file(
    cli_args: &[&str],
    parse_context: &'static str,
    merge_context: &'static str,
) -> Result<DefaultParityArgs> {
    let (_temp_dir, _cwd_guard) = config_dir(DEFAULT_PARITY_FILE)?;
    let matches = DefaultParityArgs::command().get_matches_from(cli_args);
    let args = DefaultParityArgs::from_arg_matches(&matches).context(parse_context)?;
    load_and_merge_subcommand_with_matches_with_sources(
        &Prefix::new("APP_"),
        &args,
        &matches,
        Arc::new(MapEnv::new()),
    )
    .context(merge_context)
}

/// File values replace inferred clap defaults that the user did not supply.
fn assert_file_overrides_inferred_default_parity() -> Result<()> {
    let merged = merge_default_parity_over_file(
        &["default-parity"],
        "parse defaults",
        "merge parser-faithful defaults",
    )?;
    assert_default_parity(
        &merged,
        DefaultParityExpected {
            count: 5,
            mode: Mode::Safe,
            port: 6,
            label: "file",
        },
    )
}

/// Explicit CLI values win over file values and are parsed by clap.
fn assert_explicit_cli_overrides_default_parity() -> Result<()> {
    let merged = merge_default_parity_over_file(
        &[
            "default-parity",
            "--count",
            "9",
            "--mode",
            "fast",
            "--port",
            "tcp:10",
            "--label",
            "cli",
        ],
        "parse explicit values",
        "merge explicit values",
    )?;
    assert_default_parity(
        &merged,
        DefaultParityExpected {
            count: 9,
            mode: Mode::Fast,
            port: 10,
            label: "cli",
        },
    )
}

/// The generated CLI applies the field's custom `value_parser` to explicit
/// arguments.
#[rstest]
#[serial]
fn generated_cli_uses_captured_value_parser() -> Result<()> {
    let no_file_dir = tempfile::tempdir().context("create no-file config dir")?;
    let _cwd_guard = cwd::set_dir(no_file_dir.path())?;
    let parsed = DefaultParityArgs::load_from_iter(["default-parity", "--port", "tcp:10"])
        .context("parse explicit custom CLI value")?;
    ensure!(
        parsed.port == 10,
        "expected parsed port 10, got {}",
        parsed.port
    );
    Ok(())
}

/// Subcommand that ensures inferred true defaults are absent until explicitly set.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "bool-default")]
#[ortho_config(prefix = "APP_")]
struct BoolDefaultArgs {
    #[arg(long, default_value = "true")]
    #[ortho_config(cli_default_as_absent)]
    enabled: bool,
}

impl Default for BoolDefaultArgs {
    /// Matches the clap `default_value = "true"`.
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// A `true` bool default is used only when no file, environment, or CLI value
/// is supplied, and each higher-precedence source overrides the one below.
#[rstest]
#[serial]
fn inferred_true_bool_default_preserves_source_precedence() -> Result<()> {
    {
        let no_file_dir = tempfile::tempdir().context("create no-file config dir")?;
        let _cwd_guard = cwd::set_dir(no_file_dir.path())?;
        let loaded = BoolDefaultArgs::load_from_iter(["bool-default"])?;
        ensure!(
            loaded.enabled,
            "expected inferred bool default to remain true"
        );
    }
    {
        let (_temp_dir, _cwd_guard) = config_dir("enabled = false\n")?;
        let source = Arc::new(MapEnv::new());
        let loaded =
            BoolDefaultArgs::load_from_iter_with_sources(["bool-default"], source.clone(), source)?;
        ensure!(
            !loaded.enabled,
            "expected file value to override bool default"
        );
    }
    {
        let (_temp_dir, _cwd_guard) = config_dir("enabled = true\n")?;
        let source = Arc::new(MapEnv::new().with_var("APP_ENABLED", "false"));
        let loaded =
            BoolDefaultArgs::load_from_iter_with_sources(["bool-default"], source.clone(), source)?;
        ensure!(
            !loaded.enabled,
            "expected environment value to override file value"
        );
    }
    {
        let (_temp_dir, _cwd_guard) = config_dir("enabled = false\n")?;
        let source = Arc::new(MapEnv::new().with_var("APP_ENABLED", "false"));
        let loaded = BoolDefaultArgs::load_from_iter_with_sources(
            ["bool-default", "--enabled"],
            source.clone(),
            source,
        )?;
        ensure!(loaded.enabled, "expected explicit CLI value to win");
    }
    Ok(())
}

/// Explicit `OrthoConfig` defaults continue to override inferred clap defaults.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "explicit-default")]
#[ortho_config(prefix = "APP_")]
struct ExplicitDefaultArgs {
    #[arg(long, default_value = "8")]
    #[ortho_config(default = 11, cli_default_as_absent)]
    count: u16,
}

impl Default for ExplicitDefaultArgs {
    /// Matches the explicit `OrthoConfig` default rather than the clap default.
    fn default() -> Self {
        Self { count: 11 }
    }
}

/// An explicit `#[ortho_config(default = ...)]` takes precedence over the
/// inferred clap `default_value`.
#[rstest]
#[serial]
fn explicit_ortho_default_overrides_inferred_default_value() -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir("")?;
    let prefix = Prefix::new("APP_");
    let matches = ExplicitDefaultArgs::command().get_matches_from(["explicit-default"]);
    let args = ExplicitDefaultArgs::from_arg_matches(&matches).context("parse defaults")?;
    let merged = load_and_merge_subcommand_with_matches_with_sources(
        &prefix,
        &args,
        &matches,
        Arc::new(MapEnv::new()),
    )
    .context("merge explicit OrthoConfig default")?;
    ensure!(
        merged.count == 11,
        "expected explicit default, got {}",
        merged.count
    );
    Ok(())
}

/// Configuration with a clap default that its field parser rejects.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "invalid-default")]
#[ortho_config(prefix = "APP_")]
struct InvalidDefaultArgs {
    #[arg(long, default_value = "udp:7", value_parser = parse_tcp_port)]
    #[ortho_config(cli_default_as_absent)]
    port: u16,
}

impl Default for InvalidDefaultArgs {
    /// Supplies a valid port so only the clap default is invalid.
    fn default() -> Self {
        Self { port: 7 }
    }
}

/// Reports whether `error`, or any entry of an aggregate, is a
/// `DefaultValueConversion` failure.
fn contains_default_value_conversion(error: &OrthoError) -> bool {
    match error {
        OrthoError::DefaultValueConversion { .. } => true,
        OrthoError::Aggregate(errors) => errors
            .iter()
            .any(|entry| matches!(entry, OrthoError::DefaultValueConversion { .. })),
        _ => false,
    }
}

/// A clap default rejected by the field parser surfaces as a redacted
/// `DefaultValueConversion` error instead of a panic.
#[rstest]
#[serial]
fn invalid_inferred_default_is_reported_without_panicking() -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir("")?;
    let error = InvalidDefaultArgs::load_from_iter(["invalid-default"])
        .err()
        .ok_or_else(|| anyhow::anyhow!("expected invalid default to fail"))?;
    ensure!(
        contains_default_value_conversion(error.as_ref()),
        "expected DefaultValueConversion, got {error:?}",
    );
    let rendered = error.to_string();
    ensure!(
        rendered.contains("port"),
        "expected field key, got {rendered}"
    );
    for sensitive_fragment in ["udp:7", "tcp:7", "expected a tcp: port"] {
        ensure!(
            !rendered.contains(sensitive_fragment),
            "expected conversion error to redact {sensitive_fragment:?}, got {rendered}",
        );
    }
    Ok(())
}
