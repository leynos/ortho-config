//! Regression tests for documentation metadata that describes clap's surface.
//!
//! Issue #717 reports that a subcommand argument struct deriving both
//! `clap::Parser` and `OrthoConfig` was documented with invented long flags,
//! root-form environment variables and file keys, and a subcommand `about_id`
//! that disagreed with the struct's own `ABOUT_ID`.

use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use ortho_config::docs::{
    DocMetadata, FieldMetadata, OrthoConfigDocs as _, OrthoConfigSubcommandDocs as _,
    PositionalMetadata,
};
use ortho_config::{OrthoConfig, OrthoConfigLocalization};
use serde::{Deserialize, Serialize};

/// Top-level commands exercised by the regression.
#[derive(Debug, clap::Subcommand, ortho_config::OrthoConfigSubcommandDocs)]
enum Commands {
    /// Validates a proposal.
    Validate(ValidateArgs),
}

/// Arguments that clap parses and the subcommand merge reads.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[command(name = "validate")]
#[ortho_config(prefix = "SPLITTERS", localization_base = "splitters.cli.validate")]
struct ValidateArgs {
    /// Proposal identifier.
    #[arg(value_name = "PROPOSAL")]
    proposal: String,
    /// Check command.
    #[arg(long, value_name = "COMMAND")]
    check: Option<String>,
}

/// A boolean flag that clap parses as a bare switch.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
struct PurgeArgs {
    /// Skip the confirmation prompt.
    #[arg(long)]
    force: bool,
}

/// Arguments whose container casing changes the derived long flag.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[command(rename_all = "snake_case")]
#[ortho_config(prefix = "SPLITTERS")]
struct SnakeCaseArgs {
    /// Output file path.
    #[arg(long)]
    output_file: String,
}

/// A clap-required argument that also carries a Serde default.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
struct HostArgs {
    /// Host to contact.
    #[arg(long)]
    #[serde(default)]
    host: String,
}

/// A skipped CLI field whose configuration default keeps it non-required.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
struct SkippedArgs {
    /// Internal value supplied by configuration.
    #[arg(skip)]
    #[ortho_config(default = String::from("fallback"))]
    internal: String,
}

/// Returns one field's metadata by name.
fn field<'a>(metadata: &'a DocMetadata, name: &str) -> Result<&'a FieldMetadata> {
    metadata
        .fields
        .iter()
        .find(|field| field.name == name)
        .ok_or_else(|| anyhow!("missing field {name}"))
}

/// Asserts a field looks like a positional operand at the expected index.
fn ensure_positional(field: &FieldMetadata, index: u16, value_name: &str) -> Result<()> {
    let cli = field
        .cli
        .as_ref()
        .ok_or_else(|| anyhow!("{} should have CLI metadata", field.name))?;
    ensure!(
        cli.long.is_none(),
        "{} must not advertise a long flag, got {:?}",
        field.name,
        cli.long
    );
    ensure!(
        cli.short.is_none(),
        "{} must not advertise a short flag, got {:?}",
        field.name,
        cli.short
    );
    ensure!(
        cli.value_name.as_deref() == Some(value_name),
        "{} should carry value name {value_name}, got {:?}",
        field.name,
        cli.value_name
    );
    ensure!(
        cli.positional == Some(PositionalMetadata::new(index)),
        "{} should be positional at index {index}, got {:?}",
        field.name,
        cli.positional
    );
    Ok(())
}

#[test]
fn standalone_metadata_describes_clap_positionals_and_value_names() -> Result<()> {
    let metadata = ValidateArgs::get_doc_metadata();

    ensure_positional(field(&metadata, "proposal")?, 1, "PROPOSAL")?;

    let check = field(&metadata, "check")?;
    let check_cli = check
        .cli
        .as_ref()
        .ok_or_else(|| anyhow!("check should have CLI metadata"))?;
    ensure!(
        check_cli.long.as_deref() == Some("check"),
        "check should keep its clap long flag, got {:?}",
        check_cli.long
    );
    ensure!(
        check_cli.short.is_none(),
        "check must not invent a short flag, got {:?}",
        check_cli.short
    );
    ensure!(
        check_cli.value_name.as_deref() == Some("COMMAND"),
        "check should keep the declared value name, got {:?}",
        check_cli.value_name
    );
    ensure!(
        check_cli.positional.is_none(),
        "check should not be positional, got {:?}",
        check_cli.positional
    );

    ensure!(
        field(&metadata, "proposal")?
            .env
            .as_ref()
            .map(|env| env.var_name.as_str())
            == Some("SPLITTERS_PROPOSAL"),
        "standalone metadata keeps the root environment name"
    );
    ensure!(
        field(&metadata, "check")?
            .file
            .as_ref()
            .map(|file| file.key_path.as_str())
            == Some("check"),
        "standalone metadata keeps the root file key"
    );
    ensure!(
        metadata.about_id == ValidateArgs::ABOUT_ID,
        "standalone about_id should equal ABOUT_ID: {} != {}",
        metadata.about_id,
        ValidateArgs::ABOUT_ID
    );
    Ok(())
}

