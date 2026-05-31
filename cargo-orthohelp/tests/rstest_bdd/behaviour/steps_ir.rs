//! IR assertion step definitions for `cargo-orthohelp` behavioural tests.

use std::io::Read;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::then;
use serde_json::Value;

use super::steps::{OrthoHelpContext, StepResult, get_out_dir};

#[then("the output contains localized IR JSON for {locale}")]
fn output_contains_locale(
    orthohelp_context: &mut OrthoHelpContext,
    locale: String,
) -> StepResult<()> {
    let succeeded = orthohelp_context
        .last_output
        .with_ref(|output| output.status.success())
        .ok_or("last_output should be set")?;
    if !succeeded {
        return Err("cargo-orthohelp should succeed".into());
    }

    let out_root = get_out_dir(orthohelp_context)?;
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())?;
    let mut file = dir.open(Utf8PathBuf::from(format!("ir/{locale}.json")))?;

    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;

    let json: Value = serde_json::from_str(&buffer)?;
    let ir_version = json
        .get("ir_version")
        .and_then(Value::as_str)
        .ok_or("ir_version field missing")?;
    assert_eq!(
        ir_version,
        ortho_config::docs::ORTHO_DOCS_IR_VERSION,
        "IR version should match schema"
    );
    let json_locale = json
        .get("locale")
        .and_then(Value::as_str)
        .ok_or("locale field missing")?;
    assert_eq!(json_locale, locale);
    let about = json
        .get("about")
        .and_then(Value::as_str)
        .ok_or("about field missing")?;
    assert_eq!(about, expected_about(&locale));

    let help = json
        .get("fields")
        .and_then(Value::as_array)
        .and_then(|fields| fields.first())
        .and_then(|field| field.get("help"))
        .and_then(Value::as_str)
        .ok_or("field help missing")?;
    assert_eq!(help, expected_help(&locale));
    Ok(())
}

fn expected_about(locale: &str) -> &'static str {
    match locale {
        "fr-FR" => "Configuration du fixture Orthohelp.",
        _ => "Orthohelp fixture configuration.",
    }
}

fn expected_help(locale: &str) -> &'static str {
    match locale {
        "fr-FR" => "Port utilisé par le service de test.",
        _ => "Port used by the fixture service.",
    }
}
