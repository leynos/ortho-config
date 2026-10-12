//! Tests for the identifier-generation pass, base resolution, exclusion
//! rules and the collision message contract.

use rstest::rstest;
use syn::parse_quote;

use super::test_helpers::{
    assert_fluent_message_id, assert_only_visible_arg, expect_model_error, model_for,
};
use super::*;
use crate::derive::parse::parse_input;
use anyhow::{Result, anyhow, ensure};

/// A numeric argument id is a valid *fragment*, even though it would be an
/// invalid root. Clap allows `#[arg(id = "123")]`, and the runtime builds
/// `acme-args-123-help` because it validates only the joined id's first
/// character. The derive must not be stricter than the lookup it mirrors.
#[test]
fn numeric_argument_id_composes_with_the_base_root() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        #[ortho_config(localization_base = "acme")]
        struct App {
            #[arg(id = "123")]
            moment: String,
        }
    };
    let model = model_for(&input)?;
    let arg = model
        .args
        .first()
        .ok_or_else(|| anyhow!("numeric argument id must produce an arg model"))?;
    ensure!(
        arg.help_id.as_ref() == "acme-args-123-help",
        "numeric argument id must compose with the base root, got `{}`",
        arg.help_id.as_ref()
    );
    ensure!(
        arg.long_help_id.as_ref() == "acme-args-123-long_help",
        "numeric argument id must compose its long-help id, got `{}`",
        arg.long_help_id.as_ref()
    );
    Ok(())
}

/// A dotted base splits on `.` and each segment normalizes separately.
///
/// The dots are structural, not literal: `Acme.CLI` becomes the two Fluent
/// segments `acme` and `cli`, joined with `-` rather than kept verbatim.
#[test]
fn dotted_localization_base_normalizes_to_fluent_segments() -> Result<()> {
    let base = LocalizationBase("Acme.CLI".to_owned());
    let segments = base.normalize(proc_macro2::Span::call_site())?;
    assert_fluent_message_id(
        &segments,
        "acme-cli",
        "dotted localization base should normalize per segment",
    )
}

/// A dotted `clap` argument id splits and normalizes the same way as a base.
///
/// Both identifier producers share `normalize_segment`, so this case guards
/// against the two paths drifting apart on dotted input.
#[test]
fn dotted_clap_arg_id_normalizes_to_fluent_segments() -> Result<()> {
    let arg_id = ClapArgId("Kebab.TAIL".to_owned());
    let segments = arg_id.normalize(proc_macro2::Span::call_site())?;
    assert_fluent_message_id(
        &segments,
        "kebab-tail",
        "dotted clap id should normalize per segment",
    )
}

/// The suffix vocabulary converts to the wire spellings the runtime looks up.
///
/// `LongAbout` and `ValueName` are sampled because they are the two suffixes
/// where the Rust variant name and the Fluent segment differ most.
#[test]
fn message_suffixes_convert_to_their_fluent_segments() -> Result<()> {
    ensure!(
        String::from(MessageSuffix::LongAbout) == "long_about",
        "long-about suffix conversion changed"
    );
    ensure!(
        MessageSuffix::ValueName.as_ref() == "value_name",
        "value-name suffix conversion changed"
    );
    Ok(())
}

/// The two invalid-base diagnostics keep their exact wording.
///
/// An empty segment and a non-letter root are distinct failures and must stay
/// distinguishable: pinning both messages stops a refactor from collapsing
/// them into one less specific error.
#[test]
fn domain_types_preserve_invalid_segment_and_root_diagnostics() -> Result<()> {
    let invalid = LocalizationBase("acme..cli".to_owned())
        .normalize(proc_macro2::Span::call_site())
        .err()
        .ok_or_else(|| anyhow!("empty localization-base segment must fail"))?;
    ensure!(
        invalid.to_string() == "invalid Fluent identifier segment: segment must not be empty",
        "invalid segment diagnostic changed: {invalid}"
    );

    let segments = LocalizationBase("123".to_owned()).normalize(proc_macro2::Span::call_site())?;
    let invalid_root = segments
        .ensure_leading_ascii_letter(proc_macro2::Span::call_site())
        .err()
        .ok_or_else(|| anyhow!("numeric localization root must fail"))?;
    ensure!(
        invalid_root
            .to_string()
            .contains("Fluent identifier must start with an ASCII letter: \"123\""),
        "non-letter root diagnostic changed: {invalid_root}"
    );
    Ok(())
}

/// An explicit dot-free base names the root exactly as written.
///
/// The base is not run through the segment normalizer when it contains no
/// dots, so a `snake_case` root such as `hello_world` survives unchanged.
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
        model.base.as_ref() == "hello_world.cli",
        "base mismatch: {}",
        model.base.as_ref()
    );
    ensure!(
        model.command.about_id.as_ref() == "hello_world-cli-about",
        "about id mismatch: {}",
        model.command.about_id.as_ref()
    );
    ensure!(model.args.len() == 1, "expected 1 arg model");
    let arg = model
        .args
        .first()
        .expect("args length was checked above: recipient");
    ensure!(
        arg.name.as_ref() == "recipient",
        "arg name mismatch: {}",
        arg.name.as_ref()
    );
    ensure!(
        arg.help_id.as_ref() == "hello_world-cli-args-recipient-help",
        "help id mismatch: {}",
        arg.help_id.as_ref()
    );
    Ok(())
}

