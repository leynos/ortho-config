//! Identifier generation for `OrthoConfigLocalization` derive emission.
//!
//! This pass converts a deriving struct's fields and base into a model whose
//! identifiers agree with `ortho_config::message_id_for`.

mod artefact;
mod identifier;
mod model;
pub(crate) mod suffix;
#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use heck::ToKebabCase;
use proc_macro2::Span;
use syn::Ident;

use crate::derive::parse::{FieldAttrs, StructAttrs, clap_arg_id, clap_field_is_flattened};

pub(crate) use model::{
    ArgIdsModel, ClapArgId, CommandIds, FluentMessageId, FluentSegments, LocalizationBase,
    LocalizationIds,
};
use suffix::MessageSuffix;

/// Resolves the dotted catalogue base for a deriving struct (Decision D-5).
///
/// Precedence: `#[ortho_config(localization_base = "…")]`, then
/// `discovery.app_name`, then the kebab-cased derive default app name.
fn resolve_base(struct_attrs: &StructAttrs, ident: &Ident) -> LocalizationBase {
    LocalizationBase(struct_attrs.localization_base.clone().unwrap_or_else(|| {
        struct_attrs
            .discovery
            .as_ref()
            .and_then(|discovery| discovery.app_name.clone())
            .unwrap_or_else(|| {
                crate::derive::build::default_app_name(struct_attrs, ident).to_kebab_case()
            })
    }))
}

/// Builds the identifier model for a deriving struct.
///
/// # Errors
///
/// Returns spanned errors for invalid base segments, invalid or duplicate
/// argument ids, and command paths that do not begin with an ASCII letter
/// (mirroring the runtime panics of `message_id_for` as compile-time
/// diagnostics).
pub(crate) fn generate_localization_ids(
    struct_attrs: &StructAttrs,
    ident: &Ident,
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> syn::Result<LocalizationIds> {
    debug_assert_eq!(fields.len(), field_attrs.len());

    let base = resolve_base(struct_attrs, ident);
    let base_segments = normalize_base(&base, ident.span())?;
    check_leading_letter(&base_segments, ident.span())?;
    let args = build_arg_models(fields, field_attrs, &base_segments)?;

    Ok(LocalizationIds {
        base,
        command: build_command_ids(&base_segments)?,
        args,
    })
}

/// Emits the `OrthoConfigLocalization` implementation tokens for a deriving
/// struct (Milestone 3). All constant values are literals computed at
/// expansion time from the model, so derive output and runtime lookups agree
/// byte-for-byte.
pub(crate) fn emit_localization_impl(
    model: &LocalizationIds,
    ident: &Ident,
    krate: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    use quote::quote;

    let base_lit = syn::LitStr::new(model.base.as_ref(), Span::call_site());
    let about = lit(&model.command.about_id);
    let long_about = lit(&model.command.long_about_id);
    let usage = lit(&model.command.usage_id);
    let version = lit(&model.command.version_id);
    let long_version = lit(&model.command.long_version_id);
    let after_help = lit(&model.command.after_help_id);
    let after_long_help = lit(&model.command.after_long_help_id);

    let arg_entries = model
        .args
        .iter()
        .map(|arg| {
            let name = syn::LitStr::new(arg.name.as_ref(), Span::call_site());
            let help = lit(&arg.help_id);
            let long_help = lit(&arg.long_help_id);
            let value_name = lit(&arg.value_name_id);
            quote! {
                #krate::ArgLocalizationIds {
                    name: #name,
                    help_id: #help,
                    long_help_id: #long_help,
                    value_name_id: #value_name,
                }
            }
        })
        .collect::<Vec<_>>();

    quote! {
        #[automatically_derived]
        impl #krate::OrthoConfigLocalization for #ident {
            const LOCALIZATION_BASE: &'static str = #base_lit;
            const ABOUT_ID: &'static str = #about;
            const LONG_ABOUT_ID: &'static str = #long_about;
            const USAGE_ID: &'static str = #usage;
            const VERSION_ID: &'static str = #version;
            const LONG_VERSION_ID: &'static str = #long_version;
            const AFTER_HELP_ID: &'static str = #after_help;
            const AFTER_LONG_HELP_ID: &'static str = #after_long_help;
            const ARG_IDS: &'static [#krate::ArgLocalizationIds] = &[ #( #arg_entries ),* ];
        }
    }
}

/// Emits the optional identifier artefact after model validation succeeds.
pub(crate) fn emit_identifier_artefact(model: &LocalizationIds, ident: &Ident) -> syn::Result<()> {
    artefact::emit(model, ident, ident.span())
}

/// Converts a domain message id into a literal for generated code.
fn lit(value: &FluentMessageId) -> syn::LitStr {
    syn::LitStr::new(value.as_ref(), Span::call_site())
}

/// Normalizes the resolved localization base.
fn normalize_base(base: &LocalizationBase, span: Span) -> syn::Result<FluentSegments> {
    base.normalize(span)
}

