//! Bridge tests for profile redaction records and their disclosure boundary.

use super::support::*;
use crate::agent_context::bridge_ir_to_agent_context;
use crate::schema::{
    DocProfilesMeta, FieldMetadata, FileMetadata, ProfileFieldExposure, ValueType,
};

fn profiles() -> DocProfilesMeta {
    DocProfilesMeta {
        flag: "profile".to_owned(),
        env_var: "APP_PROFILE".to_owned(),
    }
}

fn secret_field() -> FieldMetadata {
    let mut field = cli_field_with_possible_values(
        FieldSpec {
            name: "token",
            long: Some("token"),
            short: None,
            takes_value: true,
            hide_in_help: false,
            value: Some(ValueType::Enum {
                variants: vec!["ALPHA".to_owned(), "BETA".to_owned()],
            }),
            default: Some("TOKEN-SENTINEL"),
            required: false,
        },
        ["ALPHA", "BETA"],
    );
    field.profile_field = Some(ProfileFieldExposure::Secret);
    field.file = Some(FileMetadata {
        key_path: "auth.token".to_owned(),
    });
    field
}

/// Returns the sole redaction record, which these fixtures declare one of.
fn only_record(context: &ortho_config::AgentContext) -> &ortho_config::ProfileFieldRedaction {
    context
        .profiles
        .redaction()
        .first()
        .expect("the fixture declares one protected field")
}

/// Returns the root command, which the bridge always emits first.
fn root_command(context: &ortho_config::AgentContext) -> &ortho_config::AgentCommand {
    context
        .commands
        .first()
        .expect("the bridge always emits the root command")
}

/// Returns the named invocable input of the root command, if it has one.
fn root_input<'a>(
    context: &'a ortho_config::AgentContext,
    name: &str,
) -> Option<&'a ortho_config::AgentInput> {
    root_command(context)
        .inputs
        .iter()
        .find(|input| input.name == name)
}

fn root(fields: Vec<FieldMetadata>) -> crate::schema::DocMetadata {
    doc(DocSpec {
        app_name: "demo",
        bin_name: Some("demo-bin"),
        about_id: "root.about",
        fields,
        subcommands: Vec::new(),
        behaviour: None,
    })
}

#[test]
fn protected_field_is_published_by_identity_only() {
    let mut metadata = root(vec![secret_field()]);
    metadata.profiles = Some(profiles());

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None).expect("bridge succeeds");

    assert_eq!(
        context.profiles.redaction().len(),
        1,
        "a single protected field must yield one redaction record"
    );
    let record = only_record(&context);
    assert_eq!(
        record.command_path,
        ["demo-bin"],
        "the record must identify the command that owns the protected field"
    );
    assert_eq!(
        record.field_name, "token",
        "the record must carry the emitted metadata field name"
    );
    assert_eq!(
        record.key_path.as_deref(),
        Some("auth.token"),
        "the record must retain the file key path verbatim"
    );
    assert_eq!(
        record.exposure,
        ProfileFieldExposure::Secret,
        "the declared secret classification must survive into the record"
    );

    let input = root_input(&context, "token").expect("the protected field is still invocable");
    assert!(
        input.default.is_none(),
        "the default must not reach context"
    );
    assert!(
        input.enum_values.is_empty(),
        "enumerated values must not reach context"
    );
}

#[test]
fn hidden_protected_field_is_recorded_without_becoming_an_input() {
    let mut hidden = non_cli_field("api_key");
    hidden.profile_field = Some(ProfileFieldExposure::ReferenceOnly);
    hidden.file = Some(FileMetadata {
        key_path: "auth.api_key".to_owned(),
    });
    let mut metadata = root(vec![hidden]);
    metadata.profiles = Some(profiles());

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None).expect("bridge succeeds");

    assert_eq!(
        context.profiles.redaction().len(),
        1,
        "the hidden field must still yield one redaction record"
    );
    assert_eq!(
        only_record(&context).field_name,
        "api_key",
        "the record must name the hidden protected field"
    );
    assert!(
        root_input(&context, "api_key").is_none(),
        "a config-only field is not an invocable input"
    );
}

#[test]
fn unsupported_declaration_never_gains_redaction_metadata() {
    let metadata = root(vec![secret_field()]);

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None).expect("bridge succeeds");

    assert!(
        !context.profiles.supported,
        "an undeclared profile surface must remain unsupported"
    );
    assert_eq!(
        context.profiles.redaction(),
        &[] as &[ortho_config::ProfileFieldRedaction],
        "an unsupported declaration must carry no redaction records"
    );
    let input = root_input(&context, "token").expect("the field is still invocable");
    assert!(
        input.default.is_none(),
        "projection still removes the default"
    );
    assert_eq!(
        input.enum_values,
        [] as [String; 0],
        "an unsupported declaration must not project enumerated values"
    );
}

#[test]
fn duplicate_identity_is_reported_rather_than_arbitrarily_chosen() {
    let mut metadata = root(vec![secret_field(), secret_field()]);
    metadata.profiles = Some(profiles());

    let error = bridge_ir_to_agent_context(&metadata, "demo_pkg", None)
        .expect_err("duplicate identities fail the bridge");

    assert!(
        matches!(
            error.as_ref(),
            ortho_config::OrthoError::DuplicateProfileRedaction { field_name, .. }
                if field_name == "token"
        ),
        "the duplicate identity must be reported with the colliding field name"
    );
}
