//! Tests for the identifier-generation pass, base resolution, exclusion
//! rules and the collision message contract.

use super::*;
use crate::derive::parse::parse_input;
use anyhow::{Result, anyhow, ensure};
use syn::parse_quote;

fn model_for(input: &syn::DeriveInput) -> Result<LocalizationIds> {
    let (ident, fields, struct_attrs, field_attrs) =
        parse_input(input).map_err(|err| anyhow!(err))?;
    generate_localization_ids(&struct_attrs, &ident, &fields, &field_attrs)
        .map_err(|err| anyhow!(err))
}

fn expect_model_error(input: &syn::DeriveInput) -> String {
    model_for(input)
        .expect_err("model should reject this input")
        .to_string()
}

#[test]
fn explicit_localization_base_is_used_verbatim() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        #[ortho_config(localization_base = "hello_world.cli")]
        struct App {
            recipient: String,
        }
    };
    let model = model_for(&input)?;
    ensure!(
        model.base == "hello_world.cli",
        "base mismatch: {}",
        model.base
    );
    ensure!(
        model.command.about_id == "hello_world-cli-about",
        "about id mismatch: {}",
        model.command.about_id
    );
    ensure!(model.args.len() == 1, "expected 1 arg model");
    let arg = model
        .args
        .first()
        .expect("args length was checked above: recipient");
    ensure!(arg.name == "recipient", "arg name mismatch: {}", arg.name);
    ensure!(
        arg.help_id == "hello_world-cli-args-recipient-help",
        "help id mismatch: {}",
        arg.help_id
    );
    Ok(())
}

#[test]
fn default_base_resolves_to_kebab_struct_name() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        struct AppConfig {
            port: u16,
        }
    };
    let model = model_for(&input)?;
    ensure!(
        model.base == "app_config",
        "default base should be kebabed struct name, got {}",
        model.base
    );
    ensure!(
        model.command.about_id == "app_config-about",
        "about id mismatch: {}",
        model.command.about_id
    );
    Ok(())
}

#[test]
fn subcommand_and_skip_cli_fields_are_excluded() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        #[ortho_config(localization_base = "acme.cli")]
        struct App {
            #[ortho_config(skip_cli)]
            hidden: String,
            #[command(subcommand)]
            command: AppCommand,
            visible: String,
        }
    };
    let model = model_for(&input)?;
    ensure!(
        model.args.len() == 1,
        "expected 1 arg model, got {}",
        model.args.len()
    );
    let visible = model
        .args
        .first()
        .expect("args length was checked above: visible");
    ensure!(
        visible.name == "visible",
        "only the visible field should be in ARG_IDS"
    );
    Ok(())
}

#[test]
fn flattened_fields_are_excluded() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        #[ortho_config(localization_base = "acme.cli")]
        struct App {
            #[command(flatten)]
            common: CommonArgs,
            visible: String,
        }
    };
    let model = model_for(&input)?;
    ensure!(
        model.args.len() == 1,
        "expected 1 arg model, got {}",
        model.args.len()
    );
    let visible = model
        .args
        .first()
        .expect("args length was checked above: visible");
    ensure!(
        visible.name == "visible",
        "flattened field must be excluded from ARG_IDS"
    );
    Ok(())
}

#[test]
fn colliding_normalised_arg_ids_fail_with_pinned_message() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        #[ortho_config(localization_base = "acme.cli")]
        struct App {
            foo_bar: String,
            #[arg(id = "foo-bar")]
            other: String,
        }
    };
    let message = expect_model_error(&input);
    ensure!(
        message.contains("duplicate localized argument id 'foo-bar'"),
        "message must name the colliding normalised id: {message}"
    );
    ensure!(
        message.contains("for field 'foo_bar' and 'other'"),
        "message must name both fields: {message}"
    );
    ensure!(
        message.contains("rename the field or set `#[arg(id = \"…\")]`"),
        "message must carry the remediation hint: {message}"
    );
    // The "first defined here" note is emitted via `err.combine` and only
    // renders in the rustc diagnostic (trybuild `.stderr`, pinned in
    // Milestone 3); `syn::Error::to_string` intentionally omits it.
    Ok(())
}

#[test]
fn dotted_arg_id_is_normalised_per_segment() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        #[ortho_config(localization_base = "acme.cli")]
        struct App {
            #[arg(id = "kebab.tail")]
            field: String,
        }
    };
    let model = model_for(&input)?;
    let arg = model.args.first().ok_or_else(|| anyhow!("missing arg"))?;
    ensure!(
        arg.help_id == "acme-cli-args-kebab-tail-help",
        "dotted arg id must be normalised per segment: {}",
        arg.help_id
    );
    Ok(())
}

#[test]
fn empty_path_without_root_is_rejected() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        #[ortho_config(localization_base = "")]
        struct App {
            field: String,
        }
    };
    let message = expect_model_error(&input);
    ensure!(
        message.contains("segment must not be empty"),
        "empty base should be rejected: {message}"
    );
    Ok(())
}

#[test]
fn localization_default_attribute_is_rejected_at_parse_time() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        #[ortho_config(localized_default = "Hello")]
        struct App {
            field: String,
        }
    };
    let Err(error) = parse_input(&input) else {
        return Err(anyhow!("localized_default must be rejected"));
    };
    let message = error.to_string();
    ensure!(
        message.contains("`localized_default` is not yet implemented"),
        "deferral message must name the attribute: {message}"
    );
    ensure!(
        message.contains("§8.2"),
        "deferral message must point at the design section: {message}"
    );
    Ok(())
}
