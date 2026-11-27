//! Token assembly for generated documentation implementations.
//!
//! The generated helper and trait implementation are emitted from pre-computed
//! fragments so each impl stays a compact declarative block instead of
//! repeating the whole metadata construction.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use super::fields;
use super::projection::ProjectionTokens;
use super::{
    AppName, DocsArgs, behaviour, build_profiles_meta, build_subcommands_metadata,
    option_string_tokens, output, renderer, sections,
};

/// Names the helper's mount-path parameter, marking it unused when no
/// generated expression refers to it.
///
/// The trait's default identifiers are composed from the mount path, so a
/// struct with explicit `about_id` and `synopsis_id` overrides, no localized
/// arguments, and no subcommand selector never reads the path. Naming the
/// parameter `_command_path` keeps the generated code warning-free without a
/// blanket `unused_variables` allowance that would hide a future defect.
fn command_path_parameter(args: &DocsArgs<'_>) -> TokenStream {
    if metadata_uses_command_path(args) {
        quote! { command_path }
    } else {
        quote! { _command_path }
    }
}

/// Reports whether any generated expression reads the mount path.
fn metadata_uses_command_path(args: &DocsArgs<'_>) -> bool {
    uses_section_path(args) || uses_subcommands(args) || uses_localized_field_ids(args)
}

/// Reports whether the about or synopsis identifier defaults to the path.
const fn uses_section_path(args: &DocsArgs<'_>) -> bool {
    let doc = &args.struct_attrs.doc;
    doc.about_id.is_none() || doc.synopsis_id.is_none()
}

/// Reports whether the struct nests a subcommand selector.
fn uses_subcommands(args: &DocsArgs<'_>) -> bool {
    args.field_attrs
        .iter()
        .any(|attrs| attrs.cli.is_subcommand())
}

/// Reports whether a documented field composes its identifier from the path.
fn uses_localized_field_ids(args: &DocsArgs<'_>) -> bool {
    args.fields
        .iter()
        .zip(args.field_attrs)
        .filter(|(_, attrs)| !attrs.cli.is_subcommand() && !has_explicit_field_ids(attrs))
        .filter_map(|(field, _)| field.ident.as_ref())
        .any(|ident| has_localized_arg(args, &ident.to_string()))
}

/// Reports whether both field identifier overrides are written explicitly.
const fn has_explicit_field_ids(attrs: &crate::derive::parse::FieldAttrs) -> bool {
    attrs.doc.help_id.is_some() && attrs.doc.long_help_id.is_some()
}

/// Reports whether a field is present in the captured localization argument ids.
fn has_localized_arg(args: &DocsArgs<'_>, field_name: &str) -> bool {
    args.localization_ids
        .args
        .iter()
        .any(|arg| arg.field_name == field_name)
}

/// Pre-computed token fragments shared by the generated helper and trait impl.
///
/// Keeping them in one owner lets each emitted impl stay a compact declarative
/// block instead of repeating the whole metadata construction.
pub(super) struct DocTokens {
    /// Resolved crate path for generated references.
    krate: TokenStream,
    /// Application name literal recorded in the IR.
    app_name_lit: syn::LitStr,
    /// Section metadata initializer.
    headings: TokenStream,
    /// Windows metadata initializer.
    windows: TokenStream,
    /// One field record per documented field.
    records: Vec<TokenStream>,
    /// Recursive subcommand metadata expression.
    subcommands: TokenStream,
    /// Behaviour metadata initializer.
    behaviour: TokenStream,
    /// Renderer metadata initializer.
    renderer: TokenStream,
    /// Structured-output metadata initializer.
    output: TokenStream,
    /// Command-level about identifier expression.
    about_id: TokenStream,
    /// Binary name expression.
    bin_name_tokens: TokenStream,
    /// Synopsis identifier expression.
    synopsis_tokens: TokenStream,
    /// Profile selection metadata initializer.
    profiles_tokens: TokenStream,
    /// Helper parameter name, marked unused when nothing references it.
    command_path_parameter: TokenStream,
    /// Helper parameter name, marked unused when nothing projects.
    subcommand_parameter: TokenStream,
    /// Environment-prefix local, when one is needed.
    env_prefix_local: TokenStream,
    /// File-prefix local, when one is needed.
    file_prefix_local: TokenStream,
    /// Whether the struct declares an explicit localization base.
    has_explicit_base: bool,
    /// Explicit clap command name expression.
    command_name: TokenStream,
}

