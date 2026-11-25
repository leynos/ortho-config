//! Field-level `#[ortho_config(...)]` attribute parsing.
//!
//! The struct-level keys live in `mod.rs`; the field-level keys live here so
//! the two vocabularies stay separately readable and the parent module stays
//! within its line budget. Both entry points share `parse_ortho_config` and
//! `discard_unknown` from the parent.

use syn::Token;

use super::literals::{lit_char, lit_str};
use super::localization::reject_localized_default;
use super::{
    CliFieldMode, FieldAttrs, MergeStrategy, apply_field_doc_attr, clap_default_value,
    clap_field_is_flattened, clap_field_is_subcommand, discard_unknown, parse_ortho_config,
    reject_subcommand_ortho_config_attrs, serde_field_is_flattened,
};

/// The two disclosure classifications an author may declare on a field.
///
/// The label is syntax-level data until field emission converts it into the
/// runtime `ortho_config::docs::ProfileFieldExposure` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProfileFieldLabel {
    /// Confidential material whose value must never be published.
    Secret,
    /// A sensitive locator the application resolves elsewhere.
    ReferenceOnly,
}

impl ProfileFieldLabel {
    /// Parses the label text accepted by `profile_field = "..."`.
    fn parse(value: &syn::LitStr) -> syn::Result<Self> {
        match value.value().as_str() {
            "secret" => Ok(Self::Secret),
            "reference_only" => Ok(Self::ReferenceOnly),
            other => Err(syn::Error::new(
                value.span(),
                format!(
                    "unknown profile_field label '{other}'; expected \"secret\" or \
\"reference_only\""
                ),
            )),
        }
    }
}

/// A parsed `profile_field` annotation, including the span for diagnostics.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProfileFieldAnnotation {
    /// The declared classification.
    pub(crate) label: ProfileFieldLabel,
    /// Span of the label literal, used for parse and flatten diagnostics.
    pub(crate) span: proc_macro2::Span,
}

/// Parses field-level `#[ortho_config(...)]` attributes.
///
/// Recognised keys include `cli_long`, `cli_short`, `default`,
/// `merge_strategy`, `skip_cli`, and `cli_default_as_absent`. Unknown keys are
/// ignored, matching [`super::parse_struct_attrs`] for forwards compatibility.
/// This lenience may permit misspelt attribute names; users wanting stricter
/// validation can insert a manual `compile_error!` guard.
///
/// When `cli_default_as_absent` is active and no explicit `default` is
/// provided, this function attempts to infer a default from clap's default
/// attributes. String defaults retain the field parser metadata so generated
/// code can reproduce clap's conversion at runtime.
///
/// Used internally by the derive macro to extract configuration metadata
/// from field-level attributes.
pub(crate) fn parse_field_attrs(field: &syn::Field) -> Result<FieldAttrs, syn::Error> {
    let mut out = FieldAttrs {
        cli: if clap_field_is_subcommand(field)? {
            CliFieldMode::SubcommandSelector
        } else {
            CliFieldMode::Option
        },
        ..FieldAttrs::default()
    };
    if out.cli.is_subcommand() {
        reject_subcommand_ortho_config_attrs(field)?;
        return Ok(out);
    }
    parse_ortho_config(&field.attrs, |meta| {
        if !apply_field_attr(meta, &mut out)? {
            // Unknown attributes are intentionally discarded to preserve
            // forwards compatibility while still allowing callers to add
            // new keys in future versions.
            discard_unknown(meta)?;
        }
        Ok(())
    })?;
    if out.cli_default_as_absent && out.default.is_none() {
        out.inferred_clap_default = clap_default_value(field)?;
    }
    reject_profiled_flattened_container(field, out.profile_field)?;
    Ok(out)
}

/// Rejects `profile_field` on a container whose leaves the derive cannot see.
///
/// The docs builder does not expand flattened containers into descendant
/// metadata, so accepting the annotation here would promise protection the
/// generated metadata cannot provide. The author must mark the emitted leaf
/// fields on the separately derived child type instead.
fn reject_profiled_flattened_container(
    field: &syn::Field,
    annotation: Option<ProfileFieldAnnotation>,
) -> syn::Result<()> {
    let Some(declared) = annotation else {
        return Ok(());
    };
    let is_flattened = clap_field_is_flattened(field)? || serde_field_is_flattened(&field.attrs)?;
    if is_flattened {
        return Err(syn::Error::new(
            declared.span,
            "profile_field cannot be applied to a flattened container because its \
emitted metadata has no descendant fields; mark the leaf fields on the child type instead",
        ));
    }
    Ok(())
}

