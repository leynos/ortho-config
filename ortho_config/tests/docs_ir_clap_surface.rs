//! Regression tests for documentation metadata that describes clap's surface.
//!
//! Issue #717 reports that a subcommand argument struct deriving both
//! `clap::Parser` and `OrthoConfig` was documented with invented long flags,
//! root-form environment variables and file keys, and a subcommand `about_id`
//! that disagreed with the struct's own `ABOUT_ID`.

use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfigLocalization;
use ortho_config::docs::{
    DocMetadata, FieldMetadata, OrthoConfigDocs as _, OrthoConfigSubcommandDocs as _,
    PositionalMetadata,
};

#[path = "support/docs_ir_clap_surface_fixtures.rs"]
mod fixtures;

use fixtures::{
    Commands, FlattenedOperandArgs, GlobalArgs, HostArgs, PartlyDeclaredArgs, PurgeArgs,
    RawIndexArgs, RawSpellingArgs, SkippedArgs, SnakeCaseArgs, UnnamedArgs, UnnamedCommands,
    UnreadableArgs, UnreadableOperandArgs, UnreadableOptionArgs, ValidateArgs,
};

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
fn an_inherited_global_field_has_no_cli_metadata() -> Result<()> {
    let metadata = GlobalArgs::get_doc_metadata();
    ensure!(
        field(&metadata, "verbose")?.cli.is_none(),
        "a from_global field owns no field-local CLI metadata"
    );
    ensure_positional(field(&metadata, "input")?, 1, "INPUT")?;
    Ok(())
}

#[test]
fn an_unreadable_declaration_does_not_advertise_a_generated_flag() -> Result<()> {
    let metadata = UnreadableArgs::get_doc_metadata();
    ensure!(
        field(&metadata, "proposal")?.cli.is_none(),
        "a clap declaration the derive cannot read must not fall back to an invented flag"
    );
    Ok(())
}

#[test]
fn an_undeclared_operand_receives_claps_positional_index() -> Result<()> {
    let metadata = PartlyDeclaredArgs::get_doc_metadata();

    ensure_positional(field(&metadata, "source")?, 1, "SOURCE")?;
    ensure_positional(field(&metadata, "second")?, 2, "SECOND")?;
    let verbose = field(&metadata, "verbose")?;
    let verbose_cli = verbose
        .cli
        .as_ref()
        .ok_or_else(|| anyhow!("verbose should have CLI metadata"))?;
    ensure!(
        verbose_cli.long.as_deref() == Some("verbose") && verbose_cli.positional.is_none(),
        "a declared switch should stay a named flag, got {verbose_cli:?}"
    );
    Ok(())
}

/// Returns the index clap assigns to an argument id after building the command.
fn clap_positional_index<T: clap::CommandFactory>(id: &str) -> Option<u16> {
    let mut command = T::command();
    command.build();
    command
        .get_arguments()
        .find(|argument| argument.get_id() == id)
        .and_then(clap::Arg::get_index)
        .and_then(|index| u16::try_from(index).ok())
}

#[test]
fn unreadable_declarations_do_not_shift_clap_indices() -> Result<()> {
    let operand_metadata = UnreadableOperandArgs::get_doc_metadata();
    ensure!(
        field(&operand_metadata, "proposal")?.cli.is_none(),
        "an unreadable operand publishes no CLI metadata"
    );
    ensure_positional(field(&operand_metadata, "second")?, 2, "SECOND")?;
    ensure!(
        clap_positional_index::<UnreadableOperandArgs>("second") == Some(2),
        "clap should place the declared operand at index 2"
    );

    let option_metadata = UnreadableOptionArgs::get_doc_metadata();
    ensure!(
        field(&option_metadata, "check")?.cli.is_none(),
        "an unreadable option publishes no CLI metadata"
    );
    ensure_positional(field(&option_metadata, "input")?, 1, "INPUT")?;
    ensure!(
        clap_positional_index::<UnreadableOptionArgs>("input") == Some(1),
        "a named option should not consume a positional index"
    );
    Ok(())
}

