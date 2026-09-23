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
    fn default() -> Self {
        Self {
            count: 8,
            mode: Mode::Fast,
            port: 7,
            label: Some(String::from("default")),
        }
    }
}

#[rstest]
#[serial]
fn inferred_default_value_preserves_clap_parsers() -> Result<()> {
    assert_inferred_default_parity()?;
    assert_file_overrides_inferred_default_parity()?;
    assert_explicit_cli_overrides_default_parity()?;
    Ok(())
}

#[derive(Clone, Copy)]
struct DefaultParityExpected {
    count: u16,
    mode: Mode,
    port: u16,
    label: &'static str,
}

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

fn assert_file_overrides_inferred_default_parity() -> Result<()> {
    let temp_dir = config_dir(
        "[cmds.default-parity]\ncount = 5\nmode = \"safe\"\nport = 6\nlabel = \"file\"\n",
    )?;
    let matches = DefaultParityArgs::command().get_matches_from(["default-parity"]);
    let args = DefaultParityArgs::from_arg_matches(&matches).context("parse defaults")?;
    let merged = load_and_merge_subcommand_with_matches_with_sources_at(
        &Prefix::new("APP_"),
        &SubcommandCliMatches::new(&args, &matches),
        SubcommandFileContext::new(temp_dir.path(), &MapEnv::new()),
        Arc::new(MapEnv::new()),
    )
    .context("merge parser-faithful defaults")?;
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

fn assert_explicit_cli_overrides_default_parity() -> Result<()> {
    let temp_dir = config_dir(
        "[cmds.default-parity]\ncount = 5\nmode = \"safe\"\nport = 6\nlabel = \"file\"\n",
    )?;
    let matches = DefaultParityArgs::command().get_matches_from([
        "default-parity",
        "--count",
        "9",
        "--mode",
        "fast",
        "--port",
        "tcp:10",
        "--label",
        "cli",
    ]);
    let args = DefaultParityArgs::from_arg_matches(&matches).context("parse explicit values")?;
    let merged = load_and_merge_subcommand_with_matches_with_sources_at(
        &Prefix::new("APP_"),
        &SubcommandCliMatches::new(&args, &matches),
        SubcommandFileContext::new(temp_dir.path(), &MapEnv::new()),
        Arc::new(MapEnv::new()),
    )
    .context("merge explicit values")?;
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
    fn default() -> Self {
        Self { enabled: true }
    }
}

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
        let (_temp_dir, _cwd_guard) = config_dir_with_cwd("enabled = false\n")?;
        let source = Arc::new(MapEnv::new());
        let loaded =
            BoolDefaultArgs::load_from_iter_with_sources(["bool-default"], source.clone(), source)?;
        ensure!(
            !loaded.enabled,
            "expected file value to override bool default"
        );
    }
    {
        let (_temp_dir, _cwd_guard) = config_dir_with_cwd("enabled = true\n")?;
        let source = Arc::new(MapEnv::new().with_var("APP_ENABLED", "false"));
        let loaded =
            BoolDefaultArgs::load_from_iter_with_sources(["bool-default"], source.clone(), source)?;
        ensure!(
            !loaded.enabled,
            "expected environment value to override file value"
        );
    }
    {
        let (_temp_dir, _cwd_guard) = config_dir_with_cwd("enabled = false\n")?;
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
    fn default() -> Self {
        Self { count: 11 }
    }
}

#[rstest]
fn explicit_ortho_default_overrides_inferred_default_value() -> Result<()> {
    let temp_dir = config_dir("")?;
    let prefix = Prefix::new("APP_");
    let matches = ExplicitDefaultArgs::command().get_matches_from(["explicit-default"]);
    let args = ExplicitDefaultArgs::from_arg_matches(&matches).context("parse defaults")?;
    let merged = load_and_merge_subcommand_with_matches_with_sources_at(
        &prefix,
        &SubcommandCliMatches::new(&args, &matches),
        SubcommandFileContext::new(temp_dir.path(), &MapEnv::new()),
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
    fn default() -> Self {
        Self { port: 7 }
    }
}

fn contains_default_value_conversion(error: &OrthoError) -> bool {
    match error {
        OrthoError::DefaultValueConversion { .. } => true,
        OrthoError::Aggregate(errors) => errors
            .iter()
            .any(|entry| matches!(entry, OrthoError::DefaultValueConversion { .. })),
        _ => false,
    }
}

#[rstest]
#[serial]
fn invalid_inferred_default_is_reported_without_panicking() -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir_with_cwd("")?;
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
