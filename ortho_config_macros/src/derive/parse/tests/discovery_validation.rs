//! Tests for discovery-attribute validation errors raised by `parse_input`.
//!
//! The parser rejects a `project_root_from` that names a missing, skipped, or
//! wrongly typed field, and an `env_var`/`env_vars` pair that would otherwise
//! leave the env selector chain ambiguous.

use super::super::*;
use anyhow::{Result, anyhow, ensure};
use syn::{DeriveInput, parse_quote};

/// A field name that exists nowhere in the struct is refused by name.
#[test]
fn project_root_from_missing_field_errors() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(discovery(project_root_from = "absent_path"))]
        struct Demo {
            config_path: std::path::PathBuf,
        }
    };
    let err = parse_input(&input)
        .err()
        .ok_or_else(|| anyhow!("expected a project_root_from error"))?;
    let message = err.to_string();
    ensure!(
        message.contains("`project_root_from` must name a CLI PathBuf field"),
        "unexpected error message: {message}",
    );
    ensure!(
        message.contains("`absent_path` was not found"),
        "error message did not name the missing field: {message}",
    );
    Ok(())
}

/// A `#[ortho_config(skip_cli)]` `PathBuf` cannot supply a project root.
#[test]
fn project_root_from_rejects_skipped_field() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(discovery(project_root_from = "config_path"))]
        struct Demo {
            #[ortho_config(skip_cli)]
            config_path: std::path::PathBuf,
        }
    };
    let err = parse_input(&input)
        .err()
        .ok_or_else(|| anyhow!("expected a project_root_from error"))?;
    ensure!(
        err.to_string()
            == "`project_root_from` must name a non-skipped `PathBuf` or `Option<PathBuf>` field",
        "unexpected error message: {err}",
    );
    Ok(())
}

/// A `String` field is the wrong type, even when it is not skipped.
#[test]
fn project_root_from_rejects_non_pathbuf_field() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(discovery(project_root_from = "config_path"))]
        struct Demo {
            config_path: String,
        }
    };
    let err = parse_input(&input)
        .err()
        .ok_or_else(|| anyhow!("expected a project_root_from error"))?;
    ensure!(
        err.to_string()
            == "`project_root_from` must name a non-skipped `PathBuf` or `Option<PathBuf>` field",
        "unexpected error message: {err}",
    );
    Ok(())
}

/// Declaring both selector attributes leaves the chain order ambiguous.
#[test]
fn env_var_and_env_vars_conflict_errors() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(discovery(
            env_var = "DEMO_CONFIG",
            env_vars = ["PRIMARY_CONFIG"],
        ))]
        struct Demo {
            value: u32,
        }
    };
    let err = parse_input(&input)
        .err()
        .ok_or_else(|| anyhow!("expected an env selector conflict error"))?;
    ensure!(
        err.to_string() == "`env_var` and `env_vars` are mutually exclusive",
        "unexpected error message: {err}",
    );
    Ok(())
}

/// Both list keys accept the bracketed spelling, preserving written order.
#[test]
fn parses_env_vars_and_scope_order() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(discovery(
            env_vars = ["PRIMARY_CONFIG", "LEGACY_CONFIG"],
            scope_order = ["user", "project"],
        ))]
        struct Demo {
            value: u32,
        }
    };
    let (_, _, struct_attrs, _) = parse_input(&input).map_err(|err| anyhow!(err))?;
    let discovery = struct_attrs
        .discovery
        .ok_or_else(|| anyhow!("expected discovery attributes"))?;
    let expected_env_vars = ["PRIMARY_CONFIG", "LEGACY_CONFIG"].map(str::to_owned);
    let expected_scopes = ["user", "project"].map(str::to_owned);
    ensure!(
        discovery.env_vars == expected_env_vars,
        "env_vars order not preserved: {:?}",
        discovery.env_vars
    );
    ensure!(
        discovery.scope_order == expected_scopes,
        "scope_order order not preserved: {:?}",
        discovery.scope_order
    );
    Ok(())
}

/// Both list keys also accept the parenthesized spelling.
///
/// `assign_string_list` reads a list two ways, and the array form above takes
/// the first of them. This pins the second, which nothing else exercises: a
/// bracketed list and a parenthesized one are the same attribute by the time
/// the policy is emitted, so a regression in either parse must fail here. The
/// two spellings are syntactically distinct — the parenthesized form carries no
/// `=`, so it cannot fall into the array branch — which is why this is a second
/// test rather than another case in the first.
#[test]
fn parses_parenthesized_env_vars_and_scope_order() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(discovery(
            env_vars("PRIMARY_CONFIG", "LEGACY_CONFIG"),
            scope_order("user", "project"),
        ))]
        struct Demo {
            value: u32,
        }
    };
    let (_, _, struct_attrs, _) = parse_input(&input).map_err(|err| anyhow!(err))?;
    let discovery = struct_attrs
        .discovery
        .ok_or_else(|| anyhow!("expected discovery attributes"))?;
    let expected_env_vars = ["PRIMARY_CONFIG", "LEGACY_CONFIG"].map(str::to_owned);
    let expected_scopes = ["user", "project"].map(str::to_owned);
    ensure!(
        discovery.env_vars == expected_env_vars,
        "parenthesized env_vars order not preserved: {:?}",
        discovery.env_vars
    );
    ensure!(
        discovery.scope_order == expected_scopes,
        "parenthesized scope_order order not preserved: {:?}",
        discovery.scope_order
    );
    Ok(())
}