#[test]
fn unreadable_flag_spellings_keep_claps_indices() -> Result<()> {
    let metadata = RawSpellingArgs::get_doc_metadata();

    ensure!(
        field(&metadata, "check")?.cli.is_none(),
        "an unreadable spelling publishes no CLI metadata"
    );
    ensure_positional(field(&metadata, "input")?, 1, "INPUT")?;
    ensure!(
        clap_positional_index::<RawSpellingArgs>("input") == Some(1),
        "a named option should not consume a positional index"
    );
    Ok(())
}

#[test]
fn an_unreadable_explicit_index_does_not_shift_claps_indices() -> Result<()> {
    let metadata = RawIndexArgs::get_doc_metadata();

    ensure!(
        field(&metadata, "first")?.cli.is_none(),
        "an unreadable index publishes no CLI metadata"
    );
    ensure_positional(field(&metadata, "second")?, 1, "SECOND")?;
    ensure!(
        clap_positional_index::<RawIndexArgs>("second") == Some(1),
        "clap does not advance its counter for an indexed operand"
    );
    Ok(())
}

#[test]
fn an_operand_after_a_flattened_child_publishes_no_index() -> Result<()> {
    let metadata = FlattenedOperandArgs::get_doc_metadata();

    ensure!(
        field(&metadata, "input")?.cli.is_none(),
        "an implicit index after a flattened child is a guess"
    );
    ensure!(
        clap_positional_index::<FlattenedOperandArgs>("input") == Some(2),
        "clap numbers the parent operand after the child's operand"
    );
    Ok(())
}

#[test]
fn a_path_mount_keeps_the_payload_namespace() -> Result<()> {
    let command_path = [String::from("root"), String::from("shown")];
    let metadata = ValidateArgs::get_doc_metadata_for_path(&command_path);

    ensure!(
        metadata.about_id == "root-shown-about",
        "localization identifiers should follow the supplied path, got {}",
        metadata.about_id
    );
    ensure!(
        field(&metadata, "proposal")?
            .env
            .as_ref()
            .map(|env| env.var_name.as_str())
            == Some("SPLITTERS_CMDS_VALIDATE_PROPOSAL"),
        "the merge namespace should follow the payload command name"
    );
    ensure!(
        field(&metadata, "check")?
            .file
            .as_ref()
            .map(|file| file.key_path.as_str())
            == Some("cmds.validate.check"),
        "the file key should follow the payload command name"
    );

    let unnamed_path = [String::from("root"), String::from("shown")];
    let unnamed = UnnamedArgs::get_doc_metadata_for_path(&unnamed_path);
    ensure!(
        field(&unnamed, "token")?
            .env
            .as_ref()
            .map(|env| env.var_name.as_str())
            == Some("SPLITTERS_CMDS_ORTHO_CONFIG_TOKEN"),
        "a Parser payload should project its package-name command at any mount"
    );
    Ok(())
}

#[test]
fn a_parser_payload_without_a_name_uses_claps_package_default() -> Result<()> {
    let mut metadata = UnnamedCommands::get_subcommand_doc_metadata();
    ensure!(
        metadata.len() == 1,
        "expected one subcommand node, got {}",
        metadata.len()
    );
    let run = metadata.remove(0);
    ensure!(
        run.app_name == "run",
        "the enum label should remain the application name, got {:?}",
        run.app_name
    );

    let token = field(&run, "token")?;
    ensure!(
        token.env.as_ref().map(|env| env.var_name.as_str())
            == Some("SPLITTERS_CMDS_ORTHO_CONFIG_TOKEN"),
        "the merge reads clap's package-name command name, got {:?}",
        token.env
    );
    ensure!(
        token.file.as_ref().map(|file| file.key_path.as_str()) == Some("cmds.ortho_config.token"),
        "the file key should follow the same command name, got {:?}",
        token.file
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
