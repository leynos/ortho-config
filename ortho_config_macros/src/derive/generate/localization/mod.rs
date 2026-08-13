//! Identifier generation for `OrthoConfigLocalization` derive emission.
//!
//! This pass converts a deriving struct's resolved base and field list into a
//! [`LocalizationIds`] model. Milestone 3 emits the trait implementation from
//! this model; Milestone 4 reuses the same identifiers for the docs IR;
//! Milestone 5 serialises the model into the opt-in JSON artefact. The
//! identifier convention here is the §4.1 grammar implemented by
//! `ortho_config::message_id_for`, and the agreement is locked by the
//! dev-dependency-cycle property tests plus the cross-crate agreement tests.

mod identifier;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use heck::ToKebabCase;
use proc_macro2::Span;
use syn::Ident;

use crate::derive::parse::{FieldAttrs, StructAttrs, clap_arg_id, clap_field_is_flattened};

pub(crate) use identifier::{join_identifier, normalize_segment};

/// Compile-time Fluent identifiers for one derived argument.
#[derive(Debug, Clone)]
pub(crate) struct ArgIdsModel {
    /// The argument's clap id (explicit override or kebab-cased field name).
    pub name: String,
    /// Identifier for `help`.
    pub help_id: String,
    /// Identifier for `long_help`.
    pub long_help_id: String,
    /// Identifier for the value name placeholder.
    pub value_name_id: String,
}

/// Compile-time Fluent identifiers for the deriving command.
///
/// Every member is a separate identifier constant named after the runtime
/// suffix it represents; the shared `_id` postfix is intentional and
/// meaningful (each is a Fluent message id, not a generic field).
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_field_names,
    reason = "each field is a distinct identifier constant; the `_id` postfix is the point"
)]
pub(crate) struct CommandIds {
    pub about_id: String,
    pub long_about_id: String,
    pub usage_id: String,
    pub version_id: String,
    pub long_version_id: String,
    pub after_help_id: String,
    pub after_long_help_id: String,
}

/// Identifier model for a derived command-line surface.
#[derive(Debug, Clone)]
pub(crate) struct LocalizationIds {
    /// Dotted catalogue base (e.g. `hello_world.cli`).
    pub base: String,
    pub command: CommandIds,
    /// One entry per own, non-subcommand, non-`skip_cli`, non-flattened field.
    pub args: Vec<ArgIdsModel>,
}

