//! Tests for container-level clap command attributes.

use crate::derive::parse::clap_command_name;
use anyhow::{Result, anyhow, ensure};
use syn::{DeriveInput, parse_quote};

#[test]
fn clap_command_name_accepts_both_attribute_forms() -> Result<()> {
    let cases: &[(&str, DeriveInput)] = &[
        (
            "name = \"deploy\"",
            parse_quote! {
                #[command(name = "deploy")]
                struct Demo;
            },
        ),
        (
            "name(\"deploy\")",
            parse_quote! {
                #[clap(name("deploy"))]
                struct Demo;
            },
        ),
    ];

    for (form, input) in cases {
        let name = clap_command_name(&input.attrs)?
            .ok_or_else(|| anyhow!("missing command name for {form}"))?;
        ensure!(
            name == "deploy",
            "expected `deploy` from {form}, got `{name}`",
        );
    }
    Ok(())
}

#[test]
fn an_absent_command_name_is_not_invented() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[command(rename_all = "snake_case")]
        struct Demo {
            #[arg(long)]
            output_file: String,
        }
    };

    ensure!(
        clap_command_name(&input.attrs)?.is_none(),
        "a struct without a name override must not report one",
    );
    Ok(())
}