/// Validates the root segment required by Fluent identifiers.
fn check_leading_letter(base_segments: &FluentSegments, span: Span) -> syn::Result<()> {
    base_segments.ensure_leading_ascii_letter(span)
}

/// Builds all fixed command-level message identifiers.
fn build_command_ids(base_segments: &FluentSegments) -> syn::Result<CommandIds> {
    let span = Span::call_site();
    Ok(CommandIds {
        about_id: composed_id(base_segments, MessageSuffix::About, span)?,
        long_about_id: composed_id(base_segments, MessageSuffix::LongAbout, span)?,
        usage_id: composed_id(base_segments, MessageSuffix::Usage, span)?,
        version_id: composed_id(base_segments, MessageSuffix::Version, span)?,
        long_version_id: composed_id(base_segments, MessageSuffix::LongVersion, span)?,
        after_help_id: composed_id(base_segments, MessageSuffix::AfterHelp, span)?,
        after_long_help_id: composed_id(base_segments, MessageSuffix::AfterLongHelp, span)?,
    })
}

/// Joins the base with one fixed command-level suffix.
fn composed_id(
    base: &FluentSegments,
    suffix: MessageSuffix,
    span: Span,
) -> syn::Result<FluentMessageId> {
    base.with_suffix(suffix).join(span)
}

/// Builds and validates identifiers for eligible struct fields.
fn build_arg_models(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
    base_segments: &FluentSegments,
) -> syn::Result<Vec<ArgIdsModel>> {
    let mut args = Vec::new();
    let mut seen: HashMap<FluentMessageId, (Span, String)> = HashMap::new();

    for (field, attrs) in fields.iter().zip(field_attrs) {
        if attrs.is_subcommand {
            continue;
        }
        if attrs.skip_cli || clap_field_is_flattened(field)? {
            continue;
        }
        let Some(name_ident) = &field.ident else {
            return Err(syn::Error::new_spanned(
                field,
                "unnamed (tuple) fields are not supported for localization derive",
            ));
        };

        // Clap's derived arg id is the raw field name: `clap_derive` emits
        // `Name::Derived(ident)` as `ident.unraw().to_string()` and only
        // kebab-cases the *long flag*, not the id. Deriving from the
        // kebab-cased spelling here would therefore emit a key that no
        // runtime lookup ever requests for an underscored field.
        let arg_id = ClapArgId(clap_arg_id(field)?.unwrap_or_else(|| name_ident.to_string()));
        let (normalized, arg_segments) = normalize_arg_id(&arg_id, name_ident.span())?;

        if let Some((first_span, first_name)) = seen.get(&normalized) {
            let mut err = syn::Error::new_spanned(
                name_ident,
                format!(
                    "duplicate localized argument id '{}' for field '{first_name}' and '{name_ident}'; rename the field or set `#[arg(id = \"…\")]`",
                    normalized.as_ref(),
                ),
            );
            err.combine(syn::Error::new(*first_span, "first defined here"));
            return Err(err);
        }
        seen.insert(
            normalized.clone(),
            (name_ident.span(), name_ident.to_string()),
        );

        args.push(ArgIdsModel {
            field_name: name_ident.to_string(),
            name: arg_id,
            help_id: arg_id_composed(
                base_segments,
                &arg_segments,
                MessageSuffix::Help,
                name_ident.span(),
            )?,
            long_help_id: arg_id_composed(
                base_segments,
                &arg_segments,
                MessageSuffix::LongHelp,
                name_ident.span(),
            )?,
            value_name_id: arg_id_composed(
                base_segments,
                &arg_segments,
                MessageSuffix::ValueName,
                name_ident.span(),
            )?,
        });
    }

    Ok(args)
}

/// Normalizes a (possibly dotted) clap argument id into its joined form plus
/// its per-segment parts. Mirrors the runtime `message_id_for` suffix handling:
/// `args.<arg_id>.help` is split on `.`, with each segment normalized.
///
/// The returned key is used only to detect collisions between argument ids in
/// one derived struct, so it is joined without the command-root check. Here
/// the argument is a *fragment* of the final identifier, not a root: clap
/// treats `#[arg(id = "123")]` as an argument name, and the runtime accepts
/// the complete `acme-args-123-help` because it validates only the first
/// character of the joined id. Applying the standalone-root rule here would
/// reject an identifier the runtime is happy to build.
fn normalize_arg_id(
    arg_id: &ClapArgId,
    span: Span,
) -> syn::Result<(FluentMessageId, FluentSegments)> {
    let segments = arg_id.normalize(span)?;
    Ok((FluentMessageId(segments.0.join("-")), segments))
}

/// Builds an argument identifier: base + `args` + arg segments + suffix.
/// Joins a base, argument path, and suffix into one argument id.
fn arg_id_composed(
    base: &FluentSegments,
    arg_segments: &FluentSegments,
    suffix: MessageSuffix,
    span: Span,
) -> syn::Result<FluentMessageId> {
    let mut segments = base.with_suffix(MessageSuffix::Args);
    segments.0.extend(arg_segments.0.iter().cloned());
    segments.with_suffix(suffix).join(span)
}