/// Resolves the dotted catalogue base for a deriving struct (Decision D-5).
///
/// Precedence: `#[ortho_config(localization_base = "…")]`, then the docs app
/// name resolution (`discovery.app_name` if present, else the derive's default
/// app name), matching `generate::docs::sections::resolve_app_name`.
fn resolve_base(struct_attrs: &StructAttrs, ident: &Ident) -> String {
    struct_attrs
        .localization_base
        .clone()
        .unwrap_or_else(|| super::docs::sections::resolve_app_name(struct_attrs, ident))
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

    let base_lit = syn::LitStr::new(&model.base, Span::call_site());
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
            let name = lit(&arg.name);
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

fn lit(value: &str) -> syn::LitStr {
    syn::LitStr::new(value, Span::call_site())
}

/// Normalises the dotted base into segments, rejecting unrepresentable paths.
fn normalize_base(base: &str, span: Span) -> syn::Result<Vec<String>> {
    let mut segments = Vec::new();
    for segment in base.split('.') {
        segments.push(normalize_segment(segment, span)?);
    }
    Ok(segments)
}

/// Rejects a command path whose joined identifier would not start with a
/// letter, matching the runtime `message_id_for` panic condition.
fn check_leading_letter(base_segments: &[String], span: Span) -> syn::Result<()> {
    let Some(first) = base_segments.first() else {
        return Err(syn::Error::new(
            span,
            "Fluent identifier must start with an ASCII letter: missing command root",
        ));
    };
    let Some(ch) = first.chars().next() else {
        return Err(syn::Error::new(
            span,
            "Fluent identifier must start with an ASCII letter: empty command root",
        ));
    };
    if ch.is_ascii_alphabetic() {
        Ok(())
    } else {
        Err(syn::Error::new(
            span,
            format!("Fluent identifier must start with an ASCII letter: {first:?}"),
        ))
    }
}

fn build_command_ids(base_segments: &[String]) -> syn::Result<CommandIds> {
    let span = Span::call_site();
    Ok(CommandIds {
        about_id: composed_id(base_segments, &["about"], span)?,
        long_about_id: composed_id(base_segments, &["long_about"], span)?,
        usage_id: composed_id(base_segments, &["usage"], span)?,
        version_id: composed_id(base_segments, &["version"], span)?,
        long_version_id: composed_id(base_segments, &["long_version"], span)?,
        after_help_id: composed_id(base_segments, &["after_help"], span)?,
        after_long_help_id: composed_id(base_segments, &["after_long_help"], span)?,
    })
}

/// Joins base segments plus extra segments into one identifier.
fn composed_id(base: &[String], tail: &[&str], span: Span) -> syn::Result<String> {
    let mut segments = base.to_vec();
    segments.extend(tail.iter().map(|s| (*s).to_owned()));
    join_identifier(&segments, span)
}

/// The fixed `args` segment inserted before an argument's identifier parts.
const ARGS_SEGMENT: &str = "args";

fn build_arg_models(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
    base_segments: &[String],
) -> syn::Result<Vec<ArgIdsModel>> {
    let mut args = Vec::new();
    let mut seen: HashMap<String, (Span, String)> = HashMap::new();

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

        let arg_id = clap_arg_id(field)?.unwrap_or_else(|| name_ident.to_string().to_kebab_case());
        let (normalised, arg_segments) = normalise_arg_id(&arg_id, name_ident.span())?;

        if let Some((first_span, first_name)) = seen.get(&normalised) {
            let mut err = syn::Error::new_spanned(
                name_ident,
                format!(
                    "duplicate localized argument id '{normalised}' for field '{first_name}' and '{name_ident}'; rename the field or set `#[arg(id = \"…\")]`",
                ),
            );
            err.combine(syn::Error::new(*first_span, "first defined here"));
            return Err(err);
        }
        seen.insert(
            normalised.clone(),
            (name_ident.span(), name_ident.to_string()),
        );

        args.push(ArgIdsModel {
            name: arg_id,
            help_id: arg_id_composed(base_segments, &arg_segments, "help", name_ident.span())?,
            long_help_id: arg_id_composed(
                base_segments,
                &arg_segments,
                "long_help",
                name_ident.span(),
            )?,
            value_name_id: arg_id_composed(
                base_segments,
                &arg_segments,
                "value_name",
                name_ident.span(),
            )?,
        });
    }

    Ok(args)
}

/// Normalises a (possibly dotted) clap argument id into its joined form plus
/// its per-segment parts. Mirrors the runtime `message_id_for` suffix handling:
/// `args.<arg_id>.help` is split on `.`, with each segment normalised.
fn normalise_arg_id(arg_id: &str, span: Span) -> syn::Result<(String, Vec<String>)> {
    let segments = arg_id
        .split('.')
        .map(|segment| normalize_segment(segment, span))
        .collect::<Result<Vec<_>, _>>()?;
    let joined = join_identifier(&segments, span)?;
    Ok((joined, segments))
}

/// Builds an argument identifier: base + `args` + arg segments + suffix.
fn arg_id_composed(
    base: &[String],
    arg_segments: &[String],
    suffix: &str,
    span: Span,
) -> syn::Result<String> {
    let mut segments = base.to_vec();
    segments.push(ARGS_SEGMENT.to_owned());
    segments.extend(arg_segments.iter().cloned());
    segments.push(suffix.to_owned());
    join_identifier(&segments, span)
}