/// Applies a recognised field attribute, returning `true` if handled.
///
/// Unknown keys fall through to [`apply_field_doc_attr`], which reports
/// whether it recognised the key; a `false` there leaves the caller to
/// discard the value.
///
/// `localized_default` is recognised only to be rejected: it is deferred
/// (Decision D-4), and a key that is discarded as unknown would leave an
/// author who placed it on a field with no diagnostic at all — the value would
/// simply never appear in the catalogue. Rejecting here means the deferral is
/// reported wherever it is written, not only at struct level.
///
/// # Examples
///
/// ```rust,ignore
/// # use syn::meta::ParseNestedMeta;
/// # fn demo(meta: &ParseNestedMeta) -> syn::Result<()> {
/// let mut out = FieldAttrs::default();
/// if !apply_field_attr(meta, &mut out)? {
///     // unknown attribute
/// }
/// # Ok(())
/// # }
/// ```
fn apply_field_attr(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<bool, syn::Error> {
    let Some(ident) = meta.path.get_ident() else {
        return Ok(false);
    };
    let key = ident.to_string();
    let recognised = match key.as_str() {
        "localized_default" => Err(reject_localized_default(&meta.path)),
        "cli_long" => set_cli_long(meta, out),
        "cli_short" => set_cli_short(meta, out),
        "default" => set_default(meta, out),
        "merge_strategy" => set_merge_strategy(meta, out),
        // A bare flag: there is no value to parse, so it cannot fail.
        "skip_cli" => Ok(()).inspect(|()| out.cli = CliFieldMode::Skipped),
        "cli_default_as_absent" => set_cli_default_as_absent(meta, out),
        // `nested` is likewise bare; the child's schema is composed because of
        // this flag, so it must be recorded rather than discarded.
        "nested" => Ok(()).inspect(|()| out.nested = true),
        "profile_field" => set_profile_field(meta, out),
        _ => return apply_field_doc_attr(meta, out),
    };
    recognised.map(|()| true)
}

/// Records the `cli_long` flag name.
fn set_cli_long(meta: &syn::meta::ParseNestedMeta, out: &mut FieldAttrs) -> Result<(), syn::Error> {
    out.cli_long = Some(lit_str(meta, "cli_long")?.value());
    Ok(())
}

/// Records the `cli_short` flag character.
fn set_cli_short(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<(), syn::Error> {
    out.cli_short = Some(lit_char(meta, "cli_short")?);
    Ok(())
}

/// Records the explicit `default` expression.
fn set_default(meta: &syn::meta::ParseNestedMeta, out: &mut FieldAttrs) -> Result<(), syn::Error> {
    out.default = Some(meta.value()?.parse()?);
    Ok(())
}

/// Records the `merge_strategy`, reporting an unknown name at its own span.
fn set_merge_strategy(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<(), syn::Error> {
    let name = lit_str(meta, "merge_strategy")?;
    out.merge_strategy = Some(MergeStrategy::parse(&name.value(), name.span())?);
    Ok(())
}

/// Records whether `cli_default_as_absent` is enabled.
fn set_cli_default_as_absent(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<(), syn::Error> {
    out.cli_default_as_absent = parse_cli_default_as_absent(meta)?;
    Ok(())
}

/// Records the disclosure classification of a profile-protected field.
///
/// A second annotation is rejected rather than overwritten: a field with two
/// classifications would have no single disclosure rule to apply.
fn set_profile_field(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<(), syn::Error> {
    let value = lit_str(meta, "profile_field")?;
    if out.profile_field.is_some() {
        return Err(syn::Error::new(
            value.span(),
            "duplicate profile_field annotation; a field has exactly one \
disclosure classification",
        ));
    }
    let label = ProfileFieldLabel::parse(&value)?;
    out.profile_field = Some(ProfileFieldAnnotation {
        label,
        span: value.span(),
    });
    Ok(())
}

/// Parses the optional boolean value of `cli_default_as_absent`.
///
/// A bare `cli_default_as_absent` enables the behaviour; an explicit
/// `cli_default_as_absent = <bool>` sets it directly.
fn parse_cli_default_as_absent(meta: &syn::meta::ParseNestedMeta) -> Result<bool, syn::Error> {
    if meta.input.peek(Token![=]) {
        Ok(meta.value()?.parse::<syn::LitBool>()?.value)
    } else {
        Ok(true)
    }
}
