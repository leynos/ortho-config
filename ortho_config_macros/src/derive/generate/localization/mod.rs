//! Identifier generation for `OrthoConfigLocalization` derive emission.
//!
//! This pass converts a deriving struct's fields and base into a model whose
//! identifiers agree with `ortho_config::message_id_for`.

mod identifier;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use heck::ToKebabCase;
use proc_macro2::Span;
use syn::Ident;

use crate::derive::parse::{FieldAttrs, StructAttrs, clap_arg_id, clap_field_is_flattened};

pub(crate) use identifier::{join_identifier, normalize_segment};

#[derive(Debug, Clone)]
pub(crate) struct LocalizationBase(String);

impl LocalizationBase {
    fn normalize(&self, span: Span) -> syn::Result<FluentSegments> {
        self.0
            .split('.')
            .map(|segment| normalize_segment(segment, span))
            .collect::<Result<Vec<_>, _>>()
            .map(FluentSegments)
    }
}

impl AsRef<str> for LocalizationBase {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClapArgId(String);

impl ClapArgId {
    fn normalize(&self, span: Span) -> syn::Result<FluentSegments> {
        self.0
            .split('.')
            .map(|segment| normalize_segment(segment, span))
            .collect::<Result<Vec<_>, _>>()
            .map(FluentSegments)
    }
}

impl AsRef<str> for ClapArgId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FluentSegments(Vec<String>);

impl FluentSegments {
    fn ensure_leading_ascii_letter(&self, span: Span) -> syn::Result<()> {
        let Some(first) = self.0.first() else {
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

    fn with_segment(&self, segment: impl Into<String>) -> Self {
        let mut segments = self.0.clone();
        segments.push(segment.into());
        Self(segments)
    }

    fn with_suffix(&self, suffix: MessageSuffix) -> Self {
        self.with_segment(suffix.as_ref())
    }

    fn join(&self, span: Span) -> syn::Result<FluentMessageId> {
        join_identifier(&self.0, span).map(FluentMessageId)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct FluentMessageId(String);

impl AsRef<str> for FluentMessageId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy)]
enum MessageSuffix {
    About,
    LongAbout,
    Usage,
    Version,
    LongVersion,
    AfterHelp,
    AfterLongHelp,
    Args,
    Help,
    LongHelp,
    ValueName,
}

impl AsRef<str> for MessageSuffix {
    fn as_ref(&self) -> &str {
        match self {
            Self::About => "about",
            Self::LongAbout => "long_about",
            Self::Usage => "usage",
            Self::Version => "version",
            Self::LongVersion => "long_version",
            Self::AfterHelp => "after_help",
            Self::AfterLongHelp => "after_long_help",
            Self::Args => "args",
            Self::Help => "help",
            Self::LongHelp => "long_help",
            Self::ValueName => "value_name",
        }
    }
}

impl From<MessageSuffix> for String {
    fn from(value: MessageSuffix) -> Self {
        value.as_ref().to_owned()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ArgIdsModel {
    pub name: ClapArgId,
    pub help_id: FluentMessageId,
    pub long_help_id: FluentMessageId,
    pub value_name_id: FluentMessageId,
}

#[derive(Debug, Clone)]
#[expect(
    clippy::struct_field_names,
    reason = "each field is a distinct identifier constant; the `_id` postfix is the point"
)]
pub(crate) struct CommandIds {
    pub about_id: FluentMessageId,
    pub long_about_id: FluentMessageId,
    pub usage_id: FluentMessageId,
    pub version_id: FluentMessageId,
    pub long_version_id: FluentMessageId,
    pub after_help_id: FluentMessageId,
    pub after_long_help_id: FluentMessageId,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalizationIds {
    pub base: LocalizationBase,
    pub command: CommandIds,
    pub args: Vec<ArgIdsModel>,
}

/// Resolves the dotted catalogue base for a deriving struct (Decision D-5).
///
/// Precedence: `#[ortho_config(localization_base = "…")]`, then the docs app
/// name resolution (`discovery.app_name` if present, else the derive's default
/// app name), matching `generate::docs::sections::resolve_app_name`.
fn resolve_base(struct_attrs: &StructAttrs, ident: &Ident) -> LocalizationBase {
    LocalizationBase(
        struct_attrs
            .localization_base
            .clone()
            .unwrap_or_else(|| super::docs::sections::resolve_app_name(struct_attrs, ident)),
    )
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

fn lit(value: &FluentMessageId) -> syn::LitStr {
    syn::LitStr::new(value.as_ref(), Span::call_site())
}

fn normalize_base(base: &LocalizationBase, span: Span) -> syn::Result<FluentSegments> {
    base.normalize(span)
}

fn check_leading_letter(base_segments: &FluentSegments, span: Span) -> syn::Result<()> {
    base_segments.ensure_leading_ascii_letter(span)
}

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

/// Joins base segments plus extra segments into one identifier.
fn composed_id(
    base: &FluentSegments,
    suffix: MessageSuffix,
    span: Span,
) -> syn::Result<FluentMessageId> {
    base.with_suffix(suffix).join(span)
}

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

        let arg_id = ClapArgId(
            clap_arg_id(field)?.unwrap_or_else(|| name_ident.to_string().to_kebab_case()),
        );
        let (normalised, arg_segments) = normalise_arg_id(&arg_id, name_ident.span())?;

        if let Some((first_span, first_name)) = seen.get(&normalised) {
            let mut err = syn::Error::new_spanned(
                name_ident,
                format!(
                    "duplicate localized argument id '{}' for field '{first_name}' and '{name_ident}'; rename the field or set `#[arg(id = \"…\")]`",
                    normalised.as_ref(),
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

/// Normalises a (possibly dotted) clap argument id into its joined form plus
/// its per-segment parts. Mirrors the runtime `message_id_for` suffix handling:
/// `args.<arg_id>.help` is split on `.`, with each segment normalised.
fn normalise_arg_id(
    arg_id: &ClapArgId,
    span: Span,
) -> syn::Result<(FluentMessageId, FluentSegments)> {
    let segments = arg_id.normalize(span)?;
    let joined = segments.join(span)?;
    Ok((joined, segments))
}

/// Builds an argument identifier: base + `args` + arg segments + suffix.
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