/// With no `localization_base`, the struct name kebabed becomes the root.
///
/// `AppConfig` resolves to `app-config`, which is the documented fallback and
/// the value every generated id is then composed under.
#[test]
fn default_base_resolves_to_kebab_struct_name() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        struct AppConfig {
            port: u16,
        }
    };
    let model = model_for(&input)?;
    ensure!(
        model.base.as_ref() == "app-config",
        "default base should be kebabed struct name, got {}",
        model.base.as_ref()
    );
    ensure!(
        model.command.about_id.as_ref() == "app-config-about",
        "about id mismatch: {}",
        model.command.about_id.as_ref()
    );
    Ok(())
}

/// `skip_cli` and subcommand fields produce no argument identifiers.
///
/// Neither field reaches the CLI, so neither has help text to localize; only
/// the plain field should appear in the generated argument model.
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
    assert_only_visible_arg(&input, "skip_cli and subcommand fields")
}

/// A `#[command(flatten)]` field produces no argument identifiers.
///
/// The flattened struct owns its own fields' ids, so emitting one here would
/// duplicate identifiers the flattened type already defines.
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
    assert_only_visible_arg(&input, "flattened field")
}

/// Two argument ids that normalize alike are rejected, with remediation.
///
/// The collision is real rather than assumed: normalization only lowercases,
/// so a plain field name and an explicitly upper-cased `arg(id = ...)` are the
/// only way to reach it. The message must name the shared id, both fields, and
/// how to rename out of the clash.
#[test]
fn colliding_normalized_arg_ids_fail_with_pinned_message() -> Result<()> {
    // The collision must be genuine, not assumed. `normalize_segment` only
    // lowercases; it preserves `-` and `_`, so `foo_bar` and `foo-bar` are
    // *distinct* segments even though they are the same `clap` flag. Clap also
    // keeps the raw Rust field name as the argument id, so two plain field
    // names can only collide through case alone.
    let input: syn::DeriveInput = parse_quote! {
        #[ortho_config(localization_base = "acme.cli")]
        struct App {
            foo_bar: String,
            #[arg(id = "FOO_BAR")]
            other: String,
        }
    };
    let message = expect_model_error(&input);
    ensure!(
        message.contains("duplicate localized argument id 'foo_bar'"),
        "message must name the colliding normalized id: {message}"
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

/// An argument id containing dots normalizes each segment independently.
///
/// `kebab.tail` becomes `kebab-tail`, which is what the runtime builds when it
/// looks the help id up, so the derive must produce the same spelling.
#[test]
fn dotted_arg_id_is_normalized_per_segment() -> Result<()> {
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
        arg.help_id.as_ref() == "acme-cli-args-kebab-tail-help",
        "dotted arg id must be normalized per segment: {}",
        arg.help_id.as_ref()
    );
    Ok(())
}

/// The base must be rejected with a diagnostic that names the rule it broke.
///
/// The two cases cover the two ways a base can be unusable as a Fluent root,
/// and they are asymmetric on purpose. An empty segment fails the empty-segment
/// rule. A numeric base fails the ASCII-letter rule, because there the segment
/// *is* the root and a Fluent message id cannot start with a digit. Contrast
/// `numeric_argument_id_composes_with_the_base_root`, where the same `123` is
/// a valid argument *fragment* under an alphabetic root.
#[rstest]
#[case("", "segment must not be empty", "empty base")]
#[case(
    "123",
    "Fluent identifier must start with an ASCII letter",
    "numeric root"
)]
/// A base is rejected with a diagnostic that names the rule it broke.
///
/// The two cases cover the two ways a base can be unusable as a Fluent root,
/// and they are asymmetric on purpose. An empty segment fails the empty-segment
/// rule. A numeric base fails the ASCII-letter rule, because there the segment
/// *is* the root and a Fluent message id cannot start with a digit. Contrast
/// `numeric_argument_id_composes_with_the_base_root`, where the same `123` is
/// a valid argument *fragment* under an alphabetic root.
fn rejected_localization_base_reports_its_reason(
    #[case] base: &str,
    #[case] expected: &str,
    #[case] context: &str,
) -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        #[ortho_config(localization_base = #base)]
        struct App {
            field: String,
        }
    };
    let message = expect_model_error(&input);
    ensure!(
        message.contains(expected),
        "{context} must be rejected with a reason naming the rule; \
         expected `{expected}` in `{message}`"
    );
    Ok(())
}

/// The deferred `localized_default` attribute fails with a pointer to why.
///
/// The attribute is deliberately unimplemented, so the diagnostic must name
/// the attribute and cite the design section rather than failing obscurely.
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
