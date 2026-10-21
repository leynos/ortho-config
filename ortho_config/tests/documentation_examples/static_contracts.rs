//! Static contracts for documented data and command examples.
//!
//! These examples are configuration files or commands whose full execution
//! is covered elsewhere: the Rust flows run the programs that read the files,
//! `identifier_artefact_e2e.rs` runs the identifier-export build, and
//! `cargo-orthohelp/tests/documented_command.rs` runs the policy check. The
//! assertions here pin the published text to the shapes those flows rely on.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::toml;

use crate::documentation_examples::documented_example;
use crate::{parse_toml, repository_root};

/// The profile file defines a `staging` overlay and no reserved profile keys.
#[test]
fn profile_file_declares_a_valid_staging_overlay() -> Result<()> {
    let file = parse_toml("guide-profiles-file")?;
    ensure!(file.get("log_level").and_then(toml::Value::as_str) == Some("info"));
    let profiles = file
        .get("profile")
        .and_then(toml::Value::as_table)
        .context("the file should declare [profile.*] tables")?;
    ensure!(
        !profiles.contains_key("default"),
        "`default` is a reserved profile name"
    );
    let staging = profiles
        .get("staging")
        .context("the file should declare [profile.staging]")?;
    ensure!(staging.get("log_level").and_then(toml::Value::as_str) == Some("debug"));
    ensure!(staging.get("port").and_then(toml::Value::as_integer) == Some(9000));
    for forbidden in ["cmds", "inherits"] {
        ensure!(
            staging.get(forbidden).is_none(),
            "profile bodies must not contain `{forbidden}`"
        );
    }
    Ok(())
}

/// The migration guide's boolean file supplies the `true` the run clears.
#[test]
fn migration_boolean_file_configures_true() -> Result<()> {
    let file = parse_toml("migration-boolean-file")?;
    ensure!(file.get("excited").and_then(toml::Value::as_bool) == Some(true));
    Ok(())
}

/// The precedence summary names the five tiers in their applied order.
///
/// `documentation_examples/guide_flows.rs` demonstrates each adjacent pair at
/// run time; this assertion keeps the one-line summary in step with it.
#[test]
fn migration_profile_precedence_lists_five_tiers() -> Result<()> {
    let example = documented_example("migration-profile-precedence")?;
    ensure!(example.language == "plaintext");
    ensure!(
        example.body
            == "built-in defaults < config files < selected profile < environment < flags\n",
        "profile precedence summary drifted"
    );
    Ok(())
}

/// The identifier-export command uses the variables the macro crate reads.
///
/// `identifier_artefact_e2e.rs` runs this clean-then-build sequence against a
/// fixture crate, so this contract only has to keep the published spelling
/// and the implementation's variable names aligned.
#[test]
fn identifier_artefact_command_names_the_emission_variable() -> Result<()> {
    let example = documented_example("guide-identifier-artefact")?;
    ensure!(example.language == "console");
    ensure!(
        example.body
            == concat!(
                "cargo clean -p <package> && ",
                "ORTHO_CONFIG_EMIT_IDENTIFIERS=1 cargo build -p <package>\n",
            ),
        "identifier-export command drifted"
    );

    let macros = Dir::open_ambient_dir(
        repository_root()
            .join("ortho_config_macros/src")
            .as_std_path(),
        ambient_authority(),
    )
    .context("open the macro crate sources")?;
    for (path, variable) in [
        (
            "derive/generate/localization/artefact/mod.rs",
            "ORTHO_CONFIG_EMIT_IDENTIFIERS",
        ),
        (
            "derive/generate/localization/artefact/artefact_report.rs",
            "ORTHO_CONFIG_ARTEFACT_REPORT",
        ),
    ] {
        let source = macros
            .read_to_string(path)
            .with_context(|| format!("read ortho_config_macros/src/{path}"))?;
        ensure!(
            source.contains(&format!("\"{variable}\"")),
            "ortho_config_macros/src/{path} should read {variable}"
        );
    }
    Ok(())
}

/// The policy check command and CI recipe are the flags that
/// `cargo-orthohelp/tests/documented_command.rs` runs against the policy
/// fixtures.
#[test]
fn policy_check_commands_match_the_executed_flow() -> Result<()> {
    let check = documented_example("guide-policy-check")?;
    ensure!(check.language == "console");
    let command = "cargo orthohelp --check-agent-native --package my-cli --out-dir out\n";
    ensure!(check.body == command, "policy check command drifted");
    let gate = documented_example("guide-policy-ci")?;
    ensure!(
        gate.body == format!("{command}jq -e '.mode != \"off\"' out/policy-report.json\n"),
        "policy CI recipe drifted"
    );
    Ok(())
}

/// The documented policy table is the one the warn fixture carries, so the
/// executed policy check exercises the documented configuration rather than a
/// copy of it.
#[test]
fn documented_policy_table_matches_the_warn_fixture() -> Result<()> {
    let documented = parse_toml("guide-policy-config")?;
    let fixture: toml::Value = toml::from_str(
        &Dir::open_ambient_dir(repository_root().as_std_path(), ambient_authority())?
            .read_to_string("tests/fixtures/orthohelp_policy_warn_fixture/Cargo.toml")
            .context("read the warn policy fixture manifest")?,
    )?;
    let policy = |manifest: &toml::Value| {
        manifest
            .get("package")
            .and_then(|package| package.get("metadata"))
            .and_then(|metadata| metadata.get("ortho_config"))
            .and_then(|ortho| ortho.get("policy"))
            .cloned()
    };
    ensure!(
        policy(&documented).is_some() && policy(&documented) == policy(&fixture),
        "guide-policy-config should match the warn fixture's policy table"
    );
    Ok(())
}
