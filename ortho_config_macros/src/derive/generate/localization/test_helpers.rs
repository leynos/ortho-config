//! Shared helpers for the localization identifier-generation tests.
//!
//! The helpers convert `syn` and `anyhow` errors into one `Result` shape so
//! the test bodies can stay focused on the identifiers under assertion.

use super::*;
use crate::derive::parse::parse_input;
use anyhow::{Result, anyhow, ensure};

/// Parses `input` and runs the identifier-generation pass over it.
///
/// The two `syn` errors are converted to `anyhow` so callers can use `?` in a
/// `Result<(), anyhow::Error>` test without a conversion shim at each site.
pub(super) fn model_for(input: &syn::DeriveInput) -> Result<LocalizationIds> {
    let (ident, fields, struct_attrs, field_attrs) =
        parse_input(input).map_err(|err| anyhow!(err))?;
    generate_localization_ids(&struct_attrs, &ident, &fields, &field_attrs)
        .map_err(|err| anyhow!(err))
}

/// Asserts that `segments` join to `expected`, naming `context` on failure.
pub(super) fn assert_fluent_message_id(
    segments: &FluentSegments,
    expected: &str,
    context: &str,
) -> Result<()> {
    let id = segments.join(proc_macro2::Span::call_site())?;
    ensure!(
        id.as_ref() == expected,
        "{context}: expected `{expected}`, got `{}`",
        id.as_ref()
    );
    Ok(())
}

/// Asserts that `input` yields exactly one argument model, named `visible`.
///
/// The exclusion cases share this shape: their inputs differ only in which
/// field carries the excluding attribute, so one assertion serves them all.
pub(super) fn assert_only_visible_arg(input: &syn::DeriveInput, context: &str) -> Result<()> {
    let model = model_for(input)?;
    ensure!(
        model.args.len() == 1,
        "{context}: expected 1 arg model, got {}",
        model.args.len()
    );
    let visible = model
        .args
        .first()
        .ok_or_else(|| anyhow!("{context}: args length was checked above"))?;
    ensure!(
        visible.name.as_ref() == "visible",
        "{context}: only the visible field should be in ARG_IDS"
    );
    Ok(())
}

/// Returns the diagnostic `input` is rejected with.
///
/// Panics if the model accepts the input, so a test that expects rejection
/// fails loudly rather than asserting against an empty message.
pub(super) fn expect_model_error(input: &syn::DeriveInput) -> String {
    model_for(input)
        .expect_err("model should reject this input")
        .to_string()
}

/// The two fragments every `localized_default` deferral must quote.
///
/// Both spellings of the attribute are refused by the same diagnostic, so the
/// phrases are named once here rather than restated per spelling.
const LOCALIZED_DEFAULT_REFUSAL: [&str; 2] = ["`localized_default` is not yet implemented", "§8.2"];

/// Asserts the deferred `localized_default` diagnostic for one `input`.
///
/// `spelling` names which placement of the attribute is under test, because
/// the two placements reach the same diagnostic through different parsers: a
/// field-level key that is not recognised is discarded in silence and never
/// reaches a catalogue, so the field spelling needs a rejection of its own.
/// The spelling is also what the acceptance message quotes when the input is
/// not rejected at all.
pub(super) fn assert_localized_default_is_refused(
    input: &syn::DeriveInput,
    spelling: &str,
) -> Result<()> {
    let Err(error) = parse_input(input) else {
        return Err(anyhow!(
            "a {spelling} localized_default must be rejected rather than \
             discarded as unknown"
        ));
    };
    let message = error.to_string();
    for expected in LOCALIZED_DEFAULT_REFUSAL {
        ensure!(
            message.contains(expected),
            "the {spelling} deferral message must contain `{expected}`: {message}"
        );
    }
    Ok(())
}