#[test]
fn subcommand_metadata_uses_merge_namespaces_and_declared_base() -> Result<()> {
    let mut metadata = Commands::get_subcommand_doc_metadata();
    ensure!(
        metadata.len() == 1,
        "expected one subcommand node, got {}",
        metadata.len()
    );
    let validate = metadata.remove(0);

    ensure!(
        validate.app_name == "validate",
        "expected the clap label as the application name, got {:?}",
        validate.app_name
    );
    ensure!(
        validate.about_id == ValidateArgs::ABOUT_ID,
        "subcommand about_id should equal ABOUT_ID: {} != {}",
        validate.about_id,
        ValidateArgs::ABOUT_ID
    );

    let proposal = field(&validate, "proposal")?;
    ensure_positional(proposal, 1, "PROPOSAL")?;
    ensure!(
        proposal.env.as_ref().map(|env| env.var_name.as_str())
            == Some("SPLITTERS_CMDS_VALIDATE_PROPOSAL"),
        "expected the subcommand environment name, got {:?}",
        proposal.env
    );
    ensure!(
        proposal.file.as_ref().map(|file| file.key_path.as_str()) == Some("cmds.validate.proposal"),
        "expected the subcommand file key, got {:?}",
        proposal.file
    );

    let check = field(&validate, "check")?;
    ensure!(
        check.env.as_ref().map(|env| env.var_name.as_str())
            == Some("SPLITTERS_CMDS_VALIDATE_CHECK"),
        "expected the subcommand environment name, got {:?}",
        check.env
    );
    ensure!(
        check.file.as_ref().map(|file| file.key_path.as_str()) == Some("cmds.validate.check"),
        "expected the subcommand file key, got {:?}",
        check.file
    );

    for name in ["proposal", "check"] {
        let declared = ValidateArgs::ARG_IDS
            .iter()
            .find(|arg| arg.name == name)
            .ok_or_else(|| anyhow!("missing ARG_IDS entry for {name}"))?;
        let documented = field(&validate, name)?;
        ensure!(
            documented.help_id == declared.help_id,
            "{name} help_id should agree with ARG_IDS: {} != {}",
            documented.help_id,
            declared.help_id
        );
        ensure!(
            documented.long_help_id.as_deref() == Some(declared.long_help_id),
            "{name} long_help_id should agree with ARG_IDS"
        );
    }
    Ok(())
}

#[test]
fn a_parent_path_rebases_identifiers_but_keeps_subcommand_namespaces() -> Result<()> {
    let parent_path = [String::from("root")];
    let mut metadata = Commands::get_subcommand_doc_metadata_for_path(&parent_path);
    ensure!(
        metadata.len() == 1,
        "expected one subcommand node, got {}",
        metadata.len()
    );
    let validate = metadata.remove(0);

    ensure!(
        validate.about_id == "root-validate-about",
        "a parent path should rebase the about id, got {}",
        validate.about_id
    );
    ensure!(
        field(&validate, "proposal")?
            .env
            .as_ref()
            .map(|env| env.var_name.as_str())
            == Some("SPLITTERS_CMDS_VALIDATE_PROPOSAL"),
        "a parent path must not change the subcommand environment namespace"
    );
    Ok(())
}

#[test]
fn a_subcommand_path_projects_environment_and_file_metadata() -> Result<()> {
    let command_path = [String::from("root"), String::from("validate")];
    let metadata = ValidateArgs::get_doc_metadata_for_path(&command_path);

    ensure!(
        metadata.about_id == "root-validate-about",
        "a mounted path should scope the about id, got {}",
        metadata.about_id
    );
    ensure!(
        field(&metadata, "proposal")?
            .env
            .as_ref()
            .map(|env| env.var_name.as_str())
            == Some("SPLITTERS_CMDS_VALIDATE_PROPOSAL"),
        "a mounted path should project the environment name"
    );
    ensure!(
        field(&metadata, "check")?
            .file
            .as_ref()
            .map(|file| file.key_path.as_str())
            == Some("cmds.validate.check"),
        "a mounted path should project the file key"
    );
    Ok(())
}

#[test]
fn container_casing_reaches_derived_flags() -> Result<()> {
    let metadata = SnakeCaseArgs::get_doc_metadata();
    let field = field(&metadata, "output_file")?;
    let cli = field
        .cli
        .as_ref()
        .ok_or_else(|| anyhow!("output_file should have CLI metadata"))?;
    ensure!(
        cli.long.as_deref() == Some("output_file"),
        "rename_all = \"snake_case\" should keep the underscore, got {:?}",
        cli.long
    );
    Ok(())
}

#[test]
fn a_serde_default_does_not_relax_clap_requiredness() -> Result<()> {
    let metadata = HostArgs::get_doc_metadata();
    ensure!(
        field(&metadata, "host")?.required,
        "a value-taking clap argument with no clap default must stay required"
    );
    Ok(())
}

#[test]
fn a_skipped_clap_field_keeps_configuration_requiredness() -> Result<()> {
    let metadata = SkippedArgs::get_doc_metadata();
    let internal = field(&metadata, "internal")?;
    ensure!(
        internal.cli.is_none(),
        "a skipped clap field should have no CLI metadata"
    );
    ensure!(
        !internal.required,
        "a skipped clap field should fall back to configuration requiredness"
    );
    Ok(())
}

#[test]
fn a_bool_flag_is_a_switch_without_an_invented_short() -> Result<()> {
    let metadata = PurgeArgs::get_doc_metadata();
    let force = field(&metadata, "force")?;
    let cli = force
        .cli
        .as_ref()
        .ok_or_else(|| anyhow!("force should have CLI metadata"))?;

    ensure!(
        cli.long.as_deref() == Some("force"),
        "force should keep its long flag, got {:?}",
        cli.long
    );
    ensure!(
        cli.short.is_none(),
        "force must not invent a short flag, got {:?}",
        cli.short
    );
    ensure!(
        !cli.takes_value,
        "a clap bool flag is a switch, not an optional-value flag"
    );
    ensure!(
        !cli.value_optional,
        "a switch must not mark a value optional"
    );
    ensure!(
        cli.possible_values.is_empty(),
        "a switch has no possible values, got {:?}",
        cli.possible_values
    );
    ensure!(cli.positional.is_none(), "a switch is not positional");
    Ok(())
}
