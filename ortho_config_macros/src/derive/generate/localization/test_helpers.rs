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
