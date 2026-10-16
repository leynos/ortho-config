//! Bridge tests for profile-selection declarations.
//!
//! A profile declaration only means something if the renamed selector is
//! actually invocable, so these cases assert the command surface rather than
//! the contract alone: every emitted command must accept the declared long
//! option. That check is deliberately stronger than a name comparison, which
//! would pass for an input that shares the flag's name while exposing a
//! different long option.

use crate::schema::ValueType;

use super::support::*;
use crate::agent_context::bridge_ir_to_agent_context;

/// The bridge maps profile-enabled IR into a supported `ProfilesDeclaration`.
#[test]
fn transform_maps_profile_metadata_into_supported_declaration() {
    let mut metadata = doc(DocSpec::root_without_fields());
    metadata.profiles = Some(crate::schema::DocProfilesMeta {
        flag: String::from("profile"),
        env_var: String::from("APP_PROFILE"),
    });
    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    assert!(context.profiles.supported);
    let selection = context
        .profiles
        .selection
        .as_ref()
        .expect("selection contract");
    assert_eq!(selection.flag, "profile");
    assert_eq!(selection.env_var, "APP_PROFILE");
    // The contract is useless if it names a flag no command accepts: an agent
    // would be told which flag selects a profile and then find it absent from
    // every invocable surface. Assert the long option, not the input name: an
    // input can share the flag's name while exposing a different long option,
    // which leaves the contract naming a flag the command does not accept.
    assert!(
        !context.commands.is_empty(),
        "the bridge must emit at least a root command to assert against"
    );
    for command in &context.commands {
        assert!(
            command
                .inputs
                .iter()
                .any(|input| input.long.as_deref() == Some(selection.flag.as_str())),
            "command {:?} does not expose the contract's selector flag `--{}`",
            command.path,
            selection.flag
        );
    }
}

/// An input that shares the selector's name but exposes a different long
/// option must not suppress the injected selector.
///
/// The name/long split is what makes this reachable: a configuration field
/// called `profile` that clap renames to `--profile-name` produces an input
/// named `profile` with long `profile-name`. Treating that as the selector
/// would leave the selection contract pointing at `--profile`, which no
/// command accepts.
#[test]
fn transform_injects_selector_when_a_same_named_input_has_another_long() {
    let mut metadata = doc(DocSpec {
        app_name: "demo",
        bin_name: Some("demo-bin"),
        about_id: "root.about",
        fields: vec![cli_field(FieldSpec {
            name: "profile",
            long: Some("profile-name"),
            short: None,
            takes_value: true,
            hide_in_help: false,
            value: Some(ValueType::String),
            default: None,
            required: false,
        })],
        subcommands: Vec::new(),
        behaviour: None,
    });
    metadata.profiles = Some(crate::schema::DocProfilesMeta {
        flag: String::from("profile"),
        env_var: String::from("APP_PROFILE"),
    });
    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    let selection = context
        .profiles
        .selection
        .as_ref()
        .expect("selection contract");
    assert!(context.profiles.supported);
    for command in &context.commands {
        assert!(
            command
                .inputs
                .iter()
                .any(|input| input.long.as_deref() == Some(selection.flag.as_str())),
            "command {:?} does not expose the contract's selector flag `--{}`",
            command.path,
            selection.flag
        );
    }
}

/// The bridge leaves the declaration unsupported for legacy IR.
#[test]
fn transform_leaves_declaration_unsupported_for_legacy_ir() {
    let metadata = doc(DocSpec::root_without_fields());
    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    assert!(!context.profiles.supported);
    assert!(context.profiles.selection.is_none());
}