impl DocTokens {
    /// Resolves every component the generated implementations need.
    pub(super) fn build(args: &DocsArgs<'_>) -> syn::Result<Self> {
        let krate = args.krate;
        let app_name = sections::resolve_app_name(args.struct_attrs, args.ident);
        let app_name_value = AppName::new(app_name);
        let headings =
            sections::build_sections_metadata(&app_name_value, args.struct_attrs, krate)?;
        let windows = sections::build_windows_metadata(args.struct_attrs, krate);
        let fields::FieldsMetadata {
            records,
            needs_env_projection,
            needs_file_projection,
        } = fields::build_fields_metadata(&fields::FieldDocArgs {
            app_name: &app_name_value,
            prefix: args.struct_attrs.prefix.as_deref(),
            fields: args.fields,
            field_attrs: args.field_attrs,
            serde_rename_all: args.serde_rename_all,
            cli_fields: args.cli_fields,
            localization_ids: args.localization_ids,
            clap_command_name: args.clap_command_name,
            krate,
        })?;
        let subcommands = build_subcommands_metadata(args)?;
        let behaviour = behaviour::build_behaviour_metadata(&args.struct_attrs.doc, krate);
        let renderer = renderer::build_renderer_metadata(&args.struct_attrs.doc, krate);
        let output = output::build_output_metadata(&args.struct_attrs.doc, krate);
        let projection = ProjectionTokens::new(
            args.struct_attrs.prefix.as_deref(),
            needs_env_projection,
            needs_file_projection,
        );
        let command_name = args.clap_command_name.map_or_else(
            || quote! { None },
            |name| {
                let lit = syn::LitStr::new(name, proc_macro2::Span::call_site());
                quote! { Some(#lit) }
            },
        );

        Ok(Self {
            krate: krate.clone(),
            app_name_lit: syn::LitStr::new(&app_name_value, proc_macro2::Span::call_site()),
            headings,
            windows,
            records,
            subcommands,
            behaviour,
            renderer,
            output,
            about_id: sections::about_id_tokens(&args.struct_attrs.doc, krate),
            bin_name_tokens: option_string_tokens(args.struct_attrs.doc.bin_name.as_deref()),
            synopsis_tokens: sections::synopsis_id_tokens(&args.struct_attrs.doc, krate),
            profiles_tokens: build_profiles_meta(args, krate),
            command_path_parameter: command_path_parameter(args),
            subcommand_parameter: projection.subcommand_parameter(),
            env_prefix_local: projection.env_prefix_local(),
            file_prefix_local: projection.file_prefix_local(),
            has_explicit_base: args.struct_attrs.localization_base.is_some(),
            command_name,
        })
    }

    /// Emits the private helper that builds one mount's metadata.
    pub(super) fn render_helper(&self, ident: &Ident) -> TokenStream {
        let Self {
            krate,
            app_name_lit,
            headings,
            windows,
            records,
            subcommands,
            behaviour,
            renderer,
            output,
            about_id,
            bin_name_tokens,
            synopsis_tokens,
            profiles_tokens,
            command_path_parameter,
            subcommand_parameter,
            env_prefix_local,
            file_prefix_local,
            ..
        } = self;
        quote! {
            impl #ident {
                /// Builds documentation metadata for the command mounted at
                /// `command_path`.
                ///
                /// A present `subcommand` names the mount and projects
                /// environment and file metadata into the namespace the
                /// subcommand merge reads.
                #[doc(hidden)]
                fn __ortho_config_doc_metadata(
                    #command_path_parameter: &[String],
                    #subcommand_parameter: Option<&str>,
                ) -> #krate::docs::DocMetadata {
                    #env_prefix_local
                    #file_prefix_local
                    #krate::docs::DocMetadata {
                        ir_version: #krate::docs::ORTHO_DOCS_IR_VERSION.to_string(),
                        app_name: #app_name_lit.to_string(),
                        bin_name: #bin_name_tokens,
                        about_id: #about_id,
                        synopsis_id: #synopsis_tokens,
                        sections: #headings,
                        fields: vec![ #( #records ),* ],
                        subcommands: #subcommands,
                        windows: #windows,
                        behaviour: #behaviour,
                        renderer: #renderer,
                        output: #output,
                        profiles: #profiles_tokens,
                    }
                    .into_redacted_for_output()
                }
            }
        }
    }

    /// Emits the public `OrthoConfigDocs` implementation.
    pub(super) fn render_trait_impl(&self, ident: &Ident) -> TokenStream {
        let Self {
            krate,
            has_explicit_base,
            command_name,
            ..
        } = self;
        quote! {
            impl #krate::docs::OrthoConfigDocs for #ident {
                fn get_doc_metadata() -> #krate::docs::DocMetadata {
                    let command_path =
                        <Self as #krate::OrthoConfigLocalization>::LOCALIZATION_BASE
                            .split('.')
                            .map(String::from)
                            .collect::<Vec<_>>();
                    Self::__ortho_config_doc_metadata(&command_path, None)
                }

                fn get_doc_metadata_for_path(
                    command_path: &[String],
                ) -> #krate::docs::DocMetadata {
                    if command_path.is_empty() {
                        return Self::get_doc_metadata();
                    }
                    let subcommand = command_path.last().map(String::as_str);
                    Self::__ortho_config_doc_metadata(command_path, subcommand)
                }

                fn get_doc_metadata_for_subcommand(
                    parent_path: &[String],
                    name: &str,
                ) -> #krate::docs::DocMetadata {
                    let command_path = if parent_path.is_empty() {
                        if #has_explicit_base {
                            <Self as #krate::OrthoConfigLocalization>::LOCALIZATION_BASE
                                .split('.')
                                .map(String::from)
                                .collect::<Vec<_>>()
                        } else {
                            vec![name.to_owned()]
                        }
                    } else {
                        let mut path = parent_path.to_vec();
                        path.push(name.to_owned());
                        path
                    };
                    let mount_name = {
                        use #krate::docs::MountCommandName as _;
                        let explicit: Option<&str> = #command_name;
                        explicit
                            .map(str::to_owned)
                            .or_else(|| {
                                let probe = core::marker::PhantomData::<&Self>;
                                (&&probe).mount_command_name()
                            })
                            .unwrap_or_else(|| name.to_owned())
                    };
                    let mut metadata = Self::__ortho_config_doc_metadata(
                        &command_path,
                        Some(mount_name.as_str()),
                    );
                    metadata.app_name = name.to_owned();
                    metadata
                }
            }
        }
    }
}
